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
            BatchItemInput { id: 2, value: 0 },
            BatchItemInput { id: 3, value: 100 },
        ];

        let result = BatchCalculator::process_batch(&env, items, true);
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert_eq!(error.code, Symbol::new(&env, "InvalidItemStrictAbort"));
        assert_eq!(error.index, 1);
    }

    #[test]
    fn test_non_strict_mode_captures_errors_and_continues() {
        let env = Env::default();
        let items = vec![
            &env,
            BatchItemInput { id: 1, value: 50 },
            BatchItemInput { id: 2, value: 0 },
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

    #[test]
    fn test_mixed_validity_failure_leaves_storage_unchanged() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        let valid = vec![
            &env,
            BatchItemInput { id: 1, value: 10 },
            BatchItemInput { id: 2, value: 20 },
        ];
        env.as_contract(&contract, || {
            BatchCalculator::process_batch_with_commit(&env, valid, true).unwrap();
        });

        let history_before = env.as_contract(&contract, || batch_process_history(&env));
        let stats_before = env.as_contract(&contract, || batch_process_stats(&env));

        let mixed = vec![
            &env,
            BatchItemInput { id: 3, value: 30 },
            BatchItemInput { id: 4, value: 0 },
            BatchItemInput { id: 5, value: 50 },
        ];
        let error = env.as_contract(&contract, || {
            BatchCalculator::process_batch_with_commit(&env, mixed, true)
                .err()
                .unwrap()
        });
        assert_eq!(error.index, 1);

        assert_eq!(
            env.as_contract(&contract, || batch_process_history(&env)),
            history_before
        );
        assert_eq!(
            env.as_contract(&contract, || batch_process_stats(&env)),
            stats_before
        );
    }
}
