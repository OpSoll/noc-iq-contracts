// Configuration version hash tracking. Complements config_bundle.rs.
use soroban_sdk::{contracttype, symbol_short, Bytes, BytesN, Env, Symbol};

/// Computes the SHA256 version hash of the active configuration bytes
/// (caller serializes the SLA threshold parameters into `config_bytes`).
pub fn compute_config_version_hash(env: &Env, config_bytes: &Bytes) -> BytesN<32> {
    env.crypto().sha256(config_bytes).into()
}

/// Getter mirroring get_config_version_hash: returns the hash for a
/// given serialized config snapshot.
pub fn get_config_version_hash(env: &Env, config_bytes: &Bytes) -> BytesN<32> {
    compute_config_version_hash(env, config_bytes)
}

pub const CONFIG_REVISION_KEY: Symbol = symbol_short!("CFGREV");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigRevisionSnapshot {
    pub revision: u64,
    pub digest: BytesN<32>,
}

pub fn config_revision_digest(env: &Env, config_bytes: &Bytes) -> BytesN<32> {
    compute_config_version_hash(env, config_bytes)
}

pub fn write_config_revision(env: &Env, revision: u64, digest: &BytesN<32>) {
    env.storage().instance().set(
        &CONFIG_REVISION_KEY,
        &ConfigRevisionSnapshot {
            revision,
            digest: digest.clone(),
        },
    );
}

/// #1021 — outcome of two interleaved revision computations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevisionOutcome {
    /// Both interleavings produced the same configuration revision.
    Converged(BytesN<32>),
    /// The interleavings disagree; the conflict is surfaced rather than
    /// silently picking a winner.
    Conflict,
}

/// #1021 — deterministically reconcile the revision observed by an operator
/// action with the revision observed by a configuration update.
///
/// The function is symmetric, so both action orders yield the same result:
/// either a single converged revision or a documented conflict.
pub fn reconcile_revisions(operator_revision: &BytesN<32>, config_revision: &BytesN<32>) -> RevisionOutcome {
    if operator_revision == config_revision {
        RevisionOutcome::Converged(operator_revision.clone())
    } else {
        RevisionOutcome::Conflict
    }
}

#[cfg(test)]
mod interleave_tests {
    use super::*;
    use soroban_sdk::Bytes;

    #[test]
    fn both_action_orders_converge_when_revisions_match() {
        let env = Env::default();
        let revision = BytesN::from_array(&env, &[7u8; 32]);
        let operator_then_config = reconcile_revisions(&revision, &revision);
        let config_then_operator = reconcile_revisions(&revision, &revision);
        assert_eq!(operator_then_config, config_then_operator);
        assert_eq!(operator_then_config, RevisionOutcome::Converged(revision));
    }

    #[test]
    fn both_action_orders_report_the_same_conflict() {
        let env = Env::default();
        let operator = BytesN::from_array(&env, &[1u8; 32]);
        let config = BytesN::from_array(&env, &[2u8; 32]);
        assert_eq!(reconcile_revisions(&operator, &config), RevisionOutcome::Conflict);
        assert_eq!(reconcile_revisions(&config, &operator), RevisionOutcome::Conflict);
    }

    #[test]
    fn hashed_configuration_is_a_stable_revision() {
        let env = Env::default();
        let bytes = Bytes::from_slice(&env, b"threshold=30;penalty=50");
        assert_eq!(get_config_version_hash(&env, &bytes), compute_config_version_hash(&env, &bytes));
    }
}

#[cfg(test)]
mod digest_stability_tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Address};

    #[test]
    fn change_digest_binds_actor_revision_and_values() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let other = Address::generate(&env);
        let severity = symbol_short!("critical");
        let digest = compute_change_digest(&env, &actor, 3, &severity, 15, 30, 10, 20, 100, 200);

        assert_ne!(
            digest,
            compute_change_digest(&env, &other, 3, &severity, 15, 30, 10, 20, 100, 200)
        );
        assert_ne!(
            digest,
            compute_change_digest(&env, &actor, 4, &severity, 15, 30, 10, 20, 100, 200)
        );
        assert_eq!(
            digest,
            compute_change_digest(&env, &actor, 3, &severity, 15, 30, 10, 20, 100, 200)
        );
    }
}
