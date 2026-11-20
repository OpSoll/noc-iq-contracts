use soroban_sdk::{contracttype, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    RewardOutOfBounds,
    MetadataTooLong,
    InvalidVersion,
}

pub const MAX_METADATA_LENGTH: u32 = 64;

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

#[cfg(test)]
mod metadata_validation_tests {
    use super::*;

    #[test]
    fn metadata_length_boundary_is_inclusive() {
        assert!(ConfigManager::is_valid_metadata_length(MAX_METADATA_LENGTH));
        assert!(ConfigManager::is_valid_metadata_length(MAX_METADATA_LENGTH - 1));
        assert!(!ConfigManager::is_valid_metadata_length(MAX_METADATA_LENGTH + 1));
        assert!(ConfigManager::is_valid_metadata_length(0));
    }

    #[test]
    fn version_encoding_accepts_only_a_v_prefixed_integer() {
        for valid in ["v0", "v1", "v12", "v999999999"] {
            assert!(ConfigManager::is_valid_version_encoding(valid), "{valid}");
        }
        for invalid in ["", "v", "1", "V1", "v1.0", "version1", "v01", "v1a"] {
            assert!(!ConfigManager::is_valid_version_encoding(invalid), "{invalid}");
        }
    }
}