import { render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";
import { TeamHistoryDrawerV2 } from "./TeamHistoryDrawerV2";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const EQUIPE = { id: "T1", nome: "Track Day Heroes", cor_primaria: "#37b6c9", categoria: "mazda_rookie" };

const RECORDS = [
  { id: "titles", label: "Títulos", rank: "10º", value: "0", rank_position: 10, rank_total: 19, group_average: "0,4" },
  { id: "wins", label: "Vitórias", rank: "14º", value: "0", rank_position: 14, rank_total: 19, group_average: "6,2" },
  { id: "podiums", label: "Pódios", rank: "7º", value: "21", rank_position: 7, rank_total: 19, group_average: "16,8" },
  { id: "podium_rate", label: "Taxa de pódio", rank: "7º", value: "64%", rank_position: 7, rank_total: 19, group_average: "44%" },
  { id: "win_rate", label: "Taxa de vitória", rank: "14º", value: "0%", rank_position: 14, rank_total: 19, group_average: "18%" },
];

// Payload mínimo do get_team_history_dossier com o que a faixa consome.
function dossieCom(seasonResults, extra = {}) {
  return {
    record_scope: "Grupo Mazda",
    has_history: true,
    records: RECORDS,
    season_results: seasonResults,
    sport: { seasons: "7 Temporadas" },
    ...extra,
  };
}

describe("TeamHistoryDrawerV2 — cabeçalho e records", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(dossieCom([]));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="mazda_rookie"
        activeTab="records"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onClose={() => {}}
      />,
    );
  });

  it("acende o âncora da melhor colocação da equipe", async () => {
    const drawer = await screen.findByTestId("team-history-drawer");
    // 7º em pódios é a melhor colocação; títulos (10º) e vitórias (14º) ficam apagados.
    await waitFor(() =>
      expect(drawer.querySelector("[data-anchor='podiums']")).toHaveAttribute("data-highlighted", "true"),
    );
    expect(drawer.querySelector("[data-anchor='titles']")).not.toHaveAttribute("data-highlighted");
    expect(drawer.querySelector("[data-anchor='wins']")).not.toHaveAttribute("data-highlighted");
  });

  it("separa o número da unidade no âncora de temporadas", async () => {
    const drawer = await screen.findByTestId("team-history-drawer");
    const seasons = await waitFor(() => {
      const node = drawer.querySelector("[data-anchor='seasons']");
      expect(node.textContent).toMatch(/7/);
      return node;
    });
    // O número em fonte de número, a palavra em fonte de texto — dois nós.
    expect(seasons.querySelector(".font-mono").textContent).toBe("7");
    expect(seasons.textContent).toMatch(/7\s*Temporadas/);
  });

  // Records precisa ter altura estável entre equipes: os marcos variam de zero a
  // três itens e faziam a tela pular ao navegar com as setas. Eles vivem em
  // Esportivo agora.
  it("mantém os marcos fora da seção Records", async () => {
    const drawer = await screen.findByTestId("team-history-drawer");
    await waitFor(() => expect(drawer.querySelectorAll("[data-record]")).toHaveLength(5));
    expect(screen.queryByTestId("team-history-milestones")).not.toBeInTheDocument();
  });

  it("separa contagens e taxas em duas linhas de cards", async () => {
    const drawer = await screen.findByTestId("team-history-drawer");
    await waitFor(() => expect(drawer.querySelectorAll("[data-record]")).toHaveLength(5));
    const cards = [...drawer.querySelectorAll("[data-record]")];
    expect(cards.map((card) => card.dataset.record)).toEqual([
      "titles",
      "wins",
      "podiums",
      "podium_rate",
      "win_rate",
    ]);
    // As três contagens numa linha, as duas taxas noutra.
    expect(cards[0].parentElement).not.toBe(cards[3].parentElement);
    expect(cards[0].parentElement).toBe(cards[2].parentElement);
    expect(cards[3].parentElement).toBe(cards[4].parentElement);
  });
});

