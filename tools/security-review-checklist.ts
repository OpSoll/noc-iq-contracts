interface ChecklistItem { id: string; description: string; automated: boolean; passed?: boolean; }

export const securityChecklist: ChecklistItem[] = [
  { id: "AUTH-01",   description: "All privileged paths require explicit role check",          automated: true  },
  { id: "AUTH-02",   description: "No anonymous access to state-mutating methods",             automated: true  },
  { id: "REENT-01",  description: "Re-entrancy guards on all callbacks",                       automated: false },
  { id: "INPUT-01",  description: "Symbol and ID length constraints enforced",                 automated: true  },
  { id: "INPUT-02",  description: "Numeric bounds checked before arithmetic",                  automated: true  },
  { id: "EVENT-01",  description: "All state changes emit a corresponding event",              automated: false },
  { id: "GOVERN-01", description: "Governance actions require quorum before execution",        automated: true  },
];

export function runChecklist(items: ChecklistItem[]): void {
  console.log("Security Review Checklist\n" + "=".repeat(40));
  for (const item of items) {
    const status = item.passed === true ? "PASS" : item.passed === false ? "FAIL" : "PENDING";
    const auto   = item.automated ? "[auto]  " : "[manual]";
    console.log(`  [${status.padEnd(7)}] ${auto} ${item.id}: ${item.description}`);
  }
}

runChecklist(securityChecklist);
