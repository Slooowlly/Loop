import { describe, expect, it } from "vitest";

import { getCategoryColor } from "./categoryColors";

describe("categoryColors", () => {
  it("returns the shared championship palette for rookie and amateur categories", () => {
    expect(getCategoryColor("mazda_rookie")).toBe("#FFD400");
    expect(getCategoryColor("toyota_rookie")).toBe("#FFD400");
    expect(getCategoryColor("mazda_amador")).toBe("#E73F47");
    expect(getCategoryColor("toyota_amador")).toBe("#E73F47");
  });

  it("returns the shared championship palette for production and upper categories", () => {
    expect(getCategoryColor("bmw_m2")).toBe("#E00010");
    expect(getCategoryColor("production_challenger")).toBe("#8020D0");
    expect(getCategoryColor("gt4")).toBe("#2070F0");
    expect(getCategoryColor("gt3")).toBe("#00F0F0");
    expect(getCategoryColor("endurance")).toBe("#3fb950");
  });
});
