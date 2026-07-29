#![no_std]

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::{SLAResult, SLAError, SLAConfig, ADMIN_KEY, OPERATOR_KEY, CONFIG_KEY, HISTORY_KEY};

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Batch calculation request item.
#[soroban_sdk::contracttype]
pub struct BatchRequest {
    /// Unique identifier for this outage.
    pub outage_id: Symbol,
    /// Severity tier to evaluate against.
    pub severity: Symbol,
    /// Mean time to resolution in minutes.
    pub mttr_minutes: u32,
}

/// Batch calculation result item.
#[soroban_sdk::contracttype]
pub struct BatchResult {
    /// The outage ID.
    pub outage_id: Symbol,
    /// Whether calculation succeeded.
    pub success: bool,
    /// The SLA result (if successful).
    pub result: Option<SLAResult>,
    /// Error message (if failed).
    pub error: Option<Symbol>,
}

/// Batch calculation summary.
#[soroban_sdk::contracttype]
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
    let paused: bool = env.storage().instance().get(&symbol_short!("PAUSED")).unwrap_or(false);
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
        match process_single(&env, &configs, &req) {
            Ok(result) => {
                succeeded = succeeded.saturating_add(1);
                if result.status == symbol_short!("viol") {
                    total_penalties = total_penalties.saturating_add(result.amount);
                } else {
                    total_rewards = total_rewards.saturating_add(result.amount);
                }
                results.push_back(BatchResult {
                    outage_id: req.outage_id,
                    success: true,
                    result: Some(result),
                    error: None,
                });
            }
            Err(e) => {
                failed = failed.saturating_add(1);
                let error_msg = match e {
                    SLAError::ConfigNotFound => symbol_short!("no_config"),
                    SLAError::InvalidSeverity => symbol_short!("bad_sev"),
                    SLAError::InvalidThreshold => symbol_short!("bad_thresh"),
                    _ => symbol_short!("unknown"),
                };
                results.push_back(BatchResult {
                    outage_id: req.outage_id,
                    success: false,
                    result: None,
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
fn process_single(
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
        return Err(SLAError::InvalidThreshold);
    }

    // Calculate result
    let threshold = cfg.threshold_minutes;
    if req.mttr_minutes > threshold {
        // Violation
        let overtime = (req.mttr_minutes - threshold) as i128;
        let penalty = overtime.saturating_mul(cfg.penalty_per_minute);
        Ok(SLAResult {
            outage_id: req.outage_id,
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
        let performance_ratio = (req.mttr_minutes * 100) / threshold;
        let (multiplier, rating) = if performance_ratio < 50 {
            (200u32, symbol_short!("top"))
        } else if performance_ratio < 75 {
            (150u32, symbol_short!("excel"))
        } else {
            (100u32, symbol_short!("good"))
        };

        let reward = cfg
            .reward_base
            .saturating_mul(multiplier as i128)
            .div_euclid(100);

        Ok(SLAResult {
            outage_id: req.outage_id,
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
    if requests.len() == 0 {
        return Err(SLAError::InvalidThreshold);
    }

    if requests.len() > get_batch_limit() {
        return Err(SLAError::InvalidThreshold);
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