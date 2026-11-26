// Deprecation support.
//
// 1. Contract deprecation shutdown protocol: puts the contract into a
//    permanent read-only retirement mode. Complements emergency.rs.
// 2. Deprecated configuration fields (#1014): a legacy field and its
//    replacement are governed by a ledger-based compatibility window so that
//    upgrades cannot accept a legacy field indefinitely. See
//    `docs/schema-evolution.md` ("Deprecated Configuration Fields") for the
//    exact boundary table.
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DeprecationError {
    ContractDeprecated = 1,
    /// `activation_ledger` must be strictly lower than `removal_ledger`.
    InvalidWindow = 2,
    /// A legacy field was supplied at or after its removal ledger.
    LegacyFieldRemoved = 3,
    /// A replacement field was supplied before its activation ledger.
    ReplacementNotActive = 4,
    /// No deprecation window is registered for the field.
    WindowNotFound = 5,
}

const DEPRECATED_KEY: Symbol = symbol_short!("DEPRCTD");
const FIELD_WINDOW_PREFIX: Symbol = symbol_short!("DEPFLD");

/// Admin-only: permanently deprecates the contract. Mutating calls must
/// call `assert_not_deprecated` first; read-only queries remain usable.
pub fn deprecate_contract(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&DEPRECATED_KEY, &true);
}

pub fn is_deprecated(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DEPRECATED_KEY)
        .unwrap_or(false)
}

/// Call at the top of any state-mutating entrypoint.
pub fn assert_not_deprecated(env: &Env) -> Result<(), DeprecationError> {
    if is_deprecated(env) {
        Err(DeprecationError::ContractDeprecated)
    } else {
        Ok(())
    }
}

// -----------------------------------------------------------------------
// Deprecated configuration fields (#1014)
// -----------------------------------------------------------------------

/// Ledger boundaries for one deprecated configuration field.
///
/// * `activation_ledger`: first ledger at which the deprecation is in force
///   (the replacement field is accepted, the legacy field is accepted but
///   reported as deprecated). Inclusive.
/// * `removal_ledger`: first ledger at which the legacy field is rejected.
///   The compatibility window is therefore `[activation_ledger, removal_ledger)`.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeprecationWindow {
    pub activation_ledger: u32,
    pub removal_ledger: u32,
}

/// Which spelling of a configuration field a caller supplied.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Legacy,
    Replacement,
}

/// Phase of a deprecated field at a given ledger.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeprecationPhase {
    /// `ledger < activation_ledger`: legacy field is the current field.
    PreActivation,
    /// `activation_ledger <= ledger < removal_ledger`: both fields accepted,
    /// legacy reported as deprecated.
    CompatibilityWindow,
    /// `ledger >= removal_ledger`: legacy field rejected.
    Removed,
}

/// Outcome of an accepted field usage.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldAcceptance {
    /// Field accepted with no deprecation notice.
    Accepted,
    /// Legacy field accepted inside the compatibility window; callers should
    /// migrate before `removal_ledger`.
    AcceptedDeprecated,
}

impl DeprecationWindow {
    pub fn new(activation_ledger: u32, removal_ledger: u32) -> Result<Self, DeprecationError> {
        let window = DeprecationWindow {
            activation_ledger,
            removal_ledger,
        };
        window.validate()?;
        Ok(window)
    }

    /// A window must be non-empty: `activation_ledger < removal_ledger`.
    pub fn validate(&self) -> Result<(), DeprecationError> {
        if self.activation_ledger < self.removal_ledger {
            Ok(())
        } else {
            Err(DeprecationError::InvalidWindow)
        }
    }

    pub fn phase_at(&self, ledger: u32) -> DeprecationPhase {
        if ledger < self.activation_ledger {
            DeprecationPhase::PreActivation
        } else if ledger < self.removal_ledger {
            DeprecationPhase::CompatibilityWindow
        } else {
            DeprecationPhase::Removed
        }
    }

