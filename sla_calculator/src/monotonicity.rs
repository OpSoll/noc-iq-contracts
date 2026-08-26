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