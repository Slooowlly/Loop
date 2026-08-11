import { render, screen, waitFor, act, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import RosterGenPanel from "./RosterGenPanel";

// O painel de exportação para o iRacing: a metade de IDA do ciclo exportar-correr-importar.
// 726 linhas e 12 pontos de `invoke`, sem nenhum teste até 11/08/2026 — a vistoria de 10/08
// marcou como [Alta] porque é a tela que escreve arquivo dentro da instalação do iRacing do
// jogador, e um argumento com nome trocado chega ao Rust como `undefined` sem erro nenhum.
//
// RESSALVA DE ALCANCE, medida em 11/08/2026: este componente NÃO É IMPORTADO por nenhum
// arquivo do frontend. A exportação que o jogador dispara hoje vem de
// `race/nextrace/useIracingExport.js`, que chama os MESMOS `iracing_generate_roster` e
// `iracing_generate_season` — este painel é a bancada anterior, com os controles de
// diagnóstico que a tela do jogador não tem. O roadmap registra os dois painéis como decisão
// pendente: ligar ou apagar. Os casos abaixo valem nos dois desfechos — travam o contrato dos
// comandos, que a tela viva também usa.
//
// O foco aqui é o CONTRATO, não o desenho: os nomes dos argumentos que atravessam a ponte, a
// carga que precisa sobreviver ao iRacing fechado, e o poll de conexão que não pode continuar
// depois de a aba fechar.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const SAVE = {
  career_id: "C1",
  player_name: "Piloto Teste",
  category: "gt3",
  difficulty: "medio",
};

/// Responde os comandos do painel. `sobrescreve` troca a resposta de um comando específico.
function responder(sobrescreve = {}) {
  invoke.mockImplementation((cmd, args) => {
    if (cmd in sobrescreve) {
      const v = sobrescreve[cmd];
      return typeof v === "function" ? v(args) : Promise.resolve(v);
    }
    if (cmd === "list_saves") return Promise.resolve([SAVE]);
    if (cmd === "iracing_connected") return Promise.resolve(false);
    if (cmd === "iracing_player_paint") return Promise.resolve({ cor: "#ff0000" });
    if (cmd === "iracing_player_custid") return Promise.resolve(123456);
    return Promise.resolve(null);
  });
}

/// O botão cujo rótulo casa a expressão. O painel tem uma fileira deles e o texto vem do i18n.
function botao(re) {
  return screen.getAllByRole("button").find((b) => re.test(b.textContent));
}

