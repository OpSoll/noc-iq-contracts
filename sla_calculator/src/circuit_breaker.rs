// Global circuit breaker pause toggle. Complements emergency.rs.
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

const CIRCUIT_PAUSED_KEY: Symbol = symbol_short!("CBPAUSE");
/// #1032 — full breaker state, persisted so an upgrade cannot reset a
/// cooldown and admit unsafe calls immediately.
const CIRCUIT_STATE_KEY: Symbol = symbol_short!("CBSTATE");
const PROBE_IN_FLIGHT_KEY: Symbol = symbol_short!("CBPROBE");

/// Persistent state of the circuit breaker.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitState {
    /// Whether the breaker is currently open.
    pub open: bool,
    /// Ledger from which a probe call may be attempted again.
    pub next_probe_ledger: u64,
    /// Length of the cooldown in ledgers.
    pub cooldown_ledgers: u64,
}

impl Default for CircuitState {
    fn default() -> Self {
        Self {
            open: false,
            next_probe_ledger: 0,
            cooldown_ledgers: 0,
        }
    }
}

/// Emergency Guard freezes all state mutations (outage reporting,
/// penalty payouts, dispute filings). Read-only queries are unaffected.
pub fn set_emergency_pause(env: &Env, emergency_guard: &Address, paused: bool) {
    emergency_guard.require_auth();
    env.storage().instance().set(&CIRCUIT_PAUSED_KEY, &paused);
}

pub fn is_emergency_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&CIRCUIT_PAUSED_KEY)
        .unwrap_or(false)
}

/// Open the breaker with a cooldown, recording the next allowed probe ledger.
pub fn open_circuit(
    env: &Env,
    emergency_guard: &Address,
    now_ledger: u64,
    cooldown_ledgers: u64,
) {
    emergency_guard.require_auth();
    let state = CircuitState {
        open: true,
        next_probe_ledger: now_ledger.saturating_add(cooldown_ledgers),
        cooldown_ledgers,
    };
    env.storage().instance().set(&CIRCUIT_STATE_KEY, &state);
    env.storage().instance().set(&CIRCUIT_PAUSED_KEY, &true);
    env.storage().instance().remove(&PROBE_IN_FLIGHT_KEY);
}

/// Close the breaker. The persisted next-probe ledger is retained for audit.
pub fn close_circuit(env: &Env, emergency_guard: &Address) {
    emergency_guard.require_auth();
    let mut state = circuit_state(env);
    state.open = false;
    env.storage().instance().set(&CIRCUIT_STATE_KEY, &state);
    env.storage().instance().set(&CIRCUIT_PAUSED_KEY, &false);
    env.storage().instance().remove(&PROBE_IN_FLIGHT_KEY);
}

/// Read the persisted breaker state.
pub fn circuit_state(env: &Env) -> CircuitState {
    env.storage()
        .instance()
        .get(&CIRCUIT_STATE_KEY)
        .unwrap_or_default()
}

/// #1032 — re-stamp the exact breaker state after a contract upgrade.
pub fn restore_state(env: &Env, state: &CircuitState) {
    env.storage().instance().set(&CIRCUIT_STATE_KEY, state);
    let paused = state.open;
    env.storage().instance().set(&CIRCUIT_PAUSED_KEY, &paused);
}

/// Whether a probe call is allowed at `now_ledger` under the current state.
pub fn probe_allowed(state: &CircuitState, now_ledger: u64) -> bool {
    !state.open || now_ledger >= state.next_probe_ledger
}

pub fn probe_in_flight(env: &Env) -> bool {
    env.storage().instance().has(&PROBE_IN_FLIGHT_KEY)
}

pub fn try_claim_probe(env: &Env, now_ledger: u64) -> bool {
    if !probe_allowed(&circuit_state(env), now_ledger) || probe_in_flight(env) {
        return false;
    }
    env.storage()
        .instance()
        .set(&PROBE_IN_FLIGHT_KEY, &now_ledger);
    true
}

pub fn probe_success(env: &Env, emergency_guard: &Address) {
    close_circuit(env, emergency_guard);
}

pub fn probe_failure(env: &Env, emergency_guard: &Address, now_ledger: u64) {
    let cooldown_ledgers = circuit_state(env).cooldown_ledgers;
    open_circuit(env, emergency_guard, now_ledger, cooldown_ledgers);
}

#[cfg(test)]
mod upgrade_state_tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn cooldown_blocks_probes_until_the_next_probe_ledger() {
        let state = CircuitState {
            open: true,
            next_probe_ledger: 100,
            cooldown_ledgers: 50,
        };
        assert!(!probe_allowed(&state, 99));
        assert!(probe_allowed(&state, 100));
        assert!(probe_allowed(&CircuitState::default(), 0));
    }

    #[test]
    fn only_one_probe_can_be_claimed_after_cooldown() {
        let env = Env::default();
        let contract = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&contract, || {
            let guard = Address::generate(&env);
            open_circuit(&env, &guard, 100, 20);
            assert!(!try_claim_probe(&env, 119));
            assert!(try_claim_probe(&env, 120));
            assert!(probe_in_flight(&env));
            assert!(!try_claim_probe(&env, 120));
            assert!(!try_claim_probe(&env, 200));

            probe_success(&env, &guard);
            assert!(!probe_in_flight(&env));
            assert!(!circuit_state(&env).open);
        });
    }

    #[test]
    fn failed_probe_reopens_for_the_configured_interval() {
        let env = Env::default();
        let contract = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&contract, || {
            let guard = Address::generate(&env);
            open_circuit(&env, &guard, 100, 20);
            assert!(try_claim_probe(&env, 120));
            probe_failure(&env, &guard, 120);

            let state = circuit_state(&env);
            assert!(state.open);
            assert_eq!(state.next_probe_ledger, 140);
            assert_eq!(state.cooldown_ledgers, 20);
            assert!(!probe_in_flight(&env));
            assert!(!try_claim_probe(&env, 139));
            assert!(try_claim_probe(&env, 140));
        });
    }

    #[test]
    fn breaker_state_round_trips_through_storage() {
        let env = Env::default();
        let contract = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&contract, || {
            let guard = Address::generate(&env);
            open_circuit(&env, &guard, 1_000, 500);
            let before = circuit_state(&env);
            assert!(before.open);
            assert_eq!(before.next_probe_ledger, 1_500);

            restore_state(&env, &before);
            let after = circuit_state(&env);
            assert_eq!(after, before);
            assert!(is_emergency_paused(&env));
        });
    }
}
