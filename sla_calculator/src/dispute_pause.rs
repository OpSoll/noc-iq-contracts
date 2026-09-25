// Emergency dispute pause toggle. Complements emergency.rs.
use soroban_sdk::{symbol_short, Address, Env, Symbol};

const DISPUTES_PAUSED_KEY: Symbol = symbol_short!("DPPAUSE");

/// Admin governance toggles pause_disputes to suspend new filings and
/// arbitrator vote submissions during a security incident. Existing
/// dispute deadlines are preserved (callers should not reset them).
pub fn set_disputes_paused(env: &Env, admin: &Address, paused: bool) {
    admin.require_auth();
    env.storage().instance().set(&DISPUTES_PAUSED_KEY, &paused);
}

pub fn are_disputes_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DISPUTES_PAUSED_KEY)
        .unwrap_or(false)
}
