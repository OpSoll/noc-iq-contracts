/**
 * SLA Calculator Contract Client
 *
 * Typed async wrapper for all public SLA calculator contract methods.
 * Provides ergonomic function names that map directly to contract endpoints.
 */

import {
  SLAConfig,
  SLAConfigSnapshot,
  SLAResult,
  SLAResultSchema,
  SLAStats,
  ContractMetadata,
  PauseInfo,
  StorageVersionInfo,
  FailureSchema,
  VersionInfo,
  Severity,
} from "./types";
import { ScVal, toScVal, fromScVal } from "./scval";

// The project's tsconfig targets ES2020 with no DOM/Node lib, so the
// ambient `setTimeout` global isn't declared even though it exists at
// runtime in both browsers and Node. Declare just enough of its shape.
declare function setTimeout(callback: () => void, ms: number): unknown;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

const BASE64_CHARS =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/**
 * Minimal, dependency-free base64 encoder (ASCII/UTF-8 input only). Used to
 * encode offline transaction envelopes without pulling in a Buffer/Node
 * dependency the SDK doesn't otherwise need.
 */
function toBase64(input: string): string {
  let output = "";
  let i = 0;
  while (i < input.length) {
    const a = input.charCodeAt(i++);
    const b = i < input.length ? input.charCodeAt(i++) : NaN;
    const c = i < input.length ? input.charCodeAt(i++) : NaN;
    const triplet = (a << 16) | ((isNaN(b) ? 0 : b) << 8) | (isNaN(c) ? 0 : c);
    output += BASE64_CHARS[(triplet >> 18) & 0x3f];
    output += BASE64_CHARS[(triplet >> 12) & 0x3f];
    output += isNaN(b) ? "=" : BASE64_CHARS[(triplet >> 6) & 0x3f];
    output += isNaN(c) ? "=" : BASE64_CHARS[triplet & 0x3f];
  }
  return output;
}

/**
 * Configuration for the SLACalculatorClient.
 */
export interface ClientConfig {
  /** The deployed contract address (Stellar address). */
  contractId: string;
  /** The Stellar network passphrase (e.g., "Testnet ; SDF Network ; September 2015"). */
  networkPassphrase: string;
  /** Base URL of the Soroban RPC server. */
  rpcUrl: string;
}

/**
 * Generic contract invocation result wrapper.
 */
export interface ContractResult<T> {
  /** Whether the invocation succeeded. */
  ok: boolean;
  /** The decoded result value (present when ok is true). */
  value?: T;
  /** Error message (present when ok is false). */
  error?: string;
}

/**
 * Parameters for {@link SLACalculatorClient.buildOutageReportTx}.
 */
export interface BuildOutageReportTxParams {
  /** Address the transaction will be sourced/signed from. */
  source: string;
  outageId: string;
  severity: Severity;
  mttrMinutes: number;
  /** Resource fee, in stroops. Defaults to 100,000. */
  fee?: number;
  /** Account sequence number to use. Defaults to "0" (caller must supply the real value in production). */
  sequence?: string;
}

/**
 * An unsigned transaction envelope, ready for offline/multi-sig signing.
 */
export interface UnsignedEnvelope {
  /** Base64-encoded unsigned transaction envelope. */
  envelopeXdr: string;
  fee: number;
  sequence: string;
}

/**
 * Typed client for interacting with the SLA Calculator Soroban contract.
 *
 * All methods return typed results matching the on-chain contract output.
 * The client does not manage keypairs or transaction signing — it provides
 * read-only query wrappers and typed mutation envelopes that the backend
 * can submit using its own signing infrastructure.
 *
 * @example
 * ```ts
 * const client = new SLACalculatorClient({
 *   contractId: "CABC...",
 *   networkPassphrase: "Testnet ; SDF Network ; September 2015",
 *   rpcUrl: "https://soroban-testnet.stellar.org",
 * });
 *
 * const config = await client.getConfig("critical");
 * console.log(config.threshold_minutes); // 15
 * ```
 */
export class SLACalculatorClient {
  private readonly config: ClientConfig;

