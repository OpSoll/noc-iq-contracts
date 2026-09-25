use soroban_sdk::{symbol_short, Address, Env, Symbol};

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN_KEY: Symbol = symbol_short!("PADMIN");
const ADMIN_RENOUNCED_EVENT: Symbol = symbol_short!("adm_renc");

/// Permanently renounces the admin role, locking admin-gated methods.
/// Caller must be the current admin; authentication is enforced here.
pub fn renounce_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    env.storage().instance().remove(&ADMIN_KEY);
    env.storage().instance().remove(&PENDING_ADMIN_KEY);
    env.events()
        .publish((ADMIN_RENOUNCED_EVENT,), caller.clone());
}
