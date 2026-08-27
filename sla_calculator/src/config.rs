use soroban_sdk::{contracttype, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    RewardOutOfBounds,
}

pub struct ConfigManager;

impl ConfigManager {
    const MIN_REWARD_BASE: u64 = 1;
    const MAX_REWARD_BASE: u64 = 100_000;

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