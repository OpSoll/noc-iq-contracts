import { describe, it, expect } from "vitest";
import { MockSlaCalculatorClient } from "../src/mock-client";

describe("MockSlaCalculatorClient", () => {
  it("returns the default stub response when no mock is configured", async () => {
    const client = new MockSlaCalculatorClient();
    const result = await client.isPaused();
    expect(result.ok).toBe(true);
  });

  it("returns a configured mock value for a given method", async () => {
    const client = new MockSlaCalculatorClient();
    client.mockResponse("get_admin", { value: "GADMIN123" });
    const result = await client.getAdmin();
    expect(result.ok).toBe(true);
    expect(result.value).toBe("GADMIN123");
  });

  it("returns a configured mock error for a given method", async () => {
    const client = new MockSlaCalculatorClient();
    client.mockResponse("get_admin", { error: "not initialized" });
    const result = await client.getAdmin();
    expect(result.ok).toBe(false);
    expect(result.error).toBe("not initialized");
  });

  it("records call history with method name and arguments", async () => {
    const client = new MockSlaCalculatorClient();
    await client.calculateSla("op1", "outage-1", "high", 42);
    expect(client.calls).toEqual([
      { method: "calculate_sla", args: ["op1", "outage-1", "high", 42] },
    ]);
  });

  it("records multiple calls in order", async () => {
    const client = new MockSlaCalculatorClient();
    await client.getAdmin();
    await client.getOperator();
    expect(client.calls.map((c) => c.method)).toEqual([
      "get_admin",
      "get_operator",
    ]);
  });

  it("reset() clears both mock responses and call history", async () => {
    const client = new MockSlaCalculatorClient();
    client.mockResponse("get_admin", { value: "GADMIN123" });
    await client.getAdmin();
    client.reset();

    expect(client.calls).toEqual([]);
    const result = await client.getAdmin();
    // Falls back to the default stub, not the cleared mock value.
    expect(result.value).not.toBe("GADMIN123");
  });
});
