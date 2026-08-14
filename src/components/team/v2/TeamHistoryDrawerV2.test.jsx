import { StrictMode } from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { invoke } from "@tauri-apps/api/core";
import { pisoDeAbertura } from "../../ui/aberturaDePainel.js";
import { TeamHistoryDrawerV2 } from "./TeamHistoryDrawerV2";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// O piso não corre debaixo do vitest de qualquer jeito; o espião existe para
// poder afirmar COM QUE ARGUMENTO ele foi pedido, que é onde o bug morava.
vi.mock("../../ui/aberturaDePainel.js", () => ({
  ABERTURA_MS: 0,
  pisoDeAbertura: vi.fn(() => Promise.resolve()),
}));

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

describe("TeamHistoryDrawerV2 — abertura", () => {
  it("esconde o dossie inteiro enquanto abre, e nao so o miolo", async () => {
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

    // A equipe ja vem no `prop`, entao cabecalho e abas desenhariam antes do
    // `invoke` — e o drawer abria de estalo enquanto a ficha do piloto, que
    // depende do payload para tudo, tinha a sequencia de abertura.
    expect(screen.getByTestId("team-history-loading")).toBeInTheDocument();
    expect(screen.queryByText(EQUIPE.nome)).toBeNull();

    await waitFor(() => expect(screen.queryByTestId("team-history-loading")).toBeNull());
    expect(await screen.findByText(EQUIPE.nome)).toBeInTheDocument();
  });

  it("nao esvazia o miolo ao trocar de equipe nem rebusca ao trocar de aba", async () => {
    invoke.mockReset();
    invoke.mockResolvedValue(dossieCom([]));

    const props = {
      careerId: "c1",
      teams: [EQUIPE],
      playerTeam: null,
      activeCategory: "mazda_rookie",
      activeTab: "records",
      onTabChange: () => {},
      onSelectTeam: () => {},
      onClose: () => {},
    };
    const tela = render(<TeamHistoryDrawerV2 {...props} team={EQUIPE} />);
    await waitFor(() => expect(screen.queryByTestId("team-history-loading")).toBeNull());
    expect(screen.getByText(EQUIPE.nome)).toBeInTheDocument();

    // Trocar de ABA não toca no banco: o payload já está em mãos.
    const buscas = () => invoke.mock.calls.filter(([cmd]) => cmd === "get_team_history_dossier").length;
    const antes = buscas();
    tela.rerender(<TeamHistoryDrawerV2 {...props} team={EQUIPE} activeTab="sport" />);
    tela.rerender(<TeamHistoryDrawerV2 {...props} team={EQUIPE} activeTab="identity" />);
    expect(buscas()).toBe(antes);

    // Trocar de EQUIPE busca — e enquanto a resposta não vem, o dossiê anterior
    // continua desenhado. Sem isso o miolo cai no aviso de carga e nos números
    // placeholder, e o painel parece fechar e abrir.
    let entregar;
    invoke.mockImplementation(() => new Promise((resolve) => {
      entregar = resolve;
    }));
    const OUTRA = { ...EQUIPE, id: "T2", nome: "Weekend Warriors Racing" };
    tela.rerender(<TeamHistoryDrawerV2 {...props} teams={[EQUIPE, OUTRA]} team={OUTRA} />);

    expect(screen.queryByTestId("team-history-loading")).toBeNull();
    expect(screen.getByText(OUTRA.nome)).toBeInTheDocument();

    await act(async () => {
      entregar(dossieCom([], { record_scope: "Grupo Toyota" }));
    });
    expect(await screen.findByText("Grupo Toyota")).toBeInTheDocument();
  });

  it("nao gasta a abertura na passagem descartada do StrictMode", async () => {
    invoke.mockReset();
    invoke.mockResolvedValue(dossieCom([]));
    pisoDeAbertura.mockClear();

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
      { wrapper: StrictMode },
    );

    await waitFor(() => expect(screen.queryByTestId("team-history-loading")).toBeNull());

    // Em dev o StrictMode monta, desmonta e remonta o efeito. Enquanto a
    // bandeira da primeira carga era baixada ao PEDIR o piso, quem a gastava era
    // a passagem descartada — e a passagem que de fato desenha pedia o piso como
    // se fosse navegação entre equipes. O dossiê abria de estalo, e mexer no
    // ABERTURA_MS não mudava nada.
    expect(pisoDeAbertura).toHaveBeenCalled();
    expect(pisoDeAbertura.mock.calls.every(([ehAbertura]) => ehAbertura === true)).toBe(true);
  });
});

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

  // Records precisa ter altura estável entre equipes: o que fala de gente varia
  // de zero a cinco linhas e fazia a tela pular ao navegar com as setas. Vive em
  // Esportivo.
  it("mantém os blocos de pilotos fora da seção Records", async () => {
    const drawer = await screen.findByTestId("team-history-drawer");
    await waitFor(() => expect(drawer.querySelectorAll("[data-record]")).toHaveLength(5));
    expect(screen.queryByTestId("team-history-best-drivers")).not.toBeInTheDocument();
    expect(screen.queryByTestId("team-history-lineup")).not.toBeInTheDocument();
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

  // As duas faixas desenham o mesmo eixo de anos em escalas diferentes. Sem o
  // elo, achar na faixa de top 5 o ano do título da régua era contar coluna com
  // o dedo — que é justamente a pergunta que as duas juntas respondem.
  it("acende o mesmo ano na régua de títulos e na faixa de top 5, nos dois sentidos", async () => {
    abrirGaleria(
      [titulo(2019)],
      [temporada(2018), temporada(2019), temporada(2020)],
    );

    const regua = await screen.findByTestId("team-history-title-rail");
    const faixa = await screen.findByTestId("team-history-trajectory");
    const celula = [...regua.children].find((el) => el.dataset.year === "2019");
    const coluna = faixa.querySelector("[data-year='2019']");
    const outraColuna = faixa.querySelector("[data-year='2018']");

    // Régua → faixa.
    fireEvent.mouseEnter(celula);
    expect(coluna).toHaveAttribute("data-aceso", "true");
    expect(outraColuna).not.toHaveAttribute("data-aceso");
    fireEvent.mouseLeave(celula);
    expect(coluna).not.toHaveAttribute("data-aceso");

    // Faixa → régua.
    fireEvent.mouseEnter(coluna);
    expect(celula).toHaveAttribute("data-aceso", "true");
    fireEvent.mouseLeave(coluna);
    expect(celula).not.toHaveAttribute("data-aceso");
  });

  // Um título só ganha a MESMA tela de quem tem seis. Dar a tela boa só para a
  // dinastia premia quem já tem muito — e a régua é mais informativa aí, porque
  // mostra o único ano que importou dentro de uma carreira longa.
  it("dá a régua e a tabela também para quem ganhou uma vez só", async () => {
    abrirGaleria(
      [titulo(2019, { points: "268", wins: 5, champion_driver: "Lucas Prado" })],
      [temporada(2017), temporada(2018), temporada(2019), temporada(2020), temporada(2021)],
    );

    const regua = await screen.findByTestId("team-history-title-rail");
    // Cinco temporadas na régua, uma só marcada — é o que localiza o título na
    // carreira em vez de deixá-lo como fato solto.
    const celulas = [...regua.children];
    expect(celulas).toHaveLength(5);
    expect(celulas.filter((el) => el.dataset.title === "true").map((el) => el.dataset.year)).toEqual(["2019"]);

    const grupo = screen.getByTestId("team-history-title-group");
    expect(within(grupo).getByText(/1 título · 2019 · 1 dobradinha/)).toBeInTheDocument();
    expect(grupo.querySelectorAll("[data-title-year]")).toHaveLength(1);
    expect(within(grupo).getByText("268")).toBeInTheDocument();
    expect(within(grupo).getByText(/Lucas Prado/)).toBeInTheDocument();
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

describe("TeamHistoryDrawerV2 — blocos de Esportivo", () => {
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
    expect(within(fita.parentElement).getByText(/Agora na Production/)).toBeInTheDocument();

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
    // A proporção a barra já desenha; o número e a fatia é o que ela escondia.
    expect([...barra.children].map((el) => el.textContent)).toEqual([
      "5 (25%)",
      "7 (35%)",
      "3 (15%)",
      "4 (20%)",
      "1 (5%)",
    ]);
  });

  // Faixa fina demais recorta o número no meio. Ela fica só como cor, e a
  // contagem passa para a legenda em vez de sumir.
  it("manda o número da faixa estreita para a legenda", async () => {
    abrirEsportivo({
      recent_form: [{ year: "2025", round: 1, category: "Mazda Championship", category_id: "mazda_rookie", position: 5 }],
      result_spread: { races: 100, first: 1, podium: 99 },
    });

    const barra = await screen.findByTestId("team-history-spread");
    expect([...barra.children].map((el) => el.textContent)).toEqual(["", "99 (99%)"]);
    expect(within(barra.parentElement).getByText("1º · 1 (1%)")).toBeInTheDocument();
    expect(within(barra.parentElement).getByText("2º-3º")).toBeInTheDocument();
  });

  // A galeria e o ranking listam as MESMAS pessoas por critérios diferentes.
  // Achar num o nome que está no outro é o gesto mais repetido da seção — e o
  // par quase nunca cabe na mesma tela.
  describe("elo entre a galeria e o ranking", () => {
    const ELENCO = [
      { slot: 1, driver_id: "d1", name: "Sami Nieminen", races: 60, titles: 0, wins: 15, podiums: 32, best_position: 1, first_year: "2017", last_year: "2023" },
      { slot: 1, driver_id: "d2", name: "Hugo Ramirez", races: 40, titles: 2, wins: 6, podiums: 17, best_position: 1, first_year: "2013", last_year: "2018" },
      { slot: 2, driver_id: "d3", name: "Ramiro Herrera", races: 20, titles: 1, wins: 3, podiums: 6, best_position: 1, first_year: "2015", last_year: "2016" },
    ];

    it("acende o mesmo piloto na galeria e no ranking, nos dois sentidos", async () => {
      abrirEsportivo({ lineup: ELENCO });

      const galeria = await screen.findByTestId("team-history-lineup");
      const ranking = screen.getByTestId("team-history-best-drivers");
      const naGaleria = (id) => galeria.querySelector(`[data-driver='${id}']`);
      const noRanking = (id) => ranking.querySelector(`[data-driver='${id}']`);

      // Ranking → galeria.
      fireEvent.mouseEnter(noRanking("d3"));
      expect(naGaleria("d3")).toHaveAttribute("data-aceso", "true");
      expect(naGaleria("d1")).not.toHaveAttribute("data-aceso");
      fireEvent.mouseLeave(noRanking("d3"));
      expect(naGaleria("d3")).not.toHaveAttribute("data-aceso");

      // Galeria → ranking.
      fireEvent.mouseEnter(naGaleria("d1"));
      expect(noRanking("d1")).toHaveAttribute("data-aceso", "true");
      fireEvent.mouseLeave(naGaleria("d1"));
      expect(noRanking("d1")).not.toHaveAttribute("data-aceso");
    });

    // O elo acende e só. Rolar a página até o par foi tentado e desfeito: a tela
    // se mexendo sozinha sob o cursor é pior que o problema que resolvia.
    it("não mexe na página ao acender", async () => {
      const rolagem = vi.fn();
      Element.prototype.scrollIntoView = rolagem;
      abrirEsportivo({ lineup: ELENCO });

      const ranking = await screen.findByTestId("team-history-best-drivers");
      fireEvent.mouseEnter(ranking.querySelector("[data-driver='d3']"));
      expect(rolagem).not.toHaveBeenCalled();
    });
  });

  // A cronologia de marcos saiu do lugar dela e o ranking entrou: a galeria
  // acima conta a sucessão, e ordenar os mesmos nomes por currículo é o que ela
  // não responde.
  it("põe o título acima da vitória ao ordenar os melhores pilotos", async () => {
    abrirEsportivo({
      lineup: [
        // Quinze vitórias e nenhum título: a lista dizia que este era o melhor
        // da casa, e o campeão de seis vitórias vinha abaixo dele.
        { slot: 1, driver_id: "d1", name: "Sami Nieminen", races: 60, titles: 0, wins: 15, podiums: 32, best_position: 1, first_year: "2017", last_year: "2023" },
        { slot: 1, driver_id: "d2", name: "Hugo Ramirez", races: 40, titles: 2, wins: 6, podiums: 17, best_position: 1, first_year: "2013", last_year: "2018" },
        { slot: 2, driver_id: "d3", name: "Ramiro Herrera", races: 20, titles: 1, wins: 3, podiums: 6, best_position: 1, first_year: "2015", last_year: "2016" },
        { slot: 2, driver_id: "d4", name: "Kai Fim", races: 9, titles: 0, wins: 0, podiums: 0, best_position: 7, first_year: "2023", last_year: "2023" },
        { slot: 2, driver_id: "d5", name: "Zoe Ultima", races: 9, titles: 0, wins: 0, podiums: 0, best_position: 12, first_year: "2024", last_year: "2024" },
      ],
    });

    const ranking = await screen.findByTestId("team-history-best-drivers");
    const linhas = [...ranking.querySelectorAll("[data-driver]")];
    // Título primeiro; empatados, decide a vitória, depois o pódio. A melhor
    // colocação separa quem não tem nenhum dos três — Kai (P7) na frente de Zoe
    // (P12) sem aparecer em coluna nenhuma.
    expect(linhas.map((li) => li.dataset.driver)).toEqual(["d2", "d3", "d1", "d4", "d5"]);
    expect(linhas.map((li) => li.dataset.rank)).toEqual(["1", "2", "3", "4", "5"]);
    // Título, vitória, pódio e corrida. Nada de barra: ela mediria um eixo só e
    // a ordem tem quatro.
    const colunas = (li) => [...li.querySelectorAll("[data-col]")].map((el) => el.textContent);
    expect(colunas(linhas[0])).toEqual(["2", "6", "17", "40"]);
    expect(colunas(linhas[2])).toEqual(["—", "15", "32", "60"]);
    // Zero vira travessão: "0" alinhado com os outros números pesa como dado e
    // é ausência de dado. A corrida nunca é travessão — é o filtro de entrada.
    expect(colunas(linhas[3])).toEqual(["—", "—", "—", "9"]);
  });

  // Dez é o corte: uma equipe antiga alcança quem correu na década anterior, e
  // é onde a lista para de crescer.
  it("corta o ranking nos dez melhores", async () => {
    abrirEsportivo({
      lineup: Array.from({ length: 14 }, (_, index) => ({
        slot: (index % 2) + 1,
        driver_id: `d${index}`,
        name: `Piloto ${index}`,
        races: 10,
        titles: 0,
        wins: 14 - index,
        podiums: 14 - index,
        best_position: 1,
        first_year: String(2010 + index),
        last_year: String(2010 + index),
      })),
    });

    const ranking = await screen.findByTestId("team-history-best-drivers");
    const linhas = [...ranking.querySelectorAll("[data-driver]")];
    expect(linhas).toHaveLength(10);
    expect(linhas.at(-1).dataset.driver).toBe("d9");
  });

  // Uma passagem interrompida não parte o currículo em dois: quem saiu e voltou
  // tem dois mandatos na galeria e um só lugar no ranking.
  it("soma as passagens do mesmo piloto num currículo só", async () => {
    abrirEsportivo({
      lineup: [
        { slot: 1, driver_id: "d1", name: "Ana Duarte", races: 10, titles: 0, wins: 1, podiums: 2, best_position: 1, first_year: "2020", last_year: "2020" },
        { slot: 1, driver_id: "d2", name: "Rui Matos", races: 10, titles: 1, wins: 1, podiums: 3, best_position: 1, first_year: "2021", last_year: "2021" },
        { slot: 1, driver_id: "d1", name: "Ana Duarte", races: 12, titles: 1, wins: 2, podiums: 4, best_position: 1, first_year: "2023", last_year: "2024" },
      ],
    });

    const ranking = await screen.findByTestId("team-history-best-drivers");
    const linhas = [...ranking.querySelectorAll("[data-driver]")];
    expect(linhas.map((li) => li.dataset.driver)).toEqual(["d1", "d2"]);
    expect([...linhas[0].querySelectorAll("[data-col]")].map((el) => el.textContent)).toEqual(["1", "3", "6", "22"]);
    expect(within(linhas[0]).getByText("2020–2024")).toBeInTheDocument();
  });

  // Um nome sozinho não é ranking — e a galeria logo acima já o mostra com mais
  // dado do que o ranking teria.
  it("não desenha o ranking com um piloto só", async () => {
    abrirEsportivo({
      lineup: [
        { slot: 1, driver_id: "d1", name: "Ana Duarte", races: 32, wins: 4, podiums: 11, best_position: 1, first_year: "2020", last_year: "2023" },
      ],
    });

    await screen.findByTestId("team-history-lineup");
    expect(screen.queryByTestId("team-history-best-drivers")).not.toBeInTheDocument();
  });
});

describe("TeamHistoryDrawerV2 — confiabilidade e pilotos", () => {
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

  // A barra reparte as LARGADAS, não os abandonos: chegar ao fim é a maior
  // fatia numa equipe saudável, e é ela que dá a escala das outras três.
  it("reparte as largadas entre chegadas e as causas de abandono", async () => {
    abrirEsportivo({
      reliability: {
        races: 40,
        finished: 33,
        finish_rate: 83,
        group_finish_rate: 89,
        mechanical: 4,
        driver_error: 2,
        other: 1,
        worst_part: "Câmbio",
      },
    });

    const painel = await screen.findByTestId("team-history-reliability");
    expect(screen.getByTestId("team-history-finish-rate").textContent).toBe("83%");
    expect([...painel.querySelectorAll("[data-band]")].map((el) => [el.dataset.band, el.style.flexGrow])).toEqual([
      ["finished", "33"],
      ["mechanical", "4"],
      ["driverError", "2"],
      ["other", "1"],
    ]);
    // A média do grupo é o que dá escala: 83% é abaixo de 89%.
    expect(within(painel).getByText(/Grupo em 89%/)).toBeInTheDocument();
    expect(within(painel.parentElement).getByText(/Câmbio/)).toBeInTheDocument();
  });

  // Faixa zerada não vira fatia invisível na barra nem entrada morta na legenda.
  it("omite as causas sem nenhum abandono", async () => {
    abrirEsportivo({
      reliability: { races: 20, finished: 20, finish_rate: 100, group_finish_rate: 94 },
    });

    const painel = await screen.findByTestId("team-history-reliability");
    expect([...painel.querySelectorAll("[data-band]")].map((el) => el.dataset.band)).toEqual(["finished"]);
  });

  it("não desenha confiabilidade sem largada registrada", async () => {
    abrirEsportivo({ recent_form: [{ year: "2025", round: 1, category: "Mazda Championship", category_id: "mazda_rookie", position: 5 }], reliability: { races: 0 } });
    await screen.findByTestId("team-history-recent-form");
    expect(screen.queryByTestId("team-history-reliability")).not.toBeInTheDocument();
  });

  // As duas vagas lado a lado são o que responde "essa equipe troca muito de
  // piloto?". Cada coluna é uma sucessão em ordem cronológica.
  it("reparte as passagens pelas duas vagas, em ordem cronológica", async () => {
    abrirEsportivo({
      lineup: [
        {
          slot: 1,
          driver_id: "d1",
          name: "Ana Duarte",
          nationality: "Brasil",
          first_year: "2020",
          last_year: "2023",
          races: 32,
          wins: 4,
          podiums: 11,
          best_position: 1,
          current_label: "Hoje na GT Pro",
          current_team_name: "Isar Track",
          current_team_color: "#e5793a",
        },
        {
          slot: 1,
          driver_id: "d2",
          name: "Você Mesmo",
          first_year: "2024",
          last_year: "2026",
          races: 18,
          is_player: true,
          still_here: true,
          current_label: "Ainda na equipe",
        },
        {
          slot: 2,
          driver_id: "d3",
          name: "Rui Matos",
          first_year: "2021",
          last_year: "2021",
          races: 9,
          best_position: 5,
          current_label: "Aposentado em 2022",
        },
        {
          slot: 0,
          driver_id: "d4",
          name: "Nino Sub",
          first_year: "2022",
          last_year: "2022",
          races: 1,
          best_position: 14,
          current_label: "Paradeiro desconhecido",
        },
      ],
    });

    const galeria = await screen.findByTestId("team-history-lineup");
    const vaga1 = galeria.querySelector("[data-slot='1']");
    const vaga2 = galeria.querySelector("[data-slot='2']");
    expect([...vaga1.querySelectorAll("[data-driver]")].map((li) => li.dataset.driver)).toEqual(["d1", "d2"]);
    expect([...vaga2.querySelectorAll("[data-driver]")].map((li) => li.dataset.driver)).toEqual(["d3"]);
    // Quem correu sem constar como titular de temporada arquivada não some: cai
    // numa faixa própria em vez de ser empurrado para uma das vagas.
    const avulsos = galeria.querySelector("[data-slot='0']");
    expect([...avulsos.querySelectorAll("[data-driver]")].map((li) => li.dataset.driver)).toEqual(["d4"]);

    // Mesma leitura para todo mundo: quanto correu, até onde chegou. As vitórias
    // entram só quando existem — "melhor P1" não diz se foi uma vez ou dez.
    const linhaAna = galeria.querySelector("[data-driver='d1']");
    // A bandeira é o retrato que a galeria não tem. Quem não tem país no save
    // fica sem ela — e não com a de outro país qualquer.
    expect(linhaAna.querySelector("[data-nationality]").dataset.nationality).toBe("Brasil");
    expect(galeria.querySelector("[data-driver='d3'] [data-nationality]")).toBeNull();
    expect(within(linhaAna).getByText("32 corridas · melhor P1 · 4x")).toBeInTheDocument();
    // Sem vitória, nada de "0x" pendurado no fim da linha.
    expect(within(galeria.querySelector("[data-driver='d3']")).getByText("9 corridas · melhor P5")).toBeInTheDocument();
    expect(within(linhaAna).getByText("2020–2023")).toBeInTheDocument();

    // Quem foi para outra equipe aparece COM a equipe, na cor dela.
    const destino = linhaAna.querySelector("[data-current-team]");
    expect(destino.dataset.currentTeam).toBe("Isar Track");
    expect(destino.style.color).toBe("rgb(229, 121, 58)");

    // Quem continua na equipe é marcado na própria linha, e não repete a frase
    // "ainda na equipe" na coluna de destino.
    const linhaAtual = galeria.querySelector("[data-driver='d2']");
    expect(linhaAtual.dataset.current).toBe("true");
    expect(linhaAtual.dataset.player).toBe("true");
    expect(within(linhaAtual).getByText("Atual")).toBeInTheDocument();
    expect(within(linhaAtual).queryByText("Ainda na equipe")).not.toBeInTheDocument();
  });

  // Save recém-criado: o titular de hoje já está no carro e a rodada 1 ainda não
  // rolou. Ele tem de aparecer — a galeria é onde o jogador confere quem está na
  // equipe — mas sem o "0 corridas", que é ruído com cara de dado.
  it("mostra o titular vigente que ainda não largou", async () => {
    abrirEsportivo({
      lineup: [
        {
          slot: 1,
          driver_id: "d1",
          name: "Você Mesmo",
          first_year: "2026",
          last_year: "2026",
          races: 0,
          is_player: true,
          still_here: true,
        },
      ],
    });

    const galeria = await screen.findByTestId("team-history-lineup");
    const linha = galeria.querySelector("[data-driver='d1']");
    expect(linha.dataset.current).toBe("true");
    expect(within(linha).getByText("ainda não correu")).toBeInTheDocument();
    expect(within(linha).queryByText(/0 corridas/)).not.toBeInTheDocument();
  });

  it("não desenha a galeria quando ninguém correu pela equipe", async () => {
    abrirEsportivo({
      recent_form: [{ year: "2025", round: 1, category: "Mazda Championship", category_id: "mazda_rookie", position: 5 }],
      lineup: [],
    });
    await screen.findByTestId("team-history-recent-form");
    expect(screen.queryByTestId("team-history-lineup")).not.toBeInTheDocument();
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
    await waitFor(() => expect(within(faixa).getByLabelText(/2026/)).toBeInTheDocument());

    // Eixo Y: três marcas fixas, que é o que dá escala à coluna.
    expect(within(faixa).getByText("0%")).toBeInTheDocument();
    expect(within(faixa).getByText("50%")).toBeInTheDocument();
    expect(within(faixa).getByText("100%")).toBeInTheDocument();

    const coluna = within(faixa).getByLabelText(/2026/);
    // A coluna tem piso e teto: sem o teto, uma carreira de 3 temporadas
    // esticaria cada barra por um terço do painel.
    expect(coluna).toHaveClass("min-w-[24px]", "max-w-[64px]");
    // 12 resultados no top 5 em 13 corridas ≈ 92% da altura total.
    const pilha = coluna.querySelector("[style*='height']");
    expect(pilha.getAttribute("style")).toMatch(/height: 92\./);

    // De cima para baixo, proporcionais à contagem. 4º e 5º entram somados numa
    // faixa só: a coluna responde quantas vezes a equipe chegou perto, não em
    // qual das duas casas ela parou.
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
    const coluna = await within(faixa).findByLabelText(/2024/);
    const degraus = [...coluna.querySelectorAll("[data-step]")];
    expect(degraus.map((el) => el.dataset.step)).toEqual(["third"]);
  });

  it("monta o nome acessível da coluna em linhas e sem colocação zerada", async () => {
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
        dnfs: 2,
        podiums: 5,
        points: "180",
        races: 13,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByLabelText(/2026/);

    expect(coluna.getAttribute("aria-label").split("\n")).toEqual([
      "2026 · Mazda Rookie",
      "P3 no campeonato · 6 de 13 corridas no top 5",
      "",
      "3× 1º",
      "2× 3º",
      "1× 4º-5º",
      "2× DNF",
    ]);
    // O balão é do app, e não do sistema: o `title` nativo desenhava a caixa
    // branca do Windows no meio de um gráfico escuro.
    expect(coluna).not.toHaveAttribute("title");
  });

  it("abre o balão do app no hover da coluna, com a cor de cada colocação", async () => {
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
        dnfs: 2,
        podiums: 5,
        points: "180",
        races: 13,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByLabelText(/2026/);
    expect(screen.queryByTestId("team-history-trajectory-tooltip")).not.toBeInTheDocument();

    fireEvent.mouseEnter(coluna);
    const balao = await screen.findByTestId("team-history-trajectory-tooltip");
    expect(within(balao).getByText("2026 · Mazda Rookie")).toBeInTheDocument();
    expect(within(balao).getByText("P3 no campeonato · 6 de 13 corridas no top 5")).toBeInTheDocument();
    // Uma linha por colocação que aconteceu, com o quadradinho na cor do bloco.
    // Só a contagem: quem diz a colocação é a cor, igual à legenda do gráfico.
    // O abandono é a exceção — não é colocação nenhuma — e guarda o "DNF".
    const linhas = [...balao.querySelectorAll("li")];
    expect(linhas.map((li) => li.textContent)).toEqual(["3×", "2×", "1×", "2× DNF"]);
    expect(linhas[0].querySelector("span").style.backgroundColor).toBe("rgb(242, 196, 109)");
    expect(linhas[3].querySelector("span").style.backgroundColor).toBe("rgb(239, 68, 68)");

    fireEvent.mouseLeave(coluna);
    await waitFor(() =>
      expect(screen.queryByTestId("team-history-trajectory-tooltip")).not.toBeInTheDocument(),
    );
  });

  // O vermelho é a outra ponta do ano: o top 5 sobe do chão, o abandono desce
  // do teto, e o meio vazio é o que sobrou.
  it("pendura os abandonos no teto da coluna, em vermelho", async () => {
    abrir([
      {
        year: "2026",
        category: "Mazda Rookie",
        position: "P8",
        wins: 0,
        seconds: 0,
        thirds: 2,
        fourths: 0,
        fifths: 0,
        dnfs: 3,
        podiums: 2,
        points: "40",
        races: 10,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByLabelText(/2026/);
    const vermelho = coluna.querySelector("[data-dnf]");
    expect(vermelho.getAttribute("style")).toContain("height: 30%");
    expect(vermelho).toHaveClass("top-0");

    // Ano sem abandono não ganha bloco algum: um fio vermelho de 3px onde não
    // houve abandono afirmaria o contrário do que aconteceu.
    expect(faixa.querySelectorAll("[data-dnf]")).toHaveLength(1);
  });

  it("não deixa o vermelho invadir o top 5 quando os dois carros abandonam muito", async () => {
    // 9 abandonos em 10 corridas é 90% — mas 80% do ano terminou no top 5 (os
    // dois carros: um quebra, o outro pontua). O vermelho para onde o top 5
    // começa; o número cheio vive no balão.
    abrir([
      {
        year: "2026",
        category: "Mazda Rookie",
        position: "P2",
        wins: 4,
        seconds: 4,
        thirds: 0,
        fourths: 0,
        fifths: 0,
        dnfs: 9,
        podiums: 8,
        points: "200",
        races: 10,
      },
    ]);

    const faixa = await screen.findByTestId("team-history-trajectory");
    const coluna = await within(faixa).findByLabelText(/2026/);
    expect(coluna.querySelector("[data-dnf]").getAttribute("style")).toContain("height: 20%");
    expect(coluna.getAttribute("aria-label")).toContain("9× DNF");
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
    expect(colunas[0].getAttribute("aria-label")).toContain("não disputou");
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
    const coluna = await within(faixa).findByLabelText(/2023/);
    expect(coluna.querySelectorAll("[data-step]")).toHaveLength(0);
    // A coluna existe e tem altura, mesmo sem barra preenchida.
    expect(coluna).toHaveClass("h-full");
    expect(within(faixa).getByText("2023")).toBeInTheDocument();
    expect(coluna.getAttribute("aria-label")).toContain("Nenhum resultado no top 5");
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
    const coluna = await within(faixa).findByLabelText(/2025/);
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
    expect(colunas[1].getAttribute("aria-label")).toContain("Endurance");
    expect(colunas[1].getAttribute("aria-label")).toContain("fora deste recorte");
    expect(colunas[0].getAttribute("aria-label")).toContain("não disputou");

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

// Os dois arranjos da seção Esportivo. O contrato é curto e é o que importa:
// nenhum dado existe num e falta no outro, e o clássico continua a um clique.
describe("TeamHistoryDrawerV2 — layout da seção Esportivo", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  const DADOS = {
    reliability: { races: 40, finished: 33, finish_rate: 83, group_finish_rate: 89, mechanical: 4, driver_error: 2, other: 1 },
    result_spread: { races: 20, first: 5, podium: 7, near_miss: 3, top_ten: 4, outside: 1 },
    recent_form: [
      { year: "2025", round: 7, category: "Mazda Championship", category_id: "mazda_rookie", position: 1 },
      { year: "2025", round: 8, category: "Mazda Championship", category_id: "mazda_rookie", position: 3 },
    ],
    season_results: [
      { year: "2024", category: "Mazda Championship", category_id: "mazda_rookie", position: "P2", races: 8, wins: 4, podiums: 8, points: "224" },
      { year: "2025", category: "Mazda Championship", category_id: "mazda_rookie", position: "P1", races: 8, wins: 7, podiums: 8, points: "215" },
    ],
    lineup: [
      { slot: 1, driver_id: "d1", name: "Luke Brown", races: 13, best_position: 1, wins: 3, podiums: 6, first_year: "2024", last_year: "2026", country: "GB" },
      { slot: 2, driver_id: "d2", name: "Ana Duarte", races: 13, best_position: 2, wins: 0, podiums: 2, first_year: "2024", last_year: "2026" },
    ],
  };

  function abrirEsportivo() {
    invoke.mockResolvedValue(dossieCom([], DADOS));
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

  const BLOCOS = [
    "team-history-reliability",
    "team-history-spread",
    "team-history-curve",
    "team-history-recent-form",
    "team-history-lineup",
    "team-history-best-drivers",
  ];

  it("abre no arranjo arrumado, com os seis blocos sob os três títulos de grupo", async () => {
    abrirEsportivo();
    await screen.findByTestId("team-history-reliability");

    expect(screen.getByText("Como a equipe termina")).toBeInTheDocument();
    expect(screen.getByText("Como a equipe evolui")).toBeInTheDocument();
    expect(screen.getByText("Quem correu por ela")).toBeInTheDocument();
    BLOCOS.forEach((id) => expect(screen.getByTestId(id)).toBeInTheDocument());

    // Confiabilidade e assinatura são a mesma figura: no arrumado elas dividem
    // a mesma linha, e é isso que o pareamento tem de garantir.
    const linha = screen.getByTestId("team-history-reliability").parentElement.parentElement;
    expect(linha).toContain(screen.getByTestId("team-history-spread"));
  });

  // O empilhamento clássico e o botão que levava até ele foram removidos: a
  // seção tem um arranjo só, e a faixa do topo voltou a ser conteúdo.
  it("não oferece mais alternador de layout", async () => {
    abrirEsportivo();
    await screen.findByTestId("team-history-reliability");

    expect(screen.queryByTestId("team-history-sport-layout")).not.toBeInTheDocument();
  });
});

// A campanha do campeonato: a equipe do dossiê contra o campo, rodada a rodada.
// O contrato é que TODAS as equipes viram linha e só uma acende — sem isso o
// gráfico volta a ser um traço subindo, que é o que toda linha acumulada faz.
describe("TeamHistoryDrawerV2 — campanha do campeonato", () => {
  beforeEach(() => {
    invoke.mockReset();
    localStorage.clear();
  });

  const CAMPANHA = {
    year: "2026",
    category: "Mazda Championship",
    category_id: "mazda_rookie",
    live: true,
    rounds: [1, 2, 3],
    lines: [
      { team_id: "T2", team: "Falcon", selected: false, position: 1, total: "60", points: [25, 43, 60] },
      { team_id: "T1", team: "Track Day Heroes", selected: true, position: 2, total: "48", points: [18, 33, 48] },
      { team_id: "T3", team: "Isar Track", selected: false, position: 3, total: "20", points: [12, 20, 20] },
    ],
  };

  function abrirEsportivo(extra) {
    invoke.mockResolvedValue(dossieCom([], extra));
    return render(
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

  it("desenha uma linha por equipe e acende só a do dossiê", async () => {
    abrirEsportivo({ championship_run: CAMPANHA });

    const grafico = await screen.findByTestId("team-history-championship-run");
    const linhas = [...grafico.querySelectorAll("[data-line]")];
    expect(linhas.map((el) => el.dataset.line).sort()).toEqual(["T1", "T2", "T3"]);
    // Uma só acesa, e é a última desenhada — nenhuma linha do campo passa por cima dela.
    const acesas = linhas.filter((el) => el.dataset.selected === "true");
    expect(acesas).toHaveLength(1);
    expect(acesas[0].dataset.line).toBe("T1");
    expect(linhas[linhas.length - 1]).toBe(acesas[0]);

    // A colocação é o veredito e vira pílula, em vez de sair de contar linhas.
    expect(screen.getByTestId("team-history-run-position").textContent).toMatch(/P2/);
    expect(screen.getByText(/2026/)).toBeInTheDocument();
  });

  // A campanha e a fita de forma recente desenham as MESMAS corridas — uma
  // somada contra o grid, a outra uma a uma. A rodada é o que elas têm em comum,
  // e sem o elo achar na curva a corrida do quadradinho era contar no eixo.
  it("acende a mesma rodada na campanha e na fita de forma recente, nos dois sentidos", async () => {
    abrirEsportivo({
      championship_run: CAMPANHA,
      recent_form: [
        { year: "2026", round: 1, category: "Mazda Championship", category_id: "mazda_rookie", position: 4 },
        { year: "2026", round: 2, category: "Mazda Championship", category_id: "mazda_rookie", position: 1 },
        { year: "2026", round: 3, category: "Mazda Championship", category_id: "mazda_rookie", position: 2 },
      ],
    });

    const grafico = await screen.findByTestId("team-history-championship-run");
    const fita = screen.getByTestId("team-history-recent-form");
    const quadrado = (chave) => fita.querySelector(`[data-round='${chave}']`);
    expect(screen.queryByTestId("team-history-run-round-mark")).not.toBeInTheDocument();

    // Fita → campanha: o fio vertical cai no x da rodada 2, que é o meio do
    // eixo de três rodadas.
    fireEvent.mouseEnter(quadrado("2026-2"));
    const marca = screen.getByTestId("team-history-run-round-mark");
    const fio = marca.querySelector("line");
    const x = Number(fio.getAttribute("x1"));
    expect(x).toBeGreaterThan(Number(grafico.querySelector("[data-round-band='2026-1']").getAttribute("x")));
    expect(x).toBeLessThan(Number(grafico.querySelector("[data-round-band='2026-3']").getAttribute("x")));
    // E o marcador cheio pousa na linha da equipe do dossiê.
    expect(marca.querySelector("circle")).toBeInTheDocument();

    fireEvent.mouseLeave(quadrado("2026-2"));
    expect(screen.queryByTestId("team-history-run-round-mark")).not.toBeInTheDocument();

    // Campanha → fita: a faixa invisível da rodada acende o quadradinho dela.
    fireEvent.mouseEnter(grafico.querySelector("[data-round-band='2026-3']"));
    expect(quadrado("2026-3")).toHaveAttribute("data-aceso", "true");
    expect(quadrado("2026-1")).not.toHaveAttribute("data-aceso");
    fireEvent.mouseLeave(grafico.querySelector("[data-round-band='2026-3']"));
    expect(quadrado("2026-3")).not.toHaveAttribute("data-aceso");
  });

  // A fita atravessa temporadas; a campanha é de uma só. Acender pela rodada sem
  // olhar o ano faria a rodada 2 do ano passado marcar a rodada 2 deste.
  it("não acende a campanha por uma corrida de outra temporada", async () => {
    abrirEsportivo({
      championship_run: CAMPANHA,
      recent_form: [
        { year: "2025", round: 2, category: "Mazda Championship", category_id: "mazda_rookie", position: 6 },
        { year: "2026", round: 2, category: "Mazda Championship", category_id: "mazda_rookie", position: 1 },
      ],
    });

    const fita = await screen.findByTestId("team-history-recent-form");
    fireEvent.mouseEnter(fita.querySelector("[data-round='2025-2']"));
    // O quadradinho acende — a pergunta foi feita —, mas o gráfico não responde
    // por uma corrida que não está nele.
    expect(fita.querySelector("[data-round='2025-2']")).toHaveAttribute("data-aceso", "true");
    expect(screen.queryByTestId("team-history-run-round-mark")).not.toBeInTheDocument();
  });

  const ultimoY = (el) => Number(el.getAttribute("d").split(" ").pop().split(",")[1]);

  // Colocação é o modo padrão porque é o que descomprime: em pontos, um líder
  // disparado come o eixo sozinho e o pelotão vira um feixe de retas coladas;
  // em colocação cada equipe ocupa uma faixa da altura, por construção.
  //
  // A prova é a distância entre vizinhos de tabela. Aqui T1 (48) e T3 (20) estão
  // a 28 pontos um do outro num eixo que vai até 60 — menos de metade da altura —
  // e são vizinhos diretos na classificação, a exatamente uma faixa de distância.
  it("abre em colocação e reparte a altura igualmente entre as equipes", async () => {
    abrirEsportivo({ championship_run: CAMPANHA });

    const grafico = await screen.findByTestId("team-history-championship-run");
    const [p1, p2, p3] = ["T2", "T1", "T3"].map((id) => ultimoY(grafico.querySelector(`[data-line='${id}']`)));
    // P1 no topo, P3 embaixo, e os degraus entre eles são idênticos.
    expect(p1).toBeLessThan(p2);
    expect(p2).toBeLessThan(p3);
    expect(p2 - p1).toBeCloseTo(p3 - p2, 1);
    expect(grafico.querySelector("[data-line='T1']").getAttribute("d")).not.toContain("NaN");
  });

  // O modo de pontos continua a um clique — é a leitura que o eixo de colocação
  // descarta: colocação não diz se a briga é de dois pontos ou de duzentos.
  it("troca para pontos e escala o eixo pelo líder", async () => {
    abrirEsportivo({ championship_run: CAMPANHA });

    const seletor = await screen.findByTestId("team-history-run-mode");
    fireEvent.click(within(seletor).getByText("Pontos"));

    const grafico = screen.getByTestId("team-history-championship-run");
    const lider = grafico.querySelector("[data-line='T2']");
    const equipe = grafico.querySelector("[data-line='T1']");
    // Menos pontos = mais baixo no desenho (y cresce para baixo).
    expect(ultimoY(equipe)).toBeGreaterThan(ultimoY(lider));
    // 60 é o total do líder: o topo do eixo é ele, e a linha dele o encosta.
    expect(ultimoY(lider)).toBeCloseTo(16, 0);
  });

  const DUAS_TEMPORADAS = [
    { year: "2024", category: "Mazda Championship", category_id: "mazda_rookie", position: "P2", races: 8, wins: 4, podiums: 8, points: "224" },
    { year: "2025", category: "Mazda Championship", category_id: "mazda_rookie", position: "P1", races: 8, wins: 7, podiums: 8, points: "215" },
  ];

  // O bloco é UM sistema com dois eixos de escolha: QUANDO (entre campeonatos ·
  // campeonato atual) e O QUÊ (colocação · pontos). A métrica morava dentro da
  // campanha, então trocar de vista fazia um segundo seletor e uma pílula
  // aparecerem do nada — e as duas vistas pareciam blocos diferentes.
  it("mantem a metrica escolhida ao trocar de vista, e a curva desenha pontos", async () => {
    abrirEsportivo({ championship_run: CAMPANHA, season_results: DUAS_TEMPORADAS });

    await screen.findByTestId("team-history-curve");
    // O seletor de métrica existe na curva também, e no mesmo lugar.
    fireEvent.click(within(screen.getByTestId("team-history-run-mode")).getByText("Pontos"));

    // 2024 fez 224 pontos e 2025 fez 215: em pontos o ano de MAIS pontos sobe,
    // mesmo tendo sido o pior no campeonato (P2 contra P1).
    const curva = screen.getByTestId("team-history-curve");
    const cy = (ano) => Number(curva.querySelector(`[data-season="${ano}"]`).getAttribute("cy"));
    expect(cy("2024")).toBeLessThan(cy("2025"));

    // A escolha atravessa a troca de escala — é a métrica do bloco, não do gráfico.
    fireEvent.click(within(screen.getByTestId("team-history-evolution-view")).getByText("Campeonato atual"));
    expect(
      screen.getByTestId("team-history-run-mode").querySelector('[data-mode="pontos"]'),
    ).toHaveAttribute("data-active", "true");
  });

  // O bloco tem duas vistas do mesmo assunto, e a curva entre campeonatos é o
  // padrão: panorama antes do zoom. A campanha do ano corrente fica a um clique.
  it("abre na curva entre campeonatos e troca para a campanha pelo seletor", async () => {
    abrirEsportivo({ championship_run: CAMPANHA, season_results: DUAS_TEMPORADAS });

    await screen.findByTestId("team-history-curve");
    expect(screen.queryByTestId("team-history-championship-run")).not.toBeInTheDocument();

    const seletor = screen.getByTestId("team-history-evolution-view");
    fireEvent.click(within(seletor).getByText("Campeonato atual"));

    expect(screen.getByTestId("team-history-championship-run")).toBeInTheDocument();
    expect(screen.queryByTestId("team-history-curve")).not.toBeInTheDocument();
  });

  // O uso real do gráfico é COMPARAR equipes, e o caminho até a próxima equipe
  // desmonta o bloco: a aba inativa sai da árvore e fechar o dossiê leva o drawer
  // junto. Com a escolha só no `useState`, o gráfico voltava para "entre
  // campeonatos" no meio da comparação — a pergunta do jogador era esquecida a
  // cada equipe. As duas escolhas são preferência de leitura, e persistem.
  it("preserva vista e métrica quando o bloco desmonta e volta", async () => {
    const primeira = abrirEsportivo({ championship_run: CAMPANHA, season_results: DUAS_TEMPORADAS });

    await screen.findByTestId("team-history-curve");
    fireEvent.click(within(screen.getByTestId("team-history-run-mode")).getByText("Pontos"));
    fireEvent.click(within(screen.getByTestId("team-history-evolution-view")).getByText("Campeonato atual"));
    primeira.unmount();

    abrirEsportivo({ championship_run: CAMPANHA, season_results: DUAS_TEMPORADAS });

    // Volta na campanha, e em pontos: os dois eixos de escolha atravessam.
    await screen.findByTestId("team-history-championship-run");
    expect(screen.queryByTestId("team-history-curve")).not.toBeInTheDocument();
    expect(
      screen.getByTestId("team-history-run-mode").querySelector('[data-mode="pontos"]'),
    ).toHaveAttribute("data-active", "true");
  });

  // Sem campanha o bloco não some: a curva de posição por temporada é a reserva.
  it("cai na curva de posição quando não há campanha para desenhar", async () => {
    abrirEsportivo({
      season_results: [
        { year: "2024", category: "Mazda Championship", category_id: "mazda_rookie", position: "P2", races: 8, wins: 4, podiums: 8, points: "224" },
        { year: "2025", category: "Mazda Championship", category_id: "mazda_rookie", position: "P1", races: 8, wins: 7, podiums: 8, points: "215" },
      ],
    });

    await screen.findByTestId("team-history-curve");
    expect(screen.queryByTestId("team-history-championship-run")).not.toBeInTheDocument();
    // Uma vista só não gera seletor: um segmentado de um botão promete escolha
    // que não existe.
    expect(screen.queryByTestId("team-history-evolution-view")).not.toBeInTheDocument();
  });

});

// Os cards de record são a porta para a tabela de recordes de equipes. O que o
// clique carrega é o contrato: a métrica define a ordenação de chegada, e a
// categoria define o RECORTE — o card diz "11º de 19" dentro de um grupo, e a
// tabela precisa abrir no mesmo grupo para o 11º continuar sendo o 11º.
describe("TeamHistoryDrawerV2 — cards de record abrem o ranking", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  function abrirRecords(onOpenRecordsRanking) {
    invoke.mockResolvedValue(dossieCom([]));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="gt3"
        activeTab="records"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onOpenRecordsRanking={onOpenRecordsRanking}
        onClose={() => {}}
      />,
    );
  }

  it("manda métrica, categoria e equipe ao clicar num card", async () => {
    const abrir = vi.fn();
    abrirRecords(abrir);

    const botao = await screen.findByTestId("team-history-record-open-wins");
    fireEvent.click(botao);

    // A classe vai junto: numa multiclasse é ela que diz em qual dos
    // campeonatos da categoria a equipe corre.
    expect(abrir).toHaveBeenCalledWith({
      metric: "wins",
      category: "gt3",
      teamClass: "",
      teamId: "T1",
    });
  });

  it("abre pela métrica de cada card, e não sempre pela primeira", async () => {
    const abrir = vi.fn();
    abrirRecords(abrir);

    fireEvent.click(await screen.findByTestId("team-history-record-open-podium_rate"));
    expect(abrir).toHaveBeenCalledWith(expect.objectContaining({ metric: "podium_rate" }));
  });

  // O dossiê também abre fora do Dashboard (overlay de pré-temporada), onde não
  // há aba para onde navegar. Ali o card continua card.
  it("não vira botão sem destino", async () => {
    invoke.mockResolvedValue(dossieCom([]));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="gt3"
        activeTab="records"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onClose={() => {}}
      />,
    );

    // Os cards existem; o que não existe é o botão dentro deles.
    await waitFor(() => expect(document.querySelectorAll("[data-record]")).toHaveLength(5));
    expect(screen.queryAllByTestId(/^team-history-record-open-/)).toHaveLength(0);
  });
});

describe("TeamHistoryDrawerV2 — Gestão sem temporada medida", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  function abrirGestao(management) {
    invoke.mockResolvedValue(dossieCom([], { management }));
    render(
      <TeamHistoryDrawerV2
        careerId="c1"
        team={EQUIPE}
        teams={[EQUIPE]}
        playerTeam={null}
        activeCategory="gt3"
        activeTab="management"
        onTabChange={() => {}}
        onSelectTeam={() => {}}
        onClose={() => {}}
      />,
    );
  }

  // Carreira que ainda não correu: o livro-caixa EXISTE (o sorteio histórico grava
  // o prêmio de construtores), mas nenhuma temporada foi medida, então `rounds` é
  // 0 e o backend manda a frase que explica a causa. O normalizador anulava o
  // ledger inteiro por causa do zero, a frase nunca chegava, e a aba caía nos
  // cards de retrato atual — igualzinha à versão anterior ao livro-caixa.
  it("mostra a frase do backend em vez de sumir com o bloco", async () => {
    const nota =
      "Nenhuma temporada jogada ainda. As temporadas anteriores à carreira registram só o prêmio de construtores.";
    abrirGestao({
      operation_health: "Estável",
      peak_cash: "$16.864.526",
      ledger: { rounds: 0, seasons: 0, flow_seasons: 0, flow_note: nota, cash_curve: [] },
    });

    const bloco = await screen.findByTestId("team-history-money-flow");
    expect(within(bloco).getByText(nota)).toBeInTheDocument();
    // Sem série não há curva: um ponto solto anunciaria uma trajetória inexistente.
    expect(screen.queryByTestId("team-history-money-flow-chart")).toBeNull();
  });

  it("omite o bloco quando o save não tem livro-caixa nenhum", async () => {
    abrirGestao({ operation_health: "Estável", peak_cash: "$16.864.526", ledger: null });

    expect(await screen.findByText("Estável")).toBeInTheDocument();
    expect(screen.queryByTestId("team-history-money-flow")).toBeNull();
  });
});
