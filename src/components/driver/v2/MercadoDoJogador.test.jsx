import { render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";

import { MercadoDoJogador } from "./MercadoDoJogador.jsx";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

// Os dois blocos de mercado do jogador, montados sozinhos.
//
// Eles são o que sobrou da seção Mercado da aba Carreira (apagada em 14/08/2026), e
// são a única parte da ficha que busca dado do MUNDO em vez de ler o `detail`. Por
// isso este arquivo mocka o `invoke` e o `MarketSection.test.jsx` não precisa: lá a
// ficha é de piloto de IA, e sem `is_jogador` nada disto monta.
describe("MercadoDoJogador", () => {
  const vaga = (overrides = {}) => ({
    team_id: "t1",
    team_name: "Arclight",
    team_color: "#dc0000",
    categoria: "gt3",
    classe: null,
    papel: "Numero2",
    car_performance_rating: 78,
    licenca_ok: true,
    tier_ok: true,
    salario_estimado: 1300000,
    ...overrides,
  });

  function responder({ board = null, teamInterest = null } = {}) {
    invoke.mockImplementation((comando) => {
      if (comando === "get_season_market_board") return Promise.resolve(board);
      if (comando === "get_inbox_messages") return Promise.resolve({ team_interest: teamInterest });
      return Promise.resolve(null);
    });
  }

  beforeEach(() => {
    invoke.mockReset();
  });

  it("lista as vagas e conta quantas sao para o jogador", async () => {
    responder({
      board: {
        vagas: [vaga(), vaga({ team_id: "t2", team_name: "Nordwand", tier_ok: false, salario_estimado: null })],
        vagas_elegiveis: 1,
      },
    });

    render(<MercadoDoJogador careerId="c1" />);

    await waitFor(() => expect(screen.getByText("Arclight")).toBeInTheDocument());
    expect(screen.getByText("1 para você, de 2")).toBeInTheDocument();
    // Número cheio com sufixo anual, o mesmo das barras de custo da aba: a forma
    // compacta da aba Carreira ("$1.3M") faria duas grafias do mesmo tipo de número
    // conviverem na mesma tela.
    expect(screen.getByText("$1,300,000/ano")).toBeInTheDocument();
    // A vaga fora da faixa continua na lista, marcada, em vez de sumir: ver a
    // cadeira da categoria de cima e saber que ela ainda não é dele é o ponto.
    expect(screen.getByText("Fora da sua faixa")).toBeInTheDocument();
    const linhas = document.querySelectorAll("[data-elegivel]");
    expect([...linhas].map((li) => li.dataset.elegivel)).toEqual(["true", "false"]);
  });

  it("diz quem esta de olho, com o plural do numero de equipes", async () => {
    responder({
      teamInterest: { teams: [{ team_name: "Nordwand", category: "gt3" }, { team_name: "Kestrel", category: "gt3" }] },
    });

    render(<MercadoDoJogador careerId="c1" />);

    await waitFor(() => expect(screen.getByText("Nordwand")).toBeInTheDocument());
    expect(screen.getByText("2 equipes cobiçam o seu nome pelo apelo comercial.")).toBeInTheDocument();
  });

  it("sem interesse e sem vaga, explica o vazio em vez de deixar a caixa muda", async () => {
    responder({ board: { vagas: [], vagas_elegiveis: 0 } });

    render(<MercadoDoJogador careerId="c1" />);

    await waitFor(() =>
      expect(
        screen.getByText("Ninguém de olho por enquanto. Interesse ativo nasce da fama."),
      ).toBeInTheDocument(),
    );
    expect(screen.getByText("Nenhum assento aberto no mundo agora.")).toBeInTheDocument();
  });

  // A falha das duas buscas não pode derrubar a aba: o termômetro e a situação
  // contratual já estão desenhados quando isto chega, e nenhum dos dois depende
  // delas. O rastro fica no `loop.log` pelo `bestEffort`.
  it("com as duas buscas falhando, cai no estado vazio sem quebrar", async () => {
    invoke.mockImplementation((comando) => {
      if (comando === "diagnostico_registrar") return Promise.resolve(null);
      return Promise.reject("banco fora do ar");
    });

    render(<MercadoDoJogador careerId="c1" />);

    await waitFor(() =>
      expect(screen.getByText("Nenhum assento aberto no mundo agora.")).toBeInTheDocument(),
    );
    expect(invoke).toHaveBeenCalledWith("diagnostico_registrar", expect.anything());
  });
});
