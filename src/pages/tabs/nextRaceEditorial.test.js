import { describe, expect, it } from "vitest";

import {
  buildEditorialCopy,
  classifyChampionshipState,
  classifyWeekendState,
  EDITORIAL_COPY_POOLS,
} from "./nextRaceEditorial";

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

describe("classifyWeekendState", () => {
  it("marks a heated weekend when stories and rival are active", () => {
    expect(
      classifyWeekendState({
        trackHistory: { has_data: true, best_finish: 2, dnfs: 0 },
        briefingRival: { driver_name: "M. Costa" },
        nextRace: { clima: "Wet" },
        weekendStories: [{ importanceLabel: "Alta" }, { importanceLabel: "Media" }],
      }),
    ).toBe("weekend_hot");
  });

  it("marks a negative-history weekend when the track has only bad memories", () => {
    expect(
      classifyWeekendState({
        trackHistory: { has_data: true, starts: 3, best_finish: 11, dnfs: 2 },
        briefingRival: null,
        nextRace: { clima: "Dry" },
        weekendStories: [],
      }),
    ).toBe("history_negative");
  });
});

describe("buildEditorialCopy", () => {
  it("uses title-chase language when the championship is alive", () => {
    const copy = buildEditorialCopy({
      championshipState: "chase",
      weekendState: "rival_spotlight",
      playerStanding: { posicao_campeonato: 2, pontos: 88 },
      leader: { nome: "M. Costa", pontos: 94 },
      rival: { nome: "M. Costa" },
      briefingRival: {
        driver_name: "M. Costa",
        championship_position: 1,
        gap_points: 6,
        is_ahead: true,
      },
      playerTeam: { nome: "Equipe Aurora" },
      nextRace: { track_name: "Interlagos" },
      trackHistory: { has_data: true, starts: 4, best_finish: 1, dnfs: 0, last_visit_season: 1 },
      weekendStories: [{ title: "Duelo esquenta a abertura", summary: "Paddock em alerta.", importanceLabel: "Alta" }],
      gapToLeader: 6,
      gapBehind: 16,
      remainingRounds: 5,
      audienceEstimate: 84200,
    });

    expect(copy.headline).toMatch(/encurtar|pressionar|aproximar/i);
    expect(copy.rivalSummary).toMatch(/M\. Costa/i);
    expect(copy.actionHint).toMatch(/duelo|simular/i);
  });

  it("uses survival language when the title is already out of reach", () => {
    const copy = buildEditorialCopy({
      championshipState: "outsider",
      weekendState: "history_positive",
      playerStanding: { posicao_campeonato: 8, pontos: 9 },
      leader: { nome: "M. Costa", pontos: 50 },
      rival: { nome: "M. Costa" },
      briefingRival: {
        driver_name: "M. Costa",
        championship_position: 1,
        gap_points: 41,
        is_ahead: true,
      },
      playerTeam: { nome: "Equipe Aurora" },
      nextRace: { track_name: "Interlagos" },
      trackHistory: { has_data: true, starts: 4, best_finish: 1, dnfs: 0, last_visit_season: 1 },
      weekendStories: [],
      gapToLeader: 41,
      gapBehind: 2,
      remainingRounds: 1,
      audienceEstimate: 84200,
    });

    expect(copy.headline).toMatch(/dignidade competitiva|reagir|fechar a temporada/i);
    expect(copy.scenario).toMatch(/muito improvavel|somar pontos|caos da prova/i);
    expect(copy.weekendStoriesEmpty).toMatch(/paddock|pista/i);
  });
});

describe("EDITORIAL_COPY_POOLS", () => {
  it("keeps at least 10 alternatives for each main editorial block", () => {
    const sections = [
      EDITORIAL_COPY_POOLS.headline,
      EDITORIAL_COPY_POOLS.championshipParagraph,
      EDITORIAL_COPY_POOLS.weekendParagraph,
      EDITORIAL_COPY_POOLS.quote,
      EDITORIAL_COPY_POOLS.rivalSummaryAhead,
      EDITORIAL_COPY_POOLS.scenario,
      EDITORIAL_COPY_POOLS.actionHint,
    ];

    for (const section of sections) {
      for (const variants of Object.values(section)) {
        expect(Array.isArray(variants)).toBe(true);
        expect(variants.length).toBeGreaterThanOrEqual(10);
      }
    }
  });
});
