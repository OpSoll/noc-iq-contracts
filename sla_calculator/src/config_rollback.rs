// Config rollback helper to previous version. Complements
// config_freeze.rs.
use crate::config_history_retention::{
    current_revision, find_version, set_current_revision, ConfigVersionRecord,
};
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RollbackError {
    VersionNotFound = 1,
    RevisionConflict = 2,
}

const ROLLED_BACK_EVENT: Symbol = symbol_short!("cfgrbck");
const RESTORED_VERSION_KEY: Symbol = symbol_short!("CFGRSTVER");
/// Provenance event: `(previous_revision, restored_source_revision, resulting_revision)`.
pub const ROLLBACK_PROVENANCE_EVENT: Symbol = symbol_short!("cfgrbprv");

/// Governance reverts config settings to a recorded historical version,
/// validating the target version exists in the history log first.
pub fn rollback_config(
    env: &Env,
    governance: &Address,
    target_version: u32,
    known_versions: &[u32],
) -> Result<(), RollbackError> {
    governance.require_auth();
    if !known_versions.contains(&target_version) {
        return Err(RollbackError::VersionNotFound);
    }
    env.events()
        .publish((ROLLED_BACK_EVENT,), target_version);
    Ok(())
}

/// Rolls the configuration back to a recorded historical revision, as a **new** revision.
///
/// The restored configuration becomes revision `current_revision + 1`; the historical revision it
/// was copied from is left exactly as it was, and no provenance record is written, edited or
/// removed here. The returned revision and a [`ROLLBACK_PROVENANCE_EVENT`] make the rollback
/// auditable: the event carries the revision that was current, the historical revision that was
/// restored, and the revision that now holds the restored configuration.
pub fn rollback_with_provenance(
    env: &Env,
    governance: &Address,
    current_revision: u32,
    source_revision: u32,
    known_versions: &[u32],
) -> Result<u32, RollbackError> {
    governance.require_auth();
    if !known_versions.contains(&source_revision) {
        return Err(RollbackError::VersionNotFound);
    }
    let resulting_revision = current_revision + 1;
    env.events().publish(
        (ROLLBACK_PROVENANCE_EVENT,),
        (current_revision, source_revision, resulting_revision),
    );
    Ok(resulting_revision)
}

pub fn rollback_config_to_version(
    env: &Env,
    governance: &Address,
    expected_revision: u32,
    target_version: u32,
    history: &Vec<ConfigVersionRecord>,
) -> Result<ConfigVersionRecord, RollbackError> {
    governance.require_auth();
    if expected_revision != current_revision(env) {
        return Err(RollbackError::RevisionConflict);
    }
    let restored = find_version(history, target_version).ok_or(RollbackError::VersionNotFound)?;
    let resulting_revision = expected_revision + 1;
    set_current_revision(env, resulting_revision);
    env.storage()
        .instance()
        .set(&RESTORED_VERSION_KEY, &target_version);
    env.events().publish(
        (ROLLBACK_PROVENANCE_EVENT,),
        (expected_revision, target_version, resulting_revision),
    );
    Ok(restored)
}

