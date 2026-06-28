export type FormatVersion = "v1" | "v2";

export interface HistoryRecord { outageId: string; data: unknown; formatVersion: FormatVersion; }

export interface FormatSpec { version: FormatVersion; fields: string[]; deprecated: boolean; }

export const formatSpecs: FormatSpec[] = [
  { version: "v1", fields: ["outageId", "duration", "severity"],                                     deprecated: true  },
  { version: "v2", fields: ["outageId", "duration", "severity", "region", "correlationId"],           deprecated: false },
];

export function tagRecord(record: Omit<HistoryRecord, "formatVersion">, version: FormatVersion): HistoryRecord {
  return { ...record, formatVersion: version };
}

export function canRead(readerVersion: FormatVersion, recordVersion: FormatVersion): boolean {
  const ri = formatSpecs.findIndex((s) => s.version === readerVersion);
  const rr = formatSpecs.findIndex((s) => s.version === recordVersion);
  return ri >= rr;
}

export function latestFormat(): FormatVersion {
  return formatSpecs.filter((s) => !s.deprecated).at(-1)!.version;
}