beforeEach(() => {
  invoke.mockReset();
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("carga", () => {
  it("lista os saves e seleciona o primeiro", async () => {
    responder();
    render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_saves"));
    // A categoria do save escolhido pré-preenche o campo: sem isso o jogador exportaria o
    // roster de uma categoria em que ele não corre.
    await waitFor(() => {
      const chamadas = invoke.mock.calls.filter(([c]) => c === "iracing_player_paint");
      expect(chamadas.length).toBeGreaterThan(0);
      expect(chamadas[0][1]).toEqual({ careerId: "C1" });
    });
  });

  it("mostra o erro quando a lista de saves falha", async () => {
    responder({ list_saves: () => Promise.reject("banco travado") });
    render(<RosterGenPanel />);
    await waitFor(() => expect(screen.getByText(/banco travado/)).toBeInTheDocument());
  });

  it("uma pintura indisponível não impede o resto do painel", async () => {
    // A cor do time é decoração; falhar nela não pode bloquear a exportação, que é o motivo
    // de a tela existir.
    responder({ iracing_player_paint: () => Promise.reject("sem pintura") });
    render(<RosterGenPanel />);
    await waitFor(() => expect(screen.getByText(/sem pintura/)).toBeInTheDocument());
    expect(botao(/roster/i)).toBeTruthy();
  });

  it("sobrevive a uma lista de saves vazia", async () => {
    responder({ list_saves: [] });
    const { container } = render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_saves"));
    expect(container).not.toBeEmptyDOMElement();
  });

  it("sobrevive a uma lista de saves nula", async () => {
    // O comando devolve `Option<Vec<_>>`; um `None` chega como `null` e um `.length` direto
    // derrubaria a tela na abertura.
    responder({ list_saves: null });
    const { container } = render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_saves"));
    expect(container).not.toBeEmptyDOMElement();
  });
});

describe("contrato dos comandos de exportação", () => {
  it("gera o roster com os quatro argumentos nomeados que o Rust espera", async () => {
    // Os nomes viajam como chave de objeto e o Rust os casa por nome. Renomear um aqui não
    // quebra nada visível: o campo chega `None` e o roster sai com o valor padrão.
    responder({ iracing_generate_roster: { path: "roster.json", drivers: 20 } });
    render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_saves"));
    await act(async () => {
      botao(/roster/i).click();
    });
    const chamada = invoke.mock.calls.find(([c]) => c === "iracing_generate_roster");
    expect(chamada, "o botão de roster não disparou o comando").toBeTruthy();
    expect(Object.keys(chamada[1]).sort()).toEqual(["carKey", "careerId", "categoria", "rosterName"]);
    expect(chamada[1].careerId).toBe("C1");
    expect(chamada[1].categoria).toBe("gt3");
  });

  it("gera a temporada com o alvo de pista NULO quando o campo está vazio", async () => {
    // O campo é texto livre. Mandar string vazia faria o Rust tentar `parse::<i64>` e falhar;
    // o painel converte para `null`, que é o "sem pista alvo" do contrato.
    responder({ iracing_generate_season: { path: "season.json" } });
    render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_saves"));
    expect(botao(/season|temporada/i), "o painel precisa ter o botão de gerar temporada")
      .toBeTruthy();
    // O botão é procurado DENTRO do `act`, como nos dois casos vizinhos. Guardar o nó antes
    // e clicar depois pegava a versão anterior ao commit que o `act` faz: o clique saía num
    // nó que já não estava mais na árvore e o comando nunca era disparado.
    await act(async () => {
      fireEvent.click(botao(/season|temporada/i));
    });
    const chamada = invoke.mock.calls.find(([c]) => c === "iracing_generate_season");
    expect(chamada, "o botão de temporada não disparou o comando").toBeTruthy();
    expect(chamada[1].targetTrackId).toBeNull();
  });

  it("mostra o erro de exportação em vez de falhar calado", async () => {
    // A exportação escreve na pasta do iRacing. Falha de permissão é o caso comum, e sem a
    // mensagem o jogador acha que exportou e vai correr com o grid velho.
    responder({ iracing_generate_roster: () => Promise.reject("acesso negado") });
    render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_saves"));
    await act(async () => {
      botao(/roster/i).click();
    });
    await waitFor(() => expect(screen.getByText(/acesso negado/)).toBeInTheDocument());
  });
});

describe("poll de conexão com o sim", () => {
  it("pergunta pelo custid só quando o sim está conectado", async () => {
    // `iracing_player_custid` só tem resposta com o sim aberto. Perguntar sempre encheria o
    // log de erro a cada 4 segundos com o iRacing fechado.
    responder({ iracing_connected: false });
    render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_connected"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(9000);
    });
    expect(invoke.mock.calls.filter(([c]) => c === "iracing_player_custid")).toHaveLength(0);
  });

  it("busca o custid quando o sim responde conectado", async () => {
    responder({ iracing_connected: true });
    render(<RosterGenPanel />);
    await waitFor(() =>
      expect(invoke.mock.calls.filter(([c]) => c === "iracing_player_custid").length).toBeGreaterThan(0),
    );
  });

  it("continua perguntando enquanto a aba está aberta", async () => {
    responder({ iracing_connected: false });
    render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_connected"));
    const antes = invoke.mock.calls.filter(([c]) => c === "iracing_connected").length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(12_500);
    });
    expect(invoke.mock.calls.filter(([c]) => c === "iracing_connected").length).toBeGreaterThan(antes);
  });

  it("para de perguntar quando a aba fecha", async () => {
    // O poll conversa com a memória compartilhada do SDK. Um intervalo sobrevivente continua
    // batendo nela depois de o jogador ter saído da tela.
    responder({ iracing_connected: false });
    const { unmount } = render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_connected"));
    unmount();
    const antes = invoke.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(12_000);
    });
    expect(invoke.mock.calls.length).toBe(antes);
  });

  it("um erro no poll não derruba o painel", async () => {
    responder({ iracing_connected: () => Promise.reject("shared memory fechada") });
    const { container } = render(<RosterGenPanel />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_connected"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(container).not.toBeEmptyDOMElement();
  });
});