pub fn rollback_with_expected_revision(
    env: &Env,
    governance: &Address,
    expected_revision: u32,
    target_version: u32,
    history: &Vec<ConfigVersionRecord>,
) -> Result<ConfigVersionRecord, RollbackError> {
    rollback_config_to_version(env, governance, expected_revision, target_version, history)
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::{
        testutils::{Address as _, Events as _},
        IntoVal,
    };

    const PROV_KEY: Symbol = symbol_short!("PROV");

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SLACalculatorContract);
        let governance = Address::generate(&env);
        (env, id, governance)
    }

    #[test]
    fn rollback_emits_previous_source_and_resulting_revisions() {
        let (env, id, governance) = setup();
        let resulting = env.as_contract(&id, || {
            rollback_with_provenance(&env, &governance, 7, 3, &[1, 2, 3, 4, 5, 6, 7]).unwrap()
        });

        assert_eq!(resulting, 8);
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    id.clone(),
                    (ROLLBACK_PROVENANCE_EVENT,).into_val(&env),
                    (7u32, 3u32, 8u32).into_val(&env)
                )
            ]
        );
    }

    #[test]
    fn rollback_creates_a_new_revision_instead_of_rewriting_history() {
        let (env, id, governance) = setup();
        env.as_contract(&id, || {
            // Stand-in for the historical provenance log.
            env.storage().instance().set(&PROV_KEY, &77u32);

            let resulting =
                rollback_with_provenance(&env, &governance, 5, 2, &[1, 2, 3, 4, 5]).unwrap();

            assert_ne!(resulting, 2);
            assert_eq!(resulting, 6);
            assert_eq!(env.storage().instance().get::<Symbol, u32>(&PROV_KEY), Some(77));
        });
    }

    #[test]
    fn an_unknown_source_revision_is_rejected_without_an_event() {
        let (env, id, governance) = setup();
        let result = env.as_contract(&id, || {
            rollback_with_provenance(&env, &governance, 5, 99, &[1, 2, 3, 4, 5])
        });

        assert_eq!(result, Err(RollbackError::VersionNotFound));
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    #[should_panic]
    fn rollback_requires_governance_authorization() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let governance = Address::generate(&env);
        env.as_contract(&id, || {
            let _ = rollback_with_provenance(&env, &governance, 1, 1, &[1]);
        });
    }
}

#[cfg(test)]
mod expected_revision_tests {
    use super::*;
    use crate::config_history_retention::set_current_revision;
    use soroban_sdk::testutils::{Address as _, Events as _};

    fn setup() -> (Env, soroban_sdk::Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SLACalculatorContract);
        let governance = Address::generate(&env);
        (env, id, governance)
    }

    fn history(env: &Env) -> Vec<ConfigVersionRecord> {
        let mut records = Vec::new(env);
        for version in 1..=5 {
            records.push_back(ConfigVersionRecord {
                version,
                applied_at: version as u64,
            });
        }
        records
    }

    #[test]
    fn stale_expected_revision_conflicts_and_preserves_latest_config() {
        let (env, id, governance) = setup();
        env.as_contract(&id, || {
            set_current_revision(&env, 5);
            env.storage()
                .instance()
                .set(&RESTORED_VERSION_KEY, &4u32);

            let result = rollback_config_to_version(&env, &governance, 4, 2, &history(&env));

            assert_eq!(result, Err(RollbackError::RevisionConflict));
            assert_eq!(current_revision(&env), 5);
            assert_eq!(
                env.storage().instance().get::<Symbol, u32>(&RESTORED_VERSION_KEY),
                Some(4)
            );
            assert_eq!(env.events().all().len(), 0);
        });
    }

    #[test]
    fn current_expected_revision_restores_the_selected_snapshot() {
        let (env, id, governance) = setup();
        env.as_contract(&id, || {
            set_current_revision(&env, 5);

            let restored = rollback_config_to_version(&env, &governance, 5, 2, &history(&env))
                .expect("rollback with the current revision succeeds");

            assert_eq!(restored.version, 2);
            assert_eq!(current_revision(&env), 6);
            assert_eq!(
                env.storage().instance().get::<Symbol, u32>(&RESTORED_VERSION_KEY),
                Some(2)
            );
            assert_eq!(env.events().all().len(), 1);
        });
    }

    #[test]
    fn unknown_target_is_rejected_without_mutating_state() {
        let (env, id, governance) = setup();
        env.as_contract(&id, || {
            set_current_revision(&env, 5);

            let result = rollback_with_expected_revision(&env, &governance, 5, 99, &history(&env));

            assert_eq!(result, Err(RollbackError::VersionNotFound));
            assert_eq!(current_revision(&env), 5);
            assert!(!env.storage().instance().has(&RESTORED_VERSION_KEY));
            assert_eq!(env.events().all().len(), 0);
        });
    }
}
