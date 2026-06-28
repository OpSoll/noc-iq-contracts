import { describe, it, expect } from "vitest";

const MAX_SYMBOL_LEN  = 12;
const MAX_OUTAGE_LEN  = 64;
const SYMBOL_PATTERN  = /^[A-Z0-9_]+$/;

function validateSymbol(s: string): string | null {
  if (!s)                        return "Symbol must not be empty";
  if (s.length > MAX_SYMBOL_LEN) return `Symbol exceeds max length ${MAX_SYMBOL_LEN}`;
  if (!SYMBOL_PATTERN.test(s))   return "Symbol contains invalid characters";
  return null;
}

function validateOutageId(id: string): string | null {
  if (!id)                      return "Outage ID must not be empty";
  if (id.length > MAX_OUTAGE_LEN) return `Outage ID exceeds ${MAX_OUTAGE_LEN} chars`;
  return null;
}

describe("defensive input-length and symbol-domain constraints", () => {
  it("rejects empty symbol",                () => expect(validateSymbol("")).not.toBeNull());
  it("rejects symbol over max length",      () => expect(validateSymbol("A".repeat(13))).not.toBeNull());
  it("rejects symbol with invalid chars",   () => expect(validateSymbol("bad-sym!")).not.toBeNull());
  it("accepts valid symbol",                () => expect(validateSymbol("NOC_IQ")).toBeNull());
  it("rejects empty outage ID",             () => expect(validateOutageId("")).not.toBeNull());
  it("rejects outage ID over max length",   () => expect(validateOutageId("x".repeat(65))).not.toBeNull());
  it("accepts valid outage ID",             () => expect(validateOutageId("OUTAGE-2025-001")).toBeNull());
});
