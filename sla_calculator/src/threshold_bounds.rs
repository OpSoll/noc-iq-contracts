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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimum_and_maximum_are_accepted() {
        let env = Env::default();
        assert_eq!(validate_threshold_minutes(&env, MIN_THRESHOLD_MINUTES), Ok(()));
        assert_eq!(validate_threshold_minutes(&env, MAX_THRESHOLD_MINUTES), Ok(()));
    }

    #[test]
    fn test_below_minimum_is_rejected() {
        let env = Env::default();
        assert_eq!(
            validate_threshold_minutes(&env, MIN_THRESHOLD_MINUTES - 1),
            Err(ThresholdValidationError::ThresholdOutOfBounds)
        );
    }

    #[test]
    fn test_above_maximum_is_rejected() {
        let env = Env::default();
        assert_eq!(
            validate_threshold_minutes(&env, MAX_THRESHOLD_MINUTES + 1),
            Err(ThresholdValidationError::ThresholdOutOfBounds)
        );
        assert_eq!(
            validate_threshold_minutes(&env, u32::MAX),
            Err(ThresholdValidationError::ThresholdOutOfBounds)
        );
    }

    #[test]
    fn test_bounds_span_a_full_day() {
        assert_eq!(MAX_THRESHOLD_MINUTES, 24 * 60);
        assert!(MIN_THRESHOLD_MINUTES < MAX_THRESHOLD_MINUTES);
    }
}
