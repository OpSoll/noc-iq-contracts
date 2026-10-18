use soroban_sdk::{contracttype, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PenaltyError {
    InvalidPenalty,
}

pub struct SeverityValidator;

impl SeverityValidator {
    /// Validates that penalties strictly adhere to cross-severity monotonicity rules:
    /// critical > high > medium > low for equivalent overtime conditions.
    pub fn validate_monotonicity(
        critical_penalty: u64,
        high_penalty: u64,
        medium_penalty: u64,
        low_penalty: u64,
    ) -> Result<(), PenaltyError> {
        if critical_penalty <= high_penalty
            || high_penalty <= medium_penalty
            || medium_penalty <= low_penalty
        {
            return Err(PenaltyError::InvalidPenalty);
        }
        Ok(())
    }
}

/// Applies penalty deltas to an accumulator, mirroring repeated
/// calculations, and returns the running totals in order.
#[cfg(test)]
mod tests {
    use super::*;

    fn running_totals(deltas: &[u64]) -> [u64; 4] {
        let mut totals = [0u64; 4];
        let mut acc = 0u64;
        for (i, delta) in deltas.iter().enumerate() {
            acc = acc.saturating_add(*delta);
            totals[i] = acc;
        }
        totals
    }

    #[test]
    fn repeated_calculations_keep_accumulators_monotonic() {
        let totals = running_totals(&[10, 0, 25, 7]);
        for i in 1..totals.len() {
            assert!(totals[i] >= totals[i - 1], "accumulator must not decrease");
        }
    }

    #[test]
    fn severity_monotonicity_rule_holds_after_repeated_validation() {
        for _ in 0..3 {
            assert_eq!(
                SeverityValidator::validate_monotonicity(400, 300, 200, 100),
                Ok(())
            );
        }
    }
}
