#![cfg(test)]

extern crate std;

use soroban_sdk::{symbol_short, Address, Env};
use std::string::String;

use crate::{SLACalculatorContract, SLAConfig};

/// Benchmark: Contract initialization overhead
#[test]
fn bench_initialize() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);

    let _ = client.initialize(&admin, &operator);
}

/// Benchmark: Config read performance
#[test]
fn bench_get_config() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let _ = client.get_config(&symbol_short!("critical"));
    let _ = client.get_config(&symbol_short!("high"));
    let _ = client.get_config(&symbol_short!("medium"));
    let _ = client.get_config(&symbol_short!("low"));
}

/// Benchmark: SLA calculation (view-only) performance
#[test]
fn bench_calculate_sla_view() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Benchmark: SLA met case
    let _ = client.calculate_sla_view(&symbol_short!("out_b1"), &symbol_short!("high"), &10);

    // Benchmark: SLA violated case
    let _ = client.calculate_sla_view(&symbol_short!("out_b2"), &symbol_short!("high"), &60);
}

/// Benchmark: Full SLA calculation with persistence (operator)
#[test]
fn bench_calculate_sla_full() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple calculations to measure average
    for i in 0..10 {
        let outage_id = symbol_short!(&format!("out_{}", i));
        let _ = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &(10 + i * 5));
    }
}

/// Benchmark: History pagination performance
#[test]
fn bench_history_pagination() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Populate history
    for i in 0..100 {
        let outage_id = symbol_short!(&format!("out_{}", i));
        let _ = client.calculate_sla(&operator, &outage_id, &symbol_short!("high"), &(10 + i));
    }

    // Benchmark pagination
    let _ = client.get_history_page(&0, &10);
    let _ = client.get_history_page(&50, &10);
    let _ = client.get_history_page(&90, &10);
}

/// Benchmark: Config snapshot generation
#[test]
fn bench_config_snapshot() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple snapshot reads
    for _ in 0..10 {
        let _ = client.get_config_snapshot();
    }
}

/// Benchmark: Version hash computation
#[test]
fn bench_config_version_hash() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple hash computations
    for _ in 0..10 {
        let _ = client.get_config_version_hash();
    }
}

/// Benchmark: Stats query performance
#[test]
fn bench_get_stats() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple stats queries
    for _ in 0..10 {
        let _ = client.get_stats();
    }
}

/// Benchmark: Pause/unpause cycle
#[test]
fn bench_pause_unpause() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple pause/unpause cycles
    for _ in 0..10 {
        let _ = client.pause(&admin, &String::from_str(&env, "benchmark"));
        let _ = client.unpause(&admin);
    }
}

/// Benchmark: History by outage query
#[test]
fn bench_history_by_outage() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Create multiple calculations for same outage
    for i in 0..10 {
        let _ = client.calculate_sla(&operator, &symbol_short!("out_rep"), &symbol_short!("high"), &(10 + i));
    }

    // Query by outage
    let _ = client.get_history_by_outage(&symbol_short!("out_rep"));
    let _ = client.get_latest_by_outage(&symbol_short!("out_rep"));
}

/// Benchmark: Full contract metadata retrieval
#[test]
fn bench_get_contract_metadata() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple metadata queries
    for _ in 0..10 {
        let _ = client.get_contract_metadata();
        let _ = client.get_result_schema();
        let _ = client.get_failure_schema();
    }
}

/// Benchmark: Version negotiation endpoint
#[test]
fn bench_get_version_info() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Multiple version queries
    for _ in 0..10 {
        let _ = client.get_version_info();
        let _ = client.get_migration_state();
    }
}
