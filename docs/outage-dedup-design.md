"""Outage ID collision detection and deduplication guard (#388).

Prevents silent overwrite of existing outage records by checking for
ID existence before writing.
"""

# New error variant (add to SLAError enum):
#   DuplicateOutageId = 18

# In outage recording path, before writing:
#   if env.storage().persistent().has(&DataKey::Outage(outage_id)) {
#       return Err(SLAError::DuplicateOutageId);
#   }

# This guard is executed atomically with the write since both happen
# in the same contract invocation (Soroban guarantees atomicity).
