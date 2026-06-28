export interface GovernanceEvent {
  type: string;
  actor: string;
  timestamp: number;
  payload: Record<string, unknown>;
}

export interface ChangelogEntry {
  date: string;
  action: string;
  actor: string;
  summary: string;
}

export function eventToEntry(e: GovernanceEvent): ChangelogEntry {
  return {
    date: new Date(e.timestamp).toISOString().slice(0, 10),
    action: e.type,
    actor: e.actor,
    summary: `${e.type} by ${e.actor}: ${JSON.stringify(e.payload)}`,
  };
}

export function generateChangelog(events: GovernanceEvent[]): string {
  const sorted  = [...events].sort((a, b) => b.timestamp - a.timestamp);
  const entries = sorted.map(eventToEntry);
  const lines   = ["# Governance Changelog", ""];
  for (const e of entries) {
    lines.push(`## ${e.date} — ${e.action}`);
    lines.push(`**Actor:** ${e.actor}`);
    lines.push(`> ${e.summary}`);
    lines.push("");
  }
  return lines.join("\n");
}
