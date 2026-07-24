#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY, STORAGE_VERSION_KEY, STORAGE_VERSION};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const UPGRADE_LOG_KEY: Symbol = symbol_short!("UPLOG");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Record of a past upgrade.
#[soroban_sdk::contracttype]
pub struct UpgradeRecord {
    /// Storage version before upgrade.
    pub from_version: u32,
    /// Storage version after upgrade.
    pub to_version: u32,
    /// Ledger timestamp of upgrade.
    pub timestamp: u64,
    /// Address that performed the upgrade.
    pub upgraded_by: Address,
}

/// Upgrade plan for forward compatibility.
#[soroban_sdk::contracttype]
pub struct UpgradePlan {
    /// Target storage version.
    pub target_version: u32,
    /// Whether the plan has been executed.
    pub executed: bool,
    /// Timestamp when plan was created.
    pub created_at: u64,
}

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_UPGRADED: Symbol = symbol_short!("upgraded");
const EVENT_PLAN_CREATED: Symbol = symbol_short!("up_plan");
const EVENT_PLAN_EXECUTED: Symbol = symbol_short!("up_exec");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Perform a versioned upgrade from the current storage version to the next.
///
/// This function should be called after deploying a new contract binary that
/// bumps STORAGE_VERSION. It applies all migration steps sequentially and
/// records the upgrade in the log.
///
/// # Arguments
/// - `caller`: Must be the current admin.
///
/// # Events
/// - `upgraded`: Emitted after successful upgrade with version details.
///
/// # Errors
/// Returns `Unauthorized` if caller is not admin, `VersionMismatch` if
/// already at current version or if stored version is newer than expected.
pub fn perform_upgrade(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }

    let stored: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(0);

    if stored == STORAGE_VERSION {
        return Ok(()); // Already current
    }

    if stored > STORAGE_VERSION {
        return Err(SLAError::VersionMismatch);
    }

    let from_version = stored;
    let mut current = stored;

    // v0 → v1: stamp version (initialize sets all other fields)
    if current == 0 {
        env.storage().instance().set(&STORAGE_VERSION_KEY, &1u32);
        current = 1;
    }

    // Future migrations go here:
    // if current == 1 { ... current = 2; }

    if current != STORAGE_VERSION {
        return Err(SLAError::VersionMismatch);
    }

    // Log the upgrade
    let record = UpgradeRecord {
        from_version,
        to_version: STORAGE_VERSION,
        timestamp: env.ledger().timestamp(),
        upgraded_by: caller.clone(),
    };

    let mut log: soroban_sdk::Vec<UpgradeRecord> = env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    log.push_back(record);
    env.storage().instance().set(&UPGRADE_LOG_KEY, &log);

    env.events().publish(
        (EVENT_UPGRADED, EVENT_VERSION, caller),
        (from_version, STORAGE_VERSION),
    );

    Ok(())
}

/// Returns the full upgrade history.
pub fn get_upgrade_log(env: &Env) -> Result<soroban_sdk::Vec<UpgradeRecord>, SLAError> {
    Ok(env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env)))
}

/// Returns the number of upgrades that have been performed.
pub fn get_upgrade_count(env: &Env) -> Result<u32, SLAError> {
    let log: soroban_sdk::Vec<UpgradeRecord> = env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    Ok(log.len())
}

/// Returns the most recent upgrade record, if any.
pub fn get_last_upgrade(env: &Env) -> Result<Option<UpgradeRecord>, SLAError> {
    let log: soroban_sdk::Vec<UpgradeRecord> = env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    if log.is_empty() {
        Ok(None)
    } else {
        Ok(Some(log.get(log.len() - 1).unwrap()))
    }
}

/// Check whether an upgrade is available (stored version < binary version).
pub fn upgrade_available(env: &Env) -> Result<bool, SLAError> {
    let stored: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(0);
    Ok(stored < STORAGE_VERSION)
}

/// Returns the current and expected storage versions for pre-upgrade checks.
pub fn get_version_pair(env: &Env) -> Result<(u32, u32), SLAError> {
    let stored: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .ok_or(SLAError::NotInitialized)?;
    Ok((stored, STORAGE_VERSION))
}
