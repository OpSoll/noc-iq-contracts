use soroban_sdk::Env;

pub const MIN_THRESHOLD_MINUTES: u32 = 1;
pub const MAX_THRESHOLD_MINUTES: u32 = 1440;

#[derive(Debug, Clone, PartialEq)]
pub enum ThresholdValidationError {
    ThresholdOutOfBounds,
}

/// Validates that `threshold_minutes` stays within the 24-hour SLA cap.
pub fn validate_threshold_minutes(
    _env: &Env,
    threshold_minutes: u32,
) -> Result<(), ThresholdValidationError> {
    if threshold_minutes < MIN_THRESHOLD_MINUTES
        || threshold_minutes > MAX_THRESHOLD_MINUTES
    {
        return Err(ThresholdValidationError::ThresholdOutOfBounds);
    }
    Ok(())
}
