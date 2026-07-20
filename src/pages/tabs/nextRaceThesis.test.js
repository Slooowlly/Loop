import { describe, expect, it } from "vitest";

import { selectThesis, THESIS_PRIORITY } from "./nextRaceThesis";

// Base de sinais "neutra" — nenhuma tese quente dispara, cai no baseline.
function baseSignals(overrides = {}) {
  return {
    trackName: "Interlagos",
    championshipUnderway: true,
    playerIsLeader: false,
    championshipState: "survival",
    gapToLeader: 40,
    gapBehind: 20,
    remainingRounds: 5,
    leaderName: "R. Silva",
    lastResult: { position: 6, is_dnf: false },
    averageFinish: 7,
    nemesis: null,
    trackHistory: null,
    climaWet: false,
    climaLabel: null,
    breakdownNotable: false,
    breakdownLevel: null,
    breakdownParts: null,
    grandStage: false,
    eventOccasion: "Etapa de destaque do calendário",
    audienceLabel: null,
    ...overrides,
  };
}

describe("selectThesis", () => {
  it("falls back to baseline when nothing stands out", () => {
    const thesis = selectThesis(baseSignals());
    expect(thesis.key).toBe("baseline");
    expect(thesis.statement).toMatch(/SOMAR E CRESCER/);
    expect(thesis.support.size).toBeGreaterThan(0);
  });

  it("always returns a thesis even with empty signals", () => {
    const thesis = selectThesis({});
    expect(THESIS_PRIORITY).toContain(thesis.key);
    expect(typeof thesis.statement).toBe("string");
  });

  it("elects redemption after a DNF when out of an active title fight", () => {
    const thesis = selectThesis(
      baseSignals({
        lastResult: { position: null, is_dnf: true, trackName: "Spa" },
      }),
    );
    expect(thesis.key).toBe("redemption");
    expect(thesis.statement).toMatch(/DNF/);
    expect(thesis.statement).toMatch(/Spa/);
    expect(thesis.support.has("recent_form")).toBe(true);
  });

  it("yields to title defense when the DNF did NOT cost the lead (still leading)", () => {
    const thesis = selectThesis(
      baseSignals({
        playerIsLeader: true,
        championshipState: "leader",
        lastResult: { position: null, is_dnf: true, trackName: "Spa" },
      }),
    );
    expect(thesis.key).toBe("title_defense");
    // A defesa reconhece o susto do DNF recente.
    expect(thesis.statement).toMatch(/depois de um DNF/);
  });

  it("lets a costly DNF (dropped to the chase) override the title chase", () => {
    const thesis = selectThesis(
      baseSignals({
        championshipState: "chase",
        gapToLeader: 7,
        lastResult: { position: null, is_dnf: true, trackName: "Spa" },
      }),
    );
    expect(thesis.key).toBe("redemption");
  });

  it("treats a finish far below the driver's average as redemption (relative, not absolute)", () => {
    const thesis = selectThesis(
      baseSignals({ averageFinish: 4, lastResult: { position: 18, is_dnf: false } }),
    );
    expect(thesis.key).toBe("redemption");
    expect(thesis.statement).toMatch(/P18/);
  });

  it("does not fire redemption when a below-par finish is close to the driver's average", () => {
    // P12 vs média 6 = 6 posições abaixo, menos que a margem de 8.
    const thesis = selectThesis(
      baseSignals({ averageFinish: 6, lastResult: { position: 12, is_dnf: false } }),
    );
    expect(thesis.key).not.toBe("redemption");
  });

  it("does not fire redemption on a clean mid-pack finish", () => {
    const thesis = selectThesis(baseSignals({ lastResult: { position: 8, is_dnf: false } }));
    expect(thesis.key).not.toBe("redemption");
  });

  it("elects title defense for the leader with a clean recent race", () => {
    const thesis = selectThesis(
      baseSignals({
        playerIsLeader: true,
        championshipState: "leader",
        lastResult: { position: 2, is_dnf: false },
      }),
    );
    expect(thesis.key).toBe("title_defense");
    expect(thesis.statement).toMatch(/LIDERANÇA/);
    expect(thesis.statement).not.toMatch(/depois de um DNF/);
  });

  it("elects title chase when the state is chase and the last race was clean", () => {
    const thesis = selectThesis(
      baseSignals({ championshipState: "chase", gapToLeader: 8, lastResult: { position: 3, is_dnf: false } }),
    );
    expect(thesis.key).toBe("title_chase");
    expect(thesis.statement).toMatch(/8 ponto/);
  });

  it("elects nemesis only when the rival is in the grid", () => {
    const inGrid = selectThesis(
      baseSignals({
        nemesis: { driver_name: "K. Novak", label: "O Carrasco", in_grid: true, chapters: 3, h2h_player_wins: 2, h2h_rival_wins: 1 },
      }),
    );
    expect(inGrid.key).toBe("nemesis");
    expect(inGrid.statement).toMatch(/K\. Novak/);
    expect(inGrid.statement).toMatch(/2-1/);

    const notInGrid = selectThesis(
      baseSignals({ nemesis: { driver_name: "K. Novak", in_grid: false, chapters: 3 } }),
    );
    expect(notInGrid.key).not.toBe("nemesis");
  });

  it("prioritizes the championship fight over a personal duel", () => {
    const thesis = selectThesis(
      baseSignals({
        championshipState: "chase",
        gapToLeader: 6,
        nemesis: { driver_name: "K. Novak", in_grid: true, chapters: 1, h2h_player_wins: 1, h2h_rival_wins: 0 },
      }),
    );
    expect(thesis.key).toBe("title_chase");
  });

  it("elects track trauma when the track holds DNFs", () => {
    const thesis = selectThesis(
      baseSignals({ trackHistory: { has_data: true, dnfs: 2, best_finish: 12, starts: 4 } }),
    );
    expect(thesis.key).toBe("track_trauma");
    expect(thesis.statement).toMatch(/2 abandono/);
  });

  it("elects track fortress on a strong, clean track record", () => {
    const thesis = selectThesis(
      baseSignals({ trackHistory: { has_data: true, dnfs: 0, best_finish: 2, starts: 3 } }),
    );
    expect(thesis.key).toBe("track_fortress");
    expect(thesis.statement).toMatch(/P2/);
  });

  it("elects weather on a wet forecast", () => {
    const thesis = selectThesis(baseSignals({ climaWet: true, climaLabel: "chuva forte" }));
    expect(thesis.key).toBe("weather");
    expect(thesis.statement).toMatch(/chuva forte/);
  });

  it("ranks weather above track history when both apply", () => {
    const thesis = selectThesis(
      baseSignals({
        climaWet: true,
        climaLabel: "pista molhada",
        trackHistory: { has_data: true, dnfs: 0, best_finish: 2, starts: 3 },
      }),
    );
    expect(thesis.key).toBe("weather");
  });

  it("appends a stage clause when a grand stage amplifies another thesis", () => {
    const thesis = selectThesis(
      baseSignals({
        playerIsLeader: true,
        championshipState: "leader",
        lastResult: { position: 2, is_dnf: false },
        grandStage: true,
        eventOccasion: "Grande final da temporada",
        audienceLabel: "80.000",
      }),
    );
    expect(thesis.key).toBe("title_defense");
    expect(thesis.statement).toMatch(/palco/i);
    expect(thesis.statement).toMatch(/grande final da temporada/i);
  });

  it("does not double up the stage clause when grand_stage is itself the eixo", () => {
    const thesis = selectThesis(
      baseSignals({
        grandStage: true,
        eventOccasion: "Grande final da temporada",
        audienceLabel: "80.000",
      }),
    );
    expect(thesis.key).toBe("grand_stage");
    expect(thesis.statement).not.toMatch(/palco qualquer/);
  });

  it("elects breakdown when the risk is notable", () => {
    const thesis = selectThesis(
      baseSignals({ breakdownNotable: true, breakdownLevel: "ALTO", breakdownParts: "câmbio (risco alto)" }),
    );
    expect(thesis.key).toBe("breakdown");
    expect(thesis.statement).toMatch(/câmbio/);
  });

  it("elects debut for a season opener", () => {
    const thesis = selectThesis(baseSignals({ championshipUnderway: false, lastResult: null }));
    expect(thesis.key).toBe("debut");
    expect(thesis.statement).toMatch(/RECOMEÇO/);
  });
});
