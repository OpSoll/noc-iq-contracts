"""Emergency reset for outage records (#385).

Admin-only function to nullify a specific outage without wiping history.
"""

# New error variant:
#   OutageNotFound = 19

# New function:
#   pub fn emergency_reset_outage(env: Env, admin: Address, outage_id: Symbol) {
#       admin.require_auth();
#       if !env.storage().persistent().has(&DataKey::Outage(outage_id)) {
#           return Err(SLAError::OutageNotFound);
#       }
#       env.storage().persistent().remove(&DataKey::Outage(outage_id));
#       env.events().publish(..., OutageEmergencyReset { outage_id, reset_by: admin, ... });
#       Ok(())
#   }
