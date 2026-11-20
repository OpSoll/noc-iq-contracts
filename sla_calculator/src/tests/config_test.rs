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

#[cfg(test)]
mod config_digest_tests {
    use crate::config_version_hash::{compute_config_digest, compute_config_map_digest};
    use soroban_sdk::{symbol_short, Env, Map, Symbol};

    fn config_map(env: &Env, entries: &[(Symbol, u32)]) -> Map<Symbol, u32> {
        let mut configs = Map::new(env);
        for (severity, value) in entries {
            configs.set(severity.clone(), *value);
        }
        configs
    }

    #[test]
    fn equivalent_maps_digest_identically_regardless_of_insertion_order() {
        let env = Env::default();
        let first = config_map(
            &env,
            &[(symbol_short!("crit"), 15), (symbol_short!("high"), 30)],
        );
        let second = config_map(
            &env,
            &[(symbol_short!("high"), 30), (symbol_short!("crit"), 15)],
        );

        assert_eq!(
            compute_config_map_digest(&env, &first, 1),
            compute_config_map_digest(&env, &second, 1)
        );
        assert_eq!(
            compute_config_digest(&env, &first, 1),
            compute_config_digest(&env, &second, 1)
        );
    }

    #[test]
    fn digest_tracks_values_and_schema_version() {
        let env = Env::default();
        let original = config_map(&env, &[(symbol_short!("crit"), 15)]);
        let changed = config_map(&env, &[(symbol_short!("crit"), 16)]);

        assert_ne!(
            compute_config_digest(&env, &original, 1),
            compute_config_digest(&env, &changed, 1)
        );
        assert_ne!(
            compute_config_digest(&env, &original, 1),
            compute_config_digest(&env, &original, 2)
        );
    }
}