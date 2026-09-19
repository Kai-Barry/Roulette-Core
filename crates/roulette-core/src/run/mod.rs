//! Run layer (Phase 6, GOAL-007): map generation (§2.3), node resolution,
//! shop/forge/events, curses, color levels, and the run flow API per §16.2
//! `RunState`.

pub mod events;
pub mod forge;
pub mod map;
pub mod rewards;
pub mod shop;
pub mod state;

pub use events::EventOutcome;
pub use forge::{ForgeError, ForgeOffer};
pub use map::{Map, MapNode, NodeType};
pub use shop::{ShopItem, ShopOffer};
pub use state::{
    level_cost, BattleResult, Difficulty, GameState, LoadoutOffer, RunError, RunState,
    ShopItemBought,
};
