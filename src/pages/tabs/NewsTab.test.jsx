import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";

import NewsTab from "./NewsTab";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args) => mockInvoke(...args),
}));

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) =>
    selector({
      careerId: "career_001",
      season: {
        numero: 1,
        rodada_atual: 2,
      },
    }),
}));

function buildBootstrap() {
  return {
    default_scope_type: "category",
    default_scope_id: "mazda_rookie",
    default_primary_filter: null,
    scopes: [
      {
        id: "mazda_rookie",
        label: "Mazda MX-5 Rookie Cup",
        short_label: "Mazda MX-5",
        scope_type: "category",
        special: false,
      },
      {
        id: "toyota_rookie",
        label: "Toyota GR86 Rookie Cup",
        short_label: "Toyota Rookie",
        scope_type: "category",
        special: false,
      },
      {
        id: "mazda_amador",
        label: "Mazda MX-5 Championship",
        short_label: "Mazda Championship",
        scope_type: "category",
        special: false,
      },
      {
        id: "toyota_amador",
        label: "Toyota GR86 Cup",
        short_label: "Toyota Cup",
        scope_type: "category",
        special: false,
      },
      {
        id: "bmw_m2",
        label: "BMW M2 CS Racing",
        short_label: "BMW M2",
        scope_type: "category",
        special: false,
      },
      {
        id: "production_challenger",
        label: "Production Car Challenger",
        short_label: "Production",
        scope_type: "category",
        special: false,
      },
      {
        id: "gt4",
        label: "GT4 Series",
        short_label: "GT4",
        scope_type: "category",
        special: false,
      },
      {
        id: "gt3",
        label: "GT3 Championship",
        short_label: "GT3",
        scope_type: "category",
        special: false,
      },
      {
        id: "lmp2",
        label: "LMP2 Prototype Championship",
        short_label: "LMP2",
        scope_type: "category",
        special: false,
      },
      {
        id: "endurance",
        label: "Endurance Championship",
        short_label: "Endurance",
        scope_type: "category",
        special: false,
      },
      {
        id: "mais_famosos",
        label: "Mais famosos",
        short_label: "Mais famosos",
        scope_type: "famous",
        special: true,
      },
    ],
    season_number: 1,
    season_year: 2026,
    current_round: 2,
    total_rounds: 5,
  };
}

function buildStories() {
  return [
    {
      id: "NT001",
      icon: "N",
      title: "LEGADO_OPENING_TITLE",
      headline: "Abertura forte em Okayama",
      summary: "LEGADO_OPENING_SUMMARY",
      deck: "A etapa inicial redefiniu o humor do grid.",
      body_text: "LEGADO_OPENING_BODY_TEXT",
      blocks: [
        {
          label: "Resumo",
          text: "Okayama abriu a temporada com o grid embaralhado logo na primeira volta.",
        },
        {
          label: "Impacto",
          text: "A margem entre os ponteiros caiu e o paddock saiu da etapa com leitura mais agressiva.",
        },
        {
          label: "Leitura",
          text: "A abertura indica um campeonato menos previsível do que o desenho de pr?-temporada sugeria.",
        },
      ],
      news_type: "Corrida",
      importance: "Destaque",
      importance_label: "Destaque",
      category_label: "Mazda MX-5 Rookie Cup",
      meta_label: "Corrida · R1 · T1",
      time_label: "Edicao recente",
      entity_label: "Apex Academy Racing",
      team_label: "Apex Academy Racing",
      driver_label: "Thomas Baker",
      race_label: "Okayama",
      accent_tone: "gold",
      driver_id: "P001",
      team_id: "T001",
      round: 1,
    },
    {
      id: "NT002",
      icon: "M",
      title: "LEGADO_MARKET_TITLE",
      headline: "Mercado observa pilotos em ascensao",
      summary: "LEGADO_MARKET_SUMMARY",
      deck: "O paddock comenta nomes que chegam valorizados.",
      body_text: "LEGADO_MARKET_BODY_TEXT",
      blocks: [
        {
          label: "Movimento",
          text: "O paddock reposicionou Kenji Sato no mapa das conversas de mercado desta semana.",
        },
        {
          label: "Impacto",
          text: "A valorizacao muda a leitura de vagas competitivas para a janela seguinte.",
        },
        {
          label: "Próximo passo",
          text: "O nome segue em observação enquanto as equipes cruzam desempenho recente e margem salarial.",
        },
      ],
      news_type: "Mercado",
      importance: "Alta",
      importance_label: "Alta",
      category_label: "Mazda MX-5 Rookie Cup",
      meta_label: "Mercado · T1",
      time_label: "Mais cedo",
      entity_label: "Kenji Sato",
      team_label: null,
      driver_label: "Kenji Sato",
      race_label: null,
      accent_tone: "blue",
      driver_id: "P002",
      team_id: null,
      round: null,
    },
  ];
}

