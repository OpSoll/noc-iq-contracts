# NOC IQ SLA Calculator SDK

Typed TypeScript client for interacting with the SLA Calculator Soroban contract.

## Usage

```ts
import { SLACalculatorClient } from "@noc-iq/sla-calculator-sdk";

const client = new SLACalculatorClient({
  contractId: "CABC...",
  networkPassphrase: "Testnet ; SDF Network ; September 2015",
  rpcUrl: "https://soroban-testnet.stellar.org",
});

// Read configuration
const config = await client.getConfig("critical");
console.log(config.threshold_minutes); // 15

// Calculate SLA (view-only, no persistence)
const result = await client.calculateSlaView("outage-123", "high", 90);
console.log(result.status); // "met"

// Get stats
const stats = await client.getStats();
```

## Contract Methods

All public contract methods are wrapped with typed signatures:

| Method | Description |
|---|---|
| `initialize(admin, operator)` | Initialize the contract |
| `setConfig(severity, ...)` | Configure an SLA tier (admin) |
| `getConfig(severity)` | Read config for a tier |
| `listConfigs()` | List all tiers |
| `getConfigSnapshot()` | Backend-friendly config snapshot |
| `getConfigVersionHash()` | Drift detection hash |
| `calculateSla(caller, outageId, severity, mttr)` | Persist SLA result (operator) |
| `calculateSlaView(outageId, severity, mttr)` | View-only calculation |
| `getHistory()` | Full calculation history |
| `getHistoryPage(offset, limit)` | Paginated history |
| `getHistoryByOutage(outageId)` | History by outage |
| `getLatestByOutage(outageId)` | Latest result for outage |
| `pruneHistory(caller, keepLatest)` | Prune old entries (admin) |
| `pruneHistoryByAge(caller, minAgeSeconds)` | Age-based pruning (admin) |
| `setRetentionLimit(caller, limit)` | Set max history size (admin) |
| `getRetentionLimit()` | Current retention limit |
| `getStats()` | Cumulative statistics |
| `getResultSchema()` | Result field semantics |
| `getContractMetadata()` | Contract capabilities |
| `getFailureSchema()` | Failure code catalogue |
| `getStorageVersion()` | Storage schema version |
| `pause(caller, reason)` / `unpause(caller)` | Pause controls (admin) |
| `isPaused()` / `getPauseInfo()` | Pause state |
| `proposeAdmin(newAdmin)` / `acceptAdmin()` | Admin transfer |
| `proposeOperator(newOperator)` / `acceptOperator()` | Operator transfer |
| `getAdmin()` / `getOperator()` | Role queries |
| `getMigrationState()` | Storage version info |
| `getVersionInfo()` | Version negotiation snapshot |
| `migrate(caller)` | Run migration (admin) |

## Types

All Soroban contract types are exported as TypeScript interfaces:

`SLAConfig`, `SLAResult`, `SLAStats`, `SLAResultSchema`, `ContractMetadata`,
`FailureSchema`, `PauseInfo`, `StorageVersionInfo`, `VersionInfo`, `Severity`
