use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY, STORAGE_VERSION, STORAGE_VERSION_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const UPGRADE_LOG_KEY: Symbol = symbol_short!("UPLOG");
/// Issue #707: configured backup guardian addresses (up to
/// `GUARDIAN_QUORUM_TOTAL`, i.e. 5).
const GUARDIANS_KEY: Symbol = symbol_short!("GUARDIAN");
/// Issue #707: the currently pending emergency admin key rotation
/// proposal, if any.
const ROTATION_PROPOSAL_KEY: Symbol = symbol_short!("ROT_PROP");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Record of a past upgrade.
#[soroban_sdk::contracttype]
pub struct UpgradeRecord {
    /// Storage version before upgrade.
    pub from_version: u32,
    /// Storage version after upgrade.
    pub to_version: u32,
    /// Ledger timestamp of upgrade.
    pub timestamp: u64,
    /// Address that performed the upgrade.
    pub upgraded_by: Address,
}

/// Upgrade plan for forward compatibility.
#[soroban_sdk::contracttype]
pub struct UpgradePlan {
    /// Target storage version.
    pub target_version: u32,
    /// Whether the plan has been executed.
    pub executed: bool,
    /// Timestamp when plan was created.
    pub created_at: u64,
}

/// Issue #707: a pending emergency admin key rotation, proposed by backup
/// guardians when the primary admin key is lost.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RotationProposal {
    /// The address that would become admin if this proposal executes.
    pub new_admin: Address,
    /// When the proposal was first initiated — the 72-hour timelock
    /// (`ROTATION_TIMELOCK_SECS`) counts from here.
    pub initiated_at: u64,
    /// Guardians who have approved this proposal so far (deduplicated).
    pub approvals: soroban_sdk::Vec<Address>,
}

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_UPGRADED: Symbol = symbol_short!("upgraded");
const EVENT_PLAN_CREATED: Symbol = symbol_short!("up_plan");
const EVENT_PLAN_EXECUTED: Symbol = symbol_short!("up_exec");
const EVENT_VERSION: Symbol = symbol_short!("v1");
/// Issue #707.
const EVENT_ROTATION_PROPOSED: Symbol = symbol_short!("rot_prop");
const EVENT_ROTATION_EXECUTED: Symbol = symbol_short!("rot_exec");
const EVENT_ROTATION_CANCELLED: Symbol = symbol_short!("rot_cncl");

// -----------------------------------------------------------------------
// Issue #707: emergency admin key rotation config / errors
// -----------------------------------------------------------------------

/// Number of guardian approvals required to execute a rotation proposal.
const GUARDIAN_QUORUM: u32 = 3;
/// Maximum number of configured guardians.
const GUARDIAN_QUORUM_TOTAL: u32 = 5;
/// Delay from a rotation proposal's initiation until it can execute.
const ROTATION_TIMELOCK_SECS: u64 = 72 * 60 * 60;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum KeyRotationError {
    /// Guardian set has not been configured, or `caller` is not on it.
    NotAGuardian = 1,
    /// `caller` is not the currently configured admin.
    NotAdmin = 2,
    /// The guardian set passed to `set_guardians` isn't between 1 and
    /// `GUARDIAN_QUORUM_TOTAL` addresses, or contains a duplicate.
    InvalidGuardianSet = 3,
    /// No rotation proposal is currently pending.
    NoActiveProposal = 4,
    /// Fewer than `GUARDIAN_QUORUM` guardians have approved yet.
    InsufficientApprovals = 5,
    /// The 72-hour timelock has not yet elapsed.
    TimelockNotElapsed = 6,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Perform a versioned upgrade from the current storage version to the next.
///
/// This function should be called after deploying a new contract binary that
/// bumps STORAGE_VERSION. It applies all migration steps sequentially and
/// records the upgrade in the log.
///
/// # Arguments
/// - `caller`: Must be the current admin.
///
/// # Events
/// - `upgraded`: Emitted after successful upgrade with version details.
///
/// # Errors
/// Returns `Unauthorized` if caller is not admin, `VersionMismatch` if
/// already at current version or if stored version is newer than expected.
pub fn perform_upgrade(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }

    let stored: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(0);

    if stored == STORAGE_VERSION {
        return Ok(()); // Already current
    }

    if stored > STORAGE_VERSION {
        return Err(SLAError::VersionMismatch);
    }

    let from_version = stored;
    let mut current = stored;

    // v0 → v1: stamp version (initialize sets all other fields)
    if current == 0 {
        env.storage().instance().set(&STORAGE_VERSION_KEY, &1u32);
        current = 1;
    }

    // Future migrations go here:
    // if current == 1 { ... current = 2; }

    if current != STORAGE_VERSION {
        return Err(SLAError::VersionMismatch);
    }

    // Log the upgrade
    let record = UpgradeRecord {
        from_version,
        to_version: STORAGE_VERSION,
        timestamp: env.ledger().timestamp(),
        upgraded_by: caller.clone(),
    };

    let mut log: soroban_sdk::Vec<UpgradeRecord> = env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    log.push_back(record);
    env.storage().instance().set(&UPGRADE_LOG_KEY, &log);

    env.events().publish(
        (EVENT_UPGRADED, EVENT_VERSION, caller),
        (from_version, STORAGE_VERSION),
    );

    Ok(())
}

