/**
 * SC-W5-115: Failure recovery semantics for partially observed tx results.
 *
 * A "partially observed" result occurs when the backend receives a response
 * from Stellar that is incomplete or contradictory (e.g. response received
 * but ledger inclusion unconfirmed, or error after event emission).
 * Defines recovery classifications and safe handling per partial observation type.
 */

import { errorCatalog } from "./deterministicErrorCatalog";

export type PartialObservationType =
  | "response_no_ledger"   // got a Soroban response but no ledger sequence
  | "event_no_result"      // saw contract event but no function return value
  | "result_no_event"      // got return value but no event emitted
  | "ledger_no_response"   // tx in ledger but backend call timed out
  | "error_after_event";   // error response but event was already emitted

export type RecoveryAction =
  | "query_and_verify"   // re-read contract state; do not retry yet
  | "safe_to_retry"      // definitively not applied; retry allowed
  | "manual_review";     // contradictory signals; escalate to operator

export interface PartialObservation {
  outage_id: string;
  type: PartialObservationType;
}

export interface RecoverySemantics {
  action: RecoveryAction;
  rationale: string;
  idempotent: boolean; // whether retrying is safe without re-checking state
}

const SEMANTICS: Record<PartialObservationType, RecoverySemantics> = {
  response_no_ledger: {
    action: "query_and_verify",
    rationale: "Soroban response received but ledger unconfirmed; tx may or may not have landed",
    idempotent: false,
  },
  event_no_result: {
    action: "query_and_verify",
    rationale: "Contract event observed implies state was written; verify before retry",
    idempotent: false,
  },
  result_no_event: {
    action: "query_and_verify",
    rationale: "Return value without event is unexpected; query state to resolve",
    idempotent: false,
  },
  ledger_no_response: {
    action: "query_and_verify",
    rationale: "Tx confirmed in ledger; backend just missed the response — verify state, no retry",
    idempotent: false,
  },
  error_after_event: {
    action: "manual_review",
    rationale: "Contradictory: event emitted but function errored — operator must reconcile",
    idempotent: false,
  },
};

export function classifyRecovery(obs: PartialObservation): RecoverySemantics {
  return SEMANTICS[obs.type];
}

// ---------------------------------------------------------------------------
// #1059: retry eligibility by failure class
// ---------------------------------------------------------------------------

export type FailureClass =
  | "permanent_auth"     // caller is not authorized; retrying cannot help
  | "permanent_config"   // contract/config state forbids the call until an operator acts
  | "permanent_request"  // the request itself is invalid or already processed
  | "transient_rpc"      // network / RPC trouble; the same call may succeed later
  | "transient_sequence" // stale account sequence; rebuild with a fresh sequence and retry
  | "transient_state"    // state may change by itself (for example a quorum forming)
  | "unknown";

export interface FailurePolicy {
  failure_class: FailureClass;
  retryable: boolean;
  /** Upper bound on retries; 0 for permanent failures. */
  max_retries: number;
  backoff_ms: number;
  /** Stable code for logs and dashboards. */
  diagnostic_code: string;
}

const PERMANENT: Omit<FailurePolicy, "failure_class" | "diagnostic_code"> = { retryable: false, max_retries: 0, backoff_ms: 0 };
const TRANSIENT: Omit<FailurePolicy, "failure_class" | "diagnostic_code"> = { retryable: true, max_retries: 5, backoff_ms: 1_000 };

/** Failure code (contract `SLAError` name, catalog code, or RPC/HTTP code) -> failure class. */
const FAILURE_CLASSES: Record<string, FailureClass> = {
  // authorization
  Unauthorized: "permanent_auth",
  TX_BAD_AUTH: "permanent_auth",
  AUTH_REQUIRED: "permanent_auth",
  // configuration / contract state
  NotInitialized: "permanent_config",
  ConfigNotFound: "permanent_config",
  VersionMismatch: "permanent_config",
  ContractPaused: "permanent_config",
  CONFIG_LOCKED: "permanent_config",
  // the request itself
  InvalidThreshold: "permanent_request",
  // #1040 — bound-specific threshold rejections are still caller errors.
  ThresholdBelowMinimum: "permanent_request",
  ThresholdAboveMaximum: "permanent_request",
  InvalidSeverity: "permanent_request",
  InvalidOutageId: "permanent_request",
  DuplicateOutageInput: "permanent_request",
  OUTAGE_NOT_FOUND: "permanent_request",
  CALC_OVERFLOW: "permanent_request",
  DUPLICATE_SUBMISSION: "permanent_request",
  // transient transport problems
  RPC_TIMEOUT: "transient_rpc",
  RPC_UNAVAILABLE: "transient_rpc",
  NETWORK_ERROR: "transient_rpc",
  HTTP_429: "transient_rpc",
  HTTP_503: "transient_rpc",
  TRY_AGAIN_LATER: "transient_rpc",
  // stale sequence number
  TX_BAD_SEQ: "transient_sequence",
  SEQUENCE_MISMATCH: "transient_sequence",
  // state that can resolve on its own
  QUORUM_NOT_MET: "transient_state",
};

