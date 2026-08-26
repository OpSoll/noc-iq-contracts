use soroban_sdk::{contracttype, symbol_short, Env, Symbol};

use crate::{SLAConfig, SLAError, SLAResult, CONFIG_KEY, OPERATOR_KEY};

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
    /// Total penalties from successful calculations.
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
pub fn batch_calculate(
    env: &Env,
    caller: &soroban_sdk::Address,
    requests: soroban_sdk::Vec<BatchRequest>,
) -> Result<(BatchSummary, soroban_sdk::Vec<BatchResult>), SLAError> {
    // Validate batch size and contents before processing
    validate_batch(env, &requests)?;

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
    let mut total_rewards: i128 = 0;
    let mut total_penalties: i128 = 0;

    for i in 0..requests.len() {
        let req = requests.get(i).unwrap();

        // Try to calculate
        match process_single(env, &configs, &req) {
            Ok(res) => {
                succeeded = succeeded.saturating_add(1);
                if res.status == symbol_short!("viol") {
                    total_penalties = total_penalties.saturating_add(res.amount);
                } else {
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
            config_version_hash: 0,
            recorded_at: env.ledger().timestamp(),
        })
    } else {
        // Met
        let performance_ratio =
            (req.mttr_minutes as i128).saturating_mul(100).div_euclid(threshold as i128);
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
            config_version_hash: 0,
            recorded_at: env.ledger().timestamp(),
        })
    }
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

    // Check for duplicate outage IDs
    let mut seen = soroban_sdk::Map::new(env);
    for i in 0..requests.len() {
        let req = requests.get(i).unwrap();
        if seen.get(req.outage_id.clone()).unwrap_or(false) {
            return Err(SLAError::DuplicateOutageInput);
        }
        seen.set(req.outage_id, true);
/// - `requests`: List of batch calculation requests.
///
/// # Returns
/// BatchSummary with overall results and individual item outcomes.
pub fn batch_calculate(
    env: &Env,
    caller: &soroban_sdk::Address,
    requests: soroban_sdk::Vec<BatchRequest>,
) -> Result<(BatchSummary, soroban_sdk::Vec<BatchResult>), SLAError> {
    // Validate batch size and contents before processing
    validate_batch(env, &requests)?;

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
    let mut total_rewards: i128 = 0;
    let mut total_penalties: i128 = 0;

    for i in 0..requests.len() {
        let req = requests.get(i).unwrap();

        // Try to calculate
        match process_single(env, &configs, &req) {
            Ok(res) => {
                succeeded = succeeded.saturating_add(1);
                if res.status == symbol_short!("viol") {
                    total_penalties = total_penalties.saturating_add(res.amount);
                } else {
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
        let performance_ratio =
            (req.mttr_minutes as i128).saturating_mul(100).div_euclid(threshold as i128);
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

    // Check for duplicate outage IDs
    let mut seen = soroban_sdk::Map::new(env);
    for i in 0..requests.len() {
        let req = requests.get(i).unwrap();
        if seen.get(req.outage_id.clone()).unwrap_or(false) {
            return Err(SLAError::DuplicateOutageInput);
        }
        seen.set(req.outage_id, true);
    }

    Ok(requests.len())
}

use soroban_sdk::{contracttype, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SLAResult {
    pub item_id: u64,
    pub success: bool,
    pub recorded_at: u64, // Ledger timestamp stamp
}

pub struct BatchExecutionManager;

impl BatchExecutionManager {
    /// Executes batch items and stamps each result with the active ledger timestamp.
    pub fn process_batch(env: &Env, item_ids: Vec<u64>) -> Vec<SLAResult> {
        let current_timestamp = env.ledger().timestamp();
        let mut results = Vec::new(env);

        for item_id in item_ids.iter() {
            let result = SLAResult {
                item_id,
                success: true,
                recorded_at: current_timestamp,
            };
            results.push_back(result);
        }

        results
    }
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
