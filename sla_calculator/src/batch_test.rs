#[cfg(test)]
mod batch_atomicity_tests {
    use super::*;
    use soroban_sdk::{vec, Env};

    #[test]
    fn test_strict_mode_aborts_on_first_error() {
        let env = Env::default();
        let items = vec![
            &env,
            BatchItemInput { id: 1, value: 50 },
            BatchItemInput { id: 2, value: 0 }, // Triggers error
            BatchItemInput { id: 3, value: 100 },
        ];

        let result = BatchCalculator::process_batch(&env, items, true);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), Symbol::new(&env, "InvalidItemStrictAbort"));
    }

    #[test]
    fn test_non_strict_mode_captures_errors_and_continues() {
        let env = Env::default();
        let items = vec![
            &env,
            BatchItemInput { id: 1, value: 50 },
            BatchItemInput { id: 2, value: 0 }, // Records error and continues
            BatchItemInput { id: 3, value: 100 },
        ];

        let result = BatchCalculator::process_batch(&env, items, false);
        assert!(result.is_ok());

        let batch_res = result.unwrap();
        assert!(batch_res.has_failed);
        assert_eq!(batch_res.processed_count, 3);
        
        let second_item = batch_res.results.get(1).unwrap();
        assert!(second_item.error.is_some());
        
        let third_item = batch_res.results.get(2).unwrap();
        assert_eq!(third_item.result, 200);
        assert!(third_item.error.is_none());
    }
}