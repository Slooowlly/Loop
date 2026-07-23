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
  it("head-to-head: PT baseline com plural, ordinal e HTML", () => {
    const [msg] = buildInboxMessages(H2H);
    expect(msg.body).toContain("3 vezes");
    expect(msg.body).toContain("seu 2º em Monza");
    expect(msg.body).toContain("<b>Ruiz</b>");
    expect(msg.body).toContain("<b>1</b>");
    expect(msg.subject).toBe("Você e Ruiz voltam a se cruzar.");
  });

  it("head-to-head: troca pra EN (plural + ordinal en)", () => {
    withLang("en-US", () => {
      const [msg] = buildInboxMessages(H2H);
      expect(msg.body).toContain("3 times");
      expect(msg.body).toContain("your 2nd at Monza");
      expect(msg.subject).toBe("You and Ruiz cross paths again.");
      expect(msg.from).toBe("Grid bulletin");
    });
  });

  it("head-to-head: singular (1 vez / 1 time)", () => {
    const facts = { head_to_head: { ...H2H.head_to_head, races_together: 1, player_ahead: 0 } };
    expect(buildInboxMessages(facts)[0].body).toContain("1 vez");
    withLang("en-US", () => {
      expect(buildInboxMessages(facts)[0].body).toContain("1 time");
    });
  });

  it("favorito: perfil/standing/traits com plural e atributos", () => {
    const [msg] = buildInboxMessages(FAV);
    expect(msg.body).toContain("veterano com 2 títulos");
    expect(msg.body).toContain("lidera o campeonato por 5 pontos");
    expect(msg.body).toContain("Impecável no corpo a corpo, mas vulnerável quando atacado.");
    withLang("en-US", () => {
      const [m] = buildInboxMessages(FAV);
      expect(m.body).toContain("a veteran with 2 career titles");
      expect(m.body).toContain("leads the championship by 5 points");
      expect(m.body).toContain("Flawless wheel-to-wheel, but vulnerable when attacked.");
    });
  });

  it("interesse: concordância de plural (dessa/dessas) e nível de fama", () => {
    const two = buildInboxMessages(INTEREST(["Alpha", "Beta"], 75))[0];
    expect(two.body).toContain("demonstraram interesse");
    expect(two.body).toContain("dessas equipes");
    expect(two.body).toContain("<b>Estrela</b>");
    expect(two.body).toContain("<b>Alpha</b> e <b>Beta</b>");
    expect(two.kind).toBe("Interesse de 2 equipes");

    const one = buildInboxMessages(INTEREST(["Alpha"], 40))[0];
    expect(one.body).toContain("demonstrou interesse");
    expect(one.body).toContain("dessa equipe");

    withLang("en-US", () => {
      const t2 = buildInboxMessages(INTEREST(["Alpha", "Beta"], 75))[0];
      expect(t2.body).toContain("have shown interest");
      expect(t2.body).toContain("those teams");
      expect(t2.body).toContain("<b>Star</b>");
      expect(t2.body).toContain("<b>Alpha</b> and <b>Beta</b>");
      const t1 = buildInboxMessages(INTEREST(["Alpha"], 40))[0];
      expect(t1.body).toContain("has shown interest");
      expect(t1.body).toContain("that team");
    });
  });
});
