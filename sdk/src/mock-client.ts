/**
 * Mock SLA Calculator Client
 *
 * A drop-in stand-in for SLACalculatorClient for use in web app unit
 * tests, so tests don't need a live RPC endpoint. Every public method on
 * SLACalculatorClient is inherited unchanged (they all funnel through the
 * single `invoke()` seam), and this subclass intercepts that seam to
 * return configured mock responses and record call history instead.
 */

import { SLACalculatorClient, ClientConfig, ContractResult } from "./client";

export interface MockCallRecord {
  method: string;
  args: unknown[];
}

export interface MockResponse<T = unknown> {
  value?: T;
  error?: string;
}

const DEFAULT_MOCK_CONFIG: ClientConfig = {
  contractId: "MOCK_CONTRACT_ID",
  networkPassphrase: "Test SDF Network ; September 2015",
  rpcUrl: "mock://rpc",
};

/**
 * Mock client implementing the same public interface as
 * SLACalculatorClient (via inheritance), with configurable mock responses
 * and call-history tracking for test assertions.
 *
 * @example
 * ```ts
 * const client = new MockSlaCalculatorClient();
 * client.mockResponse("get_admin", { value: "GADMIN123" });
 * const result = await client.getAdmin();
 * expect(client.calls).toEqual([{ method: "get_admin", args: [] }]);
 * ```
 */
export class MockSlaCalculatorClient extends SLACalculatorClient {
  /** Ordered history of every contract method invoked on this mock. */
  readonly calls: MockCallRecord[] = [];

  private readonly responses = new Map<string, MockResponse>();

  constructor(config: ClientConfig = DEFAULT_MOCK_CONFIG) {
    super(config);
  }

  /**
   * Configures the mock return value (or error) for a given contract
   * method name (e.g. "get_admin", "calculate_sla").
   */
  mockResponse<T>(method: string, response: MockResponse<T>): void {
    this.responses.set(method, response as MockResponse);
  }

  /** Clears all configured mock responses and recorded call history. */
  reset(): void {
    this.responses.clear();
    this.calls.length = 0;
  }

  protected async invoke<T>(
    method: string,
    args: unknown[],
  ): Promise<ContractResult<T>> {
    this.calls.push({ method, args });

    const configured = this.responses.get(method);
    if (configured) {
      if (configured.error !== undefined) {
        return { ok: false, error: configured.error };
      }
      return { ok: true, value: configured.value as T };
    }

    return super.invoke<T>(method, args);
  }
}