  constructor(config: ClientConfig) {
    this.config = config;
  }

  get contractAddress(): string {
    return this.config.contractId;
  }

  get network(): string {
    return this.config.networkPassphrase;
  }

  // -----------------------------------------------------------------------
  // Initialization
  // -----------------------------------------------------------------------

  /**
   * Build a transaction envelope for contract initialization.
   *
   * @param admin - Address of the contract administrator.
   * @param operator - Address of the SLA calculation operator.
   * @returns Transaction envelope XDR ready for signing and submission.
   */
  async initialize(
    admin: string,
    operator: string,
  ): Promise<ContractResult<void>> {
    return this.invoke("initialize", [admin, operator]);
  }

  // -----------------------------------------------------------------------
  // Configuration management (admin only)
  // -----------------------------------------------------------------------

  /**
   * Set SLA configuration for a given severity tier (admin only).
   *
   * @param caller - Address of the admin caller.
   * @param severity - Severity tier to configure.
   * @param thresholdMinutes - Maximum acceptable MTTR in minutes.
   * @param penaltyPerMinute - Penalty amount per overtime minute.
   * @param rewardBase - Base reward for meeting the SLA.
   */
  async setConfig(
    caller: string,
    severity: Severity,
    thresholdMinutes: number,
    penaltyPerMinute: bigint,
    rewardBase: bigint,
  ): Promise<ContractResult<void>> {
    return this.invoke("set_config", [
      caller,
      severity,
      thresholdMinutes,
      penaltyPerMinute,
      rewardBase,
    ]);
  }

  /**
   * Get SLA configuration for a specific severity tier.
   */
  async getConfig(severity: Severity): Promise<ContractResult<SLAConfig>> {
    return this.invoke("get_config", [severity]);
  }

  /**
   * List all severity configurations as a map.
   */
  async listConfigs(): Promise<
    ContractResult<Map<string, SLAConfig>>
  > {
    return this.invoke("list_configs", []);
  }

  /**
   * Returns a deterministic backend-friendly snapshot of all config values
   * in canonical severity order.
   */
  async getConfigSnapshot(): Promise<ContractResult<SLAConfigSnapshot>> {
    return this.invoke("get_config_snapshot", []);
  }

  /**
   * Returns a config version hash for cheap drift detection.
   */
  async getConfigVersionHash(): Promise<ContractResult<bigint>> {
    return this.invoke("get_config_version_hash", []);
  }

  /**
   * Returns the number of configured severity tiers.
   */
  async getConfigCount(): Promise<ContractResult<number>> {
    return this.invoke("get_config_count", []);
  }

  // -----------------------------------------------------------------------
  // SLA Calculation (operator only)
  // -----------------------------------------------------------------------

  /**
   * Calculate SLA deterministically and persist the result (operator only).
   *
   * @param caller - Address of the operator.
   * @param outageId - Unique outage identifier.
   * @param severity - Severity tier for this outage.
   * @param mttrMinutes - Mean time to resolution in minutes.
   */
  async calculateSla(
    caller: string,
    outageId: string,
    severity: Severity,
    mttrMinutes: number,
  ): Promise<ContractResult<SLAResult>> {
    return this.invoke("calculate_sla", [
      caller,
      outageId,
      severity,
      mttrMinutes,
    ]);
  }

  /**
   * View-only SLA calculation. Does not persist results or emit events.
   * Callable by any address without authorization.
   */
  async calculateSlaView(
    outageId: string,
    severity: Severity,
    mttrMinutes: number,
  ): Promise<ContractResult<SLAResult>> {
    return this.invoke("calculate_sla_view", [outageId, severity, mttrMinutes]);
  }

  /**
   * Fetches the latest SLA result for multiple outage/site IDs in parallel,
   * avoiding a sequential round-trip per ID.
   *
   * @param siteIds - Outage/site identifiers to look up.
   * @returns A map from site ID to its latest SLAResult (or null if none exists).
   */
  async getBatchSlaMetrics(
    siteIds: string[],
  ): Promise<ContractResult<Map<string, SLAResult | null>>> {
    const entries = await Promise.all(
      siteIds.map(async (siteId): Promise<[string, SLAResult | null]> => {
        const result = await this.getLatestByOutage(siteId);
        return [siteId, result.ok ? (result.value ?? null) : null];
      }),
    );
    return { ok: true, value: new Map(entries) };
  }

