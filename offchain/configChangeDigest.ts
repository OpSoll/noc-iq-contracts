export interface ConfigChange {
  key: string;
  oldValue: unknown;
  newValue: unknown;
  changedAt: number;
  changedBy: string;
}

export interface DigestedChange {
  change: ConfigChange;
  digest: string;
}

function simpleHash(input: string): string {
  let h = 0;
  for (let i = 0; i < input.length; i++) h = ((h << 5) - h + input.charCodeAt(i)) | 0;
  return (h >>> 0).toString(16).padStart(8, "0");
}

export function digestChange(c: ConfigChange): string {
  return simpleHash(JSON.stringify([c.key, c.oldValue, c.newValue, c.changedAt, c.changedBy]));
}

export function wrapWithDigest(change: ConfigChange): DigestedChange {
  return { change, digest: digestChange(change) };
}

export function verifyDigest(d: DigestedChange): boolean {
  return digestChange(d.change) === d.digest;
}
