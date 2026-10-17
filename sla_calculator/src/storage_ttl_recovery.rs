// Storage TTL expiration panic recovery guard (issue: Storage TTL
// expiration panic recovery guard). Complements pruning_perf.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StorageRecoveryError {
    StorageExpired = 1,
}

const RENT_PAID_EVENT: Symbol = symbol_short!("rent_pd");

/// Checks whether instance storage TTL has expired; returns a clear
/// error instead of letting the entrypoint panic.
pub fn check_storage_not_expired(env: &Env) -> Result<(), StorageRecoveryError> {
    if env.storage().instance().has(&symbol_short!("ALIVE")) {
        Ok(())
    } else {
        Err(StorageRecoveryError::StorageExpired)
    }
}

/// Anyone can call this to pay the storage rent fee and re-activate an
/// expired contract instance.
pub fn bump_contract_instance_ttl(env: &Env, payer: &Address, extend_by: u32) {
    payer.require_auth();
    env.storage().instance().extend_ttl(extend_by, extend_by);
    env.storage().instance().set(&symbol_short!("ALIVE"), &true);
    env.events().publish((RENT_PAID_EVENT,), payer.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_missing_liveness_marker_reports_storage_expired() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            assert_eq!(
                check_storage_not_expired(&env),
                Err(StorageRecoveryError::StorageExpired)
            );
        });
    }

    #[test]
    fn test_bump_recovers_an_expired_instance() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SLACalculatorContract);
        let payer = Address::generate(&env);
        env.as_contract(&id, || {
            assert!(check_storage_not_expired(&env).is_err());
            bump_contract_instance_ttl(&env, &payer, 1_000);
            assert_eq!(check_storage_not_expired(&env), Ok(()));
        });
    }

    #[test]
    fn test_repeated_bumps_keep_the_instance_alive() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SLACalculatorContract);
        let payer = Address::generate(&env);
        env.as_contract(&id, || {
            bump_contract_instance_ttl(&env, &payer, 500);
            bump_contract_instance_ttl(&env, &payer, 500);
            assert_eq!(check_storage_not_expired(&env), Ok(()));
        });
    }

    #[test]
    #[should_panic]
    fn test_bump_requires_payer_authorization() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let payer = Address::generate(&env);
        env.as_contract(&id, || bump_contract_instance_ttl(&env, &payer, 500));
    }
}
