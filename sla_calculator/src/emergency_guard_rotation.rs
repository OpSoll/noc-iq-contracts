// Emergency Guard role rotation protocol. Complements whitelist.rs.
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

const GUARD_KEY: Symbol = symbol_short!("EMGUARD");
const ROTATED_EVENT: Symbol = symbol_short!("eg_rot");

/// Active admin replaces the current Emergency Guard key immediately,
/// revoking pause authorization from the former key.
pub fn rotate_emergency_guard(
    env: &Env,
    admin: &Address,
    old_guard: &Address,
    new_guard: &Address,
) {
    admin.require_auth();
    let current: Address = env
        .storage()
        .instance()
        .get(&GUARD_KEY)
        .unwrap_or_else(|| old_guard.clone());
    if &current == old_guard {
        env.storage().instance().set(&GUARD_KEY, new_guard);
        env.events()
            .publish((ROTATED_EVENT,), (old_guard.clone(), new_guard.clone()));
    }
}

pub fn current_emergency_guard(env: &Env) -> Option<Address> {
    env.storage().instance().get(&GUARD_KEY)
}

/// Returns the active guard set (currently a single Emergency Guard key).
pub fn active_guard_set(env: &Env) -> Vec<Address> {
    let mut guards = Vec::new(env);
    if let Some(guard) = current_emergency_guard(env) {
        guards.push_back(guard);
    }
    guards
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    #[test]
    #[should_panic]
    fn rotation_without_admin_auth_is_rejected() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let admin = Address::generate(&env);
            let old_guard = Address::generate(&env);
            let new_guard = Address::generate(&env);
            rotate_emergency_guard(&env, &admin, &old_guard, &new_guard);
        });
    }

    #[test]
    fn rotation_from_a_non_current_guard_is_ignored() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let admin = Address::generate(&env);
            let old_guard = Address::generate(&env);
            let new_guard = Address::generate(&env);
            rotate_emergency_guard(&env, &admin, &old_guard, &new_guard);
            let stranger = Address::generate(&env);
            rotate_emergency_guard(&env, &admin, &stranger, &Address::generate(&env));
            assert_eq!(current_emergency_guard(&env), Some(new_guard));
        });
    }

    #[test]
    fn active_guard_set_returns_the_current_guard() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            assert_eq!(active_guard_set(&env).len(), 0);
            let admin = Address::generate(&env);
            let old_guard = Address::generate(&env);
            let new_guard = Address::generate(&env);
            rotate_emergency_guard(&env, &admin, &old_guard, &new_guard);
            let guards = active_guard_set(&env);
            assert_eq!(guards.len(), 1);
            assert_eq!(guards.get(0), Some(new_guard));
        });
    }
}
