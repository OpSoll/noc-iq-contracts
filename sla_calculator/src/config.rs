use soroban_sdk::{contracttype, Env, Map, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    RewardOutOfBounds,
    MetadataTooLong,
    InvalidVersion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigValidationError {
    DuplicateSeverity(Symbol),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageWriteError {
    InjectedFault,
}

pub struct ConfigManager;

impl ConfigManager {
    const MIN_REWARD_BASE: u64 = 1;
    const MAX_REWARD_BASE: u64 = 100_000;

    pub fn is_valid_metadata_length(length: u32) -> bool {
        length <= MAX_METADATA_LENGTH
    }

    pub fn is_valid_version_encoding(version: &str) -> bool {
        let bytes = version.as_bytes();
        if bytes.len() < 2 || bytes.len() > 10 {
            return false;
        }
        if bytes[0] != b'v' || !bytes[1..].iter().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        bytes.len() == 2 || bytes[1] != b'0'
    }

    /// Sets and validates the reward base configuration parameter ensuring it falls
    /// strictly within the 1 to 100,000 allowable range.
    pub fn set_reward_base(env: &Env, reward_base: u64) -> Result<(), ConfigError> {
        if reward_base < Self::MIN_REWARD_BASE || reward_base > Self::MAX_REWARD_BASE {
            return Err(ConfigError::RewardOutOfBounds);
        }

        // Configuration storage logic...
        Ok(())
    }
}

pub fn validate_unique_severities(
    env: &Env,
    severities: &Vec<Symbol>,
) -> Result<(), ConfigValidationError> {
    let mut seen = Map::<Symbol, bool>::new(env);
    for severity in severities.iter() {
        if seen.contains_key(severity.clone()) {
            return Err(ConfigValidationError::DuplicateSeverity(severity));
        }
        seen.set(severity, true);
    }
    Ok(())
}

pub fn apply_atomic_writes<F>(
    env: &Env,
    writes: &[(Symbol, u64)],
    mut write: F,
) -> Result<(), StorageWriteError>
where
    F: FnMut(&Env, Symbol, u64) -> Result<(), StorageWriteError>,
{
    let mut previous = Map::<Symbol, u64>::new(env);
    let mut absent = Vec::<Symbol>::new(env);
    for (key, _) in writes.iter() {
        match env.storage().instance().get::<Symbol, u64>(key) {
            Some(value) => previous.set(key.clone(), value),
            None => absent.push_back(key.clone()),
        }
    }

    for (key, value) in writes.iter() {
        if let Err(err) = write(env, key.clone(), *value) {
            for (old_key, old_value) in previous.iter() {
                env.storage().instance().set(&old_key, &old_value);
            }
            for old_key in absent.iter() {
                env.storage().instance().remove(&old_key);
            }
            return Err(err);
        }
    }
    Ok(())
}

pub fn config_version_hash(env: &Env, configs: &Map<Symbol, crate::SLAConfig>) -> u64 {
    let digest = crate::compute_config_version_hash(env, configs).to_array();
    let mut hash = 0u64;
    for byte in digest.iter().take(8) {
        hash = (hash << 8) | (*byte as u64);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::symbol_short;

    #[test]
    fn duplicate_severity_is_reported_with_the_duplicate_key() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let severities = soroban_sdk::vec![&env, symbol_short!("high"), symbol_short!("high")];
            assert_eq!(
                validate_unique_severities(&env, &severities),
                Err(ConfigValidationError::DuplicateSeverity(symbol_short!(
                    "high"
                )))
            );
        });
    }

    #[test]
    fn failed_later_write_restores_every_previous_value() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let first = symbol_short!("valone");
            let second = symbol_short!("valtwo");
            let third = symbol_short!("valthree");
            env.storage().instance().set(&first, &1u64);
            env.storage().instance().set(&second, &2u64);

            let writes = [
                (first.clone(), 10u64),
                (second.clone(), 20u64),
                (third.clone(), 30u64),
            ];
            let mut calls = 0usize;
            let result = apply_atomic_writes(&env, &writes, |env, key, value| {
                if calls == 2 {
                    calls += 1;
                    return Err(StorageWriteError::InjectedFault);
                }
                calls += 1;
                env.storage().instance().set(&key, &value);
                Ok(())
            });

            assert_eq!(result, Err(StorageWriteError::InjectedFault));
            assert_eq!(env.storage().instance().get::<Symbol, u64>(&first), Some(1));
            assert_eq!(env.storage().instance().get::<Symbol, u64>(&second), Some(2));
            assert!(!env.storage().instance().has(&third));
        });
    }
}
