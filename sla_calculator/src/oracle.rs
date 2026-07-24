#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const ORACLE_KEY: Symbol = symbol_short!("ORCL");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_ORACLE_SET: Symbol = symbol_short!("orc_set");
const EVENT_ORACLE_DATA: Symbol = symbol_short!("orc_dat");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Oracle configuration for external data feeds.
#[soroban_sdk::contracttype]
pub struct OracleConfig {
    /// Address of the oracle contract.
    pub oracle_address: Address,
    /// Whether oracle integration is enabled.
    pub enabled: bool,
    /// Maximum acceptable staleness in seconds.
    pub max_staleness: u64,
    /// Required number of confirmations.
    pub required_confirmations: u32,
}

/// Oracle data point received from external feed.
#[soroban_sdk::contracttype]
pub struct OracleDataPoint {
    /// Identifier for the data point.
    pub data_id: Symbol,
    /// The value reported by the oracle.
    pub value: i128,
    /// Timestamp of the data point.
    pub timestamp: u64,
    /// Number of confirmations received.
    pub confirmations: u32,
    /// Source identifier.
    pub source: Symbol,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize oracle integration as disabled.
pub fn init_oracle(env: &Env) {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .unwrap_or_else(|| Address::generate(env));

    env.storage().instance().set(
        &ORACLE_KEY,
        &OracleConfig {
            oracle_address: admin,
            enabled: false,
            max_staleness: 300,
            required_confirmations: 1,
        },
    );
}

/// Configure oracle integration (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `oracle_address`: Address of the oracle contract.
/// - `max_staleness`: Maximum acceptable data age in seconds.
/// - `required_confirmations`: Required confirmations for data validity.
///
/// # Events
/// - `orc_set`: Emitted with the new oracle configuration.
pub fn set_oracle(
    env: &Env,
    caller: &Address,
    oracle_address: Address,
    max_staleness: u64,
    required_confirmations: u32,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    env.storage().instance().set(
        &ORACLE_KEY,
        &OracleConfig {
            oracle_address,
            enabled: true,
            max_staleness,
            required_confirmations,
        },
    );

    env.events().publish(
        (EVENT_ORACLE_SET, EVENT_VERSION, caller),
        (max_staleness, required_confirmations),
    );

    Ok(())
}

/// Disable oracle integration (admin only).
pub fn disable_oracle(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut config: OracleConfig = env
        .storage()
        .instance()
        .get(&ORACLE_KEY)
        .ok_or(SLAError::NotInitialized)?;

    config.enabled = false;
    env.storage().instance().set(&ORACLE_KEY, &config);

    Ok(())
}

/// Validate an oracle data point against freshness and confirmation requirements.
///
/// # Arguments
/// - `data_point`: The oracle data point to validate.
///
/// # Returns
/// Ok(()) if valid, or error describing why invalid.
pub fn validate_oracle_data(
    env: &Env,
    data_point: &OracleDataPoint,
) -> Result<(), SLAError> {
    let config: OracleConfig = match env.storage().instance().get(&ORACLE_KEY) {
        Some(c) => c,
        None => return Ok(()), // No oracle config = skip validation
    };

    if !config.enabled {
        return Ok(()); // Oracle disabled = skip validation
    }

    let now = env.ledger().timestamp();

    // Check staleness
    if now.saturating_sub(data_point.timestamp) > config.max_staleness {
        return Err(SLAError::InvalidThreshold);
    }

    // Check confirmations
    if data_point.confirmations < config.required_confirmations {
        return Err(SLAError::InvalidThreshold);
    }

    Ok(())
}

/// Record oracle data for audit trail.
pub fn record_oracle_data(
    env: &Env,
    data_point: &OracleDataPoint,
) {
    env.events().publish(
        (EVENT_ORACLE_DATA, EVENT_VERSION, data_point.source.clone()),
        (
            data_point.data_id.clone(),
            data_point.value,
            data_point.timestamp,
            data_point.confirmations,
        ),
    );
}

/// Get oracle configuration.
pub fn get_oracle_config(
    env: &Env,
) -> Result<Option<OracleConfig>, SLAError> {
    Ok(env.storage().instance().get(&ORACLE_KEY))
}

/// Check if oracle integration is enabled.
pub fn is_oracle_enabled(env: &Env) -> Result<bool, SLAError> {
    let config: OracleConfig = match env.storage().instance().get(&ORACLE_KEY) {
        Some(c) => c,
        None => return Ok(false),
    };
    Ok(config.enabled)
}

/// Helper to verify admin role.
fn require_admin(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }
    Ok(())
}
