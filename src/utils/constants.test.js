import { describe, expect, it } from "vitest";

import { LOADING_MESSAGE_INTERVAL_MS, LOADING_MESSAGES } from "./constants";

describe("LOADING_MESSAGES", () => {
  it("covers one minute of historical generation without repeating", () => {
    const messagesShownInOneMinute = Math.ceil(60_000 / LOADING_MESSAGE_INTERVAL_MS);

    expect(LOADING_MESSAGE_INTERVAL_MS).toBe(2000);
    expect(LOADING_MESSAGES.length).toBeGreaterThanOrEqual(messagesShownInOneMinute);
    expect(new Set(LOADING_MESSAGES).size).toBe(LOADING_MESSAGES.length);
  });

  it("keeps early messages compatible with saves that finish around one minute", () => {
    const messagesShownInOneMinute = Math.ceil(60_000 / LOADING_MESSAGE_INTERVAL_MS);
    const firstMinuteMessages = LOADING_MESSAGES.slice(0, messagesShownInOneMinute);

    expect(firstMinuteMessages.join(" ")).not.toMatch(/2025|fase final|ultimos anos/i);
  });

  it("follows broad historical draft creation phases", () => {
    const findIndex = (pattern) => LOADING_MESSAGES.findIndex((message) => pattern.test(message));

    const baseWorld = findIndex(/base.*2000/i);
    const firstSeason = findIndex(/primeiras temporadas|inicio do arquivo/i);
    const market = findIndex(/movimentando contratos|janela de evolucao/i);
    const transition = findIndex(/promocoes|rebaixamentos/i);
    const archive = findIndex(/arquivos historicos|memoria/i);
    const playableYear = findIndex(/2025/i);

    expect(baseWorld).toBeGreaterThanOrEqual(0);
    expect(firstSeason).toBeGreaterThan(baseWorld);
    expect(market).toBeGreaterThan(firstSeason);
    expect(transition).toBeGreaterThan(market);
    expect(archive).toBeGreaterThan(transition);
    expect(playableYear).toBeGreaterThan(archive);
  });
});
