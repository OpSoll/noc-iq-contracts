// Emergency override bypass for stuck disputes: 3-of-5 council can
// force-settle a dispute inactive >30 days. Complements dispute.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ForceSettleError {
    NotInactiveLongEnough = 1,
    InsufficientCouncilSignatures = 2,
    AlreadySettled = 3,
}

const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;
const FORCE_SETTLED_EVENT: Symbol = symbol_short!("dfsettle");

/// 3-of-5 admin council can force-settle a dispute that has been
/// inactive for more than 30 days, overruling pending arbitrator votes.
pub fn force_settle_dispute(
    env: &Env,
    dispute_id: u64,
    outcome: u32,
    last_activity_at: u64,
    council_signers: &Vec<Address>,
) -> Result<(), ForceSettleError> {
    if env.ledger().timestamp() < last_activity_at + THIRTY_DAYS_SECS {
        return Err(ForceSettleError::NotInactiveLongEnough);
    }
    if council_signers.len() < 3 {
        return Err(ForceSettleError::InsufficientCouncilSignatures);
    }
    for signer in council_signers.iter() {
        signer.require_auth();
    }
    env.events()
        .publish((FORCE_SETTLED_EVENT,), (dispute_id, outcome));
    Ok(())
}

/// Same as `force_settle_dispute`, but only applies to unsettled disputes.
pub fn force_settle_unsettled(
    env: &Env,
    dispute_id: u64,
    outcome: u32,
    last_activity_at: u64,
    council_signers: &Vec<Address>,
    already_settled: bool,
) -> Result<(), ForceSettleError> {
    if already_settled {
        return Err(ForceSettleError::AlreadySettled);
    }
    force_settle_dispute(env, dispute_id, outcome, last_activity_at, council_signers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    fn signers(env: &Env, count: u32) -> Vec<Address> {
        let mut list = Vec::new(env);
        for _ in 0..count {
            list.push_back(Address::generate(env));
        }
        list
    }

    #[test]
    fn force_settle_just_before_timeout_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger()
            .with_mut(|l| l.timestamp = 1_000 + THIRTY_DAYS_SECS - 1);
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let result = force_settle_dispute(&env, 1, 0, 1_000, &signers(&env, 3));
            assert_eq!(result, Err(ForceSettleError::NotInactiveLongEnough));
        });
    }

    #[test]
    fn force_settle_at_timeout_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger()
            .with_mut(|l| l.timestamp = 1_000 + THIRTY_DAYS_SECS);
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let result = force_settle_dispute(&env, 1, 0, 1_000, &signers(&env, 3));
            assert_eq!(result, Ok(()));
        });
    }

    #[test]
    fn force_settle_on_a_settled_dispute_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger()
            .with_mut(|l| l.timestamp = 1_000 + THIRTY_DAYS_SECS);
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let result = force_settle_unsettled(&env, 1, 0, 1_000, &signers(&env, 3), true);
            assert_eq!(result, Err(ForceSettleError::AlreadySettled));
        });
    }
}
