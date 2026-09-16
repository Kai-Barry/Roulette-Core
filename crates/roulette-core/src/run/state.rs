//! Run-level state machine (§16.2 `RunState`, TASK-036/039/040).

use std::collections::BTreeMap;

use roulette_content::schema::{CardDef, Content, CurseDef, EnemyTier, SlotColor};
use serde::{Deserialize, Serialize};

use crate::battle::state::{BattleState, CardInstance, CombatMode};
use crate::rng::Rng;
use crate::wheel::{ColorLevels, WheelConfig, MAX_COLOR_LEVEL};

use super::events::{apply_choice_effects, EventOutcome};
use super::forge::{self, ForgeError, ForgeOffer};
use super::map::{self, Map, MapNode, NodeType};
use super::rewards;
use super::shop::{self, ShopItem, ShopOffer};

/// Macro screen state (§16.2 `GameState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameState {
    Menu,
    LoadoutStore,
    Map,
    Combat,
    Shop,
    Event,
    Forge,
    Victory,
    GameOver,
}

/// Run length (§2.1: SHORT 7 [default] / MEDIUM 11 / LONG 15 floors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    #[default]
    Short,
    Medium,
    Long,
}

impl Difficulty {
    pub fn floors(self) -> usize {
        match self {
            Difficulty::Short => 7,
            Difficulty::Medium => 11,
            Difficulty::Long => 15,
        }
    }
}

/// Run-flow errors.
#[derive(Debug, Clone, PartialEq)]
pub enum RunError {
    NotEnoughChips,
    NotEnoughPoints,
    InvalidNode(String),
    NodeNotPickable(String),
    NotInState(GameState),
    NoCurrentNode,
    ContentMissing(String),
    Forge(ForgeError),
    HealingBlocked,
    NoBattleInProgress,
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::NotEnoughChips => write!(f, "not enough ⚡"),
            RunError::NotEnoughPoints => write!(f, "not enough store PTS"),
            RunError::InvalidNode(id) => write!(f, "unknown node {id}"),
            RunError::NodeNotPickable(id) => write!(f, "node {id} is not on your path"),
            RunError::NotInState(s) => write!(f, "not allowed in state {s:?}"),
            RunError::NoCurrentNode => write!(f, "no current node"),
            RunError::ContentMissing(id) => write!(f, "content missing: {id}"),
            RunError::Forge(e) => write!(f, "forge: {e}"),
            RunError::HealingBlocked => write!(f, "healing disabled (Fragile)"),
            RunError::NoBattleInProgress => write!(f, "no battle in progress"),
        }
    }
}

/// Loadout store offer (§2.1): 5 unique random common cards + 1 common wheel.
/// Decision: common cards price 2 PTS (budget 10 = exactly 5 cards), the
/// common wheel drafts free (§4.6 commons have no points cost).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadoutOffer {
    pub card_ids: Vec<String>,
    pub wheel_id: String,
    pub card_price: u16,
}

/// Run-level state (§16.2 `RunState`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    pub hp: u16,
    pub max_hp: u16,
    /// ⚡ Essence — the same pool used for combat betting (§2.5).
    pub chips: u16,
    /// Loadout draft budget (§2.1: 10 PTS).
    pub store_points: u16,
    /// The full run deck (persistent across battles; battles copy piles).
    pub deck: Vec<CardInstance>,
    /// §16.2 `relics` — field exists; unused in v1 (no relic content yet).
    pub relics: Vec<String>,
    pub player_wheel: WheelConfig,
    pub color_levels: ColorLevels,
    /// §4.5 unlock tracking (`fever_ignition` / `glacier_core` / `synapse_link`).
    pub color_unlocks: BTreeMap<String, bool>,
    pub curses: Vec<CurseDef>,
    pub current_floor: usize,
    pub map: Map,
    pub current_node: Option<String>,
    pub combat_mode: CombatMode,
    pub difficulty: Difficulty,
    pub state: GameState,
    /// Wheels owned (starter + purchased drafts); `player_wheel` is the active one.
    pub owned_wheels: Vec<WheelConfig>,
    pub selected_wheel_id: String,
    // --- per-node caches (cleared on leaving the node) ---
    pub loadout_offer: Option<LoadoutOffer>,
    pub shop_offer: Option<ShopOffer>,
    pub forge_offer: Option<ForgeOffer>,
    pub current_event_title: Option<String>,
    pub event_flavor: Option<String>,
}

