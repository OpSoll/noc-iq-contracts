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

/// Returns the seconds remaining before the pending change activates.
/// The result is zero once the 48-hour timelock has elapsed.
pub fn remaining_timelock(env: &Env, change: &PendingParamChange) -> u64 {
    let ready_at = change.proposed_at + FORTY_EIGHT_HOURS_SECS;
    ready_at.saturating_sub(env.ledger().timestamp())
}

pub fn is_timelock_elapsed(env: &Env, change: &PendingParamChange) -> bool {
    env.ledger().timestamp() >= change.proposed_at + FORTY_EIGHT_HOURS_SECS
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger as _};

    fn pending(env: &Env, proposed_at: u64) -> PendingParamChange {
        PendingParamChange {
            proposer: Address::generate(env),
            proposed_at,
            param_key: soroban_sdk::Symbol::new(env, "sla_target"),
            new_value: 9_000,
        }
    }

    #[test]
    fn remaining_time_counts_down_to_expiry() {
        let env = Env::default();
        let change = pending(&env, 1_000);

        env.ledger().set_timestamp(1_000);
        assert_eq!(remaining_timelock(&env, &change), FORTY_EIGHT_HOURS_SECS);

        env.ledger().set_timestamp(1_000 + FORTY_EIGHT_HOURS_SECS);
        assert_eq!(remaining_timelock(&env, &change), 0);

        env.ledger().set_timestamp(1_000 + FORTY_EIGHT_HOURS_SECS + 10_000);
        assert_eq!(remaining_timelock(&env, &change), 0);
    }
}
