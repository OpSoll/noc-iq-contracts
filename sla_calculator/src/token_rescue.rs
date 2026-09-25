// Emergency collateral rescue for deprecated/mistakenly-deposited
// tokens. Complements emergency.rs.
use soroban_sdk::{contracterror, symbol_short, token, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RescueError {
    CannotRescueCollateralToken = 1,
}

const RESCUE_EVENT: Symbol = symbol_short!("rescued");

/// Admin-only: rescue a non-collateral token balance accidentally sent
/// to the contract address.
pub fn rescue_tokens(
    env: &Env,
    admin: &Address,
    collateral_token: &Address,
    token_address: &Address,
    recipient: &Address,
    amount: i128,
) -> Result<(), RescueError> {
    admin.require_auth();
    if token_address == collateral_token {
        return Err(RescueError::CannotRescueCollateralToken);
    }
    let client = token::Client::new(env, token_address);
    client.transfer(&env.current_contract_address(), recipient, &amount);
    env.events().publish((RESCUE_EVENT,), token_address.clone());
    Ok(())
}
