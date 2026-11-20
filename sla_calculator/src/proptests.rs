#![cfg(test)]

extern crate std;

use proptest::prelude::*;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};
use std::string::String;

use crate::{SLACalculatorContract, SLAError, SLAConfig, state_integrity::ConfigState, batch::{BatchRequest, BatchSummary, BatchResult}};
use crate::batch::{optimized_batch_effects, process_reference_batch, process_reference_batch_with_effects};
use crate::batch_optimized::{OptimizedBatchCalculator, OptimizedBatchItem};

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
        let config = client.get_config(&Symbol::new(&env, severity));

        // Test MTTR = 0 (best case)
        let result = client.calculate_sla_view(
            &symbol_short!("out_0"),
            &Symbol::new(&env, severity),
            &0,
        );
        assert_eq!(result.status, symbol_short!("met"));
        assert_eq!(result.payment_type, symbol_short!("rew"));
        assert!(result.amount >= 0, "Reward must be non-negative when SLA met");

        // Test MTTR = threshold (boundary)
        let result = client.calculate_sla_view(
            &symbol_short!("out_1"),
            &Symbol::new(&env, severity),
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
        let config = client.get_config(&Symbol::new(&env, severity));

        // Test MTTR = threshold + 1 (just over boundary)
        let result = client.calculate_sla_view(
            &symbol_short!("out_v1"),
            &Symbol::new(&env, severity),
            &(config.threshold_minutes + 1),
        );
        assert_eq!(result.status, symbol_short!("viol"));
        assert_eq!(result.payment_type, symbol_short!("pen"));
        assert!(result.amount < 0, "Penalty must be negative when SLA violated");

        // Test MTTR = threshold + 100 (large violation)
        let result = client.calculate_sla_view(
            &symbol_short!("out_v2"),
            &Symbol::new(&env, severity),
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
            &Symbol::new(&env, severity),
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

/// Property: For all valid batch requests, succeeded + failed == total
#[test]
fn prop_batch_succeeded_plus_failed_equals_total() {
    proptest!(|(requests in arb_batch_requests())| {
        let env = Env::default();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContract::new_client(&env, &contract_id);

        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        client.initialize(&admin, &operator);

        // Convert std::vec::Vec to soroban_sdk::Vec
        let mut soroban_requests = Vec::new(&env);
        for req in requests {
            soroban_requests.push_back(req);
        }

        // Call view version (no state mutation)
        let (summary, _results) = client.batch_calculate_view(&soroban_requests).unwrap();
        
        // Verify the invariant
        assert_eq!(summary.succeeded + summary.failed, summary.total);
    });
}

/// Property: batch_calculate_view never mutates contract state, even when processing batches with errors
#[test]
fn prop_batch_view_never_mutates_state() {
    proptest!(|(requests in arb_batch_requests())| {
        let env = Env::default();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContract::new_client(&env, &contract_id);

        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        client.initialize(&admin, &operator);

        // Get initial state
        let initial_stats = client.get_stats();
        let initial_history_len = client.get_history().len();
        let initial_admin = client.get_admin();
        let initial_operator = client.get_operator();

        // Convert std::vec::Vec to soroban_sdk::Vec
        let mut soroban_requests = Vec::new(&env);
        for req in requests {
            soroban_requests.push_back(req);
        }

        // Call view version - should not change anything
        let _ = client.batch_calculate_view(&soroban_requests);

        // Verify all state is unchanged
        let final_stats = client.get_stats();
        let final_history_len = client.get_history().len();
        let final_admin = client.get_admin();
        let final_operator = client.get_operator();

        assert_eq!(initial_stats, final_stats, "Stats must not change after view call");
        assert_eq!(initial_history_len, final_history_len, "History length must not change after view call");
        assert_eq!(initial_admin, final_admin, "Admin must not change after view call");
        assert_eq!(initial_operator, final_operator, "Operator must not change after view call");
    });
}

// -----------------------------------------------------------------------
// #1023 — config mutation sequences must preserve cross-field invariants
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigOp {
    /// Governance sets a new configured threshold.
    Update(u32),
    /// Roll back the last configured threshold.
    Rollback(u32),
    /// Freeze or unfreeze configuration changes.
    Freeze(bool),
    /// Apply a configuration revision (clears one pending revision).
    ApplyRevision,
    /// Read-only observation; must never change state.
    Read,
}

fn arb_config_op() -> impl Strategy<Value = ConfigOp> {
    prop_oneof![
        (1u32..=1440).prop_map(ConfigOp::Update),
        (1u32..=1440).prop_map(ConfigOp::Rollback),
        any::<bool>().prop_map(ConfigOp::Freeze),
        Just(ConfigOp::ApplyRevision),
        Just(ConfigOp::Read),
    ]
}

/// Apply one operation to a state, modelling the guard rules as a state
/// machine. Invalid operations are rejected (state unchanged) so that no
/// generated sequence can ever violate the cross-field invariants.
fn apply_op(state: ConfigState, op: ConfigOp) -> ConfigState {
    match op {
        ConfigOp::Update(value) if !state.frozen => ConfigState {
            configured_minutes: value,
            effective_minutes: crate::threshold_bounds::resolve_effective_threshold(value)
                .map(|r| r.effective_minutes)
                .unwrap_or(state.effective_minutes),
            pending_revisions: state.pending_revisions.saturating_add(1),
            ..state
        },
        ConfigOp::Rollback(value) if !state.frozen && state.pending_revisions > 0 => ConfigState {
            configured_minutes: value,
            effective_minutes: crate::threshold_bounds::resolve_effective_threshold(value)
                .map(|r| r.effective_minutes)
                .unwrap_or(state.effective_minutes),
            pending_revisions: state.pending_revisions.saturating_sub(1),
            ..state
        },
        ConfigOp::Freeze(frozen) if !frozen || state.pending_revisions == 0 => ConfigState {
            frozen,
            ..state
        },
        ConfigOp::ApplyRevision if state.pending_revisions > 0 => ConfigState {
            pending_revisions: state.pending_revisions.saturating_sub(1),
            ..state
        },
        // Reads and rejected operations leave state untouched.
        _ => state,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn prop_config_mutation_sequences_preserve_invariants(
        ops in proptest::collection::vec(arb_config_op(), 1..40)
    ) {
        let mut state = ConfigState {
            configured_minutes: 60,
            effective_minutes: 60,
            frozen: false,
            pending_revisions: 0,
        };
        for (index, op) in ops.iter().enumerate() {
            state = apply_op(state, *op);
            if let Err(violation) = crate::state_integrity::check_config_invariants(state) {
                // Proptest shrinks to a minimal sequence; naming the prefix
                // and the operation makes the reproduction self-contained.
                return Err(TestCaseError::fail(std::format!(
                    "invariant {violation:?} after operation #{index} ({op:?}); prefix: {:?}",
                    &ops[..=index]
                )));
            }
        }
        Ok(())
    }
}

fn to_optimized_items(
    env: &Env,
    source: &std::vec::Vec<OptimizedBatchItem>,
) -> Vec<OptimizedBatchItem> {
    let mut items = Vec::new(env);
    for item in source {
        items.push_back(OptimizedBatchItem {
            id: item.id,
            score: item.score,
        });
    }
    items
}

fn optimized_pairs(results: &Vec<OptimizedBatchItem>) -> std::vec::Vec<(u64, u64)> {
    let mut pairs = std::vec::Vec::new();
    for i in 0..results.len() {
        let item = results.get(i).unwrap();
        pairs.push((item.id, item.score));
    }
    pairs
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prop_optimized_batch_matches_reference(
        generated in proptest::collection::vec((any::<u32>(), 1u64..1_000_000u64), 1..20)
    ) {
        let source: std::vec::Vec<OptimizedBatchItem> = generated
            .into_iter()
            .map(|(id, score)| OptimizedBatchItem { id: id as u64, score })
            .collect();

        let env_ref = Env::default();
        let contract_ref = env_ref.register_contract(None, SLACalculatorContract);
        let env_opt = Env::default();
        let contract_opt = env_opt.register_contract(None, SLACalculatorContract);
        let items_ref = to_optimized_items(&env_ref, &source);
        let items_opt = to_optimized_items(&env_opt, &source);

        let reference = process_reference_batch(&env_ref, &items_ref);
        let optimized = OptimizedBatchCalculator::process_optimized_batch(&env_opt, &items_opt);
        prop_assert_eq!(optimized_pairs(&optimized), optimized_pairs(&reference));

        env_ref.as_contract(&contract_ref, || {
            process_reference_batch_with_effects(&env_ref, &items_ref);
        });
        env_opt.as_contract(&contract_opt, || {
            OptimizedBatchCalculator::process_optimized_batch_with_effects(&env_opt, &items_opt);
        });
        let reference_effects = env_ref.as_contract(&contract_ref, || optimized_batch_effects(&env_ref));
        let optimized_effects = env_opt.as_contract(&contract_opt, || optimized_batch_effects(&env_opt));
        prop_assert_eq!(optimized_pairs(&reference_effects.0), optimized_pairs(&optimized_effects.0));
        prop_assert_eq!(reference_effects.1, optimized_effects.1);
    }
}