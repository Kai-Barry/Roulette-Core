//! Content validator placeholder (Phase 1).

/// Validation error taxonomy (fleshed out in Phase 1).
#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("validation not yet implemented")]
    NotImplemented,
}

/// Validates a content bundle (fleshed out in Phase 1).
pub fn validate_noop() -> Result<(), ContentError> {
    Ok(())
}
