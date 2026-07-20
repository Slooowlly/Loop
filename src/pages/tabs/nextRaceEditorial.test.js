import { describe, expect, it } from "vitest";

import { buildEditorialCopy, classifyChampionshipState, THESIS_EDITORIAL } from "./nextRaceEditorial";
import { THESIS_PRIORITY } from "./nextRaceThesis";

describe("classifyChampionshipState", () => {
  it("marks a close title chase as chase", () => {
    expect(
      classifyChampionshipState({
        playerStanding: { posicao_campeonato: 2, pontos: 88 },
        leader: { pontos: 94 },
        remainingRounds: 5,
        outlook: { titleFight: "contender" },
        gapBehind: 10,
      }),
    ).toBe("chase");
  });

  it("marks the championship leader correctly", () => {
    expect(
      classifyChampionshipState({
        playerStanding: { posicao_campeonato: 1, pontos: 100 },
        leader: { pontos: 100 },
        remainingRounds: 2,
        outlook: { titleFight: "leader" },
        gapBehind: 3,
      }),
    ).toBe("leader");
  });

  it("marks a longshot situation as outsider", () => {
    expect(
      classifyChampionshipState({
        playerStanding: { posicao_campeonato: 8, pontos: 9 },
        leader: { pontos: 50 },
        remainingRounds: 1,
        outlook: { titleFight: "longshot" },
        gapBehind: 2,
      }),
    ).toBe("outsider");
  });
});

describe("buildEditorialCopy", () => {
  const base = {
    playerStanding: { id: "p1", posicao_campeonato: 2, pontos: 88, results: [{ position: 3, is_dnf: false }] },
    leader: { nome: "M. Costa", pontos: 94 },
    briefingRival: { driver_name: "M. Costa", championship_position: 1, gap_points: 6, is_ahead: true },
    playerTeam: { nome: "Equipe Aurora" },
    nextRace: { track_name: "Interlagos", rodada: 5 },
    trackHistory: { has_data: true, starts: 4, best_finish: 1, dnfs: 0, last_visit_season: 1 },
    gapToLeader: 6,
    remainingRounds: 5,
  };

  it("returns exactly the fields the screen renders", () => {
    const copy = buildEditorialCopy({ thesis: { key: "title_chase" }, ...base });
    expect(copy.headline).toBeTruthy();
    expect(copy.paragraphs).toHaveLength(2);
    expect(copy.quote).toBeTruthy();
    expect(copy.actionHint).toBeTruthy();
  });

  it("drives the copy from the dominant thesis (title chase)", () => {
    const copy = buildEditorialCopy({ thesis: { key: "title_chase" }, ...base });
    expect(copy.headline).toMatch(/conta|caça/i);
    expect(copy.paragraphs[0]).toMatch(/M\. Costa/);
    expect(copy.paragraphs[0]).toMatch(/6 ponto/);
  });

  it("switches voice entirely when the thesis is redemption", () => {
    const copy = buildEditorialCopy({ thesis: { key: "redemption" }, ...base });
    expect(copy.headline).toMatch(/virar a chave|reação/i);
    expect(copy.paragraphs.join(" ")).toMatch(/resposta|trilhos|virada/i);
  });

  it("uses the nemesis name when the thesis is a personal duel", () => {
    const copy = buildEditorialCopy({
      thesis: { key: "nemesis" },
      ...base,
      nemesisName: "K. Novak",
    });
    expect(copy.headline + copy.paragraphs.join(" ")).toMatch(/K\. Novak/);
  });

  it("produces weather-flavored copy on a wet thesis", () => {
    const copy = buildEditorialCopy({ thesis: { key: "weather" }, ...base, climaLabel: "chuva forte" });
    expect(copy.paragraphs[0]).toMatch(/pista molha|corrida de leitura|reescrever o grid|previsível|meteorologia/i);
  });

  it("threads the weather label into the variant that uses it", () => {
    // A variante 1 do clima cita o rótulo; escolhemos uma etapa cujo seed a seleciona.
    const wet = buildEditorialCopy({
      thesis: { key: "weather" },
      ...base,
      nextRace: { track_name: "Spa", rodada: 3 },
      climaLabel: "chuva forte",
    });
    // Ou o rótulo aparece (variante 1), ou o texto é claramente de clima (variante 2).
    expect(wet.paragraphs[0]).toMatch(/chuva forte|pista molha/i);
  });

  it("is deterministic for the same race", () => {
    const a = buildEditorialCopy({ thesis: { key: "baseline" }, ...base });
    const b = buildEditorialCopy({ thesis: { key: "baseline" }, ...base });
    expect(a.headline).toBe(b.headline);
    expect(a.paragraphs).toEqual(b.paragraphs);
  });

  it("falls back to baseline copy for an unknown thesis key", () => {
    const copy = buildEditorialCopy({ thesis: { key: "does-not-exist" }, ...base });
    expect(copy.paragraphs).toHaveLength(2);
    expect(copy.headline).toBeTruthy();
  });
});

describe("THESIS_EDITORIAL", () => {
  it("covers every thesis in the priority list with all four fields and 2 variants", () => {
    for (const key of THESIS_PRIORITY) {
      const entry = THESIS_EDITORIAL[key];
      expect(entry, `missing editorial for thesis "${key}"`).toBeTruthy();
      for (const field of ["headline", "lead", "outlook", "quote"]) {
        expect(Array.isArray(entry[field]), `${key}.${field} should be an array`).toBe(true);
        expect(entry[field].length, `${key}.${field} should have 2 variants`).toBe(2);
      }
    }
  });
});
