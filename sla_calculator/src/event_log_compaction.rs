use soroban_sdk::{contracttype, symbol_short, Env, Map, String, Symbol};

pub const COMPACTED_EVENT: Symbol = symbol_short!("compact");
pub const SUMMARY_BYTES: u32 = 32;
pub const SUMMARY_KEY: Symbol = symbol_short!("osum");
pub const DETAIL_KEY: Symbol = symbol_short!("odtl");

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct DetailedOutageRecord {
    pub outage_id: Symbol,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub severity: u32,
    pub description: String,
    pub metadata: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct OutageSummary {
    pub outage_id: Symbol,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub severity: u32,
}

pub fn compact(record: &DetailedOutageRecord) -> OutageSummary {
    OutageSummary {
        outage_id: record.outage_id.clone(),
        start_timestamp: record.start_timestamp,
        end_timestamp: record.end_timestamp,
        severity: record.severity,
    }
}

pub fn summary_bytes() -> u32 {
    SUMMARY_BYTES
}

pub fn verbose_bytes(record: &DetailedOutageRecord) -> u32 {
    let fixed: u32 = 4 + 8 + 8 + 4;
    let strings: u32 = 4 + record.description.len() + 4 + record.metadata.len();
    fixed.saturating_add(strings)
}

pub fn byte_reduction(record: &DetailedOutageRecord) -> u32 {
    verbose_bytes(record).saturating_sub(SUMMARY_BYTES)
}

fn summary_map(env: &Env) -> Map<Symbol, OutageSummary> {
    env.storage()
        .persistent()
        .get(&SUMMARY_KEY)
        .unwrap_or_else(|| Map::new(env))
}

fn detail_map(env: &Env) -> Map<Symbol, DetailedOutageRecord> {
    env.storage()
        .persistent()
        .get(&DETAIL_KEY)
        .unwrap_or_else(|| Map::new(env))
}

pub fn store_summary(env: &Env, summary: &OutageSummary) {
    let mut summaries = summary_map(env);
    summaries.set(summary.outage_id.clone(), summary.clone());
    env.storage().persistent().set(&SUMMARY_KEY, &summaries);
    env.events().publish(
        (COMPACTED_EVENT, summary.outage_id.clone()),
        summary_bytes(),
    );
}

pub fn load_summary(env: &Env, outage_id: &Symbol) -> Option<OutageSummary> {
    summary_map(env).get(outage_id.clone())
}

pub fn store_detailed(env: &Env, record: &DetailedOutageRecord) {
    let mut details = detail_map(env);
    details.set(record.outage_id.clone(), record.clone());
    env.storage().persistent().set(&DETAIL_KEY, &details);
}

pub fn load_detailed(env: &Env, outage_id: &Symbol) -> Option<DetailedOutageRecord> {
    detail_map(env).get(outage_id.clone())
}

pub fn evict_details(env: &Env, outage_id: &Symbol) -> bool {
    let mut details = detail_map(env);
    if !details.contains_key(outage_id.clone()) {
        return false;
    }
    details.remove(outage_id.clone());
    env.storage().persistent().set(&DETAIL_KEY, &details);
    true
}

pub fn compact_outage(env: &Env, record: &DetailedOutageRecord) -> OutageSummary {
    let summary = compact(record);
    store_summary(env, &summary);
    evict_details(env, &record.outage_id);
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn contract(env: &Env) -> soroban_sdk::Address {
        env.register_contract(None, SLACalculatorContract)
    }

    fn record(env: &Env, description: &str) -> DetailedOutageRecord {
        DetailedOutageRecord {
            outage_id: Symbol::new(env, "OUT1"),
            start_timestamp: 1_000,
            end_timestamp: 4_600,
            severity: 3,
            description: String::from_str(env, description),
            metadata: String::from_str(env, "region=eu-west"),
        }
    }

    #[test]
    fn compaction_preserves_the_audit_trail() {
        let env = Env::default();
        let detailed = record(&env, "gateway timeouts");
        let summary = compact(&detailed);
        assert_eq!(summary.outage_id, detailed.outage_id);
        assert_eq!(summary.start_timestamp, 1_000);
        assert_eq!(summary.end_timestamp, 4_600);
        assert_eq!(summary.severity, 3);
    }

    #[test]
    fn a_summary_fits_the_thirty_two_byte_budget() {
        assert_eq!(summary_bytes(), 32);
    }

    #[test]
    fn compaction_reduces_storage_bytes() {
        let env = Env::default();
        let detailed = record(
            &env,
            "gateway timeouts across the eu-west ingress tier requiring a rolling restart",
        );
        let before = verbose_bytes(&detailed);
        assert!(before > summary_bytes());
        assert_eq!(byte_reduction(&detailed), before - summary_bytes());
    }

    #[test]
    fn savings_grow_with_the_verbose_metadata() {
        let env = Env::default();
        let short = record(&env, "brief");
        let long = record(&env, "a considerably longer description of the same outage");
        assert!(byte_reduction(&long) > byte_reduction(&short));
    }

    #[test]
    fn verbose_size_accounts_for_both_strings() {
        let env = Env::default();
        let detailed = record(&env, "12345");
        let expected = 4 + 8 + 8 + 4 + 4 + 5 + 4 + detailed.metadata.len();
        assert_eq!(verbose_bytes(&detailed), expected);
    }

    #[test]
    fn a_stored_summary_can_be_read_back() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let detailed = record(&env, "gateway timeouts");
            let summary = compact_outage(&env, &detailed);
            let stored = load_summary(&env, &detailed.outage_id).expect("summary stored");
            assert_eq!(stored, summary);
        });
    }

    #[test]
    fn compaction_evicts_the_verbose_record() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let detailed = record(&env, "gateway timeouts");
            store_detailed(&env, &detailed);
            assert!(load_detailed(&env, &detailed.outage_id).is_some());
            compact_outage(&env, &detailed);
            assert!(load_detailed(&env, &detailed.outage_id).is_none());
            assert!(load_summary(&env, &detailed.outage_id).is_some());
        });
    }

    #[test]
    fn evicting_a_missing_record_is_harmless() {
        let env = Env::default();
        let evicted = env.as_contract(&contract(&env), || {
            evict_details(&env, &Symbol::new(&env, "OUT9"))
        });
        assert!(!evicted);
    }
}
