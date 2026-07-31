import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import TeamRecordsTab from "./TeamRecordsTab";

let mockState = {};

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Por padrão os totais de carreira são iguais aos do recorte — é a equipe que só
// correu aqui, e nela a tela não desenha o segundo número. Quem quiser testar o
// "5/87" passa os `total_*` explicitamente.
function linha(team_id, team, extra = {}) {
  const base = {
    team_id,
    team,
    color: "#58a6ff",
    category: "GT3",
    category_id: "gt3",
    active: true,
    titles: 0,
    wins: 0,
    podiums: 0,
    races: 40,
    podium_rate: 0,
    win_rate: 0,
    first_year: "2020",
    last_year: "2026",
    ...extra,
  };
  return {
    total_titles: base.titles,
    total_wins: base.wins,
    total_podiums: base.podiums,
    total_races: base.races,
    ...base,
  };
}

const PAYLOAD = {
  scope: "Grupo GT3",
  category: "gt3",
  scope_kind: "group",
  categories: [
    { key: "mazda_rookie", id: "mazda_rookie", class: "", label: "Mazda Rookie", group_label: "Grupo Mazda" },
    { key: "mazda_amador", id: "mazda_amador", class: "", label: "Mazda Championship", group_label: "Grupo Mazda" },
    { key: "production_challenger:mazda", id: "production_challenger", class: "mazda", label: "Production · Mazda", group_label: "Grupo Production" },
    { key: "production_challenger:toyota", id: "production_challenger", class: "toyota", label: "Production · Toyota", group_label: "Grupo Production" },
    { key: "gt3", id: "gt3", class: "", label: "GT3", group_label: "Grupo GT3" },
  ],
  rows: [
    linha("T1", "Track Day Heroes", { titles: 1, wins: 2, podiums: 20, podium_rate: 6, win_rate: 1 }),
    linha("T2", "Falcon", { titles: 4, wins: 30, podiums: 60, podium_rate: 40, win_rate: 20 }),
    linha("T3", "Isar Track", { titles: 0, wins: 12, podiums: 44, podium_rate: 30, win_rate: 8 }),
  ],
};

function abrir(props = {}) {
  mockState = { careerId: "c1" };
  invoke.mockResolvedValue(PAYLOAD);
  render(<TeamRecordsTab category="gt3" {...props} />);
}

function idsDasLinhas() {
  return [...screen.getByTestId("team-records-table").querySelectorAll("tbody tr")].map(
    (row) => row.dataset.team,
  );
}

describe("TeamRecordsTab", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  // O ponto inteiro da tela: chegar do card de "vitórias" e já encontrar a lista
  // ordenada por vitórias, sem um clique a mais.
  it("chega ordenada pela métrica que veio do card", async () => {
    abrir({ metric: "wins" });

    await waitFor(() => expect(idsDasLinhas()).toEqual(["T2", "T3", "T1"]));
    // Abre na CATEGORIA da ficha que estava aberta: quem clica num record quer
    // saber como a equipe se compara com quem corre com ela.
    expect(invoke).toHaveBeenCalledWith("get_team_records_ranking", {
      careerId: "c1",
      category: "gt3",
      scope: "category",
      class: null,
    });
  });

  // Métrica diferente, ordem diferente — e as taxas não seguem as contagens: a
  // Isar tem menos vitórias que a Falcon e ainda assim mais pódios por corrida
  // que a Track Day, que é o que a coluna de taxa existe para mostrar.
  it("reordena pela métrica clicada no cabeçalho", async () => {
    abrir({ metric: "titles" });

    await waitFor(() => expect(idsDasLinhas()).toEqual(["T2", "T1", "T3"]));

    fireEvent.click(screen.getByTestId("team-records-table").querySelector("[data-sort='podium_rate']"));
    expect(idsDasLinhas()).toEqual(["T2", "T3", "T1"]);
  });

  // O recorte é o do card, e a legenda diz qual é — sem isso o jogador não tem
  // como saber contra quem os números estão sendo comparados.
  it("anuncia o recorte de comparação devolvido pelo backend", async () => {
    abrir({ metric: "wins", highlightTeamId: "T1" });

    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 2 }).textContent).toBe("Grupo GT3 · 3 equipes"),
    );
    // A equipe de onde o clique veio fica marcada: numa lista de 19 ela some.
    const marcada = screen.getByTestId("team-records-table").querySelector("[data-highlighted='true']");
    expect(marcada.dataset.team).toBe("T1");
  });

  // Trocar a categoria refaz a consulta; é o mesmo comando com outro recorte.
  it("refaz a consulta ao trocar de categoria", async () => {
    abrir({ metric: "wins" });
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));

    fireEvent.change(screen.getByTestId("team-records-category"), { target: { value: "mazda_amador" } });
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("get_team_records_ranking", {
        careerId: "c1",
        category: "mazda_amador",
        scope: "category",
        class: null,
      }),
    );
  });
});

