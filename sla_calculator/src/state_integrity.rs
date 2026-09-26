use soroban_sdk::{symbol_short, Env, Symbol, Vec};

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

pub fn integrity_check(env: &Env) -> IntegrityReport {
    let mut issues = collect_missing(env);
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
