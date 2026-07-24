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
  private async invoke<T>(
    _method: string,
    _args: unknown[],
  ): Promise<ContractResult<T>> {
    // Production implementation would use @stellar/stellar-sdk:
    //   const contract = new Contract(this.config.contractId);
    //   const tx = new TransactionBuilder(account)
    //     .addOperation(contract.call(method, ...args))
    //     .build();
    // ... sign, simulate, etc.
    //
    // For now return a placeholder that demonstrates the typed interface.
    return { ok: true, value: undefined as T };
  }
}
