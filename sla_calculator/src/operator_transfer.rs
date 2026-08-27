use soroban_sdk::{symbol_short, Address, Env, Symbol};

const OPERATOR_KEY: Symbol = symbol_short!("OPERATOR");
const PENDING_OP_KEY: Symbol = symbol_short!("POP");

/// Admin proposes a new operator; must be accepted before taking effect.
pub fn propose_operator(env: &Env, admin: &Address, new_operator: Address) {
    admin.require_auth();
    env.storage().instance().set(&PENDING_OP_KEY, &new_operator);
}

/// The proposed operator accepts the role, replacing the current operator.
pub fn accept_operator(env: &Env, new_operator: &Address) {
    new_operator.require_auth();
    env.storage().instance().set(&OPERATOR_KEY, new_operator);
    env.storage().instance().remove(&PENDING_OP_KEY);
}

/// Admin cancels a pending operator proposal before it is accepted.
pub fn cancel_operator_proposal(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().remove(&PENDING_OP_KEY);
}