describe("TeamHistoryDrawerV2 — galeria de títulos", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  function abrirGaleria(titles, seasonResults = []) {
    invoke.mockResolvedValue(dossieCom(seasonResults, { title_categories: titles }));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="mazda_rookie"
        activeTab="records"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onClose={() => {}}
      />,
    );
  }

  function titulo(year, extra = {}) {
    return {
      category: "Production",
      category_id: "production_challenger",
      year: String(year),
      color: "#58a6ff",
      points: "400",
      wins: 10,
      champion_driver: "Marcin Kowalczyk",
      champion_team: "",
      champion_is_team: true,
      ...extra,
    };
  }

  function temporada(year, extra = {}) {
    return {
      year: String(year),
      category: "Production",
      category_id: "production_challenger",
      position: "P1",
      races: 10,
      wins: 10,
      podiums: 10,
      points: "400",
      ...extra,
    };
  }

  // Seis cards repetindo a mesma categoria e a mesma frase escondiam o que era a
  // história: seis títulos SEGUIDOS. O que repete virou cabeçalho, o que varia
  // virou coluna.
  it("agrupa os títulos por categoria e resume o reinado no cabeçalho", async () => {
    abrirGaleria(
      [
        titulo(2018),
        titulo(2019, { points: "420" }),
        titulo(2022, { champion_driver: "Grant Lee", champion_team: "Isar Track", champion_is_team: false }),
      ],
      [temporada(2018), temporada(2019), temporada(2020), temporada(2021), temporada(2022)],
    );

    const grupo = await screen.findByTestId("team-history-title-group");
    // Categoria uma vez, com o resumo — não uma vez por título.
    expect(within(grupo).getAllByText("Production")).toHaveLength(1);
    expect(within(grupo).getByText(/3 títulos · 2018–2022 · 2 dobradinhas/)).toBeInTheDocument();

    // Uma linha por ano, e a coluna de pontos empilhada para comparação.
    const linhas = [...grupo.querySelectorAll("[data-title-year]")];
    expect(linhas.map((el) => el.getAttribute("data-title-year"))).toEqual([
      "2018",
      "2019",
      "2022",
    ]);
    expect(within(linhas[1]).getByText("420")).toBeInTheDocument();
    // Dobradinha é atributo da linha, e a frase não se repete: só o nome.
    expect(linhas[0]).toHaveAttribute("data-double", "true");
    expect(linhas[2]).not.toHaveAttribute("data-double");
    expect(within(linhas[2]).getByText(/Grant Lee/)).toBeInTheDocument();
    expect(within(grupo).queryByText(/Dobradinha:/)).not.toBeInTheDocument();
  });

  // A régua cobre TODAS as temporadas, não só as de título: sem os anos vazios em
  // volta, títulos seguidos desenham igual a títulos espalhados.
  it("marca na régua os anos de título e deixa os outros vazios", async () => {
    abrirGaleria(
      [titulo(2019), titulo(2020, { champion_is_team: false, champion_driver: "Ana Reis", champion_team: "Falcon" })],
      [temporada(2018), temporada(2019), temporada(2020), temporada(2021)],
    );

    const regua = await screen.findByTestId("team-history-title-rail");
    const celulas = [...regua.children];
    expect(celulas.map((el) => el.dataset.year)).toEqual(["2018", "2019", "2020", "2021"]);
    expect(celulas.map((el) => el.dataset.title)).toEqual([undefined, "true", "true", undefined]);
    // O anel dourado é só da dobradinha.
    expect(celulas[1].dataset.double).toBe("true");
    expect(celulas[2].dataset.double).toBeUndefined();
    // Ano de título ganha a cor da categoria; ano sem título fica no tom de fundo.
    expect(celulas[1].style.backgroundColor).toBe("rgb(128, 32, 208)");
    expect(celulas[0].style.backgroundColor).toBe("rgb(20, 31, 44)");
  });

  // Um título só não sustenta régua, cabeçalho e tabela — seria mais moldura que
  // conteúdo.
  it("colapsa um título único numa linha", async () => {
    abrirGaleria([titulo(2019, { points: "268", wins: 5, champion_driver: "Lucas Prado" })], [temporada(2019)]);

    const galeria = await screen.findByTestId("team-history-title-gallery");
    expect(galeria).toHaveAttribute("data-single", "true");
    expect(within(galeria).getByText("268 pts · 5 vitórias")).toBeInTheDocument();
    expect(within(galeria).getByText(/Lucas Prado/)).toBeInTheDocument();
    expect(screen.queryByTestId("team-history-title-rail")).not.toBeInTheDocument();
    expect(screen.queryByTestId("team-history-title-group")).not.toBeInTheDocument();
  });

  it("omite a linha do campeão quando o banco não sabe quem foi", async () => {
    abrirGaleria(
      [titulo(2018, { champion_driver: "", champion_is_team: false }), titulo(2019, { champion_driver: "", champion_is_team: false })],
      [temporada(2018), temporada(2019)],
    );

    const grupo = await screen.findByTestId("team-history-title-group");
    const linhas = [...grupo.querySelectorAll("[data-title-year]")];
    expect(linhas).toHaveLength(2);
    expect(within(grupo).queryByText(/Marcin/)).not.toBeInTheDocument();
    // Sem dobradinha conhecida, o resumo não inventa uma.
    expect(within(grupo).getByText(/2 títulos · 2018–2019$/)).toBeInTheDocument();
  });
});

