#![cfg(test)]

use super::\*;
use soroban*sdk::testutils::Address as *;
use soroban_sdk::{Env};
use soroban_sdk::testutils::Budget;

#[test]
fn test_admin_can_set_and_get_config() {
let env = Env::default();
let contract_id = env.register_contract(None, SLACalculatorContract);
let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let attacker = soroban_sdk::Address::generate(&env);

    client.initialize(&admin);


    client.set_config(
        &admin,
        &symbol_short!("critical"),
        &15,
        &100,
        &750,
    );

    let cfg = client.get_config(&symbol_short!("critical"));

    assert_eq!(cfg.threshold_minutes, 15);
    assert_eq!(cfg.penalty_per_minute, 100);
    assert_eq!(cfg.reward_base, 750);

}

#[test] #[should_panic]
fn test_non_admin_cannot_set_config() {
let env = Env::default();
let contract_id = env.register_contract(None, SLACalculatorContract);
let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    let attacker = soroban_sdk::Address::generate(&env);

    client.initialize(&admin);


    client.set_config(
        &attacker,
        &symbol_short!("critical"),
        &15,
        &100,
        &750,
    );

}

#[test]
fn test_defaults_exist_after_initialize() {
let env = Env::default();
let contract_id = env.register_contract(None, SLACalculatorContract);
let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin);

    let critical = client.get_config(&symbol_short!("critical"));
    assert_eq!(critical.threshold_minutes, 15);

    let high = client.get_config(&symbol_short!("high"));
    assert_eq!(high.threshold_minutes, 30);

    let medium = client.get_config(&symbol_short!("medium"));
    assert_eq!(medium.threshold_minutes, 60);

    let low = client.get_config(&symbol_short!("low"));
    assert_eq!(low.threshold_minutes, 120);

}

#[test]
fn test_calculate_sla_budget_is_reasonable() {
let env = Env::default();
env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin).unwrap();


    let before = env.budget().cpu_instruction_count();

    let _ = client.calculate_sla(
        &symbol_short!("OUT_BUDGET"),
        &symbol_short!("critical"),
        &25,
    ).unwrap();


    let after = env.budget().cpu_instruction_count();

    let delta = after - before;



    assert!(
        delta < 200_000,
        "SLA calculation is too expensive: {} instructions",
        delta
    );

}

#[test]
fn test_set_config_budget_is_reasonable() {
let env = Env::default();
env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin).unwrap();

    let before = env.budget().cpu_instruction_count();

    client.set_config(
        &admin,
        &symbol_short!("critical"),
        &15,
        &100,
        &750,
    ).unwrap();

    let after = env.budget().cpu_instruction_count();

    let delta = after - before;

    assert!(
        delta < 150_000,
        "set_config is too expensive: {} instructions",
        delta
    );

}

// SC-079: read-only history/retention helpers

#[test]
fn test_get_config_count_returns_default_tier_count() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin);

    // initialize sets 4 default tiers: critical, high, medium, low
    let count = client.get_config_count();
    assert_eq!(count, 4);
}

#[test]
fn test_get_storage_version_returns_current_version() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin);

    let version = client.get_storage_version();
    assert_eq!(version, 1);
}

// SC-080: performance coverage for read-heavy helpers

#[test]
fn test_get_config_count_budget_is_reasonable() {
    let env = Env::default();
    env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin).unwrap();

    let before = env.budget().cpu_instruction_count();
    let _ = client.get_config_count();
    let after = env.budget().cpu_instruction_count();

    assert!(
        after - before < 100_000,
        "get_config_count is too expensive: {} instructions",
        after - before
    );
}

#[test]
fn test_get_config_budget_is_reasonable() {
    let env = Env::default();
    env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin).unwrap();

    let before = env.budget().cpu_instruction_count();
    let _ = client.get_config(&symbol_short!("critical"));
    let after = env.budget().cpu_instruction_count();

    assert!(
        after - before < 100_000,
        "get_config read is too expensive: {} instructions",
        after - before
    );
}

#[test]
fn test_list_configs_budget_is_reasonable() {
    let env = Env::default();
    env.budget().reset_unlimited();

    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContractClient::new(&env, &contract_id);

    let admin = soroban_sdk::Address::generate(&env);
    client.initialize(&admin).unwrap();

    let before = env.budget().cpu_instruction_count();
    let _ = client.list_configs();
    let after = env.budget().cpu_instruction_count();

    assert!(
        after - before < 150_000,
        "list_configs is too expensive: {} instructions",
        after - before
    );
}
