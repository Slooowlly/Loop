import { describe, expect, it } from "vitest";

import { isPortuguese, localizedAiError, fallbackMode } from "./aiFallback";

describe("isPortuguese", () => {
  it("treats pt-BR and pt-PT (any pt variant) as Portuguese", () => {
    expect(isPortuguese("pt-BR")).toBe(true);
    expect(isPortuguese("pt-PT")).toBe(true);
    expect(isPortuguese("PT-br")).toBe(true);
  });

  it("treats other languages as non-Portuguese", () => {
    expect(isPortuguese("en-US")).toBe(false);
    expect(isPortuguese("es-ES")).toBe(false);
  });

  it("defaults to Portuguese when language is missing", () => {
    expect(isPortuguese(null)).toBe(true);
    expect(isPortuguese(undefined)).toBe(true);
  });
});

describe("localizedAiError", () => {
  it("returns the error phrase in the requested language", () => {
    expect(localizedAiError("pt-BR")).toMatch(/gera/i);
    expect(localizedAiError("en-US")).toMatch(/generation/i);
  });

  it("falls back to English for an unknown language", () => {
    expect(localizedAiError("de-DE")).toBe(localizedAiError("en-US"));
  });
});

describe("fallbackMode", () => {
  it("uses AI text whenever it exists, regardless of language", () => {
    expect(fallbackMode(true, "en-US")).toBe("ai");
    expect(fallbackMode(true, "pt-BR")).toBe("ai");
  });

  it("shows the deterministic template in Portuguese when there is no AI text", () => {
    expect(fallbackMode(false, "pt-BR")).toBe("template");
  });

  it("shows a localized error in other languages when there is no AI text", () => {
    expect(fallbackMode(false, "en-US")).toBe("error");
  });
});