/// §4.5 color level cost: cost per level is 15 + 5(L−1) ⚡ (15, 20, 25, …),
/// with L the level being left — so reaching `target` costs 15 + 5(target−2).
pub fn level_cost(target: u8) -> u16 {
    15 + 5 * (target.saturating_sub(2) as u16)
}

/// Battle result of a completed node fight.
#[derive(Debug, Clone, PartialEq)]
pub enum BattleResult {
    /// Won the fight: rewards were rolled into `rewards`.
    Victory { rewards: Vec<String> },
    /// Lost on score with HP remaining: the node stays open for a retry.
    Defeat,
    /// HP hit 0: run over.
    GameOver,
}

impl RunState {
    /// Starts a fresh run (§2.1 verbatim defaults): HP 80/80, ⚡ 20, 10 PTS,
    /// levels red/black/green at 1, a random common wheel, no curses.
    pub fn start_new_run(content: &Content, seed: &str, difficulty: Difficulty) -> Self {
        let mut rng = Rng::from_string_seed(seed);
        let common_wheels: Vec<&roulette_content::schema::WheelDef> = content
            .wheels
            .iter()
            .filter(|w| w.rarity == roulette_content::schema::WheelRarity::Common)
            .collect();
        let wheel_def =
            rng.pick(&common_wheels).copied().expect("content must ship a common wheel");
        let wheel = WheelConfig::from_def(wheel_def);
        let selected = wheel.id.clone();
        let mut rng = rng.derive("map");
        let mut run = Self {
            hp: 80,
            max_hp: 80,
            chips: 20,
            store_points: 10,
            deck: Vec::new(),
            relics: Vec::new(),
            player_wheel: wheel,
            color_levels: ColorLevels::default(),
            color_unlocks: BTreeMap::new(),
            curses: Vec::new(),
            current_floor: 0,
            map: map::generate(difficulty.floors(), &mut rng),
            current_node: None,
            combat_mode: CombatMode::Points,
            difficulty,
            state: GameState::LoadoutStore,
            owned_wheels: Vec::new(),
            selected_wheel_id: selected,
            loadout_offer: None,
            shop_offer: None,
            forge_offer: None,
            current_event_title: None,
            event_flavor: None,
        };
        run.color_levels.level_up(SlotColor::Red, 1);
        run.color_levels.level_up(SlotColor::Black, 1);
        run.color_levels.level_up(SlotColor::Green, 1);
        run.owned_wheels.push(run.player_wheel.clone());
        run
    }

    // ------------------------------------------------------------------
    // Loadout store (§2.2: before floor 1 only)
    // ------------------------------------------------------------------

    /// Generates the loadout offer: 5 unique random common cards + 1 common wheel.
    pub fn open_loadout(&mut self, content: &Content, rng: &mut Rng) {
        debug_assert_eq!(self.state, GameState::LoadoutStore);
        let commons: Vec<&CardDef> = content
            .cards
            .iter()
            .filter(|c| c.rarity == roulette_content::schema::CardRarity::Common)
            .collect();
        let mut picked: Vec<String> = Vec::new();
        let pool = commons;
        for _ in 0..5 {
            let available: Vec<&&CardDef> =
                pool.iter().filter(|c| !picked.contains(&c.id)).collect();
            if available.is_empty() {
                break;
            }
            if let Some(card) = rng.pick(&available) {
                picked.push(card.id.clone());
            }
        }
        let wheel_id = self.player_wheel.id.clone();
        self.loadout_offer = Some(LoadoutOffer { card_ids: picked, wheel_id, card_price: 2 });
    }

    /// Drafts one offered card into the deck (2 PTS each).
    pub fn draft_card(&mut self, card_id: &str) -> Result<(), RunError> {
        let offer = self.loadout_offer.as_ref().ok_or(RunError::NotInState(self.state))?;
        if !offer.card_ids.contains(&card_id.to_string()) {
            return Err(RunError::ContentMissing(card_id.to_string()));
        }
        if self.store_points < offer.card_price {
            return Err(RunError::NotEnoughPoints);
        }
        self.store_points -= offer.card_price;
        self.deck.push(CardInstance {
            def_id: card_id.to_string(),
            marked_slots: Vec::new(),
            temp: false,
            retained: false,
            cost_override: None,
        });
        Ok(())
    }

