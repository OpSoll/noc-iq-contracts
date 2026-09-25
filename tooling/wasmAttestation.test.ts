import { describe, it, expect, afterEach } from "vitest";
import { mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import {
  computeChecksum,
  publishChecksum,
  verifyChecksum,
  generateAttestationPayload,
} from "./wasmAttestation";

describe("wasmAttestation", () => {
  const tmpDirs: string[] = [];

  function makeTmpDir(): string {
    const dir = mkdtempSync(join(tmpdir(), "wasm-attestation-test-"));
    tmpDirs.push(dir);
    return dir;
  }

  afterEach(() => {
    while (tmpDirs.length) {
      rmSync(tmpDirs.pop()!, { recursive: true, force: true });
    }
  });

  describe("computeChecksum", () => {
    it("produces a reproducible SHA-256 hash for identical content", () => {
      const dir = makeTmpDir();
      const filePath = join(dir, "artifact.wasm");
      writeFileSync(filePath, "deterministic build content");

      const first = computeChecksum(filePath);
      const second = computeChecksum(filePath);

      expect(first).toBe(second);
      expect(first).toMatch(/^[0-9a-f]{64}$/);
    });

    it("produces different hashes for different content", () => {
      const dir = makeTmpDir();
      const fileA = join(dir, "a.wasm");
      const fileB = join(dir, "b.wasm");
      writeFileSync(fileA, "build A");
      writeFileSync(fileB, "build B");

      expect(computeChecksum(fileA)).not.toBe(computeChecksum(fileB));
    });
  });

  describe("publishChecksum / verifyChecksum", () => {
    it("verifies successfully against a checksum it just published", () => {
      const dir = makeTmpDir();
      const filePath = join(dir, "artifact.wasm");
      const checksumPath = join(dir, "artifact.sha256");
      writeFileSync(filePath, "some wasm bytes");

      publishChecksum(filePath, checksumPath);

      expect(verifyChecksum(filePath, checksumPath)).toBe(true);
    });

    it("fails verification if the artifact changes after publishing", () => {
      const dir = makeTmpDir();
      const filePath = join(dir, "artifact.wasm");
      const checksumPath = join(dir, "artifact.sha256");
      writeFileSync(filePath, "original bytes");

      publishChecksum(filePath, checksumPath);
      writeFileSync(filePath, "tampered bytes");

      expect(verifyChecksum(filePath, checksumPath)).toBe(false);
    });

    it("returns false when the checksum file doesn't exist", () => {
      const dir = makeTmpDir();
      const filePath = join(dir, "artifact.wasm");
      writeFileSync(filePath, "some wasm bytes");

      expect(verifyChecksum(filePath, join(dir, "missing.sha256"))).toBe(
        false,
      );
    });
  });

  describe("generateAttestationPayload", () => {
    it("includes a reproducible checksum, injected commit SHA, and flags", () => {
      const dir = makeTmpDir();
      const filePath = join(dir, "artifact.wasm");
      writeFileSync(filePath, "deterministic build content");

      const payload = generateAttestationPayload(
        filePath,
        "cargo build --release",
        () => "abc123deadbeef",
      );

      expect(payload.wasmSha256).toBe(computeChecksum(filePath));
      expect(payload.gitCommitSha).toBe("abc123deadbeef");
      expect(payload.compilerFlags).toBe("cargo build --release");
      expect(typeof payload.generatedAt).toBe("string");
    });

    it("produces the same wasmSha256 across repeated calls (reproducible build hash)", () => {
      const dir = makeTmpDir();
      const filePath = join(dir, "artifact.wasm");
      writeFileSync(filePath, "deterministic build content");

      const a = generateAttestationPayload(filePath, "flags", () => "sha-a");
      const b = generateAttestationPayload(filePath, "flags", () => "sha-a");

      expect(a.wasmSha256).toBe(b.wasmSha256);
    });
  });
});
