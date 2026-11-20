use soroban_sdk::{symbol_short, Env, IntoVal, Map, Symbol, TryFromVal, Val, Vec};

use crate::config_version_hash::ConfigRevisionSnapshot;
use crate::storage_helpers::{read_or_default, StorageReadError};

pub const SCHEMA_VERSION_KEY: Symbol = symbol_short!("VER");
pub const CONFIG_KEY: Symbol = symbol_short!("CONFIG");
pub const STATS_KEY: Symbol = symbol_short!("STATS");
pub const HISTORY_KEY: Symbol = symbol_short!("HIST");
pub const EXPECTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[soroban_sdk::contracttype]
pub enum IntegrityIssue {
    MissingKey,
    SchemaVersionMismatch,
    /// History holds more entries than the stats counter accounts for.
    OrphanedHistory,
    CorruptValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    pub healthy: bool,
    pub issues: Vec<IntegrityIssue>,
    pub checked_keys: u32,
}

impl IntegrityReport {
    pub fn issue_count(&self) -> u32 {
        self.issues.len()
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy
    }
}

pub fn essential_keys() -> [Symbol; 4] {
    [CONFIG_KEY, STATS_KEY, HISTORY_KEY, SCHEMA_VERSION_KEY]
}

pub fn read_checked<K, V>(env: &Env, key: &K, default: V) -> Result<V, IntegrityIssue>
where
    K: IntoVal<Env, Val>,
    V: TryFromVal<Env, Val>,
{
    read_or_default(env, key, default)
        .map_err(|StorageReadError::CorruptValue| IntegrityIssue::CorruptValue)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigRevisionIntegrity {
    Healthy,
    MissingSnapshot,
    DigestMismatch,
}

pub fn check_config_revision_integrity(env: &Env) -> ConfigRevisionIntegrity {
    let snapshot: Option<ConfigRevisionSnapshot> = env
        .storage()
        .instance()
        .get(&crate::config_version_hash::CONFIG_REVISION_KEY);
    let snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => return ConfigRevisionIntegrity::MissingSnapshot,
    };
    let configs: Option<Map<Symbol, crate::SLAConfig>> =
        env.storage().instance().get(&crate::CONFIG_KEY);
    let configs = match configs {
        Some(configs) => configs,
        None => return ConfigRevisionIntegrity::MissingSnapshot,
    };
    let current = crate::compute_config_version_hash(env, &configs);
    if snapshot.digest == current {
        ConfigRevisionIntegrity::Healthy
    } else {
        ConfigRevisionIntegrity::DigestMismatch
    }
}

fn collect_missing(env: &Env) -> Vec<IntegrityIssue> {
    let mut issues = Vec::new(env);
    for key in essential_keys() {
        if !env.storage().persistent().has(&key) {
            issues.push_back(IntegrityIssue::MissingKey);
        }
    }
    issues
}

fn collect_version_issues(env: &Env) -> Vec<IntegrityIssue> {
    let mut issues = Vec::new(env);
    let stored = env
        .storage()
        .persistent()
        .get::<Symbol, u32>(&SCHEMA_VERSION_KEY);
    if let Some(version) = stored {
        if version != EXPECTED_SCHEMA_VERSION {
            issues.push_back(IntegrityIssue::SchemaVersionMismatch);
        }
    }
    issues
}

/// Number of history entries not accounted for by the calculation counter.
///
/// Every history entry is written together with a stats increment, so
/// `history.len() > stats.total_calculations` means entries were written
/// without their counter update (e.g. after a partial failure). History may
/// legitimately be *shorter* than the counter because of pruning.
pub fn orphaned_history_entries(env: &Env) -> u32 {
    let history: Vec<crate::SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| Vec::new(env));
    let counted: u64 = env
        .storage()
        .instance()
        .get::<Symbol, crate::SLAStats>(&STATS_KEY)
        .map(|s| s.total_calculations)
        .unwrap_or(0);
    let counted = counted.min(u32::MAX as u64) as u32;
    history.len().saturating_sub(counted)
}

