//! Per-card EV engine (§15): `evaluateCard(card, defaultBets=[red/10],
//! EUROPEAN_WHEEL, DEFAULT_BOARD_MODIFIERS)` as a synthetic points-mode
//! battle harness — one card, the real §10 resolve pipeline, no enemy
//! interference, deep chip bankroll, fight-long scenario.
//!
//! Ratings and bands per §14.3; physics cards are never repriced (§15.2).

use std::sync::Arc;

use roulette_content::schema::{CardDef, CardRarity, CardType, Content};
use roulette_core::battle::state::{BattleState, CombatMode, Side, SpinInput};
use roulette_core::bets::BetType;
use roulette_core::board::BoardModifiers;
use roulette_core::cards::effects::{after_spin_resolve, play_card_with_effects, EffectCtx};
use roulette_core::phys::{PhysicsModifiers, Simulator, WheelLayout};
use roulette_core::rng::Rng;
use roulette_core::wheel::WheelConfig;

/// Scenario: EUROPEAN_WHEEL ("classic") + default board (§15.2).
pub const SCENARIO_WHEEL_ID: &str = "classic";
/// `defaultBets = [{type:'red', amount:10}]` (§15.2).
const BET: BetType = BetType::Red;
const BET_AMOUNT: u16 = 10;
/// Filler deck card: any cheap common keeps hand-surgery effects realistic.
const FILLER_CARD: &str = "crimson_double";

/// §14.3 ΔEV-per-⚡ efficiency bands by rarity.
fn band(rarity: CardRarity) -> (f32, f32) {
    match rarity {
        CardRarity::Common => (2.5, 10.0),
        CardRarity::Uncommon => (6.0, 22.0),
        CardRarity::Rare => (12.0, 45.0),
        CardRarity::Legendary => (25.0, 120.0),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rating {
    Overpowered,
    Balanced,
    Underpowered,
    Dead,
    PhysicsDependent,
}

impl std::fmt::Display for Rating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Rating::Overpowered => "OP",
            Rating::Balanced => "BALANCED",
            Rating::Underpowered => "UP",
            Rating::Dead => "DEAD",
            Rating::PhysicsDependent => "PHYSICS",
        })
    }
}

#[derive(Debug, Clone)]
pub struct EvConfig {
    /// Spins per scenario (§15.1 studio uses 1000).
    pub spins: usize,
    /// `true` runs the real 120 Hz sim per spin (slow); default is the
    /// §15 uniform fast path.
    pub physics: bool,
    pub seed_prefix: String,
}

impl Default for EvConfig {
    fn default() -> Self {
        Self { spins: 400, physics: false, seed_prefix: "ev".into() }
    }
}

#[derive(Debug, Clone)]
pub struct CardReport {
    pub id: String,
    pub name: String,
    pub card_type: CardType,
    pub rarity: CardRarity,
    pub cost: u8,
    /// Baseline pts/spin with `[red 10]` bets.
    pub baseline: f32,
    /// Modified pts per spin with the card armed.
    pub modified: f32,
    /// `modified − baseline` (net ΔEV).
    pub delta_ev: f32,
    /// ΔEV per ⚡ (§14.3 band metric).
    pub efficiency: f32,
    pub rating: Rating,
    pub suggested_cost: u8,
}

/// Player wheel layout with effective colors (§10.1 step 1) — mirrors the
/// engine's `layout_for` without exposing internals.
pub fn player_layout(wheel: &WheelConfig, board: &BoardModifiers, green_level: u8) -> WheelLayout {
    let colors =
        wheel.numbers.iter().map(|&n| wheel.effective_color(n, green_level, Some(board))).collect();
    WheelLayout {
        numbers: wheel.numbers.clone(),
        colors,
        slot_width: std::f64::consts::TAU / wheel.numbers.len().max(1) as f64,
    }
}