    /// Decide whether `kind` may be supplied at `ledger`.
    pub fn check_at(
        &self,
        ledger: u32,
        kind: FieldKind,
    ) -> Result<FieldAcceptance, DeprecationError> {
        match (self.phase_at(ledger), kind) {
            (DeprecationPhase::PreActivation, FieldKind::Legacy) => Ok(FieldAcceptance::Accepted),
            (DeprecationPhase::PreActivation, FieldKind::Replacement) => {
                Err(DeprecationError::ReplacementNotActive)
            }
            (DeprecationPhase::CompatibilityWindow, FieldKind::Legacy) => {
                Ok(FieldAcceptance::AcceptedDeprecated)
            }
            (DeprecationPhase::CompatibilityWindow, FieldKind::Replacement) => {
                Ok(FieldAcceptance::Accepted)
            }
            (DeprecationPhase::Removed, FieldKind::Legacy) => {
                Err(DeprecationError::LegacyFieldRemoved)
            }
            (DeprecationPhase::Removed, FieldKind::Replacement) => Ok(FieldAcceptance::Accepted),
        }
    }

    /// Ledgers left before the legacy field is rejected (0 once removed).
    pub fn ledgers_until_removal(&self, ledger: u32) -> u32 {
        self.removal_ledger.saturating_sub(ledger)
    }
}

fn field_key(field: &Symbol) -> (Symbol, Symbol) {
    (FIELD_WINDOW_PREFIX, field.clone())
}

/// Admin-only: register (or replace) the deprecation window for `field`.
pub fn set_field_deprecation_window(
    env: &Env,
    admin: &Address,
    field: Symbol,
    window: DeprecationWindow,
) -> Result<(), DeprecationError> {
    admin.require_auth();
    window.validate()?;
    env.storage().instance().set(&field_key(&field), &window);
    Ok(())
}

pub fn get_field_deprecation_window(env: &Env, field: Symbol) -> Option<DeprecationWindow> {
    env.storage().instance().get(&field_key(&field))
}

