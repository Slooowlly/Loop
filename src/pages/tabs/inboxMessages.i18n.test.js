import { describe, it, expect, afterEach } from "vitest";
import i18n, { DEFAULT_LANGUAGE } from "../../i18n/index.js";
import { buildInboxMessages } from "./inboxMessages.js";

function withLang(lang, fn) {
  i18n.changeLanguage(lang);
  try {
    return fn();
  } finally {
    i18n.changeLanguage(DEFAULT_LANGUAGE);
  }
}

afterEach(() => i18n.changeLanguage(DEFAULT_LANGUAGE));

// O corpo é lista de parágrafos de trechos tipados. Para conferir a prosa basta o
// texto colado; para conferir a ênfase, a lista dos trechos em negrito.
const corpo = (msg) =>
  msg.paragrafos.map((p) => p.map((t) => t.texto).join("")).join("");
const fortes = (msg) =>
  msg.paragrafos.flat().filter((t) => t.tipo === "forte").map((t) => t.texto);

const H2H = {
  head_to_head: {
    races_together: 3,
    player_ahead: 1,
    best_finish: 2,
    best_track: "Monza",
    rival_name: "Ruiz",
    rival_team: "RT",
  },
};

const FAV = {
  title_favorite: {
    veteran: true,
    career_titles: 2,
    position: 1,
    points_lead: 5,
    leads_player: false,
    strong_attr: "racecraft",
    weak_attr: "defesa",
    driver_name: "Silva",
    driver_team: "ST",
  },
};

const INTEREST = (teams, fama) => ({
  team_interest: { teams: teams.map((team_name) => ({ team_name })), player_fama: fama },
});

describe("inboxMessages i18n (Fase 2 — gerador de prosa)", () => {
  it("head-to-head: PT baseline com plural, ordinal e ênfase", () => {
    const [msg] = buildInboxMessages(H2H);
    expect(msg.paragrafos).toHaveLength(1);
    expect(corpo(msg)).toContain("3 vezes");
    expect(corpo(msg)).toContain("seu 2º em Monza");
    expect(corpo(msg)).toContain("Ruiz (RT)");
    expect(fortes(msg)).toContain("Ruiz");
    expect(fortes(msg)).toContain("1");
    expect(msg.subject).toBe("Você e Ruiz voltam a se cruzar.");
  });

  it("head-to-head: troca pra EN (plural + ordinal en)", () => {
    withLang("en-US", () => {
      const [msg] = buildInboxMessages(H2H);
      expect(corpo(msg)).toContain("3 times");
      expect(corpo(msg)).toContain("your 2nd at Monza");
      expect(msg.subject).toBe("You and Ruiz cross paths again.");
      expect(msg.from).toBe("Grid bulletin");
    });
  });

  it("head-to-head: singular (1 vez / 1 time)", () => {
    const facts = { head_to_head: { ...H2H.head_to_head, races_together: 1, player_ahead: 0 } };
    expect(corpo(buildInboxMessages(facts)[0])).toContain("1 vez");
    withLang("en-US", () => {
      expect(corpo(buildInboxMessages(facts)[0])).toContain("1 time");
    });
  });

  it("favorito: perfil/standing/traits com plural e atributos", () => {
    const [msg] = buildInboxMessages(FAV);
    expect(msg.paragrafos).toHaveLength(1);
    expect(corpo(msg)).toContain("veterano com 2 títulos");
    expect(corpo(msg)).toContain("lidera o campeonato por 5 pontos");
    expect(corpo(msg)).toContain("Impecável no corpo a corpo, mas vulnerável quando atacado.");
    expect(fortes(msg)).toContain("Silva");
    withLang("en-US", () => {
      const [m] = buildInboxMessages(FAV);
      expect(corpo(m)).toContain("a veteran with 2 career titles");
      expect(corpo(m)).toContain("leads the championship by 5 points");
      expect(corpo(m)).toContain("Flawless wheel-to-wheel, but vulnerable when attacked.");
    });
  });

  it("interesse: concordância de plural (dessa/dessas) e nível de fama", () => {
    const two = buildInboxMessages(INTEREST(["Alpha", "Beta"], 75))[0];
    expect(two.paragrafos).toHaveLength(2);
    expect(corpo(two)).toContain("demonstraram interesse");
    expect(corpo(two)).toContain("dessas equipes");
    expect(corpo(two)).toContain("Alpha e Beta");
    expect(fortes(two)).toEqual(
      expect.arrayContaining(["Alpha", "Beta", "Estrela", "piloto titular"]),
    );
    expect(two.kind).toBe("Interesse de 2 equipes");

    const one = buildInboxMessages(INTEREST(["Alpha"], 40))[0];
    expect(corpo(one)).toContain("demonstrou interesse");
    expect(corpo(one)).toContain("dessa equipe");

    withLang("en-US", () => {
      const t2 = buildInboxMessages(INTEREST(["Alpha", "Beta"], 75))[0];
      expect(corpo(t2)).toContain("have shown interest");
      expect(corpo(t2)).toContain("those teams");
      expect(corpo(t2)).toContain("Alpha and Beta");
      expect(fortes(t2)).toEqual(expect.arrayContaining(["Alpha", "Beta", "Star"]));
      const t1 = buildInboxMessages(INTEREST(["Alpha"], 40))[0];
      expect(corpo(t1)).toContain("has shown interest");
      expect(corpo(t1)).toContain("that team");
    });
  });

  it("nenhum trecho carrega marcação: o <b> do locale vira tipo, não texto", () => {
    const msgs = buildInboxMessages({ ...H2H, ...FAV, ...INTEREST(["Alpha", "Beta"], 75) });
    expect(msgs).toHaveLength(3);
    for (const msg of msgs) {
      for (const trecho of msg.paragrafos.flat()) {
        expect(trecho.tipo).toMatch(/^(texto|forte)$/);
        expect(trecho.texto).not.toContain("<b>");
        expect(trecho.texto).not.toContain("</b>");
        expect(trecho.texto).not.toContain("<p>");
      }
    }
  });
});
