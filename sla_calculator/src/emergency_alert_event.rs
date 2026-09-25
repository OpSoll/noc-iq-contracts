// Emergency contact notification event dispatcher: high-priority alert
// event on any emergency circuit activation. Complements emergency.rs.
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

const ALERT_TOPIC: Symbol = symbol_short!("EMALERT");

#[contracttype]
pub struct EmergencyAlert {
    pub trigger_reason_code: u32,
    pub actor: Address,
    pub ledger_sequence: u32,
    pub timestamp: u64,
}

/// Emits a high-priority EmergencyAlert event for instant off-chain
/// webhook filtering via the indexed topic.
pub fn dispatch_emergency_alert(env: &Env, trigger_reason_code: u32, actor: &Address) {
    let alert = EmergencyAlert {
        trigger_reason_code,
        actor: actor.clone(),
        ledger_sequence: env.ledger().sequence(),
        timestamp: env.ledger().timestamp(),
    };
    env.events().publish((ALERT_TOPIC,), alert);
}
