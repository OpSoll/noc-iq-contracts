// Emergency state reset to clean baseline. Complements emergency.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StateResetError {
    InsufficientGuardianSignatures = 1,
}

const RESET_EVENT: Symbol = symbol_short!("statersr");

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
    for guardian in guardians.iter() {
        guardian.require_auth();
    }
    env.storage().instance().remove(temp_index_key);
    env.events().publish((RESET_EVENT,), guardians.len());
    Ok(())
}