function buildSnapshot(overrides = {}) {
  return {
    hero: {
      section_label: "Central de Notícias",
      title: "Panorama do Campeonato",
      subtitle: "O paddock chega aquecido para a próxima etapa da categoria.",
      badge: "Rodada 2/5",
      badge_tone: "blue",
    },
    primary_filters: [
      { id: "Corridas", label: "Corridas", meta: null, tone: "cool", kind: "tag", color_primary: null, color_secondary: null },
      { id: "Pilotos", label: "Pilotos", meta: null, tone: "cool", kind: "tag", color_primary: null, color_secondary: null },
      { id: "Equipes", label: "Equipes", meta: null, tone: "cool", kind: "tag", color_primary: null, color_secondary: null },
      { id: "Mercado", label: "Mercado", meta: null, tone: "warm", kind: "tag", color_primary: null, color_secondary: null },
    ],
    contextual_filters: [],
    stories: buildStories(),
    scope_meta: {
      scope_type: "category",
      scope_id: "mazda_rookie",
      scope_label: "Mazda MX-5 Rookie Cup",
      scope_class: null,
      primary_filter: null,
      context_type: null,
      context_id: null,
      context_label: null,
      is_special: false,
    },
    ...overrides,
  };
}

function scopeLabelFor(scopeId) {
  const labels = {
    mazda_rookie: "Mazda MX-5 Rookie Cup",
    toyota_rookie: "Toyota GR86 Rookie Cup",
    mazda_amador: "Mazda MX-5 Championship",
    toyota_amador: "Toyota GR86 Cup",
    bmw_m2: "BMW M2 CS Racing",
    production_challenger: "Production Car Challenger",
    gt4: "GT4 Series",
    gt3: "GT3 Championship",
    lmp2: "LMP2 Prototype Championship",
    endurance: "Endurance Championship",
    mais_famosos: "Mais famosos",
  };

  return labels[scopeId] ?? scopeId;
}

