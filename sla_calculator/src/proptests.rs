#![cfg(test)]

extern crate std;

use proptest::prelude::*;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};
use std::string::String;

use crate::{SLACalculatorContract, SLAError, SLAConfig, batch::{BatchRequest, BatchSummary, BatchResult}};

mod utils;

/// Proptest strategy to generate valid Symbol values for outage IDs
fn arb_outage_id() -> impl Strategy<Value = Symbol> {
    // Generate valid symbol strings (alphanumeric, <=32 chars)
    "[a-zA-Z0-9_]{1,32}".prop_map(|s| Symbol::from_string(s))
}

/// Proptest strategy to generate valid severity symbols
fn arb_severity() -> impl Strategy<Value = Symbol> {
    prop_oneof![
        Just(symbol_short!("critical")),
        Just(symbol_short!("high")),
        Just(symbol_short!("medium")),
        Just(symbol_short!("low")),
    ]
}

/// Proptest strategy to generate a single valid BatchRequest
fn arb_batch_request() -> impl Strategy<Value = BatchRequest> {
    (arb_outage_id(), arb_severity(), 1u32..10000) // MTTR from 1 to 9999 minutes
        .prop_map(|(outage_id, severity, mttr_minutes)| BatchRequest {
            outage_id,
            severity,
            mttr_minutes,
        })
}

/// Proptest strategy to generate a vector of BatchRequests with unique outage IDs
fn arb_batch_requests() -> impl Strategy<Value = std::vec::Vec<BatchRequest>> {
    proptest::collection::vec(arb_batch_request(), 1..50) // Batch size 1 to 50 (max batch limit)
        .prop_filter("All outage IDs must be unique", |requests| {
            let mut seen = std::collections::HashSet::new();
            requests.iter().all(|req| seen.insert(req.outage_id))
        })
}

/// Property: SLA met result always has positive amount and "rew" payment type
#[test]
fn prop_sla_met_implies_positive_reward() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // For any valid config and MTTR <= threshold, result should be "met" with positive reward
    for severity in &["critical", "high", "medium", "low"] {
        let config = client.get_config(&symbol_short!(severity));

        // Test MTTR = 0 (best case)
        let result = client.calculate_sla_view(
            &symbol_short!("out_0"),
            &symbol_short!(severity),
            &0,
        );
        assert_eq!(result.status, symbol_short!("met"));
        assert_eq!(result.payment_type, symbol_short!("rew"));
        assert!(result.amount > 0, "Reward must be positive when SLA met");

        // Test MTTR = threshold (boundary)
        let result = client.calculate_sla_view(
            &symbol_short!("out_1"),
            &symbol_short!(severity),
            &config.threshold_minutes,
        );
        assert_eq!(result.status, symbol_short!("met"));
        assert!(result.amount > 0);
    }
}

/// Property: SLA violated result always has negative amount and "pen" payment type
#[test]
fn prop_sla_violated_implies_negative_penalty() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    for severity in &["critical", "high", "medium", "low"] {
        let config = client.get_config(&symbol_short!(severity));

        // Test MTTR = threshold + 1 (just over boundary)
        let result = client.calculate_sla_view(
            &symbol_short!("out_v1"),
            &symbol_short!(severity),
            &(config.threshold_minutes + 1),
        );
        assert_eq!(result.status, symbol_short!("viol"));
        assert_eq!(result.payment_type, symbol_short!("pen"));
        assert!(result.amount < 0, "Penalty must be negative when SLA violated");

        // Test MTTR = threshold + 100 (large violation)
        let result = client.calculate_sla_view(
            &symbol_short!("out_v2"),
            &symbol_short!(severity),
            &(config.threshold_minutes + 100),
        );
        assert_eq!(result.status, symbol_short!("viol"));
        assert!(result.amount < 0);
    }
}

/// Property: SLA result always binds to the config version hash
#[test]
fn prop_config_version_hash_is_deterministic() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    // Hash should be stable across repeated reads
    let hash1 = client.get_config_version_hash();
    let hash2 = client.get_config_version_hash();
    let hash3 = client.get_config_version_hash();
    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);

    // Same inputs should produce same result
    let r1 = client.calculate_sla_view(&symbol_short!("out_d1"), &symbol_short!("high"), &10);
    let r2 = client.calculate_sla_view(&symbol_short!("out_d2"), &symbol_short!("high"), &10);
    assert_eq!(r1.config_version_hash, r2.config_version_hash);
}