/**
 * Unknown failures get a bounded, conservative policy: one retry after a pause, and a diagnostic
 * code so they show up and get classified. The raw failure text is never copied into the code.
 */
const UNKNOWN_POLICY: FailurePolicy = {
  failure_class: "unknown",
  retryable: true,
  max_retries: 1,
  backoff_ms: 5_000,
  diagnostic_code: "FAILURE_UNCLASSIFIED",
};

export function classifyFailureCode(code: string): FailurePolicy {
  const failure_class = Object.prototype.hasOwnProperty.call(FAILURE_CLASSES, code) ? FAILURE_CLASSES[code] : undefined;
  if (failure_class === undefined) return { ...UNKNOWN_POLICY };
  const base = failure_class.startsWith("permanent_") ? PERMANENT : TRANSIENT;
  return { ...base, failure_class, diagnostic_code: `FAILURE_${failure_class.toUpperCase()}` };
}

function runFailureClassTests(): void {
  console.log("\n[#1059] Failure class retry eligibility tests\n");

  const table: Array<[string, FailureClass, boolean]> = [
    ["Unauthorized", "permanent_auth", false],
    ["TX_BAD_AUTH", "permanent_auth", false],
    ["ConfigNotFound", "permanent_config", false],
    ["ContractPaused", "permanent_config", false],
    ["VersionMismatch", "permanent_config", false],
    ["CONFIG_LOCKED", "permanent_config", false],
    ["InvalidSeverity", "permanent_request", false],
    ["DUPLICATE_SUBMISSION", "permanent_request", false],
    ["RPC_TIMEOUT", "transient_rpc", true],
    ["HTTP_503", "transient_rpc", true],
    ["TX_BAD_SEQ", "transient_sequence", true],
    ["QUORUM_NOT_MET", "transient_state", true],
  ];
  for (const [code, cls, retryable] of table) {
    const p = classifyFailureCode(code);
    if (p.failure_class !== cls) throw new Error(`${code}: expected class ${cls}, got ${p.failure_class}`);
    if (p.retryable !== retryable) throw new Error(`${code}: expected retryable=${retryable}`);
    if (!retryable && p.max_retries !== 0) throw new Error(`${code}: permanent failures must have max_retries=0`);
    if (retryable && p.max_retries <= 0) throw new Error(`${code}: transient failures need a retry budget`);
    console.log(`  ✓ ${code} -> ${cls} (retryable=${retryable})`);
  }

  // Unknown failures: bounded, safe, with a diagnostic code that does not echo the input.
  for (const code of ["", "SOMETHING_NEW", "constructor", "__proto__", "Error: balance of GABC below 5"]) {
    const p = classifyFailureCode(code);
    if (p.failure_class !== "unknown" || p.diagnostic_code !== "FAILURE_UNCLASSIFIED")
      throw new Error(`unknown code ${JSON.stringify(code)} must use the unclassified policy`);
    if (p.max_retries > 1) throw new Error("unknown failures must be tightly bounded");
    if (p.diagnostic_code.includes(code) && code !== "") throw new Error("diagnostic code must not echo the raw failure");
  }
  console.log("  ✓ unknown failures get the bounded unclassified policy and a stable diagnostic code");

  // Wherever a catalog code is classified, retry eligibility agrees with the catalog's own flag.
  let compared = 0;
  for (const entry of errorCatalog) {
    const p = classifyFailureCode(entry.code);
    if (p.failure_class === "unknown") continue;
    if (p.retryable !== entry.retryable) throw new Error(`catalog code ${entry.code}: retry eligibility disagrees with the catalog`);
    compared++;
  }
  if (compared === 0) throw new Error("no catalog code was classified; the failure table and the catalog have diverged");
  console.log(`  ✓ failure classes agree with the error catalog's retryable flags (${compared} codes)`);

  console.log("\nAll failure class tests passed.");
}

function runTests(): void {
  console.log("[SC-W5-115] Failure recovery semantics tests\n");

  const cases: Array<{ type: PartialObservationType; expectedAction: RecoveryAction }> = [
    { type: "response_no_ledger", expectedAction: "query_and_verify" },
    { type: "event_no_result",    expectedAction: "query_and_verify" },
    { type: "result_no_event",    expectedAction: "query_and_verify" },
    { type: "ledger_no_response", expectedAction: "query_and_verify" },
    { type: "error_after_event",  expectedAction: "manual_review" },
  ];

  for (const c of cases) {
    const sem = classifyRecovery({ outage_id: "OUT001", type: c.type });
    if (sem.action !== c.expectedAction)
      throw new Error(`${c.type}: expected ${c.expectedAction}, got ${sem.action}`);
    if (sem.idempotent)
      throw new Error(`${c.type}: all partial observations must be non-idempotent`);
    console.log(`  ✓ ${c.type} -> ${sem.action}`);
  }

  // Adversarial: error_after_event must not be retried without manual review
  const risky = classifyRecovery({ outage_id: "OUT002", type: "error_after_event" });
  if (risky.action === "safe_to_retry")
    throw new Error("error_after_event must never be safe_to_retry");
  console.log("  ✓ adversarial: error_after_event not safe_to_retry");

  console.log("\nAll failure recovery semantics tests passed.");
}

runTests();
runFailureClassTests();