/// Total player payout (points won) for one resolved spin.
fn spin_points(
    battle: &mut BattleState,
    layout: &WheelLayout,
    physics: bool,
    rng: &mut Rng,
    bet: BetType,
) -> u16 {
    battle.begin_betting(Side::Player);
    battle.chips_pool = 30; // Deep bankroll assumption (§15 studio baseline).
    let _ = battle.place_bet(bet, BET_AMOUNT);
    let (_, result) = if physics {
        Simulator::new(layout.clone(), PhysicsModifiers::default(), rng.clone())
            .run_to_completion(None)
    } else {
        Simulator::uniform_run(layout, 1, rng.clone())
    };
    let landed = layout.numbers[result.slots[0]];
    let outcome = battle.resolve_spin(&SpinInput {
        side: Side::Player,
        landed,
        extra_balls: vec![],
        prediction: None,
        low_rarity_physics_played: false,
    });
    after_spin_resolve(battle);
    outcome.total_payout
}

/// Average points per spin over the scenario, optionally with `card` played
/// once at fight start (armed for its natural duration).
fn scenario_ev(
    content: &Arc<Content>,
    card: Option<&CardDef>,
    cfg: &EvConfig,
    wheel: &WheelConfig,
) -> f32 {
    let board = BoardModifiers::default();
    let layout = player_layout(wheel, &board, 0);
    let filler = content
        .cards
        .iter()
        .find(|c| c.id == FILLER_CARD)
        .or_else(|| content.cards.first())
        .cloned()
        .expect("content has cards");
    let deck: Vec<CardDef> = std::iter::repeat_n(filler, 8).chain(card.cloned()).collect();

    let mut battle = BattleState::new(
        wheel.clone(),
        wheel.clone(),
        80,
        80,
        9999,
        99,
        vec![],
        CombatMode::Points,
    );
    let mut rng = Rng::new(djb2(&format!(
        "{}:{}:deal",
        cfg.seed_prefix,
        card.map(|c| c.id.as_str()).unwrap_or("baseline")
    )));
    battle.deal_from_defs(&deck, &mut rng);

    // Put the card under test into the hand, then play it once.
    if let Some(def) = card {
        if let Some(idx) = battle.draw_pile.iter().position(|c| c.def_id == def.id) {
            let inst = battle.draw_pile.swap_remove(idx);
            battle.hand.push(inst);
        }
        let mut hand_idx =
            battle.hand.iter().position(|c| c.def_id == def.id).expect("test card dealt");
        let defs: std::collections::BTreeMap<String, CardDef> =
            content.cards.iter().map(|c| (c.id.clone(), c.clone())).collect();
        // Hand-surgery cards need the state a real game would have; without it
        // their play is a no-op for EV (copy needs a non-utility card on the
        // felt; recycle needs a discard-pile entry).
        match def.id.as_str() {
            "copy_paste" | "golden_mirror" => {
                let mut primer_ctx = EffectCtx { defs: &defs, rng: &mut rng };
                let _ = play_card_with_effects(&mut battle, 0, &mut primer_ctx);
                // The primer consumed hand[0]; recompute the target's index.
                hand_idx = battle
                    .hand
                    .iter()
                    .position(|c| c.def_id == def.id)
                    .expect("test card still in hand");
            }
            "recycle_bin" => battle.discard_pile.push(roulette_core::battle::state::CardInstance {
                def_id: FILLER_CARD.to_string(),
                marked_slots: Vec::new(),
                temp: false,
                retained: false,
                cost_override: None,
            }),
            _ => {}
        }
        let mut ctx = EffectCtx { defs: &defs, rng: &mut rng };
        let played = play_card_with_effects(&mut battle, hand_idx, &mut ctx);
        if !matches!(played, Ok(true)) {
            eprintln!(
                "WARN: card {} did not play cleanly ({:?}); treated as 0-effect",
                def.id, played
            );
            // Rebuild a fresh battle without the card armed.
            let filler = content
                .cards
                .iter()
                .find(|c| c.id == FILLER_CARD)
                .or_else(|| content.cards.first())
                .cloned()
                .expect("content has cards");
            let deck: Vec<CardDef> = std::iter::repeat_n(filler, 8).collect();
            let board = BoardModifiers::default();
            let layout = player_layout(wheel, &board, 0);
            let mut battle = BattleState::new(
                wheel.clone(),
                wheel.clone(),
                80,
                80,
                9999,
                99,
                vec![],
                CombatMode::Points,
            );
            let mut deal_rng = Rng::new(djb2(&format!("{}:{}:deal", cfg.seed_prefix, def.id)));
            battle.deal_from_defs(&deck, &mut deal_rng);
            let mut total = 0u64;
            for i in 0..cfg.spins {
                let mut rng = Rng::new(djb2(&format!("{}:{}:spin:{i}", cfg.seed_prefix, def.id)));
                total += u64::from(spin_points(&mut battle, &layout, cfg.physics, &mut rng, BET));
            }
            return total as f32 / cfg.spins as f32;
        }
    }

    let mut total = 0u64;
    for i in 0..cfg.spins {
        let spin_rng = Rng::new(djb2(&format!(
            "{}:{}:spin:{i}",
            cfg.seed_prefix,
            card.map(|c| c.id.as_str()).unwrap_or("baseline")
        )));
        let mut spin_rng = spin_rng;
        let pts = spin_points(&mut battle, &layout, cfg.physics, &mut spin_rng, BET);
        if std::env::var("EV_DEBUG").is_ok() {
            eprintln!("DBG i={i} pts={pts} bets={}", battle.bets.len());
        }
        total += u64::from(pts);
    }
    total as f32 / cfg.spins as f32
}

