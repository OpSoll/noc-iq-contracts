#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const WHITELIST_KEY: Symbol = symbol_short!("WL");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_WL_ADD: Symbol = symbol_short!("wl_add");
const EVENT_WL_REM: Symbol = symbol_short!("wl_rem");
const EVENT_WL_CLR: Symbol = symbol_short!("wl_clr");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Cross-contract whitelist state.
#[soroban_sdk::contracttype]
pub struct WhitelistState {
    /// Set of whitelisted contract addresses.
    pub contracts: soroban_sdk::Map<Address, bool>,
    /// Number of whitelisted contracts.
    pub count: u32,
    /// Whether whitelist checking is enabled.
    /// When disabled, all contracts are allowed.
    pub enforcement_enabled: bool,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize the whitelist as empty with enforcement disabled.
pub fn init_whitelist(env: &Env) {
    env.storage().instance().set(
        &WHITELIST_KEY,
        &WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        },
    );
}

/// Add a contract address to the whitelist (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `contract_address`: Address to whitelist.
///
/// # Events
/// - `wl_add`: Emitted with the whitelisted address.
pub fn add_to_whitelist(
    env: &Env,
    caller: &Address,
    contract_address: &Address,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState = env
        .storage()
        .instance()
        .get(&WHITELIST_KEY)
        .unwrap_or(WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        });

    if state.contracts.get(contract_address.clone()).unwrap_or(false) {
        return Ok(()); // Already whitelisted (idempotent)
    }

    state.contracts.set(contract_address.clone(), true);
    state.count = state.count.saturating_add(1);
    env.storage().instance().set(&WHITELIST_KEY, &state);

    env.events().publish(
        (EVENT_WL_ADD, EVENT_VERSION, caller),
        (contract_address.clone(),),
    );

    Ok(())
}

/// Remove a contract address from the whitelist (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `contract_address`: Address to remove.
///
/// # Events
/// - `wl_rem`: Emitted with the removed address.
pub fn remove_from_whitelist(
    env: &Env,
    caller: &Address,
    contract_address: &Address,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState = env
        .storage()
        .instance()
        .get(&WHITELIST_KEY)
        .unwrap_or(WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        });

    if !state.contracts.get(contract_address.clone()).unwrap_or(false) {
        return Ok(()); // Not in whitelist (idempotent)
    }

    state.contracts.set(contract_address.clone(), false);
    state.count = state.count.saturating_sub(1);
    env.storage().instance().set(&WHITELIST_KEY, &state);

    env.events().publish(
        (EVENT_WL_REM, EVENT_VERSION, caller),
        (contract_address.clone(),),
    );

    Ok(())
}

/// Clear the entire whitelist (admin only).
///
/// # Events
/// - `wl_clr`: Emitted when whitelist is cleared.
pub fn clear_whitelist(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    env.storage().instance().set(
        &WHITELIST_KEY,
        &WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        },
    );

    env.events()
        .publish((EVENT_WL_CLR, EVENT_VERSION, caller), ());

    Ok(())
}

/// Enable whitelist enforcement (admin only).
///
/// When enabled, only whitelisted contracts can interact with this contract.
pub fn enable_whitelist(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState = env
        .storage()
        .instance()
        .get(&WHITELIST_KEY)
        .unwrap_or(WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        });

    state.enforcement_enabled = true;
    env.storage().instance().set(&WHITELIST_KEY, &state);

    Ok(())
}

/// Disable whitelist enforcement (admin only).
///
/// When disabled, all contracts are allowed.
pub fn disable_whitelist(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState = env
        .storage()
        .instance()
        .get(&WHITELIST_KEY)
        .unwrap_or(WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        });

    state.enforcement_enabled = false;
    env.storage().instance().set(&WHITELIST_KEY, &state);

    Ok(())
}

/// Check if a contract address is whitelisted (or if enforcement is disabled).
///
/// Returns Ok(()) if allowed, or Unauthorized if enforcement is on and
/// the contract is not in the whitelist.
pub fn check_whitelist(env: &Env, contract_address: &Address) -> Result<(), SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok(()), // No whitelist = allow all
    };

    if !state.enforcement_enabled {
        return Ok(()); // Enforcement off = allow all
    }

    if state.contracts.get(contract_address.clone()).unwrap_or(false) {
        Ok(())
    } else {
        Err(SLAError::Unauthorized)
    }
}

/// Check if a contract address is in the whitelist (without enforcement check).
pub fn is_whitelisted(env: &Env, contract_address: &Address) -> Result<bool, SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok(false),
    };

    Ok(state.contracts.get(contract_address.clone()).unwrap_or(false))
}

/// Returns the whitelist status and count.
pub fn get_whitelist_status(env: &Env) -> Result<(bool, u32), SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok((false, 0)),
    };

    Ok((state.enforcement_enabled, state.count))
}

/// Returns all whitelisted contract addresses.
pub fn list_whitelisted(env: &Env) -> Result<soroban_sdk::Vec<Address>, SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok(soroban_sdk::Vec::new(env)),
    };

    let mut result = soroban_sdk::Vec::new(env);
    for (addr, active) in state.contracts.iter() {
        if active {
            result.push_back(addr);
        }
    }
    Ok(result)
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
