// Emergency Guard role rotation protocol. Complements whitelist.rs.
use soroban_sdk::{symbol_short, Address, Env, Symbol};

const GUARD_KEY: Symbol = symbol_short!("EMGUARD");
const ROTATED_EVENT: Symbol = symbol_short!("eg_rot");

/// Active admin replaces the current Emergency Guard key immediately,
/// revoking pause authorization from the former key.
pub fn rotate_emergency_guard(
    env: &Env,
    admin: &Address,
    old_guard: &Address,
    new_guard: &Address,
) {
    admin.require_auth();
    let current: Address = env
        .storage()
        .instance()
        .get(&GUARD_KEY)
        .unwrap_or_else(|| old_guard.clone());
    if &current == old_guard {
        env.storage().instance().set(&GUARD_KEY, new_guard);
        env.events()
            .publish((ROTATED_EVENT,), (old_guard.clone(), new_guard.clone()));
    }
}

pub fn current_emergency_guard(env: &Env) -> Option<Address> {
    env.storage().instance().get(&GUARD_KEY)
}
