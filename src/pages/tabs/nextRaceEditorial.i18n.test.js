import { describe, it, expect, afterEach } from "vitest";
import i18n, { DEFAULT_LANGUAGE } from "../../i18n/index.js";
import { buildEditorialCopy } from "./nextRaceEditorial.js";

function withLang(lang, fn) {
  i18n.changeLanguage(lang);
  try {
    return fn();
  } finally {
    i18n.changeLanguage(DEFAULT_LANGUAGE);
  }
}

afterEach(() => i18n.changeLanguage(DEFAULT_LANGUAGE));

const base = {
  playerStanding: { id: "p1", posicao_campeonato: 2, pontos: 88, results: [{ position: 3, is_dnf: false }] },
  leader: { nome: "M. Costa", pontos: 94 },
  briefingRival: { driver_name: "M. Costa" },
  playerTeam: { nome: "Equipe Aurora" },
  nextRace: { track_name: "Interlagos", rodada: 5 },
  trackHistory: { has_data: true, best_finish: 1, dnfs: 0 },
  gapToLeader: 6,
  remainingRounds: 5,
};

describe("nextRaceEditorial i18n (Fase 2 — fallback determinístico)", () => {
  it("title_chase troca PT↔EN com interpolação e forma", () => {
    const pt = buildEditorialCopy({ thesis: { key: "title_chase" }, ...base });
    expect(pt.paragraphs[0]).toMatch(/6 ponto/);
    expect(pt.paragraphs[0]).toContain("M. Costa");

    withLang("en-US", () => {
      const en = buildEditorialCopy({ thesis: { key: "title_chase" }, ...base });
      expect(en.paragraphs[0]).toContain("6 point(s)");
      expect(en.paragraphs[0]).toContain("M. Costa");
      // formSentence (ordinal en) entra no 2º parágrafo (outlook).
      expect(en.paragraphs[1]).toMatch(/3rd place/);
    });
  });

  it("mantém as 3 leituras: '3º lugar' (PT) → '3rd place' (EN)", () => {
    const pt = buildEditorialCopy({ thesis: { key: "baseline" }, ...base });
    expect(pt.paragraphs[1]).toMatch(/3º lugar/);
    withLang("en-US", () => {
      const en = buildEditorialCopy({ thesis: { key: "baseline" }, ...base });
      expect(en.paragraphs[1]).toMatch(/3rd place/);
    });
  });

  it("fallback de contexto por locale (sem equipe/pista)", () => {
    withLang("en-US", () => {
      const en = buildEditorialCopy({ thesis: { key: "baseline" }, playerStanding: null });
      expect(en.headline).toContain("this round");
      expect(en.paragraphs[1]).toContain("The team wants to turn practice pace");
    });
  });
});
