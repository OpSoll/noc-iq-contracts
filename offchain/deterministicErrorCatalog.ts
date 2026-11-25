export type ErrorCode = string;

export interface CatalogEntry {
  code: ErrorCode;
  message: string;
  retryable: boolean;
  bridgeFacing: boolean;
  /** Numeric value of the matching `SLAError` variant, when the error comes from the contract. */
  contractCode?: number;
  /** Name of the matching `SLAError` variant in `sla_calculator/src/lib.rs`. */
  contractName?: string;
}

export const errorCatalog: CatalogEntry[] = [
  { code: "OUTAGE_NOT_FOUND",     message: "Outage ID does not exist in storage.",            retryable: false, bridgeFacing: true },
  { code: "CALC_OVERFLOW",        message: "Calculation exceeded safe numeric bounds.",        retryable: false, bridgeFacing: true },
  { code: "CONFIG_LOCKED",        message: "Configuration is frozen and cannot be mutated.",  retryable: false, bridgeFacing: true },
  { code: "QUORUM_NOT_MET",       message: "Governance action lacks required quorum.",         retryable: true,  bridgeFacing: true },
  { code: "DUPLICATE_SUBMISSION", message: "This outage ID has already been processed.",      retryable: false, bridgeFacing: true },

  // One entry per `SLAError` variant. `scripts/check-error-catalog.ts` (and CI) fail when this list
  // and the contract's enum diverge, so a new contract error cannot ship without a message here.
  { code: "ALREADY_INITIALIZED", contractCode: 1, contractName: "AlreadyInitialized", message: "The contract is already initialized.", retryable: false, bridgeFacing: false },
  { code: "NOT_INITIALIZED", contractCode: 2, contractName: "NotInitialized", message: "The contract has not been initialized.", retryable: false, bridgeFacing: true },
  { code: "UNAUTHORIZED", contractCode: 3, contractName: "Unauthorized", message: "The caller is not authorized for this action.", retryable: false, bridgeFacing: true },
  { code: "CONFIG_NOT_FOUND", contractCode: 4, contractName: "ConfigNotFound", message: "No SLA configuration exists for this severity.", retryable: false, bridgeFacing: true },
  { code: "VERSION_MISMATCH", contractCode: 5, contractName: "VersionMismatch", message: "The stored schema version does not match this contract; a migration is required.", retryable: false, bridgeFacing: true },
  { code: "CONTRACT_PAUSED", contractCode: 6, contractName: "ContractPaused", message: "The contract is paused and mutating calls are blocked.", retryable: false, bridgeFacing: true },
  { code: "NO_PENDING_TRANSFER", contractCode: 7, contractName: "NoPendingTransfer", message: "There is no pending transfer to accept or cancel.", retryable: false, bridgeFacing: true },
  { code: "INVALID_THRESHOLD", contractCode: 8, contractName: "InvalidThreshold", message: "The SLA threshold is invalid.", retryable: false, bridgeFacing: true },
  { code: "INVALID_PENALTY", contractCode: 9, contractName: "InvalidPenalty", message: "The penalty per minute is invalid.", retryable: false, bridgeFacing: true },
  { code: "INVALID_REWARD", contractCode: 10, contractName: "InvalidReward", message: "The reward base is invalid.", retryable: false, bridgeFacing: true },
  { code: "INVALID_SEVERITY", contractCode: 11, contractName: "InvalidSeverity", message: "The severity is not recognized.", retryable: false, bridgeFacing: true },
  { code: "RETENTION_LIMIT_OUT_OF_RANGE", contractCode: 12, contractName: "RetentionLimitOutOfRange", message: "The history retention limit is outside the allowed range.", retryable: false, bridgeFacing: true },
  { code: "DUPLICATE_OUTAGE_INPUT", contractCode: 13, contractName: "DuplicateOutageInput", message: "The same outage was submitted more than once in this batch.", retryable: false, bridgeFacing: true },
  { code: "INVALID_PENALTY_AMOUNT", contractCode: 14, contractName: "InvalidPenaltyAmount", message: "The computed penalty amount is invalid.", retryable: false, bridgeFacing: true },
  { code: "INVALID_REWARD_AMOUNT", contractCode: 15, contractName: "InvalidRewardAmount", message: "The computed reward amount is invalid.", retryable: false, bridgeFacing: true },
  { code: "INVALID_OUTAGE_ID", contractCode: 16, contractName: "InvalidOutageId", message: "The outage ID is invalid.", retryable: false, bridgeFacing: true },
  { code: "MALFORMED_SYMBOL_INPUT", contractCode: 17, contractName: "MalformedSymbolInput", message: "An input symbol is malformed.", retryable: false, bridgeFacing: true },
  { code: "INVALID_MTTR", contractCode: 18, contractName: "InvalidMTTR", message: "The mean time to repair is invalid.", retryable: false, bridgeFacing: true },
  { code: "THRESHOLD_OUT_OF_BOUNDS", contractCode: 19, contractName: "ThresholdOutOfBounds", message: "The threshold is outside the allowed bounds.", retryable: false, bridgeFacing: true },
  { code: "PENALTY_OUT_OF_BOUNDS", contractCode: 20, contractName: "PenaltyOutOfBounds", message: "The penalty is outside the allowed bounds.", retryable: false, bridgeFacing: true },
  { code: "REWARD_OUT_OF_BOUNDS", contractCode: 21, contractName: "RewardOutOfBounds", message: "The reward is outside the allowed bounds.", retryable: false, bridgeFacing: true },
  { code: "INVALID_MONTH", contractCode: 22, contractName: "InvalidMonth", message: "The month is invalid.", retryable: false, bridgeFacing: true },
  { code: "INVALID_TIMESTAMP_SEQUENCE", contractCode: 23, contractName: "InvalidTimestampSequence", message: "Outage timestamps are out of order.", retryable: false, bridgeFacing: true },
  { code: "DIVISION_BY_ZERO", contractCode: 24, contractName: "DivisionByZero", message: "A calculation would divide by zero.", retryable: false, bridgeFacing: true },
  // #1040 — out-of-range SLA thresholds name the bound that rejected them.
  { code: "THRESHOLD_BELOW_MINIMUM", contractCode: 25, contractName: "ThresholdBelowMinimum", message: "The SLA threshold is below the minimum of 1 minute.", retryable: false, bridgeFacing: true },
  { code: "THRESHOLD_ABOVE_MAXIMUM", contractCode: 26, contractName: "ThresholdAboveMaximum", message: "The SLA threshold exceeds the maximum of 1440 minutes.", retryable: false, bridgeFacing: true },
];

