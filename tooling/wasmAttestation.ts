/**
 * SC-W5-074: WASM artifact attestation and checksum publication.
 * Computes and verifies the SHA-256 checksum of the WASM build artifact.
 */

import { createHash } from "crypto";
import { execSync } from "child_process";
import { existsSync, readFileSync, writeFileSync } from "fs";

const WASM_PATH = "sla_calculator/target/wasm32-unknown-unknown/release/sla_calculator.wasm";
const CHECKSUM_FILE = "artifacts/sla_calculator.wasm.sha256";
const ATTESTATION_FILE = "artifacts/sla_calculator.attestation.json";

export function computeChecksum(filePath: string): string {
  const bytes = readFileSync(filePath);
  return createHash("sha256").update(bytes).digest("hex");
}

export function publishChecksum(filePath: string, outPath: string): void {
  const checksum = computeChecksum(filePath);
  writeFileSync(outPath, `${checksum}  ${filePath}\n`, "utf8");
  console.log(`Checksum written: ${checksum}`);
}

export function verifyChecksum(filePath: string, checksumFile: string): boolean {
  if (!existsSync(checksumFile)) return false;
  const expected = readFileSync(checksumFile, "utf8").split(/\s+/)[0];
  return computeChecksum(filePath) === expected;
}

export interface AttestationPayload {
  wasmSha256: string;
  gitCommitSha: string;
  compilerFlags: string;
  generatedAt: string;
}

/**
 * Builds a JSON attestation payload tying a compiled WASM's checksum to
 * the exact git commit and compiler flags it was built from, so a
 * deployed contract can be verified against the open-source repository.
 * `getGitCommitSha` is injectable so tests don't depend on a real git repo.
 */
export function generateAttestationPayload(
  filePath: string,
  compilerFlags: string,
  getGitCommitSha: () => string = defaultGetGitCommitSha,
): AttestationPayload {
  return {
    wasmSha256: computeChecksum(filePath),
    gitCommitSha: getGitCommitSha(),
    compilerFlags,
    generatedAt: new Date().toISOString(),
  };
}

function defaultGetGitCommitSha(): string {
  return execSync("git rev-parse HEAD").toString().trim();
}

if (require.main === module) {
  if (!existsSync(WASM_PATH)) { console.error(`WASM not found: ${WASM_PATH}`); process.exit(1); }
  publishChecksum(WASM_PATH, CHECKSUM_FILE);
  const ok = verifyChecksum(WASM_PATH, CHECKSUM_FILE);
  console.log(ok ? "Attestation verified." : "Attestation FAILED.");

  const payload = generateAttestationPayload(
    WASM_PATH,
    "cargo build --target wasm32-unknown-unknown --release",
  );
  writeFileSync(ATTESTATION_FILE, JSON.stringify(payload, null, 2) + "\n", "utf8");
  console.log(`Attestation payload written: ${ATTESTATION_FILE}`);

  if (!ok) process.exit(1);
}
