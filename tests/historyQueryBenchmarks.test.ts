import { describe, it, expect } from "vitest";

interface HistoryRecord { outageId: string; timestamp: number; value: number; }

function queryHistory(records: HistoryRecord[], outageId: string, limit: number): HistoryRecord[] {
  return records.filter((r) => r.outageId === outageId).slice(0, limit);
}

function generateRecords(count: number): HistoryRecord[] {
  return Array.from({ length: count }, (_, i) => ({
    outageId:  `OUTAGE-${(i % 100).toString().padStart(3, "0")}`,
    timestamp: Date.now() - i * 1000,
    value:     i % 999,
  }));
}

describe("high-volume history query performance benchmarks", () => {
  const records = generateRecords(10_000);

  it("queries 10k records in under 50ms", () => {
    const start = Date.now();
    queryHistory(records, "OUTAGE-001", 100);
    expect(Date.now() - start).toBeLessThan(50);
  });

  it("returns no more than the requested limit", () => {
    expect(queryHistory(records, "OUTAGE-001", 10).length).toBeLessThanOrEqual(10);
  });

  it("returns only matching outage records", () => {
    const res = queryHistory(records, "OUTAGE-042", 50);
    expect(res.every((r) => r.outageId === "OUTAGE-042")).toBe(true);
  });
});