    /// Takes the offered common wheel into the owned wheels (free).
    pub fn draft_wheel(&mut self) -> Result<(), RunError> {
        let offer = self.loadout_offer.as_ref().ok_or(RunError::NotInState(self.state))?;
        if !self.owned_wheels.iter().any(|w| w.id == offer.wheel_id) {
            self.owned_wheels.push(self.player_wheel.clone());
        }
        Ok(())
    }

    /// Finishes the loadout draft and enters the map.
    pub fn complete_loadout(&mut self) -> Result<(), RunError> {
        if self.state != GameState::LoadoutStore {
            return Err(RunError::NotInState(self.state));
        }
        self.state = GameState::Map;
        self.loadout_offer = None;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Map navigation (§2.3 path rule)
    // ------------------------------------------------------------------

    /// Nodes the player may move to: on floor 0 any node, else the current
    /// node's connections (§2.3 path rule). From the Map screen an
    /// un-completed current node is re-pickable so a score-loss walk-away
    /// can retry the node; inside a node screen it is not (offers would be
    /// regenerated forever).
    pub fn pickable_nodes(&self) -> Vec<String> {
        match &self.current_node {
            None => self
                .map
                .floors
                .first()
                .map(|row| row.iter().map(|n| n.id.clone()).collect())
                .unwrap_or_default(),
            Some(current) => {
                let mut ids = self
                    .map
                    .node(current)
                    .map(|n| n.connections.clone())
                    .unwrap_or_default();
                if self.state == GameState::Map {
                    let retry = self
                        .map
                        .node(current)
                        .map(|n| !n.completed)
                        .unwrap_or(false);
                    if retry && !ids.iter().any(|id| id == current) {
                        ids.push(current.clone());
                    }
                }
                ids
            }
        }
    }

    /// Picks the next node and opens its encounter.
    pub fn pick_node(
        &mut self,
        node_id: &str,
        content: &Content,
        rng: &mut Rng,
    ) -> Result<(), RunError> {
        let node = self
            .map
            .node(node_id)
            .ok_or_else(|| RunError::InvalidNode(node_id.to_string()))?
            .clone();
        let pickable = self.pickable_nodes();
        if !pickable.contains(&node.id) {
            return Err(RunError::NodeNotPickable(node.id));
        }
        self.current_node = Some(node.id.clone());
        self.current_floor = node.floor;
        match node.node_type {
            NodeType::Combat | NodeType::Elite | NodeType::Boss => {
                self.state = GameState::Combat;
                Ok(())
            }
            NodeType::Shop => {
                self.shop_offer = Some(ShopOffer::generate(&content.cards, &content.wheels, rng));
                self.state = GameState::Shop;
                Ok(())
            }
            NodeType::Event => {
                if let Some(ev) = crate::run::events::pick_event(&content.events, rng) {
                    self.current_event_title = Some(ev.title.clone());
                    self.event_flavor = Some(ev.flavor.clone());
                }
                self.state = GameState::Event;
                Ok(())
            }
            NodeType::Forge => {
                self.forge_offer = Some(ForgeOffer::generate(&content.forge_ops, rng));
                self.state = GameState::Forge;
                Ok(())
            }
        }
    }

    // ------------------------------------------------------------------
    // Combat lifecycle
    // ------------------------------------------------------------------

    /// Enemy tier expected at the current node.
    pub fn current_node_tier(&self) -> Result<EnemyTier, RunError> {
        let id = self.current_node.as_ref().ok_or(RunError::NoCurrentNode)?;
        let node = self.map.node(id).ok_or_else(|| RunError::InvalidNode(id.clone()))?;
        Ok(match node.node_type {
            NodeType::Combat => EnemyTier::Normal,
            NodeType::Elite => EnemyTier::Elite,
            NodeType::Boss => EnemyTier::Boss,
            _ => EnemyTier::Normal,
        })
    }

    /// Curses attached for the node tier (§8: elite 2, boss 2–3).
    pub fn roll_curses(tier: EnemyTier, content: &Content, rng: &mut Rng) -> Vec<CurseDef> {
        let count = match tier {
            EnemyTier::Normal => 0,
            EnemyTier::Elite => 2,
            EnemyTier::Boss => 2 + (rng.next_u32() % 2) as usize,
        };
        let pool: Vec<&CurseDef> = content.curses.iter().collect();
        let mut picked = Vec::new();
        for _ in 0..count {
            let available: Vec<&&CurseDef> =
                pool.iter().filter(|c| !picked.iter().any(|p: &CurseDef| p.id == c.id)).collect();
            if available.is_empty() {
                break;
            }
            if let Some(curse) = rng.pick(&available) {
                picked.push((**curse).clone());
            }
        }
        picked
    }

    /// Builds the battle for the current combat node (§3.1 + §2.5 shared pool).
    ///
    /// Pool rule: the house stakes the table at 30 ⚡ (§3.1); a bankroll above
    /// 30 carries in untouched. Curses attach per tier (elite 2, boss 2–3).
    pub fn enter_battle(
        &mut self,
        content: &Content,
        rng: &mut Rng,
    ) -> Result<BattleState, RunError> {
        if self.state != GameState::Combat {
            return Err(RunError::NotInState(self.state));
        }
        let tier = self.current_node_tier()?;
        let pool: Vec<&roulette_content::schema::EnemyDef> =
            content.enemies.iter().filter(|e| e.tier == tier).collect();
        let enemy_def = rng
            .pick(&pool)
            .copied()
            .ok_or_else(|| RunError::ContentMissing(format!("enemy of tier {tier:?}")))?;
        let enemy_wheel = content
            .wheels
            .iter()
            .find(|w| w.id == enemy_def.wheel_id)
            .cloned()
            .ok_or_else(|| RunError::ContentMissing(enemy_def.wheel_id.clone()))?;

        let curses = Self::roll_curses(tier, content, rng);
        self.curses = curses.clone();

        let max_rounds = match tier {
            EnemyTier::Normal => 3,
            EnemyTier::Elite => 5,
            EnemyTier::Boss => 8,
        };
        let mut battle = BattleState::new(
            self.player_wheel.clone(),
            crate::battle::state::BattleState::enemy_wheel_from_def(&enemy_wheel),
            self.hp,
            self.max_hp,
            enemy_def.max_hp,
            max_rounds,
            curses,
            self.combat_mode,
        );
        // §3.1: pool starts at 30 (or 15 under Greed — handled by the curse
        // override); a carried bankroll above the floor rides in.
        if battle.chips_pool < self.chips {
            battle.chips_pool = self.chips;
            battle.turn_start_pool = self.chips;
        }
        battle.enemy_difficulty = enemy_def.difficulty;
        battle.player_levels = self.color_levels.clone();
        battle.load_intent(&enemy_def.intent_pattern);
        // Deal the run deck (§3.1: exactly 2 cards at start).
        let defs: Vec<CardDef> = self
            .deck
            .iter()
            .filter_map(|i| content.cards.iter().find(|c| c.id == i.def_id).cloned())
            .collect();
        battle.deal_from_defs(&defs, rng);
        // §4.5 unlocks arm the streak trackers.
        if self.player_wheel.unlocked_abilities.contains(&SlotColor::Red) {
            battle.player_board.red_streak_active = true;
        }
        if self.player_wheel.unlocked_abilities.contains(&SlotColor::Black) {
            battle.player_board.black_streak_active = true;
        }
        Ok(battle)
    }

    /// Folds a finished battle back into the run (§2.5): chips carry out via
    /// the shared pool (plus the Capital Venture bank), HP persists, and a
    /// victory rolls rewards, completes the node and advances the flow.
    pub fn complete_battle(
        &mut self,
        content: &Content,
        battle: &mut BattleState,
        rng: &mut Rng,
    ) -> Result<BattleResult, RunError> {
        if self.state != GameState::Combat {
            return Err(RunError::NotInState(self.state));
        }
        self.chips = battle.chips_pool;
        // Capital Venture banks ⚡ on round wins; the run layer drains it here.
        self.chips = self.chips.saturating_add(battle.venture_bank);
        battle.venture_bank = 0;
        self.hp = battle.player_hp;
        if self.hp == 0 {
            self.state = GameState::GameOver;
            return Ok(BattleResult::GameOver);
        }
        let victory = battle.chips_pool > battle.enemy_chips_pool;
        if !victory {
            // Score loss with HP left: walk away and retry the node.
            self.state = GameState::Map;
            return Ok(BattleResult::Defeat);
        }
        let tier = self.current_node_tier()?;
        let granted = rewards::reward_cards(&content.cards, tier, rng)
            .into_iter()
            .map(|c| {
                self.deck.push(CardInstance {
                    def_id: c.id.clone(),
                    marked_slots: Vec::new(),
                    temp: false,
                    retained: false,
                    cost_override: None,
                });
                c.id.clone()
            })
            .collect();
        let boss_beaten = tier == EnemyTier::Boss;
        self.complete_node()?;
        if boss_beaten {
            self.state = GameState::Victory;
        }
        Ok(BattleResult::Victory { rewards: granted })
    }

    /// Completes the current node and returns to the map (non-combat exits:
    /// leaving shop/forge/event screens).
    pub fn complete_node(&mut self) -> Result<(), RunError> {
        let id = self.current_node.clone().ok_or(RunError::NoCurrentNode)?;
        let node = self.map.node_mut(&id).ok_or(RunError::InvalidNode(id))?;
        node.completed = true;
        self.state = GameState::Map;
        // Clear per-node caches (§9.1: RETURN TO PATHS clears the cache).
        self.shop_offer = None;
        self.forge_offer = None;
        self.current_event_title = None;
        self.event_flavor = None;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Shop purchases (§9.1)
    // ------------------------------------------------------------------

    /// Buys a shop item by index.
    pub fn shop_buy(&mut self, item_index: usize) -> Result<ShopItemBought, RunError> {
        if self.state != GameState::Shop {
            return Err(RunError::NotInState(self.state));
        }
        let offer = self.shop_offer.as_ref().ok_or(RunError::NotInState(self.state))?;
        let item = offer
            .items
            .get(item_index)
            .ok_or_else(|| RunError::ContentMissing(format!("shop item {item_index}")))?;
        if self.chips < item.price() {
            return Err(RunError::NotEnoughChips);
        }
        if matches!(item, ShopItem::BloodInfusion { .. }) && !self.healing_allowed() {
            return Err(RunError::HealingBlocked);
        }
        self.chips -= item.price();
        Ok(match item.clone() {
            ShopItem::Card { def, .. } => {
                self.deck.push(CardInstance {
                    def_id: def.id.clone(),
                    marked_slots: Vec::new(),
                    temp: false,
                    retained: false,
                    cost_override: None,
                });
                ShopItemBought::Card(def.id)
            }
            ShopItem::Wheel { def, .. } => {
                let wheel = WheelConfig::from_def(&def);
                self.owned_wheels.push(wheel.clone());
                ShopItemBought::Wheel(wheel.id)
            }
            ShopItem::BloodInfusion { .. } => {
                self.hp = (self.hp + shop::BLOOD_INFUSION_HEAL).min(self.max_hp);
                ShopItemBought::Heal(shop::BLOOD_INFUSION_HEAL)
            }
        })
    }

    /// Fragile disables all healing (§8).
    pub fn healing_allowed(&self) -> bool {
        !self
            .curses
            .iter()
            .any(|c| matches!(c.effect, roulette_content::schema::CurseEffect::DisableHealing))
    }

    /// Equips an owned wheel as the active player wheel.
    pub fn select_wheel(&mut self, wheel_id: &str) -> Result<(), RunError> {
        let wheel = self
            .owned_wheels
            .iter()
            .find(|w| w.id == wheel_id)
            .cloned()
            .ok_or_else(|| RunError::ContentMissing(wheel_id.to_string()))?;
        self.player_wheel = wheel;
        self.selected_wheel_id = wheel_id.to_string();
        Ok(())
    }

    // ------------------------------------------------------------------
    // Forge (§9.2)
    // ------------------------------------------------------------------

    /// Takes a forge offer: free while budget lasts, else priced in ⚡.
    pub fn forge_take(&mut self, op_index: usize, rng: &mut Rng) -> Result<(), RunError> {
        if self.state != GameState::Forge {
            return Err(RunError::NotInState(self.state));
        }
        let offer = self.forge_offer.as_ref().ok_or(RunError::NotInState(self.state))?;
        let price = offer
            .op_price(op_index)
            .ok_or_else(|| RunError::ContentMissing(format!("forge op {op_index}")))?;
        if self.chips < price {
            return Err(RunError::NotEnoughChips);
        }
        let op = offer.ops[op_index].clone();
        let green_level = self.color_levels.get(SlotColor::Green);
        forge::apply_op(&mut self.player_wheel, &op, green_level, rng).map_err(RunError::Forge)?;
        self.chips -= price;
        self.sync_green_ladder();
        if let Some(o) = self.forge_offer.as_mut() {
            o.free_ops_remaining = o.free_ops_remaining.saturating_sub(1);
            o.ops.remove(op_index);
        }
        // Keep the active wheel in sync with the customized copy.
        self.owned_wheels =
            self.owned_wheels
                .iter()
                .map(|w| {
                    if w.id == self.player_wheel.id {
                        self.player_wheel.clone()
                    } else {
                        w.clone()
                    }
                })
                .collect();
        Ok(())
    }

    /// Rerolls the forge offers for 5 ⚡ (§9.2).
    pub fn forge_reroll(&mut self, content: &Content, rng: &mut Rng) -> Result<(), RunError> {
        if self.state != GameState::Forge {
            return Err(RunError::NotInState(self.state));
        }
        if self.chips < forge::REROLL_PRICE {
            return Err(RunError::NotEnoughChips);
        }
        self.chips -= forge::REROLL_PRICE;
        if let Some(o) = self.forge_offer.as_mut() {
            o.reroll(&content.forge_ops, rng);
        }
        Ok(())
    }

    /// Keeps the wheel's green ladder slots in sync with the run level (§4.5).
    fn sync_green_ladder(&mut self) {
        let level = self.color_levels.get(SlotColor::Green);
        self.player_wheel.apply_green_ladder(level);
    }

    // ------------------------------------------------------------------
    // Events (§9.4)
    // ------------------------------------------------------------------

    /// Applies the chosen event choice to the run.
    pub fn event_choose(
        &mut self,
        content: &Content,
        choice_id: &str,
    ) -> Result<EventOutcome, RunError> {
        if self.state != GameState::Event {
            return Err(RunError::NotInState(self.state));
        }
        let event = content
            .events
            .iter()
            .find(|e| Some(&e.title) == self.current_event_title.as_ref())
            .ok_or_else(|| RunError::ContentMissing("current event".into()))?;
        let choice = event
            .choices
            .iter()
            .find(|c| c.id == choice_id)
            .ok_or_else(|| RunError::ContentMissing(choice_id.to_string()))?
            .clone();
        let mut deck = std::mem::take(&mut self.deck);
        let outcome = apply_choice_effects(
            &choice.effects,
            &content.cards,
            &mut self.hp,
            self.max_hp,
            &mut self.chips,
            &mut deck,
        );
        self.deck = deck;
        if self.hp == 0 {
            self.state = GameState::GameOver;
        } else {
            self.complete_node()?;
        }
        Ok(outcome)
    }

    // ------------------------------------------------------------------
    // Color levels & unlocks (§4.5)
    // ------------------------------------------------------------------

    /// Buys a level for `color` at 15 + 5(L−1) ⚡ (max 10).
    pub fn buy_color_level(&mut self, color: SlotColor) -> Result<u8, RunError> {
        let current = self.color_levels.get(color);
        if current >= MAX_COLOR_LEVEL {
            return Err(RunError::NotEnoughChips);
        }
        let target = current + 1;
        let cost = level_cost(target);
        if self.chips < cost {
            return Err(RunError::NotEnoughChips);
        }
        self.chips -= cost;
        self.color_levels.level_up(color, 1);
        self.sync_green_ladder();
        Ok(target)
    }

    // ------------------------------------------------------------------
    // Snapshots (REQ-009 undo)
    // ------------------------------------------------------------------

    /// Serializes the run for undo snapshots.
    pub fn snapshot(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Restores a run snapshot.
    pub fn restore(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Derives a run-scoped RNG substream (deterministic per seed + label).
    pub fn rng(&self, seed: &str, label: &str) -> Rng {
        Rng::from_string_seed(seed).derive(label)
    }
}

/// What a shop purchase granted.
#[derive(Debug, Clone, PartialEq)]
pub enum ShopItemBought {
    Card(String),
    Wheel(String),
    Heal(u16),
}

/// Node accessor helper for UIs.
pub fn node_type_at(run: &RunState, node_id: &str) -> Option<NodeType> {
    run.map.node(node_id).map(|n: &MapNode| n.node_type)
}
