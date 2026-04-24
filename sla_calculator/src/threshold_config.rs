#[cfg(test)]
mod threshold_tests {
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

    use crate::{SLACalculatorContract, SLACalculatorContractClient, SLAConfig};

    fn setup(env: &Env) -> (Address, Address, SLACalculatorContractClient) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let operator = Address::generate(env);
        client.initialize(&admin, &operator);
        (admin, operator, client)
    }

    #[test]
    fn test_zero_threshold_always_violated() {
        let env = Env::default();
        let (admin, operator, client) = setup(&env);
        client.set_config(
            &admin,
            &symbol_short!("low"),
            &SLAConfig {
                threshold_minutes: 0,
                penalty_per_minute: 10,
                reward_base: 100,
            },
        );
        let result = client.calculate_sla(
            &operator,
            &symbol_short!("OUT1"),
            &symbol_short!("low"),
            &1,
        );
        assert_eq!(result.status, symbol_short!("viol"));
    }

    #[test]
    fn test_near_zero_threshold_one_minute() {
        let env = Env::default();
        let (admin, operator, client) = setup(&env);
        client.set_config(
            &admin,
            &symbol_short!("low"),
            &SLAConfig {
                threshold_minutes: 1,
                penalty_per_minute: 5,
                reward_base: 50,
            },
        );
        let met = client.calculate_sla(
            &operator,
            &symbol_short!("OUT2"),
            &symbol_short!("low"),
            &1,
        );
        assert_eq!(met.status, symbol_short!("met"));

        let viol = client.calculate_sla(
            &operator,
            &symbol_short!("OUT3"),
            &symbol_short!("low"),
            &2,
        );
        assert_eq!(viol.status, symbol_short!("viol"));
    }
}