pub fn integrity_check(env: &Env) -> IntegrityReport {
    let mut issues = collect_missing(env);
    if orphaned_history_entries(env) > 0 {
        issues.push_back(IntegrityIssue::OrphanedHistory);
    }
    for issue in collect_version_issues(env) {
        issues.push_back(issue);
    }
    let checked_keys = essential_keys().len() as u32;
    IntegrityReport {
        healthy: issues.is_empty(),
        issues,
        checked_keys,
    }
}

pub fn duplicate_lock_amount_allowed(existing_amount: i128, proposed_amount: i128) -> bool {
    existing_amount == proposed_amount
}

// -----------------------------------------------------------------------
// #1023 — cross-field invariants for config mutation sequences
// -----------------------------------------------------------------------

/// A snapshot of the cross-field state that config mutations must preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigState {
    /// Threshold configured by governance, in minutes.
    pub configured_minutes: u32,
    /// Threshold actually applied after safety bounds, in minutes.
    pub effective_minutes: u32,
    /// Whether configuration changes are frozen.
    pub frozen: bool,
    /// Number of pending (not yet applied) revisions.
    pub pending_revisions: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigInvariantViolation {
    ConfiguredOutOfBounds,
    EffectiveOutOfBounds,
    EffectiveExceedsConfigured,
    FrozenWithPendingRevision,
}

/// `update`, `rollback`, `freeze` and `read` operations must all leave the
/// configuration in a state that satisfies these cross-field invariants.
pub fn check_config_invariants(state: ConfigState) -> Result<(), ConfigInvariantViolation> {
    const MIN: u32 = 1;
    const MAX: u32 = 1440;

    if state.configured_minutes < MIN || state.configured_minutes > MAX {
        return Err(ConfigInvariantViolation::ConfiguredOutOfBounds);
    }
    if state.effective_minutes < MIN || state.effective_minutes > MAX {
        return Err(ConfigInvariantViolation::EffectiveOutOfBounds);
    }
    if state.effective_minutes > state.configured_minutes {
        return Err(ConfigInvariantViolation::EffectiveExceedsConfigured);
    }
    if state.frozen && state.pending_revisions > 0 {
        return Err(ConfigInvariantViolation::FrozenWithPendingRevision);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::{testutils::Events as _, Address};

    fn seed(env: &Env, contract: &Address, version: u32) {
        env.as_contract(contract, || {
            let store = env.storage().persistent();
            store.set(&CONFIG_KEY, &1u32);
            store.set(&STATS_KEY, &2u32);
            store.set(&HISTORY_KEY, &3u32);
            store.set(&SCHEMA_VERSION_KEY, &version);
        });
    }

    #[test]
    fn a_fully_seeded_contract_is_healthy() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        seed(&env, &contract, EXPECTED_SCHEMA_VERSION);
        let report = env.as_contract(&contract, || integrity_check(&env));
        assert!(report.is_healthy());
        assert_eq!(report.issue_count(), 0);
        assert_eq!(report.checked_keys, 4);
    }

    #[test]
    fn an_uninitialized_contract_reports_every_missing_key() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        let report = env.as_contract(&contract, || integrity_check(&env));
        assert!(!report.is_healthy());
        assert_eq!(report.issue_count(), 4);
    }

    #[test]
    fn a_single_missing_key_is_reported() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        seed(&env, &contract, EXPECTED_SCHEMA_VERSION);
        env.as_contract(&contract, || {
            env.storage().persistent().remove(&STATS_KEY);
        });
        let report = env.as_contract(&contract, || integrity_check(&env));
        assert!(!report.is_healthy());
        assert_eq!(report.issue_count(), 1);
        assert_eq!(report.issues.get(0), Some(IntegrityIssue::MissingKey));
    }

    #[test]
    fn a_stale_schema_version_is_reported() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        seed(&env, &contract, EXPECTED_SCHEMA_VERSION + 1);
        let report = env.as_contract(&contract, || integrity_check(&env));
        assert!(!report.is_healthy());
        assert_eq!(report.issue_count(), 1);
        assert_eq!(
            report.issues.get(0),
            Some(IntegrityIssue::SchemaVersionMismatch)
        );
    }

    #[test]
    fn the_check_is_read_only() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        seed(&env, &contract, EXPECTED_SCHEMA_VERSION);
        env.as_contract(&contract, || integrity_check(&env));
        let report = env.as_contract(&contract, || integrity_check(&env));
        assert!(report.is_healthy());
        assert!(env.events().all().is_empty());
    }

    #[test]
    fn essential_keys_cover_config_stats_history_and_version() {
        assert_eq!(essential_keys().len(), 4);
    }
}

