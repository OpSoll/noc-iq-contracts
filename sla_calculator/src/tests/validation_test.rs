#[cfg(test)]
mod batch_validation_tests {
    use super::*;
    use soroban_sdk::{Env, String};

    #[test]
    fn test_valid_batch_parameters() {
        let env = Env::default();
        let item = BatchValidationItem {
            outage_id: String::from_str(&env, "outage-alpha-101"),
            mttr: 120,
        };

        assert!(BatchValidator::validate_item(&env, &item).is_ok());
    }

    #[test]
    fn test_invalid_outage_id_boundaries() {
        let env = Env::default();
        
        // Empty outage ID
        let empty_item = BatchValidationItem {
            outage_id: String::from_str(&env, ""),
            mttr: 60,
        };
        assert_eq!(
            BatchValidator::validate_item(&env, &empty_item),
            Err(ValidationError::InvalidOutageId)
        );

        // Excessively long outage ID (> 64 chars)
        let long_str = "x".repeat(65);
        let long_item = BatchValidationItem {
            outage_id: String::from_str(&env, &long_str),
            mttr: 60,
        };
        assert_eq!(
            BatchValidator::validate_item(&env, &long_item),
            Err(ValidationError::InvalidOutageId)
        );
    }

    #[test]
    fn test_invalid_mttr_zero() {
        let env = Env::default();
        let item = BatchValidationItem {
            outage_id: String::from_str(&env, "outage-beta-202"),
            mttr: 0,
        };

        assert_eq!(
            BatchValidator::validate_item(&env, &item),
            Err(ValidationError::InvalidMttr)
        );
    }
}