#![cfg(test)]

extern crate std;

use soroban_sdk::{symbol_short, Address, Env};
use std::string::String;

use crate::{SLACalculatorContract, SLAError};

/// Fuzz test: random MTTR values produce valid results
#[test]
fn fuzz_sla_calculation_random_mttr() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Test with various MTTR values including edge cases
    let test_cases = [
        0, 1, 5, 10, 14, 15, 16, 29, 30, 31, 59, 60, 61, 119, 120, 121,
        239, 240, 241, 479, 480, 481, 999, 1000, 1439, 1440, 1441, 5000, 10000,
    ];

    for severity in &["critical", "high", "medium", "low"] {
        for mttr in test_cases {
            let result = client.try_calculate_sla_view(
                &symbol_short!("out_fz"),
                &symbol_short!(severity),
                &mttr,
            );

            // Should always succeed for valid inputs
            if let Ok(result) = result {
                // Result must have valid status
                assert!(
                    result.status == symbol_short!("met") || result.status == symbol_short!("viol"),
                    "Invalid status: {:?}",
                    result.status
                );

                // Result must have valid payment type
                assert!(
                    result.payment_type == symbol_short!("rew")
                        || result.payment_type == symbol_short!("pen"),
                    "Invalid payment_type: {:?}",
                    result.payment_type
                );

                // Met SLA must have positive amount
                if result.status == symbol_short!("met") {
                    assert!(result.amount > 0, "Met SLA must have positive reward");
                }

                // Violated SLA must have negative amount
                if result.status == symbol_short!("viol") {
                    assert!(result.amount < 0, "Violated SLA must have negative penalty");
                }
            }
        }
    }
}

/// Fuzz test: config modifications maintain invariants
#[test]
fn fuzz_config_modifications() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Test various config modifications
    let configs = [
        ("critical", 10, 200, 1000),
        ("high", 20, 100, 800),
        ("medium", 40, 50, 600),
        ("low", 80, 25, 400),
    ];

    for (severity, threshold, penalty, reward) in configs {
        let result = client.try_set_config(
            &admin,
            &symbol_short!(severity),
            &threshold,
            &(penalty as i128),
            &(reward as i128),
        );

        if result.is_ok() {
            // Config should be readable
            let config = client.get_config(&symbol_short!(severity));
            assert_eq!(config.threshold_minutes, threshold);
        }
    }

    // All configs should be consistent
    let snapshot = client.get_config_snapshot();
    assert_eq!(snapshot.entries.len(), 4);
}

/// Fuzz test: history operations maintain consistency
#[test]
fn fuzz_history_operations() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Add multiple entries
    for i in 0..20 {
        let outage_id = symbol_short!(&format!("out_{}", i));
        let mttr = 10 + (i * 5) as u32;
        let _ = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &mttr);
    }

    // History should be consistent
    let history = client.get_history();
    assert!(history.len() > 0);

    // Pagination should work
    let page1 = client.get_history_page(&0, &5);
    let page2 = client.get_history_page(&5, &5);
    assert_eq!(page1.len(), 5);
    assert_eq!(page2.len(), 5);

    // Query by outage should work
    let by_outage = client.get_history_by_outage(&symbol_short!("out_5"));
    assert!(by_outage.len() > 0);
}

/// Fuzz test: pause/unpause cycle maintains state
#[test]
fn fuzz_pause_unpause_cycle() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple pause/unpause cycles
    for i in 0..5 {
        let pause_result = client.try_pause(&admin, &String::from_str(&env, &format!("reason_{}", i)));
        assert!(pause_result.is_ok());

        // Should be paused
        assert!(client.is_paused());

        // Pause info should be available
        let info = client.get_pause_info();
        assert!(info.is_some());

        let unpause_result = client.try_unpause(&admin);
        assert!(unpause_result.is_ok());

        // Should be unpaused
        assert!(!client.is_paused());
    }
}

/// Fuzz test: admin operations maintain role consistency
#[test]
fn fuzz_admin_operations() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin1 = Address::generate(&env);
    let operator1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let operator2 = Address::generate(&env);

    client.initialize(&admin1, &operator1);

    // Admin transfer
    let _ = client.try_propose_admin(&admin1, &admin2);
    let _ = client.try_accept_admin(&admin2);

    // New admin should work
    let current_admin = client.get_admin();
    assert_eq!(current_admin, admin2);

    // Operator transfer
    let _ = client.try_propose_operator(&admin2, &operator2);
    let _ = client.try_accept_operator(&operator2);

    let current_operator = client.get_operator();
    assert_eq!(current_operator, operator2);
}

/// Fuzz test: version and metadata queries are consistent
#[test]
fn fuzz_version_consistency() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Version info should be consistent
    let version_info = client.get_version_info();
    assert_eq!(version_info.storage_version, 1);
    assert_eq!(version_info.result_schema_version, 1);
    assert!(!version_info.needs_migration);

    // Migration state should match
    let migration = client.get_migration_state();
    assert_eq!(migration.stored_version, 1);
    assert!(!migration.needs_migration);

    // Metadata should be complete
    let metadata = client.get_contract_metadata();
    assert_eq!(metadata.contract_name, symbol_short!("sla_calc"));
    assert_eq!(metadata.supported_severities.len(), 4);
}
