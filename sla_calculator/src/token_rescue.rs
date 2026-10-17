// Emergency collateral rescue for deprecated/mistakenly-deposited
// tokens. Complements emergency.rs.
use soroban_sdk::{contracterror, symbol_short, token, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RescueError {
    CannotRescueCollateralToken = 1,
    /// The requested amount would dip into collateral that is still locked.
    ExceedsUnlockedSurplus = 2,
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

/// Admin-only: rescue *surplus* collateral-token balance, i.e. only what the
/// contract holds above the amount still locked for providers. Rescuing
/// locked collateral is refused.
pub fn rescue_surplus_collateral(
    env: &Env,
    admin: &Address,
    collateral_token: &Address,
    recipient: &Address,
    amount: i128,
) -> Result<(), RescueError> {
    admin.require_auth();
    let client = token::Client::new(env, collateral_token);
    let balance = client.balance(&env.current_contract_address());
    let locked = crate::collateral_lock::total_locked(env);
    if amount > balance - locked {
        return Err(RescueError::ExceedsUnlockedSurplus);
    }
    client.transfer(&env.current_contract_address(), recipient, &amount);
    env.events()
        .publish((RESCUE_EVENT,), collateral_token.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collateral_lock::lock_collateral;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

    struct Fixture {
        env: Env,
        contract: Address,
        admin: Address,
        recipient: Address,
    }

    fn setup() -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        Fixture {
            admin: Address::generate(&env),
            recipient: Address::generate(&env),
            contract,
            env,
        }
    }

    fn token_with_balance(f: &Fixture, amount: i128) -> Address {
        let token = f
            .env
            .register_stellar_asset_contract_v2(Address::generate(&f.env))
            .address();
        StellarAssetClient::new(&f.env, &token).mint(&f.contract, &amount);
        token
    }

    #[test]
    fn test_admin_rescues_a_non_collateral_token() {
        let f = setup();
        let collateral = Address::generate(&f.env);
        let stray = token_with_balance(&f, 500);
        f.env.as_contract(&f.contract, || {
            rescue_tokens(&f.env, &f.admin, &collateral, &stray, &f.recipient, 500).unwrap();
        });
        assert_eq!(TokenClient::new(&f.env, &stray).balance(&f.recipient), 500);
    }

    #[test]
    fn test_collateral_token_cannot_be_rescued_via_rescue_tokens() {
        let f = setup();
        let collateral = token_with_balance(&f, 500);
        f.env.as_contract(&f.contract, || {
            assert_eq!(
                rescue_tokens(&f.env, &f.admin, &collateral, &collateral, &f.recipient, 1),
                Err(RescueError::CannotRescueCollateralToken)
            );
        });
    }

    #[test]
    #[should_panic]
    fn test_rescue_requires_admin_authorization() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        let (admin, other, recipient) = (
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        );
        env.as_contract(&contract, || {
            let _ = rescue_tokens(&env, &admin, &other, &Address::generate(&env), &recipient, 1);
        });
    }

    #[test]
    fn test_surplus_rescue_stops_at_the_locked_collateral() {
        let f = setup();
        let collateral = token_with_balance(&f, 1_000);
        let provider = Address::generate(&f.env);
        f.env.as_contract(&f.contract, || {
            lock_collateral(&f.env, &provider, 600, 10_000, 0).unwrap();
            assert_eq!(
                rescue_surplus_collateral(&f.env, &f.admin, &collateral, &f.recipient, 401),
                Err(RescueError::ExceedsUnlockedSurplus)
            );
            rescue_surplus_collateral(&f.env, &f.admin, &collateral, &f.recipient, 400).unwrap();
        });
        let t = TokenClient::new(&f.env, &collateral);
        assert_eq!(t.balance(&f.recipient), 400);
        assert_eq!(t.balance(&f.contract), 600);
    }

    #[test]
    fn test_with_nothing_locked_the_whole_balance_is_surplus() {
        let f = setup();
        let collateral = token_with_balance(&f, 250);
        f.env.as_contract(&f.contract, || {
            rescue_surplus_collateral(&f.env, &f.admin, &collateral, &f.recipient, 250).unwrap();
        });
        assert_eq!(TokenClient::new(&f.env, &collateral).balance(&f.recipient), 250);
    }
}
