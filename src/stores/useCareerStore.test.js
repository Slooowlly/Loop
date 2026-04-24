import { vi, describe, it, expect, beforeEach, afterEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import useCareerStore, {
  buildCalendarAdvanceTiming,
} from "./useCareerStore";

describe("buildCalendarAdvanceTiming", () => {
  it("uses a short 1.5s animation for very small day jumps", () => {
    expect(buildCalendarAdvanceTiming(1)).toEqual({
      totalDurationMs: 1500,
      stepMs: 1500,
    });

    expect(buildCalendarAdvanceTiming(3)).toEqual({
      totalDurationMs: 1500,
      stepMs: 500,
    });
  });

  it("caps large day jumps at 3s and speeds the steps up", () => {
    expect(buildCalendarAdvanceTiming(14)).toEqual({
      totalDurationMs: 3000,
      stepMs: 214,
    });

    expect(buildCalendarAdvanceTiming(30)).toEqual({
      totalDurationMs: 3000,
      stepMs: 100,
    });
  });
});

describe("useCareerStore startCalendarAdvance", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useCareerStore.setState({
      isLoaded: false,
      isLoading: false,
      isSimulating: false,
      isAdvancing: false,
      isCalendarAdvancing: false,
      isAdvancingWeek: false,
      isRespondingProposal: false,
      isDirty: false,
      lastSaved: null,
      error: null,
      careerId: "career-1",
      difficulty: null,
      player: null,
      playerTeam: {
        categoria: "mazda_amador",
      },
      season: {
        id: "season-1",
      },
      nextRace: {
        id: "race-1",
        display_date: "2026-03-04",
      },
      temporalSummary: {
        current_display_date: "2026-03-01",
        next_event_display_date: "2026-03-04",
        days_until_next_event: 3,
      },
      calendarDisplayDate: "2026-03-01",
      displayDaysUntilNextEvent: 3,
      totalDrivers: 0,
      totalTeams: 0,
      lastRaceResult: null,
      otherCategoriesResult: null,
      showResult: false,
      showRaceBriefing: false,
      preseasonState: null,
      preseasonWeeks: [],
      playerProposals: [],
      playerSpecialOffers: [],
      acceptedSpecialOffer: null,
      endOfSeasonResult: null,
      showEndOfSeason: false,
      showPreseason: false,
      showConvocation: false,
    });
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it("opens the briefing after 1.5s when there are only a few days left", async () => {
    const promise = useCareerStore.getState().startCalendarAdvance();

    await vi.advanceTimersByTimeAsync(1499);
    expect(useCareerStore.getState().showRaceBriefing).toBe(false);
    expect(useCareerStore.getState().isCalendarAdvancing).toBe(true);

    await vi.advanceTimersByTimeAsync(1);
    await promise;

    expect(useCareerStore.getState().showRaceBriefing).toBe(true);
    expect(useCareerStore.getState().isCalendarAdvancing).toBe(false);
    expect(useCareerStore.getState().calendarDisplayDate).toBe("2026-03-04");
  });
});

