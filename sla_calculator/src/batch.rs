use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contracttype, symbol_short, Env, Symbol};

use crate::{SLAConfig, SLAError, SLAResult, CONFIG_KEY, OPERATOR_KEY};

// -----------------------------------------------------------------------
// Events (#540)
// -----------------------------------------------------------------------
const EVENT_BATCH_CALC: Symbol = symbol_short!("btch_calc");
const EVENT_VERSION: Symbol = symbol_short!("v1");

pub const XDR_BATCH_ITEM_LIMIT: u32 = 50;
pub const XDR_BATCH_BYTE_BUDGET: u32 = 64 * 1024;
const BATCH_ITEM_HISTORY_KEY: Symbol = symbol_short!("PBHIST");
const BATCH_ITEM_STATS_KEY: Symbol = symbol_short!("PBSTATS");
const OPTIMIZED_HISTORY_KEY: Symbol = symbol_short!("OPHIST");
const OPTIMIZED_COUNT_KEY: Symbol = symbol_short!("OPCOUNT");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Batch calculation request item.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchRequest {
    /// Unique identifier for this outage.
    pub outage_id: Symbol,
    /// Severity tier to evaluate against.
    pub severity: Symbol,
    /// Mean time to resolution in minutes.
    pub mttr_minutes: u32,
}

/// Batch calculation result item.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchResult {
    /// The outage ID.
    pub outage_id: Symbol,
    /// Whether calculation succeeded.
    pub success: bool,
    /// The SLA result (if successful, empty vec if failed).
    pub result: soroban_sdk::Vec<SLAResult>,
    /// Error message (if failed).
    pub error: Option<Symbol>,
}

/// Batch calculation summary.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchSummary {
    /// Total items in batch.
    pub total: u32,
    /// Successfully calculated.
    pub succeeded: u32,
    /// Failed calculations.
    pub failed: u32,
    /// Total rewards from successful calculations.
    pub total_rewards: i128,
    /// Total penalties from successful calculations (signed; negative amounts summed).
    pub total_penalties: i128,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Calculate SLA for multiple outages in a single transaction.