/// EV evaluation of one card (§15.2): baseline vs modified, ΔEV, efficiency,
/// rating, suggested cost.
pub fn evaluate_card(
    content: &Arc<Content>,
    def: &CardDef,
    cfg: &EvConfig,
    wheel: &WheelConfig,
) -> CardReport {
    let baseline = scenario_ev(content, None, cfg, wheel);
    let modified = if def.card_type == CardType::Physics {
        // Physics cards are sim-dependent; still measured, but never repriced.
        scenario_ev(content, Some(def), cfg, wheel)
    } else {
        scenario_ev(content, Some(def), cfg, wheel)
    };
    let delta_ev = modified - baseline;
    let cost = def.cost.max(1);
    let efficiency = delta_ev / cost as f32;
    let rating = if def.card_type == CardType::Physics {
        Rating::PhysicsDependent
    } else if delta_ev <= 0.0 {
        Rating::Dead
    } else {
        let (bottom, top) = band(def.rarity);
        if efficiency > top {
            Rating::Overpowered
        } else if efficiency >= bottom {
            Rating::Balanced
        } else {
            Rating::Underpowered
        }
    };
    let suggested_cost = match rating {
        Rating::Overpowered => def.cost.saturating_add(1).min(5),
        Rating::Underpowered => def.cost.saturating_sub(1),
        _ => def.cost,
    };
    CardReport {
        id: def.id.clone(),
        name: def.name.clone(),
        card_type: def.card_type,
        rarity: def.rarity,
        cost: def.cost,
        baseline,
        modified,
        delta_ev,
        efficiency,
        rating,
        suggested_cost,
    }
}

pub fn djb2(s: &str) -> u32 {
    let mut hash = 5381u32;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u32);
    }
    hash
}

/// Rewrites `cost:` fields per card id in `content/cards.ron` (§15.2
/// `--apply`). Only the integer after the first `cost:` following each
/// `(id: "<id>"` block is touched. Returns the number of cards rewritten.
pub fn apply_costs(text: &str, updates: &[(String, u8)]) -> (String, usize) {
    let mut out = text.to_string();
    let mut rewritten = 0;
    for (id, new_cost) in updates {
        let Some(id_pos) = out.find(&format!(r#"(id: "{id}""#)) else {
            continue;
        };
        let Some(cost_pos) = out[id_pos..].find("cost:") else {
            continue;
        };
        let start = id_pos + cost_pos + "cost:".len();
        let digits_start = out[start..].find(|c: char| c.is_ascii_digit()).map(|d| start + d);
        let Some(digits_start) = digits_start else { continue };
        let digits_end = out[digits_start..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|e| digits_start + e)
            .unwrap_or(out.len());
        out.replace_range(digits_start..digits_end, &new_cost.to_string());
        rewritten += 1;
    }
    (out, rewritten)
}
