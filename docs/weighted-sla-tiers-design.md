"""Weighted SLA tier support (#384).

Different thresholds for different service classes (gold/silver/bronze).
"""

# New struct:
#   #[contracttype]
#   pub struct ServiceTier {
#       pub tier_id: Symbol,
#       pub uptime_threshold_bps: u32,
#       pub penalty_rate_bps: u32,
#       pub weight: u32,
#   }

# Added to ContractConfig:
#   pub tiers: Vec<ServiceTier>  (max 10 tiers)

# In calculate_sla, resolve tier by tier_id parameter;
# fall back to first tier if not specified.
# Unknown tier returns ContractError::UnknownTier (new variant).
