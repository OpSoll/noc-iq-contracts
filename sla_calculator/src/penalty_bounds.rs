use soroban_sdk::Env;

pub const MIN_PENALTY_PER_MINUTE: i128 = 1;
pub const MAX_PENALTY_PER_MINUTE: i128 = 10_000;

#[derive(Debug, Clone, PartialEq)]
pub enum PenaltyValidationError {
    PenaltyOutOfBounds,
}

/// Validates that `penalty_per_minute` falls within the allowed range.
pub fn validate_penalty_per_minute(
    _env: &Env,
    penalty_per_minute: i128,
) -> Result<(), PenaltyValidationError> {
    if penalty_per_minute < MIN_PENALTY_PER_MINUTE
        || penalty_per_minute > MAX_PENALTY_PER_MINUTE
    {
        return Err(PenaltyValidationError::PenaltyOutOfBounds);
    }
    Ok(())
}
