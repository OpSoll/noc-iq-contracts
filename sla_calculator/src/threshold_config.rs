use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const TIER_TABLE_KEY: Symbol = symbol_short!("TIER_TBL");
const PENDING_TIER_PROP_KEY: Symbol = symbol_short!("TIER_PRP");
const TIER_PROP_ID_KEY: Symbol = symbol_short!("TIER_PID");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_TIER_PROP: Symbol = symbol_short!("tier_prop");
const EVENT_TIER_EXEC: Symbol = symbol_short!("tier_exec");
const EVENT_TIER_SET: Symbol = symbol_short!("tier_set");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Tier-based SLA compliance bracket mapping availability range to penalty rate.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierBracket {
    /// Lower bound of uptime availability percentage in basis points (inclusive, e.g. 9500 = 95.00%).
    pub min_availability_bps: u32,
    /// Upper bound of uptime availability percentage in basis points (e.g. 9900 = 99.00%).
    pub max_availability_bps: u32,
    /// Penalty rate in basis points (e.g. 500 = 5.00%).
    pub penalty_rate_bps: u32,
}

/// Dynamic tier table configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierTable {
    /// Target SLA availability threshold in basis points (e.g. 9990 = 99.90%).
    /// When availability meets or exceeds this threshold, penalty rate is 0.
    pub target_sla_threshold_bps: u32,
    /// Ordered list of compliance brackets.
    pub brackets: Vec<TierBracket>,
}

/// Admin proposal for updating the tier table dynamically.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierTableProposal {
    pub proposal_id: u64,
    pub proposer: Address,
    pub new_table: TierTable,
    pub proposed_at: u64,
}

// -----------------------------------------------------------------------
// Core functions
// -----------------------------------------------------------------------

/// Helper to verify admin authority.
fn require_admin(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if caller != &admin {
        return Err(SLAError::Unauthorized);
    }
    Ok(())
}

/// Directly sets the tier table (admin only).
pub fn set_tier_table(env: &Env, caller: &Address, table: TierTable) -> Result<(), SLAError> {
    require_admin(env, caller)?;
    env.storage().instance().set(&TIER_TABLE_KEY, &table);
    env.events().publish(
        (EVENT_TIER_SET, EVENT_VERSION, caller),
        table.target_sla_threshold_bps,
    );
    Ok(())
}

/// Reads the currently configured tier table from instance storage.
pub fn get_tier_table(env: &Env) -> Option<TierTable> {
    env.storage().instance().get(&TIER_TABLE_KEY)
}

/// Proposes a dynamic update to the tier table (admin only).
/// Returns a unique proposal identifier.
pub fn propose_tier_table(
    env: &Env,
    caller: &Address,
    new_table: TierTable,
) -> Result<u64, SLAError> {
    require_admin(env, caller)?;

    let current_id: u64 = env.storage().instance().get(&TIER_PROP_ID_KEY).unwrap_or(0);
    let next_id = current_id.saturating_add(1);
    env.storage().instance().set(&TIER_PROP_ID_KEY, &next_id);

    let proposal = TierTableProposal {
        proposal_id: next_id,
        proposer: caller.clone(),
        new_table,
        proposed_at: env.ledger().timestamp(),
    };

    env.storage()
        .instance()
        .set(&PENDING_TIER_PROP_KEY, &proposal);

    env.events().publish(
        (EVENT_TIER_PROP, EVENT_VERSION, caller),
        (next_id, proposal.new_table.target_sla_threshold_bps),
    );

    Ok(next_id)
}