#[cfg(test)]
mod orphan_and_partial_failure_tests {
    use super::*;
    use crate::{SLAResult, SLAStats, SLACalculatorContract};
    use soroban_sdk::{BytesN, Vec};

    fn entry(env: &Env, n: u64) -> SLAResult {
        SLAResult {
            outage_id: symbol_short!("o"),
            status: symbol_short!("met"),
            mttr_minutes: 5,
            threshold_minutes: 30,
            amount: 10,
            payment_type: symbol_short!("rew"),
            rating: symbol_short!("good"),
            config_version_hash: BytesN::from_array(env, &[0u8; 32]),
            recorded_at: n,
        }
    }

    fn write_history(env: &Env, count: u64) {
        let mut h: Vec<SLAResult> = Vec::new(env);
        for n in 0..count {
            h.push_back(entry(env, n));
        }
        env.storage().instance().set(&HISTORY_KEY, &h);
    }

    fn write_counter(env: &Env, calcs: u64) {
        env.storage().instance().set(
            &STATS_KEY,
            &SLAStats {
                total_calculations: calcs,
                total_violations: 0,
                total_rewards: 0,
                total_penalties: 0,
            },
        );
    }

    fn seed_persistent(env: &Env) {
        let store = env.storage().persistent();
        store.set(&CONFIG_KEY, &1u32);
        store.set(&STATS_KEY, &2u32);
        store.set(&HISTORY_KEY, &3u32);
        store.set(&SCHEMA_VERSION_KEY, &EXPECTED_SCHEMA_VERSION);
    }

    #[test]
    fn test_no_orphans_when_history_matches_the_counter() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            write_history(&env, 3);
            write_counter(&env, 3);
            assert_eq!(orphaned_history_entries(&env), 0);
        });
    }

    #[test]
    fn test_pruned_history_is_not_an_orphan() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            write_history(&env, 2);
            write_counter(&env, 10);
            assert_eq!(orphaned_history_entries(&env), 0);
        });
    }

    #[test]
    fn test_history_written_without_its_counter_update_is_orphaned() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            seed_persistent(&env);
            write_history(&env, 5);
            write_counter(&env, 3); // partial failure: counter lagged behind
            assert_eq!(orphaned_history_entries(&env), 2);
            let report = integrity_check(&env);
            assert!(!report.is_healthy());
            assert_eq!(report.issues.get(0), Some(IntegrityIssue::OrphanedHistory));
        });
    }

    #[test]
    fn test_integrity_recovers_once_the_partial_write_is_repaired() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            seed_persistent(&env);
            write_history(&env, 5);
            write_counter(&env, 3);
            assert!(!integrity_check(&env).is_healthy());
            write_counter(&env, 5);
            assert!(integrity_check(&env).is_healthy());
        });
    }

    #[test]
    fn test_a_lost_key_after_a_partial_failure_is_reported_without_masking_others() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            seed_persistent(&env);
            env.storage().persistent().remove(&CONFIG_KEY);
            write_history(&env, 4);
            write_counter(&env, 1);
            let report = integrity_check(&env);
            assert_eq!(report.issue_count(), 2);
            assert_eq!(report.issues.get(0), Some(IntegrityIssue::MissingKey));
            assert_eq!(report.issues.get(1), Some(IntegrityIssue::OrphanedHistory));
        });
    }
}

#[cfg(test)]
mod lock_id_integrity_tests {
    use super::*;

    #[test]
    fn same_amount_is_an_idempotent_duplicate() {
        assert!(duplicate_lock_amount_allowed(1_000, 1_000));
    }

