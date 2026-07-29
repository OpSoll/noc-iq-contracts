#[cfg(test)]
mod outage_id_tests {
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol};
    use crate::{SLACalculatorContract, SLACalculatorContractClient, SLAError};

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
    fn test_repeated_outage_id_each_recorded() {
        let env = Env::default();
        let (_admin, operator, client) = setup(&env);
        let outage_id = symbol_short!("INC01");

        client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &10);
        client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &10);

        let stats = client.get_stats();
        assert_eq!(stats.total_calculations, 2);
    }

    #[test]
    fn test_repeated_outage_id_results_are_consistent() {
        let env = Env::default();
        let (_admin, operator, client) = setup(&env);
        let outage_id = symbol_short!("INC02");

        let r1 = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &10);
        let r2 = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &10);

        assert_eq!(r1.status, r2.status);
        assert_eq!(r1.amount, r2.amount);
    }

    #[test]
    fn test_different_outage_ids_tracked_independently() {
        let env = Env::default();
        let (_admin, operator, client) = setup(&env);

        client.calculate_sla(&operator, &symbol_short!("A"), &symbol_short!("high"), &5);
        client.calculate_sla(&operator, &symbol_short!("B"), &symbol_short!("high"), &50);

        let stats = client.get_stats();
        assert_eq!(stats.total_calculations, 2);
    }

    #[test]
    #[should_panic(expected = "InvalidOutageId")]
    fn test_empty_outage_id_rejected() {
        let env = Env::default();
        let (_admin, operator, client) = setup(&env);
        // Create an empty symbol which should be rejected
        let empty_outage_id = Symbol::new(&env, "");
        client.calculate_sla(&operator, &empty_outage_id, &symbol_short!("high"), &10);
    }

    #[test]
    #[should_panic(expected = "InvalidOutageId")]
    fn test_outage_id_exceeding_32_chars_rejected() {
        let env = Env::default();
        let (_admin, operator, client) = setup(&env);
        // Create a string longer than 32 characters
        let long_outage_id = Symbol::new(&env, "this_string_is_exactly_thirty_three_chars");
        client.calculate_sla(&operator, &long_outage_id, &symbol_short!("high"), &10);
    }

    #[test]
    #[should_panic(expected = "InvalidOutageId")]
    fn test_outage_id_with_invalid_characters_rejected() {
        let env = Env::default();
        let (_admin, operator, client) = setup(&env);
        // Symbol with invalid characters (space and special chars)
        let invalid_outage_id = Symbol::new(&env, "OUTAGE 123!@#");
        client.calculate_sla(&operator, &invalid_outage_id, &symbol_short!("high"), &10);
    }

    #[test]
    #[should_panic(expected = "InvalidOutageId")]
    fn test_calculate_sla_view_rejects_invalid_outage_id() {
        let env = Env::default();
        let (_admin, _operator, client) = setup(&env);
        // Create an invalid outage ID to test the view function
        let invalid_outage_id = Symbol::new(&env, "OUTAGE!@#");
        client.calculate_sla_view(&invalid_outage_id, &symbol_short!("high"), &10);
    }

    #[test]
    fn test_valid_outage_ids_accepted() {
        let env = Env::default();
        let (_admin, operator, client) = setup(&env);
        
        // Test with exactly 32 characters
        let max_length_id = Symbol::new(&env, "abcdefghijklmnopqrstuvwxyz012345");
        let result = client.try_calculate_sla(&operator, &max_length_id, &symbol_short!("high"), &10);
        assert!(result.is_ok());

        // Test with underscores and numbers
        let valid_id = Symbol::new(&env, "OUTAGE_123_TEST");
        let result = client.try_calculate_sla(&operator, &valid_id, &symbol_short!("high"), &10);
        assert!(result.is_ok());

        // Test view function with valid ID
        let view_result = client.try_calculate_sla_view(&valid_id, &symbol_short!("high"), &10);
        assert!(view_result.is_ok());
    }
}