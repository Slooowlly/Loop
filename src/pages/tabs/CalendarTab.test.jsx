import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import CalendarTab, { getRaceTooltipStyle } from "./CalendarTab";

let mockState = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("../../components/ui/GlassCard", () => ({
  default: ({ children, className = "" }) => <div className={className}>{children}</div>,
}));

vi.mock("../../components/ui/GlassButton", () => ({
  default: ({ children, ...props }) => <button {...props}>{children}</button>,
}));

describe("CalendarTab", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((command, payload) => {
      if (command === "get_calendar_for_category" && payload?.category === "mazda_rookie") {
        return Promise.resolve([
          {
            id: "race-1",
            rodada: 1,
            track_id: 166,
            track_name: "Okayama",
            categoria: "mazda_rookie",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 25,
            clima: "Clear",
          },
          {
            id: "race-2",
            rodada: 2,
            track_id: 47,
            track_name: "Laguna Seca",
            categoria: "mazda_rookie",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-17",
            duracao_corrida_min: 30,
            clima: "Wet",
          },
        ]);
      }

      if (command === "get_calendar_for_category" && payload?.category === "toyota_rookie") {
        return Promise.resolve([
          {
            id: "other-race-toyota-1",
            rodada: 1,
            track_id: 14,
            track_name: "Lime Rock",
            categoria: "toyota_rookie",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 20,
            clima: "Clear",
          },
        ]);
      }

      if (command === "get_calendar_for_category" && payload?.category === "mazda_amador") {
        return Promise.resolve([
          {
            id: "other-race-mazda-amador-1",
            rodada: 1,
            track_id: 261,
            track_name: "Oulton Park",
            categoria: "mazda_amador",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 25,
            clima: "Clear",
          },
          {
            id: "other-race-mazda-amador-2",
            rodada: 2,
            track_id: 47,
            track_name: "Laguna Seca",
            categoria: "mazda_amador",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-13",
            duracao_corrida_min: 30,
            clima: "Wet",
          },
        ]);
      }

      if (command === "get_calendar_for_category" && payload?.category === "endurance") {
        return Promise.resolve([
          {
            id: "special-race-1",
            rodada: 1,
            track_id: 515,
            track_name: "Navarra",
            categoria: "endurance",
            season_phase: "BlocoEspecial",
            status: "Pendente",
            display_date: "2026-09-13",
            duracao_corrida_min: 45,
            clima: "Clear",
          },
          {
            id: "special-race-2",
            rodada: 2,
            track_id: 554,
            track_name: "Charlotte",
            categoria: "endurance",
            season_phase: "BlocoEspecial",
            status: "Pendente",
            display_date: "2026-10-04",
            duracao_corrida_min: 50,
            clima: "Damp",
          },
        ]);
      }

      return Promise.resolve([]);
    });

    mockState = {
      careerId: "career-1",
      playerTeam: {
        categoria: "mazda_rookie",
      },
      nextRace: {
        id: "race-2",
      },
      season: {
        ano: 2026,
        rodada_atual: 1,
        fase: "BlocoRegular",
      },
      specialWindowState: null,
      playerSpecialOffers: [],
      acceptedSpecialOffer: null,
      isConvocating: false,
      loadSpecialWindowState: vi.fn(),
      isCalendarAdvancing: true,
      calendarDisplayDate: "2026-03-12",
    };
  });

  it("highlights the animated current day and renders the month progress rail while the calendar tab is active", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const activeDay = await screen.findByTestId("calendar-day-2026-03-12");
    expect(activeDay).toHaveAttribute("data-animated-current-day", "true");

    const activeMonthRail = screen.getByTestId("calendar-progress-2026-03");
    expect(activeMonthRail).toHaveAttribute("data-animated-month", "true");
  });

  it("shows only phase/event legends in the calendar header", async () => {
    render(<CalendarTab activeTab="calendar" />);

    await screen.findByTestId("calendar-day-2026-03-12");
    const legend = within(screen.getByTestId("calendar-legend"));

    expect(legend.queryByText("Próxima")).not.toBeInTheDocument();
    expect(legend.queryByText("Concluída")).not.toBeInTheDocument();
    expect(legend.queryByText("Pendente")).not.toBeInTheDocument();
    expect(legend.getByText("Mercado")).toBeInTheDocument();
    expect(legend.getByText("Convocação")).toBeInTheDocument();
    expect(legend.getByText("Bloco Especial")).toBeInTheDocument();
  });

  it("keeps the day-pass animation visible even when the current date has no race", async () => {
    mockState.calendarDisplayDate = "2026-03-13";

    render(<CalendarTab activeTab="calendar" />);

    const activeDay = await screen.findByTestId("calendar-day-2026-03-13");
    expect(activeDay).toHaveAttribute("data-animated-current-day", "true");
    expect(activeDay).toHaveAttribute("data-animated-visual", "true");
  });

  it("keeps the current-day marker visible even after the advance animation ends", async () => {
    mockState.isCalendarAdvancing = false;
    mockState.calendarDisplayDate = "2026-03-14";

    render(<CalendarTab activeTab="calendar" />);

    const currentDay = await screen.findByTestId("calendar-day-2026-03-14");
    expect(currentDay).toHaveAttribute("data-current-calendar-day", "true");
    expect(currentDay).toHaveAttribute("data-animated-visual", "true");
    expect(within(currentDay).queryByText(/sábado/i)).not.toBeInTheDocument();
    expect(within(currentDay).queryByText(/hoje/i)).not.toBeInTheDocument();
    expect(screen.queryByTestId("calendar-progress-2026-03")).not.toBeInTheDocument();
  });

  it("keeps the current month visually highlighted as the active calendar window", async () => {
    mockState.isCalendarAdvancing = false;
    mockState.calendarDisplayDate = "2026-03-14";

    render(<CalendarTab activeTab="calendar" />);

    const activeMonth = await screen.findByTestId("calendar-month-2026-03");
    const inactiveMonth = screen.getByTestId("calendar-month-2026-04");

    expect(activeMonth).toHaveAttribute("data-active-month-window", "true");
    expect(inactiveMonth).toHaveAttribute("data-active-month-window", "false");
  });

  it("keeps passed days bright while future days in the current month remain muted", async () => {
    mockState.isCalendarAdvancing = false;
    mockState.calendarDisplayDate = "2026-03-14";

    render(<CalendarTab activeTab="calendar" />);

    const reachedCurrentMonthDay = await screen.findByTestId("calendar-day-2026-03-11");
    const futureCurrentMonthDay = screen.getByTestId("calendar-day-2026-03-16");
    const futureMonthDay = screen.getByTestId("calendar-day-2026-04-01");

    expect(reachedCurrentMonthDay).toHaveClass("text-text-primary");
    expect(futureCurrentMonthDay).toHaveClass("text-text-muted/50");
    expect(futureMonthDay).toHaveClass("text-text-muted/50");
    expect(futureCurrentMonthDay).not.toHaveClass("text-text-primary");
  });

  it("opens the calendar after the player category loads without waiting for other categories", async () => {
    const pendingOtherCategories = new Promise(() => {});

    invoke.mockImplementation((command, payload) => {
      if (command === "get_calendar_for_category" && payload?.category === "mazda_rookie") {
        return Promise.resolve([
          {
            id: "race-1",
            rodada: 1,
            track_id: 166,
            track_name: "Okayama",
            categoria: "mazda_rookie",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 25,
            clima: "Clear",
          },
        ]);
      }

      return pendingOtherCategories;
    });

    render(<CalendarTab activeTab="calendar" />);

    const raceDay = await screen.findByTestId(
      "calendar-day-2026-03-12",
      {},
      { timeout: 300 },
    );

    expect(raceDay).toBeInTheDocument();
    expect(screen.queryByText(/carregando calendário/i)).not.toBeInTheDocument();
  });

  it("shows ghost dots for races from other categories with lower visual priority", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const dayWithOtherCategories = await screen.findByTestId("calendar-day-2026-03-12");

    expect(dayWithOtherCategories).toHaveAttribute("data-other-category-count", "2");
    expect(within(dayWithOtherCategories).getByTestId("calendar-other-categories-2026-03-12")).toBeInTheDocument();
    expect(within(dayWithOtherCategories).getAllByTestId("calendar-other-category-dot")).toHaveLength(2);
  });

  it("uses the championship color families to differentiate rookie and amateur ghost dots", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const mazdaOnlyDay = await screen.findByTestId("calendar-day-2026-03-13");
    const [mazdaDot] = within(mazdaOnlyDay).getAllByTestId("calendar-other-category-dot");

    expect(mazdaDot).toHaveStyle({ backgroundColor: "#E73F47" });

    const mixedDay = await screen.findByTestId("calendar-day-2026-03-12");
    const mixedDots = within(mixedDay).getAllByTestId("calendar-other-category-dot");

    expect(mixedDots[0]).toHaveStyle({ backgroundColor: "#FFD400" });
    expect(mixedDots[1]).toHaveStyle({ backgroundColor: "#E73F47" });
  });

  it("keeps ghost-dot category colors visually active in the calendar", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const mixedDay = await screen.findByTestId("calendar-day-2026-03-12");
    const ghostDotGroup = within(mixedDay).getByTestId("calendar-other-categories-2026-03-12");

    expect(ghostDotGroup).toHaveClass("opacity-90");
    expect(ghostDotGroup).not.toHaveClass("opacity-45");
  });

  it("colors ghost dots by the fetched category even when a backend entry omits categoria", async () => {
    invoke.mockImplementation((command, payload) => {
      if (command !== "get_calendar_for_category") return Promise.resolve([]);
      if (payload?.category === "toyota_rookie") {
        return Promise.resolve([
          {
            id: "other-race-without-category",
            rodada: 1,
            track_id: 14,
            track_name: "Lime Rock",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 20,
            clima: "Clear",
          },
        ]);
      }
      return Promise.resolve([]);
    });

    render(<CalendarTab activeTab="calendar" />);

    const dayWithOtherCategory = await screen.findByTestId("calendar-day-2026-03-12");
    const [ghostDot] = within(dayWithOtherCategory).getAllByTestId("calendar-other-category-dot");

    expect(ghostDot).toHaveAttribute("data-category", "toyota_rookie");
    expect(ghostDot).toHaveStyle({ backgroundColor: "#FFD400" });
  });

  it("renders the player race day with the track image instead of a central category marker", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const raceDay = await screen.findByTestId("calendar-day-2026-03-12");

    expect(within(raceDay).queryByText("R1")).not.toBeInTheDocument();
    expect(within(raceDay).queryByTestId("calendar-category-logo")).not.toBeInTheDocument();
    expect(within(raceDay).queryByTestId("calendar-player-race-dot")).not.toBeInTheDocument();
    expect(within(raceDay).getByRole("img", { name: "Okayama" })).toBeInTheDocument();
  });

  it("adds a short arrival-feedback flash to the current race day when requested", async () => {
    mockState.isCalendarAdvancing = false;
    mockState.calendarDisplayDate = "2026-03-12";

    render(<CalendarTab activeTab="calendar" raceArrivalFeedbackActive />);

    const raceDay = await screen.findByTestId("calendar-day-2026-03-12");
    const flash = within(raceDay).getByTestId("calendar-race-arrival-flash");

    expect(raceDay).toHaveAttribute("data-race-arrival-feedback", "true");
    expect(flash).toBeInTheDocument();
    expect(flash).toHaveClass("calendar-race-arrival-flash");
    expect(flash.className).not.toContain("pulse");
  });

  it("keeps future race days in the current month muted while reached race days stay lit", async () => {
    mockState.isCalendarAdvancing = false;
    mockState.calendarDisplayDate = "2026-03-14";

    render(<CalendarTab activeTab="calendar" />);

    const reachedRaceDay = await screen.findByTestId("calendar-day-2026-03-12");
    const futureRaceDay = screen.getByTestId("calendar-day-2026-03-17");

    expect(reachedRaceDay).toHaveAttribute("data-current-month-progress", "reached");
    expect(futureRaceDay).toHaveAttribute("data-current-month-progress", "future");
  });

  it("resolves known track images by track name when the calendar id is not mapped", async () => {
    invoke.mockImplementation((command, payload) => {
      if (command === "get_calendar_for_category" && payload?.category === "mazda_rookie") {
        return Promise.resolve([
          {
            id: "race-charlotte",
            rodada: 1,
            track_id: 9999,
            track_name: "Charlotte Motor Speedway - Roval",
            categoria: "mazda_rookie",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 25,
            clima: "Clear",
          },
        ]);
      }

      return Promise.resolve([]);
    });

    render(<CalendarTab activeTab="calendar" />);

    const raceDay = await screen.findByTestId("calendar-day-2026-03-12");
    const image = within(raceDay).getByRole("img", { name: "Charlotte Motor Speedway - Roval" });

    expect(image).toHaveAttribute("src", "/tracks/charlotte.png");
  });

  it("keeps the dedicated BMW M2 Cup logo in the tooltip while the race day relies on the track image", async () => {
    mockState.playerTeam = { categoria: "bmw_m2" };
    invoke.mockImplementation((command, payload) => {
      if (command === "get_calendar_for_category" && payload?.category === "bmw_m2") {
        return Promise.resolve([
          {
            id: "bmw-race-1",
            rodada: 1,
            track_id: 325,
            track_name: "Tsukuba",
            categoria: "bmw_m2",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 30,
            clima: "Clear",
          },
        ]);
      }

      return Promise.resolve([]);
    });

    render(<CalendarTab activeTab="calendar" />);

    const raceDay = await screen.findByTestId("calendar-day-2026-03-12");
    expect(within(raceDay).queryByTestId("calendar-player-race-dot")).not.toBeInTheDocument();
    expect(within(raceDay).getByRole("img", { name: "Tsukuba" })).toBeInTheDocument();

    fireEvent.mouseEnter(raceDay);

    const tooltip = await screen.findByTestId("calendar-tooltip");
    const logo = within(tooltip).getByTestId("calendar-tooltip-category-logo");
    expect(logo).toHaveAttribute("alt", "BMW M2 CS Racing");
    expect(logo).toHaveAttribute("src", "/categorias/M2%20CUP.png");
  });

  it("shows a tooltip with other-category race details when hovering a ghost-dot day", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const dayWithOnlyOtherCategories = await screen.findByTestId("calendar-day-2026-03-13");
    fireEvent.mouseEnter(dayWithOnlyOtherCategories);

    const tooltip = await screen.findByTestId("calendar-tooltip");
    expect(within(tooltip).queryByText(/outras categorias no dia/i)).not.toBeInTheDocument();
    expect(within(tooltip).getByText(/mazda mx-5 championship/i)).toBeInTheDocument();
    expect(within(tooltip).getByText(/laguna seca/i)).toBeInTheDocument();
    expect(within(tooltip).getByText("Etapa 2")).toBeInTheDocument();
  });

  it("shows category logos inside tooltip race details", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const raceDay = await screen.findByTestId("calendar-day-2026-03-12");
    fireEvent.mouseEnter(raceDay);

    const mainTooltip = await screen.findByTestId("calendar-tooltip");
    expect(mainTooltip.parentElement).toBe(document.body);
    const mainTicket = within(mainTooltip).getByTestId("calendar-tooltip-race-ticket");
    const mainLogo = within(mainTooltip).getByTestId("calendar-tooltip-category-logo");
    expect(mainTicket).toBeInTheDocument();
    expect(mainLogo).toHaveAttribute("alt", "Mazda MX-5 Rookie Cup");
    expect(mainLogo).toHaveAttribute("src", "/categorias/MX5%20ROOKIE.png");

    fireEvent.mouseLeave(raceDay);

    const dayWithOnlyOtherCategories = await screen.findByTestId("calendar-day-2026-03-13");
    fireEvent.mouseEnter(dayWithOnlyOtherCategories);

    const otherTooltip = await screen.findByTestId("calendar-tooltip");
    const otherLogo = within(otherTooltip).getByTestId("calendar-tooltip-other-category-logo");
    expect(otherLogo).toHaveAttribute("alt", "Mazda MX-5 Championship");
    expect(otherLogo).toHaveAttribute("src", "/categorias/MX5%20CUP.png");
  });

  it("renders other-category races as compact ticket tooltips", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const dayWithOnlyOtherCategories = await screen.findByTestId("calendar-day-2026-03-13");
    fireEvent.mouseEnter(dayWithOnlyOtherCategories);

    const tooltip = await screen.findByTestId("calendar-tooltip");
    expect(within(tooltip).queryByText(/1 corrida no mesmo calendário/i)).not.toBeInTheDocument();
    expect(within(tooltip).queryByText(/outras categorias no dia/i)).not.toBeInTheDocument();
    expect(within(tooltip).queryByTestId("calendar-tooltip-header-category-logo")).not.toBeInTheDocument();

    const surface = within(tooltip).getByTestId("calendar-tooltip-surface");
    expect(surface).not.toHaveClass("border");
    expect(surface).not.toHaveClass("rounded-xl");
    expect(surface).not.toHaveClass("shadow-2xl");

    const ticket = within(tooltip).getByTestId("calendar-tooltip-other-race-ticket");
    expect(ticket).toHaveTextContent("Etapa 2");
    expect(ticket).toHaveTextContent("Laguna Seca");
    expect(ticket).toHaveTextContent("Duração");
    expect(ticket).toHaveTextContent("30 min");
    expect(ticket).toHaveTextContent("Clima");
    expect(ticket).toHaveTextContent("A definir");
    expect(ticket).toHaveTextContent("Status");
    expect(ticket).toHaveTextContent("Pendente");

    const logo = within(ticket).getByTestId("calendar-tooltip-other-category-logo");
    expect(logo).toHaveAttribute("alt", "Mazda MX-5 Championship");
    expect(logo).toHaveAttribute("src", "/categorias/MX5%20CUP.png");
    expect(logo).toHaveClass("h-32");
    expect(logo).toHaveClass("w-[200px]");

    const barcode = within(ticket).getByTestId("calendar-tooltip-ticket-barcode");
    expect(barcode).toHaveClass("absolute");
    expect(barcode).toHaveClass("bottom-0");
    expect(barcode).toHaveClass("top-0");

    const detail = within(ticket).getByTestId("calendar-tooltip-ticket-detail-duration");
    expect(detail).toHaveClass("text-center");
  });

  it("renders the player race tooltip with the same ticket mold used by other categories", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const raceDay = await screen.findByTestId("calendar-day-2026-03-12");
    fireEvent.mouseEnter(raceDay);

    const tooltip = await screen.findByTestId("calendar-tooltip");
    const surface = within(tooltip).getByTestId("calendar-tooltip-surface");
    expect(surface).not.toHaveClass("border");
    expect(surface).not.toHaveClass("rounded-xl");
    expect(surface).not.toHaveClass("shadow-2xl");

    const ticket = within(tooltip).getByTestId("calendar-tooltip-race-ticket");
    expect(ticket).toHaveTextContent("Etapa 1");
    expect(ticket).toHaveTextContent("Okayama");
    expect(within(ticket).queryByText("Rodada")).not.toBeInTheDocument();
    expect(within(ticket).queryByText("R1")).not.toBeInTheDocument();

    const logo = within(ticket).getByTestId("calendar-tooltip-category-logo");
    expect(logo).toHaveAttribute("alt", "Mazda MX-5 Rookie Cup");
    expect(logo).toHaveClass("h-32");
    expect(logo).toHaveClass("w-[200px]");

    const barcode = within(ticket).getByTestId("calendar-tooltip-ticket-barcode");
    expect(barcode).toHaveClass("bottom-0");
    expect(barcode).toHaveClass("top-0");

    const detail = within(ticket).getByTestId("calendar-tooltip-ticket-detail-duration");
    expect(detail).toHaveClass("text-center");
  });

  it("renders the convocation week inside the month grid, fetches the special calendar, and removes the standalone card", async () => {
    mockState.acceptedSpecialOffer = {
      id: "offer-1",
      team_name: "Team Orion",
      special_category: "endurance",
      class_name: "gt4",
    };

    render(<CalendarTab activeTab="calendar" />);

    const convocationDay = await screen.findByTestId("calendar-day-2026-09-10");
    const specialRaceDay = screen.getByTestId("calendar-day-2026-09-13");

    expect(convocationDay).toHaveAttribute("data-convocation-day", "true");
    expect(specialRaceDay).toHaveAttribute("data-special-race-day", "true");
    expect(within(convocationDay).queryByText(/conv/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/mercado de convocações especiais/i)).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("get_calendar_for_category", {
      careerId: "career-1",
      category: "endurance",
    });
  });

  it("anchors the convocation fallback to the second sunday of september when the special calendar is still empty", async () => {
    mockState.acceptedSpecialOffer = {
      id: "offer-1",
      team_name: "Team Orion",
      special_category: "endurance",
      class_name: "gt4",
    };
    invoke.mockImplementation((command, payload) => {
      if (command === "get_calendar_for_category" && payload?.category === "mazda_rookie") {
        return Promise.resolve([
          {
            id: "race-1",
            rodada: 1,
            track_id: 166,
            track_name: "Okayama",
            categoria: "mazda_rookie",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-12",
            duracao_corrida_min: 25,
            clima: "Clear",
          },
        ]);
      }

      if (command === "get_calendar_for_category" && payload?.category === "endurance") {
        return Promise.resolve([]);
      }

      return Promise.resolve([]);
    });

    render(<CalendarTab activeTab="calendar" />);

    const saturdayBeforeAnchor = await screen.findByTestId("calendar-day-2026-09-05");
    const lastConvocationDay = screen.getByTestId("calendar-day-2026-09-12");

    expect(saturdayBeforeAnchor).toHaveAttribute("data-convocation-day", "false");
    expect(lastConvocationDay).toHaveAttribute("data-convocation-day", "true");
  });

  it("keeps the current-day marker visible even when today falls inside the convocation week", async () => {
    mockState.acceptedSpecialOffer = {
      id: "offer-1",
      team_name: "Team Orion",
      special_category: "endurance",
      class_name: "gt4",
    };
    mockState.isCalendarAdvancing = false;
    mockState.calendarDisplayDate = "2026-09-10";

    render(<CalendarTab activeTab="calendar" />);

    const currentDay = await screen.findByTestId("calendar-day-2026-09-10");
    expect(currentDay).toHaveAttribute("data-convocation-day", "true");
    expect(currentDay).toHaveAttribute("data-current-calendar-day", "true");
    expect(within(currentDay).queryByText(/quinta/i)).not.toBeInTheDocument();
    expect(within(currentDay).queryByText(/hoje/i)).not.toBeInTheDocument();
  });

  it("does not mark a regular-phase race as special only because the categories match", async () => {
    mockState.playerTeam = { categoria: "endurance" };
    mockState.acceptedSpecialOffer = {
      id: "offer-1",
      team_name: "Team Orion",
      special_category: "endurance",
      class_name: "gt4",
    };
    invoke.mockImplementation((command, payload) => {
      if (command === "get_calendar_for_category" && payload?.category === "endurance") {
        return Promise.resolve([
          {
            id: "race-regular-endurance",
            rodada: 3,
            track_id: 515,
            track_name: "Navarra",
            categoria: "endurance",
            season_phase: "BlocoRegular",
            status: "Pendente",
            display_date: "2026-03-20",
            duracao_corrida_min: 40,
            clima: "Clear",
          },
        ]);
      }

      return Promise.resolve([]);
    });

    render(<CalendarTab activeTab="calendar" />);

    const regularRaceDay = await screen.findByTestId("calendar-day-2026-03-20");
    expect(regularRaceDay).toHaveAttribute("data-special-race-day", "false");
  });

  it("keeps early september days in the normal color before the convocation week starts", async () => {
    mockState.acceptedSpecialOffer = {
      id: "offer-1",
      team_name: "Team Orion",
      special_category: "endurance",
      class_name: "gt4",
    };

    render(<CalendarTab activeTab="calendar" />);

    const preWindowDay = await screen.findByTestId("calendar-day-2026-09-03");
    const convocationDay = screen.getByTestId("calendar-day-2026-09-10");

    expect(preWindowDay).toHaveAttribute("data-pre-special-day", "true");
    expect(preWindowDay).toHaveAttribute("data-convocation-day", "false");
    expect(convocationDay).toHaveAttribute("data-pre-special-day", "false");
  });

  it("hides the exact weather for pending races in the tooltip", async () => {
    render(<CalendarTab activeTab="calendar" />);

    const pendingRaceDay = await screen.findByTestId("calendar-day-2026-03-12");
    fireEvent.mouseEnter(pendingRaceDay);

    const tooltip = await screen.findByTestId("calendar-tooltip");
    expect(within(tooltip).getAllByText("Clima").length).toBeGreaterThan(0);
    expect(within(tooltip).getAllByText("A definir").length).toBeGreaterThan(0);
    expect(within(tooltip).queryByText("Seco")).not.toBeInTheDocument();
  });

  it("keeps the tooltip inside the right edge of the window", () => {
    const cellRect = { left: 980, top: 300, width: 36, height: 36 };
    const style = getRaceTooltipStyle(
      cellRect,
      { width: 1024, height: 768 },
      { width: 240, height: 176 },
    );

    expect(style.left).toBeLessThanOrEqual(772);
    expect(style.left).toBeLessThan(cellRect.left);
    expect(style.top).toBe(cellRect.top - 176 - 8);
  });

  it("opens the tooltip to the left of right-edge days even before overflowing the window", () => {
    const cellRect = { left: 1090, top: 300, width: 36, height: 36 };
    const tooltipSize = { width: 240, height: 176 };
    const style = getRaceTooltipStyle(
      cellRect,
      { width: 1280, height: 768 },
      tooltipSize,
    );

    expect(style.left + tooltipSize.width).toBeLessThanOrEqual(cellRect.left + cellRect.width);
    expect(style.top).toBe(cellRect.top - tooltipSize.height - 8);
  });

  it("flips the tooltip below the cell when there is no room above", () => {
    const style = getRaceTooltipStyle(
      { left: 24, top: 40, width: 36, height: 36 },
      { width: 1024, height: 768 },
      { width: 208, height: 176 },
    );

    expect(style.top).toBe(84);
    expect(style.transform).toBe("translate(0, 0)");
  });

  it("can offset the tooltip upward from the hovered cell", () => {
    const style = getRaceTooltipStyle(
      { left: 24, top: 40, width: 36, height: 36 },
      { width: 1024, height: 768 },
      { width: 208, height: 176 },
      { verticalOffset: 20 },
    );

    expect(style.top).toBe(64);
    expect(style.transform).toBe("translate(0, 0)");
  });

  it("keeps the tooltip inside the bottom edge after scrolling near the end of the calendar", () => {
    const tooltipSize = { width: 240, height: 220 };
    const style = getRaceTooltipStyle(
      { left: 720, top: 690, width: 36, height: 36 },
      { width: 1024, height: 768 },
      tooltipSize,
    );

    expect(style.top + tooltipSize.height).toBeLessThanOrEqual(756);
  });
});