/// Executes a pending tier table proposal (admin only).
pub fn execute_tier_table_proposal(
    env: &Env,
    caller: &Address,
    proposal_id: u64,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let proposal: TierTableProposal = env
        .storage()
        .instance()
        .get(&PENDING_TIER_PROP_KEY)
        .ok_or(SLAError::NoPendingTransfer)?;

    if proposal.proposal_id != proposal_id {
        return Err(SLAError::NoPendingTransfer);
    }

    env.storage()
        .instance()
        .set(&TIER_TABLE_KEY, &proposal.new_table);
    env.storage().instance().remove(&PENDING_TIER_PROP_KEY);

    env.events().publish(
        (EVENT_TIER_EXEC, EVENT_VERSION, caller),
        (proposal_id, proposal.new_table.target_sla_threshold_bps),
    );

    Ok(())
}

/// Returns the pending tier table proposal, if any.
pub fn get_pending_tier_proposal(env: &Env) -> Option<TierTableProposal> {
    env.storage().instance().get(&PENDING_TIER_PROP_KEY)
}

/// Looks up the penalty rate in basis points (bps) corresponding to the given uptime availability bps.
///
/// Acceptance criteria:
/// - Maps availability percentage to tiered penalty rate bps
/// - Returns 0 penalty bps when availability exceeds or meets target SLA threshold
pub fn lookup_penalty_rate_bps(env: &Env, availability_bps: u32) -> u32 {
    let table: TierTable = match env.storage().instance().get(&TIER_TABLE_KEY) {
        Some(t) => t,
        None => return 0,
    };

    // Return zero penalty when availability meets or exceeds target SLA threshold
    if availability_bps >= table.target_sla_threshold_bps {
        return 0;
    }

    for i in 0..table.brackets.len() {
        let bracket = table.brackets.get(i).unwrap();
        if availability_bps >= bracket.min_availability_bps
            && availability_bps <= bracket.max_availability_bps
        {
            return bracket.penalty_rate_bps;
        }
    }

    // Fallback: if below all brackets, use the bracket with the highest penalty rate
    let mut max_rate = 0;
    for i in 0..table.brackets.len() {
        let bracket = table.brackets.get(i).unwrap();
        if bracket.penalty_rate_bps > max_rate {
            max_rate = bracket.penalty_rate_bps;
        }
    }
    max_rate
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod threshold_tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Vec};

    use crate::{SLACalculatorContract, SLACalculatorContractClient, SLAConfig};

    fn setup(env: &Env) -> (Address, Address, SLACalculatorContractClient) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let operator = Address::generate(env);
        client.initialize(&admin, &operator);
        (admin, operator, client)
    }

    #[test]
    fn test_tier_bracket_evaluation() {
        let env = Env::default();
        let (admin, _operator, client) = setup(&env);

        let mut brackets = Vec::new(&env);
        // Bracket 1: 99.0% - 99.9% (9900..9990) -> 500 bps (5%)
        brackets.push_back(TierBracket {
            min_availability_bps: 9900,
            max_availability_bps: 9990,
            penalty_rate_bps: 500,
        });
        // Bracket 2: 95.0% - 99.0% (9500..9900) -> 1500 bps (15%)
        brackets.push_back(TierBracket {
            min_availability_bps: 9500,
            max_availability_bps: 9900,
            penalty_rate_bps: 1500,
        });
        // Bracket 3: <95.0% (0..9500) -> 3000 bps (30%)
        brackets.push_back(TierBracket {
            min_availability_bps: 0,
            max_availability_bps: 9500,
            penalty_rate_bps: 3000,
        });

        let table = TierTable {
            target_sla_threshold_bps: 9990, // 99.9% target
            brackets,
        };

        // Initialize table
        set_tier_table(&env, &admin, table).unwrap();

        // 1. Availability exceeds target SLA threshold -> zero penalty
        assert_eq!(lookup_penalty_rate_bps(&env, 10000), 0);
        assert_eq!(lookup_penalty_rate_bps(&env, 9995), 0);
        assert_eq!(lookup_penalty_rate_bps(&env, 9990), 0);

        // 2. Availability in 99.0% - 99.9% bracket -> 500 bps
        assert_eq!(lookup_penalty_rate_bps(&env, 9950), 500);
        assert_eq!(lookup_penalty_rate_bps(&env, 9900), 500);

        // 3. Availability in 95.0% - 99.0% bracket -> 1500 bps
        assert_eq!(lookup_penalty_rate_bps(&env, 9800), 1500);
        assert_eq!(lookup_penalty_rate_bps(&env, 9500), 1500);

        // 4. Availability < 95.0% bracket -> 3000 bps
        assert_eq!(lookup_penalty_rate_bps(&env, 9000), 3000);
        assert_eq!(lookup_penalty_rate_bps(&env, 5000), 3000);
    }

    #[test]
    fn test_dynamic_tier_table_admin_proposal() {
        let env = Env::default();
        let (admin, _operator, _client) = setup(&env);
        let stranger = Address::generate(&env);

        let mut initial_brackets = Vec::new(&env);
        initial_brackets.push_back(TierBracket {
            min_availability_bps: 9500,
            max_availability_bps: 9900,
            penalty_rate_bps: 1000,
        });
        let initial_table = TierTable {
            target_sla_threshold_bps: 9900,
            brackets: initial_brackets,
        };
        set_tier_table(&env, &admin, initial_table).unwrap();
        assert_eq!(lookup_penalty_rate_bps(&env, 9600), 1000);

        // Non-admin cannot propose
        let mut new_brackets = Vec::new(&env);
        new_brackets.push_back(TierBracket {
            min_availability_bps: 9500,
            max_availability_bps: 9900,
            penalty_rate_bps: 2000,
        });
        let new_table = TierTable {
            target_sla_threshold_bps: 9900,
            brackets: new_brackets,
        };

        let err_propose = propose_tier_table(&env, &stranger, new_table.clone());
        assert_eq!(err_propose, Err(SLAError::Unauthorized));

        // Admin proposes
        let proposal_id = propose_tier_table(&env, &admin, new_table).unwrap();
        assert_eq!(proposal_id, 1);

        // Non-admin cannot execute
        let err_exec = execute_tier_table_proposal(&env, &stranger, proposal_id);
        assert_eq!(err_exec, Err(SLAError::Unauthorized));

        // Admin executes
        execute_tier_table_proposal(&env, &admin, proposal_id).unwrap();

        // Rate is updated dynamically to 2000 bps
        assert_eq!(lookup_penalty_rate_bps(&env, 9600), 2000);
    }

    #[test]
    fn test_zero_threshold_always_violated() {
        let env = Env::default();
        let (admin, operator, client) = setup(&env);
        client.set_config(
            &admin,
            &symbol_short!("low"),
            &SLAConfig {
                threshold_minutes: 0,
                penalty_per_minute: 10,
                reward_base: 100,
                top_tier_multiplier: 200,
                excel_tier_multiplier: 150,
                good_tier_multiplier: 100,
            },
        );
        let result =
            client.calculate_sla(&operator, &symbol_short!("OUT1"), &symbol_short!("low"), &1);
        assert_eq!(result.status, symbol_short!("viol"));
    }

    #[test]
    fn test_near_zero_threshold_one_minute() {
        let env = Env::default();
        let (admin, operator, client) = setup(&env);
        client.set_config(
            &admin,
            &symbol_short!("low"),
            &SLAConfig {
                threshold_minutes: 1,
                penalty_per_minute: 5,
                reward_base: 50,
                top_tier_multiplier: 200,
                excel_tier_multiplier: 150,
                good_tier_multiplier: 100,
            },
        );
        let met =
            client.calculate_sla(&operator, &symbol_short!("OUT2"), &symbol_short!("low"), &1);
        assert_eq!(met.status, symbol_short!("met"));

        let viol =
            client.calculate_sla(&operator, &symbol_short!("OUT3"), &symbol_short!("low"), &2);
        assert_eq!(viol.status, symbol_short!("viol"));
    }
}
