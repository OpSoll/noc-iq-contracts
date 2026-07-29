# Storage Layout and TTL Management
This document details the SLA Calculator contract's storage layout, key management, storage types, and TTL (time-to-live) policies for operators and backend developers.

## Storage Overview
The SLA Calculator contract uses Soroban's **instance storage** exclusively for all persistent state. Instance storage is appropriate for this contract as it maintains long-lived protocol state that needs to persist across ledger upgrades.

| Storage Category | Storage Type | Usage |
|-----------------|--------------|-------|
| Core State      | Instance     | Roles, configuration, and operational state |
| History         | Instance     | SLA calculation results and audit trail |
| Versioning      | Instance     | Storage schema version tracking for migrations |

---

## Storage Keys Reference
All storage keys are defined as Soroban `Symbol` constants to minimize gas costs and ensure type safety. Below is the complete inventory of all storage keys used by the contract:

| Constant Symbol | Key Name | Data Type | Access Control | Description |
|-----------------|----------|-----------|----------------|-------------|
| `ADMIN`         | Admin address | `Address` | Admin only (write) | The contract's administrator address with full privileges. Can update config, pause the contract, and manage roles. |
| `OPERATOR`      | Operator address | `Address` | Admin only (write) | The operator address that may call `calculate_sla` to process real outages. |
| `PADMIN`        | Pending admin | `Address` | Admin only | Holds the address of a proposed admin during the two-step transfer process. |
| `POP`           | Pending operator | `Address` | Admin only | Holds the address of a proposed operator during the two-step handoff process. |
| `CONFIG`        | SLA configurations | `Map<Symbol, SLAConfig>` | Admin only (write) | Maps severity symbols to their respective SLA configurations (thresholds, penalties, rewards). |
| `PAUSED`        | Contract pause flag | `bool` | Admin only | Flag indicating if the contract is currently paused (blocks all mutating operations). |
| `PAUSEINF`      | Pause metadata | `PauseInfo` | Admin only | Stores detailed information when the contract is paused: reason, timestamp, and pauser address. |
| `STATS`         | Cumulative statistics | `SLAStats` | Internal only | Running totals of calculations, violations, rewards, and penalties. Updated automatically by `calculate_sla`. |
| `HIST`          | Calculation history | `Vec<SLAResult>` | Internal only | Persistent log of all SLA calculations. Appended to on every successful `calculate_sla` call. |
| `VER`           | Storage version | `u32` | Admin only (via migrate) | Tracks the storage schema version to enable seamless upgrades and migrations. |
| `RETLIM`        | Retention limit | `u32` | Admin only | Configurable maximum number of history entries to retain. Defaults to `MAX_HISTORY_SIZE = 1000`. |

### Key Symbols Definition
All key symbols are defined at the contract level to ensure consistency:
```rust
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const OPERATOR_KEY: Symbol = symbol_short!("OPERATOR");
const PENDING_ADMIN_KEY: Symbol = symbol_short!("PADMIN");
const PENDING_OP_KEY: Symbol = symbol_short!("POP");
const CONFIG_KEY: Symbol = symbol_short!("CONFIG");
const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const PAUSE_INFO_KEY: Symbol = symbol_short!("PAUSEINF");
const STATS_KEY: Symbol = symbol_short!("STATS");
const HISTORY_KEY: Symbol = symbol_short!("HIST");
const STORAGE_VERSION_KEY: Symbol = symbol_short!("VER");
const RETENTION_LIMIT_KEY: Symbol = symbol_short!("RETLIM");
```

---

## Storage Entry Lifecycle
### Instance Storage Properties
The contract exclusively uses instance storage (`env.storage().instance()`) which provides:
- Persistence across ledger upgrades
- Automatic TTL extension when accessed
- Lower gas costs compared to persistent storage for frequently accessed data

### Instance Storage TTL Management
Soroban automatically extends the TTL of instance storage entries when they are accessed. The contract's usage pattern ensures that:
1. **Read operations** (all view functions: `get_history`, `simulate_sla`, `calculate_sla_view`, etc.) extend TTL
2. **Write operations** (all mutating functions: `calculate_sla`, `set_retention_limit`, etc.) extend TTL
3. Regular activity maintains the instance storage indefinitely

### History Pruning and Retention
To prevent unbounded storage growth:
1. **Configurable Retention**: The `RETLIM` key controls the maximum number of history entries (default: 1000)
2. **Automatic Pruning**: When `calculate_sla` is called and history exceeds the retention limit, the oldest entry is dropped
3. **Admin-Controlled Pruning**: Admins can manually prune history using:
   - `prune_history(keep_latest: u32)` - Prune to keep only the N most recent entries
   - `prune_history_by_age(min_age_seconds: u64)` - Remove entries older than the specified age

---

## Storage Migration Guidelines
The contract implements a structured migration framework to handle storage schema changes during contract upgrades:

### Migration Process
1. **Version Tracking**: The `VER` key stores the current storage schema version
2. **Version Check**: All contract functions (except migration helpers) call `check_version()` to ensure compatibility
3. **Migration Execution**: Admins must call `migrate()` after upgrading the contract to apply any necessary storage transformations
4. **Idempotent Operations**: The `migrate()` function is safe to call multiple times - it only applies necessary changes

### Supported Migrations
Current storage version: `STORAGE_VERSION = 1`

| From Version | To Version | Changes |
|--------------|------------|---------|
| 0            | 1          | Initial storage schema with all core keys |

### Pre-Migration Checks
Before executing `migrate()`, backend systems should:
1. Call `get_migration_state()` to check if migration is needed
2. Verify `stored_version` matches the binary's `expected_version` after migration
3. Confirm no errors are returned before resuming normal operations

---

## Read-Only Simulation Functions
The contract provides read-only simulation functions that **never modify storage**:
- `simulate_sla(outage: OutageInput)` - Pure calculation that returns SLA results without persistence
- `calculate_sla_view(...)` - Returns the full SLAResult structure without modifying state
- All getter functions (`get_history`, `get_admin`, etc.) - Only read from storage

These functions are safe to call repeatedly and do not affect contract state or TTL extension beyond normal read operation behavior.