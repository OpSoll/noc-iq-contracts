use soroban_sdk::{symbol_short, Env, Symbol};

const LAST_CFG_UPDATE_KEY: Symbol = symbol_short!("LCFGUPD");

pub fn record_config_update(env: &Env) {
    let ledger = env.ledger().sequence();
    env.storage()
        .instance()
        .set(&LAST_CFG_UPDATE_KEY, &ledger);
}

pub fn get_last_config_update(env: &Env) -> Option<u32> {
    env.storage().instance().get(&LAST_CFG_UPDATE_KEY)
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
}
