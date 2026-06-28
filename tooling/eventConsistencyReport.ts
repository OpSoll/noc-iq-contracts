export interface ContractEvent {
  name: string;
  fields: string[];
  emittedAt: number;
  txHash: string;
}

export interface ConsistencyReport {
  totalEvents: number;
  missingFields: Record<string, string[]>;
  duplicateTxHashes: string[];
  consistent: boolean;
}

const REQUIRED_FIELDS = ["outageId", "timestamp", "actor"];

export function generateConsistencyReport(events: ContractEvent[]): ConsistencyReport {
  const missingFields: Record<string, string[]> = {};
  const txSeen = new Map<string, number>();

  for (const e of events) {
    const missing = REQUIRED_FIELDS.filter((f) => !e.fields.includes(f));
    if (missing.length) missingFields[e.name] = missing;
    txSeen.set(e.txHash, (txSeen.get(e.txHash) ?? 0) + 1);
  }

  const duplicateTxHashes = [...txSeen.entries()].filter(([, n]) => n > 1).map(([h]) => h);
  return {
    totalEvents: events.length,
    missingFields,
    duplicateTxHashes,
    consistent: !Object.keys(missingFields).length && !duplicateTxHashes.length,
  };
}
