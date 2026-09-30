//! SC-W5-041 – Canonical event schema for SLA calculation outputs.
//!
//! This module defines the canonical schema for all contract events consumed
//! by backend indexers. Every event follows the same structural contract:
//!
//! Topic layout (3 topics):
//!   topic[0] = event name (Symbol constant)
//!   topic[1] = event version ("v1")
//!   topic[2] = event-specific context (severity, caller address, etc.)
//!
//! Payload field ordering and types are documented below per event variant.
//! These schemas MUST NOT be changed without a corresponding version bump.
//!
//! # Event Catalog
//!
//! ## sla_calc (`sla_calc`)
//! Emitted on every successful `calculate_sla` call.
//! - topic[2]: severity Symbol
//! - payload: (outage_id: Symbol, status: Symbol, payment_type: Symbol,
//!   rating: Symbol, mttr_minutes: u32, threshold_minutes: u32, amount: i128)
//!
//! ## set_int (`set_int`)
//! Settlement intent emitted alongside sla_calc for backend reconciliation.
//! - topic[2]: severity Symbol
//! - payload: (outage_id: Symbol, status: Symbol, payment_type: Symbol,
//!   amount: i128, config_version_hash: u64, recorded_at: u64)
//!
//! ## cfg_upd (`cfg_upd`)
//! Emitted on every successful `set_config` call.
//! - topic[2]: severity Symbol
//! - payload: (threshold_minutes: u32, penalty_per_minute: i128, reward_base: i128)
//!
//! ## paused (`paused`)
//! Emitted when the contract is paused.
//! - topic[2]: caller Address
//! - payload: (true,)
//!
//! ## unpause (`unpause`)
//! Emitted when the contract is unpaused.
//! - topic[2]: caller Address
//! - payload:  (false,)
//!
//! ## op_set (`op_set`)
//! Emitted on operator change.
//! - topic[2]: caller Address
//! - payload:  (new_operator: Address,)
//!
//! ## pruned (`pruned`)
//! Emitted after a prune_history call removes entries.
//! - topic[2]: caller Address
//! - payload:  (removed_count: u32, kept_count: u32)
//!
//! ## pruned_a (`pruned_a`)
//! Emitted after a prune_history_by_age call removes entries.
//! - topic[2]: caller Address
//! - payload:  (removed_count: u32, kept_count: u32)
//!
//! ## adm_prop (`adm_prop`)
//! Emitted when a new admin is proposed.
//! - topic[2]: caller Address
//! - payload:  (new_admin: Address,)
//!
//! ## adm_acc (`adm_acc`)
//! Emitted when a pending admin proposal is accepted.
//! - topic[2]: caller Address
//! - payload:  ()
//!
//! ## adm_can (`adm_can`)
//! Emitted when a pending admin proposal is cancelled.
//! - topic[2]: caller Address
//! - payload:  ()
//!
//! ## adm_ren (`adm_ren`)
//! Emitted when the admin renounces their role.
//! - topic[2]: caller Address
//! - payload:  ()
//!
//! ## op_prop (`op_prop`)
//! Emitted when a new operator is proposed.
//! - topic[2]: caller Address
//! - payload:  (new_operator: Address,)
//!
//! ## op_acc (`op_acc`)
//! Emitted when a pending operator proposal is accepted.
//! - topic[2]: caller Address
//! - payload:  ()
//!
//! ## op_can (`op_can`)
//! Emitted when a pending operator proposal is cancelled.
//! - topic[2]: caller Address
//! - payload:  ()
//!
//! # Schema Versioning
//!
//! Breaking changes (field removal, type changes, reordering) MUST increment
//! the version symbol from "v1" to "v2". Additive changes (new fields at the
//! end) are NOT considered breaking and do not require a version bump as long
//! as old consumers ignore unrecognised trailing fields.

use soroban_sdk::{contracttype, symbol_short, Env, Symbol};

use crate::SLAError;

/// Canonical event version symbol used by all events.
#[allow(dead_code)]
pub const EVENT_VERSION: Symbol = symbol_short!("v1");

