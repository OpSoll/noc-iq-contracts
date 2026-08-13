use soroban_sdk::{Env, IntoVal, Val};

// Assuming ~5 seconds per ledger, 1 day = 17,280 ledgers
pub const DAY_IN_LEDGERS: u32 = 17280;

// Bump amounts and thresholds for TTL extensions
pub const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub const INSTANCE_LIFETIME_THRESHOLD: u32 = 14 * DAY_IN_LEDGERS;

pub const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = 14 * DAY_IN_LEDGERS;

/// Automatically extends the TTL of the instance storage using default thresholds.
pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

/// Automatically extends the TTL of a persistent storage key using default thresholds.
pub fn extend_persistent_ttl<K>(env: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    env.storage().persistent().extend_ttl(
        key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{symbol_short, Env};

    #[test]
    fn test_extend_instance_ttl() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            // Just calling it to ensure it doesn't panic in test environment
            extend_instance_ttl(&env);
        });
    }

    #[test]
    fn test_extend_persistent_ttl() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            let key = symbol_short!("test");
            env.storage().persistent().set(&key, &1u32);
            // Just calling it to ensure it doesn't panic in test environment
            extend_persistent_ttl(&env, &key);
        });
    }
}
