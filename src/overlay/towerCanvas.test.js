import { describe, expect, it } from "vitest";
import { formatTowerPosition } from "./towerCanvas";

describe("formatTowerPosition", () => {
  it.each([
    [1, "1"],
    [27, "27"],
    [0, "–"],
    [-1, "–"],
  ])("formata a posição %s como %s", (position, expected) => {
    expect(formatTowerPosition(position)).toBe(expected);
  });
});
