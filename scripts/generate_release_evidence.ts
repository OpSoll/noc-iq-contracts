/**
 * SC-042: Automated release evidence package builder.
 *
 * Builds a zip archive containing:
 * - Compiled WASM binary (sla_calculator.wasm)
 * - SHA-256 checksum file (.sha256)
 * - Contract spec JSON (XDR spec)
 * - Test execution log (cargo test output)
 * - WASM size report
 *
 * Usage:
 *   npx tsx scripts/generate_release_evidence.ts [--tag v1.0.0]
 *
 * Output:
 *   release-evidence-<tag>-<sha>.zip
 */

import * as crypto from "crypto";
import * as fs from "fs";
import * as path from "path";
import { execSync } from "child_process";

const WASM_PATH = path.resolve(
  "sla_calculator/target/wasm32-unknown-unknown/release/sla_calculator.wasm"
);
const OUTPUT_DIR = path.resolve("release-evidence");
const SPEC_FILE = path.resolve("sla_calculator/src/spec_xdr.json");
const CHANGELOG = path.resolve("CHANGELOG.md");

function getGitSha(): string {
  try {
    return execSync("git rev-parse --short HEAD", { encoding: "utf8" }).trim();
  } catch {
    return "unknown";
  }
}

function getGitTag(): string {
  const tagArg = process.argv.find((a) => a === "--tag");
  if (tagArg) {
    const idx = process.argv.indexOf("--tag");
    if (idx + 1 < process.argv.length) return process.argv[idx + 1];
  }
  try {
    return execSync("git describe --tags --always", { encoding: "utf8" }).trim();
  } catch {
    return getGitSha();
  }
}

function sha256(filePath: string): string {
  const data = fs.readFileSync(filePath);
  return crypto.createHash("sha256").update(data).digest("hex");
}

function runCommand(cmd: string, label: string): string {
  console.log(`  Running: ${label}...`);
  try {
    return execSync(cmd, { encoding: "utf8", timeout: 300_000 });
  } catch (err: any) {
    console.warn(`  ⚠️  ${label} failed: ${err.message}`);
    return `[FAILED] ${err.message}`;
  }
}

function buildEvidencePackage(): void {
  const tag = getGitTag();
  const sha = getGitSha();
  const timestamp = new Date().toISOString();

  console.log(`\n📦 Building release evidence package`);
  console.log(`   Tag: ${tag}`);
  console.log(`   SHA: ${sha}`);
  console.log(`   Time: ${timestamp}\n`);

  // 1. Build WASM if not present
  if (!fs.existsSync(WASM_PATH)) {
    console.log("🔧 Building WASM binary...");
    const buildOutput = runCommand(
      "cargo build --target wasm32-unknown-unknown --release -p sla_calculator",
      "WASM build"
    );
    console.log(buildOutput);
  }

  if (!fs.existsSync(WASM_PATH)) {
    console.error("❌ WASM binary not found after build. Aborting.");
    process.exit(1);
  }

  // 2. Prepare output directory
  const evidenceDir = path.join(OUTPUT_DIR, `evidence-${tag}-${sha}`);
  if (fs.existsSync(evidenceDir)) {
    fs.rmSync(evidenceDir, { recursive: true });
  }
  fs.mkdirSync(evidenceDir, { recursive: true });

  // 3. Copy WASM binary
  const wasmDest = path.join(evidenceDir, "sla_calculator.wasm");
  fs.copyFileSync(WASM_PATH, wasmDest);
  const wasmSize = fs.statSync(wasmDest).size;
  console.log(`✅ WASM binary: ${(wasmSize / 1024).toFixed(2)} KB`);

  // 4. Generate SHA-256 checksum
  const hash = sha256(wasmDest);
  const checksumContent = `${hash}  sla_calculator.wasm\n`;
  const checksumPath = path.join(evidenceDir, "sla_calculator.wasm.sha256");
  fs.writeFileSync(checksumPath, checksumContent);
  console.log(`✅ SHA-256: ${hash}`);

  // 5. Copy spec file if available
  if (fs.existsSync(SPEC_FILE)) {
    fs.copyFileSync(SPEC_FILE, path.join(evidenceDir, "spec.json"));
    console.log("✅ Contract spec JSON included");
  } else {
    console.log("⚠️  No spec JSON found — skipping");
  }

  // 6. Run tests and capture output
  console.log("🧪 Running test suite...");
  const testOutput = runCommand(
    "cargo test -p sla_calculator 2>&1",
    "Full test suite"
  );
  fs.writeFileSync(path.join(evidenceDir, "test-output.log"), testOutput);
  console.log("✅ Test output captured");

  // 7. Generate WASM size report
  const sizeReport = runCommand(
    "cargo bloat --target wasm32-unknown-unknown --release -n 20 2>&1 || true",
    "WASM size profile"
  );
  fs.writeFileSync(path.join(evidenceDir, "wasm-size-report.txt"), sizeReport);
  console.log("✅ WASM size report generated");

  // 8. Copy CHANGELOG if available
  if (fs.existsSync(CHANGELOG)) {
    fs.copyFileSync(CHANGELOG, path.join(evidenceDir, "CHANGELOG.md"));
    console.log("✅ CHANGELOG included");
  }

  // 9. Generate metadata JSON
  const metadata = {
    tag,
    sha,
    timestamp,
    wasm_size_bytes: wasmSize,
    wasm_sha256: hash,
    rust_version: runCommand("rustc --version", "Rust version").trim(),
    cargo_version: runCommand("cargo --version", "Cargo version").trim(),
    build_target: "wasm32-unknown-unknown",
    build_profile: "release",
  };
  fs.writeFileSync(
    path.join(evidenceDir, "metadata.json"),
    JSON.stringify(metadata, null, 2)
  );
  console.log("✅ Metadata JSON generated");

  // 10. Create zip archive
  const zipName = `release-evidence-${tag}-${sha}.zip`;
  const zipPath = path.join(OUTPUT_DIR, zipName);
  try {
    execSync(
      `cd "${evidenceDir}" && zip -r "${zipPath}" .`,
      { encoding: "utf8" }
    );
    console.log(`\n📦 Release evidence package: ${zipPath}`);
    const zipSize = fs.statSync(zipPath).size;
    console.log(`   Size: ${(zipSize / 1024).toFixed(2)} KB`);
  } catch {
    // Fallback: if zip is not available, just leave the directory
    console.log(`\n📦 Release evidence directory: ${evidenceDir}`);
    console.log("   (zip command not available — directory preserved)");
  }

  console.log("\n✅ Release evidence package built successfully.\n");
}

buildEvidencePackage();