    #[test]
    fn a_different_amount_is_rejected() {
        assert!(!duplicate_lock_amount_allowed(1_000, 1_001));
    }
}

#[cfg(test)]
mod checked_read_tests {
    use super::*;
    use crate::SLACalculatorContract;

    #[test]
    fn a_missing_key_uses_the_documented_default() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let key = symbol_short!("none");
            assert_eq!(read_checked::<_, u32>(&env, &key, 5), Ok(5));
        });
    }

    #[test]
    fn a_corrupt_value_maps_to_a_typed_integrity_issue() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let key = symbol_short!("bad");
            env.storage().instance().set(&key, &symbol_short!("nope"));
            assert_eq!(
                read_checked::<_, u32>(&env, &key, 5),
                Err(IntegrityIssue::CorruptValue)
            );
            let raw: soroban_sdk::Symbol = env.storage().instance().get(&key).unwrap();
            assert_eq!(raw, symbol_short!("nope"));
        });
    }
}

#[cfg(test)]
mod config_revision_integrity_tests {
    use super::*;
    use crate::config_version_hash::{write_config_revision, CONFIG_REVISION_KEY};
    use crate::{SLAConfig, SLACalculatorContract};
    use soroban_sdk::{testutils::Events as _, BytesN};

    fn configs(env: &Env) -> Map<Symbol, SLAConfig> {
        let mut map = Map::new(env);
        map.set(
            symbol_short!("critical"),
            SLAConfig {
                threshold_minutes: 30,
                penalty_per_minute: 50,
                reward_base: 500,
                top_tier_multiplier: 200,
                excel_tier_multiplier: 150,
                good_tier_multiplier: 100,
            },
        );
        map
    }

    fn with_contract<R>(env: &Env, f: impl FnOnce() -> R) -> R {
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, f)
    }

    #[test]
    fn a_missing_revision_snapshot_is_reported() {
        let env = Env::default();
        with_contract(&env, || {
            env.storage()
                .instance()
                .set(&crate::CONFIG_KEY, &configs(&env));
            assert_eq!(
                check_config_revision_integrity(&env),
                ConfigRevisionIntegrity::MissingSnapshot
            );
        });
    }

    #[test]
    fn a_stale_digest_is_reported_as_a_mismatch() {
        let env = Env::default();
        with_contract(&env, || {
            env.storage()
                .instance()
                .set(&crate::CONFIG_KEY, &configs(&env));
            write_config_revision(&env, 1, &BytesN::from_array(&env, &[1u8; 32]));
            assert_eq!(
                check_config_revision_integrity(&env),
                ConfigRevisionIntegrity::DigestMismatch
            );
        });
    }

    #[test]
    fn a_matching_digest_is_healthy() {
        let env = Env::default();
        with_contract(&env, || {
            let cfg = configs(&env);
            env.storage().instance().set(&crate::CONFIG_KEY, &cfg);
            let digest = crate::compute_config_version_hash(&env, &cfg);
            write_config_revision(&env, 1, &digest);
            assert_eq!(
                check_config_revision_integrity(&env),
                ConfigRevisionIntegrity::Healthy
            );
        });
    }

    #[test]
    fn the_check_is_read_only() {
        let env = Env::default();
        with_contract(&env, || {
            let cfg = configs(&env);
            env.storage().instance().set(&crate::CONFIG_KEY, &cfg);
            let digest = crate::compute_config_version_hash(&env, &cfg);
            write_config_revision(&env, 1, &digest);

            assert_eq!(
                check_config_revision_integrity(&env),
                ConfigRevisionIntegrity::Healthy
            );
            assert_eq!(
                check_config_revision_integrity(&env),
                ConfigRevisionIntegrity::Healthy
            );
            let stored: ConfigRevisionSnapshot = env
                .storage()
                .instance()
                .get(&CONFIG_REVISION_KEY)
                .unwrap();
            assert_eq!(stored.revision, 1);
            assert_eq!(stored.digest, digest);
            assert!(env.events().all().is_empty());
        });
    }
}