/// Returns the full upgrade history.
pub fn get_upgrade_log(env: &Env) -> Result<soroban_sdk::Vec<UpgradeRecord>, SLAError> {
    Ok(env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env)))
}

/// Returns the number of upgrades that have been performed.
pub fn get_upgrade_count(env: &Env) -> Result<u32, SLAError> {
    let log: soroban_sdk::Vec<UpgradeRecord> = env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    Ok(log.len())
}

/// Returns the most recent upgrade record, if any.
pub fn get_last_upgrade(env: &Env) -> Result<Option<UpgradeRecord>, SLAError> {
    let log: soroban_sdk::Vec<UpgradeRecord> = env
        .storage()
        .instance()
        .get(&UPGRADE_LOG_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    if log.is_empty() {
        Ok(None)
    } else {
        Ok(Some(log.get(log.len() - 1).unwrap()))
    }
}

/// Check whether an upgrade is available (stored version < binary version).
pub fn upgrade_available(env: &Env) -> Result<bool, SLAError> {
    let stored: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(0);
    Ok(stored < STORAGE_VERSION)
}

/// Returns the current and expected storage versions for pre-upgrade checks.
pub fn get_version_pair(env: &Env) -> Result<(u32, u32), SLAError> {
    let stored: u32 = env
        .storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .ok_or(SLAError::NotInitialized)?;
    Ok((stored, STORAGE_VERSION))
}

// -----------------------------------------------------------------------
// Issue #707: emergency admin key rotation protocol
// -----------------------------------------------------------------------

/// Admin-only: configure the backup guardian set allowed to initiate an
/// emergency key rotation. Replaces any previously configured set.
///
/// # Errors
/// Returns `InvalidGuardianSet` if `guardians` is empty, has more than
/// `GUARDIAN_QUORUM_TOTAL` (5) entries, or contains a duplicate address.
pub fn set_guardians(
    env: &Env,
    caller: &Address,
    guardians: soroban_sdk::Vec<Address>,
) -> Result<(), KeyRotationError> {
    require_current_admin(env, caller)?;

    if guardians.is_empty() || guardians.len() > GUARDIAN_QUORUM_TOTAL {
        return Err(KeyRotationError::InvalidGuardianSet);
    }
    for i in 0..guardians.len() {
        for j in (i + 1)..guardians.len() {
            if guardians.get(i).unwrap() == guardians.get(j).unwrap() {
                return Err(KeyRotationError::InvalidGuardianSet);
            }
        }
    }

    env.storage().instance().set(&GUARDIANS_KEY, &guardians);
    Ok(())
}

/// A backup guardian initiates (or adds their approval to) an emergency
/// admin key rotation proposal. The first guardian to call this starts
/// the 72-hour timelock; later guardians calling with the *same*
/// `new_admin` add their approval to the existing proposal rather than
/// restarting the clock.
///
/// # Events
/// - `rot_prop`: Emitted when a guardian's approval is recorded.
///
/// # Errors
/// Returns `NotAGuardian` if `caller` is not on the configured guardian
/// set.
pub fn propose_key_rotation(
    env: &Env,
    caller: &Address,
    new_admin: Address,
) -> Result<(), KeyRotationError> {
    caller.require_auth();

    let guardians = load_guardians(env);
    if !guardians.contains(caller) {
        return Err(KeyRotationError::NotAGuardian);
    }

    let now = env.ledger().timestamp();
    let mut proposal = match load_proposal(env) {
        Some(existing) if existing.new_admin == new_admin => existing,
        _ => RotationProposal {
            new_admin,
            initiated_at: now,
            approvals: soroban_sdk::Vec::new(env),
        },
    };

    if !proposal.approvals.contains(caller) {
        proposal.approvals.push_back(caller.clone());
    }

    env.storage()
        .instance()
        .set(&ROTATION_PROPOSAL_KEY, &proposal);
    env.events().publish(
        (EVENT_ROTATION_PROPOSED, EVENT_VERSION, caller.clone()),
        (proposal.new_admin, proposal.approvals.len()),
    );

    Ok(())
}

/// Execute a pending rotation proposal once it has both the required
/// 3-of-5 guardian quorum and the 72-hour timelock has elapsed. Callable
/// by anyone — the guardians' approvals are the authorization, not the
/// caller of this function.
///
/// # Events
/// - `rot_exec`: Emitted when the admin key is rotated.
///
/// # Errors
/// Returns `NoActiveProposal`, `InsufficientApprovals` if fewer than
/// `GUARDIAN_QUORUM` guardians have approved, or `TimelockNotElapsed` if
/// the 72-hour delay hasn't passed yet.
pub fn execute_key_rotation(env: &Env) -> Result<Address, KeyRotationError> {
    let proposal = load_proposal(env).ok_or(KeyRotationError::NoActiveProposal)?;

    if proposal.approvals.len() < GUARDIAN_QUORUM {
        return Err(KeyRotationError::InsufficientApprovals);
    }
    if env.ledger().timestamp() < proposal.initiated_at + ROTATION_TIMELOCK_SECS {
        return Err(KeyRotationError::TimelockNotElapsed);
    }

    env.storage()
        .instance()
        .set(&ADMIN_KEY, &proposal.new_admin);
    env.storage().instance().remove(&ROTATION_PROPOSAL_KEY);

    env.events().publish(
        (EVENT_ROTATION_EXECUTED, EVENT_VERSION),
        proposal.new_admin.clone(),
    );

    Ok(proposal.new_admin)
}

/// The active admin cancels a pending rotation proposal before its
/// timelock expires (e.g. because the primary key was recovered).
///
/// # Events
/// - `rot_cncl`: Emitted when a proposal is cancelled.
///
/// # Errors
/// Returns `NotAdmin` if `caller` is not the current admin, or
/// `NoActiveProposal` if there is nothing to cancel.
pub fn cancel_key_rotation(env: &Env, caller: &Address) -> Result<(), KeyRotationError> {
    require_current_admin(env, caller)?;

    if load_proposal(env).is_none() {
        return Err(KeyRotationError::NoActiveProposal);
    }

    env.storage().instance().remove(&ROTATION_PROPOSAL_KEY);
    env.events()
        .publish((EVENT_ROTATION_CANCELLED, EVENT_VERSION), caller.clone());

    Ok(())
}

/// Returns the currently pending rotation proposal, if any.
pub fn get_rotation_proposal(env: &Env) -> Option<RotationProposal> {
    load_proposal(env)
}

fn load_guardians(env: &Env) -> soroban_sdk::Vec<Address> {
    env.storage()
        .instance()
        .get(&GUARDIANS_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

fn load_proposal(env: &Env) -> Option<RotationProposal> {
    env.storage().instance().get(&ROTATION_PROPOSAL_KEY)
}

fn require_current_admin(env: &Env, caller: &Address) -> Result<(), KeyRotationError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(KeyRotationError::NotAdmin)?;
    if admin != *caller {
        return Err(KeyRotationError::NotAdmin);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    fn setup(env: &Env, admin: &Address) {
        env.storage().instance().set(&ADMIN_KEY, admin);
    }

    fn guardians(env: &Env, n: u32) -> soroban_sdk::Vec<Address> {
        let mut v = soroban_sdk::Vec::new(env);
        for _ in 0..n {
            v.push_back(Address::generate(env));
        }
        v
    }

    #[test]
    fn test_set_guardians_rejects_empty_or_oversized_set() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            let empty = soroban_sdk::Vec::new(&env);
            assert_eq!(
                set_guardians(&env, &admin, empty),
                Err(KeyRotationError::InvalidGuardianSet)
            );
        });
        env.as_contract(&cid, || {
            let too_many = guardians(&env, GUARDIAN_QUORUM_TOTAL + 1);
            assert_eq!(
                set_guardians(&env, &admin, too_many),
                Err(KeyRotationError::InvalidGuardianSet)
            );
        });
    }

    #[test]
    fn test_non_guardian_cannot_propose_rotation() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let gs = guardians(&env, 5);
        let stranger = Address::generate(&env);
        let new_admin = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            set_guardians(&env, &admin, gs).unwrap();
        });
        env.as_contract(&cid, || {
            let result = propose_key_rotation(&env, &stranger, new_admin);
            assert_eq!(result, Err(KeyRotationError::NotAGuardian));
        });
    }

    #[test]
    fn test_rotation_requires_quorum_and_timelock() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let gs = guardians(&env, 5);
        let g0 = gs.get(0).unwrap();
        let g1 = gs.get(1).unwrap();
        let g2 = gs.get(2).unwrap();
        let new_admin = Address::generate(&env);

        env.ledger().set_timestamp(1_000);
        env.as_contract(&cid, || {
            setup(&env, &admin);
            set_guardians(&env, &admin, gs.clone()).unwrap();
        });

        env.as_contract(&cid, || {
            propose_key_rotation(&env, &g0, new_admin.clone()).unwrap();
        });
        // Only 1 approval so far: quorum not met yet even once the
        // timelock later elapses.
        env.ledger()
            .set_timestamp(1_000 + ROTATION_TIMELOCK_SECS + 1);
        env.as_contract(&cid, || {
            assert_eq!(
                execute_key_rotation(&env),
                Err(KeyRotationError::InsufficientApprovals)
            );
        });

        // Reach 3-of-5 quorum, but the timelock started at 1_000 and
        // hasn't elapsed relative to *now* being reset below.
        env.ledger().set_timestamp(1_000 + 10);
        env.as_contract(&cid, || {
            propose_key_rotation(&env, &g1, new_admin.clone()).unwrap();
        });
        env.as_contract(&cid, || {
            propose_key_rotation(&env, &g2, new_admin.clone()).unwrap();
        });
        env.as_contract(&cid, || {
            assert_eq!(
                execute_key_rotation(&env),
                Err(KeyRotationError::TimelockNotElapsed)
            );
        });

        // Once both quorum and timelock are satisfied, rotation executes.
        env.ledger()
            .set_timestamp(1_000 + ROTATION_TIMELOCK_SECS + 1);
        env.as_contract(&cid, || {
            let rotated_to = execute_key_rotation(&env).unwrap();
            assert_eq!(rotated_to, new_admin);
            let admin_now: Address = env.storage().instance().get(&ADMIN_KEY).unwrap();
            assert_eq!(admin_now, new_admin);
            assert!(get_rotation_proposal(&env).is_none());
        });
    }

    #[test]
    fn test_admin_can_cancel_pending_rotation() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let gs = guardians(&env, 5);
        let g0 = gs.get(0).unwrap();
        let new_admin = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            set_guardians(&env, &admin, gs).unwrap();
        });
        env.as_contract(&cid, || {
            propose_key_rotation(&env, &g0, new_admin).unwrap();
        });
        env.as_contract(&cid, || {
            assert!(get_rotation_proposal(&env).is_some());
            cancel_key_rotation(&env, &admin).unwrap();
        });
        env.as_contract(&cid, || {
            assert!(get_rotation_proposal(&env).is_none());
            let admin_now: Address = env.storage().instance().get(&ADMIN_KEY).unwrap();
            assert_eq!(admin_now, admin);
        });
    }

    #[test]
    fn test_cancel_rejects_non_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let not_admin = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            let result = cancel_key_rotation(&env, &not_admin);
            assert_eq!(result, Err(KeyRotationError::NotAdmin));
        });
    }
}
