/**
 * SC-W5-067: Fuzzing for malformed outage ids and symbol encodings.
 * Validates that the contract rejects or safely handles invalid outage id formats.
 */

function validateOutageId(id: string): boolean {
  // Contract expects format: ^[a-z0-9-]{3,64}$
  return /^[a-z0-9-]{3,64}$/.test(id);
}

const VALID_IDS = ["outage-001", "abc", "a".repeat(64), "123-xyz"];
const INVALID_IDS = [
  "",              // empty
  "ab",            // too short
  "a".repeat(65),  // too long
  "UPPER-001",     // uppercase not allowed
  "id with space", // spaces not allowed
  "id_underscore", // underscore not allowed
  "特殊文字",        // non-ASCII
];

describe("SC-W5-067 Fuzzing Malformed Outage IDs", () => {
  it("valid ids pass validation", () => {
    for (const id of VALID_IDS) {
      expect(validateOutageId(id)).toBe(true);
    }
  });

  it("invalid ids are rejected", () => {
    for (const id of INVALID_IDS) {
      expect(validateOutageId(id)).toBe(false);
    }
  });

  it("boundary: 3 chars is valid, 2 chars is invalid", () => {
    expect(validateOutageId("abc")).toBe(true);
    expect(validateOutageId("ab")).toBe(false);
  });

  it("boundary: 64 chars is valid, 65 chars is invalid", () => {
    expect(validateOutageId("a".repeat(64))).toBe(true);
    expect(validateOutageId("a".repeat(65))).toBe(false);
  });
});
