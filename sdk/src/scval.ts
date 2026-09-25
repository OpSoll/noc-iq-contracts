/**
 * Soroban ScVal conversion helpers.
 *
 * Lightweight, dependency-free helpers for converting JS/TS primitive
 * values into a typed, Soroban ScVal-shaped structure and back. A
 * production build would delegate the actual XDR encoding/decoding to
 * `@stellar/stellar-sdk`'s `xdr.ScVal`, but the type tagging and value
 * mapping here mirror that conversion so the client's typed methods have
 * a single place to route argument/result conversion through.
 */

export type ScValType =
  | "void"
  | "bool"
  | "u32"
  | "i32"
  | "u128"
  | "i128"
  | "string"
  | "symbol"
  | "address"
  | "vec"
  | "map";

export interface ScVal {
  type: ScValType;
  value: unknown;
}

/** Stellar strkey account (G...) or contract (C...) address pattern. */
const ADDRESS_PATTERN = /^[GC][A-Z2-7]{55}$/;

/**
 * Converts a JS/TS primitive value into a typed ScVal-shaped structure.
 *
 * Numbers are mapped to u32/i32, bigints to u128/i128 (by sign), strings
 * that look like a Stellar address to the `address` type, arrays to `vec`,
 * and plain objects to `map`.
 */
export function toScVal(value: unknown): ScVal {
  if (value === null || value === undefined) {
    return { type: "void", value: null };
  }
  if (typeof value === "boolean") {
    return { type: "bool", value };
  }
  if (typeof value === "bigint") {
    return { type: value < 0n ? "i128" : "u128", value };
  }
  if (typeof value === "number") {
    if (!Number.isInteger(value)) {
      throw new TypeError(
        `Cannot convert non-integer number ${value} to ScVal — use a bigint for fractional/large values`,
      );
    }
    return { type: value < 0 ? "i32" : "u32", value };
  }
  if (typeof value === "string") {
    return { type: ADDRESS_PATTERN.test(value) ? "address" : "string", value };
  }
  if (Array.isArray(value)) {
    return { type: "vec", value: value.map(toScVal) };
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>).map(
      ([key, val]) => [key, toScVal(val)] as const,
    );
    return { type: "map", value: entries };
  }
  throw new TypeError(`Cannot convert value of type ${typeof value} to ScVal`);
}

/**
 * Converts an ScVal-shaped structure back into a plain JS/TS value,
 * recursively unwrapping `vec` and `map` entries.
 */
export function fromScVal(scVal: ScVal): unknown {
  switch (scVal.type) {
    case "void":
      return null;
    case "vec":
      return (scVal.value as ScVal[]).map(fromScVal);
    case "map":
      return Object.fromEntries(
        (scVal.value as Array<readonly [string, ScVal]>).map(([key, val]) => [
          key,
          fromScVal(val),
        ]),
      );
    default:
      return scVal.value;
  }
}