/// Event name constants — these form topic[0] of every event.
#[allow(dead_code)]
pub const EVENT_SLA_CALC: Symbol = symbol_short!("sla_calc");
#[allow(dead_code)]
pub const EVENT_SETTLE_INTENT: Symbol = symbol_short!("set_int");
#[allow(dead_code)]
pub const EVENT_CONFIG_UPD: Symbol = symbol_short!("cfg_upd");
#[allow(dead_code)]
pub const EVENT_PAUSED: Symbol = symbol_short!("paused");
#[allow(dead_code)]
pub const EVENT_UNPAUSED: Symbol = symbol_short!("unpause");
#[allow(dead_code)]
pub const EVENT_OP_SET: Symbol = symbol_short!("op_set");
#[allow(dead_code)]
pub const EVENT_PRUNED: Symbol = symbol_short!("pruned");
#[allow(dead_code)]
pub const EVENT_PRUNED_AGE: Symbol = symbol_short!("pruned_a");
#[allow(dead_code)]
pub const EVENT_ADMIN_PROP: Symbol = symbol_short!("adm_prop");
#[allow(dead_code)]
pub const EVENT_ADMIN_ACC: Symbol = symbol_short!("adm_acc");
#[allow(dead_code)]
pub const EVENT_ADMIN_CAN: Symbol = symbol_short!("adm_can");
#[allow(dead_code)]
pub const EVENT_ADMIN_REN: Symbol = symbol_short!("adm_ren");
#[allow(dead_code)]
pub const EVENT_OP_PROP: Symbol = symbol_short!("op_prop");
#[allow(dead_code)]
pub const EVENT_OP_ACC: Symbol = symbol_short!("op_acc");
#[allow(dead_code)]
pub const EVENT_OP_CAN: Symbol = symbol_short!("op_can");
#[allow(dead_code)]
pub const EVENT_SLA_VIOLATED: Symbol = symbol_short!("sla_viol"); // #594
#[allow(dead_code)]
pub const EVENT_SLA_MET: Symbol = symbol_short!("sla_met"); // #595

/// Returns the canonical event version string for consumer documentation.
#[allow(dead_code)]
pub fn current_event_version() -> Symbol {
    EVENT_VERSION
}

// -----------------------------------------------------------------------
// Issue #664 – non-monotonic timestamp detection in outage ingestion
// -----------------------------------------------------------------------

/// Instance-storage key holding the end timestamp of the newest event that was
/// successfully ingested — the monotonic high-water mark.
const LAST_EVENT_END_KEY: Symbol = symbol_short!("LASTEND");

/// A raw outage event submitted by the ingest pipeline.
///
/// `start_timestamp` / `end_timestamp` are ledger timestamps in seconds.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutageEvent {
    pub outage_id: Symbol,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

/// Pure timestamp-sequence guard.
///
/// Rejects a payload whose `start_timestamp` predates the last accepted event's
/// end timestamp, so out-of-order events cannot corrupt availability metrics.
/// Identical timestamps are permitted so several sites that fail (or recover)
/// in the same ledger can be ingested together.
///
/// A payload that ends before it starts is malformed and rejected as well.
/// Both cases surface as [`SLAError::InvalidTimestampSequence`].
pub fn validate_timestamp_sequence(
    start_timestamp: u64,
    end_timestamp: u64,
    last_end_timestamp: Option<u64>,
) -> Result<(), SLAError> {
    if end_timestamp < start_timestamp {
        return Err(SLAError::InvalidTimestampSequence);
    }

    if let Some(last_end) = last_end_timestamp {
        if start_timestamp < last_end {
            return Err(SLAError::InvalidTimestampSequence);
        }
    }

    Ok(())
}

/// Returns the stored monotonic high-water mark, if any event has been ingested.
pub fn last_event_end_timestamp(env: &Env) -> Option<u64> {
    env.storage().instance().get(&LAST_EVENT_END_KEY)
}

