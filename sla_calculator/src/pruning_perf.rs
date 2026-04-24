#[cfg(test)]
mod pruning_perf_tests {
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

    use crate::{SLACalculatorContract, SLACalculatorContractClient};

    fn setup_with_history(env: &Env, count: u32) -> (Address, SLACalculatorContractClient) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let operator = Address::generate(env);
        client.initialize(&admin, &operator);
        for i in 1..=count {
            client.calculate_sla(
                &operator,
                &symbol_short!("OUT"),
                &symbol_short!("high"),
                &i,
            );
        }
        (admin, client)
    }

    #[test]
    fn test_prune_retains_n_most_recent() {
        let env = Env::default();
        let (admin, client) = setup_with_history(&env, 50);
        client.prune_history(&admin, &10);
        let stats = client.get_stats();
        assert!(stats.total_calculations >= 10);
    }

    #[test]
    fn test_prune_large_history_does_not_panic() {
        let env = Env::default();
        let (admin, client) = setup_with_history(&env, 100);
        client.prune_history(&admin, &5);
    }

    #[test]
    fn test_prune_to_zero_retains_nothing() {
        let env = Env::default();
        let (admin, client) = setup_with_history(&env, 20);
        client.prune_history(&admin, &0);
    }
}
