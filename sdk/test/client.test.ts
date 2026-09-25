import { describe, it, expect } from "vitest";
import {
  SLACalculatorClient,
  TimeoutError,
  decodeContractEvent,
  TransactionStatusResult,
} from "../src/client";
import { toScVal, fromScVal } from "../src/scval";
import { CANONICAL_SEVERITIES } from "../src/types";

describe("SLACalculatorClient", () => {
  const client = new SLACalculatorClient({
    contractId: "CABC1234567890ABCDEF",
    networkPassphrase: "Testnet ; SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
  });

  it("exposes contract address and network", () => {
    expect(client.contractAddress).toBe("CABC1234567890ABCDEF");
    expect(client.network).toContain("Testnet");
  });

  describe("configuration methods", () => {
    it("getConfig returns ok", async () => {
      const result = await client.getConfig("critical");
      expect(result.ok).toBe(true);
    });

    it("listConfigs returns ok", async () => {
      const result = await client.listConfigs();
      expect(result.ok).toBe(true);
    });

    it("getConfigSnapshot returns ok", async () => {
      const result = await client.getConfigSnapshot();
      expect(result.ok).toBe(true);
    });

    it("getConfigVersionHash returns ok", async () => {
      const result = await client.getConfigVersionHash();
      expect(result.ok).toBe(true);
    });

    it("getConfigCount returns ok", async () => {
      const result = await client.getConfigCount();
      expect(result.ok).toBe(true);
    });
  });

  describe("SLA calculation methods", () => {
    it("calculateSla returns ok", async () => {
      const result = await client.calculateSla(
        "operator1",
        "outage-001",
        "high",
        90,
      );
      expect(result.ok).toBe(true);
    });

    it("calculateSlaView returns ok", async () => {
      const result = await client.calculateSlaView(
        "outage-002",
        "critical",
        60,
      );
      expect(result.ok).toBe(true);
    });
  });

  describe("history methods", () => {
    it("getHistory returns ok", async () => {
      const result = await client.getHistory();
      expect(result.ok).toBe(true);
    });

    it("getHistoryPage returns ok", async () => {
      const result = await client.getHistoryPage(0, 10);
      expect(result.ok).toBe(true);
    });

    it("getHistoryByOutage returns ok", async () => {
      const result = await client.getHistoryByOutage("outage-001");
      expect(result.ok).toBe(true);
    });

    it("getLatestByOutage returns ok", async () => {
      const result = await client.getLatestByOutage("outage-001");
      expect(result.ok).toBe(true);
    });
  });

  describe("pause methods", () => {
    it("isPaused returns ok", async () => {
      const result = await client.isPaused();
      expect(result.ok).toBe(true);
    });

    it("getPauseInfo returns ok", async () => {
      const result = await client.getPauseInfo();
      expect(result.ok).toBe(true);
    });
  });

  describe("role methods", () => {
    it("getAdmin returns ok", async () => {
      const result = await client.getAdmin();
      expect(result.ok).toBe(true);
    });

    it("getOperator returns ok", async () => {
      const result = await client.getOperator();
      expect(result.ok).toBe(true);
    });
  });

  describe("version methods", () => {
    it("getVersionInfo returns ok", async () => {
      const result = await client.getVersionInfo();
      expect(result.ok).toBe(true);
    });

    it("getMigrationState returns ok", async () => {
      const result = await client.getMigrationState();
      expect(result.ok).toBe(true);
    });

    it("getStorageVersion returns ok", async () => {
      const result = await client.getStorageVersion();
      expect(result.ok).toBe(true);
    });
  });

  describe("types", () => {
    it("CANONICAL_SEVERITIES contains expected entries", () => {
      expect(CANONICAL_SEVERITIES).toEqual([
        "critical",
        "high",
        "medium",
        "low",
      ]);
    });
  });

  describe("pre-flight simulation", () => {
    it("preflightInvoke returns success and a recommended fee", async () => {
      const result = await client.preflightInvoke("calculate_sla", [
        "outage-001",
        "high",
        90,
      ]);
      expect(result.success).toBe(true);
      expect(result.recommendedFee).toBeGreaterThan(0n);
    });
  });

  describe("contract event decoding", () => {
    it("decodeContractEvent unwraps ScVal event data into a typed object", () => {
      const event = decodeContractEvent<{ outage_id: string; amount: bigint }>({
        type: "sla_calculated",
        ledger: 123,
        data: toScVal({ outage_id: "outage-001", amount: 500n }),
      });
      expect(event.type).toBe("sla_calculated");
      expect(event.ledger).toBe(123);
      expect(event.data).toEqual({ outage_id: "outage-001", amount: 500n });
    });
  });

  describe("transaction confirmation polling", () => {
    class PendingThenSuccessClient extends SLACalculatorClient {
      callCount = 0;
      callTimestamps: number[] = [];

      protected async getTransactionStatus(
        txHash: string,
      ): Promise<TransactionStatusResult> {
        this.callTimestamps.push(Date.now());
        this.callCount += 1;
        if (this.callCount < 2) {
          return { status: "PENDING", txHash };
        }
        return { status: "SUCCESS", txHash };
      }
    }

    it("resolves once the transaction reaches a terminal status", async () => {
      const testClient = new PendingThenSuccessClient({
        contractId: "CABC1234567890ABCDEF",
        networkPassphrase: "Testnet ; SDF Network ; September 2015",
        rpcUrl: "https://soroban-testnet.stellar.org",
      });

      const result =
        await testClient.waitForTransactionConfirmation("txhash-1");

      expect(result).toEqual({ status: "SUCCESS", txHash: "txhash-1" });
      expect(testClient.callCount).toBe(2);
      // First retry should wait ~1s (the initial backoff interval).
      const gap =
        testClient.callTimestamps[1] - testClient.callTimestamps[0];
      expect(gap).toBeGreaterThanOrEqual(900);
    });

    it("throws TimeoutError if no terminal status is reached in time", async () => {
      class AlwaysPendingClient extends SLACalculatorClient {
        protected async getTransactionStatus(
          txHash: string,
        ): Promise<TransactionStatusResult> {
          return { status: "PENDING", txHash };
        }
      }

      const testClient = new AlwaysPendingClient({
        contractId: "CABC1234567890ABCDEF",
        networkPassphrase: "Testnet ; SDF Network ; September 2015",
        rpcUrl: "https://soroban-testnet.stellar.org",
      });

      await expect(
        testClient.waitForTransactionConfirmation("txhash-2", 500),
      ).rejects.toBeInstanceOf(TimeoutError);
    });
  });
});

describe("scval conversions", () => {
  it("round-trips primitives through toScVal/fromScVal", () => {
    expect(fromScVal(toScVal(42))).toBe(42);
    expect(fromScVal(toScVal(true))).toBe(true);
    expect(fromScVal(toScVal("hello"))).toBe("hello");
    expect(fromScVal(toScVal(500n))).toBe(500n);
    expect(fromScVal(toScVal(null))).toBeNull();
  });

  it("tags Stellar addresses distinctly from plain strings", () => {
    const address = "G" + "A".repeat(55);
    expect(toScVal(address).type).toBe("address");
    expect(toScVal("not-an-address").type).toBe("string");
  });

  it("round-trips arrays and objects", () => {
    expect(fromScVal(toScVal([1, 2, 3]))).toEqual([1, 2, 3]);
    expect(fromScVal(toScVal({ a: 1, b: "two" }))).toEqual({
      a: 1,
      b: "two",
    });
  });
});
