// Dispute withdrawal mechanism before voting begins. Complements
// dispute.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum WithdrawalError {
    WindowExpired = 1,
    VotesAlreadyCast = 2,
    AlreadyFinalized = 3,
    DisputesPaused = 4,
}

const TWO_HOURS_SECS: u64 = 2 * 60 * 60;
const WITHDRAWN_EVENT: Symbol = symbol_short!("dwithdrw");

/// Submitter withdraws a dispute within 2 hours, refunding 95% of the
/// bond, provided no arbitrator votes have been recorded yet.
pub fn withdraw_dispute(
    env: &Env,
    submitter: &Address,
    dispute_id: u64,
    filed_at: u64,
    votes_cast: u32,
    bond_amount: i128,
) -> Result<i128, WithdrawalError> {
    submitter.require_auth();
    if env.ledger().timestamp() > filed_at + TWO_HOURS_SECS {
        return Err(WithdrawalError::WindowExpired);
    }
    if votes_cast > 0 {
        return Err(WithdrawalError::VotesAlreadyCast);
    }
    let refund = bond_amount * 95 / 100;
    env.events().publish((WITHDRAWN_EVENT,), dispute_id);
    Ok(refund)
}

/// Same as `withdraw_dispute`, but rejects withdrawals once the dispute is
/// finalized or while disputes are paused.
#[allow(clippy::too_many_arguments)]
pub fn withdraw_dispute_checked(
    env: &Env,
    submitter: &Address,
    dispute_id: u64,
    filed_at: u64,
    votes_cast: u32,
    bond_amount: i128,
    finalized: bool,
    disputes_paused: bool,
) -> Result<i128, WithdrawalError> {
    if disputes_paused {
        return Err(WithdrawalError::DisputesPaused);
    }
    if finalized {
        return Err(WithdrawalError::AlreadyFinalized);
    }
    withdraw_dispute(env, submitter, dispute_id, filed_at, votes_cast, bond_amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn withdrawal_before_finalization_refunds_the_bond() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let submitter = Address::generate(&env);
            let result =
                withdraw_dispute_checked(&env, &submitter, 1, 0, 0, 1_000, false, false);
            assert_eq!(result, Ok(950));
        });
    }

    #[test]
    fn withdrawal_after_finalization_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let submitter = Address::generate(&env);
            let result = withdraw_dispute_checked(&env, &submitter, 1, 0, 0, 1_000, true, false);
            assert_eq!(result, Err(WithdrawalError::AlreadyFinalized));
        });
    }

    #[test]
    fn withdrawal_while_paused_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let submitter = Address::generate(&env);
            let result = withdraw_dispute_checked(&env, &submitter, 1, 0, 0, 1_000, false, true);
            assert_eq!(result, Err(WithdrawalError::DisputesPaused));
        });
    }
}
