use soroban_sdk::{symbol_short, Env, Symbol};

const STORAGE_VERSION_KEY: Symbol = symbol_short!("VER");
const MIGRATION_FLAG_KEY: Symbol = symbol_short!("MIGRATED");

pub fn read_storage_version(env: &Env) -> Option<u32> {
    env.storage().instance().get(&STORAGE_VERSION_KEY)
}

pub fn is_migration_complete(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<Symbol, bool>(&MIGRATION_FLAG_KEY)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_storage_version_unset_returns_none() {
        let env = Env::default();
        assert_eq!(read_storage_version(&env), None);
    }

    #[test]
    fn test_migration_flag_defaults_to_false() {
        let env = Env::default();
        assert!(!is_migration_complete(&env));
    }
}
