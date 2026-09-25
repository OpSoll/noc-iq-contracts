// Contract deprecation shutdown protocol: puts the contract into a
// permanent read-only retirement mode. Complements emergency.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DeprecationError {
    ContractDeprecated = 1,
}

const DEPRECATED_KEY: Symbol = symbol_short!("DEPRCTD");

/// Admin-only: permanently deprecates the contract. Mutating calls must
/// call `assert_not_deprecated` first; read-only queries remain usable.
pub fn deprecate_contract(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&DEPRECATED_KEY, &true);
}

pub fn is_deprecated(env: &Env) -> bool {
    env.storage().instance().get(&DEPRECATED_KEY).unwrap_or(false)
}

/// Call at the top of any state-mutating entrypoint.
pub fn assert_not_deprecated(env: &Env) -> Result<(), DeprecationError> {
    if is_deprecated(env) {
        Err(DeprecationError::ContractDeprecated)
    } else {
        Ok(())
    }
}