describe("TeamRecordsTab — colunas", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("mostra contagens e taxas na mesma linha da equipe", async () => {
    abrir({ metric: "wins" });

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(3));
    const linhaFalcon = screen.getByTestId("team-records-table").querySelector("[data-team='T2']");
    const celulas = [...linhaFalcon.querySelectorAll("td")].map((td) => td.textContent);
    // #, equipe, período, títulos, vitórias, pódios, corridas, taxa de pódio, taxa de vitória.
    expect(celulas.slice(2)).toEqual(["2020–2026", "4", "30", "60", "40", "40%", "20%"]);
    expect(within(linhaFalcon).getByText("Falcon")).toBeInTheDocument();
  });

  // O período é o que dá escala à contagem: "5 corridas" sozinho não diz se a
  // equipe é nova, se passou correndo ou se está lá há uma década. E ele vem do
  // RECORTE, então numa equipe que subiu depois de cinco corridas ele mostra os
  // anos dessas cinco, não os da carreira inteira.
  it("mostra o período da equipe dentro do recorte", async () => {
    mockState = { careerId: "c1" };
    invoke.mockResolvedValue({
      ...PAYLOAD,
      rows: [
        linha("T4", "Rookfield", { races: 5, first_year: "2024", last_year: "2025" }),
        linha("T5", "Overland", { races: 3, first_year: "2026", last_year: "2026" }),
      ],
    });
    render(<TeamRecordsTab category="mazda_rookie" metric="wins" />);

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(2));
    const tabela = screen.getByTestId("team-records-table");
    expect(tabela.querySelector("[data-team='T4'] [data-span]").textContent).toBe("2024–2025");
    // Ano único não vira intervalo: "2026–2026" seria ruído com cara de dado.
    expect(tabela.querySelector("[data-team='T5'] [data-span]").textContent).toBe("2026");
  });
});

// A tela não está no menu: ela é resposta a uma pergunta feita no dossiê, e o
// caminho de volta é a única saída. Sem `onBack` (montada solta, em teste ou em
// outro contexto) o botão não aparece em vez de aparecer sem levar a lugar nenhum.
describe("TeamRecordsTab — volta", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("oferece a volta para a tela de origem", async () => {
    const voltar = vi.fn();
    abrir({ metric: "wins", onBack: voltar });

    fireEvent.click(await screen.findByTestId("team-records-back"));
    expect(voltar).toHaveBeenCalled();
  });

  it("não desenha o botão de volta sem destino", async () => {
    abrir({ metric: "wins" });

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(3));
    expect(screen.queryByTestId("team-records-back")).not.toBeInTheDocument();
  });
});

