// Emergency contact notification event dispatcher: high-priority alert
// event on any emergency circuit activation. Complements emergency.rs.
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

const ALERT_TOPIC: Symbol = symbol_short!("EMALERT");
const DEFAULT_ALERT_SEVERITY: u32 = 1;

#[contracttype]
pub struct EmergencyAlert {
    pub trigger_reason_code: u32,
    pub actor: Address,
    pub ledger_sequence: u32,
    pub timestamp: u64,
    pub severity: u32,
}

/// Emits a high-priority EmergencyAlert event for instant off-chain
/// webhook filtering via the indexed topic.
pub fn dispatch_emergency_alert(env: &Env, trigger_reason_code: u32, actor: &Address) {
    dispatch_emergency_alert_with_severity(env, trigger_reason_code, actor, DEFAULT_ALERT_SEVERITY);
}

/// Same as `dispatch_emergency_alert`, with an explicit severity for triage.
pub fn dispatch_emergency_alert_with_severity(
    env: &Env,
    trigger_reason_code: u32,
    actor: &Address,
    severity: u32,
) {
    let alert = EmergencyAlert {
        trigger_reason_code,
        actor: actor.clone(),
        ledger_sequence: env.ledger().sequence(),
        timestamp: env.ledger().timestamp(),
        severity,
    };
    env.events().publish((ALERT_TOPIC,), alert);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::{Address as _, Events as _};

    #[test]
    fn alert_payload_has_the_expected_fields() {
        let env = Env::default();
        let alert = EmergencyAlert {
            trigger_reason_code: 7u32,
            actor: Address::generate(&env),
            ledger_sequence: 1u32,
            timestamp: 2u64,
            severity: 3u32,
        };
        assert_eq!(alert.trigger_reason_code, 7u32);
        assert_eq!(alert.ledger_sequence, 1u32);
        assert_eq!(alert.timestamp, 2u64);
        assert_eq!(alert.severity, 3u32);
    }

    #[test]
    fn dispatch_emits_one_alert_event() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let actor = Address::generate(&env);
            dispatch_emergency_alert_with_severity(&env, 9, &actor, 2);
            assert_eq!(env.events().all().len(), 1);
        });
    }
}