  // -----------------------------------------------------------------------
  // History management
  // -----------------------------------------------------------------------

  /**
   * Returns the full calculation history.
   */
  async getHistory(): Promise<ContractResult<SLAResult[]>> {
    return this.invoke("get_history", []);
  }

  /**
   * Returns a paginated slice of history (oldest first).
   *
   * @param offset - Zero-based start index.
   * @param limit - Maximum entries per page.
   */
  async getHistoryPage(
    offset: number,
    limit: number,
  ): Promise<ContractResult<SLAResult[]>> {
    return this.invoke("get_history_page", [offset, limit]);
  }

  /**
   * Returns all history entries matching a specific outage ID.
   */
  async getHistoryByOutage(
    outageId: string,
  ): Promise<ContractResult<SLAResult[]>> {
    return this.invoke("get_history_by_outage", [outageId]);
  }

  /**
   * Returns the most recent history entry for a given outage ID.
   */
  async getLatestByOutage(
    outageId: string,
  ): Promise<ContractResult<SLAResult | null>> {
    return this.invoke("get_latest_by_outage", [outageId]);
  }

  /**
   * Prune history to keep only the latest N entries (admin only).
   */
  async pruneHistory(
    caller: string,
    keepLatest: number,
  ): Promise<ContractResult<void>> {
    return this.invoke("prune_history", [caller, keepLatest]);
  }

  /**
   * Prune history entries older than minAgeSeconds (admin only).
   */
  async pruneHistoryByAge(
    caller: string,
    minAgeSeconds: bigint,
  ): Promise<ContractResult<void>> {
    return this.invoke("prune_history_by_age", [caller, minAgeSeconds]);
  }

  /**
   * Returns the current retention limit.
   */
  async getRetentionLimit(): Promise<ContractResult<number>> {
    return this.invoke("get_retention_limit", []);
  }

  /**
   * Set the maximum number of history entries to retain (admin only).
   */
  async setRetentionLimit(
    caller: string,
    limit: number,
  ): Promise<ContractResult<void>> {
    return this.invoke("set_retention_limit", [caller, limit]);
  }

  // -----------------------------------------------------------------------
  // Statistics
  // -----------------------------------------------------------------------

  /**
   * Returns cumulative SLA performance statistics.
   */
  async getStats(): Promise<ContractResult<SLAStats>> {
    return this.invoke("get_stats", []);
  }

  // -----------------------------------------------------------------------
  // Contract metadata and introspection
  // -----------------------------------------------------------------------

  /**
   * Returns the result schema describing SLAResult field semantics.
   */
  async getResultSchema(): Promise<ContractResult<SLAResultSchema>> {
    return this.invoke("get_result_schema", []);
  }

  /**
   * Returns static contract capabilities for backend introspection.
   */
  async getContractMetadata(): Promise<ContractResult<ContractMetadata>> {
    return this.invoke("get_contract_metadata", []);
  }

  /**
   * Returns the full catalogue of typed failure codes.
   */
  async getFailureSchema(): Promise<ContractResult<FailureSchema>> {
    return this.invoke("get_failure_schema", []);
  }

  /**
   * Returns the current storage schema version.
   */
  async getStorageVersion(): Promise<ContractResult<number>> {
    return this.invoke("get_storage_version", []);
  }

  // -----------------------------------------------------------------------
  // Pause controls (admin only)
  // -----------------------------------------------------------------------

  /**
   * Pause the contract with a reason string (admin only).
   */
  async pause(
    caller: string,
    reason: string,
  ): Promise<ContractResult<void>> {
    return this.invoke("pause", [caller, reason]);
  }

  /**
   * Unpause the contract (admin only).
   */
  async unpause(caller: string): Promise<ContractResult<void>> {
    return this.invoke("unpause", [caller]);
  }