///
/// Processes each item in the batch sequentially. Failed items do not
/// halt the batch; they are recorded as failures in the results.
///
/// # Arguments
/// - `caller`: Must be the operator.
/// - `requests`: List of batch calculation requests.
///
/// # Returns
/// BatchSummary with overall results and individual item outcomes.
///
/// # Events
/// Emits `btch_calc` (#540) with payload
/// `(total_items, met_count, violation_count, total_penalty)`.
pub fn batch_calculate(
    env: &Env,
    caller: &soroban_sdk::Address,
    requests: soroban_sdk::Vec<BatchRequest>,
) -> Result<(BatchSummary, soroban_sdk::Vec<BatchResult>), SLAError> {
    // Validate batch size and contents before processing
    validate_batch(env, &requests)?;
    validate_xdr_budget(env, &requests)?;

    // Verify operator role
    let operator: soroban_sdk::Address = env
        .storage()
        .instance()
        .get(&OPERATOR_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != operator {
        return Err(SLAError::Unauthorized);
    }

    // Check not paused
    let paused: bool = env
        .storage()
        .instance()
        .get(&symbol_short!("PAUSED"))
        .unwrap_or(false);
    if paused {
        return Err(SLAError::ContractPaused);
    }

    let configs: soroban_sdk::Map<Symbol, SLAConfig> = env
        .storage()
        .instance()
        .get(&CONFIG_KEY)
        .ok_or(SLAError::NotInitialized)?;

    let mut results = soroban_sdk::Vec::new(env);
    let mut succeeded: u32 = 0;
    let mut failed: u32 = 0;
    let mut met_count: u32 = 0;
    let mut violation_count: u32 = 0;
    let mut total_rewards: i128 = 0;
    let mut total_penalties: i128 = 0;

    for i in 0..requests.len() {
        let req = requests.get(i).unwrap();

        match process_single(env, &configs, &req) {
            Ok(res) => {
                succeeded = succeeded.saturating_add(1);
                if res.status == symbol_short!("viol") {
                    violation_count = violation_count.saturating_add(1);
                    // amount is negative for penalties; keep signed sum for BatchSummary.
                    total_penalties = total_penalties.saturating_add(res.amount);
                } else {
                    met_count = met_count.saturating_add(1);
                    total_rewards = total_rewards.saturating_add(res.amount);
                }
                let mut res_vec = soroban_sdk::Vec::new(env);
                res_vec.push_back(res);
                results.push_back(BatchResult {
                    outage_id: req.outage_id.clone(),
                    success: true,
                    result: res_vec,
                    error: None,
                });
            }
            Err(e) => {
                failed = failed.saturating_add(1);
                let error_msg = match e {
                    SLAError::ConfigNotFound => symbol_short!("no_config"),
                    SLAError::InvalidSeverity => symbol_short!("bad_sev"),
                    SLAError::InvalidThreshold => symbol_short!("bad_thres"),
                    _ => symbol_short!("unknown"),
                };
                results.push_back(BatchResult {
                    outage_id: req.outage_id.clone(),
                    success: false,
                    result: soroban_sdk::Vec::new(env),
                    error: Some(error_msg),
                });
            }
        }
    }

    let summary = BatchSummary {
        total: requests.len(),
        succeeded,
        failed,
        total_rewards,
        total_penalties,
    };

    // #540 – Emit a single batch_calculated summary event.
    // topics: (btch_calc, v1, caller)
    // data:   (total_items, met_count, violation_count, total_penalty)
    // total_penalty is the absolute (non-negative) sum of penalty amounts.
    let total_items = requests.len();
    let total_penalty_abs = total_penalties.abs();
    env.events().publish(
        (EVENT_BATCH_CALC, EVENT_VERSION, caller),
        (total_items, met_count, violation_count, total_penalty_abs),
    );

    Ok((summary, results))
}

/// Process a single batch item (view-only, no persistence).
pub(crate) fn process_single(
    env: &Env,
    configs: &soroban_sdk::Map<Symbol, SLAConfig>,
    req: &BatchRequest,
) -> Result<SLAResult, SLAError> {
    // Validate severity
    let valid_severities = [
        symbol_short!("critical"),
        symbol_short!("high"),
        symbol_short!("medium"),
        symbol_short!("low"),
    ];
    if !valid_severities.contains(&req.severity) {
        return Err(SLAError::InvalidSeverity);
    }

    // Get config
    let cfg = configs
        .get(req.severity.clone())
        .ok_or(SLAError::ConfigNotFound)?;

    // Validate MTTR
    if req.mttr_minutes == 0 {
        return Err(SLAError::InvalidMTTR);
    }

    // Calculate result
    let threshold = cfg.threshold_minutes;
    if req.mttr_minutes > threshold {
        // Violation
        let overtime = (req.mttr_minutes - threshold) as i128;
        let penalty = overtime.saturating_mul(cfg.penalty_per_minute);
        Ok(SLAResult {
            outage_id: req.outage_id.clone(),
            status: symbol_short!("viol"),
            mttr_minutes: req.mttr_minutes,
            threshold_minutes: threshold,
            amount: -penalty,
            payment_type: symbol_short!("pen"),
            rating: symbol_short!("poor"),
            config_version_hash: crate::compute_config_version_hash(env, configs),
            recorded_at: env.ledger().timestamp(),
        })
    } else {
        // Met
        let performance_ratio = (req.mttr_minutes as i128)
            .saturating_mul(100)
            .div_euclid(threshold as i128);
        let (multiplier, rating) = if performance_ratio < 50 {
            (cfg.top_tier_multiplier, symbol_short!("top"))
        } else if performance_ratio < 75 {
            (cfg.excel_tier_multiplier, symbol_short!("excel"))
        } else {
            (cfg.good_tier_multiplier, symbol_short!("good"))
        };

        let reward = cfg
            .reward_base
            .saturating_mul(multiplier as i128)
            .div_euclid(100);

        Ok(SLAResult {
            outage_id: req.outage_id.clone(),
            status: symbol_short!("met"),
            mttr_minutes: req.mttr_minutes,
            threshold_minutes: threshold,
            amount: reward,
            payment_type: symbol_short!("rew"),
            rating,
            config_version_hash: crate::compute_config_version_hash(env, configs),
            recorded_at: env.ledger().timestamp(),
        })
    }
}

/// Aggregated downtime and compliance for a set of site IDs.
///
/// In this contract, the persisted SLA history uses `outage_id` as the
/// identifier for an SLA downtime record. The batch getter treats each
/// requested site ID as that persisted identifier and aggregates every
/// matching history entry. This keeps the read path side-effect free and
/// avoids one contract read per requested site.
///
/// A site with no matching history is reported with zero downtime and an
/// `unknown` compliance status. A site with matching entries is `compliant`
/// only when every matching entry met its SLA threshold.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchDowntimeResult {
    pub site_id: Symbol,
    pub total_downtime: u64,
    pub compliance_status: Symbol,
}