describe("useCareerStore loadCareer", () => {
  beforeEach(() => {
    invoke.mockReset();
    useCareerStore.setState({
      isLoaded: false,
      isLoading: false,
      isSimulating: false,
      isAdvancing: false,
      isCalendarAdvancing: false,
      isAdvancingWeek: false,
      isRespondingProposal: false,
      isDirty: false,
      lastSaved: null,
      error: null,
      careerId: null,
      difficulty: null,
      player: null,
      playerTeam: null,
      season: null,
      nextRace: null,
      nextRaceBriefing: null,
      temporalSummary: null,
      calendarDisplayDate: null,
      displayDaysUntilNextEvent: null,
      totalDrivers: 0,
      totalTeams: 0,
      lastRaceResult: null,
      otherCategoriesResult: null,
      showResult: false,
      showRaceBriefing: false,
      preseasonState: null,
      preseasonWeeks: [],
      playerProposals: [],
      endOfSeasonResult: null,
      showEndOfSeason: false,
      showPreseason: false,
    });
  });

  it("stores the enriched next-race briefing from load_career", async () => {
    invoke.mockImplementation((command) => {
      if (command === "load_career") {
        return Promise.resolve({
          career_id: "career-77",
          difficulty: "medio",
          player: { id: "drv-player", nome: "R. Silva" },
          player_team: { id: "team-1", categoria: "mazda_amador" },
          season: { id: "season-1", numero: 1 },
          next_race: { id: "race-1", display_date: "2026-03-04" },
          next_race_briefing: {
            track_history: {
              has_data: true,
              starts: 2,
              best_finish: 3,
            },
            primary_rival: {
              driver_id: "drv-rival",
              driver_name: "M. Costa",
              championship_position: 1,
              gap_points: 4,
              is_ahead: true,
            },
            weekend_stories: [
              {
                id: "story-1",
                title: "Duelo esquenta a abertura",
              },
            ],
          },
          total_drivers: 16,
          total_teams: 8,
        });
      }

      if (command === "get_temporal_summary") {
        return Promise.resolve({
          current_display_date: "2026-03-01",
          days_until_next_event: 3,
        });
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().loadCareer("career-77");

    expect(useCareerStore.getState().nextRaceBriefing).toEqual({
      track_history: {
        has_data: true,
        starts: 2,
        best_finish: 3,
      },
      primary_rival: {
        driver_id: "drv-rival",
        driver_name: "M. Costa",
        championship_position: 1,
        gap_points: 4,
        is_ahead: true,
      },
      weekend_stories: [
        {
          id: "story-1",
          title: "Duelo esquenta a abertura",
        },
      ],
    });
  });

  it("restores the end-of-season screen when the save snapshot requests it", async () => {
    const endOfSeasonResult = {
      growth_reports: [],
      motivation_reports: [],
      retirements: [],
      rookies_generated: [],
      new_season_id: "season-2",
      new_year: 2027,
      licenses_earned: [],
      promotion_result: {
        movements: [],
        pilot_effects: [],
        attribute_deltas: [],
        errors: [],
      },
      preseason_initialized: true,
      preseason_total_weeks: 6,
    };

    invoke.mockImplementation((command) => {
      if (command === "load_career") {
        return Promise.resolve({
          career_id: "career-77",
          difficulty: "medio",
          player: { id: "drv-player", nome: "R. Silva" },
          player_team: { id: "team-1", categoria: "mazda_amador" },
          season: { id: "season-2", numero: 2 },
          next_race: { id: "race-1", display_date: "2027-02-10" },
          next_race_briefing: null,
          total_drivers: 16,
          total_teams: 8,
          resume_context: {
            active_view: "end_of_season",
            end_of_season_result: endOfSeasonResult,
          },
        });
      }

      if (command === "get_temporal_summary") {
        return Promise.resolve(null);
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().loadCareer("career-77");

    expect(useCareerStore.getState().showEndOfSeason).toBe(true);
    expect(useCareerStore.getState().showPreseason).toBe(false);
    expect(useCareerStore.getState().endOfSeasonResult).toEqual(endOfSeasonResult);
  });

  it("restores the preseason market screen when the save snapshot requests it", async () => {
    invoke.mockImplementation((command) => {
      if (command === "load_career") {
        return Promise.resolve({
          career_id: "career-77",
          difficulty: "medio",
          player: { id: "drv-player", nome: "R. Silva" },
          player_team: { id: "team-1", categoria: "mazda_amador" },
          season: { id: "season-2", numero: 2 },
          next_race: { id: "race-1", display_date: "2027-02-10" },
          next_race_briefing: null,
          total_drivers: 16,
          total_teams: 8,
          resume_context: {
            active_view: "preseason",
            end_of_season_result: null,
          },
        });
      }

      if (command === "get_temporal_summary") {
        return Promise.resolve(null);
      }

      if (command === "get_preseason_state") {
        return Promise.resolve({
          season_number: 2,
          current_week: 5,
          total_weeks: 6,
          is_complete: false,
        });
      }

      if (command === "get_player_proposals") {
        return Promise.resolve([{ proposal_id: "proposal-1" }]);
      }

      if (command === "get_news") {
        return Promise.resolve([
          {
            semana_pretemporada: 5,
            tipo: "Mercado",
            titulo: "Semana 5",
            texto: "Movimentacao forte no paddock.",
          },
        ]);
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().loadCareer("career-77");

    expect(useCareerStore.getState().showPreseason).toBe(true);
    expect(useCareerStore.getState().showEndOfSeason).toBe(false);
    expect(useCareerStore.getState().preseasonState).toEqual({
      season_number: 2,
      current_week: 5,
      total_weeks: 6,
      is_complete: false,
    });
    expect(useCareerStore.getState().playerProposals).toEqual([{ proposal_id: "proposal-1" }]);
    expect(useCareerStore.getState().preseasonWeeks).toHaveLength(1);
  });

  it("restores the convocation screen when the save is in JanelaConvocacao", async () => {
    invoke.mockImplementation((command) => {
      if (command === "load_career") {
        return Promise.resolve({
          career_id: "career-77",
          difficulty: "medio",
          player: {
            id: "drv-player",
            nome: "R. Silva",
            categoria_especial_ativa: "endurance",
          },
          player_team: {
            id: "team-regular",
            nome: "Roadster Cup",
            categoria: "mazda_rookie",
            classe: "mazda",
          },
          season: { id: "season-2", numero: 2, fase: "JanelaConvocacao" },
          accepted_special_offer: {
            id: "offer-accepted",
            team_id: "team-special",
            team_name: "Solar GT4",
            special_category: "endurance",
            class_name: "gt4",
            papel: "Numero2",
          },
          next_race: null,
          next_race_briefing: null,
          total_drivers: 16,
          total_teams: 8,
        });
      }

      if (command === "get_temporal_summary") {
        return Promise.resolve(null);
      }

      if (command === "get_special_window_state") {
        return Promise.resolve({
          current_day: 3,
          total_days: 7,
          status: "Aberta",
          active_offer_id: "offer-1",
          is_finished: false,
          player_offers: [
            {
              id: "offer-1",
              team_id: "team-special",
              team_name: "Solar GT4",
              special_category: "endurance",
              class_name: "gt4",
              papel: "Numero2",
              status: "AceitaAtiva",
              available_from_day: 2,
              is_available_today: true,
            },
          ],
          eligible_candidates: [],
          team_sections: [],
          last_day_log: [],
        });
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().loadCareer("career-77");

    expect(useCareerStore.getState().showConvocation).toBe(true);
    expect(useCareerStore.getState().specialWindowState).toMatchObject({
      current_day: 3,
      total_days: 7,
    });
    expect(useCareerStore.getState().playerSpecialOffers).toHaveLength(1);
    expect(useCareerStore.getState().acceptedSpecialOffer).toMatchObject({
      team_id: "team-special",
      team_name: "Solar GT4",
      special_category: "endurance",
      class_name: "gt4",
    });
  });
});

describe("useCareerStore enterPreseason", () => {
  beforeEach(() => {
    invoke.mockReset();
    useCareerStore.setState({
      careerId: "career-1",
      showEndOfSeason: true,
      showPreseason: false,
      showConvocation: true,
      endOfSeasonResult: {
        new_year: 2027,
      },
      preseasonState: null,
      preseasonWeeks: [],
      playerProposals: [],
      playerSpecialOffers: [
        {
          id: "offer-1",
          team_name: "Solar GT4",
          special_category: "endurance",
        },
      ],
      acceptedSpecialOffer: {
        id: "offer-accepted",
        team_name: "Solar GT4",
        special_category: "endurance",
      },
      error: null,
    });
  });

  it("persists preseason as the resume context when entering the market", async () => {
    invoke.mockImplementation((command) => {
      if (command === "get_preseason_state") {
        return Promise.resolve({
          season_number: 2,
          current_week: 1,
          total_weeks: 6,
          is_complete: false,
        });
      }

      if (command === "get_player_proposals") {
        return Promise.resolve([]);
      }

      if (command === "get_news") {
        return Promise.resolve([]);
      }

      if (command === "set_career_resume_context") {
        return Promise.resolve(null);
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().enterPreseason();

    expect(invoke).toHaveBeenCalledWith("set_career_resume_context", {
      careerId: "career-1",
      activeView: "preseason",
      endOfSeasonResult: null,
    });
    expect(useCareerStore.getState().showPreseason).toBe(true);
    expect(useCareerStore.getState().showEndOfSeason).toBe(false);
  });

  it("keeps a visible loading state while entering the preseason market", async () => {
    let resolvePreseasonState;

    invoke.mockImplementation((command) => {
      if (command === "get_preseason_state") {
        return new Promise((resolve) => {
          resolvePreseasonState = resolve;
        });
      }

      if (command === "get_player_proposals") {
        return Promise.resolve([]);
      }

      if (command === "get_news") {
        return Promise.resolve([]);
      }

      if (command === "set_career_resume_context") {
        return Promise.resolve(null);
      }

      return Promise.resolve([]);
    });

    const enterPromise = useCareerStore.getState().enterPreseason();

    expect(useCareerStore.getState().isEnteringPreseason).toBe(true);

    resolvePreseasonState({
      season_number: 2,
      current_week: 1,
      total_weeks: 6,
      is_complete: false,
    });
    await enterPromise;

    expect(useCareerStore.getState().isEnteringPreseason).toBe(false);
  });

  it("clears special convocation state when returning to the normal preseason market", async () => {
    invoke.mockImplementation((command) => {
      if (command === "get_preseason_state") {
        return Promise.resolve({
          season_number: 2,
          current_week: 1,
          total_weeks: 6,
          is_complete: false,
        });
      }

      if (command === "get_player_proposals") {
        return Promise.resolve([]);
      }

      if (command === "get_news") {
        return Promise.resolve([]);
      }

      if (command === "set_career_resume_context") {
        return Promise.resolve(null);
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().enterPreseason();

    expect(useCareerStore.getState().showPreseason).toBe(true);
    expect(useCareerStore.getState().showConvocation).toBe(false);
    expect(useCareerStore.getState().specialWindowState).toBe(null);
    expect(useCareerStore.getState().playerSpecialOffers).toEqual([]);
    expect(useCareerStore.getState().acceptedSpecialOffer).toBe(null);
  });
});

describe("useCareerStore advanceMarketWeek", () => {
  beforeEach(() => {
    invoke.mockReset();
    useCareerStore.setState({
      careerId: "career-1",
      preseasonState: { season_number: 2, current_week: 1, total_weeks: 4 },
      lastMarketWeekResult: null,
      playerProposals: [],
      preseasonFreeAgents: [],
      isAdvancingWeek: false,
      error: null,
    });
  });

  it("stores the latest week result for the weekly closing panel", async () => {
    const weekResult = {
      week_number: 1,
      events: [
        {
          event_type: "TransferCompleted",
          driver_name: "Marta Bianco",
          categoria: "gt3",
          championship_position: 1,
        },
      ],
      player_proposals: [],
    };

    invoke.mockImplementation((command) => {
      if (command === "advance_market_week") return Promise.resolve(weekResult);
      if (command === "get_preseason_state") {
        return Promise.resolve({ season_number: 2, current_week: 2, total_weeks: 4 });
      }
      if (command === "get_preseason_free_agents") return Promise.resolve([]);
      if (command === "get_news") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await useCareerStore.getState().advanceMarketWeek();

    expect(useCareerStore.getState().lastMarketWeekResult).toEqual(weekResult);
  });
});

describe("useCareerStore specialWindow", () => {
  beforeEach(() => {
    invoke.mockReset();
    useCareerStore.setState({
      careerId: "career-1",
      specialWindowState: null,
      playerSpecialOffers: [],
      acceptedSpecialOffer: null,
      isConvocating: false,
      error: null,
    });
  });

  it("loads the special window payload and stores the visible offers", async () => {
    invoke.mockImplementation((command) => {
      if (command === "get_special_window_state") {
        return Promise.resolve({
          current_day: 2,
          total_days: 7,
          status: "Aberta",
          active_offer_id: "offer-1",
          is_finished: false,
          player_offers: [
            {
              id: "offer-1",
              team_id: "team-special",
              team_name: "Solar GT4",
              special_category: "endurance",
              class_name: "gt4",
              papel: "Numero2",
              status: "AceitaAtiva",
              available_from_day: 2,
              is_available_today: true,
            },
          ],
          eligible_candidates: [],
          team_sections: [],
          last_day_log: [],
        });
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().loadSpecialWindowState();

    expect(useCareerStore.getState().specialWindowState).toMatchObject({
      current_day: 2,
      total_days: 7,
    });
    expect(useCareerStore.getState().playerSpecialOffers).toHaveLength(1);
    expect(useCareerStore.getState().acceptedSpecialOffer).toMatchObject({
      id: "offer-1",
      team_name: "Solar GT4",
    });
  });

  it("accepts an offer as the active choice of the day", async () => {
    invoke.mockImplementation((command) => {
      if (command === "accept_special_offer_for_day") {
        return Promise.resolve({
          current_day: 3,
          total_days: 7,
          status: "Aberta",
          active_offer_id: "offer-1",
          is_finished: false,
          player_offers: [
            {
              id: "offer-1",
              team_id: "team-special",
              team_name: "Solar GT4",
              special_category: "endurance",
              class_name: "gt4",
              papel: "Numero2",
              status: "AceitaAtiva",
              available_from_day: 2,
              is_available_today: true,
            },
          ],
          eligible_candidates: [],
          team_sections: [],
          last_day_log: [],
        });
      }

      return Promise.resolve(null);
    });

    await useCareerStore.getState().acceptSpecialOfferForDay("offer-1");

    expect(useCareerStore.getState().specialWindowState).toMatchObject({
      active_offer_id: "offer-1",
    });
    expect(useCareerStore.getState().playerSpecialOffers[0]).toMatchObject({
      status: "AceitaAtiva",
    });
  });
});

describe("useCareerStore dismissResult", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("does not auto-open the convocation window after the last regular race result is dismissed", async () => {
    const mockLoadCareer = vi.fn().mockResolvedValue({
      season: { fase: "BlocoRegular" },
      next_race: null,
    });
    const mockRunConvocationWindow = vi.fn();

    useCareerStore.setState({
      careerId: "career-1",
      showResult: true,
      loadCareer: mockLoadCareer,
      runConvocationWindow: mockRunConvocationWindow,
    });

    await useCareerStore.getState().dismissResult();

    expect(mockLoadCareer).toHaveBeenCalledWith("career-1");
    expect(mockRunConvocationWindow).not.toHaveBeenCalled();
    expect(useCareerStore.getState().showResult).toBe(false);
  });
});

describe("useCareerStore respondToSpecialOffer", () => {
  beforeEach(() => {
    invoke.mockReset();
    useCareerStore.setState({
      careerId: "career-1",
      playerSpecialOffers: [
        {
          id: "offer-1",
          team_name: "Solar GT4",
          special_category: "endurance",
          class_name: "gt4",
        },
      ],
      acceptedSpecialOffer: null,
      isConvocating: false,
      error: null,
    });
  });

  it("stores the accepted special offer and clears pending offers", async () => {
    invoke.mockImplementation((command) => {
      if (command === "respond_player_special_offer") {
        return Promise.resolve({
          success: true,
          action: "accepted",
          message: "ok",
          special_category: "endurance",
          remaining_offers: 0,
        });
      }

      return Promise.resolve([]);
    });

    await useCareerStore.getState().respondToSpecialOffer("offer-1", true);

    expect(useCareerStore.getState().playerSpecialOffers).toEqual([]);
    expect(useCareerStore.getState().acceptedSpecialOffer).toMatchObject({
      id: "offer-1",
      team_name: "Solar GT4",
      special_category: "endurance",
    });
  });
});