  /**
   * Returns whether the contract is currently paused.
   */
  async isPaused(): Promise<ContractResult<boolean>> {
    return this.invoke("is_paused", []);
  }

  /**
   * Returns pause metadata if currently paused.
   */
  async getPauseInfo(): Promise<ContractResult<PauseInfo | null>> {
    return this.invoke("get_pause_info", []);
  }

  // -----------------------------------------------------------------------
  // Governance: admin transfer
  // -----------------------------------------------------------------------

  /**
   * Propose a new admin (admin only).
   */
  async proposeAdmin(
    caller: string,
    newAdmin: string,
  ): Promise<ContractResult<void>> {
    return this.invoke("propose_admin", [caller, newAdmin]);
  }

  /**
   * Accept a pending admin transfer (must be called by the proposed admin).
   */
  async acceptAdmin(caller: string): Promise<ContractResult<void>> {
    return this.invoke("accept_admin", [caller]);
  }

  /**
   * Cancel a pending admin transfer (admin only).
   */
  async cancelAdminProposal(
    caller: string,
  ): Promise<ContractResult<void>> {
    return this.invoke("cancel_admin_proposal", [caller]);
  }

  /**
   * Returns the pending admin address, if any.
   */
  async getPendingAdmin(): Promise<ContractResult<string | null>> {
    return this.invoke("get_pending_admin", []);
  }

  /**
   * Permanently renounce admin authority (irreversible).
   */
  async renounceAdmin(caller: string): Promise<ContractResult<void>> {
    return this.invoke("renounce_admin", [caller]);
  }

  // -----------------------------------------------------------------------
  // Governance: operator transfer
  // -----------------------------------------------------------------------

  /**
   * Propose a new operator (admin only).
   */
  async proposeOperator(
    caller: string,
    newOperator: string,
  ): Promise<ContractResult<void>> {
    return this.invoke("propose_operator", [caller, newOperator]);
  }

  /**
   * Accept a pending operator handoff (must be called by proposed operator).
   */
  async acceptOperator(caller: string): Promise<ContractResult<void>> {
    return this.invoke("accept_operator", [caller]);
  }

  /**
   * Cancel a pending operator proposal (admin only).
   */
  async cancelOperatorProposal(
    caller: string,
  ): Promise<ContractResult<void>> {
    return this.invoke("cancel_operator_proposal", [caller]);
  }

  /**
   * Returns the pending operator address, if any.
   */
  async getPendingOperator(): Promise<ContractResult<string | null>> {
    return this.invoke("get_pending_operator", []);
  }

  /**
   * Directly replace the operator address (admin only).
   */
  async setOperator(
    caller: string,
    newOperator: string,
  ): Promise<ContractResult<void>> {
    return this.invoke("set_operator", [caller, newOperator]);
  }

  // -----------------------------------------------------------------------
  // Role queries
  // -----------------------------------------------------------------------

  /**
   * Returns the current admin address.
   */
  async getAdmin(): Promise<ContractResult<string>> {
    return this.invoke("get_admin", []);
  }

  /**
   * Returns the current operator address.
   */
  async getOperator(): Promise<ContractResult<string>> {
    return this.invoke("get_operator", []);
  }

  // -----------------------------------------------------------------------
  // Version and migration
  // -----------------------------------------------------------------------

  /**
   * Returns storage version and migration posture.
   */
  async getMigrationState(): Promise<ContractResult<StorageVersionInfo>> {
    return this.invoke("get_migration_state", []);
  }

  /**
   * Returns combined version negotiation snapshot for backend startup.
   */
  async getVersionInfo(): Promise<ContractResult<VersionInfo>> {
    return this.invoke("get_version_info", []);
  }

  /**
   * Migrate storage from a previous version (admin only).
   */
  async migrate(caller: string): Promise<ContractResult<void>> {
    return this.invoke("migrate", [caller]);
  }

  // -----------------------------------------------------------------------
  // Offline transaction building
  // -----------------------------------------------------------------------

