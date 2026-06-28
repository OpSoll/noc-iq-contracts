export interface VersionEntry {
  contractVersion: string;
  backendVersion: string;
  compatible: boolean;
  notes?: string;
}

export const versionCompatMatrix: VersionEntry[] = [
  { contractVersion: "0.5.0", backendVersion: "0.5.0", compatible: true },
  { contractVersion: "0.5.0", backendVersion: "0.4.x", compatible: false, notes: "Breaking: history format changed" },
  { contractVersion: "0.4.x", backendVersion: "0.5.0", compatible: false, notes: "Backend expects new event fields" },
  { contractVersion: "0.4.x", backendVersion: "0.4.x", compatible: true },
];

export function checkCompatibility(contractVer: string, backendVer: string): VersionEntry | undefined {
  return versionCompatMatrix.find(
    (e) => e.contractVersion === contractVer && e.backendVersion === backendVer
  );
}

export function printMatrix(): void {
  console.log("Version Compatibility Matrix\n" + "=".repeat(52));
  for (const e of versionCompatMatrix) {
    const status = e.compatible ? "COMPATIBLE  " : "INCOMPATIBLE";
    const note   = e.notes ? ` (${e.notes})` : "";
    console.log(`  contract@${e.contractVersion} + backend@${e.backendVersion} → ${status}${note}`);
  }
}
