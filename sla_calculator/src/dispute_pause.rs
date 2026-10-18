// Emergency dispute pause toggle. Complements emergency.rs.
use soroban_sdk::{symbol_short, Address, Env, Symbol};

const DISPUTES_PAUSED_KEY: Symbol = symbol_short!("DPPAUSE");
const PAUSE_REASON_KEY: Symbol = symbol_short!("DPREASON");

/// Admin governance toggles pause_disputes to suspend new filings and
/// arbitrator vote submissions during a security incident. Existing
/// dispute deadlines are preserved (callers should not reset them).
pub fn set_disputes_paused(env: &Env, admin: &Address, paused: bool) {
    admin.require_auth();
    env.storage().instance().set(&DISPUTES_PAUSED_KEY, &paused);
}

pub fn are_disputes_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DISPUTES_PAUSED_KEY)
        .unwrap_or(false)
}

/// Same as `set_disputes_paused`, also recording the reason for the change.
pub fn set_disputes_paused_with_reason(env: &Env, admin: &Address, paused: bool, reason: Symbol) {
    set_disputes_paused(env, admin, paused);
    env.storage().instance().set(&PAUSE_REASON_KEY, &reason);
}

/// Returns whether disputes are paused and the recorded reason.
pub fn dispute_pause_state(env: &Env) -> (bool, Symbol) {
    let reason: Symbol = env
        .storage()
        .instance()
        .get(&PAUSE_REASON_KEY)
        .unwrap_or(symbol_short!("none"));
    (are_disputes_paused(env), reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn pause_state_exposes_flag_and_reason() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let admin = Address::generate(&env);
            assert_eq!(dispute_pause_state(&env), (false, symbol_short!("none")));
            set_disputes_paused_with_reason(&env, &admin, true, symbol_short!("incident"));
            assert_eq!(dispute_pause_state(&env), (true, symbol_short!("incident")));
        });
    }

    #[test]
    fn repeated_pause_and_resume_keep_state_consistent() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let admin = Address::generate(&env);
            set_disputes_paused(&env, &admin, true);
            set_disputes_paused(&env, &admin, true);
            assert!(are_disputes_paused(&env));
            set_disputes_paused(&env, &admin, false);
            set_disputes_paused(&env, &admin, false);
            assert!(!are_disputes_paused(&env));
        });
    }
}