/// Validates and persists an ingested outage event.
///
/// Out-of-order payloads are rejected with
/// [`SLAError::InvalidTimestampSequence`] and leave the high-water mark
/// untouched. Accepted events advance the mark to the newest end timestamp
/// observed, keeping the sequence check monotonic across calls.
pub fn ingest_outage_event(env: &Env, event: &OutageEvent) -> Result<(), SLAError> {
    let last_end = last_event_end_timestamp(env);
    validate_timestamp_sequence(event.start_timestamp, event.end_timestamp, last_end)?;

    // Sequence validation guarantees `end_timestamp >= start_timestamp >= last_end`,
    // so the high-water mark can only ever move forward.
    env.storage()
        .instance()
        .set(&LAST_EVENT_END_KEY, &event.end_timestamp);

    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::format;

    #[test]
    fn test_event_version_is_stable() {
        assert_eq!(current_event_version(), symbol_short!("v1"));
    }

    #[test]
    fn test_event_names_are_distinct() {
        let names = [
            EVENT_SLA_CALC,
            EVENT_SETTLE_INTENT,
            EVENT_CONFIG_UPD,
            EVENT_PAUSED,
            EVENT_UNPAUSED,
            EVENT_OP_SET,
            EVENT_PRUNED,
            EVENT_PRUNED_AGE,
            EVENT_ADMIN_PROP,
            EVENT_ADMIN_ACC,
            EVENT_ADMIN_CAN,
            EVENT_ADMIN_REN,
            EVENT_OP_PROP,
            EVENT_OP_ACC,
            EVENT_OP_CAN,
            EVENT_SLA_VIOLATED,
            EVENT_SLA_MET,
        ];

        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                assert_ne!(
                    names[i], names[j],
                    "event name collision: {:?} == {:?}",
                    names[i], names[j]
                );
            }
        }
    }

    #[test]
    fn test_event_version_is_short_enough() {
        let version_str = format!("{:?}", current_event_version());
        assert!(version_str.len() <= 32, "Version symbol too long");
    }
}

#[cfg(test)]
mod timestamp_sequence_tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn with_contract<R>(env: &Env, f: impl FnOnce() -> R) -> R {
        let contract_id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract_id, f)
    }

    fn event(env: &Env, outage_id: &str, start_timestamp: u64, end_timestamp: u64) -> OutageEvent {
        OutageEvent {
            outage_id: Symbol::new(env, outage_id),
            start_timestamp,
            end_timestamp,
        }
    }

    #[test]
    fn the_first_event_has_no_predecessor() {
        assert_eq!(validate_timestamp_sequence(10, 20, None), Ok(()));
    }

    #[test]
    fn an_out_of_order_start_is_rejected() {
        assert_eq!(
            validate_timestamp_sequence(90, 150, Some(100)),
            Err(SLAError::InvalidTimestampSequence)
        );
    }

    #[test]
    fn identical_timestamps_are_allowed_for_simultaneous_sites() {
        // Two sites fail and recover in the same ledger.
        assert_eq!(validate_timestamp_sequence(100, 100, Some(100)), Ok(()));
        // A site recovering in the same ledger another one failed.
        assert_eq!(validate_timestamp_sequence(100, 250, Some(100)), Ok(()));
    }

    #[test]
    fn an_event_that_ends_before_it_starts_is_rejected() {
        assert_eq!(
            validate_timestamp_sequence(200, 100, None),
            Err(SLAError::InvalidTimestampSequence)
        );
        assert_eq!(
            validate_timestamp_sequence(200, 100, Some(50)),
            Err(SLAError::InvalidTimestampSequence)
        );
    }

    #[test]
    fn a_later_start_is_accepted() {
        assert_eq!(validate_timestamp_sequence(500, 600, Some(400)), Ok(()));
    }

    #[test]
    fn ingestion_advances_the_high_water_mark() {
        let env = Env::default();
        with_contract(&env, || {
            assert_eq!(last_event_end_timestamp(&env), None);

            ingest_outage_event(&env, &event(&env, "out1", 100, 200)).unwrap();
            assert_eq!(last_event_end_timestamp(&env), Some(200));

            // In-order follow-up is accepted.
            ingest_outage_event(&env, &event(&env, "out2", 200, 260)).unwrap();
            assert_eq!(last_event_end_timestamp(&env), Some(260));

            // An out-of-order event is rejected...
            assert_eq!(
                ingest_outage_event(&env, &event(&env, "out3", 150, 300)),
                Err(SLAError::InvalidTimestampSequence)
            );
            // ...and the rejected payload must not move the high-water mark.
            assert_eq!(last_event_end_timestamp(&env), Some(260));
        });
    }

    #[test]
    fn simultaneous_multi_site_events_are_ingested_together() {
        let env = Env::default();
        with_contract(&env, || {
            // Two sites fail in the same ledger, then both recover together.
            ingest_outage_event(&env, &event(&env, "site_a", 1_000, 2_000)).unwrap();
            ingest_outage_event(&env, &event(&env, "site_b", 2_000, 2_000)).unwrap();
            assert_eq!(last_event_end_timestamp(&env), Some(2_000));
        });
    }
}
