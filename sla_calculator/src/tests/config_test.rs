#[cfg(test)]
mod config_validation_tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_valid_reward_base_boundaries() {
        let env = Env::default();

        // Lower boundary
        assert!(ConfigManager::set_reward_base(&env, 1).is_ok());
        // Upper boundary
        assert!(ConfigManager::set_reward_base(&env, 100_000).is_ok());
        // Mid value
        assert!(ConfigManager::set_reward_base(&env, 50_000).is_ok());
    }

    #[test]
    fn test_reward_base_out_of_bounds() {
        let env = Env::default();

        // Zero boundary violation
        assert_eq!(
            ConfigManager::set_reward_base(&env, 0),
            Err(ConfigError::RewardOutOfBounds)
        );

        // Above maximum boundary violation
        assert_eq!(
            ConfigManager::set_reward_base(&env, 100_001),
            Err(ConfigError::RewardOutOfBounds)
        );
    }
}