describe("TeamHistoryDrawerV2 — grade de records", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(dossieCom([]));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="mazda_rookie"
        activeTab="records"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onClose={() => {}}
      />,
    );
  });

  it("mantém a ordem e o agrupamento dos cinco cards", async () => {
    const drawer = await screen.findByTestId("team-history-drawer");
    await waitFor(() => expect(drawer.querySelectorAll("[data-record]")).toHaveLength(5));
    const cards = [...drawer.querySelectorAll("[data-record]")];
    expect(cards.map((card) => card.dataset.record)).toEqual([
      "titles",
      "wins",
      "podiums",
      "podium_rate",
      "win_rate",
    ]);
    // As três contagens numa linha, as duas taxas noutra.
    expect(cards[0].parentElement).not.toBe(cards[3].parentElement);
    expect(cards[0].parentElement).toBe(cards[2].parentElement);
    expect(cards[3].parentElement).toBe(cards[4].parentElement);
  });
});

describe("TeamHistoryDrawerV2 — cronologia em Esportivo", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  function abrirEsportivo(extra) {
    invoke.mockResolvedValue(dossieCom([], extra));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="mazda_rookie"
        activeTab="sport"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onClose={() => {}}
      />,
    );
  }

  // A tabela temporada a temporada saiu: era o mesmo conteúdo da faixa de
  // Records em números. Sobra o que agregado nenhum responde.
  it("troca a tabela temporada a temporada pelos três blocos novos", async () => {
    abrirEsportivo({
      recent_form: [
        { year: "2025", round: 7, category: "Mazda Championship", category_id: "mazda_rookie", position: 1 },
        { year: "2025", round: 8, category: "Mazda Championship", category_id: "mazda_rookie", position: 3 },
        { year: "2026", round: 1, category: "Production", category_id: "production_challenger", position: 11 },
      ],
      result_spread: { races: 20, first: 5, podium: 7, near_miss: 3, top_ten: 4, outside: 1 },
      season_results: [
        { year: "2024", category: "Mazda Championship", category_id: "mazda_rookie", position: "P2", races: 8, wins: 4, podiums: 8, points: "224" },
        { year: "2025", category: "Mazda Championship", category_id: "mazda_rookie", position: "P1", races: 8, wins: 7, podiums: 8, points: "215" },
        { year: "2026", category: "Production", category_id: "production_challenger", position: "—", races: 4, wins: 0, podiums: 0, points: "30" },
      ],
    });

    await screen.findByTestId("team-history-recent-form");
    expect(screen.queryByText("Temporada a temporada")).not.toBeInTheDocument();

    // Fita: uma corrida por quadrado, na ordem em que aconteceram.
    const fita = screen.getByTestId("team-history-recent-form");
    expect([...fita.children].map((el) => el.textContent)).toEqual(["1", "3", "11"]);
    // A troca de categoria no meio da fita explica a queda — sem ela, 11º se
    // leria como perda de forma.
    expect(within(fita.parentElement).getByText(/agora na Production/)).toBeInTheDocument();

    // Curva: só as temporadas com posição conhecida viram ponto. 2026 está em
    // andamento ("—") e não pode inventar um resultado.
    const curva = screen.getByTestId("team-history-curve");
    expect([...curva.querySelectorAll("[data-season]")].map((el) => el.dataset.season)).toEqual(["2024", "2025"]);
    // A tira de categoria, essa cobre as três — inclusive a temporada corrente.
    expect([...curva.querySelectorAll("[data-category]")].map((el) => el.dataset.category)).toEqual([
      "mazda_rookie",
      "mazda_rookie",
      "production_challenger",
    ]);

    // Assinatura: uma faixa por grupo de colocação, proporcional à contagem.
    const barra = screen.getByTestId("team-history-spread");
    expect([...barra.children].map((el) => [el.dataset.band, el.style.flexGrow])).toEqual([
      ["first", "5"],
      ["podium", "7"],
      ["nearMiss", "3"],
      ["topTen", "4"],
      ["outside", "1"],
    ]);
  });

  // Marcos e linha do tempo eram dois blocos, e os dois contavam a primeira
  // vitória. Um bloco só, em ordem cronológica, e o fato repetido fica com a
  // versão que traz categoria e rodada.
  it("funde marcos e linha do tempo numa cronologia só", async () => {
    abrirEsportivo({
      milestones: [
        { label: "Primeiro pódio", year: "2016", kind: "first_podium" },
        { label: "Primeira vitória", year: "2017", kind: "first_win" },
        { label: "Primeiro título", year: "2019", kind: "first_title" },
      ],
      timeline: [
        { year: "2016", text: "Primeira corrida registrada em Mazda Championship, rodada 1.", kind: "first_race" },
        { year: "2017", text: "Primeira vitória real em Mazda Championship, rodada 4.", kind: "first_win" },
        { year: "2026", text: "Último registro em Production, rodada 5.", kind: "last_record" },
      ],
    });

    const trilho = await screen.findByTestId("team-history-milestones");
    const textos = [...trilho.querySelectorAll("li")].map((li) => li.textContent);
    expect(textos).toEqual([
      "2016Primeira corrida registrada em Mazda Championship, rodada 1.",
      "2016Primeiro pódio",
      "2017Primeira vitória real em Mazda Championship, rodada 4.",
      "2019Primeiro título",
      "2026Último registro em Production, rodada 5.",
    ]);
    // O marco cru "Primeira vitória" some: a linha do tempo já conta o mesmo
    // fato, com categoria e rodada.
    expect(textos.some((texto) => texto === "2017Primeira vitória")).toBe(false);
  });
});

