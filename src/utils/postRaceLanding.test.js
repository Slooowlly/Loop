import { beforeEach, describe, expect, it } from "vitest";

import {
  HOME_TAB,
  NEWS_TAB,
  isFinaleSlot,
  recordNewsRead,
  recordNewsSkip,
  resolvePostRaceLanding,
} from "./postRaceLanding";

const CAREER = "career-1";

beforeEach(() => {
  localStorage.clear();
});

describe("isFinaleSlot", () => {
  it("reconhece os slots de final de campeonato", () => {
    expect(isFinaleSlot("FinalDaTemporada")).toBe(true);
    expect(isFinaleSlot("FinalEspecial")).toBe(true);
  });

  it("ignora slots regulares e valores vazios", () => {
    expect(isFinaleSlot("RodadaRegular")).toBe(false);
    expect(isFinaleSlot("AberturaDaTemporada")).toBe(false);
    expect(isFinaleSlot(undefined)).toBe(false);
    expect(isFinaleSlot(null)).toBe(false);
  });
});

describe("resolvePostRaceLanding", () => {
  it("cai em Notícias e pede avaliação por padrão (carreira nova)", () => {
    expect(resolvePostRaceLanding(CAREER, 1, false)).toEqual({
      tab: NEWS_TAB,
      evaluate: true,
    });
  });

  it("migra para Home após 3 pulos seguidos e ali permanece na temporada", () => {
    resolvePostRaceLanding(CAREER, 1, false);
    recordNewsSkip(CAREER, 1);
    recordNewsSkip(CAREER, 1);
    // Ainda em Notícias no 2º pulo.
    expect(resolvePostRaceLanding(CAREER, 1, false).tab).toBe(NEWS_TAB);
    recordNewsSkip(CAREER, 1);

    expect(resolvePostRaceLanding(CAREER, 1, false)).toEqual({
      tab: HOME_TAB,
      evaluate: false,
    });
  });

  it("uma leitura zera a sequência de pulos", () => {
    resolvePostRaceLanding(CAREER, 1, false);
    recordNewsSkip(CAREER, 1);
    recordNewsSkip(CAREER, 1);
    recordNewsRead(CAREER, 1);
    recordNewsSkip(CAREER, 1);
    recordNewsSkip(CAREER, 1);
    // 2 pulos após a leitura ainda não bastam (limite volta a 3).
    expect(resolvePostRaceLanding(CAREER, 1, false).tab).toBe(NEWS_TAB);
  });

  it("no final de campeonato força Home sem avaliar nem mudar o modo", () => {
    // Modo Notícias firmado (carreira nova).
    expect(resolvePostRaceLanding(CAREER, 1, false).tab).toBe(NEWS_TAB);

    // Final de campeonato: Home forçada (é onde o pop-up de campeão abre), sem
    // avaliação — o override não conta como leitura nem como pulo.
    expect(resolvePostRaceLanding(CAREER, 1, true)).toEqual({
      tab: HOME_TAB,
      evaluate: false,
    });

    // Não mexeu no aprendizado: a corrida regular seguinte volta a cair em Notícias.
    expect(resolvePostRaceLanding(CAREER, 1, false).tab).toBe(NEWS_TAB);
  });

  it("na virada de temporada, vindo de Home, tenta Notícias com tolerância 2", () => {
    // Temporada 1 termina em Home.
    resolvePostRaceLanding(CAREER, 1, false);
    recordNewsSkip(CAREER, 1);
    recordNewsSkip(CAREER, 1);
    recordNewsSkip(CAREER, 1);

    // Temporada 2: volta pra Notícias.
    expect(resolvePostRaceLanding(CAREER, 2, false).tab).toBe(NEWS_TAB);
    recordNewsSkip(CAREER, 2);
    // 2 pulos seguidos já bastam para voltar pra Home nesta tentativa.
    recordNewsSkip(CAREER, 2);
    expect(resolvePostRaceLanding(CAREER, 2, false).tab).toBe(HOME_TAB);
  });

  it("na virada de temporada, vindo de Notícias, mantém tolerância 3", () => {
    resolvePostRaceLanding(CAREER, 1, false); // segue em Notícias a temporada toda

    expect(resolvePostRaceLanding(CAREER, 2, false).tab).toBe(NEWS_TAB);
    recordNewsSkip(CAREER, 2);
    recordNewsSkip(CAREER, 2);
    // 2 pulos não bastam quando o modo Notícias não vinha de recuperação.
    expect(resolvePostRaceLanding(CAREER, 2, false).tab).toBe(NEWS_TAB);
  });
});
