#![no_std]

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::{
    SLAConfig, SLAResult, SLAStats, SLAConfigSnapshot, SLAConfigEntry,
    SLAError, SLAResultSchema, ContractMetadata, PauseInfo, VersionInfo,
    ADMIN_KEY, OPERATOR_KEY, CONFIG_KEY, PAUSED_KEY, PAUSE_INFO_KEY,
    STATS_KEY, HISTORY_KEY, STORAGE_VERSION_KEY, STORAGE_VERSION,
    RESULT_SCHEMA_VERSION,
};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const SNAPSHOT_KEY: Symbol = symbol_short!("SNAP");
const RETENTION_LIMIT_KEY: Symbol = symbol_short!("RETLIM");
const MAX_HISTORY_SIZE: u32 = 1000;

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Complete contract state snapshot for backup and migration.
#[soroban_sdk::contracttype]
pub struct ContractSnapshot {
    /// Snapshot metadata.
    pub metadata: SnapshotMetadata,
    /// Current admin address.
    pub admin: soroban_sdk::Address,
    /// Current operator address.
    pub operator: soroban_sdk::Address,
    /// Whether the contract is paused.
    pub is_paused: bool,
    /// Pause info if paused.
    pub pause_info: Option<PauseInfo>,
    /// All severity configurations.
    pub config_snapshot: SLAConfigSnapshot,
    /// Cumulative statistics.
    pub stats: SLAStats,
    /// Calculation history.
    pub history: soroban_sdk::Vec<SLAResult>,
    /// Storage version.
    pub storage_version: u32,
    /// Result schema version.
    pub result_schema_version: u32,
    /// Retention limit.
    pub retention_limit: u32,
}

/// Snapshot metadata for tracking and validation.
#[soroban_sdk::contracttype]
pub struct SnapshotMetadata {
    /// Snapshot version for format evolution.
    pub version: u32,
    /// Ledger timestamp when snapshot was taken.
    pub created_at: u64,
    /// Contract name.
    pub contract_name: soroban_sdk::Symbol,
    /// Number of history entries included.
    pub history_count: u32,
    /// Number of severity configurations.
    pub config_count: u32,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Export a complete snapshot of the contract state.
///
/// Captures all mutable state including configuration, statistics,
/// history, and pause status. Useful for:
/// - Pre-upgrade backups
/// - State migration between contracts
/// - Audit and compliance
/// - Debugging and testing
///
/// # Returns
/// A complete `ContractSnapshot` containing all contract state.
pub fn export_snapshot(env: &Env) -> Result<ContractSnapshot, SLAError> {
    let admin: soroban_sdk::Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;

    let operator: soroban_sdk::Address = env
        .storage()
        .instance()
        .get(&OPERATOR_KEY)
        .ok_or(SLAError::NotInitialized)?;

    let is_paused: bool = env.storage().instance().get(&PAUSED_KEY).unwrap_or(false);
    let pause_info: Option<PauseInfo> = env.storage().instance().get(&PAUSE_INFO_KEY);

    let configs: soroban_sdk::Map<Symbol, SLAConfig> = env
        .storage()
        .instance()
        .get(&CONFIG_KEY)
        .ok_or(SLAError::NotInitialized)?;

    // Build config snapshot in canonical order
    let mut entries = soroban_sdk::Vec::new(env);
    for severity in [
        symbol_short!("critical"),
        symbol_short!("high"),
        symbol_short!("medium"),
        symbol_short!("low"),
    ] {
        if let Some(config) = configs.get(severity.clone()) {
            entries.push_back(SLAConfigEntry { severity, config });
        }
    }
    let config_snapshot = SLAConfigSnapshot {
        version: symbol_short!("v1"),
        entries,
    };

    let stats: SLAStats = env
        .storage()
        .instance()
        .get(&STATS_KEY)
        .unwrap_or(SLAStats {
            total_calculations: 0,
            total_violations: 0,
            total_rewards: 0,
            total_penalties: 0,
        });

    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let storage_version: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(0);

    let retention_limit: u32 = env
        .storage()
        .instance()
        .get(&RETENTION_LIMIT_KEY)
        .unwrap_or(MAX_HISTORY_SIZE);

    let now = env.ledger().timestamp();

    Ok(ContractSnapshot {
        metadata: SnapshotMetadata {
            version: 1,
            created_at: now,
            contract_name: symbol_short!("sla_calc"),
            history_count: history.len(),
            config_count: entries.len(),
        },
        admin,
        operator,
        is_paused,
        pause_info,
        config_snapshot,
        stats,
        history,
        storage_version,
        result_schema_version: RESULT_SCHEMA_VERSION,
        retention_limit,
    })
}

/// Get snapshot metadata without exporting full state.
///
/// Lightweight query for monitoring and health checks.
pub fn get_snapshot_metadata(env: &Env) -> Result<SnapshotMetadata, SLAError> {
    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let configs: soroban_sdk::Map<Symbol, SLAConfig> = env
        .storage()
        .instance()
        .get(&CONFIG_KEY)
        .ok_or(SLAError::NotInitialized)?;

    Ok(SnapshotMetadata {
        version: 1,
        created_at: env.ledger().timestamp(),
        contract_name: symbol_short!("sla_calc"),
        history_count: history.len(),
        config_count: configs.len(),
    })
}

/// Validate that a snapshot can be imported (dry-run check).
///
/// Returns Ok(()) if the snapshot is valid for import, or an error
/// describing why it cannot be imported.
pub fn validate_snapshot_import(
    env: &Env,
    snapshot: &ContractSnapshot,
) -> Result<(), SLAError> {
    // Check snapshot version
    if snapshot.metadata.version != 1 {
        return Err(SLAError::VersionMismatch);
    }

    // Check storage version compatibility
    if snapshot.storage_version > STORAGE_VERSION {
        return Err(SLAError::VersionMismatch);
    }

    // Check config count
    if snapshot.config_snapshot.entries.len() == 0 {
        return Err(SLAError::ConfigNotFound);
    }

    // Verify contract is initialized
    if !env.storage().instance().has(&ADMIN_KEY) {
        return Err(SLAError::NotInitialized);
    }

    Ok(())
}

/// Get a summary of contract state for health checks.
///
/// Returns key metrics without full state export.
pub fn get_state_summary(env: &Env) -> Result<soroban_sdk::Map<Symbol, soroban_sdk::Val>, SLAError> {
    let mut summary = soroban_sdk::Map::new(env);

    let is_paused: bool = env.storage().instance().get(&PAUSED_KEY).unwrap_or(false);
    summary.set(symbol_short!("paused"), soroban_sdk::Val::from_bool(is_paused));

    let stats: SLAStats = env
        .storage()
        .instance()
        .get(&STATS_KEY)
        .unwrap_or(SLAStats {
            total_calculations: 0,
            total_violations: 0,
            total_rewards: 0,
            total_penalties: 0,
        });

    summary.set(
        symbol_short!("calc_cnt"),
        soroban_sdk::Val::from_u64(stats.total_calculations),
    );
    summary.set(
        symbol_short!("viol_cnt"),
        soroban_sdk::Val::from_u64(stats.total_violations),
    );

    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    summary.set(
        symbol_short!("hist_len"),
        soroban_sdk::Val::from_u32(history.len()),
    );

    let storage_version: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(0);
    summary.set(
        symbol_short!("stor_ver"),
        soroban_sdk::Val::from_u32(storage_version),
    );

    Ok(summary)
}
