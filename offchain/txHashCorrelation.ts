/**
 * SC-W5-112: Duplicate transaction hash correlation strategy for retries.
 *
 * When the backend retries a submission, Stellar may return a duplicate hash
 * (same envelope resubmitted) or a new hash (rebuilt tx). This module tracks
 * correlation between original and retry submissions to prevent double-application
 * of contract calls (e.g. calculate_sla called twice for the same outage_id).
 */

export interface TxRecord {
  outage_id: string;
  tx_hash: string;
  submitted_at: number; // unix ms
  confirmed: boolean;
}

/** In-memory correlation registry (backend would back this with persistent storage). */
export class TxHashCorrelationRegistry {
  private records = new Map<string, TxRecord>(); // key: outage_id

  /**
   * Attempts to register a submission.
   * Returns false (duplicate detected) if the outage_id already has a confirmed tx.
   */
  register(record: TxRecord): { accepted: boolean; reason: string } {
    const existing = this.records.get(record.outage_id);
    if (existing?.confirmed) {
      return { accepted: false, reason: `outage_id ${record.outage_id} already confirmed via ${existing.tx_hash}` };
    }
    this.records.set(record.outage_id, record);
    return { accepted: true, reason: "registered" };
  }

  confirm(outage_id: string, tx_hash: string): void {
    const r = this.records.get(outage_id);
    if (!r) throw new Error(`[SC-W5-112] No record for outage_id ${outage_id}`);
    if (r.tx_hash !== tx_hash) throw new Error(`[SC-W5-112] Hash mismatch: expected ${r.tx_hash}, got ${tx_hash}`);
    r.confirmed = true;
  }

  isConfirmed(outage_id: string): boolean {
    return this.records.get(outage_id)?.confirmed ?? false;
  }
}

function runTests(): void {
  console.log("[SC-W5-112] Duplicate tx hash correlation tests\n");
  const reg = new TxHashCorrelationRegistry();

  // Normal registration
  const r1 = reg.register({ outage_id: "OUT001", tx_hash: "HASH1", submitted_at: 1000, confirmed: false });
  if (!r1.accepted) throw new Error("Expected accepted");
  console.log("  ✓ first submission accepted");

  // Retry before confirmation — allowed (tx may not have landed)
  const r2 = reg.register({ outage_id: "OUT001", tx_hash: "HASH1b", submitted_at: 2000, confirmed: false });
  if (!r2.accepted) throw new Error("Expected retry accepted before confirmation");
  console.log("  ✓ retry before confirmation accepted");

  // Confirm
  reg.confirm("OUT001", "HASH1b");
  if (!reg.isConfirmed("OUT001")) throw new Error("Expected confirmed");
  console.log("  ✓ confirmation recorded");

  // Retry after confirmation — must be rejected
  const r3 = reg.register({ outage_id: "OUT001", tx_hash: "HASH1c", submitted_at: 3000, confirmed: false });
  if (r3.accepted) throw new Error("Should reject post-confirmation duplicate");
  console.log("  ✓ post-confirmation duplicate rejected");

  // Hash mismatch on confirm
  const reg2 = new TxHashCorrelationRegistry();
  reg2.register({ outage_id: "OUT002", tx_hash: "HASH2", submitted_at: 1000, confirmed: false });
  try {
    reg2.confirm("OUT002", "WRONG_HASH");
    throw new Error("Should have thrown");
  } catch (e: any) {
    if (!e.message.includes("SC-W5-112")) throw e;
    console.log("  ✓ adversarial: hash mismatch on confirm rejected");
  }

  console.log("\nAll duplicate tx hash correlation tests passed.");
}

runTests();
