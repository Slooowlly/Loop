import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import GlobalTeamsTab from "./GlobalTeamsTab";

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../components/team/TeamLogoMark", () => ({
  default: ({ teamName, testId = "standings-team-logo" }) => (
    <span data-testid={testId}>{teamName} logo</span>
  ),
}));

vi.mock("./MyTeamTab", () => ({
  TeamHistoryDrawer: ({ team, activeTab, onClose }) => (
    <aside data-testid="team-history-drawer">
      <h4>{team.nome}</h4>
      <span>{activeTab}</span>
      <button type="button" onClick={onClose}>Fechar dossie</button>
    </aside>
  ),
}));

// ---------------------------------------------------------------------------
// Fixture redesigned for the right-hand "final standings" atlas.
//
// Geometry (all derived in the tests below):
//   familyMinYear   = max(min_year, min(band.starts_year)) = max(2000, 2016) = 2016
//   familyMaxYear   = latest point year across all bands             = 2026
//   familyDataWidth = 2026 - 2016 + 1                                = 11
//   pastMargin      = max(2, ceil(11 / 5))                           = 3
//   windowSize      = familyDataWidth + pastMargin (the LOCKED window)= 14
//   axisStartYear   = familyMinYear - pastMargin                     = 2013
//   axisEndYear     = familyMaxYear (navigable end)                  = 2026
//   renderEndYear   = axisEndYear + ceil(windowSize/3) + 1 = 2026+5+1 = 2032
//   years           = [2013 .. 2032]  → 20 columns (CHART_WIDTH 1000 ⇒ 50 u/col)
//   visible window LABEL is always axisStart-familyMax = 2013-2026 (locked).
//
// Each team has at most ONE band per year (single continuous track per team).
//
// Final-year (2026) rosters the right table shows:
//   mazda_amador : T001 (1) · T002 (2) · T003 (3)
//   production   : T008 (1) · T009 (2)
//   mazda_rookie : T006 (1)
//
// Trajectories of note:
//   T001 Sunday Speed Club — amador founder, detours into production
//        (2021/2023) then returns to win amador 2026; ONE continuous regular
//        line of 5 points ending at the 2026 right edge.
//   T002 Dual Exit Racing  — amador founder (2016 left edge), 3 titles,
//        reigning amador champion, finishes 2026 in P2.
//   T003 Porsche Black     — dark colour boosted for readability, no titles.
//   T004 Switchback Racing — moves divisions: amador 2020 → production 2021/22
//        (promotion marker @2020) → back to amador 2023 (demotion marker @2022).
//        Data ends 2023, so it is absent from every 2026 roster.
//   T005 Grid Start Racing — debuts mid-window (2018, entry label), single
//        one-year production cameo in 2020, back to amador 2022. Absent in 2026.
//   T006 Roadster Touring  — rookie champion, single title (no count badge).
//   T008/T009              — populate the 2026 production roster.
// ---------------------------------------------------------------------------
const payload = {
  selected_family: "mazda",
  min_year: 2000,
  max_year: 2026,
  current_year: 2026,
  window_start: 2000,
  window_end: 2026,
  window_size: 26,
  families: [
    {
      id: "mazda",
      label: "Mazda",
      bands: [
        { key: "production_mazda", label: "Mazda Production", category: "production_challenger", class_name: "mazda", starts_year: 2018, is_special: false },
        { key: "mazda_amador", label: "Mazda Championship", category: "mazda_amador", class_name: null, starts_year: 2016, is_special: false },
        { key: "mazda_rookie", label: "Mazda Rookie", category: "mazda_rookie", class_name: null, starts_year: 2020, is_special: false },
      ],
    },
    {
      id: "toyota",
      label: "Toyota",
      bands: [],
    },
    {
      id: "lmp2",
      label: "LMP2",
      bands: [
        { key: "lmp2", label: "LMP2", category: "lmp2", class_name: null, starts_year: 2004, is_special: false },
      ],
    },
  ],
  bands: [
    {
      key: "production_mazda",
      label: "Mazda Production",
      category: "production_challenger",
      class_name: "mazda",
      starts_year: 2018,
      is_special: false,
      rows: [
        {
          team_id: "T001",
          nome: "Sunday Speed Club",
          nome_curto: "SSC",
          cor_primaria: "#5ee7a8",
          cor_secundaria: "#114b5f",
          base_position: 1,
          titles: [
            { band_key: "mazda_amador", band_label: "Mazda Championship", count: 2 },
            { band_key: "production_mazda", band_label: "Mazda Production", count: 1 },
          ],
          is_reigning_champion: true,
          points: [
            { year: 2021, slot: "regular", position: 1, points: 102, wins: 4, titles: 1 },
            { year: 2023, slot: "regular", position: 1, points: 108, wins: 3, titles: 1 },
          ],
        },
        {
          team_id: "T004",
          nome: "Switchback Racing",
          nome_curto: "SBR",
          cor_primaria: "#58a6ff",
          cor_secundaria: "#0b2545",
          base_position: 2,
          titles: [],
          is_reigning_champion: false,
          points: [
            { year: 2021, slot: "regular", position: 2, points: 96, wins: 2, titles: 0 },
            { year: 2022, slot: "regular", position: 2, points: 92, wins: 1, titles: 0 },
          ],
        },
        {
          team_id: "T005",
          nome: "Grid Start Racing",
          nome_curto: "GSR",
          cor_primaria: "#f2c46d",
          cor_secundaria: "#3a2610",
          base_position: 5,
          titles: [],
          is_reigning_champion: false,
          points: [
            { year: 2020, slot: "regular", position: 5, points: 55, wins: 0, titles: 0 },
          ],
        },
        {
          team_id: "T008",
          nome: "Carbon Vanguard",
          nome_curto: "CVG",
          cor_primaria: "#c084fc",
          cor_secundaria: "#2a0f45",
          base_position: 1,
          titles: [{ band_key: "production_mazda", band_label: "Mazda Production", count: 1 }],
          is_reigning_champion: true,
          points: [
            { year: 2021, slot: "regular", position: 3, points: 84, wins: 1, titles: 0 },
            { year: 2026, slot: "regular", position: 1, points: 121, wins: 5, titles: 1 },
          ],
        },
        {
          team_id: "T009",
          nome: "Apex Theory",
          nome_curto: "APX",
          cor_primaria: "#ffa657",
          cor_secundaria: "#4a2408",
          base_position: 2,
          titles: [],
          is_reigning_champion: false,
          points: [
            { year: 2024, slot: "regular", position: 2, points: 88, wins: 1, titles: 0 },
            { year: 2026, slot: "regular", position: 2, points: 90, wins: 2, titles: 0 },
          ],
        },
      ],
    },
    {
      key: "mazda_amador",
      label: "Mazda Championship",
      category: "mazda_amador",
      class_name: null,
      starts_year: 2016,
      is_special: false,
      rows: [
        {
          team_id: "T001",
          nome: "Sunday Speed Club",
          nome_curto: "SSC",
          cor_primaria: "#5ee7a8",
          cor_secundaria: "#114b5f",
          base_position: 1,
          titles: [
            { band_key: "mazda_amador", band_label: "Mazda Championship", count: 2 },
            { band_key: "production_mazda", band_label: "Mazda Production", count: 1 },
          ],
          // Not the reigning amador champion (that flag belongs to T002 here); used
          // by the "star only for the reigning champion" test.
          is_reigning_champion: false,
          points: [
            { year: 2016, slot: "regular", position: 2, points: 88, wins: 1, titles: 0 },
            { year: 2018, slot: "regular", position: 1, points: 104, wins: 4, titles: 1 },
            { year: 2026, slot: "regular", position: 1, points: 118, wins: 5, titles: 1 },
          ],
        },
        {
          team_id: "T002",
          nome: "Dual Exit Racing",
          nome_curto: "DXR",
          cor_primaria: "#ff6b6b",
          cor_secundaria: "#70141d",
          base_position: 1,
          titles: [{ band_key: "mazda_amador", band_label: "Mazda Championship", count: 3 }],
          is_reigning_champion: true,
          points: [
            { year: 2016, slot: "regular", position: 1, points: 91, wins: 2, titles: 1 },
            { year: 2020, slot: "regular", position: 1, points: 112, wins: 4, titles: 1 },
            { year: 2026, slot: "regular", position: 2, points: 110, wins: 3, titles: 0 },
          ],
        },
        {
          team_id: "T003",
          nome: "Porsche Black",
          nome_curto: "PBK",
          cor_primaria: "#050505",
          cor_secundaria: "#111111",
          base_position: 3,
          titles: [],
          is_reigning_champion: false,
          points: [
            { year: 2016, slot: "regular", position: 3, points: 74, wins: 0, titles: 0 },
            { year: 2026, slot: "regular", position: 3, points: 80, wins: 1, titles: 0 },
          ],
        },
        {
          team_id: "T004",
          nome: "Switchback Racing",
          nome_curto: "SBR",
          cor_primaria: "#58a6ff",
          cor_secundaria: "#0b2545",
          base_position: 4,
          titles: [],
          is_reigning_champion: false,
          points: [
            { year: 2020, slot: "regular", position: 4, points: 65, wins: 0, titles: 0 },
            { year: 2023, slot: "regular", position: 4, points: 72, wins: 0, titles: 0 },
          ],
        },
        {
          team_id: "T005",
          nome: "Grid Start Racing",
          nome_curto: "GSR",
          cor_primaria: "#f2c46d",
          cor_secundaria: "#3a2610",
          base_position: 5,
          titles: [],
          is_reigning_champion: false,
          points: [
            { year: 2018, slot: "regular", position: 4, points: 69, wins: 0, titles: 0 },
            { year: 2022, slot: "regular", position: 5, points: 61, wins: 0, titles: 0 },
          ],
        },
      ],
    },
    {
      key: "mazda_rookie",
      label: "Mazda Rookie",
      category: "mazda_rookie",
      class_name: null,
      starts_year: 2020,
      is_special: false,
      rows: [
        {
          team_id: "T006",
          nome: "Roadster Touring",
          nome_curto: "RDT",
          cor_primaria: "#8bd3ff",
          cor_secundaria: "#0d2d4a",
          base_position: 1,
          titles: [{ band_key: "mazda_rookie", band_label: "Mazda Rookie", count: 1 }],
          is_reigning_champion: false,
          points: [
            { year: 2020, slot: "regular", position: 1, points: 93, wins: 3, titles: 1 },
            { year: 2026, slot: "regular", position: 1, points: 99, wins: 4, titles: 0 },
          ],
        },
      ],
    },
  ],
};

