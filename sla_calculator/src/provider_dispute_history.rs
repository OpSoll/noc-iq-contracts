// Dispute history query index for service providers. Complements
// dispute.rs.
use soroban_sdk::{contracttype, Env, Vec};

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

#[cfg(test)]
mod tests {
    use super::*;

    /// One resolved dispute, as appending a new dispute would record it.
    fn record(dispute_id: u64, impact: i128) -> ProviderDisputeRecord {
        ProviderDisputeRecord {
            dispute_id,
            outcome: DisputeOutcome::Upheld,
            financial_impact: impact,
        }
    }

    /// #905: each new dispute appends exactly one history entry, and the
    /// appended entries are exactly what the paginated getter returns.
    #[test]
    fn each_new_dispute_appends_exactly_one_entry() {
        let env = Env::default();
        let mut history: Vec<ProviderDisputeRecord> = Vec::new(&env);

        for id in 1..=5u64 {
            history.push_back(record(id, id as i128 * 100));
            assert_eq!(history.len(), id as u32, "exactly one entry per dispute");
        }

        let page = get_provider_disputes(&env, &history, 0, 5);
        assert_eq!(page.len(), 5, "every appended entry is retrievable");
        for i in 0..page.len() {
            let dispute = page.get(i).unwrap();
            assert_eq!(
                dispute.dispute_id,
                i as u64 + 1,
                "insertion order preserved"
            );
        }
    }

    #[test]
    fn appending_preserves_earlier_records_exactly() {
        let env = Env::default();
        let mut history: Vec<ProviderDisputeRecord> = Vec::new(&env);

        history.push_back(record(1, 250));
        history.push_back(record(2, -75)); // a dismissal can cost the provider

        assert_eq!(history.len(), 2, "two disputes, two entries");
        let first = history.get(0).unwrap();
        assert_eq!(first.dispute_id, 1, "earlier entry untouched");
        assert_eq!(first.financial_impact, 250);
        let second = history.get(1).unwrap();
        assert_eq!(second.financial_impact, -75, "signed impact preserved");
        assert!(matches!(second.outcome, DisputeOutcome::Upheld));
    }

    #[test]
    fn paginated_reads_cover_appended_history_without_loss_or_duplication() {
        let env = Env::default();
        let mut history: Vec<ProviderDisputeRecord> = Vec::new(&env);
        for id in 1..=7u64 {
            history.push_back(record(id, 10));
        }

        // Walk every page of 3 over 7 appended records.
        let mut seen: Vec<u64> = Vec::new(&env);
        for page in 0..3u32 {
            let chunk = get_provider_disputes(&env, &history, page, 3);
            for i in 0..chunk.len() {
                seen.push_back(chunk.get(i).unwrap().dispute_id);
            }
        }

        assert_eq!(seen.len(), 7, "no entry lost or duplicated across pages");
        for (i, id) in seen.iter().enumerate() {
            assert_eq!(id, i as u64 + 1, "global order preserved across pages");
        }

        // A page past the end is empty - not an error and not a wrap-around.
        assert_eq!(get_provider_disputes(&env, &history, 3, 3).len(), 0);
        assert_eq!(get_provider_disputes(&env, &history, 100, 3).len(), 0);
    }

    #[test]
    fn an_empty_history_yields_empty_pages() {
        let env = Env::default();
        let history: Vec<ProviderDisputeRecord> = Vec::new(&env);
        assert_eq!(get_provider_disputes(&env, &history, 0, 10).len(), 0);
    }
}