/// Check a field usage against its registered window at the current ledger
/// sequence. Call before applying a configuration value that has a legacy
/// spelling.
pub fn check_field_usage(
    env: &Env,
    field: Symbol,
    kind: FieldKind,
) -> Result<FieldAcceptance, DeprecationError> {
    let window =
        get_field_deprecation_window(env, field).ok_or(DeprecationError::WindowNotFound)?;
    window.check_at(env.ledger().sequence(), kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::{Address as _, Ledger as _};

    const ACTIVATION: u32 = 1_000;
    const REMOVAL: u32 = 2_000;

    fn field() -> Symbol {
        symbol_short!("penalty")
    }

    fn harness() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        env.as_contract(&cid, || {
            set_field_deprecation_window(
                &env,
                &admin,
                field(),
                DeprecationWindow::new(ACTIVATION, REMOVAL).unwrap(),
            )
            .unwrap();
        });
        (env, cid, admin)
    }

    fn at(
        env: &Env,
        cid: &Address,
        ledger: u32,
        kind: FieldKind,
    ) -> Result<FieldAcceptance, DeprecationError> {
        env.ledger().set_sequence_number(ledger);
        env.as_contract(cid, || check_field_usage(env, field(), kind))
    }

    #[test]
    fn activation_boundary_minus_one_at_plus_one() {
        let (env, cid, _) = harness();
        // activation - 1: still pre-activation.
        assert_eq!(
            at(&env, &cid, ACTIVATION - 1, FieldKind::Legacy),
            Ok(FieldAcceptance::Accepted)
        );
        assert_eq!(
            at(&env, &cid, ACTIVATION - 1, FieldKind::Replacement),
            Err(DeprecationError::ReplacementNotActive)
        );
        // activation: window opens (inclusive).
        assert_eq!(
            at(&env, &cid, ACTIVATION, FieldKind::Legacy),
            Ok(FieldAcceptance::AcceptedDeprecated)
        );
        assert_eq!(
            at(&env, &cid, ACTIVATION, FieldKind::Replacement),
            Ok(FieldAcceptance::Accepted)
        );
        // activation + 1: inside the window.
        assert_eq!(
            at(&env, &cid, ACTIVATION + 1, FieldKind::Legacy),
            Ok(FieldAcceptance::AcceptedDeprecated)
        );
        assert_eq!(
            at(&env, &cid, ACTIVATION + 1, FieldKind::Replacement),
            Ok(FieldAcceptance::Accepted)
        );
    }

    #[test]
    fn removal_boundary_minus_one_at_plus_one() {
        let (env, cid, _) = harness();
        // removal - 1: last ledger the legacy field is accepted.
        assert_eq!(
            at(&env, &cid, REMOVAL - 1, FieldKind::Legacy),
            Ok(FieldAcceptance::AcceptedDeprecated)
        );
        assert_eq!(
            at(&env, &cid, REMOVAL - 1, FieldKind::Replacement),
            Ok(FieldAcceptance::Accepted)
        );
        // removal: legacy rejected (exclusive upper bound).
        assert_eq!(
            at(&env, &cid, REMOVAL, FieldKind::Legacy),
            Err(DeprecationError::LegacyFieldRemoved)
        );
        assert_eq!(
            at(&env, &cid, REMOVAL, FieldKind::Replacement),
            Ok(FieldAcceptance::Accepted)
        );
        // removal + 1: still rejected.
        assert_eq!(
            at(&env, &cid, REMOVAL + 1, FieldKind::Legacy),
            Err(DeprecationError::LegacyFieldRemoved)
        );
        assert_eq!(
            at(&env, &cid, REMOVAL + 1, FieldKind::Replacement),
            Ok(FieldAcceptance::Accepted)
        );
    }

    #[test]
    fn phases_match_boundaries() {
        let w = DeprecationWindow::new(ACTIVATION, REMOVAL).unwrap();
        assert_eq!(w.phase_at(ACTIVATION - 1), DeprecationPhase::PreActivation);
        assert_eq!(
            w.phase_at(ACTIVATION),
            DeprecationPhase::CompatibilityWindow
        );
        assert_eq!(
            w.phase_at(ACTIVATION + 1),
            DeprecationPhase::CompatibilityWindow
        );
        assert_eq!(
            w.phase_at(REMOVAL - 1),
            DeprecationPhase::CompatibilityWindow
        );
        assert_eq!(w.phase_at(REMOVAL), DeprecationPhase::Removed);
        assert_eq!(w.phase_at(REMOVAL + 1), DeprecationPhase::Removed);
        assert_eq!(w.phase_at(u32::MAX), DeprecationPhase::Removed);
    }

    #[test]
    fn ledgers_until_removal_saturates() {
        let w = DeprecationWindow::new(ACTIVATION, REMOVAL).unwrap();
        assert_eq!(w.ledgers_until_removal(REMOVAL - 1), 1);
        assert_eq!(w.ledgers_until_removal(REMOVAL), 0);
        assert_eq!(w.ledgers_until_removal(REMOVAL + 1), 0);
        assert_eq!(w.ledgers_until_removal(u32::MAX), 0);
    }

    #[test]
    fn empty_or_inverted_window_is_rejected() {
        assert_eq!(
            DeprecationWindow::new(REMOVAL, REMOVAL),
            Err(DeprecationError::InvalidWindow)
        );
        assert_eq!(
            DeprecationWindow::new(REMOVAL, ACTIVATION),
            Err(DeprecationError::InvalidWindow)
        );
        let (env, cid, admin) = harness();
        env.as_contract(&cid, || {
            let bad = DeprecationWindow {
                activation_ledger: 5,
                removal_ledger: 5,
            };
            assert_eq!(
                set_field_deprecation_window(&env, &admin, field(), bad),
                Err(DeprecationError::InvalidWindow)
            );
            // The previously registered window is left untouched.
            assert_eq!(
                get_field_deprecation_window(&env, field()),
                Some(DeprecationWindow::new(ACTIVATION, REMOVAL).unwrap())
            );
        });
    }

    #[test]
    fn unregistered_field_is_rejected() {
        let (env, cid, _) = harness();
        env.as_contract(&cid, || {
            assert_eq!(
                check_field_usage(&env, symbol_short!("other"), FieldKind::Legacy),
                Err(DeprecationError::WindowNotFound)
            );
        });
    }

    #[test]
    fn contract_retirement_still_works() {
        let (env, cid, admin) = harness();
        env.as_contract(&cid, || {
            assert_eq!(assert_not_deprecated(&env), Ok(()));
            deprecate_contract(&env, &admin);
            assert!(is_deprecated(&env));
            assert_eq!(
                assert_not_deprecated(&env),
                Err(DeprecationError::ContractDeprecated)
            );
        });
    }
}