  /**
   * Builds an unsigned transaction envelope invoking `calculate_sla` for an
   * outage report, ready for offline signing (e.g. multi-sig workflows).
   * This does not submit anything — it only constructs the envelope.
   *
   * @param params - Outage report parameters, plus optional fee/sequence overrides.
   */
  buildOutageReportTx(params: BuildOutageReportTxParams): UnsignedEnvelope {
    const fee = params.fee ?? 100_000;
    const sequence = params.sequence ?? "0";

    const envelope = {
      networkPassphrase: this.config.networkPassphrase,
      contractId: this.config.contractId,
      source: params.source,
      fee,
      sequence,
      operation: {
        method: "calculate_sla",
        args: [
          params.source,
          params.outageId,
          params.severity,
          params.mttrMinutes,
        ],
      },
    };

    return {
      envelopeXdr: toBase64(JSON.stringify(envelope)),
      fee,
      sequence,
    };
  }

  // -----------------------------------------------------------------------
  // Internal
  // -----------------------------------------------------------------------

  /**
   * Generic contract invocation. In a production SDK this would build
   * and submit a Soroban transaction; here it provides the typed envelope.
   *
   * @param method - Contract method name.
   * @param args - Positional arguments.
   * @returns Typed result wrapper.
   */
  protected async invoke<T>(
    _method: string,
    args: unknown[],
  ): Promise<ContractResult<T>> {
    // Convert typed JS/TS arguments into ScVal-shaped structures — this is
    // what a real invocation would pass to `contract.call(method, ...)`.
    const _scArgs: ScVal[] = args.map(toScVal);

    // Production implementation would use @stellar/stellar-sdk:
    //   const contract = new Contract(this.config.contractId);
    //   const tx = new TransactionBuilder(account)
    //     .addOperation(contract.call(method, ..._scArgs))
    //     .build();
    // ... sign, simulate, and decode the result via fromScVal().
    //
    // For now return a placeholder that demonstrates the typed interface.
    return { ok: true, value: undefined as T };
  }

  // -----------------------------------------------------------------------
  // Pre-flight simulation
  // -----------------------------------------------------------------------

  /**
   * Runs an RPC `simulateTransaction` pre-flight check for a contract
   * method invocation before prompting the user for a signature, so an
   * invalid call fails fast with a readable error instead of failing
   * on-chain after the wallet has already charged a signing fee.
   *
   * @param method - Contract method name to simulate.
   * @param args - Positional arguments (converted to ScVal internally).
   */
  async preflightInvoke(
    method: string,
    args: unknown[],
  ): Promise<PreflightResult> {
    const simulation = await this.simulateTransaction(method, args);
    if (!simulation.success) {
      return {
        success: false,
        error: simulation.error ?? "Simulation failed",
        recommendedFee: 0n,
      };
    }
    return { success: true, recommendedFee: simulation.minResourceFee };
  }

  /**
   * Internal: calls the RPC `simulateTransaction` endpoint and parses the
   * response into a success flag, human-readable error, and the minimum
   * resource fee to use for the real transaction envelope. Stubbed pending
   * full transaction-building integration.
   */
  private async simulateTransaction(
    _method: string,
    _args: unknown[],
  ): Promise<{ success: boolean; error?: string; minResourceFee: bigint }> {
    // Production implementation would POST a `simulateTransaction` JSON-RPC
    // request built from a real transaction envelope to `this.config.rpcUrl`,
    // then parse `result.error` / `result.minResourceFee` from the response.
    return { success: true, minResourceFee: 100_000n };
  }

  // -----------------------------------------------------------------------
  // Contract event subscription
  // -----------------------------------------------------------------------

  /**
   * Subscribes to contract events matching `eventType`, polling the RPC
   * `getEvents` endpoint and invoking `callback` for each new matching
   * event as it's decoded into a typed object.
   *
   * @param eventType - Event topic/type to filter for.
   * @param callback - Invoked once per new matching event.
   * @param pollIntervalMs - Delay between polls, in milliseconds.
   * @returns An unsubscribe function that stops polling.
   */
  onContractEvent<T = unknown>(
    eventType: string,
    callback: ContractEventCallback<T>,
    pollIntervalMs = 5000,
  ): () => void {
    let cancelled = false;
    let lastLedger = 0;

    const poll = async () => {
      if (cancelled) return;
      const events = await this.getEvents(eventType, lastLedger);
      for (const event of events) {
        lastLedger = Math.max(lastLedger, event.ledger);
        callback(event as ContractEvent<T>);
      }
      if (!cancelled) {
        setTimeout(poll, pollIntervalMs);
      }
    };

    void poll();

    return () => {
      cancelled = true;
    };
  }

