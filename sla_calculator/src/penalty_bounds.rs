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
    if penalty_per_minute < MIN_PENALTY_PER_MINUTE || penalty_per_minute > MAX_PENALTY_PER_MINUTE {
        return Err(PenaltyValidationError::PenaltyOutOfBounds);
    }
    Ok(())
}

/// View returning the configured penalty bounds as `(floor, ceiling)`.
///
/// These are the same constants [`validate_penalty_per_minute`] enforces, so
/// a caller can see the accepted range without having to probe it by failure.
pub fn penalty_bounds(_env: &Env) -> (i128, i128) {
    (MIN_PENALTY_PER_MINUTE, MAX_PENALTY_PER_MINUTE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    fn accepted(env: &Env, penalty: i128) -> bool {
        validate_penalty_per_minute(env, penalty).is_ok()
    }

    #[test]
    fn penalties_are_rejected_below_the_floor() {
        let env = Env::default();
        assert!(!accepted(&env, MIN_PENALTY_PER_MINUTE - 1));
        assert!(!accepted(&env, 0));
        assert!(!accepted(&env, -1));
        assert!(!accepted(&env, i128::MIN));
    }

    #[test]
    fn penalties_are_rejected_above_the_ceiling() {
        let env = Env::default();
        assert!(!accepted(&env, MAX_PENALTY_PER_MINUTE + 1));
        assert!(!accepted(&env, i128::MAX));
    }

    #[test]
    fn the_floor_and_the_ceiling_are_both_accepted() {
        let env = Env::default();
        // Exactly-on-the-boundary must pass; only crossing it fails.
        assert!(accepted(&env, MIN_PENALTY_PER_MINUTE));
        assert!(accepted(&env, MAX_PENALTY_PER_MINUTE));
        assert_eq!(
            validate_penalty_per_minute(&env, MIN_PENALTY_PER_MINUTE - 1),
            Err(PenaltyValidationError::PenaltyOutOfBounds)
        );
        assert_eq!(
            validate_penalty_per_minute(&env, MAX_PENALTY_PER_MINUTE + 1),
            Err(PenaltyValidationError::PenaltyOutOfBounds)
        );
    }

    #[test]
    fn view_reports_the_bounds_the_validator_enforces() {
        let env = Env::default();
        let (floor, ceiling) = penalty_bounds(&env);
        assert_eq!(floor, MIN_PENALTY_PER_MINUTE);
        assert_eq!(ceiling, MAX_PENALTY_PER_MINUTE);
        assert!(floor < ceiling);

        // The view must describe exactly the accepted range: nothing inside is
        // rejected, nothing outside is accepted.
        for penalty in [
            floor - 1,
            floor,
            floor + 1,
            (floor + ceiling) / 2,
            ceiling - 1,
            ceiling,
            ceiling + 1,
        ] {
            let within = penalty >= floor && penalty <= ceiling;
            assert_eq!(
                accepted(&env, penalty),
                within,
                "view and validator disagree at penalty {penalty}"
            );
        }
    }
}
