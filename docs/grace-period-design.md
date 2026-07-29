"""Grace period before SLA breach declaration (#389).

Adds a configurable grace_period_seconds to avoid false breach
declarations from transient blips.
"""

# Added to SLAConfig struct:
#   pub grace_period_seconds: u64  (default 0 for backward compat)

# In calculate_sla, before breach assessment:
#   if mttr_minutes * 60 <= config.grace_period_seconds as u32 {
#       // Outage within grace period - record but don't count as breach
#       env.events().publish(..., GracePeriodApplied { ... });
#       return no_breach_result;
#   }

# Admin can update via set_config with validation: 0 <= grace <= 3600