describe("TeamHistoryDrawerV2 — pódios por corrida", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  function abrir(seasonResults, extra) {
    invoke.mockResolvedValue(dossieCom(seasonResults, extra));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="mazda_rookie"
        activeTab="records"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onClose={() => {}}
      />,
    );
  }

  it("reparte a coluna em 1º, 2º, 3º e 4º-5º", async () => {
    abrir([
      {
        year: "2026",
        category: "Mazda Rookie",
        position: "P3",
        wins: 3,
        seconds: 4,
        thirds: 2,
        fourths: 1,
        fifths: 2,
        podiums: 9,
        points: "180",
        races: 13,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    await waitFor(() => expect(within(faixa).getByTitle(/2026/)).toBeInTheDocument());

    // Eixo Y: três marcas fixas, que é o que dá escala à coluna.
    expect(within(faixa).getByText("0%")).toBeInTheDocument();
    expect(within(faixa).getByText("50%")).toBeInTheDocument();
    expect(within(faixa).getByText("100%")).toBeInTheDocument();

    const coluna = within(faixa).getByTitle(/2026/);
    // A coluna tem piso e teto: sem o teto, uma carreira de 3 temporadas
    // esticaria cada barra por um terço do painel.
    expect(coluna).toHaveClass("min-w-[24px]", "max-w-[64px]");
    // 12 resultados no top 5 em 13 corridas ≈ 92% da altura total.
    const pilha = coluna.querySelector("[style*='height']");
    expect(pilha.getAttribute("style")).toMatch(/height: 92\./);

    // De cima para baixo, proporcionais à contagem. 4º e 5º entram somados numa
    // faixa só — a cor apagada não distingue os dois.
    const degraus = [...coluna.querySelectorAll("[data-step]")];
    expect(degraus.map((el) => el.dataset.step)).toEqual(["first", "second", "third", "nearMiss"]);
    expect(degraus.map((el) => el.style.flexGrow)).toEqual(["3", "4", "2", "3"]);
  });

  it("omite a colocação que a temporada não teve", async () => {
    abrir([
      {
        year: "2024",
        category: "Mazda Rookie",
        position: "P8",
        wins: 0,
        seconds: 0,
        thirds: 2,
        fourths: 0,
        fifths: 0,
        podiums: 2,
        points: "40",
        races: 10,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByTitle(/2024/);
    const degraus = [...coluna.querySelectorAll("[data-step]")];
    expect(degraus.map((el) => el.dataset.step)).toEqual(["third"]);
  });

  it("monta o tooltip em linhas e sem colocação zerada", async () => {
    abrir([
      {
        year: "2026",
        category: "Mazda Rookie",
        position: "P3",
        wins: 3,
        seconds: 0,
        thirds: 2,
        fourths: 1,
        fifths: 0,
        podiums: 5,
        points: "180",
        races: 13,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByTitle(/2026/);

    expect(coluna.getAttribute("title").split("\n")).toEqual([
      "2026 · Mazda Rookie",
      "P3 no campeonato · 6 de 13 corridas no top 5",
      "",
      "3× 1º",
      "2× 3º",
      "1× 4º-5º",
    ]);
  });

  // O eixo é o do mundo: a equipe nova precisa mostrar que chegou tarde, em vez
  // de ocupar três colunas soltas num gráfico largo.
  it("preenche os anos anteriores à equipe com colunas de ausência", async () => {
    abrir(
      [
        {
          year: "2025",
          category: "Mazda Rookie",
          position: "P4",
          wins: 0,
          seconds: 0,
          thirds: 1,
          fourths: 2,
          fifths: 0,
          podiums: 1,
          points: "60",
          races: 10,
        },
      ],
      { world_first_year: 2022, world_last_year: 2026 },
    );

    const faixa = await screen.findByTestId("team-history-trajectory");
    await waitFor(() => expect(faixa.querySelectorAll("[data-year]")).toHaveLength(5));

    const colunas = [...faixa.querySelectorAll("[data-year]")];
    expect(colunas.map((c) => c.dataset.year)).toEqual(["2022", "2023", "2024", "2025", "2026"]);
    // Só 2025 tem corrida; os outros quatro são ausência e não desenham barra.
    expect(colunas.filter((c) => c.dataset.absent === "true").map((c) => c.dataset.year)).toEqual([
      "2022",
      "2023",
      "2024",
      "2026",
    ]);
    expect(colunas[0].querySelectorAll("[data-step]")).toHaveLength(0);
    expect(colunas[0].getAttribute("title")).toContain("não disputou");
    expect(colunas[3].querySelectorAll("[data-step]")).toHaveLength(2);
  });

  // Save antigo: a faixa mostra as últimas 15 temporadas, não as 27 do mundo.
  it("recorta a faixa nas últimas 15 temporadas e anuncia o intervalo", async () => {
    abrir(
      [
        {
          year: "2005",
          category: "Mazda Rookie",
          position: "P2",
          wins: 5,
          seconds: 0,
          thirds: 0,
          fourths: 0,
          fifths: 0,
          podiums: 5,
          points: "200",
          races: 10,
        },
        {
          year: "2026",
          category: "Mazda Rookie",
          position: "P4",
          wins: 0,
          seconds: 0,
          thirds: 1,
          fourths: 0,
          fifths: 0,
          podiums: 1,
          points: "40",
          races: 10,
        },
      ],
      { world_first_year: 2000, world_last_year: 2026 },
    );

    const faixa = await screen.findByTestId("team-history-trajectory");
    await waitFor(() => expect(faixa.querySelectorAll("[data-year]")).toHaveLength(15));

    const colunas = [...faixa.querySelectorAll("[data-year]")];
    expect(colunas[0].dataset.year).toBe("2012");
    expect(colunas[14].dataset.year).toBe("2026");
    // A temporada de 2005 ficou fora da janela — e o intervalo desenhado é
    // anunciado ao lado do título, para o recorte não passar por "história toda".
    expect(screen.getByText("2012–2026")).toBeInTheDocument();
  });

  // Temporada disputada e sem nenhum top 5 precisa OCUPAR espaço: o trilho vazio
  // é o que a diferencia de uma temporada que não existiu.
  it("desenha o trilho da temporada sem nenhum top 5", async () => {
    abrir([
      {
        year: "2023",
        category: "Mazda Rookie",
        position: "P14",
        wins: 0,
        seconds: 0,
        thirds: 0,
        fourths: 0,
        fifths: 0,
        podiums: 0,
        points: "12",
        races: 10,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByTitle(/2023/);
    expect(coluna.querySelectorAll("[data-step]")).toHaveLength(0);
    // A coluna existe e tem altura, mesmo sem barra preenchida.
    expect(coluna).toHaveClass("h-full");
    expect(within(faixa).getByText("2023")).toBeInTheDocument();
    expect(coluna.getAttribute("title")).toContain("Nenhum resultado no top 5");
  });

  // O motivo de o top 5 existir: sem ele, uma equipe de meio de grid virava uma
  // faixa vazia, e o gráfico não dizia nada sobre a temporada dela.
  it("desenha a temporada sem pódio algum, só com 4º e 5º", async () => {
    abrir([
      {
        year: "2025",
        category: "Mazda Rookie",
        position: "P6",
        wins: 0,
        seconds: 0,
        thirds: 0,
        fourths: 3,
        fifths: 2,
        podiums: 0,
        points: "60",
        races: 10,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByTitle(/2025/);
    const pilha = coluna.querySelector("[style*='height']");
    expect(pilha.getAttribute("style")).toContain("height: 50%");
    const degraus = [...coluna.querySelectorAll("[data-step]")];
    expect(degraus.map((el) => el.dataset.step)).toEqual(["nearMiss"]);
    expect(degraus[0].style.flexGrow).toBe("5");
  });

  // A altura da coluna diz o quão bem foi; a tira embaixo diz ONDE foi. Sem ela,
  // 40% de top 5 na categoria de entrada e 40% na GT3 desenhavam idêntico.
  it("marca a categoria de cada temporada sob a coluna", async () => {
    abrir(
      [
        {
          year: "2024",
          category: "Mazda Rookie",
          category_id: "mazda_rookie",
          position: "P2",
          wins: 4,
          seconds: 0,
          thirds: 0,
          fourths: 0,
          fifths: 0,
          podiums: 4,
          points: "150",
          races: 10,
        },
        {
          year: "2026",
          category: "GT4 Series",
          category_id: "gt4",
          position: "P7",
          wins: 0,
          seconds: 0,
          thirds: 1,
          fourths: 0,
          fifths: 0,
          podiums: 1,
          points: "60",
          races: 10,
        },
      ],
      { world_first_year: 2024, world_last_year: 2026 },
    );

    const tira = await screen.findByTestId("team-history-trajectory-categories");
    const celulas = [...tira.children];
    expect(celulas.map((el) => el.dataset.category)).toEqual(["mazda_rookie", undefined, "gt4"]);
    // Mesma paleta do resto do app — a troca de cor É a subida de categoria.
    expect(celulas[0].style.backgroundColor).toBe("rgb(255, 212, 0)");
    expect(celulas[2].style.backgroundColor).toBe("rgb(32, 112, 240)");
    // Ano sem corrida não recebe cor: a tira não pode inventar categoria.
    expect(celulas[1].style.backgroundColor).toBe("transparent");

    // A legenda nomeia só as categorias que aparecem na janela, na ordem em que
    // a equipe passou por elas.
    const legenda = screen.getByTestId("team-history-trajectory-legend");
    expect(within(legenda).getByText("Mazda Rookie")).toBeInTheDocument();
    expect(within(legenda).getByText("GT4 Series")).toBeInTheDocument();
  });

  // "Não disputou" e "disputou em outra escada" não podem desenhar igual: o
  // dossiê recorta os fatos ao grupo comparável, e o "×" afirmava que a equipe
  // tinha sumido do mundo num ano em que ela correu outro campeonato.
  it("separa o ano fora do recorte do ano sem corrida alguma", async () => {
    abrir(
      [
        {
          year: "2026",
          category: "GT3 Championship",
          category_id: "gt3",
          position: "P4",
          wins: 0,
          seconds: 0,
          thirds: 1,
          fourths: 0,
          fifths: 0,
          podiums: 1,
          points: "80",
          races: 10,
        },
      ],
      {
        world_first_year: 2024,
        world_last_year: 2026,
        outside_scope_seasons: [{ year: "2025", category: "Endurance", category_id: "endurance" }],
      },
    );

    const faixa = await screen.findByTestId("team-history-trajectory");
    const colunas = [...faixa.querySelectorAll("[data-year]")];
    expect(colunas.map((el) => el.dataset.year)).toEqual(["2024", "2025", "2026"]);
    // 2024 é ausência de verdade; 2025 é mudança de endereço.
    expect(colunas[1].getAttribute("title")).toContain("Endurance");
    expect(colunas[1].getAttribute("title")).toContain("fora deste recorte");
    expect(colunas[0].getAttribute("title")).toContain("não disputou");

    // A tira de categoria pinta o ano de fora com a cor da escada em que ela
    // estava — é o que responde "onde, então?".
    const tira = screen.getByTestId("team-history-trajectory-categories");
    expect([...tira.children].map((el) => el.dataset.category)).toEqual([undefined, "endurance", "gt3"]);
  });

  it("não desenha temporada sem corrida registrada", async () => {
    abrir([
      { year: "2023", category: "Mazda Rookie", position: "—", wins: 0, podiums: 0, points: "0", races: 0 },
    ]);

    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(screen.queryByTestId("team-history-trajectory")).not.toBeInTheDocument();
  });
});
