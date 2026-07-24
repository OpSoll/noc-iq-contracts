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
