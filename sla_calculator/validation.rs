use soroban_sdk::{contracttype, Env, String, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchValidationItem {
    pub outage_id: String,
    pub mttr: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    InvalidOutageId,
    InvalidMttr,
}

pub struct BatchValidator;

impl BatchValidator {
    /// Validates batch request parameters ensuring non-empty outage IDs within length limits
    /// and strictly positive MTTR values.
    pub fn validate_item(env: &Env, item: &BatchValidationItem) -> Result<(), ValidationError> {
        let len = item.outage_id.len();
        if len == 0 || len > 64 {
            return Err(ValidationError::InvalidOutageId);
        }

        if item.mttr == 0 {
            return Err(ValidationError::InvalidMttr);
        }

        Ok(())
    }
}