/// Get aggregated downtime and compliance for up to 50 site IDs.
pub fn get_batch_downtime(
    env: &Env,
    site_ids: &soroban_sdk::Vec<Symbol>,
) -> Result<soroban_sdk::Vec<BatchDowntimeResult>, SLAError> {
    if site_ids.len() > get_batch_limit() {
        return Err(SLAError::ThresholdOutOfBounds);
    }

    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&crate::HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    // Aggregate the retained history once, then answer the requested IDs from
    // in-memory maps. This keeps the getter O(history + batch) rather than
    // scanning the full history once for every requested site.
    let mut downtime_by_site = soroban_sdk::Map::new(env);
    let mut compliant_by_site = soroban_sdk::Map::new(env);

    for i in 0..history.len() {
        let entry = history.get(i).unwrap();
        let current = downtime_by_site
            .get(entry.outage_id.clone())
            .unwrap_or(0u64);
        downtime_by_site.set(
            entry.outage_id.clone(),
            current.saturating_add(entry.mttr_minutes as u64),
        );

        let previous_compliance = compliant_by_site
            .get(entry.outage_id.clone())
            .unwrap_or(true);
        compliant_by_site.set(
            entry.outage_id,
            previous_compliance && entry.status == symbol_short!("met"),
        );
    }

    let mut results = soroban_sdk::Vec::new(env);

    for i in 0..site_ids.len() {
        let site_id = site_ids.get(i).unwrap();

        let total_downtime = downtime_by_site.get(site_id.clone()).unwrap_or(0u64);

        let compliance_status = match compliant_by_site.get(site_id.clone()) {
            None => symbol_short!("unknown"),
            Some(true) => symbol_short!("compliant"),
            Some(false) => symbol_short!("noncomp"),
        };

        results.push_back(BatchDowntimeResult {
            site_id,
            total_downtime,
            compliance_status,
        });
    }

    Ok(results)
}

/// Get batch size limit (maximum items per batch).
pub fn get_batch_limit() -> u32 {
    50 // Reasonable limit for Soroban transaction budget
}

/// Validate a batch request before submission.
pub fn validate_batch(
    env: &Env,
    requests: &soroban_sdk::Vec<BatchRequest>,
) -> Result<u32, SLAError> {
    if requests.is_empty() {
        return Err(SLAError::ThresholdOutOfBounds);
    }

    if requests.len() > get_batch_limit() {
        return Err(SLAError::ThresholdOutOfBounds);
    }

    // Check for duplicate outage IDs and validate severity symbols (#542)
    let mut seen = soroban_sdk::Map::new(env);
    let valid_severities = [
        symbol_short!("critical"),
        symbol_short!("high"),
        symbol_short!("medium"),
        symbol_short!("low"),
    ];
    for i in 0..requests.len() {
        let req = requests.get(i).unwrap();
        if seen.get(req.outage_id.clone()).unwrap_or(false) {
            return Err(SLAError::DuplicateOutageInput);
        }
        seen.set(req.outage_id.clone(), true);

        if !valid_severities.contains(&req.severity) {
            return Err(SLAError::InvalidSeverity);
        }
    }

    Ok(requests.len())
}

