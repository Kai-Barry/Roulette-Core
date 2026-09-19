//! Card effect layer (Phase 5): typed-DSL dispatch (§6.2/§6.4) and the
//! points-mode description rewriter (§6.5).

pub mod effects;
pub mod format;

pub use effects::{EffectCtx, EffectError};
