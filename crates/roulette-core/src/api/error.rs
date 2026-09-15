//! `EngineError` taxonomy (TASK-042).

use crate::battle::state::ActionError;
use crate::run::state::RunError;

// Manual `std::error::Error` impls (the inner types use hand-written Display)
// so `#[from]`/`#[transparent]` thiserror plumbing works on the facade.
impl std::error::Error for RunError {}
impl std::error::Error for ActionError {}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    #[error("no run in progress (call StartRun first)")]
    NoRun,
    #[error("command requires an active battle")]
    NotInBattle,
    #[error(transparent)]
    Run(#[from] RunError),
    #[error(transparent)]
    Action(#[from] ActionError),
    #[error("content problem: {0}")]
    Content(String),
    #[error("prediction is corrupted by a curse (§8)")]
    PredictionBlocked,
    #[error("no prediction available (wheel has no prediction upgrade)")]
    PredictionUnavailable,
    #[error("nothing to undo")]
    NoUndo,
}