/// Property: monotonic penalty increase with overtime
#[test]
fn prop_penalty_monotonic_with_overtime() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let config = client.get_config(&symbol_short!("high"));
    let threshold = config.threshold_minutes;

    let mut prev_penalty: i128 = 0;
    for overtime in 1..=10 {
        let mttr = threshold + overtime;
        let result = client.calculate_sla_view(
            &symbol_short!("out_mono"),
            &symbol_short!("high"),
            &mttr,
        );
        assert_eq!(result.status, symbol_short!("viol"));
        // Penalty should be strictly increasing with overtime
        assert!(
            result.amount < prev_penalty,
            "Penalty should increase with overtime: {} not < {}",
            result.amount,
            prev_penalty
        );
        prev_penalty = result.amount;
    }
}

/// Property: reward decreases as performance ratio approaches threshold
#[test]
fn prop_reward_decreases_near_threshold() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let config = client.get_config(&symbol_short!("high"));
    let threshold = config.threshold_minutes;

    let mut prev_reward: i128 = i128::MAX;
    for ratio_pct in [10, 25, 49, 50, 74, 75, 99, 100] {
        let mttr = (threshold * ratio_pct) / 100;
        let result = client.calculate_sla_view(
            &symbol_short!("out_rew"),
            &symbol_short!("high"),
            &mttr,
        );
        assert_eq!(result.status, symbol_short!("met"));
        // Reward should be non-increasing as ratio approaches threshold
        assert!(
            result.amount <= prev_reward,
            "Reward should decrease near threshold: {} not <= {}",
            result.amount,
            prev_reward
        );
        prev_reward = result.amount;
    }
}

/// Property: stats always reflect cumulative operations
#[test]
fn prop_stats_monotonically_increase() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let stats0 = client.get_stats();
    assert_eq!(stats0.total_calculations, 0);

    // After first calculation
    let _ = client.calculate_sla(&operator, &symbol_short!("out_s1"), &symbol_short!("high"), &10);
    let stats1 = client.get_stats();
    assert_eq!(stats1.total_calculations, 1);

    // After second calculation
    let _ = client.calculate_sla(&operator, &symbol_short!("out_s2"), &symbol_short!("high"), &10);
    let stats2 = client.get_stats();
    assert_eq!(stats2.total_calculations, 2);
}

/// Property: invalid severity always rejected
#[test]
fn prop_invalid_severity_always_rejected() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let invalid_severities = [
        symbol_short!("invalid"),
        symbol_short!("urgent"),
        symbol_short!("normal"),
    ];

    for severity in invalid_severities {
        let result = client.try_get_config(&severity);
        assert!(result.is_err(), "Should reject invalid severity");
    }
}

/// Property: zero MTTR always results in top rating
#[test]
fn prop_zero_mttr_always_top_rating() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    for severity in &["critical", "high", "medium", "low"] {
        let result = client.calculate_sla_view(
            &symbol_short!("out_zero"),
            &symbol_short!(severity),
            &0,
        );
        assert_eq!(result.rating, symbol_short!("top"));
    }
}

/// Property: contract cannot be re-initialized
#[test]
fn prop_double_init_rejected() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin1 = Address::generate(&env);
    let operator1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let operator2 = Address::generate(&env);

    client.initialize(&admin1, &operator1);
    let result = client.try_initialize(&admin2, &operator2);
    assert!(result.is_err());
}

/// Property: config snapshot returns all four severity tiers
#[test]
fn prop_snapshot_always_has_four_entries() {
    let env = Env::default();
    let contract_id = env.register_contract(None, SLACalculatorContract);
    let client = SLACalculatorContract::new_client(&env, &contract_id);

    let admin = Address::generate(&env);
    let operator = Address::generate(&env);
    client.initialize(&admin, &operator);

    let snapshot = client.get_config_snapshot();
    assert_eq!(snapshot.entries.len(), 4);

    // Verify canonical order
    let severities: std::vec::Vec<_> = snapshot.entries.iter().map(|e| e.severity).collect();
    assert_eq!(severities[0], symbol_short!("critical"));
    assert_eq!(severities[1], symbol_short!("high"));
    assert_eq!(severities[2], symbol_short!("medium"));
    assert_eq!(severities[3], symbol_short!("low"));
}