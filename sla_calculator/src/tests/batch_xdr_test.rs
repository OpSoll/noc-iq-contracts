#[cfg(test)]
mod batch_xdr_tests {
    use super::*;
    use soroban_sdk::{Env, Symbol};

    #[test]
    fn test_error_vector_xdr_mapping() {
        let env = Env::default();

        let none_vec = BatchResultAdapter::create_error_vector(&env, None);
        assert_eq!(none_vec.len(), 0);

        let err_sym = Symbol::new(&env, "BatchError");
        let some_vec = BatchResultAdapter::create_error_vector(&env, Some(err_sym.clone()));
        assert_eq!(some_vec.len(), 1);
        assert_eq!(some_vec.get(0).unwrap(), err_sym);
    }

    #[test]
    fn test_xdr_budget_accepts_exact_limit_and_rejects_limit_plus_one() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        let mut items = Vec::new(&env);
        for i in 0..XDR_BATCH_ITEM_LIMIT {
            items.push_back(BatchItemInput {
                id: i as u64,
                value: 1,
            });
        }

        let accepted = env.as_contract(&contract, || {
            BatchCalculator::process_batch_with_commit(&env, items.clone(), true)
        });
        assert!(accepted.is_ok());
        assert_eq!(accepted.unwrap().processed_count, XDR_BATCH_ITEM_LIMIT);

        items.push_back(BatchItemInput {
            id: XDR_BATCH_ITEM_LIMIT as u64,
            value: 1,
        });
        let rejected = env.as_contract(&contract, || {
            BatchCalculator::process_batch_with_commit(&env, items.clone(), true)
        });
        assert!(rejected.is_err());
        let error = rejected.err().unwrap();
        assert_eq!(error.code, Symbol::new(&env, "BatchXdrBudgetExceeded"));
        assert_eq!(error.index, XDR_BATCH_ITEM_LIMIT);

        let history = env.as_contract(&contract, || batch_process_history(&env));
        assert_eq!(history.len(), XDR_BATCH_ITEM_LIMIT);
        let stats = env.as_contract(&contract, || batch_process_stats(&env));
        assert_eq!(stats.processed, XDR_BATCH_ITEM_LIMIT);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_validate_xdr_budget_rejects_over_limit_predictably() {
        let env = Env::default();
        let mut requests = Vec::new(&env);
        for i in 0..=XDR_BATCH_ITEM_LIMIT {
            requests.push_back(BatchRequest {
                outage_id: Symbol::new(&env, "XDRREQ"),
                severity: symbol_short!("high"),
                mttr_minutes: i + 1,
            });
        }
        assert_eq!(
            validate_xdr_budget(&env, &requests),
            Err(SLAError::ThresholdOutOfBounds)
        );

        requests.pop_back();
        assert_eq!(validate_xdr_budget(&env, &requests), Ok(XDR_BATCH_ITEM_LIMIT));
    }
}