describe("NewsTab", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((command, payload) => {
      if (command === "get_news_tab_bootstrap") {
        return Promise.resolve(buildBootstrap());
      }

      if (command === "get_news_tab_snapshot") {
        const scopeId = payload?.request?.scope_id ?? "mazda_rookie";
        const scopeType = payload?.request?.scope_type ?? "category";
        const filter = payload?.request?.primary_filter ?? null;
        const contextualFilters =
          filter === "Corridas"
            ? [
                {
                  id: "R001",
                  label: "Okayama",
                  meta: "R1",
                  tone: "cool",
                  kind: "race",
                  color_primary: null,
                  color_secondary: null,
                },
                {
                  id: "R002",
                  label: "Laguna Seca",
                  meta: "R2",
                  tone: "cool",
                  kind: "race",
                  color_primary: null,
                  color_secondary: null,
                },
                {
                  id: "R003",
                  label: "Road Atlanta",
                  meta: "R3",
                  tone: "cool",
                  kind: "race",
                  color_primary: null,
                  color_secondary: null,
                },
              ]
            : filter === "Pilotos"
              ? [
                  {
                    id: "P001",
                    label: "Thomas Baker",
                    meta: "120 pts",
                    tone: "cool",
                    kind: "driver",
                    color_primary: null,
                    color_secondary: null,
                  },
                  {
                    id: "P002",
                    label: "Kenji Sato",
                    meta: "108 pts",
                    tone: "cool",
                    kind: "driver",
                    color_primary: null,
                    color_secondary: null,
                  },
                  {
                    id: "P003",
                    label: "Mia Torres",
                    meta: "96 pts",
                    tone: "cool",
                    kind: "driver",
                    color_primary: null,
                    color_secondary: null,
                  },
                ]
              : [];

        return Promise.resolve(
          buildSnapshot({
            contextual_filters: contextualFilters,
            scope_meta: {
              scope_type: scopeType,
              scope_id: scopeId,
              scope_label: scopeLabelFor(scopeId),
              scope_class: payload?.request?.scope_class ?? null,
              primary_filter: filter,
              context_type: null,
              context_id: null,
              context_label: null,
              is_special: scopeType === "famous",
            },
          }),
        );
      }

      return Promise.reject(new Error(`unexpected command: ${command}`));
    });
  });

  it("loads the snapshot with no primary filter active by default", async () => {
    const { container } = render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });

    expect(screen.getByRole("heading", { name: "Panorama do Campeonato" })).toBeInTheDocument();
    expect(screen.getByText(/Próxima Etapa/i)).toBeInTheDocument();
    expect(screen.queryByText("Mazda MX-5 Rookie Cup")).not.toBeInTheDocument();
    expect(container.querySelector('[data-news-section="hero"] > div')).toHaveClass("rounded-[32px]");
    expect(container.querySelector('[data-news-section="hero"] > div > div.relative.z-10')).toHaveClass("px-6", "pt-10");
    await waitFor(() => {
      expect(container.querySelector('[data-news-section="main-reader"]')).not.toBeNull();
    });
    expect(screen.queryByRole("button", { name: "Expectativas" })).not.toBeInTheDocument();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          primary_filter: null,
        }),
      });
    });
  });

  it("toggles the same primary filter on and off", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });

    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);
    await screen.findByRole("button", { name: "Corridas" });

    mockInvoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Corridas" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          primary_filter: "Corridas",
        }),
      });
    });

    mockInvoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Corridas" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          primary_filter: null,
        }),
      });
    });
  });

  it("renders the context controls inline and dims future races", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });
    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Corridas" }));

    await screen.findByRole("button", { name: "Okayama" });

    const futureRaceButton = screen.getByRole("button", { name: "Laguna Seca" });
    const inlineDrawer = screen.getByRole("button", { name: "Rookie" }).closest("div.rounded-2xl");
    const activePrimaryFilter = screen.getByRole("button", { name: "Corridas" });

    expect(screen.queryByText("Filtro contextual")).not.toBeInTheDocument();
    expect(screen.queryByText("Leitura da Temporada")).not.toBeInTheDocument();
    expect(screen.queryByText("Recorte em Foco")).not.toBeInTheDocument();
    expect(inlineDrawer).not.toBeNull();
    expect(activePrimaryFilter.className).toContain("bg-accent-primary/10");
    expect(screen.getByRole("button", { name: "Rookie" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cup" })).toBeInTheDocument();
    expect(futureRaceButton.className).toContain("opacity-35");
    expect(futureRaceButton).toBeDisabled();
  });

  it("uses medal tones for the top three driver filters", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });
    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Pilotos" }));

    const firstPlace = await screen.findByRole("button", { name: "Thomas Baker" });
    const secondPlace = screen.getByRole("button", { name: "Kenji Sato" });
    const thirdPlace = screen.getByRole("button", { name: "Mia Torres" });

    expect(firstPlace.className).toContain("border-podium-gold/30");
    expect(firstPlace.className).toContain("text-podium-gold");
    expect(secondPlace.className).toContain("border-podium-silver/30");
    expect(secondPlace.className).toContain("text-podium-silver");
    expect(thirdPlace.className).toContain("border-podium-bronze/30");
    expect(thirdPlace.className).toContain("text-podium-bronze");
  });

  it("switches the open story locally when a list item is clicked", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });
    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);
    await screen.findByText("Okayama abriu a temporada com o grid embaralhado logo na primeira volta.");

    const mainReader = document.querySelector('[data-news-section="main-reader"]');
    const openStory = within(mainReader).getByRole("heading", {
      level: 2,
      name: "Abertura forte em Okayama",
    }).closest("div.space-y-4");

    expect(openStory).not.toBeNull();
    expect(within(openStory).getByText("Abertura forte em Okayama")).toBeInTheDocument();
    expect(within(openStory).getByText("Resumo")).toBeInTheDocument();
    expect(within(openStory).getByText("Impacto")).toBeInTheDocument();
    expect(within(openStory).getAllByText("Leitura").length).toBeGreaterThan(0);
    expect(screen.getAllByText("A etapa inicial redefiniu o humor do grid.").length).toBeGreaterThan(0);
    expect(screen.queryByText("LEGADO_OPENING_TITLE")).not.toBeInTheDocument();
    expect(screen.queryByText("LEGADO_OPENING_SUMMARY")).not.toBeInTheDocument();
    expect(screen.queryByText("LEGADO_OPENING_BODY_TEXT")).not.toBeInTheDocument();

    mockInvoke.mockClear();
    fireEvent.click(screen.getByText(/Mercado observa pilotos em ascensao/i));

    expect(screen.getAllByText("O paddock comenta nomes que chegam valorizados.").length).toBeGreaterThan(0);
    expect(screen.getByText("Movimento")).toBeInTheDocument();
    expect(screen.getByText("Próximo passo")).toBeInTheDocument();
    expect(screen.getByText("O nome segue em observação enquanto as equipes cruzam desempenho recente e margem salarial.")).toBeInTheDocument();
    expect(screen.queryByText("LEGADO_MARKET_TITLE")).not.toBeInTheDocument();
    expect(screen.queryByText("LEGADO_MARKET_SUMMARY")).not.toBeInTheDocument();
    expect(screen.queryByText("LEGADO_MARKET_BODY_TEXT")).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("expands a family and applies the default scope automatically", async () => {
    const { container } = render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });

    expect(screen.getAllByRole("button", { name: "Mazda" })[0]).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Toyota" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "BMW" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "GT4" })).toBeInTheDocument();

    mockInvoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Toyota" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          scope_id: "toyota_rookie",
        }),
      });
    });

    expect(screen.getByRole("button", { name: "Cup" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Corridas" })).toBeInTheDocument();
    expect(container.querySelector('[data-news-section="main-reader"]')).not.toBeNull();
  });

  it("shows inline family scope controls for the active family", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "GT3" });
    fireEvent.click(screen.getAllByRole("button", { name: "GT3" })[0]);

    expect(await screen.findByRole("button", { name: "Endurance" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rookie" })).toBeInTheDocument();
  });

  it("switches the inline family scope controls when the active family changes", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "GT3" });

    fireEvent.click(screen.getAllByRole("button", { name: "GT3" })[0]);
    expect(await screen.findByRole("button", { name: "Endurance" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Toyota" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Cup" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Production" })).toBeInTheDocument();
    });
  });

  it("uses the simplified scope labels and rankings naming in the family drawer", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });
    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);

    const familyStrip = screen.getAllByRole("button", { name: "Mazda" })[0].parentElement;
    const inlineDrawer = screen.getByRole("button", { name: "Rookie" }).closest("div.rounded-2xl");

    expect(screen.getByRole("button", { name: "Rankings" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Mais famosos" })).not.toBeInTheDocument();
    expect(screen.queryByText("Categoria")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Mazda Rookie" })).not.toBeInTheDocument();
    expect(screen.queryByText("Mazda Championship")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rookie" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cup" })).toBeInTheDocument();
    expect(screen.getByText("Production")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Mazda" })[0].className).toContain("font-bold");
    expect(screen.getAllByRole("button", { name: "Mazda" })[0].className).toContain("rounded-full");
    expect(screen.getAllByRole("button", { name: "Mazda" })[0].className).toContain("px-5");
    expect(screen.getAllByRole("button", { name: "Mazda" })[0].className).toContain("py-2");
    expect(screen.getByRole("button", { name: "Rankings" }).className).toContain("rounded-full");
    expect(familyStrip).not.toBeNull();
    expect(familyStrip.className).toContain("inline-flex");
    expect(familyStrip.className).toContain("rounded-full");
    expect(inlineDrawer).not.toBeNull();
    expect(inlineDrawer.className).toContain("rounded-2xl");
    expect(inlineDrawer.className).toContain("bg-black/40");
  });

  it("uses rookie as the first drawer step for the non-mazda families", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Toyota" });
    fireEvent.click(screen.getByRole("button", { name: "Toyota" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Rookie" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Cup" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "BMW" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Rookie" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Production" })).toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: "BMW M2" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "GT4" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Rookie" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Endurance" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "GT3" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Rookie" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Endurance" })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "LMP2" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Prototype" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Endurance" })).not.toBeInTheDocument();
    });
  });

  it("refines the scope when a championship inside the family is clicked", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });
    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);
    await screen.findByRole("button", { name: "Cup" });

    mockInvoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Cup" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          scope_id: "mazda_amador",
        }),
      });
    });
  });

  it("keeps the active primary filter when changing the category scope", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });
    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Corridas" }));
    await screen.findByRole("button", { name: "Okayama" });

    mockInvoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Cup" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          scope_id: "mazda_amador",
          primary_filter: "Corridas",
        }),
      });
    });

    expect(screen.getByRole("button", { name: "Corridas" })).toBeInTheDocument();
  });

  it("sends the class filter when a shared championship is selected inside a family", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "Mazda" });

    mockInvoke.mockClear();
    fireEvent.click(screen.getAllByRole("button", { name: "Mazda" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Production" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          scope_id: "production_challenger",
          scope_class: "mazda",
        }),
      });
    });
  });

  it("sends the endurance class filter when the shared endurance scope is refined", async () => {
    render(<NewsTab />);

    await screen.findByRole("button", { name: "GT3" });

    mockInvoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "GT3" }));
    fireEvent.click(screen.getByRole("button", { name: "Endurance" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_news_tab_snapshot", {
        careerId: "career_001",
        request: expect.objectContaining({
          scope_id: "endurance",
          scope_class: "gt3",
        }),
      });
    });
  });
});
