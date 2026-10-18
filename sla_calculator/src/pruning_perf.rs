#[cfg(test)]
mod pruning_perf_tests {
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

    use crate::{SLACalculatorContract, SLACalculatorContractClient};

    fn setup_with_history(
        env: &Env,
        count: u32,
    ) -> (Address, Address, SLACalculatorContractClient) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let operator = Address::generate(env);
        client.initialize(&admin, &operator);
        for i in 1..=count {
            client.calculate_sla(&operator, &symbol_short!("OUT"), &symbol_short!("high"), &i);
        }
        (admin, operator, client)
    }

    #[test]
    fn test_prune_retains_n_most_recent() {
        let env = Env::default();
        let (admin, _operator, client) = setup_with_history(&env, 50);
        client.prune_history(&admin, &10);
        let stats = client.get_stats();
        assert!(stats.total_calculations >= 10);
    }

    #[test]
    fn test_prune_large_history_does_not_panic() {
        let env = Env::default();
        let (admin, _operator, client) = setup_with_history(&env, 100);
        client.prune_history(&admin, &5);
    }

    #[test]
    fn test_prune_to_zero_retains_nothing() {
        let env = Env::default();
        let (admin, _operator, client) = setup_with_history(&env, 20);
        client.prune_history(&admin, &0);
    }

    #[test]
    fn test_pruning_mixed_chronology_sequence_is_stable() {
        let env = Env::default();
        let (admin, operator, client) = setup_with_history(&env, 30);
        client.prune_history(&admin, &12);
        client.calculate_sla(&operator, &symbol_short!("OUT"), &symbol_short!("high"), &5);
        client.prune_history(&admin, &10);
    }

    #[test]
    fn test_calculate_sla_cost_regression_baseline_smoke() {
        let env = Env::default();
        let (_admin, operator, client) = setup_with_history(&env, 1);
        for _ in 0..20 {
            client.calculate_sla(
                &operator,
                &symbol_short!("OUT"),
                &symbol_short!("high"),
                &10,
            );
        }
    }

    /// CPU-instruction cost of a single `prune_history(keep_latest)` call on
    /// a history of `entries` records. Only the prune itself is measured: the
    /// history is built before the budget snapshot is taken.
    fn prune_cost(entries: u32, keep_latest: u32) -> u64 {
        let env = Env::default();
        let (admin, _operator, client) = setup_with_history(&env, entries);
        let before = env.budget().cpu_instruction_cost();
        client.prune_history(&admin, &keep_latest);
        env.budget().cpu_instruction_cost() - before
    }

    /// Pruning budget: growing the history tenfold (10 -> 100 entries) may
    /// cost at most ten times as much to prune. A linear prune sits well
    /// under this (fixed overhead like the version and admin checks is paid
    /// once either way); an O(n^2) prune would cost ~100x and trip the
    /// assertion. Being relative, the budget self-calibrates instead of
    /// depending on absolute host instruction numbers.
    const PRUNE_COST_GROWTH_LIMIT: u64 = 10;

    /// #906: pruning cost must stay within budget - the benchmark fails when
    /// prune cost grows faster than linearly in history size.
    #[test]
    fn test_prune_cost_stays_within_budget() {
        let small_cost = prune_cost(10, 5);
        let large_cost = prune_cost(100, 5);

        assert!(small_cost > 0, "the budget tracker must have measured cost");
        assert!(
            large_cost <= small_cost.saturating_mul(PRUNE_COST_GROWTH_LIMIT),
            "pruning 100 entries cost {large_cost} CPU instructions vs {small_cost} \
             for 10 entries: prune cost grew faster than linearly and exceeded the \
             budget of {PRUNE_COST_GROWTH_LIMIT}x"
        );
    }
}
