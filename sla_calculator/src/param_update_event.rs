// Parameter update event stream logger. Complements config_provenance.rs.
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

const PARAM_UPDATED_TOPIC: Symbol = symbol_short!("paramupd");

#[contracttype]
pub struct ParameterUpdated {
    pub param_key: Symbol,
    pub old_value: i128,
    pub new_value: i128,
    pub admin: Address,
}

/// Emits a structured ParameterUpdated event so off-chain indexers can
/// build a clean parameter audit log without parsing raw XDR.
pub fn emit_parameter_updated(
    env: &Env,
    param_key: Symbol,
    old_value: i128,
    new_value: i128,
    admin: &Address,
) {
    let event = ParameterUpdated {
        param_key,
        old_value,
        new_value,
        admin: admin.clone(),
    };
    env.events().publish((PARAM_UPDATED_TOPIC,), event);
}
