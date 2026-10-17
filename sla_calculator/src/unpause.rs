use soroban_sdk::{symbol_short, Address, Env, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const PAUSE_INFO_KEY: Symbol = symbol_short!("PAUSEINF");
const CONTRACT_UNPAUSED_EVENT: Symbol = symbol_short!("unpaused");

/// Admin-only: clears the paused flag and pause metadata.
pub fn unpause(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&PAUSED_KEY, &false);
    env.storage().instance().remove(&PAUSE_INFO_KEY);
    env.events()
        .publish((CONTRACT_UNPAUSED_EVENT,), admin.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        (env, id, admin)
    }

    #[test]
    fn test_unpause_clears_flag_and_pause_info() {
        let (env, id, admin) = setup();
        env.as_contract(&id, || {
            env.storage().instance().set(&PAUSED_KEY, &true);
            env.storage().instance().set(&PAUSE_INFO_KEY, &7u32);
            unpause(&env, &admin);
            assert_eq!(env.storage().instance().get::<Symbol, bool>(&PAUSED_KEY), Some(false));
            assert!(!env.storage().instance().has(&PAUSE_INFO_KEY));
        });
    }

    #[test]
    fn test_unpause_is_idempotent_when_not_paused() {
        let (env, id, admin) = setup();
        env.as_contract(&id, || {
            unpause(&env, &admin);
            unpause(&env, &admin);
            assert_eq!(env.storage().instance().get::<Symbol, bool>(&PAUSED_KEY), Some(false));
        });
    }

    #[test]
    #[should_panic]
    fn test_unpause_without_auth_panics() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        env.as_contract(&id, || unpause(&env, &admin));
    }
}
