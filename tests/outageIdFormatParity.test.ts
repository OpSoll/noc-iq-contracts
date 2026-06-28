import { describe, it, expect } from "vitest";

// Canonical format agreed with backend: YYYY-{REGION}-{SEQ:06d}
const CANONICAL = /^\d{4}-[A-Z]{2,8}-\d{6}$/;

function isCanonical(id: string): boolean { return CANONICAL.test(id); }

function parseOutageId(id: string): { year: string; region: string; seq: string } | null {
  const m = id.match(/^(\d{4})-([A-Z]{2,8})-(\d{6})$/);
  return m ? { year: m[1], region: m[2], seq: m[3] } : null;
}

describe("outage ID canonical format — backend parity tests", () => {
  it("accepts valid canonical IDs", () => {
    expect(isCanonical("2025-US-000001")).toBe(true);
    expect(isCanonical("2025-EMEA-000999")).toBe(true);
    expect(isCanonical("2025-APAC-000042")).toBe(true);
  });

  it("rejects non-canonical formats", () => {
    expect(isCanonical("OUTAGE-001")).toBe(false);
    expect(isCanonical("2025_US_000001")).toBe(false);
    expect(isCanonical("2025-us-000001")).toBe(false);
  });

  it("parses a canonical ID into components", () => {
    expect(parseOutageId("2025-APAC-000042")).toEqual({ year: "2025", region: "APAC", seq: "000042" });
  });

  it("returns null for non-canonical ID", () => {
    expect(parseOutageId("bad-id")).toBeNull();
  });
});
