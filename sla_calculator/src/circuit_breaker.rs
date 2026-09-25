// Global circuit breaker pause toggle. Complements emergency.rs.
use soroban_sdk::{symbol_short, Address, Env, Symbol};

const CIRCUIT_PAUSED_KEY: Symbol = symbol_short!("CBPAUSE");

/// Emergency Guard freezes all state mutations (outage reporting,
/// penalty payouts, dispute filings). Read-only queries are unaffected.
pub fn set_emergency_pause(env: &Env, emergency_guard: &Address, paused: bool) {
    emergency_guard.require_auth();
    env.storage().instance().set(&CIRCUIT_PAUSED_KEY, &paused);
}

pub fn is_emergency_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&CIRCUIT_PAUSED_KEY)
        .unwrap_or(false)
}
