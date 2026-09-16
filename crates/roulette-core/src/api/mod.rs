//! Public headless API (Phase 7, TASK-042): the `Engine` facade.
//!
//! Every mutation is a [`Command`] applied through [`Engine::apply`]; each call
//! returns the `EngineEvent`s it produced (PAT-004 event-sourced facade) and
//! errors via [`EngineError`]. Undo is snapshot-based (REQ-009). All physics
//! spins run the real 120 Hz simulator (REQ-002) on per-spin derived RNG
//! substreams (GUD-004), and prediction is a true dry-run of the same stream
//! (REQ-003 / DEC-001).

use std::sync::Arc;

use roulette_content::schema::SlotColor;

use crate::battle::state::{
    ActionError, BattleOutcome, BattlePhase, BattleState, SpinInput, SpinOutcome,
};
use crate::bets::BetType;
use crate::phys::{PhysicsModifiers, Simulator, WheelLayout};
use crate::rng::Rng;
use crate::run::state::{BattleResult, Difficulty, GameState, RunError};
use crate::wheel::{ColorLevels, WheelConfig};

pub mod error;
pub mod events;

pub use error::EngineError;
pub use events::EngineEvent;

/// A single player action, serializable for scripting (TASK-043/044).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    StartRun {
        difficulty: Difficulty,
    },
    DraftCard {
        card_id: String,
    },
    DraftWheel,
    FinishLoadout,
    PickNode {
        node_id: String,
    },
    PlayCard {
        hand_index: usize,
    },
    BuyDraw,
    PlaceBet {
        bet: BetType,
        amount: u16,
    },
    RemoveBet {
        bet: BetType,
        amount: u16,
    },
    ClearBets,
    Rebet,
    Sacrifice,
    /// Arm the prediction sector for the upcoming spin (§10.6).
    Predict,
    /// Spin the player's wheel and let the enemy answer; ends the round.
    Spin,
    Purchase {
        item_index: usize,
    },
    ForgeTake {
        op_index: usize,
    },
    ForgeReroll,
    EventChoose {
        choice_id: String,
    },
    /// Wheel customizer color level-up (§4.5) at the forge.
    BuyLevel {
        color: SlotColor,
    },
    /// §4.8 wheel customizer: edit a draft clone of the player's wheel, then
    /// `Save` (commit) or `Cancel` (discard). Only valid at the forge.
    Customize {
        op: CustomizeOp,
    },
    /// Undo the last applied command (snapshot restore, REQ-009).
    Undo,
}

/// One §4.8 customizer edit on the draft wheel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CustomizeOp {
    /// Cycle a slot's color through the §4 color ladder.
    CycleColor { slot: usize },
    /// Add a slot carrying `number` (uniqueness enforced, ≥2 slots kept).
    AddSlot { number: u32 },
    /// Remove a slot (the wheel must keep ≥2 slots).
    RemoveSlot { slot: usize },
    /// Set a slot's number (uniqueness enforced).
    SetNumber { slot: usize, number: u32 },
    /// Commit the draft wheel to the run.
    Save,
    /// Discard the draft wheel.
    Cancel,
}

/// Undoable engine state (serialized into the undo stack).
#[derive(serde::Serialize, serde::Deserialize)]
struct EngineSnapshot {
    run: Option<crate::run::state::RunState>,
    battle: Option<BattleState>,
    spin_index: u32,
    battle_index: u32,
    run_rng_calls: u32,
    effect_rng_calls: u32,
    draft_wheel: Option<crate::wheel::WheelConfig>,
}

/// Undo-stack depth cap (cheap Clone-based snapshots, REQ-009).
const UNDO_DEPTH: usize = 64;

/// Spin sampling mode for the §15 Monte Carlo tooling (TASK-045).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinSampling {
    /// Real 120 Hz deterministic physics simulation (REQ-002). Default.
    #[default]
    Physics,
    /// Uniform slot sampling with zero sim steps — statistically equivalent
    /// for bulk balance runs. Prediction is unavailable in this mode.
    Uniform,
}

