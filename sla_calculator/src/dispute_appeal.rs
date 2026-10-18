// Dispute outcome appeal mechanism to secondary council. Complements
// dispute.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AppealError {
    WindowExpired = 1,
    AlreadyAppealed = 2,
    MaxAppealsReached = 3,
    InvalidMaxAppeals = 4,
}

const FORTY_EIGHT_HOURS_SECS: u64 = 48 * 60 * 60;
const APPEAL_FILED_EVENT: Symbol = symbol_short!("appeal_f");
const APPEAL_COUNT_KEY: Symbol = symbol_short!("APLCNT");
const MAX_APPEALS_KEY: Symbol = symbol_short!("APLMAX");
const DEFAULT_MAX_APPEALS: u32 = 1;
const MAX_APPEALS_CEILING: u32 = 10;

/// Losing party files a one-time appeal within 48 hours of settlement by
/// depositing 2x the original appeal bond, escalating to the Senior
/// Governance Council for a final binding decision.
pub fn file_appeal(
    env: &Env,
    appellant: &Address,
    dispute_id: u64,
    settled_at: u64,
    already_appealed: bool,
    original_bond: i128,
) -> Result<i128, AppealError> {
    appellant.require_auth();
    if already_appealed {
        return Err(AppealError::AlreadyAppealed);
    }
    if env.ledger().timestamp() > settled_at + FORTY_EIGHT_HOURS_SECS {
        return Err(AppealError::WindowExpired);
    }
    let required_bond = original_bond * 2;
    env.events().publish((APPEAL_FILED_EVENT,), dispute_id);
    Ok(required_bond)
}

/// Returns the number of appeals consumed by a dispute.
pub fn appeal_count(env: &Env, dispute_id: u64) -> u32 {
    env.storage()
        .persistent()
        .get(&(APPEAL_COUNT_KEY, dispute_id))
        .unwrap_or(0)
}

/// Returns the configured maximum number of appeals per dispute.
pub fn max_appeals(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&MAX_APPEALS_KEY)
        .unwrap_or(DEFAULT_MAX_APPEALS)
}

/// Admin sets the maximum number of appeals; zero or above the ceiling is rejected.
pub fn set_max_appeals(env: &Env, admin: &Address, max: u32) -> Result<(), AppealError> {
    admin.require_auth();
    if max == 0 || max > MAX_APPEALS_CEILING {
        return Err(AppealError::InvalidMaxAppeals);
    }
    env.storage().instance().set(&MAX_APPEALS_KEY, &max);
    Ok(())
}

/// Consumes one appeal for the dispute, rejecting it once the maximum is reached.
pub fn record_appeal(env: &Env, dispute_id: u64) -> Result<u32, AppealError> {
    let used = appeal_count(env, dispute_id);
    if used >= max_appeals(env) {
        return Err(AppealError::MaxAppealsReached);
    }
    env.storage()
        .persistent()
        .set(&(APPEAL_COUNT_KEY, dispute_id), &(used + 1));
    Ok(used + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn appeal_count_starts_at_zero_and_increments() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            assert_eq!(appeal_count(&env, 1), 0);
            assert_eq!(record_appeal(&env, 1), Ok(1));
            assert_eq!(appeal_count(&env, 1), 1);
        });
    }

    #[test]
    fn appeal_beyond_the_max_is_rejected() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            assert_eq!(record_appeal(&env, 1), Ok(1));
            assert_eq!(record_appeal(&env, 1), Err(AppealError::MaxAppealsReached));
        });
    }

    #[test]
    fn invalid_max_appeals_are_rejected_on_set() {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let admin = Address::generate(&env);
            assert_eq!(
                set_max_appeals(&env, &admin, 0),
                Err(AppealError::InvalidMaxAppeals)
            );
            assert_eq!(
                set_max_appeals(&env, &admin, MAX_APPEALS_CEILING + 1),
                Err(AppealError::InvalidMaxAppeals)
            );
            assert_eq!(set_max_appeals(&env, &admin, 3), Ok(()));
            assert_eq!(max_appeals(&env), 3);
        });
    }
}