// O filtro tem duas perguntas: qual categoria, e com que amplitude. A segunda
// faltava, e sem ela o seletor mentia — escolher "Mazda Rookie" trazia também a
// Mazda Championship, porque a comparação sempre foi por grupo.
describe("TeamRecordsTab — amplitude", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  // Rookie e Championship são entradas distintas, e a Production vira UMA POR
  // CLASSE: ela não é um campeonato, são três correndo na mesma pista, e
  // escolher "Production" inteira misturava Mazda, Toyota e BMW num número só.
  it("oferece a escada com as multiclasse abertas por carro", async () => {
    abrir({ metric: "wins" });

    const seletor = await screen.findByTestId("team-records-category");
    await waitFor(() =>
      expect([...seletor.options].map((option) => option.textContent)).toEqual([
        "Mazda Rookie",
        "Mazda Championship",
        "Production · Mazda",
        "Production · Toyota",
        "GT3",
      ]),
    );
  });

  it("manda categoria e classe ao escolher um campeonato da Production", async () => {
    abrir({ metric: "wins" });
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));

    fireEvent.change(screen.getByTestId("team-records-category"), {
      target: { value: "production_challenger:toyota" },
    });
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("get_team_records_ranking", {
        careerId: "c1",
        category: "production_challenger",
        scope: "category",
        class: "toyota",
      }),
    );
  });

  // Chegar pela ficha de uma equipe de Production já cai no campeonato DELA, e
  // não numa das três escolhida por sorteio.
  it("abre na classe da equipe quando a ficha veio de uma multiclasse", async () => {
    mockState = { careerId: "c1" };
    invoke.mockResolvedValue(PAYLOAD);
    render(<TeamRecordsTab category="production_challenger" teamClass="mazda" metric="wins" />);

    await waitFor(() =>
      expect(screen.getByTestId("team-records-category").value).toBe("production_challenger:mazda"),
    );
  });

  it("consulta só a categoria, o grupo ou o mundo conforme a amplitude", async () => {
    abrir({ metric: "wins" });
    const controle = await screen.findByTestId("team-records-scope");
    // Chega em "só a categoria"; alargar é o que precisa de clique.
    expect(controle.querySelector("[data-active='true']").dataset.scope).toBe("category");

    fireEvent.click(within(controle).getByText("Grupo GT3"));
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("get_team_records_ranking", {
        careerId: "c1",
        category: "gt3",
        scope: "group",
        class: null,
      }),
    );

    fireEvent.click(within(controle).getByText("Mundo"));
    await waitFor(() =>
      expect(invoke).toHaveBeenLastCalledWith("get_team_records_ranking", {
        careerId: "c1",
        category: "gt3",
        scope: "world",
        class: null,
      }),
    );
    // No mundo a categoria não decide nada, e um seletor que não decide nada
    // ligado é um convite a achar que decide.
    expect(screen.getByTestId("team-records-category")).toBeDisabled();
  });

  // "Grupo" sozinho não diz o que entra na conta. O botão mostra o nome do grupo
  // DESTA categoria, que é o que torna a escolha informada.
  it("nomeia o grupo da categoria selecionada no próprio botão", async () => {
    abrir({ metric: "wins" });

    const controle = await screen.findByTestId("team-records-scope");
    await waitFor(() => expect(within(controle).getByText("Grupo GT3")).toBeInTheDocument());

    fireEvent.change(screen.getByTestId("team-records-category"), { target: { value: "mazda_rookie" } });
    expect(within(controle).getByText("Grupo Mazda")).toBeInTheDocument();
  });

  // A categoria da linha é onde a equipe corre HOJE, não de onde vieram os
  // números — sem o "Hoje na", um time promovido lia como se os títulos fossem
  // da categoria nova.
  it("diz que a categoria da linha é a de hoje, e explica o recorte", async () => {
    abrir({ metric: "wins" });

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(3));
    const linha = screen.getByTestId("team-records-table").querySelector("[data-team='T2']");
    expect(within(linha).getByText("Hoje na GT3")).toBeInTheDocument();
    expect(screen.getByText(/Escadas equivalentes entram juntas/)).toBeInTheDocument();
  });

  // A nota de rodapé segue a amplitude: no mundo, o aviso deixa de ser sobre
  // escadas equivalentes e passa a ser sobre somar títulos incomparáveis.
  it("troca a explicação conforme a amplitude aplicada", async () => {
    mockState = { careerId: "c1" };
    invoke.mockResolvedValue({ ...PAYLOAD, scope: "Mundo inteiro", scope_kind: "world" });
    render(<TeamRecordsTab category="gt3" metric="wins" />);

    await waitFor(() => expect(screen.getByText(/pesam igual aqui/)).toBeInTheDocument());
    expect(screen.queryByText(/Escadas equivalentes entram juntas/)).not.toBeInTheDocument();
  });
});

// O segundo número é o que deixa o filtro visível NO DADO. "5" solto se parece
// com uma equipe que mal correu; "5/87" diz que ela correu 87 vezes e só 5 aqui.
describe("TeamRecordsTab — total de carreira", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  function abrirComTotais() {
    mockState = { careerId: "c1" };
    invoke.mockResolvedValue({
      ...PAYLOAD,
      scope: "Mazda Rookie",
      scope_kind: "category",
      rows: [
        linha("T4", "Rookfield", {
          races: 5,
          wins: 1,
          titles: 0,
          total_races: 87,
          total_wins: 14,
          total_titles: 2,
        }),
      ],
    });
    render(<TeamRecordsTab category="mazda_rookie" metric="wins" />);
  }

  it("mostra o total da carreira ao lado do número do recorte", async () => {
    abrirComTotais();

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(1));
    const linhaRook = screen.getByTestId("team-records-table").querySelector("[data-team='T4']");
    expect(linhaRook.querySelector("[data-metric='races']").textContent).toBe("5/87");
    expect(linhaRook.querySelector("[data-metric='wins']").textContent).toBe("1/14");
    expect(linhaRook.querySelector("[data-metric='titles']").textContent).toBe("0/2");
    // A legenda explica o número menor; sem ela "5/87" é uma fração sem unidade.
    expect(screen.getByText(/total da carreira, em todas as categorias/)).toBeInTheDocument();
  });

  // Taxa não tem par: proporção não se lê como fração de outra proporção.
  it("não pareia as taxas", async () => {
    abrirComTotais();

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(1));
    const linhaRook = screen.getByTestId("team-records-table").querySelector("[data-team='T4']");
    expect(linhaRook.querySelector("[data-metric='podium_rate'] [data-total]")).toBeNull();
  });

  // Equipe que só correu neste recorte não ganha "/40" ao lado de "40" — isso
  // seria ruído em toda a tabela, e ensinaria a ignorar o número que existe para
  // chamar atenção.
  it("omite o total quando ele é igual ao do recorte", async () => {
    abrir({ metric: "wins" });

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(3));
    expect(screen.getByTestId("team-records-table").querySelectorAll("[data-total]")).toHaveLength(0);
  });
});

