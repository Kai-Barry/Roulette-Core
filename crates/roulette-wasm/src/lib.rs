//! wasm-bindgen facade over the headless `roulette_core::api::Engine` (PLAN
//! Phase 1, TASK-006). The JS side never touches game rules: it sends
//! serialized `Command` JSON and receives `EngineEvent` JSON + a state
//! snapshot (REQ-001). Content RON is embedded at build time so the wasm
//! bundle is self-contained (REQ-002).

use std::sync::Arc;

use roulette_content::Content;
use roulette_core::api::{Command, Engine, EngineError};
use wasm_bindgen::prelude::*;

/// One live engine instance exposed to JS.
#[wasm_bindgen]
pub struct EngineHandle {
    engine: Engine,
    seed: String,
}

/// Constructs an engine over the embedded validated content (seed is the
/// deterministic djb2 string seed used by the whole engine stack).
#[wasm_bindgen]
pub fn new_engine(seed: String) -> Result<EngineHandle, JsValue> {
    console_error_panic_hook::set_once();
    let content: Arc<Content> = Arc::new(
        Content::embedded()
            .map_err(|e| JsValue::from_str(&format!("embedded content invalid: {e}")))?,
    );
    Ok(EngineHandle { engine: Engine::new(content, &seed), seed })
}

#[wasm_bindgen]
impl EngineHandle {
    /// Applies one command; returns a JSON array string of the events it
    /// produced, or `Err` with an `{error, kind, message}` JSON string
    /// (SEC-002: errors verbatim, no client-side fix-ups). Plain JSON strings
    /// keep the JS side dependency-free: `JSON.parse` is the only glue.
    pub fn apply_command(&mut self, cmd_json: &str) -> Result<String, String> {
        let cmd: Command = serde_json::from_str(cmd_json).map_err(|e| {
            format!(
                "{{\"error\":true,\"kind\":\"bad_command\",\"message\":\"bad command json: {e}\"}}"
            )
        })?;
        match self.engine.apply(&cmd) {
            Ok(events) => {
                let json: Vec<serde_json::Value> = events
                    .iter()
                    .map(|e| serde_json::to_value(e).expect("EngineEvent is serializable"))
                    .collect();
                Ok(serde_json::to_string(&json).expect("array string"))
            }
            Err(e) => Err(engine_error_json(&e)),
        }
    }

    /// Full serializable state snapshot as a JSON string: run + battle +
    /// wheel + prediction + undo depth.
    pub fn get_state_json(&self) -> String {
        let state = serde_json::json!({
            "seed": self.seed.clone(),
            // No run yet ⇒ menu (the only reachable no-run state).
            "game_state": self.engine.run().map(|r| r.state).map(state_name).unwrap_or("menu"),
            "run": self.engine.run(),
            "battle": self.engine.battle(),
            "undo_depth": self.engine.undo_depth(),
        });
        serde_json::to_string(&state).expect("state string")
    }

    /// Undoes the last undoable command (returns true if a snapshot existed).
    pub fn undo(&mut self) -> Result<bool, JsValue> {
        if self.engine.undo_depth() == 0 {
            return Ok(false);
        }
        self.engine
            .apply(&Command::Undo)
            .map(|_| true)
            .map_err(|e| JsValue::from_str(&engine_error_json(&e)))
    }

    pub fn undo_depth(&self) -> usize {
        self.engine.undo_depth()
    }

    pub fn seed(&self) -> String {
        self.seed.clone()
    }

    /// Spin telemetry side channel (TASK-008, feature `telemetry`): packed
    /// fixed-dt frames for the LAST spin — see
    /// `roulette_core::api::Engine::spin_telemetry_bytes` for the layout.
    /// `side`: 0 = player wheel, 1 = enemy wheel. Empty array when the side
    /// was not simulated (uniform fast path).
    #[cfg(feature = "telemetry")]
    pub fn spin_telemetry(&self, side: u8) -> Vec<u8> {
        self.engine.spin_telemetry_bytes(side).unwrap_or_default()
    }

    /// Physics sim events for the LAST spin with telemetry frame indices
    /// (TASK-029, feature `telemetry`, CON-001 additive forwarding): JSON
    /// array of `{"frame": u32, "event": <SimEvent>}` — see
    /// `roulette_core::api::Engine::spin_sim_events_json`. `side`: 0 = player,
    /// 1 = enemy. `"[]"` when the side was not simulated (uniform fast path).
    #[cfg(feature = "telemetry")]
    pub fn spin_sim_events(&self, side: u8) -> String {
        self.engine.spin_sim_events_json(side).unwrap_or_else(|| "[]".to_string())
    }
}

fn state_name(gs: roulette_core::run::state::GameState) -> &'static str {
    use roulette_core::run::state::GameState as G;
    match gs {
        G::Menu => "menu",
        G::LoadoutStore => "loadout_store",
        G::Map => "map",
        G::Combat => "combat",
        G::Shop => "shop",
        G::Event => "event",
        G::Forge => "forge",
        G::Victory => "victory",
        G::GameOver => "game_over",
    }
}

/// EngineError is not serde-serializable upstream; expose a stable
/// `{error, kind, message}` shape for the JS bridge (SEC-002: verbatim text).
fn engine_error_json(e: &EngineError) -> String {
    let kind = match e {
        EngineError::NoRun => "no_run",
        EngineError::NotInBattle => "not_in_battle",
        EngineError::Run(_) => "run",
        EngineError::Action(_) => "action",
        EngineError::Content(_) => "content",
        EngineError::PredictionBlocked => "prediction_blocked",
        EngineError::PredictionUnavailable => "prediction_unavailable",
        EngineError::NoUndo => "no_undo",
    };
    serde_json::json!({ "error": true, "kind": kind, "message": e.to_string() }).to_string()
}
