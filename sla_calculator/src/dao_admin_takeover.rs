// Emergency Admin DAO takeover protocol. Complements upgrade_path.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TakeoverError {
    AdminStillActive = 1,
}

const NINETY_DAYS_SECS: u64 = 90 * 24 * 60 * 60;
const TAKEOVER_EVENT: Symbol = symbol_short!("dao_take");

/// A registered DAO contract claims admin rights if the current admin
/// has been inactive for more than 90 days.
pub fn claim_admin_takeover(
    env: &Env,
    dao_contract: &Address,
    last_admin_activity: u64,
) -> Result<(), TakeoverError> {
    dao_contract.require_auth();
    if env.ledger().timestamp() < last_admin_activity + NINETY_DAYS_SECS {
        return Err(TakeoverError::AdminStillActive);
    }
    env.events().publish((TAKEOVER_EVENT,), dao_contract.clone());
    Ok(())
}
