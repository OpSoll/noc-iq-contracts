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
