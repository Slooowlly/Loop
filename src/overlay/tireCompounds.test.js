import { describe, it, expect } from "vitest";
import { tireCompoundKind, tireCompoundDryWet } from "./tireCompounds";

describe("tireCompoundKind", () => {
  it("returns null for unknown/negative indices", () => {
    expect(tireCompoundKind("gt3", -1)).toBeNull();
    expect(tireCompoundKind("gt3", undefined)).toBeNull();
    expect(tireCompoundKind("gt3", null)).toBeNull();
  });

  it("defaults to dry at index 0 and wet above it (1-slick fallback)", () => {
    expect(tireCompoundKind("gt3", 0)).toBe("dry");
    expect(tireCompoundKind("gt3", 1)).toBe("wet");
    expect(tireCompoundKind("gt3", 2)).toBe("wet");
  });

  it("uses the fallback for unknown categories", () => {
    expect(tireCompoundKind("nao_existe", 0)).toBe("dry");
    expect(tireCompoundKind(undefined, 1)).toBe("wet");
  });
});

describe("tireCompoundDryWet", () => {
  it("flattens any slick to dry and keeps wet as wet", () => {
    expect(tireCompoundDryWet("gt3", 0)).toBe("dry");
    expect(tireCompoundDryWet("gt3", 1)).toBe("wet");
    expect(tireCompoundDryWet("gt3", -1)).toBeNull();
  });
});
