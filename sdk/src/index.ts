/**
 * NOC IQ SLA Calculator — Offchain TypeScript SDK
 *
 * Typed async client for interacting with the SLA Calculator Soroban contract.
 * Provides ergonomic wrappers for all public contract methods.
 */

export { SLACalculatorClient, TimeoutError, decodeContractEvent } from "./client";
export type {
  ClientConfig,
  ContractResult,
  PreflightResult,
  ContractEvent,
  ContractEventCallback,
  TransactionStatus,
  TransactionStatusResult,
} from "./client";
export { toScVal, fromScVal } from "./scval";
export type { ScVal, ScValType } from "./scval";
export {
  CANONICAL_SEVERITIES,
  MAX_HISTORY_SIZE,
  decodeContractError,
} from "./types";
export type {
  SLAConfig,
  SLAConfigEntry,
  SLAConfigSnapshot,
  SLAResult,
  SLAResultSchema,
  SLAStats,
  ContractMetadata,
  PauseInfo,
  StorageVersionInfo,
  FailureCode,
  FailureSchema,
  VersionInfo,
  Severity,
  DecodedContractError,
} from "./types";