/// Headless engine facade over the run + battle layers (TASK-042).
pub struct Engine {
    pub(crate) content: Arc<roulette_content::schema::Content>,
    seed: String,
    run: Option<crate::run::state::RunState>,
    battle: Option<BattleState>,
    prediction: Option<(usize, u8)>,
    /// Spin sampling mode (§15 tooling): `Physics` (default) runs the real
    /// 120 Hz sim; `Uniform` draws each ball's slot uniformly with zero sim
    /// steps — statistically equivalent for bulk Monte Carlo balance runs.
    /// Prediction is unavailable in `Uniform` mode (no dry-run exists).
    spin_sampling: SpinSampling,
    /// §4.8 customizer draft wheel (None unless a Customize session is open).
    draft_wheel: Option<crate::wheel::WheelConfig>,
    /// Label counters — every RNG child stream is derived by a deterministic
    /// label so identical command sequences replay identically (GUD-004).
    spin_index: u32,
    battle_index: u32,
    run_rng_calls: u32,
    effect_rng_calls: u32,
    /// Card database keyed by id for the effect DSL (built once from content).
    card_defs: std::collections::BTreeMap<String, roulette_content::schema::CardDef>,
    events: Vec<EngineEvent>,
    undo: Vec<EngineSnapshot>,
    /// Last spin's telemetry frames, side-channel only (feature `telemetry`,
    /// REQ-008). One entry per simulated wheel per spin. Cleared and refilled
    /// on every spin; never serialized into undo.
    #[cfg(feature = "telemetry")]
    spin_frames: Vec<(crate::battle::state::Side, SpinTelemetry)>,
    /// Last spin's physics sim events paired with their telemetry frame index
    /// (feature `telemetry`, §5.4 sound hooks). Side channel like
    /// `spin_frames`: never serialized into undo or the event log (REQ-008 —
    /// floats never enter `EngineEvent`s; this lives outside them).
    #[cfg(feature = "telemetry")]
    spin_sim_events: Vec<(crate::battle::state::Side, Vec<(u32, crate::phys::events::SimEvent)>)>,
}

/// Side-channel telemetry for one simulated wheel of the last spin
/// (feature `telemetry`). Self-describing: carries the slot geometry so the
/// visual layer (and TEST-010) can map final angles back to slots without
/// any other engine surface.
#[cfg(feature = "telemetry")]
pub struct SpinTelemetry {
    pub slot_count: u32,
    pub slot_width: f64,
    pub frames: Vec<crate::phys::simulator::TelemetryFrame>,
}

impl Engine {
    pub fn new(content: Arc<roulette_content::schema::Content>, seed: &str) -> Self {
        let card_defs = content.cards.iter().map(|c| (c.id.clone(), c.clone())).collect();
        Self {
            content,
            seed: seed.to_string(),
            run: None,
            battle: None,
            prediction: None,
            spin_sampling: SpinSampling::Physics,
            draft_wheel: None,
            spin_index: 0,
            battle_index: 0,
            run_rng_calls: 0,
            effect_rng_calls: 0,
            card_defs,
            events: Vec::new(),
            undo: Vec::new(),
            #[cfg(feature = "telemetry")]
            spin_frames: Vec::new(),
            #[cfg(feature = "telemetry")]
            spin_sim_events: Vec::new(),
        }
    }

    // -- read-only getters ---------------------------------------------------

    pub fn run(&self) -> Option<&crate::run::state::RunState> {
        self.run.as_ref()
    }

    pub fn battle(&self) -> Option<&BattleState> {
        self.battle.as_ref()
    }

    pub fn events(&self) -> &[EngineEvent] {
        &self.events
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Selects the spin sampling mode (§15 tooling; default `Physics`).
    pub fn set_spin_sampling(&mut self, mode: SpinSampling) {
        self.spin_sampling = mode;
    }

    pub fn spin_sampling(&self) -> SpinSampling {
        self.spin_sampling
    }

    /// Spin telemetry side channel (feature `telemetry`, TASK-008/REQ-008):
    /// fixed-dt (1/120 s) frames for the LAST spin, packed little-endian as
    /// `[u32 frame_count][u32 ball_count][u32 slot_count][f32 slot_width]`
    /// then per frame `[f32 wheel_angle, f32 ball0..ballN]`. `side` selects
    /// the player (0) or enemy (1) wheel. Returns `None` when the side was
    /// not simulated this spin (e.g. uniform fast path — the renderer falls
    /// back to a generic animation there).
    #[cfg(feature = "telemetry")]
    pub fn spin_telemetry_bytes(&self, side: u8) -> Option<Vec<u8>> {
        use crate::battle::state::Side;
        let want = match side {
            0 => Side::Player,
            _ => Side::Enemy,
        };
        let tele = &self.spin_frames.iter().find(|(s, _)| *s == want)?.1;
        if tele.frames.is_empty() {
            return None;
        }
        let ball_count = tele.frames[0].ball_angles.len();
        let mut out = Vec::with_capacity(12 + tele.frames.len() * 4 * (1 + ball_count));
        out.extend_from_slice(&(tele.frames.len() as u32).to_le_bytes());
        out.extend_from_slice(&(ball_count as u32).to_le_bytes());
        out.extend_from_slice(&tele.slot_count.to_le_bytes());
        out.extend_from_slice(&(tele.slot_width as f32).to_le_bytes());
        for f in &tele.frames {
            out.extend_from_slice(&(f.wheel_angle as f32).to_le_bytes());
            for &a in &f.ball_angles {
                out.extend_from_slice(&(a as f32).to_le_bytes());
            }
        }
        Some(out)
    }

    /// Last spin's physics sim events with telemetry frame indices, JSON
    /// encoded (feature `telemetry`, §5.4/§10.6 sound hooks): a JSON array of
    /// `{"frame": u32, "event": <SimEvent>}`. `side` selects the player (0)
    /// or enemy (1) wheel. `None` when the side was not simulated this spin
    /// (uniform fast path) or no events fired. Plain JSON keeps the JS glue
    /// dependency-free (SEC-002 pattern); this is a side channel — it never
    /// enters the engine event log (REQ-008).
    #[cfg(feature = "telemetry")]
    pub fn spin_sim_events_json(&self, side: u8) -> Option<String> {
        use crate::battle::state::Side;
        let want = match side {
            0 => Side::Player,
            _ => Side::Enemy,
        };
        let evs = &self.spin_sim_events.iter().find(|(s, _)| *s == want)?.1;
        if evs.is_empty() {
            return None;
        }
        #[derive(serde::Serialize)]
        struct Timed<'a> {
            frame: u32,
            event: &'a crate::phys::events::SimEvent,
        }
        let timed: Vec<Timed> = evs.iter().map(|(f, e)| Timed { frame: *f, event: e }).collect();
        serde_json::to_string(&timed).ok()
    }

