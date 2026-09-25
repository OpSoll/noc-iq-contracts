// Dispute history query index for service providers. Complements
// dispute.rs.
use soroban_sdk::{contracttype, Address, Env, Vec};

#[derive(Clone)]
#[contracttype]
pub enum DisputeOutcome {
    Upheld,
    Dismissed,
    Withdrawn,
}

#[contracttype]
pub struct ProviderDisputeRecord {
    pub dispute_id: u64,
    pub outcome: DisputeOutcome,
    pub financial_impact: i128,
}

/// Paginated getter returning a provider's resolved dispute history.
/// `all_records` is expected to already be scoped to the provider.
pub fn get_provider_disputes(
    env: &Env,
    all_records: &Vec<ProviderDisputeRecord>,
    page: u32,
    page_size: u32,
) -> Vec<ProviderDisputeRecord> {
    let start = (page * page_size) as u32;
    let mut out = Vec::new(env);
    let mut i = start;
    while i < all_records.len() && i < start + page_size {
        out.push_back(all_records.get(i).unwrap());
        i += 1;
    }
    out
}
