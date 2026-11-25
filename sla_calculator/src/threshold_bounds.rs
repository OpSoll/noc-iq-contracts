use soroban_sdk::Env;

use crate::SLAError;

pub const MIN_THRESHOLD_MINUTES: u32 = 1;
pub const MAX_THRESHOLD_MINUTES: u32 = 1440;

/// #1040 — stable, tooling-facing codes for threshold bound rejections.
///
/// Both values match the corresponding `SLAError` discriminant so an operator
/// can tell a too-small threshold from a too-large one without string
/// matching on messages.
pub const THRESHOLD_BELOW_MINIMUM_CODE: u32 = 25;
pub const THRESHOLD_ABOVE_MAXIMUM_CODE: u32 = 26;

/// Which safety bound a rejected threshold value crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectedBound {
    /// The value was below `MIN_THRESHOLD_MINUTES`.
    Lower,
    /// The value was above `MAX_THRESHOLD_MINUTES`.
    Upper,
}

/// #1040 — an out-of-range threshold keeps the bound that rejected it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdValidationError {
    /// `threshold_minutes < MIN_THRESHOLD_MINUTES`
    BelowMinimum,
    /// `threshold_minutes > MAX_THRESHOLD_MINUTES`
    AboveMaximum,
}

impl ThresholdValidationError {
    /// Stable error code for logs, dashboards and operator tooling.
    pub fn code(&self) -> u32 {
        match self {
            Self::BelowMinimum => THRESHOLD_BELOW_MINIMUM_CODE,
            Self::AboveMaximum => THRESHOLD_ABOVE_MAXIMUM_CODE,
        }
    }

    /// The bound that rejected the value.
    pub fn bound(&self) -> RejectedBound {
        match self {
            Self::BelowMinimum => RejectedBound::Lower,
            Self::AboveMaximum => RejectedBound::Upper,
        }
    }

    /// The contract-level error carrying the same stable code.
    pub fn to_sla_error(&self) -> SLAError {
        match self {
            Self::BelowMinimum => SLAError::ThresholdBelowMinimum,
            Self::AboveMaximum => SLAError::ThresholdAboveMaximum,
        }
    }
}

/// Validates that `threshold_minutes` stays within the 24-hour SLA cap,
/// reporting which bound was violated when it does not.
pub fn validate_threshold_minutes(
    _env: &Env,
    threshold_minutes: u32,
) -> Result<(), ThresholdValidationError> {
    if threshold_minutes < MIN_THRESHOLD_MINUTES {
        return Err(ThresholdValidationError::BelowMinimum);
    }
    if threshold_minutes > MAX_THRESHOLD_MINUTES {
        return Err(ThresholdValidationError::AboveMaximum);
    }
    Ok(())
}

/// #1017 — a threshold as it was configured and as it will actually be used.
/// Keeping both values visible makes it explicit when a safety bound clamps
/// the stored configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveThreshold {
    pub configured_minutes: u32,
    pub effective_minutes: u32,
    pub clamped: bool,
}

/// Resolve the effective threshold, clamping into `[MIN, MAX]` while always
/// reporting the originally configured value alongside it.
pub fn resolve_effective_threshold(
    threshold_minutes: u32,
) -> Result<EffectiveThreshold, ThresholdValidationError> {
    if threshold_minutes < MIN_THRESHOLD_MINUTES {
        return Ok(EffectiveThreshold {
            configured_minutes: threshold_minutes,
            effective_minutes: MIN_THRESHOLD_MINUTES,
            clamped: true,
        });
    }
    if threshold_minutes > MAX_THRESHOLD_MINUTES {
        return Ok(EffectiveThreshold {
            configured_minutes: threshold_minutes,
            effective_minutes: MAX_THRESHOLD_MINUTES,
            clamped: true,
        });
    }
    Ok(EffectiveThreshold {
        configured_minutes: threshold_minutes,
        effective_minutes: threshold_minutes,
        clamped: false,
    })
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
            Err(ThresholdValidationError::BelowMinimum)
        );
    }

    #[test]
    fn test_above_maximum_is_rejected() {
        let env = Env::default();
        assert_eq!(
            validate_threshold_minutes(&env, MAX_THRESHOLD_MINUTES + 1),
            Err(ThresholdValidationError::AboveMaximum)
        );
        assert_eq!(
            validate_threshold_minutes(&env, u32::MAX),
            Err(ThresholdValidationError::AboveMaximum)
        );
    }

    #[test]
    fn test_bounds_span_a_full_day() {
        assert_eq!(MAX_THRESHOLD_MINUTES, 24 * 60);
        assert!(MIN_THRESHOLD_MINUTES < MAX_THRESHOLD_MINUTES);
    }
}

#[cfg(test)]
mod bound_identification_tests {
    use super::*;

    #[test]
    fn a_below_minimum_rejection_names_the_lower_bound() {
        let env = Env::default();
        let rejection = validate_threshold_minutes(&env, 0).unwrap_err();
        assert_eq!(rejection, ThresholdValidationError::BelowMinimum);
        assert_eq!(rejection.bound(), RejectedBound::Lower);
        assert_eq!(rejection.code(), 25);
        assert_eq!(rejection.code(), SLAError::ThresholdBelowMinimum as u32);
        assert_eq!(rejection.to_sla_error(), SLAError::ThresholdBelowMinimum);
    }

    #[test]
    fn an_above_maximum_rejection_names_the_upper_bound() {
        let env = Env::default();
        let rejection = validate_threshold_minutes(&env, MAX_THRESHOLD_MINUTES + 1).unwrap_err();
        assert_eq!(rejection, ThresholdValidationError::AboveMaximum);
        assert_eq!(rejection.bound(), RejectedBound::Upper);
        assert_eq!(rejection.code(), 26);
        assert_eq!(rejection.code(), SLAError::ThresholdAboveMaximum as u32);
        assert_eq!(rejection.to_sla_error(), SLAError::ThresholdAboveMaximum);
    }

    #[test]
    fn the_two_bounds_keep_distinct_stable_codes() {
        assert_eq!(THRESHOLD_ABOVE_MAXIMUM_CODE, THRESHOLD_BELOW_MINIMUM_CODE + 1);
        assert_eq!(THRESHOLD_BELOW_MINIMUM_CODE, 25);
        assert_eq!(THRESHOLD_ABOVE_MAXIMUM_CODE, 26);
    }
}

#[cfg(test)]
mod effective_threshold_tests {
    use super::*;

    #[test]
    fn a_value_below_the_range_is_reported_as_clamped() {
        let resolved = resolve_effective_threshold(MIN_THRESHOLD_MINUTES - 1).unwrap();
        assert_eq!(resolved.configured_minutes, MIN_THRESHOLD_MINUTES - 1);
        assert_eq!(resolved.effective_minutes, MIN_THRESHOLD_MINUTES);
        assert!(resolved.clamped);
    }

    #[test]
    fn a_value_within_the_range_is_reported_unchanged() {
        let resolved = resolve_effective_threshold(60).unwrap();
        assert_eq!(resolved.configured_minutes, 60);
        assert_eq!(resolved.effective_minutes, 60);
        assert!(!resolved.clamped);
    }

    #[test]
    fn a_value_above_the_range_is_reported_as_clamped() {
        let resolved = resolve_effective_threshold(MAX_THRESHOLD_MINUTES + 1).unwrap();
        assert_eq!(resolved.configured_minutes, MAX_THRESHOLD_MINUTES + 1);
        assert_eq!(resolved.effective_minutes, MAX_THRESHOLD_MINUTES);
        assert!(resolved.clamped);
    }
}