/// Find a specific result by outage ID.
pub fn find_result_by_outage_id(
    results: &soroban_sdk::Vec<BatchResult>,
    outage_id: &soroban_sdk::Symbol,
) -> Option<BatchResult> {
    for i in 0..results.len() {
        if let Some(res) = results.get(i) {
            if res.outage_id == *outage_id {
                return Some(res);
            }
        }
    }
    None
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchItemInput {
    pub id: u64,
    pub value: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchItemOutput {
    pub id: u64,
    pub result: u64,
    pub error: Option<Symbol>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchProcessResult {
    pub results: soroban_sdk::Vec<BatchItemOutput>,
    pub processed_count: u32,
    pub has_failed: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchProcessStats {
    pub processed: u32,
    pub failed: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchItemError {
    pub code: Symbol,
    pub index: u32,
}

pub struct BatchCalculator;

impl BatchCalculator {
    pub fn process_batch(
        env: &Env,
        items: soroban_sdk::Vec<BatchItemInput>,
        strict_mode: bool,
    ) -> Result<BatchProcessResult, BatchItemError> {
        if items.len() > XDR_BATCH_ITEM_LIMIT {
            return Err(BatchItemError {
                code: Symbol::new(env, "BatchXdrBudgetExceeded"),
                index: XDR_BATCH_ITEM_LIMIT,
            });
        }

        let mut results = soroban_sdk::Vec::with_capacity(env, items.len());
        let mut failed = 0u32;
        for i in 0..items.len() {
            let item = items.get(i).unwrap();
            if item.value == 0 {
                let code = if strict_mode {
                    Symbol::new(env, "InvalidItemStrictAbort")
                } else {
                    Symbol::new(env, "InvalidItem")
                };
                if strict_mode {
                    return Err(BatchItemError { code, index: i });
                }
                failed = failed.saturating_add(1);
                results.push_back(BatchItemOutput {
                    id: item.id,
                    result: 0,
                    error: Some(code),
                });
            } else {
                results.push_back(BatchItemOutput {
                    id: item.id,
                    result: item.value.saturating_mul(2),
                    error: None,
                });
            }
        }

        Ok(BatchProcessResult {
            results,
            processed_count: items.len(),
            has_failed: failed > 0,
        })
    }

    pub fn process_batch_with_commit(
        env: &Env,
        items: soroban_sdk::Vec<BatchItemInput>,
        strict_mode: bool,
    ) -> Result<BatchProcessResult, BatchItemError> {
        let result = Self::process_batch(env, items, strict_mode)?;
        let stats = BatchProcessStats {
            processed: result.processed_count,
            failed: result.results.iter().filter(|item| item.error.is_some()).count() as u32,
        };
        env.storage()
            .instance()
            .set(&BATCH_ITEM_HISTORY_KEY, &result.results);
        env.storage().instance().set(&BATCH_ITEM_STATS_KEY, &stats);
        Ok(result)
    }
}

pub struct BatchResultAdapter;

impl BatchResultAdapter {
    pub fn create_error_vector(env: &Env, error: Option<Symbol>) -> soroban_sdk::Vec<Symbol> {
        let mut errors = soroban_sdk::Vec::new(env);
        if let Some(code) = error {
            errors.push_back(code);
        }
        errors
    }
}

pub fn validate_xdr_budget(
    env: &Env,
    requests: &soroban_sdk::Vec<BatchRequest>,
) -> Result<u32, SLAError> {
    if requests.len() > XDR_BATCH_ITEM_LIMIT {
        return Err(SLAError::ThresholdOutOfBounds);
    }
    let encoded = requests.clone().to_xdr(env);
    if encoded.len() > XDR_BATCH_BYTE_BUDGET {
        return Err(SLAError::ThresholdOutOfBounds);
    }
    Ok(requests.len())
}

pub fn batch_process_history(env: &Env) -> soroban_sdk::Vec<BatchItemOutput> {
    env.storage()
        .instance()
        .get(&BATCH_ITEM_HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

pub fn batch_process_stats(env: &Env) -> BatchProcessStats {
    env.storage()
        .instance()
        .get(&BATCH_ITEM_STATS_KEY)
        .unwrap_or(BatchProcessStats {
            processed: 0,
            failed: 0,
        })
}

pub fn process_reference_batch(
    env: &Env,
    items: &soroban_sdk::Vec<crate::batch_optimized::OptimizedBatchItem>,
) -> soroban_sdk::Vec<crate::batch_optimized::OptimizedBatchItem> {
    let mut results = soroban_sdk::Vec::new(env);
    for item in items.iter() {
        results.push_back(crate::batch_optimized::OptimizedBatchItem {
            id: item.id,
            score: item.score.saturating_mul(2),
        });
    }
    results
}

pub fn process_reference_batch_with_effects(
    env: &Env,
    items: &soroban_sdk::Vec<crate::batch_optimized::OptimizedBatchItem>,
) -> soroban_sdk::Vec<crate::batch_optimized::OptimizedBatchItem> {
    let results = process_reference_batch(env, items);
    commit_optimized_results(env, &results);
    results
}

pub fn commit_optimized_results(
    env: &Env,
    results: &soroban_sdk::Vec<crate::batch_optimized::OptimizedBatchItem>,
) {
    let mut history: soroban_sdk::Vec<crate::batch_optimized::OptimizedBatchItem> = env
        .storage()
        .instance()
        .get(&OPTIMIZED_HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    for item in results.iter() {
        history.push_back(item);
    }
    env.storage()
        .instance()
        .set(&OPTIMIZED_HISTORY_KEY, &history);
    let count: u32 = env
        .storage()
        .instance()
        .get(&OPTIMIZED_COUNT_KEY)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&OPTIMIZED_COUNT_KEY, &count.saturating_add(results.len()));
}

pub fn optimized_batch_effects(
    env: &Env,
) -> (
    soroban_sdk::Vec<crate::batch_optimized::OptimizedBatchItem>,
    u32,
) {
    let history = env
        .storage()
        .instance()
        .get(&OPTIMIZED_HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    let count = env
        .storage()
        .instance()
        .get(&OPTIMIZED_COUNT_KEY)
        .unwrap_or(0u32);
    (history, count)
}
