use soroban_sdk::{Env, IntoVal, TryFromVal, Val};

// Assuming ~5 seconds per ledger, 1 day = 17,280 ledgers
pub const DAY_IN_LEDGERS: u32 = 17280;

// Bump amounts and thresholds for TTL extensions
pub const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub const INSTANCE_LIFETIME_THRESHOLD: u32 = 14 * DAY_IN_LEDGERS;

pub const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = 14 * DAY_IN_LEDGERS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageReadError {
    CorruptValue,
}

pub fn read_or_default<K, V>(env: &Env, key: &K, default: V) -> Result<V, StorageReadError>
where
    K: IntoVal<Env, Val>,
    V: TryFromVal<Env, Val>,
{
    match env.storage().instance().get::<K, Val>(key) {
        None => Ok(default),
        Some(raw) => V::try_from_val(env, &raw).map_err(|_| StorageReadError::CorruptValue),
    }
}

pub fn read_persistent_or_default<K, V>(
    env: &Env,
    key: &K,
    default: V,
) -> Result<V, StorageReadError>
where
    K: IntoVal<Env, Val>,
    V: TryFromVal<Env, Val>,
{
    match env.storage().persistent().get::<K, Val>(key) {
        None => Ok(default),
        Some(raw) => V::try_from_val(env, &raw).map_err(|_| StorageReadError::CorruptValue),
    }
}

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

    #[test]
    fn a_missing_key_returns_the_documented_default() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            let key = symbol_short!("miss");
            assert_eq!(read_or_default::<_, u32>(&env, &key, 42), Ok(42));
            assert_eq!(
                read_persistent_or_default::<_, u32>(&env, &key, 7),
                Ok(7)
            );
        });
    }

    #[test]
    fn a_malformed_value_returns_the_typed_corruption_error() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            let key = symbol_short!("bad");
            let raw = symbol_short!("nope");
            env.storage().instance().set(&key, &raw);
            env.storage().persistent().set(&key, &raw);

            assert_eq!(
                read_or_default::<_, u32>(&env, &key, 42),
                Err(StorageReadError::CorruptValue)
            );
            assert_eq!(
                read_persistent_or_default::<_, u32>(&env, &key, 7),
                Err(StorageReadError::CorruptValue)
            );
        });
    }

    #[test]
    fn the_corruption_error_path_does_not_overwrite_the_stored_value() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            let key = symbol_short!("bad");
            let raw = symbol_short!("nope");
            env.storage().instance().set(&key, &raw);

            assert_eq!(
                read_or_default::<_, u32>(&env, &key, 42),
                Err(StorageReadError::CorruptValue)
            );
            let after: soroban_sdk::Symbol = env.storage().instance().get(&key).unwrap();
            assert_eq!(after, raw);
        });
    }
}