export function lookupError(code: ErrorCode): CatalogEntry | undefined {
  return errorCatalog.find((e) => e.code === code);
}

export function bridgeFacingErrors(): CatalogEntry[] {
  return errorCatalog.filter((e) => e.bridgeFacing);
}

export function publishCatalog(): Record<ErrorCode, Omit<CatalogEntry, "code">> {
  return Object.fromEntries(errorCatalog.map(({ code, ...rest }) => [code, rest]));
}

/** Returned for any runtime error the catalog does not know. It never echoes the raw payload. */
export const GENERIC_ERROR: CatalogEntry = {
  code: "UNKNOWN_ERROR",
  message: "The request failed with an unrecognized error.",
  retryable: false,
  bridgeFacing: false,
};

const CONTRACT_ERROR_PATTERN = /Error\(Contract, #(\d+)\)/;

/**
 * Maps a runtime error (a numeric contract code, or a string containing `Error(Contract, #N)`)
 * to its catalog entry. Anything unrecognized gets {@link GENERIC_ERROR}: a safe generic message
 * with no part of the raw error in it.
 */
export function describeRuntimeError(raw: unknown): CatalogEntry {
  let code: number | undefined;
  if (typeof raw === "number" && Number.isInteger(raw)) code = raw;
  else if (typeof raw === "string") {
    const m = CONTRACT_ERROR_PATTERN.exec(raw);
    if (m) code = Number(m[1]);
  }
  const hit = code === undefined ? undefined : errorCatalog.find((e) => e.contractCode === code);
  return hit ?? GENERIC_ERROR;
}

export interface ContractErrorVariant {
  name: string;
  code: number;
}

/** Extracts the `SLAError` variants from the contract source. */
export function parseContractErrors(rustSource: string): ContractErrorVariant[] {
  const body = /pub enum SLAError\s*\{([\s\S]*?)\n\}/.exec(rustSource);
  if (!body) throw new Error("pub enum SLAError not found in contract source");
  const variants: ContractErrorVariant[] = [];
  for (const line of body[1]!.split("\n")) {
    const m = /^\s*([A-Za-z0-9_]+)\s*=\s*(\d+)\s*,/.exec(line);
    if (m) variants.push({ name: m[1]!, code: Number(m[2]) });
  }
  return variants;
}

export interface CatalogDrift {
  /** Contract errors with no catalog entry. */
  missingFromCatalog: ContractErrorVariant[];
  /** Catalog entries that name a contract error that no longer exists, or whose name/code disagree. */
  staleInCatalog: string[];
}

/** Compares the contract's error enum with the catalog; both lists empty means exhaustive. */
export function compareCatalogToContract(rustSource: string): CatalogDrift {
  const variants = parseContractErrors(rustSource);
  const missingFromCatalog = variants.filter(
    (v) => !errorCatalog.some((e) => e.contractCode === v.code && e.contractName === v.name)
  );
  const staleInCatalog = errorCatalog
    .filter((e) => e.contractCode !== undefined || e.contractName !== undefined)
    .filter((e) => !variants.some((v) => v.code === e.contractCode && v.name === e.contractName))
    .map((e) => e.code);
  return { missingFromCatalog, staleInCatalog };
}
