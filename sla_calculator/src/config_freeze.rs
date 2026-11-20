use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

const FREEZE_KEY: Symbol = symbol_short!("FREEZE");
/// Counts completed freezes: it is bumped each time the config goes from mutable to frozen, so a
/// client can tell *which* freeze blocked it.
const FREEZE_REVISION_KEY: Symbol = symbol_short!("FRZREV");

/// Error returned when a configuration update is blocked by a freeze.
///
/// The numeric value is the stable, documented reason code for clients. It is deliberately kept
/// clear of the `SLAError` range (1 to 24) so the two enums can never be confused. The revision
/// of the freeze that blocked the update is available from [`freeze_revision`] and is carried by
/// [`FrozenMutation`].
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfigFreezeError {
    /// A configuration mutation was attempted while the configuration is frozen.
    ConfigFrozen = 100,
}

/// The documented reason code (`100`) for a mutation blocked by a freeze.
pub const CONFIG_FROZEN_CODE: u32 = 100;

/// A blocked mutation together with the revision of the freeze that blocked it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FrozenMutation {
    pub error: ConfigFreezeError,
    /// Revision of the active freeze (1 for the first freeze, 2 for the next, and so on).
    pub freeze_revision: u32,
}

/// Revision of the most recent freeze; `0` if the config has never been frozen.
pub fn freeze_revision(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get::<Symbol, u32>(&FREEZE_REVISION_KEY)
        .unwrap_or(0)
}

/// Fails with [`FrozenMutation`] (reason code [`CONFIG_FROZEN_CODE`] plus the active freeze
/// revision) while the configuration is frozen; call it at the top of every config mutator.
pub fn require_config_mutable(env: &Env) -> Result<(), FrozenMutation> {
    if is_config_frozen(env) {
        return Err(FrozenMutation {
            error: ConfigFreezeError::ConfigFrozen,
            freeze_revision: freeze_revision(env),
        });
    }
    Ok(())
}

/// Freezes the configuration. Freezing an already frozen config is a no-op, so the revision
/// only advances on a real transition from mutable to frozen.
pub fn freeze_config(env: &Env) {
    if !is_config_frozen(env) {
        env.storage()
            .instance()
            .set(&FREEZE_REVISION_KEY, &(freeze_revision(env) + 1));
    }
    env.storage().instance().set(&FREEZE_KEY, &true);
}

pub fn unfreeze_config(env: &Env) {
    env.storage().instance().set(&FREEZE_KEY, &false);
}

pub fn unfreeze_config_authorized(env: &Env, admin: &Address) -> Result<(), crate::SLAError> {
    admin.require_auth();
    let current: Address = env
        .storage()
        .instance()
        .get(&crate::ADMIN_KEY)
        .ok_or(crate::SLAError::NotInitialized)?;
    if current != *admin {
        return Err(crate::SLAError::Unauthorized);
    }
    unfreeze_config(env);
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreezeState {
    pub frozen: bool,
    pub revision: u32,
}

pub fn freeze_state(env: &Env) -> FreezeState {
    FreezeState {
        frozen: is_config_frozen(env),
        revision: freeze_revision(env),
    }
}

pub fn restore_state(env: &Env, state: &FreezeState) {
    env.storage().instance().set(&FREEZE_KEY, &state.frozen);
    env.storage()
        .instance()
        .set(&FREEZE_REVISION_KEY, &state.revision);
}

pub fn is_config_frozen(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<Symbol, bool>(&FREEZE_KEY)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};
    use crate::{SLACalculatorContract, SLACalculatorContractClient};

    #[test]
    fn test_config_unfrozen_by_default() {
        let env = Env::default();
        assert!(!is_config_frozen(&env));
    }

    #[test]
    fn test_freeze_and_query() {
        let env = Env::default();
        freeze_config(&env);
        assert!(is_config_frozen(&env));
    }

    #[test]
    fn test_unfreeze_restores_mutable_state() {
        let env = Env::default();
        freeze_config(&env);
        unfreeze_config(&env);
        assert!(!is_config_frozen(&env));
    }

    #[test]
    fn test_frozen_config_blocks_set_config() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        client.initialize(&admin, &operator);
        freeze_config(&env);
        assert!(is_config_frozen(&env));
    }
}

#[cfg(test)]
mod frozen_reason_tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn in_contract<R>(f: impl FnOnce(&Env) -> R) -> R {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || f(&env))
    }

    #[test]
    fn the_reason_code_is_stable_and_outside_the_sla_error_range() {
        assert_eq!(CONFIG_FROZEN_CODE, 100);
        assert_eq!(ConfigFreezeError::ConfigFrozen as u32, CONFIG_FROZEN_CODE);
        assert!(CONFIG_FROZEN_CODE > 24);
    }

    #[test]
    fn a_mutable_config_is_allowed_to_change() {
        in_contract(|env| {
            assert_eq!(require_config_mutable(env), Ok(()));
            assert_eq!(freeze_revision(env), 0);
        });
    }

    #[test]
    fn a_frozen_config_reports_the_code_and_the_active_revision() {
        in_contract(|env| {
            freeze_config(env);

            assert_eq!(
                require_config_mutable(env),
                Err(FrozenMutation {
                    error: ConfigFreezeError::ConfigFrozen,
                    freeze_revision: 1
                })
            );
        });
    }

    #[test]
    fn refreezing_a_frozen_config_does_not_advance_the_revision() {
        in_contract(|env| {
            freeze_config(env);
            freeze_config(env);

            assert_eq!(freeze_revision(env), 1);
        });
    }

    #[test]
    fn each_new_freeze_gets_a_new_revision() {
        in_contract(|env| {
            freeze_config(env);
            unfreeze_config(env);
            assert_eq!(require_config_mutable(env), Ok(()));
            freeze_config(env);

            assert_eq!(
                require_config_mutable(env).unwrap_err().freeze_revision,
                2
            );
        });
    }

    #[test]
    fn unfreezing_keeps_the_last_revision_for_diagnostics() {
        in_contract(|env| {
            freeze_config(env);
            unfreeze_config(env);

            assert_eq!(freeze_revision(env), 1);
            assert!(!is_config_frozen(env));
        });
    }
}