describe("GlobalTeamsTab", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    invoke.mockResolvedValue(payload);
  });

  it("renders the dark fixed-grid world team atlas with logos, year headers, and family filters", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    expect(screen.getByText(/Montando hist.rico mundial de equipes/i)).toBeInTheDocument();

    expect(await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i })).toBeInTheDocument();
    // Locked window label = axisStartYear-familyMaxYear = 2013-2026 (see fixture geometry).
    expect(screen.getByText(/Mazda: janela 2013-2026/i)).toBeInTheDocument();
    expect(screen.getAllByText(/Janela visivel: 2013-2026/i)).toHaveLength(2);
    expect(invoke).toHaveBeenCalledWith("get_global_team_history", {
      careerId: "career-1",
      family: "mazda",
      startYear: 2000,
      windowSize: 32,
    });

    expect(screen.getByRole("button", { name: /Mazda/i })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: /Toyota/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /LMP2/i })).toBeInTheDocument();

    const year2020 = screen.getByTestId("world-team-year-2020");
    expect(within(year2020).getByText("2020")).toBeInTheDocument();
    expect(screen.queryByText("REG")).not.toBeInTheDocument();
    expect(screen.queryByText("ESP")).not.toBeInTheDocument();
    expect(screen.getByTestId("world-team-year-2019")).toBeInTheDocument();

    expect(screen.getAllByText("Sunday Speed Club")[0]).toHaveStyle({ color: "#5ee7a8" });
    // Porsche Black now appears both as a debut entry label and a final-standings row;
    // both render the boosted (readable) colour, never the raw near-black #050505.
    expect(screen.getAllByText("Porsche Black")[0]).not.toHaveStyle({ color: "#050505" });
    expect(screen.getAllByTestId("world-team-logo").some((logo) => logo.textContent === "Sunday Speed Club logo")).toBe(true);
    expect(screen.queryByTestId("team-color-swatch")).not.toBeInTheDocument();
    expect(screen.getByTestId("world-team-grid")).toBeInTheDocument();
    expect(screen.getAllByText(/Mazda Production/).length).toBeGreaterThan(0);
    expect(screen.queryByTestId("world-team-track-T001-special")).not.toBeInTheDocument();
    expect(screen.getByTestId("world-team-track-T001-regular")).toHaveAttribute("vector-effect", "non-scaling-stroke");
    expect(screen.getByTestId("world-team-track-T003-regular")).not.toHaveAttribute("stroke", "#050505");
    // moving grid width = round(0.75 * years.length(20) / windowSize(14) * 100) = 107.1%
    expect(screen.getByTestId("world-team-moving-grid").style.width).toBe("107.1%");
    expect(screen.queryByTestId("world-team-path-T001-mazda_amador")).not.toBeInTheDocument();
  });

  it("renders columns past the navigable end and opens on the full locked family range", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // Navigable axis end label = axisEndYear = familyMaxYear = 2026 (right label of both
    // scrubbers + the 2026 year-header column = 3 nodes).
    expect(screen.getAllByText("2026")).toHaveLength(3);
    // Locked viewport = [axisStartYear(2013), familyMaxYear(2026)]
    expect(screen.getAllByText(/Janela visivel: 2013-2026/i)).toHaveLength(2);
    // renderEndYear = axisEnd(2026) + ceil(windowSize(14)/3) + 1 = 2032 is the last column.
    expect(screen.getByTestId("world-team-year-2032")).toBeInTheDocument();
    expect(screen.queryByTestId("world-team-year-2033")).not.toBeInTheDocument();
  });

  it("opens the team dossier from the lateral team row and closes it without changing the atlas", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    fireEvent.click(screen.getByTestId("world-team-row-T001-mazda_amador"));

    const drawer = await screen.findByTestId("team-history-drawer");
    expect(within(drawer).getByText("Sunday Speed Club")).toBeInTheDocument();
    expect(within(drawer).getByText("records")).toBeInTheDocument();

    fireEvent.click(within(drawer).getByRole("button", { name: /Fechar dossie/i }));

    await waitFor(() => expect(screen.queryByTestId("team-history-drawer")).not.toBeInTheDocument());
    expect(screen.getByTestId("world-team-grid")).toBeInTheDocument();
  });

  it("opens the team dossier when a visible team line is clicked", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    fireEvent.click(screen.getByTestId("world-team-track-T001-regular"));

    const drawer = await screen.findByTestId("team-history-drawer");
    expect(within(drawer).getByText("Sunday Speed Club")).toBeInTheDocument();
  });

  it("opens the team dossier when an entry label is clicked", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    fireEvent.click(screen.getByTestId("world-team-entry-label-T005-regular-2018"));

    const drawer = await screen.findByTestId("team-history-drawer");
    expect(within(drawer).getByText("Grid Start Racing")).toBeInTheDocument();
  });

  it("shows only teams present in each band's most-recent (2026) standings on the lateral rail", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // mazda_amador reference year = 2026; roster there is T001, T002, T003.
    expect(screen.getByTestId("world-team-row-T001-mazda_amador")).toBeInTheDocument();
    expect(screen.getByTestId("world-team-row-T002-mazda_amador")).toBeInTheDocument();
    expect(screen.getByTestId("world-team-row-T003-mazda_amador")).toBeInTheDocument();
    // T004 and T005 last raced in amador before 2026, so they are absent from the
    // final-year table (and T004 is absent from the 2026 production table too).
    expect(screen.queryByTestId("world-team-row-T004-mazda_amador")).not.toBeInTheDocument();
    expect(screen.queryByTestId("world-team-row-T005-mazda_amador")).not.toBeInTheDocument();
    expect(screen.queryByTestId("world-team-row-T004-production_mazda")).not.toBeInTheDocument();
  });

  it("renders Production as a real band without special parallel lines", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    expect(screen.queryByTestId("world-team-row-T001-production_mazda")).not.toBeInTheDocument();
    expect(screen.queryByTestId("world-team-row-T004-production_mazda")).not.toBeInTheDocument();
    expect(screen.queryByTestId("world-team-entry-label-T005-special-2019")).not.toBeInTheDocument();

    expect(screen.queryByTestId("world-team-track-T001-special")).not.toBeInTheDocument();
    expect(screen.queryByTestId("world-team-track-T005-special")).not.toBeInTheDocument();
    expect(screen.getByTestId("world-team-track-T001-regular")).toBeInTheDocument();
    expect(screen.getByTestId("world-team-track-T005-regular")).toBeInTheDocument();
  });

  it("highlights a team's cross-division line through its base-category row", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    fireEvent.mouseEnter(screen.getByTestId("world-team-row-T001-mazda_amador"));

    expect(screen.getByTestId("world-team-track-T001-regular")).toHaveAttribute("opacity", "0.66");
    expect(screen.getByTestId("world-team-track-T002-regular")).toHaveAttribute("opacity", "0.15");
  });

  it("highlights the base-category team row from a cross-division line", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    fireEvent.mouseEnter(screen.getByTestId("world-team-track-T001-regular"));

    expect(screen.getByTestId("world-team-row-T001-mazda_amador")).toHaveClass("bg-white/[0.06]");
    expect(screen.getByTestId("world-team-row-T002-mazda_amador")).toHaveClass("opacity-35");
  });

  it("labels teams that enter after the lateral reference year inside the grid", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    expect(screen.queryByTestId("world-team-row-T005-mazda_amador")).not.toBeInTheDocument();
    const entryLabel = screen.getByTestId("world-team-entry-label-T005-regular-2018");
    expect(entryLabel).toHaveTextContent("Grid Start Racing");
    expect(entryLabel.tagName).toBe("BUTTON");
    // T005 debuts mid-window in 2018 (years[0]=2013, idx 5). anchorX = (5+0.5)/20*1000 = 275;
    // left = anchorX / 1000 * 100 = 27.5%.
    expect(entryLabel.style.left).toBe("27.5%");
    expect(entryLabel.style.transform).toBe("translateX(calc(-100% - 8px))");
    expect(entryLabel.className).toContain("gap-2.5");
    expect(within(entryLabel).getByTestId("world-team-entry-logo")).toHaveTextContent("Grid Start Racing logo");
  });

  it("uses the category start year for the lateral rail when a category starts inside the visible window", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const rookie = screen.getByTestId("world-team-row-T006-mazda_rookie");

    expect(rookie).toBeInTheDocument();
    expect(within(rookie).getByText("Roadster Touring")).toBeInTheDocument();
    expect(within(rookie).getByText("1")).toBeInTheDocument();
  });

  it("draws one continuous regular line when a team moves into Production", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const regularPath = screen.getByTestId("world-team-track-T001-regular").getAttribute("d");

    expect(regularPath.match(/[ML]/g)).toHaveLength(5);
    expect(screen.queryByTestId("world-team-track-T001-special")).not.toBeInTheDocument();
  });

  it("anchors a category-founding line to the year start, a mid-history entry to mid-year, and the last season to the year end", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // axis = [2013..2032] (20 cols), CHART_WIDTH=1000 → 1 column = 50 units. years[0]=2013.
    // T002 is in mazda_amador from its debut year (2016 = band's first data year),
    // so its line STARTS at the left edge of 2016: idx 3 → 3/20*1000 = 150.
    const dual = screen.getByTestId("world-team-track-T002-regular").getAttribute("d");
    expect(dual.startsWith("M 150 ")).toBe(true);

    // T005 joins mazda_amador in 2018 (after the 2016 debut) → mid-history entry,
    // kept at the mid-column anchor: idx 5 → (5+0.5)/20*1000 = 275.
    const grid = screen.getByTestId("world-team-track-T005-regular").getAttribute("d");
    expect(grid.startsWith("M 275 ")).toBe(true);
    // ...then T005 is promoted INTO production in 2020, which is production's first
    // plotted data year. Even though it's mid-line (a continuation), that point snaps
    // to the year start (left edge): idx 7 → 7/20*1000 = 350. This is the promoted case.
    // It is the SECOND vertex (first "L"), since T005 returns to amador in 2022 after.
    expect(grid.split("L")[1].trim().startsWith("350 ")).toBe(true);

    // T001's last season is 2026 (familyMaxYear) → ends at the year-end (right edge
    // of the column): idx 13 → (13+1)/20*1000 = 700. The final "L" segment is at 700.
    const sunday = screen.getByTestId("world-team-track-T001-regular").getAttribute("d");
    expect(sunday.split("L").pop().trim().startsWith("700 ")).toBe(true);
  });

  it("keeps a continuous line when a team reaches Production for one year", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const regularPath = screen.getByTestId("world-team-track-T005-regular").getAttribute("d");

    expect(regularPath.match(/[ML]/g)).toHaveLength(3);
  });

  it("marks the years before each category exists inside the moving grid", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const productionPreStart = screen.getByTestId("world-team-pre-start-production_mazda");
    const rookiePreStart = screen.getByTestId("world-team-pre-start-mazda_rookie");
    const productionDivider = screen.getByTestId("world-team-start-divider-production_mazda");
    const rookieDivider = screen.getByTestId("world-team-start-divider-mazda_rookie");

    expect(productionPreStart).toBeInTheDocument();
    // years[0]=2013, N=20. pre-start zones begin at the familyMin(2016) year-start
    // (left edge = (2016-2013)/20 = 15%) and span up to each band's own start (also
    // left-edge): production (2018) → (2018-2016)/20=10%, rookie (2020) → (2020-2016)/20=20%.
    expect(productionPreStart.style.left).toBe("15%");
    expect(productionPreStart.style.width).toBe("10%");
    expect(rookiePreStart.style.left).toBe("15%");
    expect(rookiePreStart.style.width).toBe("20%");
    // dividers mark the column boundary (left-edge anchor) and coincide exactly with
    // each pre-start zone's right edge: (2018-2013)/20=25%, (2020-2013)/20=35%.
    expect(productionDivider.style.left).toBe("25%");
    expect(rookieDivider.style.left).toBe("35%");
  });

  it("marks teams that move between real divisions", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const promotion = screen.getByTestId("world-team-promotion-T004-2020");
    const demotion = screen.getByTestId("world-team-demotion-T004-2022");

    expect(promotion).toBeInTheDocument();
    expect(demotion).toBeInTheDocument();
    expect(promotion.querySelector("circle")).not.toBeInTheDocument();
    expect(demotion.querySelector("circle")).not.toBeInTheDocument();
    expect(promotion).toHaveAttribute("data-band-key", "mazda_amador");
    expect(demotion).toHaveAttribute("data-band-key", "production_mazda");
    expect(promotion.querySelector("polyline")).toHaveAttribute("stroke-width", "1.1");
    expect(demotion.querySelector("polyline")).toHaveAttribute("stroke-width", "1.1");
  });

  it("dims movement markers when another team is emphasized", async () => {
    render(<GlobalTeamsTab selectedTeamId="T001" onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    expect(screen.getByTestId("world-team-promotion-T004-2020")).toHaveAttribute("opacity", "0.15");
    expect(screen.getByTestId("world-team-demotion-T004-2022")).toHaveAttribute("opacity", "0.15");
  });

  it("starts in the selected team's family and emphasizes its line", async () => {
    render(
      <GlobalTeamsTab
        selectedTeamId="T001"
        selectedTeamCategory="gt3"
        selectedTeamClassName="gt3"
        onBack={vi.fn()}
      />,
    );

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    expect(invoke).toHaveBeenCalledWith("get_global_team_history", {
      careerId: "career-1",
      family: "gt3",
      startYear: 2000,
      windowSize: 32,
    });
    expect(screen.getByTestId("world-team-track-T001-regular")).toHaveAttribute("stroke-width", "4");
    expect(screen.getByTestId("world-team-track-T002-regular")).toHaveAttribute("opacity", "0.15");
  });

  it("reloads the payload when the family filter changes and never refetches while dragging the locked window", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    invoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: /Toyota/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("get_global_team_history", {
        careerId: "career-1",
        family: "toyota",
        startYear: 2000,
        windowSize: 32,
      }),
    );

    invoke.mockClear();
    expect(screen.queryByRole("button", { name: /Voltar janela/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Avancar janela/i })).not.toBeInTheDocument();
    const slider = screen.getByRole("slider", { name: /^Mover janela historica$/i });
    const rail = screen.getByTestId("world-team-window-scrubber");
    rail.getBoundingClientRect = () => ({
      left: 0,
      right: 260,
      width: 260,
      top: 0,
      bottom: 20,
      height: 20,
      x: 0,
      y: 0,
      toJSON: () => {},
    });

    // The window is LOCKED: scrollMin === latestStart === axisStart, so the navigable
    // range has zero span. Dragging anywhere resolves to the same single start year,
    // so the grid never offsets (transform stays at the locked 0%) and the visible
    // window label is unchanged. Crucially, no scrubbing ever triggers a refetch.
    fireEvent.pointerDown(slider, { clientX: 236, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 189, pointerId: 1 });
    expect(screen.getByTestId("world-team-moving-grid").style.transform).toBe("translate3d(0%, 0, 0)");
    expect(invoke).not.toHaveBeenCalled();
    fireEvent.pointerUp(window, { clientX: 189, pointerId: 1 });

    expect(invoke).not.toHaveBeenCalled();
    // Window stays pinned to [axisStart(2013), familyMax(2026)] regardless of the drag.
    expect(await screen.findAllByText(/Janela visivel: 2013-2026/i)).toHaveLength(2);
  });

  it("keeps lateral team rows in fixed non-overlapping slots even when positions match", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const sunday = screen.getByTestId("world-team-row-T001-mazda_amador");
    const dual = screen.getByTestId("world-team-row-T002-mazda_amador");

    expect(sunday.style.top).not.toEqual(dual.style.top);
  });

  it("orders the lateral team rows by their final-year championship position", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // The window is locked, so dragging the scrubber resolves to the same start year
    // and the lateral table keeps showing the 2026 reference standings. Rows are placed
    // by that year's position: T001 (P1) sits above T002 (P2).
    const slider = screen.getByRole("slider", { name: /^Mover janela historica$/i });
    const rail = screen.getByTestId("world-team-window-scrubber");
    rail.getBoundingClientRect = () => ({
      left: 0,
      right: 260,
      width: 260,
      top: 0,
      bottom: 20,
      height: 20,
      x: 0,
      y: 0,
      toJSON: () => {},
    });

    fireEvent.pointerDown(slider, { clientX: 260, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 0, pointerId: 1 });

    const sunday = screen.getByTestId("world-team-row-T001-mazda_amador");
    const dual = screen.getByTestId("world-team-row-T002-mazda_amador");

    expect(parseFloat(sunday.style.top)).toBeLessThan(parseFloat(dual.style.top));
    expect(within(sunday).getByText("1")).toBeInTheDocument();
    expect(within(dual).getByText("2")).toBeInTheDocument();
  });

  it("exposes a second (bottom) scrubber that also leaves the locked window unchanged", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const slider = screen.getByRole("slider", { name: /Mover janela historica inferior/i });
    const rail = screen.getByTestId("world-team-window-scrubber-bottom");
    rail.getBoundingClientRect = () => ({
      left: 0,
      right: 260,
      width: 260,
      top: 0,
      bottom: 20,
      height: 20,
      x: 0,
      y: 0,
      toJSON: () => {},
    });

    // The bottom scrubber is a real, independent control (its own slider + rail), but
    // because the window is locked it cannot pan the atlas either: the grid stays at
    // the 0% offset and both window labels remain 2013-2026 after dragging.
    fireEvent.pointerDown(slider, { clientX: 236, pointerId: 7 });
    fireEvent.pointerMove(window, { clientX: 189, pointerId: 7 });

    expect(screen.getByTestId("world-team-moving-grid").style.transform).toBe("translate3d(0%, 0, 0)");

    fireEvent.pointerUp(window, { clientX: 189, pointerId: 7 });

    expect(await screen.findAllByText(/Janela visivel: 2013-2026/i)).toHaveLength(2);
  });

  it("shows trophy icons for teams with titles and nothing for teams without", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // T002 (Dual Exit Racing) has titles: [{ band_key: "mazda_amador", count: 3 }]
    // trophy image should render; count > 1 so the count badge is shown
    const dualRow = screen.getByTestId("world-team-row-T002-mazda_amador");
    expect(within(dualRow).getByAltText("Mazda Championship")).toBeInTheDocument();
    expect(within(dualRow).getByTestId("team-trophy-count")).toHaveTextContent("3");

    // T003 (Porsche Black) has titles: [] → no trophy images at all
    const porscheRow = screen.getByTestId("world-team-row-T003-mazda_amador");
    expect(within(porscheRow).queryByAltText("Mazda Championship")).not.toBeInTheDocument();
  });

  it("shows a star for the reigning champion and not for others", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // T002 is is_reigning_champion: true in mazda_amador
    const dualRow = screen.getByTestId("world-team-row-T002-mazda_amador");
    expect(within(dualRow).getByTitle("Campeão vigente")).toBeInTheDocument();

    // T001 is is_reigning_champion: false in mazda_amador
    const sundayRow = screen.getByTestId("world-team-row-T001-mazda_amador");
    expect(within(sundayRow).queryByTitle("Campeão vigente")).not.toBeInTheDocument();
  });

  it("does not show a count badge when a team won only once (count == 1)", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // T006 (Roadster Touring) has titles: [{ band_key: "mazda_rookie", count: 1 }]
    // Trophy image renders but the count badge must NOT appear (count ≤ 1 is omitted)
    const rookieRow = screen.getByTestId("world-team-row-T006-mazda_rookie");
    expect(within(rookieRow).getByAltText("Mazda Rookie")).toBeInTheDocument();
    expect(within(rookieRow).queryByTestId("team-trophy-count")).not.toBeInTheDocument();
  });

  it("opening viewport fills the exact span of the family's recorded data", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // familyMinYear=2016, familyMaxYear=2026 → windowSize=14 (locked, full range)
    // visibleStart=axisStart=2013, visibleEnd=familyMax=2026; rendered axis=[2013..2032]
    expect(screen.getAllByText(/Janela visivel: 2013-2026/i)).toHaveLength(2);
    // The navigable axis end (2026) labels both scrubbers (right end) + the year header.
    expect(screen.getAllByText("2026")).toHaveLength(3);
    // 2027 is a render-only column past the navigable end (under the right table); it is
    // NOT a scrubber endpoint, so it appears only once — in the year header.
    expect(screen.getAllByText("2027")).toHaveLength(1);
    // full rendered column range exists in the DOM: [axisStart 2013 .. renderEnd 2032]
    expect(screen.getByTestId("world-team-year-2013")).toBeInTheDocument();
    expect(screen.getByTestId("world-team-year-2032")).toBeInTheDocument();
    expect(screen.queryByTestId("world-team-year-2012")).not.toBeInTheDocument();
    expect(screen.queryByTestId("world-team-year-2033")).not.toBeInTheDocument();
  });

  it("hatch boundaries align with the centre anchor of the first and last data year columns", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // axis = [2013..2032] (20 years); familyMinYear=2016 (idx 3); familyMaxYear=2026 (idx 13)
    // left hatch right edge  = 3 / 20 * 100 = 15%   — the year-START (left edge) of 2016,
    //                                                 where the start divider and lines begin
    // right hatch left edge  = (13 + 1) / 20 * 100 = 70%  — boundary AFTER the last season
    const leftHatch = screen.getByTestId("world-team-axis-hatch-left");
    const rightHatch = screen.getByTestId("world-team-axis-hatch-right");

    expect(leftHatch.style.width).toBe("15%");
    expect(rightHatch.style.left).toBe("70%");

    // Both edges sit on a year boundary: the left frame ends at the start of 2016 and
    // the right "no championship" hatch starts at the end of 2026 (2026→2027 boundary),
    // so categories read as full years instead of being cut at a mid-column point.
  });

  it("per-band pre-start zones sit to the right of the global no-data frame (no double-yellow overlap)", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // mazda_amador starts at familyMinYear(2016) → it exists from the first data
    // year, so it has NO "did not exist yet" zone.
    expect(screen.queryByTestId("world-team-pre-start-mazda_amador")).not.toBeInTheDocument();

    // The global yellow frame ends at the familyMin(2016) year-start (left edge = 15%);
    // production_mazda (2018) and mazda_rookie (2020) zones begin EXACTLY there and run
    // to each band's start divider, so frame → grey → divider sit flush with no overlap
    // and no stray border line.
    const leftHatch = screen.getByTestId("world-team-axis-hatch-left");
    expect(leftHatch.style.width).toBe("15%");

    const prodBandPreStart = screen.getByTestId("world-team-pre-start-production_mazda");
    expect(prodBandPreStart.style.left).toBe("15%");
    expect(prodBandPreStart.style.width).toBe("10%");

    const rookBandPreStart = screen.getByTestId("world-team-pre-start-mazda_rookie");
    expect(rookBandPreStart.style.left).toBe("15%");
    expect(rookBandPreStart.style.width).toBe("20%");
  });

  it("renders hatched edge zones inside the moving grid for years outside family data", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    // left hatch: right edge at the year-start (left edge) of familyMinYear(2016) = 3/20*100 = 15%
    const leftHatch = screen.getByTestId("world-team-axis-hatch-left");
    expect(leftHatch).toBeInTheDocument();
    expect(leftHatch.style.width).toBe("15%");

    // right hatch: left edge at the right boundary of familyMaxYear(2026) column = (13+1)/20*100 = 70%
    const rightHatch = screen.getByTestId("world-team-axis-hatch-right");
    expect(rightHatch).toBeInTheDocument();
    expect(rightHatch.style.left).toBe("70%");
  });

  it("clamps the window so it cannot be dragged into the empty pre-data past", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const slider = screen.getByRole("slider", { name: /^Mover janela historica$/i });
    const rail = screen.getByTestId("world-team-window-scrubber");
    rail.getBoundingClientRect = () => ({
      left: 0,
      right: 260,
      width: 260,
      top: 0,
      bottom: 20,
      height: 20,
      x: 0,
      y: 0,
      toJSON: () => {},
    });

    // Drag fully left: the locked window stays pinned to [axisStart(2013), familyMax(2026)].
    // The window never slides into the empty future render columns (2027-2032) either,
    // which stay parked as numbered gutter columns under the right table.
    fireEvent.pointerDown(slider, { clientX: 260, pointerId: 3 });
    fireEvent.pointerMove(window, { clientX: 0, pointerId: 3 });
    fireEvent.pointerUp(window, { clientX: 0, pointerId: 3 });

    expect(await screen.findAllByText(/Janela visivel: 2013-2026/i)).toHaveLength(2);
    // The render-only future year 2027 is still drawn as a gutter column...
    expect(screen.getByTestId("world-team-year-2027")).toBeInTheDocument();
    // ...but it is not a navigable scrubber endpoint, so it shows only in the header.
    expect(screen.getAllByText("2027")).toHaveLength(1);
    // The scrubber's left end is the navigable minimum, axisStart(2013).
    expect(screen.getAllByText("2013").length).toBeGreaterThanOrEqual(2);
  });
});