// "Grupo" não tem tamanho fixo, e essa é a origem da confusão: o Grupo Mazda são
// duas categorias e o Grupo Production são seis, porque a Production é onde as
// escadas de entrada convergem. Alargar a partir da Mazda parece deixar coisa de
// fora quando a lista não está à vista.
describe("TeamRecordsTab — do que o grupo é feito", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  function abrirGrupo(scope, scope_categories) {
    mockState = { careerId: "c1" };
    invoke.mockResolvedValue({ ...PAYLOAD, scope, scope_kind: "group", scope_categories });
    render(<TeamRecordsTab category="mazda_rookie" metric="titles" />);
  }

  it("nomeia as categorias que entraram na conta", async () => {
    abrirGrupo("Grupo Mazda", ["Mazda Rookie", "Mazda Championship"]);

    const nota = await screen.findByTestId("team-records-scope-members");
    expect(nota.textContent).toContain("2 categorias");
    expect(nota.textContent).toContain("Mazda Rookie, Mazda Championship");
  });

  it("mostra o grupo maior da Production com as seis categorias", async () => {
    abrirGrupo("Grupo Production", [
      "Mazda Rookie",
      "Mazda Championship",
      "Toyota Rookie",
      "Toyota Cup",
      "BMW M2",
      "Production",
    ]);

    const nota = await screen.findByTestId("team-records-scope-members");
    expect(nota.textContent).toContain("6 categorias");
    expect(nota.textContent).toContain("BMW M2");
  });

  // Grupo de uma categoria só (GT3, LMP2) não ganha lista: "Entram na conta 1
  // categoria: GT3" ao lado de um filtro escrito GT3 é a mesma frase duas vezes.
  it("cala quando o grupo é a própria categoria", async () => {
    abrirGrupo("Grupo GT3", ["GT3"]);

    await waitFor(() => expect(idsDasLinhas()).toHaveLength(3));
    expect(screen.queryByTestId("team-records-scope-members")).not.toBeInTheDocument();
  });
});

// A escada da marca vai até a Production, que é multiclasse: três marcas
// disputam a MESMA categoria em campeonatos separados. A Production aparece na
// lista de categorias, e sem esta frase parece que Toyota e BMW entraram junto.
describe("TeamRecordsTab — recorte por marca", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("avisa que nas multiclasse só a classe da marca conta", async () => {
    mockState = { careerId: "c1" };
    invoke.mockResolvedValue({
      ...PAYLOAD,
      scope: "Grupo Mazda",
      scope_kind: "group",
      scope_categories: ["Mazda Rookie", "Mazda Championship", "Production"],
      scope_family: "mazda",
    });
    render(<TeamRecordsTab category="mazda_rookie" metric="titles" />);

    const nota = await screen.findByTestId("team-records-scope-members");
    expect(nota.textContent).toContain("Production");
    expect(nota.textContent).toContain("só a classe Mazda");
  });

  // Grupo sem marca (a Production, que é a convergência das três escadas) não
  // ganha a frase: não há classe recortando nada ali.
  it("cala quando o grupo não tem marca", async () => {
    mockState = { careerId: "c1" };
    invoke.mockResolvedValue({
      ...PAYLOAD,
      scope: "Grupo Production",
      scope_kind: "group",
      scope_categories: ["Mazda Rookie", "Mazda Championship", "Production"],
      scope_family: "",
    });
    render(<TeamRecordsTab category="production_challenger" metric="titles" />);

    const nota = await screen.findByTestId("team-records-scope-members");
    expect(nota.textContent).not.toContain("só a classe");
  });
});
