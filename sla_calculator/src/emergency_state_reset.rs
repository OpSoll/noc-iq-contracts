// Emergency state reset to clean baseline. Complements emergency.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StateResetError {
    InsufficientGuardianSignatures = 1,
    OutOfResetScope = 2,
}

const RESET_EVENT: Symbol = symbol_short!("statersr");

/// Keys outside the reset scope: the emergency guard and whitelist state.
fn is_protected_key(key: &Symbol) -> bool {
    *key == symbol_short!("EMGUARD") || *key == symbol_short!("WLIST")
}

/// Requires 4-of-5 admin guardian signatures to clear corrupted
/// temporary index storage, preserving whitelist and vault balances.
pub fn emergency_state_reset(
    env: &Env,
    guardians: &Vec<Address>,
    temp_index_key: &Symbol,
) -> Result<(), StateResetError> {
    if guardians.len() < 4 {
        return Err(StateResetError::InsufficientGuardianSignatures);
    }
    if is_protected_key(temp_index_key) {
        return Err(StateResetError::OutOfResetScope);
    }
    for guardian in guardians.iter() {
        guardian.require_auth();
    }
    env.storage().instance().remove(temp_index_key);
    env.events().publish((RESET_EVENT,), guardians.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    fn guardians(env: &Env, count: u32) -> Vec<Address> {
        let mut list = Vec::new(env);
        for _ in 0..count {
            list.push_back(Address::generate(env));
        }
        list
    }

    #[test]
    fn reset_clears_only_the_temp_index_key() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let temp = symbol_short!("TMPIDX");
            let kept = symbol_short!("VAULT");
            env.storage().instance().set(&temp, &1u32);
            env.storage().instance().set(&kept, &2u32);
            let result = emergency_state_reset(&env, &guardians(&env, 4), &temp);
            assert_eq!(result, Ok(()));
            assert!(!env.storage().instance().has(&temp));
            assert!(env.storage().instance().has(&kept));
        });
    }

    #[test]
    fn reset_with_too_few_guardians_changes_nothing() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let temp = symbol_short!("TMPIDX");
            env.storage().instance().set(&temp, &1u32);
            let result = emergency_state_reset(&env, &guardians(&env, 3), &temp);
            assert_eq!(result, Err(StateResetError::InsufficientGuardianSignatures));
            assert!(env.storage().instance().has(&temp));
        });
    }

    #[test]
    fn reset_refuses_out_of_scope_keys() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let guard_key = symbol_short!("EMGUARD");
            env.storage().instance().set(&guard_key, &7u32);
            let result = emergency_state_reset(&env, &guardians(&env, 4), &guard_key);
            assert_eq!(result, Err(StateResetError::OutOfResetScope));
            assert!(env.storage().instance().has(&guard_key));
        });
    }
}