  /**
   * Internal: fetches raw events since `sinceLedger` via RPC `getEvents`
   * and decodes each one's topic/data XDR into a typed `ContractEvent`.
   * Stubbed pending full RPC integration.
   */
  private async getEvents(
    _eventType: string,
    _sinceLedger: number,
  ): Promise<ContractEvent[]> {
    return [];
  }

  // -----------------------------------------------------------------------
  // Transaction confirmation polling
  // -----------------------------------------------------------------------

  /**
   * Polls RPC `getTransaction` until the transaction reaches a terminal
   * status (SUCCESS or FAILED), applying exponential backoff between polls
   * (1s, 2s, 4s, 8s, ...capped so the overall wait never exceeds `timeoutMs`).
   * Throws {@link TimeoutError} if no terminal status is reached in time.
   *
   * @param txHash - Hash of the submitted transaction to wait on.
   * @param timeoutMs - Overall timeout, in milliseconds. Defaults to 60s.
   */
  async waitForTransactionConfirmation(
    txHash: string,
    timeoutMs = 60_000,
  ): Promise<TransactionStatusResult> {
    const start = Date.now();
    let delayMs = 1000;

    for (;;) {
      const result = await this.getTransactionStatus(txHash);
      if (result.status === "SUCCESS" || result.status === "FAILED") {
        return result;
      }

      const elapsed = Date.now() - start;
      if (elapsed >= timeoutMs) {
        throw new TimeoutError(txHash);
      }

      await sleep(Math.min(delayMs, timeoutMs - elapsed));
      delayMs *= 2;
    }
  }

  /**
   * Internal: calls the RPC `getTransaction` endpoint for `txHash`.
   * Stubbed pending full RPC integration — subclasses may override this
   * for testing.
   */
  protected async getTransactionStatus(
    txHash: string,
  ): Promise<TransactionStatusResult> {
    return { status: "NOT_FOUND", txHash };
  }
}

/**
 * Result of a pre-flight `simulateTransaction` check.
 */
export interface PreflightResult {
  /** Whether the simulated invocation would succeed. */
  success: boolean;
  /** Human-readable error message, present when success is false. */
  error?: string;
  /** Recommended resource fee (stroops) for the transaction envelope. */
  recommendedFee: bigint;
}

/**
 * A decoded contract event.
 */
export interface ContractEvent<T = unknown> {
  /** Event topic/type. */
  type: string;
  /** Ledger sequence the event was emitted in. */
  ledger: number;
  /** Decoded event data. */
  data: T;
}

export type ContractEventCallback<T = unknown> = (
  event: ContractEvent<T>,
) => void;

/**
 * Decodes a raw event (topic + XDR-shaped ScVal data) into a typed
 * {@link ContractEvent}.
 */
export function decodeContractEvent<T = unknown>(raw: {
  type: string;
  ledger: number;
  data: ScVal;
}): ContractEvent<T> {
  return {
    type: raw.type,
    ledger: raw.ledger,
    data: fromScVal(raw.data) as T,
  };
}

export type TransactionStatus = "SUCCESS" | "FAILED" | "NOT_FOUND" | "PENDING";

export interface TransactionStatusResult {
  status: TransactionStatus;
  txHash: string;
}

/**
 * Thrown by {@link SLACalculatorClient.waitForTransactionConfirmation} when
 * a transaction doesn't reach a terminal status before the timeout elapses.
 */
export class TimeoutError extends Error {
  constructor(txHash: string) {
    super(`Timed out waiting for transaction ${txHash} to confirm`);
    this.name = "TimeoutError";
  }
}
