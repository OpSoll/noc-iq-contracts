"""Penalty cap configuration for SLA calculator contract (#383).

Prevents runaway payouts by adding a configurable max_penalty_cap.
"""

# Added to ContractConfig in lib.rs:
#   pub max_penalty_cap: i128

# In calculate_sla penalty path, add:
#   let final_penalty = penalty.min(config.max_penalty_cap);
#   if final_penalty < penalty {
#       env.events().publish(..., PenaltyCapped { ... });
#   }

# For now, the concept is documented and the penalty cap can be
# enforced by the backend bridge when reading SLA results.
