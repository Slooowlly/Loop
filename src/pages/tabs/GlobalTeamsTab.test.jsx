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

const payload = {
  selected_family: "mazda",
  min_year: 2000,
  max_year: 2025,
  window_start: 2000,
  window_end: 2025,
  window_size: 26,
  families: [
    {
      id: "mazda",
      label: "Mazda",
      bands: [
        { key: "production_mazda", label: "Production", category: "production_challenger", class_name: "mazda", starts_year: 2018, is_special: true },
        { key: "mazda_amador", label: "Mazda Championship", category: "mazda_amador", class_name: null, starts_year: 2016, is_special: false },
        { key: "mazda_rookie", label: "Mazda Rookie", category: "mazda_rookie", class_name: null, starts_year: 2020, is_special: false },
      ],
    },
    {
      id: "toyota",
      label: "Toyota",
      bands: [],
    },
  ],
  bands: [
    {
      key: "production_mazda",
      label: "Production",
      category: "production_challenger",
      class_name: "mazda",
      starts_year: 2018,
      is_special: true,
      rows: [
        {
          team_id: "T001",
          nome: "Sunday Speed Club",
          nome_curto: "SSC",
          cor_primaria: "#5ee7a8",
          cor_secundaria: "#114b5f",
          base_position: 1,
          delta: 0,
          points: [
            { year: 2022, slot: "special", position: 2, points: 92, wins: 2, titles: 0 },
            { year: 2023, slot: "special", position: 1, points: 108, wins: 3, titles: 1 },
          ],
        },
        {
          team_id: "T004",
          nome: "Switchback Racing",
          nome_curto: "SBR",
          cor_primaria: "#58a6ff",
          cor_secundaria: "#0b2545",
          base_position: 1,
          delta: 0,
          points: [
            { year: 2021, slot: "special", position: 1, points: 102, wins: 4, titles: 1 },
            { year: 2022, slot: "special", position: 2, points: 92, wins: 1, titles: 0 },
          ],
        },
        {
          team_id: "T005",
          nome: "Grid Start Racing",
          nome_curto: "GSR",
          cor_primaria: "#f2c46d",
          cor_secundaria: "#3a2610",
          base_position: 5,
          delta: 0,
          points: [
            { year: 2019, slot: "special", position: 5, points: 55, wins: 0, titles: 0 },
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
          delta: 2,
          points: [
            { year: 2016, slot: "regular", position: 1, points: 91, wins: 2, titles: 0 },
            { year: 2007, slot: "regular", position: 1, points: 88, wins: 2, titles: 0 },
            { year: 2020, slot: "regular", position: 1, points: 104, wins: 4, titles: 1 },
            { year: 2021, slot: "regular", position: 2, points: 96, wins: 2, titles: 0 },
            { year: 2022, slot: "regular", position: 1, points: 110, wins: 3, titles: 1 },
            { year: 2023, slot: "regular", position: 1, points: 118, wins: 4, titles: 1 },
          ],
        },
        {
          team_id: "T002",
          nome: "Dual Exit Racing",
          nome_curto: "DXR",
          cor_primaria: "#ff6b6b",
          cor_secundaria: "#70141d",
          base_position: 1,
          delta: 0,
          points: [
            { year: 2016, slot: "regular", position: 2, points: 87, wins: 1, titles: 0 },
            { year: 2007, slot: "regular", position: 2, points: 82, wins: 1, titles: 0 },
            { year: 2020, slot: "regular", position: 1, points: 112, wins: 4, titles: 1 },
            { year: 2021, slot: "regular", position: 1, points: 120, wins: 5, titles: 1 },
          ],
        },
        {
          team_id: "T003",
          nome: "Porsche Black",
          nome_curto: "PBK",
          cor_primaria: "#050505",
          cor_secundaria: "#111111",
          base_position: 3,
          delta: 0,
          points: [
            { year: 2016, slot: "regular", position: 3, points: 74, wins: 0, titles: 0 },
            { year: 2020, slot: "regular", position: 3, points: 76, wins: 0, titles: 0 },
          ],
        },
        {
          team_id: "T004",
          nome: "Switchback Racing",
          nome_curto: "SBR",
          cor_primaria: "#58a6ff",
          cor_secundaria: "#0b2545",
          base_position: 4,
          delta: -1,
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
          delta: 0,
          points: [
            { year: 2018, slot: "regular", position: 4, points: 69, wins: 0, titles: 0 },
            { year: 2019, slot: "regular", position: 3, points: 78, wins: 1, titles: 0 },
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
          delta: 0,
          points: [
            { year: 2020, slot: "regular", position: 1, points: 93, wins: 3, titles: 1 },
            { year: 2021, slot: "regular", position: 2, points: 81, wins: 1, titles: 0 },
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
    expect(screen.getByText(/Mazda: janela 2016-2025/i)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("get_global_team_history", {
      careerId: "career-1",
      family: "mazda",
      startYear: 2000,
      windowSize: 32,
    });

    expect(screen.getByRole("button", { name: /Mazda/i })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: /Toyota/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /LMP2/i })).not.toBeInTheDocument();

    const year2020 = screen.getByTestId("world-team-year-2020");
    expect(within(year2020).getByText("2020")).toBeInTheDocument();
    expect(screen.queryByText("REG")).not.toBeInTheDocument();
    expect(screen.queryByText("ESP")).not.toBeInTheDocument();
    expect(screen.getByTestId("world-team-year-2019")).toBeInTheDocument();

    expect(screen.getAllByText("Sunday Speed Club")[0]).toHaveStyle({ color: "#5ee7a8" });
    expect(screen.getByText("Porsche Black")).not.toHaveStyle({ color: "#050505" });
    expect(screen.getAllByTestId("world-team-logo").some((logo) => logo.textContent === "Sunday Speed Club logo")).toBe(true);
    expect(screen.queryByTestId("team-color-swatch")).not.toBeInTheDocument();
    expect(screen.getByTestId("world-team-grid")).toBeInTheDocument();
    expect(screen.getByTestId("world-team-track-T001-special")).toHaveAttribute("vector-effect", "non-scaling-stroke");
    expect(screen.getByTestId("world-team-track-T001-regular")).toHaveAttribute("vector-effect", "non-scaling-stroke");
    expect(screen.getByTestId("world-team-track-T003-regular")).not.toHaveAttribute("stroke", "#050505");
    expect(screen.getByTestId("world-team-moving-grid").style.width).toBe("260%");
    expect(screen.queryByTestId("world-team-path-T001-mazda_amador")).not.toBeInTheDocument();
  });

  it("shows only teams that exist in the first visible year on the lateral rail", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    expect(screen.getByTestId("world-team-row-T001-mazda_amador")).toBeInTheDocument();
    expect(screen.getByTestId("world-team-row-T002-mazda_amador")).toBeInTheDocument();
    expect(screen.getByTestId("world-team-row-T003-mazda_amador")).toBeInTheDocument();
    expect(screen.queryByTestId("world-team-row-T004-mazda_amador")).not.toBeInTheDocument();
    expect(screen.queryByTestId("world-team-row-T004-production_mazda")).not.toBeInTheDocument();
  });

  it("labels teams that enter after the lateral reference year inside the grid", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    expect(screen.queryByTestId("world-team-row-T005-mazda_amador")).not.toBeInTheDocument();
    const entryLabel = screen.getByTestId("world-team-entry-label-T005-regular-2018");
    expect(entryLabel).toHaveTextContent("Grid Start Racing");
    expect(entryLabel.tagName).toBe("DIV");
    expect(entryLabel.style.left).toBe("70.1538%");
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

  it("draws separate regular and special lines when a team has both histories", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const regularPath = screen.getByTestId("world-team-track-T001-regular").getAttribute("d");
    const specialPath = screen.getByTestId("world-team-track-T001-special").getAttribute("d");

    expect(regularPath.match(/[ML]/g)).toHaveLength(6);
    expect(specialPath.match(/[ML]/g)).toHaveLength(2);
  });

  it("draws a visible dash when a team only appears in the special category for one year", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const specialPath = screen.getByTestId("world-team-track-T005-special").getAttribute("d");

    expect(specialPath.match(/[ML]/g)).toHaveLength(2);
  });

  it("marks the years before each category exists inside the moving grid", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const productionPreStart = screen.getByTestId("world-team-pre-start-production_mazda");
    const rookiePreStart = screen.getByTestId("world-team-pre-start-mazda_rookie");
    const productionDivider = screen.getByTestId("world-team-start-divider-production_mazda");
    const rookieDivider = screen.getByTestId("world-team-start-divider-mazda_rookie");

    expect(productionPreStart).toBeInTheDocument();
    expect(productionPreStart.style.width).toBe("72.1538%");
    expect(rookiePreStart.style.width).toBe("77.8462%");
    expect(productionDivider.style.left).toBe("72.1538%");
    expect(rookieDivider.style.left).toBe("77.8462%");
  });

  it("marks teams that enter and leave a special category", async () => {
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
    expect(screen.getByTestId("world-team-track-T001-special")).toHaveAttribute("stroke-width", "5");
    expect(screen.getByTestId("world-team-track-T002-regular")).toHaveAttribute("opacity", "0.15");
  });

  it("reloads the payload when the family filter changes and commits the year pill only after release", async () => {
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

    fireEvent.pointerDown(slider, { clientX: 236, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 189, pointerId: 1 });
    expect(screen.getByTestId("world-team-moving-grid").style.transform).toContain("translate3d");
    expect(screen.getByTestId("world-team-moving-grid").style.transform).not.toBe("translate3d(0%, 0, 0)");
    expect(invoke).not.toHaveBeenCalled();
    fireEvent.pointerUp(window, { clientX: 189, pointerId: 1 });

    expect(invoke).not.toHaveBeenCalled();
    expect(await screen.findAllByText(/Janela visivel: 2012-2021/i)).toHaveLength(2);
  });

  it("keeps lateral team rows in fixed non-overlapping slots even when positions match", async () => {
    render(<GlobalTeamsTab onBack={vi.fn()} />);

    await screen.findByRole("heading", { name: /^Hist.rico mundial de equipes$/i });

    const sunday = screen.getByTestId("world-team-row-T001-mazda_amador");
    const dual = screen.getByTestId("world-team-row-T002-mazda_amador");

    expect(sunday.style.top).not.toEqual(dual.style.top);
  });

  it("reorders the lateral team list from the first visible year while dragging", async () => {
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

    fireEvent.pointerDown(slider, { clientX: 260, pointerId: 1 });
    fireEvent.pointerMove(window, { clientX: 114, pointerId: 1 });

    const sunday = screen.getByTestId("world-team-row-T001-mazda_amador");
    const dual = screen.getByTestId("world-team-row-T002-mazda_amador");

    expect(parseFloat(sunday.style.top)).toBeLessThan(parseFloat(dual.style.top));
    expect(within(sunday).getByText("1")).toBeInTheDocument();
    expect(within(dual).getByText("2")).toBeInTheDocument();
  });

  it("keeps a lower year pill available for moving the atlas from the bottom", async () => {
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

    fireEvent.pointerDown(slider, { clientX: 236, pointerId: 7 });
    fireEvent.pointerMove(window, { clientX: 189, pointerId: 7 });

    expect(screen.getByTestId("world-team-moving-grid").style.transform).toContain("translate3d");
    expect(screen.getByTestId("world-team-moving-grid").style.transform).not.toBe("translate3d(0%, 0, 0)");

    fireEvent.pointerUp(window, { clientX: 189, pointerId: 7 });

    expect(await screen.findAllByText(/Janela visivel: 2012-2021/i)).toHaveLength(2);
  });
});
