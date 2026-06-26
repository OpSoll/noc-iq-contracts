/**
 * SC-W5-110: End-to-end SLA->payment intent integration conformance suite.
 *
 * Validates that SLA calculation results deterministically map to the correct
 * payment intent shape expected by the backend. Covers met/violated paths,
 * reward tiers, penalty amounts, and negative/adversarial cases.
 */

export interface SlaResult {
  status: "met" | "viol";
  payment_type: "rew" | "pen";
  rating: "top" | "excel" | "good" | "poor";
  amount: number;
  config_version_hash: string;
}

export interface PaymentIntent {
  outage_id: string;
  payment_type: "rew" | "pen";
  amount: number;
  rating: string;
  sla_met: boolean;
}

/** Map an SLA result to the payment intent the backend must enqueue. */
export function buildPaymentIntent(
  outage_id: string,
  result: SlaResult
): PaymentIntent {
  return {
    outage_id,
    payment_type: result.payment_type,
    amount: result.amount,
    rating: result.rating,
    sla_met: result.status === "met",
  };
}

interface ConformanceCase {
  label: string;
  outage_id: string;
  result: SlaResult;
  expected: Partial<PaymentIntent>;
}

const cases: ConformanceCase[] = [
  {
    label: "critical met -> reward top",
    outage_id: "OUT001",
    result: { status: "met", payment_type: "rew", rating: "top", amount: 500, config_version_hash: "abc" },
    expected: { payment_type: "rew", sla_met: true, rating: "top", amount: 500 },
  },
  {
    label: "critical violated -> penalty",
    outage_id: "OUT002",
    result: { status: "viol", payment_type: "pen", rating: "poor", amount: 1000, config_version_hash: "abc" },
    expected: { payment_type: "pen", sla_met: false, rating: "poor", amount: 1000 },
  },
  {
    label: "high met -> reward good",
    outage_id: "OUT003",
    result: { status: "met", payment_type: "rew", rating: "good", amount: 200, config_version_hash: "abc" },
    expected: { payment_type: "rew", sla_met: true, rating: "good" },
  },
  {
    label: "zero-amount reward is valid",
    outage_id: "OUT004",
    result: { status: "met", payment_type: "rew", rating: "good", amount: 0, config_version_hash: "abc" },
    expected: { payment_type: "rew", sla_met: true, amount: 0 },
  },
];

// Adversarial: negative amount must be rejected
function assertNoNegativeAmount(result: SlaResult): void {
  if (result.amount < 0) throw new Error(`[SC-W5-110] Negative payment amount: ${result.amount}`);
}

// Adversarial: payment_type must match status
function assertTypeMatchesStatus(result: SlaResult): void {
  if (result.status === "met" && result.payment_type !== "rew")
    throw new Error(`[SC-W5-110] status=met but payment_type=${result.payment_type}`);
  if (result.status === "viol" && result.payment_type !== "pen")
    throw new Error(`[SC-W5-110] status=viol but payment_type=${result.payment_type}`);
}

function runConformance(): void {
  console.log("[SC-W5-110] SLA->payment intent conformance suite\n");
  let passed = 0;
  for (const c of cases) {
    assertNoNegativeAmount(c.result);
    assertTypeMatchesStatus(c.result);
    const intent = buildPaymentIntent(c.outage_id, c.result);
    for (const [k, v] of Object.entries(c.expected)) {
      if ((intent as any)[k] !== v)
        throw new Error(`[SC-W5-110] "${c.label}": ${k} expected ${v}, got ${(intent as any)[k]}`);
    }
    console.log(`  ✓ ${c.label}`);
    passed++;
  }

  // Adversarial: mismatch between status and type must be caught
  try {
    assertTypeMatchesStatus({ status: "met", payment_type: "pen", rating: "good", amount: 0, config_version_hash: "" });
    throw new Error("Should have thrown");
  } catch (e: any) {
    if (!e.message.includes("SC-W5-110")) throw e;
    console.log("  ✓ adversarial: met+pen rejected");
  }

  console.log(`\n${passed} conformance cases + 1 adversarial check passed.`);
}

runConformance();
