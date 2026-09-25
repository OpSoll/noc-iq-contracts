// Parameter change timelock delay. Complements config_freeze.rs.
use soroban_sdk::{contracttype, Address, Env};

const FORTY_EIGHT_HOURS_SECS: u64 = 48 * 60 * 60;

#[contracttype]
pub struct PendingParamChange {
    pub proposer: Address,
    pub proposed_at: u64,
    pub param_key: soroban_sdk::Symbol,
    pub new_value: i128,
}

/// Admin proposes a parameter change; it enters PENDING_TIMELOCK and only
/// takes effect once `is_timelock_elapsed` returns true (48 hours later).
pub fn propose_param_change(
    env: &Env,
    admin: &Address,
    param_key: soroban_sdk::Symbol,
    new_value: i128,
) -> PendingParamChange {
    admin.require_auth();
    PendingParamChange {
        proposer: admin.clone(),
        proposed_at: env.ledger().timestamp(),
        param_key,
        new_value,
    }
}

pub fn is_timelock_elapsed(env: &Env, change: &PendingParamChange) -> bool {
    env.ledger().timestamp() >= change.proposed_at + FORTY_EIGHT_HOURS_SECS
}
