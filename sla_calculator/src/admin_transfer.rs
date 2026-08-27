use soroban_sdk::{symbol_short, Address, Env, Symbol};

const PENDING_ADMIN_KEY: Symbol = symbol_short!("PADMIN");
const PENDING_ADMIN_TS_KEY: Symbol = symbol_short!("PADMINTS");
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const ADMIN_PROPOSED_EVENT: Symbol = symbol_short!("adm_prop");
const ADMIN_ACCEPTED_EVENT: Symbol = symbol_short!("adm_acpt");
const PROPOSAL_EXPIRATION_SECONDS: u64 = 604_800;

#[derive(Debug, Clone, PartialEq)]
pub enum AdminTransferError {
    ProposalExpired,
    NoPendingProposal,
}

/// Current admin proposes a new admin address.
pub fn propose_admin(env: &Env, admin: &Address, new_admin: Address) {
    admin.require_auth();
    env.storage().instance().set(&PENDING_ADMIN_KEY, &new_admin);
    env.storage()
        .instance()
        .set(&PENDING_ADMIN_TS_KEY, &env.ledger().timestamp());
    env.events().publish((ADMIN_PROPOSED_EVENT,), new_admin);
}

/// The proposed admin accepts the role within the expiration window.
pub fn accept_admin(env: &Env, new_admin: &Address) -> Result<(), AdminTransferError> {
    new_admin.require_auth();
    let proposed_at: u64 = env
        .storage()
        .instance()
        .get(&PENDING_ADMIN_TS_KEY)
        .ok_or(AdminTransferError::NoPendingProposal)?;
    if env.ledger().timestamp() > proposed_at + PROPOSAL_EXPIRATION_SECONDS {
        return Err(AdminTransferError::ProposalExpired);
    }
    env.storage().instance().set(&ADMIN_KEY, new_admin);
    env.storage().instance().remove(&PENDING_ADMIN_KEY);
    env.storage().instance().remove(&PENDING_ADMIN_TS_KEY);
    env.events()
        .publish((ADMIN_ACCEPTED_EVENT,), new_admin.clone());
    Ok(())
}
