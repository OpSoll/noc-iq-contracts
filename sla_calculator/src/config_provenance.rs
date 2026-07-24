#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const PROVENANCE_KEY: Symbol = symbol_short!("PROV");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_PROV_REC: Symbol = symbol_short!("prov_rec");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Configuration provenance record.
#[soroban_sdk::contracttype]
pub struct ProvenanceRecord {
    /// Severity tier that was modified.
    pub severity: Symbol,
    /// Admin who made the change.
    pub changed_by: Address,
    /// Timestamp of the change.
    pub changed_at: u64,
    /// Previous threshold value.
    pub old_threshold: u32,
    /// New threshold value.
    pub new_threshold: u32,
    /// Previous penalty per minute.
    pub old_penalty: i128,
    /// New penalty per minute.
    pub new_penalty: i128,
    /// Previous reward base.
    pub old_reward: i128,
    /// New reward base.
    pub new_reward: i128,
    /// Optional reason for the change.
    pub reason: soroban_sdk::String,
}

/// Provenance log for a severity tier.
#[soroban_sdk::contracttype]
pub struct ProvenanceLog {
    /// History of changes for this severity.
    pub records: soroban_sdk::Vec<ProvenanceRecord>,
    /// Total number of changes.
    pub count: u32,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Record a configuration change for provenance tracking.
///
/// Should be called after every set_config operation.
pub fn record_config_change(
    env: &Env,
    severity: &Symbol,
    old_threshold: u32,
    new_threshold: u32,
    old_penalty: i128,
    new_penalty: i128,
    old_reward: i128,
    new_reward: i128,
    reason: soroban_sdk::String,
) {
    let caller: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .unwrap_or_else(|| Address::generate(env));

    let record = ProvenanceRecord {
        severity: severity.clone(),
        changed_by: caller,
        changed_at: env.ledger().timestamp(),
        old_threshold,
        new_threshold,
        old_penalty,
        new_penalty,
        old_reward,
        new_reward,
        reason,
    };

    let mut log: ProvenanceLog = env
        .storage()
        .instance()
        .get(&PROVENANCE_KEY)
        .unwrap_or(ProvenanceLog {
            records: soroban_sdk::Vec::new(env),
            count: 0,
        });

    log.records.push_back(record);
    log.count = log.count.saturating_add(1);
    env.storage().instance().set(&PROVENANCE_KEY, &log);

    env.events().publish(
        (EVENT_PROV_REC, EVENT_VERSION, severity),
        (log.count,),
    );
}

/// Get the full provenance log.
pub fn get_provenance_log(
    env: &Env,
) -> Result<ProvenanceLog, SLAError> {
    Ok(env
        .storage()
        .instance()
        .get(&PROVENANCE_KEY)
        .unwrap_or(ProvenanceLog {
            records: soroban_sdk::Vec::new(env),
            count: 0,
        }))
}

/// Get provenance records for a specific severity.
pub fn get_severity_provenance(
    env: &Env,
    severity: &Symbol,
) -> Result<soroban_sdk::Vec<ProvenanceRecord>, SLAError> {
    let log: ProvenanceLog = env
        .storage()
        .instance()
        .get(&PROVENANCE_KEY)
        .unwrap_or(ProvenanceLog {
            records: soroban_sdk::Vec::new(env),
            count: 0,
        });

    let mut filtered = soroban_sdk::Vec::new(env);
    for i in 0..log.records.len() {
        let record = log.records.get(i).unwrap();
        if record.severity == *severity {
            filtered.push_back(record);
        }
    }

    Ok(filtered)
}

/// Get the most recent change for a severity.
pub fn get_latest_change(
    env: &Env,
    severity: &Symbol,
) -> Result<Option<ProvenanceRecord>, SLAError> {
    let records = get_severity_provenance(env, severity)?;
    if records.len() > 0 {
        Ok(Some(records.get(records.len() - 1).unwrap()))
    } else {
        Ok(None)
    }
}

/// Get total number of configuration changes.
pub fn get_change_count(env: &Env) -> Result<u32, SLAError> {
    let log: ProvenanceLog = env
        .storage()
        .instance()
        .get(&PROVENANCE_KEY)
        .unwrap_or(ProvenanceLog {
            records: soroban_sdk::Vec::new(env),
            count: 0,
        });
    Ok(log.count)
}

/// Get provenance records in a time range.
pub fn get_changes_in_range(
    env: &Env,
    from_timestamp: u64,
    to_timestamp: u64,
) -> Result<soroban_sdk::Vec<ProvenanceRecord>, SLAError> {
    let log: ProvenanceLog = env
        .storage()
        .instance()
        .get(&PROVENANCE_KEY)
        .unwrap_or(ProvenanceLog {
            records: soroban_sdk::Vec::new(env),
            count: 0,
        });

    let mut filtered = soroban_sdk::Vec::new(env);
    for i in 0..log.records.len() {
        let record = log.records.get(i).unwrap();
        if record.changed_at >= from_timestamp && record.changed_at <= to_timestamp {
            filtered.push_back(record);
        }
    }

    Ok(filtered)
}

/// Get provenance records by admin address.
pub fn get_changes_by_admin(
    env: &Env,
    admin: &Address,
) -> Result<soroban_sdk::Vec<ProvenanceRecord>, SLAError> {
    let log: ProvenanceLog = env
        .storage()
        .instance()
        .get(&PROVENANCE_KEY)
        .unwrap_or(ProvenanceLog {
            records: soroban_sdk::Vec::new(env),
            count: 0,
        });

    let mut filtered = soroban_sdk::Vec::new(env);
    for i in 0..log.records.len() {
        let record = log.records.get(i).unwrap();
        if record.changed_by == *admin {
            filtered.push_back(record);
        }
    }

    Ok(filtered)
}
