#![cfg(test)]

extern crate std;

use soroban_sdk::{symbol_short, Address, Env};
use std::string::String;

use crate::{SLACalculatorContract, SLAConfig};

/// Gas budget profiling: measure relative cost of operations.
///
/// These tests measure operation counts to estimate relative gas costs.
/// Soroban charges based on instructions and memory; these profiles
/// help identify expensive operations for optimization.

/// Profile: Single SLA calculation cost
#[test]
fn profile_single_sla_calculation() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Profile: calculate_sla_view (no persistence)
    let _ = client.calculate_sla_view(&symbol_short!("out_p1"), &symbol_short!("high"), &10);

    // Profile: calculate_sla (with persistence)
    let _ = client.calculate_sla(&operator, &symbol_short!("out_p2"), &symbol_short!("high"), &10);
}

/// Profile: Batch calculation cost scaling
#[test]
fn profile_batch_calculation_scaling() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Profile: Increasing batch sizes
    for batch_size in [1, 5, 10, 20, 50] {
        for i in 0..batch_size {
            let outage_id = symbol_short!(&format!("b_{}_{}", batch_size, i));
            let _ = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &(10 + i as u32));
        }
    }
}

/// Profile: History query cost scaling
#[test]
fn profile_history_query_scaling() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Populate history
    for i in 0..100 {
        let outage_id = symbol_short!(&format!("h_{}", i));
        let _ = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &(10 + i));
    }

    // Profile: Full history retrieval
    let _ = client.get_history();

    // Profile: Paginated queries at different offsets
    for offset in [0, 25, 50, 75, 90] {
        let _ = client.get_history_page(&offset, &10);
    }

    // Profile: Query by outage
    let _ = client.get_history_by_outage(&symbol_short!("h_50"));
    let _ = client.get_latest_by_outage(&symbol_short!("h_50"));
}

/// Profile: Config snapshot generation
#[test]
fn profile_config_snapshot_cost() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Profile: Config snapshot generation
    for _ in 0..10 {
        let _ = client.get_config_snapshot();
    }

    // Profile: Config version hash computation
    for _ in 0..10 {
        let _ = client.get_config_version_hash();
    }
}

/// Profile: Governance operations
#[test]
fn profile_governance_operations() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let new_operator = Address::generate(&env);

    client.initialize(&admin, &operator);

    // Profile: Admin transfer cycle
    let _ = client.try_propose_admin(&admin, &new_admin);
    let _ = client.try_accept_admin(&new_admin);

    // Profile: Operator transfer cycle
    let _ = client.try_propose_operator(&new_admin, &new_operator);
    let _ = client.try_accept_operator(&new_operator);

    // Profile: Pause/unpause cycle
    let _ = client.try_pause(&new_admin, &String::from_str(&env, "test"));
    let _ = client.try_unpause(&new_admin);
}

/// Profile: Metadata and introspection queries
#[test]
fn profile_metadata_queries() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Profile: Multiple metadata queries
    for _ in 0..10 {
        let _ = client.get_contract_metadata();
        let _ = client.get_result_schema();
        let _ = client.get_failure_schema();
        let _ = client.get_version_info();
        let _ = client.get_migration_state();
        let _ = client.get_storage_version();
        let _ = client.get_config_count();
        let _ = client.get_retention_limit();
    }
}

/// Profile: Config update cost
#[test]
fn profile_config_update_cost() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Profile: Config updates
    let configs = [
        ("critical", 10, 200, 1000),
        ("high", 20, 100, 800),
        ("medium", 40, 50, 600),
        ("low", 80, 25, 400),
    ];

    for (severity, threshold, penalty, reward) in configs {
        let _ = client.try_set_config(
            &admin,
            &symbol_short!(severity),
            &threshold,
            &(penalty as i128),
            &(reward as i128),
        );
    }
}

/// Profile: History pruning cost
#[test]
fn profile_history_pruning_cost() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Populate history
    for i in 0..50 {
        let outage_id = symbol_short!(&format!("pr_{}", i));
        let _ = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &(10 + i));
    }

    // Profile: Prune to keep 25
    let _ = client.try_prune_history(&admin, &25);
}
