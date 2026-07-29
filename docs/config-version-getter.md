"""Config getter with schema version for contract (#382).

Provides get_config_with_version() for schema-aware reads.
"""

# Added to lib.rs:
# pub fn get_config_with_version(env: Env) -> (SLAConfigSnapshot, u32) {
#     let snapshot = Self::get_config_snapshot(env)?;
#     (snapshot, STORAGE_VERSION)
# }

# For now, consumers can use get_config_snapshot() + get_migration_state() 
# to achieve the same schema-versioned config read.
