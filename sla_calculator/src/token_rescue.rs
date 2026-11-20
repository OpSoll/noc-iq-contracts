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

fn publish_rescue(env: &Env, token: &Address, recipient: &Address, amount: i128) {
    env.events()
        .publish((RESCUE_EVENT, token.clone()), (amount, recipient.clone()));
}

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
    if token_address == collateral_token
        || crate::collateral_lock::is_protected_asset(env, token_address)
    {
        return Err(RescueError::CannotRescueCollateralToken);
    }
    let client = token::Client::new(env, token_address);
    client.transfer(&env.current_contract_address(), recipient, &amount);
    publish_rescue(env, token_address, recipient, amount);
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
    if crate::collateral_lock::is_protected_asset(env, collateral_token) {
        return Err(RescueError::CannotRescueCollateralToken);
    }
    let client = token::Client::new(env, collateral_token);
    let balance = client.balance(&env.current_contract_address());
    let locked = crate::collateral_lock::total_locked(env);
    if amount > balance - locked {
        return Err(RescueError::ExceedsUnlockedSurplus);
    }
    crate::collateral_lock::protect_accounting_asset(env, collateral_token);
    client.transfer(&env.current_contract_address(), recipient, &amount);
    publish_rescue(env, collateral_token, recipient, amount);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collateral_lock::{lock_collateral, protect_accounting_asset};
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::{Address as _, Events as _};
    use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
    use soroban_sdk::TryIntoVal;

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

    #[test]
    fn test_a_registered_accounting_asset_cannot_be_rescued_by_an_admin() {
        let f = setup();
        let protected = token_with_balance(&f, 500);
        let unrelated_collateral = Address::generate(&f.env);
        f.env.as_contract(&f.contract, || {
            protect_accounting_asset(&f.env, &protected);
            assert_eq!(
                rescue_tokens(
                    &f.env,
                    &f.admin,
                    &unrelated_collateral,
                    &protected,
                    &f.recipient,
                    500
                ),
                Err(RescueError::CannotRescueCollateralToken)
            );
        });
        assert_eq!(TokenClient::new(&f.env, &protected).balance(&f.contract), 500);
        assert_eq!(TokenClient::new(&f.env, &protected).balance(&f.recipient), 0);
    }

    #[test]
    fn test_a_registered_accounting_asset_is_blocked_from_surplus_rescue() {
        let f = setup();
        let collateral = token_with_balance(&f, 250);
        f.env.as_contract(&f.contract, || {
            protect_accounting_asset(&f.env, &collateral);
            assert_eq!(
                rescue_surplus_collateral(&f.env, &f.admin, &collateral, &f.recipient, 10),
                Err(RescueError::CannotRescueCollateralToken)
            );
        });
        assert_eq!(TokenClient::new(&f.env, &collateral).balance(&f.contract), 250);
    }

    #[test]
    fn test_dust_rescue_event_reports_the_exact_amount_and_destination() {
        let f = setup();
        let collateral = Address::generate(&f.env);
        let stray = token_with_balance(&f, 500);
        f.env.as_contract(&f.contract, || {
            rescue_tokens(&f.env, &f.admin, &collateral, &stray, &f.recipient, 300).unwrap();
        });

        let events = f.env.events().all();
        let (_, topics, data) = events.last().unwrap();
        let topic_0: Symbol = topics.get(0).unwrap().try_into_val(&f.env).unwrap();
        let topic_1: Address = topics.get(1).unwrap().try_into_val(&f.env).unwrap();
        let reported: (i128, Address) = data.try_into_val(&f.env).unwrap();
        assert_eq!(topic_0, RESCUE_EVENT);
        assert_eq!(topic_1, stray);
        assert_eq!(reported, (300, f.recipient));
    }
}