    // -- command dispatch ----------------------------------------------------

    fn next_run_rng(&mut self) -> Rng {
        let r = self.run_rng().derive(&format!("run:{}", self.run_rng_calls));
        self.run_rng_calls += 1;
        r
    }

    fn next_effect_rng(&mut self) -> Rng {
        let r = self.run_rng().derive(&format!("effects:{}", self.effect_rng_calls));
        self.effect_rng_calls += 1;
        r
    }

    fn run_rng(&self) -> Rng {
        Rng::from_string_seed(&self.seed)
    }

    fn run_mut(&mut self) -> Result<&mut crate::run::state::RunState, EngineError> {
        self.run.as_mut().ok_or(EngineError::NoRun)
    }

    /// Applies one command, returning the events it produced. On error the
    /// engine state is restored to the pre-command snapshot.
    pub fn apply(&mut self, cmd: &Command) -> Result<Vec<EngineEvent>, EngineError> {
        let snapshot = cmd.is_undoable().then(|| self.snapshot());
        let base = self.events.len();
        let result = self.dispatch(cmd);
        match result {
            Ok(()) => {
                let produced = self.events[base..].to_vec();
                if let Some(s) = snapshot {
                    self.push_undo(s);
                }
                Ok(produced)
            }
            Err(e) => {
                if let Some(s) = snapshot {
                    self.restore(s);
                }
                Err(e)
            }
        }
    }

    fn push_undo(&mut self, s: EngineSnapshot) {
        if self.undo.len() >= UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.undo.push(s);
    }

    fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            run: self.run.clone(),
            battle: self.battle.clone(),
            spin_index: self.spin_index,
            battle_index: self.battle_index,
            run_rng_calls: self.run_rng_calls,
            effect_rng_calls: self.effect_rng_calls,
            draft_wheel: self.draft_wheel.clone(),
        }
    }

    fn restore(&mut self, s: EngineSnapshot) {
        self.run = s.run;
        self.battle = s.battle;
        self.spin_index = s.spin_index;
        self.battle_index = s.battle_index;
        self.run_rng_calls = s.run_rng_calls;
        self.effect_rng_calls = s.effect_rng_calls;
        self.draft_wheel = s.draft_wheel;
    }

    fn dispatch(&mut self, cmd: &Command) -> Result<(), EngineError> {
        match cmd {
            Command::StartRun { difficulty } => self.start_run(*difficulty),
            Command::DraftCard { card_id } => {
                self.run_mut()?.draft_card(card_id).map_err(EngineError::from)
            }
            Command::DraftWheel => self.run_mut()?.draft_wheel().map_err(EngineError::from),
            Command::FinishLoadout => {
                self.run_mut()?.complete_loadout().map_err(EngineError::from)?;
                Ok(())
            }
            Command::PickNode { node_id } => {
                let mut rng = self.next_run_rng();
                let mut run = self.run.take().ok_or(EngineError::NoRun)?;
                let outcome = run.pick_node(node_id, &self.content, &mut rng);
                self.run = Some(run);
                outcome.map_err(EngineError::from)?;
                self.post_pick(node_id)
            }
            Command::PlayCard { hand_index } => {
                let (cost, played) = {
                    let (cost, defs, mut rng) = {
                        let battle = self.battle.as_ref().ok_or(EngineError::NotInBattle)?;
                        let cost = battle.hand.get(*hand_index).and_then(|c| {
                            c.cost_override
                                .or_else(|| self.card_defs.get(&c.def_id).map(|d| d.cost))
                        });
                        (cost, self.card_defs.clone(), self.next_effect_rng())
                    };
                    let battle = self.battle.as_mut().ok_or(EngineError::NotInBattle)?;
                    let mut ctx = crate::cards::effects::EffectCtx { defs: &defs, rng: &mut rng };
                    let played = crate::cards::effects::play_card_with_effects(
                        battle,
                        *hand_index,
                        &mut ctx,
                    );
                    (cost, played)
                };
                let def_id = match played {
                    Ok(true) => {
                        let battle = self.battle.as_ref().ok_or(EngineError::NotInBattle)?;
                        battle
                            .active_played
                            .last()
                            .map(|a| a.instance.def_id.clone())
                            .unwrap_or_default()
                    }
                    Ok(false) => {
                        // Rejected play: the card returned to its hand slot.
                        let battle = self.battle.as_ref().ok_or(EngineError::NotInBattle)?;
                        battle.hand.first().map(|c| c.def_id.clone()).unwrap_or_default()
                    }
                    Err(e) => {
                        return Err(EngineError::Action(ActionError::InvalidBet(format!(
                            "card effect: {e:?}"
                        ))))
                    }
                };
                self.events.push(EngineEvent::CardPlayed {
                    card_id: def_id,
                    cost: cost.unwrap_or(0) as u16,
                });
                Ok(())
            }
            Command::BuyDraw => {
                let battle = self.battle_mut()?;
                let cost = battle.draw_cost();
                let card = battle.buy_draw().map_err(EngineError::from)?;
                self.events.push(EngineEvent::DrawBought { cost, card_id: card.def_id });
                Ok(())
            }
            Command::PlaceBet { bet, amount } => {
                let battle = self.battle_mut()?;
                battle.place_bet(*bet, *amount).map_err(EngineError::from)?;
                self.events.push(EngineEvent::BetPlaced { bet: *bet, amount: *amount });
                Ok(())
            }
            Command::RemoveBet { bet, amount } => {
                let battle = self.battle_mut()?;
                battle.remove_bet(*bet, *amount).map_err(EngineError::from)?;
                self.events.push(EngineEvent::BetRemoved { bet: *bet, amount: *amount });
                Ok(())
            }
            Command::ClearBets => {
                let battle = self.battle_mut()?;
                battle.clear_bets();
                self.events.push(EngineEvent::BetsCleared);
                Ok(())
            }
            Command::Rebet => {
                let battle = self.battle_mut()?;
                battle.rebet().map_err(EngineError::from)?;
                self.events.push(EngineEvent::Rebet);
                Ok(())
            }
            Command::Sacrifice => {
                let battle = self.battle_mut()?;
                battle.sacrifice().map_err(EngineError::from)?;
                self.events.push(EngineEvent::Sacrificed);
                Ok(())
            }
            Command::Predict => self.predict(),
            Command::Spin => self.spin(),
            Command::Purchase { item_index } => {
                let bought = self.run_mut()?.shop_buy(*item_index).map_err(EngineError::from)?;
                let price = self
                    .run()
                    .and_then(|r| r.shop_offer.as_ref())
                    .and_then(|o| o.items.get(*item_index))
                    .map(|i| i.price())
                    .unwrap_or(0);
                self.events.push(EngineEvent::Purchased { item: *item_index, price });
                match bought {
                    crate::run::ShopItemBought::Card(id) => {
                        self.events.push(EngineEvent::CardGained { id })
                    }
                    crate::run::ShopItemBought::Wheel(id) => {
                        self.events.push(EngineEvent::WheelGained { id })
                    }
                    crate::run::ShopItemBought::Heal(hp) => {
                        self.events.push(EngineEvent::Healed { hp })
                    }
                }
                Ok(())
            }
            Command::ForgeTake { op_index } => {
                let mut rng = self.next_run_rng();
                let mut run = self.run.take().ok_or(EngineError::NoRun)?;
                let outcome = run.forge_take(*op_index, &mut rng);
                self.run = Some(run);
                outcome.map_err(EngineError::from)?;
                self.events.push(EngineEvent::ForgeApplied { op: *op_index });
                Ok(())
            }
            Command::ForgeReroll => {
                let mut rng = self.next_run_rng();
                let mut run = self.run.take().ok_or(EngineError::NoRun)?;
                let outcome = run.forge_reroll(&self.content, &mut rng);
                self.run = Some(run);
                outcome.map_err(EngineError::from)?;
                self.events.push(EngineEvent::ForgeRerolled);
                Ok(())
            }
            Command::EventChoose { choice_id } => {
                let mut run = self.run.take().ok_or(EngineError::NoRun)?;
                let outcome = run.event_choose(&self.content, choice_id);
                self.run = Some(run);
                let outcome = outcome.map_err(EngineError::from)?;
                self.events.push(EngineEvent::EventChosen {
                    choice: choice_id.clone(),
                    hp_lost: outcome.hp_lost,
                    chips_gained: outcome.chips_gained,
                    cards: outcome.cards_granted,
                });
                Ok(())
            }
            Command::BuyLevel { color } => {
                let run = self.run_mut()?;
                let new_level = run.buy_color_level(*color).map_err(EngineError::from)?;
                self.events.push(EngineEvent::ColorLevelBought { color: *color, level: new_level });
                Ok(())
            }
            Command::Customize { op } => self.customize(op),
            Command::Undo => self.undo_last(),
        }
    }

    /// §4.8 wheel customizer: edits apply to a draft clone; `Save` commits the
    /// draft to the run (and syncs the owned-wheel entry), `Cancel` discards.
    fn customize(&mut self, op: &CustomizeOp) -> Result<(), EngineError> {
        if self.run.as_ref().ok_or(EngineError::NoRun)?.state != GameState::Forge {
            return Err(EngineError::Run(RunError::NotInState(
                self.run.as_ref().expect("checked above").state,
            )));
        }
        if let Some(edit) = match op {
            CustomizeOp::Save | CustomizeOp::Cancel => None,
            edit => Some(edit),
        } {
            let mut draft = self
                .draft_wheel
                .take()
                .unwrap_or_else(|| self.run.as_ref().expect("checked above").player_wheel.clone());
            let result = match edit {
                CustomizeOp::CycleColor { slot } => {
                    let color = draft.cycle_slot_color(*slot);
                    color
                        .map(|color| {
                            self.events.push(EngineEvent::SlotColorCycled { slot: *slot, color })
                        })
                        .map_err(|e| e.to_string())
                }
                CustomizeOp::AddSlot { number } => draft
                    .add_slot(*number)
                    .map(|()| self.events.push(EngineEvent::SlotAdded { number: *number }))
                    .map_err(|e| e.to_string()),
                CustomizeOp::RemoveSlot { slot } => draft
                    .remove_slot(*slot)
                    .map(|()| self.events.push(EngineEvent::SlotRemoved { slot: *slot }))
                    .map_err(|e| e.to_string()),
                CustomizeOp::SetNumber { slot, number } => draft
                    .set_slot_number(*slot, *number)
                    .map(|()| {
                        self.events
                            .push(EngineEvent::SlotNumberSet { slot: *slot, number: *number })
                    })
                    .map_err(|e| e.to_string()),
                CustomizeOp::Save | CustomizeOp::Cancel => Ok(()),
            };
            if let Err(e) = result {
                // Failed edit: put the draft back so the session can continue.
                self.draft_wheel = Some(draft);
                return Err(EngineError::Content(e));
            }
            self.draft_wheel = Some(draft);
        }
        match op {
            CustomizeOp::Save => {
                let draft = self.draft_wheel.take().expect("draft open on save");
                let id = draft.id.clone();
                let run = self.run_mut()?;
                run.player_wheel = draft;
                run.owned_wheels = run
                    .owned_wheels
                    .iter()
                    .map(|w| if w.id == id { run.player_wheel.clone() } else { w.clone() })
                    .collect();
                self.events.push(EngineEvent::WheelSaved);
            }
            CustomizeOp::Cancel => {
                self.draft_wheel = None;
                self.events.push(EngineEvent::WheelCancelled);
            }
            _ => {}
        }
        Ok(())
    }

    fn start_run(&mut self, difficulty: Difficulty) -> Result<(), EngineError> {
        let mut run =
            crate::run::state::RunState::start_new_run(&self.content, &self.seed, difficulty);
        self.battle_index = 0;
        self.spin_index = 0;
        self.run_rng_calls = 0;
        self.prediction = None;
        self.draft_wheel = None;
        self.battle = None;
        self.events.push(EngineEvent::RunStarted { seed: self.seed.clone(), difficulty });
        let mut rng = self.next_run_rng();
        run.open_loadout(&self.content, &mut rng);
        self.run = Some(run);
        Ok(())
    }

    /// After a node is picked: battles are entered immediately; shop/forge/
    /// event nodes open their screen (offers are rolled inside pick_node).
    fn post_pick(&mut self, node_id: &str) -> Result<(), EngineError> {
        let node_type = crate::run::state::node_type_at(self.run.as_ref().unwrap(), node_id)
            .ok_or(EngineError::Content(format!("node {node_id} missing")))?;
        self.events.push(EngineEvent::NodePicked { id: node_id.to_string(), node_type });
        let combat_type = matches!(
            node_type,
            crate::run::map::NodeType::Combat
                | crate::run::map::NodeType::Elite
                | crate::run::map::NodeType::Boss
        );
        let tier = combat_type.then(|| {
            self.run
                .as_ref()
                .ok_or(EngineError::NoRun)?
                .current_node_tier()
                .map_err(EngineError::from)
        });
        if let Some(tier) = tier {
            let tier = tier?;
            let mut rng = self.next_run_rng();
            let mut run = self.run.take().ok_or(EngineError::NoRun)?;
            let battle = run.enter_battle(&self.content, &mut rng).map_err(EngineError::from)?;
            self.run = Some(run);
            self.battle_index += 1;
            self.spin_index = 0;
            self.prediction = None;
            self.events.push(EngineEvent::BattleStarted {
                tier,
                round_limit: battle.max_rounds,
                enemy_hp: battle.enemy_hp,
            });
            self.battle = Some(battle);
        }
        Ok(())
    }

    fn battle_mut(&mut self) -> Result<&mut BattleState, EngineError> {
        self.battle.as_mut().ok_or(EngineError::NotInBattle)
    }

    // -- prediction & spin ---------------------------------------------------

    /// Player wheel physics layout with §10.1 step-1 effective colors folded
    /// (green ladder + board converts/swaps), matching `resolve_spin` exactly.
    fn player_layout(&self, battle: &BattleState) -> WheelLayout {
        let wheel = battle.wheel(crate::battle::state::Side::Player);
        self.layout_for(
            wheel,
            battle.player_levels.get(SlotColor::Green),
            &battle.merged_board(crate::battle::state::Side::Player),
        )
    }

    fn enemy_layout(&self, battle: &BattleState) -> WheelLayout {
        let wheel = battle.wheel(crate::battle::state::Side::Enemy);
        self.layout_for(
            wheel,
            battle.enemy_levels.get(SlotColor::Green),
            &battle.merged_board(crate::battle::state::Side::Enemy),
        )
    }

    fn layout_for(
        &self,
        wheel: &WheelConfig,
        green_level: u8,
        board: &crate::board::BoardModifiers,
    ) -> WheelLayout {
        let colors = wheel
            .numbers
            .iter()
            .map(|&n| wheel.effective_color(n, green_level, Some(board)))
            .collect();
        WheelLayout {
            numbers: wheel.numbers.clone(),
            colors,
            slot_width: std::f64::consts::TAU / wheel.numbers.len().max(1) as f64,
        }
    }

    /// Magnet target angle: center of the highest-payout slot allowed by the
    /// bias filter (red-only/black-only), tie broken by lowest slot index.
    fn bias_target_angle(battle: &BattleState, layout: &WheelLayout) -> Option<f64> {
        let mods = &battle.physics;
        if mods.target_zone_bias <= 0.0 {
            return None;
        }
        let wheel = battle.wheel(crate::battle::state::Side::Player);
        let board = battle.merged_board(crate::battle::state::Side::Player);
        let green = battle.player_levels.get(SlotColor::Green);
        let allowed = |c: SlotColor| {
            (!mods.bias_red_only || c == SlotColor::Red)
                && (!mods.bias_black_only || c == SlotColor::Black)
        };
        let base = |color: SlotColor| -> f32 {
            let bet = match color {
                SlotColor::Red => BetType::Red,
                SlotColor::Black => BetType::Black,
                SlotColor::Green => BetType::Green,
                SlotColor::Gold => BetType::Gold,
                SlotColor::Purple => BetType::Purple,
                SlotColor::Cyan => BetType::Cyan,
                SlotColor::Crimson => BetType::Crimson,
            };
            bet_base(bet, wheel, green)
        };
        let mut best: Option<(f32, usize)> = None;
        for (idx, &num) in layout.numbers.iter().enumerate() {
            let color = wheel.effective_color(num, green, Some(&board));
            if !allowed(color) {
                continue;
            }
            let payout = base(color);
            if best.map(|(p, _)| payout > p).unwrap_or(true) {
                best = Some((payout, idx));
            }
        }
        best.map(|(_, idx)| (idx as f64 + 0.5) * layout.slot_width)
    }

    /// The per-spin RNG stream (deterministic label per spin index).
    fn spin_stream(&self) -> Rng {
        self.run_rng().derive(&format!("spin:{}", self.spin_index))
    }

    fn predict(&mut self) -> Result<(), EngineError> {
        if self.spin_sampling == SpinSampling::Uniform {
            // No dry-run exists on the uniform fast path (§15 tooling mode).
            return Err(EngineError::PredictionUnavailable);
        }
        let battle = self.battle.as_ref().ok_or(EngineError::NotInBattle)?;
        if !battle.prediction_allowed() {
            return Err(EngineError::PredictionBlocked);
        }
        if battle.physics.prediction_size == 0 {
            return Err(EngineError::PredictionUnavailable);
        }
        let layout = self.player_layout(battle);
        let mut mods = battle.physics.clone();
        mods.bias_target_angle = Self::bias_target_angle(battle, &layout);
        let live = self.spin_stream();
        let oracle = self.run_rng().derive(crate::phys::PREDICTION_STREAM);
        let prediction = Simulator::predict(&layout, &mods, live, oracle);
        let Some(p) = prediction else {
            return Err(EngineError::PredictionUnavailable);
        };
        self.events
            .push(EngineEvent::PredictionShown { start_slot: p.sector_start, size: p.sector_size });
        self.prediction = Some((p.sector_start, p.sector_size));
        Ok(())
    }

    fn spin(&mut self) -> Result<(), EngineError> {
        // 1. Player spin ------------------------------------------------------
        let battle = self.battle.as_mut().ok_or(EngineError::NotInBattle)?;
        if battle.phase != BattlePhase::Betting {
            return Err(EngineError::NotInBattle);
        }
        let passed = battle.spin().map_err(EngineError::from)?.is_none();
        let mut events =
            vec![EngineEvent::SpinStarted { side: crate::battle::state::Side::Player }];
        if !passed {
            // Gather sim inputs from immutable views first, then mutate.
            let battle = self.battle.as_ref().ok_or(EngineError::NotInBattle)?;
            let layout = self.player_layout(battle);
            let mut mods = battle.physics.clone();
            mods.bias_target_angle = Self::bias_target_angle(battle, &layout);
            let live = self.spin_stream();
            let prediction = self.prediction;
            let nudge = (mods.nudge_cheat_active && mods.nudge_distance > 0)
                .then_some(mods.nudge_distance as i32);
            let battle = self.battle.as_mut().ok_or(EngineError::NotInBattle)?;
            #[cfg(not(feature = "telemetry"))]
            let (_, result) = if self.spin_sampling == SpinSampling::Uniform {
                let balls = mods.ball_count();
                Simulator::uniform_run(&layout, balls, live)
            } else {
                Simulator::new(layout.clone(), mods, live).run_to_completion(nudge)
            };
            #[cfg(feature = "telemetry")]
            let result = {
                let (result, frames, sim_evs) = if self.spin_sampling == SpinSampling::Uniform {
                    let balls = mods.ball_count();
                    let (_, r) = Simulator::uniform_run(&layout, balls, live);
                    (r, Vec::new(), Vec::new())
                } else {
                    let (evs, r, frames) = Simulator::new(layout.clone(), mods, live)
                        .run_to_completion_with_telemetry_timed(nudge);
                    (r, frames, evs)
                };
                self.spin_frames.retain(|(s, _)| *s != crate::battle::state::Side::Player);
                self.spin_frames.push((
                    crate::battle::state::Side::Player,
                    SpinTelemetry {
                        slot_count: layout.len() as u32,
                        slot_width: layout.slot_width,
                        frames,
                    },
                ));
                self.spin_sim_events.retain(|(s, _)| *s != crate::battle::state::Side::Player);
                self.spin_sim_events.push((crate::battle::state::Side::Player, sim_evs));
                result
            };
            for &slot in &result.slots {
                events.push(EngineEvent::BallLanded {
                    side: crate::battle::state::Side::Player,
                    number: layout.numbers[slot],
                });
            }
            let landed = layout.numbers[result.slots[0]];
            let extra_balls = result.slots[1..].iter().map(|&s| layout.numbers[s]).collect();
            let outcome = battle.resolve_spin(&SpinInput {
                side: crate::battle::state::Side::Player,
                landed,
                extra_balls,
                prediction,
                low_rarity_physics_played: false,
            });
            events.extend(Self::spin_resolved_events(crate::battle::state::Side::Player, &outcome));
            self.prediction = None;
        }
        let battle = self.battle.as_mut().ok_or(EngineError::NotInBattle)?;
        battle.advance_after_spin();

        // 2. Enemy turn (real physics on the enemy wheel) ---------------------
        let enemy = {
            let battle = self.battle.as_ref().ok_or(EngineError::NotInBattle)?;
            let layout = self.enemy_layout(battle);
            let mods = PhysicsModifiers::default();
            let live = self.run_rng().derive(&format!("spin:{}:enemy", self.spin_index));
            #[cfg(not(feature = "telemetry"))]
            let (_, result) = if self.spin_sampling == SpinSampling::Uniform {
                let balls = mods.ball_count();
                Simulator::uniform_run(&layout, balls, live)
            } else {
                Simulator::new(layout.clone(), mods, live).run_to_completion(None)
            };
            #[cfg(feature = "telemetry")]
            let result = {
                let (result, frames, sim_evs) = if self.spin_sampling == SpinSampling::Uniform {
                    let balls = mods.ball_count();
                    let (_, r) = Simulator::uniform_run(&layout, balls, live);
                    (r, Vec::new(), Vec::new())
                } else {
                    let (evs, r, frames) = Simulator::new(layout.clone(), mods, live)
                        .run_to_completion_with_telemetry_timed(None);
                    (r, frames, evs)
                };
                self.spin_frames.retain(|(s, _)| *s != crate::battle::state::Side::Enemy);
                self.spin_frames.push((
                    crate::battle::state::Side::Enemy,
                    SpinTelemetry {
                        slot_count: layout.len() as u32,
                        slot_width: layout.slot_width,
                        frames,
                    },
                ));
                self.spin_sim_events.retain(|(s, _)| *s != crate::battle::state::Side::Enemy);
                self.spin_sim_events.push((crate::battle::state::Side::Enemy, sim_evs));
                result
            };
            for &slot in &result.slots {
                events.push(EngineEvent::BallLanded {
                    side: crate::battle::state::Side::Enemy,
                    number: layout.numbers[slot],
                });
            }
            layout.numbers[result.slots[0]]
        };
        let mut rng = self.run_rng().derive(&format!("spin:{}:enemy:ai", self.spin_index));
        let battle = self.battle.as_mut().ok_or(EngineError::NotInBattle)?;
        let intent = battle.enemy_take_turn(&mut rng, enemy);
        if let Some(outcome) = &intent {
            // Observability (B4): the house's bets precede its resolve so the
            // pool swing is attributable from events alone.
            for bet in &battle.last_enemy_bets {
                events.push(EngineEvent::EnemyBetPlaced { bet: bet.bet_type, amount: bet.amount });
            }
            events.extend(Self::spin_resolved_events(crate::battle::state::Side::Enemy, outcome));
            if let Some(executed) = &battle.last_executed_intent {
                events.push(EngineEvent::IntentExecuted {
                    action: executed.action,
                    value: executed.value,
                    description: executed.description.clone(),
                });
            }
        }
        let battle = self.battle.as_mut().ok_or(EngineError::NotInBattle)?;
        // When the enemy was stunned, `enemy_take_turn` already handed the
        // turn to the player; advancing again would hand it back to the
        // enemy with nobody to act — a command-level softlock ("not your
        // turn" forever). Only the taken turn needs an advance.
        if intent.is_some() {
            battle.advance_after_spin();
        }
        self.spin_index += 1;

        // 3. Round end --------------------------------------------------------
        let hp_before = battle.player_hp;
        let outcome = battle.end_round();
        let battle = self.battle.as_ref().ok_or(EngineError::NotInBattle)?;
        events.push(EngineEvent::RoundEnded {
            round: battle.round,
            player_pts: battle.chips_pool,
            enemy_pts: battle.enemy_chips_pool,
            hp_delta: battle.player_hp as i16 - hp_before as i16,
            outcome: outcome.into(),
        });
        match outcome {
            BattleOutcome::InProgress | BattleOutcome::SuddenDeath => {
                // Turn handoff is already complete: advance #2 (after the
                // enemy's spin) returns the turn to the player for the next
                // betting phase (§3.2). An extra advance here would leave
                // round N+1 in the enemy's betting phase and stall all
                // command-driven multi-round battles.
                self.events.append(&mut events);
                Ok(())
            }
            BattleOutcome::PlayerVictory | BattleOutcome::PlayerDefeat => {
                self.finish_battle(events)
            }
        }
    }

    fn spin_resolved_events(side: crate::battle::state::Side, o: &SpinOutcome) -> Vec<EngineEvent> {
        let mut v = Vec::new();
        v.push(EngineEvent::SpinResolved {
            side,
            pts: o.total_payout,
            insurance_refund: o.insurance_refund,
            all_lost: o.all_lost,
        });
        if o.insurance_refund > 0 {
            v.push(EngineEvent::ChipsChanged { side, delta: o.insurance_refund as i32 });
        }
        v
    }

    fn finish_battle(&mut self, mut events: Vec<EngineEvent>) -> Result<(), EngineError> {
        let mut rng = self.next_run_rng();
        let mut run = self.run.take().ok_or(EngineError::NoRun)?;
        let mut battle = self.battle.take().ok_or(EngineError::NotInBattle)?;
        let result =
            run.complete_battle(&self.content, &mut battle, &mut rng).map_err(EngineError::from);
        self.run = Some(run);
        let result = result?;
        events.push(EngineEvent::BattleEnded { result: result.clone().into() });
        match &result {
            BattleResult::Victory { rewards } => {
                events.push(EngineEvent::NodeCompleted);
                events.push(EngineEvent::RewardCards { ids: rewards.clone() });
            }
            BattleResult::Defeat => events.push(EngineEvent::NodeFailed),
            BattleResult::GameOver => events.push(EngineEvent::GameOver),
        }
        let final_state = self.run.as_ref().map(|r| r.state).unwrap_or(GameState::Menu);
        if final_state == GameState::Victory {
            events.push(EngineEvent::RunVictory);
        }
        self.events.append(&mut events);
        Ok(())
    }

    fn undo_last(&mut self) -> Result<(), EngineError> {
        let s = self.undo.pop().ok_or(EngineError::NoUndo)?;
        self.restore(s);
        self.events.push(EngineEvent::Undone);
        Ok(())
    }
}

impl Command {
    /// Commands that mutate state and can be undone (REQ-009).
    fn is_undoable(&self) -> bool {
        !matches!(self, Command::Undo | Command::StartRun { .. })
    }
}

/// Base payout multiplier for a color bet at the given ladder level (§10.1
/// step 1; level-scaled via [`BetType::base`]).
fn bet_base(bet: BetType, wheel: &WheelConfig, green_level: u8) -> f32 {
    let mut levels = ColorLevels::default();
    levels.level_up(SlotColor::Green, green_level);
    bet.base(wheel, &levels)
}
