use crate::config::{ConfigError, ConfigManager};
use soroban_sdk::{contracttype, symbol_short, Env, String, Symbol};

pub type MetadataError = ConfigError;

const LAST_CFG_UPDATE_KEY: Symbol = symbol_short!("LCFGUPD");
const METADATA_KEY: Symbol = symbol_short!("CFGMETA");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredConfigMetadata {
    pub version: String,
    pub metadata: String,
}

pub fn record_config_update(env: &Env) {
    let ledger = env.ledger().sequence();
    env.storage()
        .instance()
        .set(&LAST_CFG_UPDATE_KEY, &ledger);
}

pub fn get_last_config_update(env: &Env) -> Option<u32> {
    env.storage().instance().get(&LAST_CFG_UPDATE_KEY)
}

fn validate_version(version: &String) -> Result<(), ConfigError> {
    let length = version.len() as usize;
    if length > 10 {
        return Err(ConfigError::InvalidVersion);
    }
    let mut buffer = [0u8; 10];
    version.copy_into_slice(&mut buffer);
    let version = core::str::from_utf8(&buffer[..length]).map_err(|_| ConfigError::InvalidVersion)?;
    if ConfigManager::is_valid_version_encoding(version) {
        Ok(())
    } else {
        Err(ConfigError::InvalidVersion)
    }
}

pub fn set_config_metadata(
    env: &Env,
    version: &String,
    metadata: &String,
) -> Result<StoredConfigMetadata, ConfigError> {
    if !ConfigManager::is_valid_metadata_length(metadata.len()) {
        return Err(ConfigError::MetadataTooLong);
    }
    validate_version(version)?;

    let stored = StoredConfigMetadata {
        version: version.clone(),
        metadata: metadata.clone(),
    };
    env.storage().instance().set(&METADATA_KEY, &stored);
    Ok(stored)
}

pub fn get_config_metadata(env: &Env) -> Option<StoredConfigMetadata> {
    env.storage().instance().get(&METADATA_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_last_config_update_unset() {
        let env = Env::default();
        assert_eq!(get_last_config_update(&env), None);
    }

    #[test]
    fn test_record_and_read_config_update() {
        let env = Env::default();
        record_config_update(&env);
        let ledger = get_last_config_update(&env);
        assert!(ledger.is_some());
    }

    #[test]
    fn metadata_length_boundary_is_enforced() {
        let env = Env::default();
        let version = String::from_str(&env, "v1");
        let at_limit = String::from_bytes(&env, &[b'x'; 64]);
        let over_limit = String::from_bytes(&env, &[b'x'; 65]);

        assert_eq!(
            set_config_metadata(&env, &version, &at_limit).map(|stored| stored.metadata),
            Ok(at_limit)
        );
        assert_eq!(
            set_config_metadata(&env, &version, &over_limit),
            Err(ConfigError::MetadataTooLong)
        );
    }

    #[test]
    fn invalid_version_encoding_is_rejected() {
        let env = Env::default();
        let metadata = String::from_str(&env, "notes");
        for invalid in ["", "v", "1", "V1", "v1.0", "v01", "version1"] {
            let version = String::from_str(&env, invalid);
            assert_eq!(
                set_config_metadata(&env, &version, &metadata),
                Err(ConfigError::InvalidVersion)
            );
        }
    }

    #[test]
    fn rejected_metadata_leaves_current_metadata_unchanged() {
        let env = Env::default();
        let version = String::from_str(&env, "v1");
        let metadata = String::from_str(&env, "original");
        set_config_metadata(&env, &version, &metadata).unwrap();

        let too_long = String::from_bytes(&env, &[b'y'; 65]);
        let bad_version = String::from_str(&env, "v1.2.3");
        assert_eq!(
            set_config_metadata(&env, &version, &too_long),
            Err(ConfigError::MetadataTooLong)
        );
        assert_eq!(
            set_config_metadata(&env, &bad_version, &metadata),
            Err(ConfigError::InvalidVersion)
        );

        assert_eq!(
            get_config_metadata(&env),
            Some(StoredConfigMetadata { version, metadata })
        );
    }
}
