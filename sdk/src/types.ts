/**
 * SLA Calculator Contract Types
 *
 * These types mirror the on-chain Soroban contract structs.
 * Keep in sync with sla_calculator/src/lib.rs contracttype definitions.
 */

export interface SLAConfig {
  threshold_minutes: number;
  penalty_per_minute: bigint;
  reward_base: bigint;
}

export interface SLAConfigEntry {
  severity: string;
  config: SLAConfig;
}

export interface SLAConfigSnapshot {
  version: string;
  entries: SLAConfigEntry[];
}

export interface SLAResult {
  outage_id: string;
  status: "met" | "viol";
  mttr_minutes: number;
  threshold_minutes: number;
  amount: bigint;
  payment_type: "rew" | "pen";
  rating: "top" | "excel" | "good" | "poor";
  config_version_hash: bigint;
  recorded_at: number;
}

export interface SLAResultSchema {
  version: string;
  schema_version: number;
  status_met: string;
  status_violated: string;
  payment_reward: string;
  payment_penalty: string;
  rating_exceptional: string;
  rating_excellent: string;
  rating_good: string;
  rating_poor: string;
  includes_config_version_hash: boolean;
}

export interface ContractMetadata {
  contract_name: string;
  storage_version: number;
  result_schema_version: number;
  supported_severities: string[];
  features: string[];
}

export interface SLAStats {
  total_calculations: bigint;
  total_violations: bigint;
  total_rewards: bigint;
  total_penalties: bigint;
}

export interface PauseInfo {
  reason: string;
  paused_at: number;
  paused_by: string;
}

export interface StorageVersionInfo {
  stored_version: number;
  expected_version: number;
  needs_migration: boolean;
}

export interface FailureCode {
  code: number;
  label: string;
  description: string;
}

export interface FailureSchema {
  version: string;
  codes: FailureCode[];
}

export interface VersionInfo {
  storage_version: number;
  result_schema_version: number;
  needs_migration: boolean;
  is_paused: boolean;
  contract_name: string;
}

export type Severity = "critical" | "high" | "medium" | "low";

export const CANONICAL_SEVERITIES: Severity[] = [
  "critical",
  "high",
  "medium",
  "low",
];

export const MAX_HISTORY_SIZE = 1000;

// -----------------------------------------------------------------------
// Contract error decoding
// -----------------------------------------------------------------------

export interface DecodedContractError {
  code: number;
  name: string;
  message: string;
  recommendedAction: string;
}

/**
 * Mirrors the `SLAError` enum in `sla_calculator/src/lib.rs`.
 * Keep in sync with the contract's `#[contracterror]` definition.
 */
const CONTRACT_ERROR_MESSAGES: Record<
  number,
  { name: string; message: string; action: string }
> = {
  1: {
    name: "AlreadyInitialized",
    message: "The contract has already been initialized.",
    action:
      "Do not call initialize() again — use the existing admin/operator addresses.",
  },
  2: {
    name: "NotInitialized",
    message: "The contract has not been initialized yet.",
    action: "Call initialize(admin, operator) before invoking other methods.",
  },
  3: {
    name: "Unauthorized",
    message: "The caller is not authorized to perform this action.",
    action:
      "Verify the caller is the current admin or operator for this method.",
  },
  4: {
    name: "ConfigNotFound",
    message: "No SLA configuration exists for the requested severity tier.",
    action:
      "Call setConfig() for this severity before querying or calculating against it.",
  },
  5: {
    name: "VersionMismatch",
    message:
      "The stored data version does not match what this contract build expects.",
    action: "Run the contract migration before retrying.",
  },
  6: {
    name: "ContractPaused",
    message: "The contract is currently paused.",
    action:
      "Wait for an admin to unpause the contract, or check getPauseInfo() for the reason.",
  },
  7: {
    name: "NoPendingTransfer",
    message: "There is no pending admin/operator transfer to accept or cancel.",
    action: "Call proposeAdmin()/proposeOperator() first.",
  },
  8: {
    name: "InvalidThreshold",
    message: "The provided threshold value is invalid.",
    action: "Use a positive threshold in minutes.",
  },
  9: {
    name: "InvalidPenalty",
    message: "The provided penalty value is invalid.",
    action: "Use a non-negative penalty amount.",
  },
  10: {
    name: "InvalidReward",
    message: "The provided reward value is invalid.",
    action: "Use a non-negative reward amount.",
  },
  11: {
    name: "InvalidSeverity",
    message: "The provided severity tier is not recognised.",
    action: "Use one of: critical, high, medium, low.",
  },
  12: {
    name: "RetentionLimitOutOfRange",
    message: "The requested history retention limit is out of the allowed range.",
    action: "Choose a retention limit within the contract's configured bounds.",
  },
  13: {
    name: "DuplicateOutageInput",
    message: "An outage with this ID has already been recorded.",
    action: "Use a unique outage ID, or fetch the existing record instead.",
  },
  14: {
    name: "InvalidPenaltyAmount",
    message: "The computed penalty amount is invalid.",
    action: "Check the configured penalty-per-minute and MTTR inputs.",
  },
  15: {
    name: "InvalidRewardAmount",
    message: "The computed reward amount is invalid.",
    action: "Check the configured reward base for this severity tier.",
  },
  16: {
    name: "InvalidOutageId",
    message: "The outage ID is malformed.",
    action: "Use a non-empty, valid identifier string.",
  },
  17: {
    name: "MalformedSymbolInput",
    message: "One of the symbol inputs is malformed.",
    action:
      "Check that severity/status symbols match the expected short-symbol format.",
  },
  18: {
    name: "InvalidMTTR",
    message: "The mean-time-to-resolution value is invalid.",
    action: "Use a non-negative MTTR in minutes.",
  },
  19: {
    name: "ThresholdOutOfBounds",
    message: "The threshold value is outside the allowed bounds.",
    action: "Use a threshold within the contract's configured min/max range.",
  },
  20: {
    name: "PenaltyOutOfBounds",
    message: "The penalty value is outside the allowed bounds.",
    action: "Use a penalty within the contract's configured min/max range.",
  },
  21: {
    name: "RewardOutOfBounds",
    message: "The reward value is outside the allowed bounds.",
    action: "Use a reward within the contract's configured min/max range.",
  },
  22: {
    name: "InvalidMonth",
    message: "The provided month value is invalid.",
    action: "Use a month value between 1 and 12.",
  },
};

/**
 * Decodes a raw contract error code (as returned by a failed Soroban
 * invocation, e.g. `Error(Contract, #4)` → `4`) into a human-readable
 * message with a recommended troubleshooting action.
 *
 * Also recognises standard Soroban host-level error codes (negative
 * values, as seen in JSON-RPC error responses such as -32603), which
 * indicate the transaction failed before/outside normal contract Err(...)
 * handling.
 */
export function decodeContractError(errorCode: number): DecodedContractError {
  if (errorCode < 0) {
    return {
      code: errorCode,
      name: "HostError",
      message: `Soroban host-level error (code ${errorCode}) — the transaction failed before or during execution, not from a contract-level Err(...) return.`,
      recommendedAction:
        "Check the transaction result's diagnostic events for the underlying panic/trap reason.",
    };
  }

  const known = CONTRACT_ERROR_MESSAGES[errorCode];
  if (known) {
    return {
      code: errorCode,
      name: known.name,
      message: known.message,
      recommendedAction: known.action,
    };
  }

  return {
    code: errorCode,
    name: "UnknownError",
    message: `Unrecognised contract error code ${errorCode}.`,
    recommendedAction:
      "Check the deployed contract version's error enum — this SDK may be out of date.",
  };
}
