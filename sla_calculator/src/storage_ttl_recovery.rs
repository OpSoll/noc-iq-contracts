// Storage TTL expiration panic recovery guard + dangling index repair (#1007).
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StorageRecoveryError {
    StorageExpired = 1,
}

const RENT_PAID_EVENT: Symbol = symbol_short!("rent_pd");
const INDEX_KEY: Symbol = symbol_short!("IDX");
const LIVE_PREFIX: Symbol = symbol_short!("REC");

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

/// Index of record ids (as Symbols). Survives after individual records expire.
pub fn site_index(env: &Env) -> Vec<Symbol> {
    env.storage()
        .instance()
        .get(&INDEX_KEY)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_site_index(env: &Env, index: &Vec<Symbol>) {
    env.storage().instance().set(&INDEX_KEY, index);
}

fn record_key(env: &Env, id: &Symbol) -> (Symbol, Symbol) {
    (LIVE_PREFIX, id.clone())
}

pub fn put_record(env: &Env, id: &Symbol, value: u32) {
    env.storage().instance().set(&record_key(env, id), &value);
}

pub fn get_record(env: &Env, id: &Symbol) -> Option<u32> {
    env.storage().instance().get(&record_key(env, id))
}

pub fn remove_record(env: &Env, id: &Symbol) {
    env.storage().instance().remove(&record_key(env, id));
}

/// Remove index entries whose referenced records are already gone (#1007).
/// Live entries and their index slots are left intact. Never panics.
pub fn repair_dangling_indexes(env: &Env) -> u32 {
    let index = site_index(env);
    let mut kept = Vec::new(env);
    let mut removed = 0u32;
    for i in 0..index.len() {
        let id = index.get(i).unwrap();
        if get_record(env, &id).is_some() {
            kept.push_back(id);
        } else {
            removed += 1;
        }
    }
    set_site_index(env, &kept);
    removed
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

    /// #1007 — expired records with surviving indexes are repaired without panic.
    #[test]
    fn test_dangling_index_repair_removes_expired_keeps_live() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let live = Symbol::new(&env, "live");
            let dead = Symbol::new(&env, "dead");
            put_record(&env, &live, 7);
            // `dead` is indexed but has no record payload (simulates TTL expiry).
            set_site_index(&env, &soroban_sdk::vec![&env, live.clone(), dead.clone()]);

            let removed = repair_dangling_indexes(&env);
            assert_eq!(removed, 1);

            let index = site_index(&env);
            assert_eq!(index.len(), 1);
            assert_eq!(index.get(0).unwrap(), live);
            assert_eq!(get_record(&env, &live), Some(7));
            assert_eq!(get_record(&env, &dead), None);
        });
    }
}
