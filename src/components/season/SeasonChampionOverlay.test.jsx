import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import PauseMenu from "../layout/PauseMenu";
import useCareerStore from "../../stores/useCareerStore";
import SeasonChampionOverlay from "./SeasonChampionOverlay";

// Payload mínimo no formato de `get_season_champion_payload` — dois pilotos, duas
// etapas. O overlay não abre sem `drivers`, então todo teste parte daqui.
const OVERLAY_STATE = {
  year: 2026,
  season_number: 1,
  category_id: "gt3",
  rounds: 2,
  player_is_champion: true,
  margin: 7,
  drivers: [
    {
      id: "D1",
      nome: "R. Silva",
      equipe: "Meridian GT",
      equipe_cor: "#F4C752",
      nacionalidade: "🇧🇷 Brasileiro",
      posicao: 1,
      pontos: 43,
      vitorias: 1,
      podios: 2,
      poles: 1,
      voltas_rapidas: 0,
      cumulative: [25, 43],
      is_champion: true,
      is_player: true,
    },
    {
      id: "D2",
      nome: "Yuki Tanaka",
      equipe: "Kaido Works",
      equipe_cor: "#9aa6b4",
      nacionalidade: "🇯🇵 Japonês",
      posicao: 2,
      pontos: 36,
      vitorias: 1,
      podios: 2,
      poles: 1,
      voltas_rapidas: 2,
      cumulative: [18, 36],
      is_champion: false,
      is_player: false,
    },
  ],
  standings: [
    { id: "D1", nome: "R. Silva", equipe: "Meridian GT", equipe_cor: "#F4C752", posicao: 1, pontos: 43, is_player: true },
    { id: "D2", nome: "Yuki Tanaka", equipe: "Kaido Works", equipe_cor: "#9aa6b4", posicao: 2, pontos: 36, is_player: false },
    { id: "D3", nome: "Marco Bianchi", equipe: "Scuderia Lume", equipe_cor: "#c8895a", posicao: 3, pontos: 20, is_player: false },
    { id: "D4", nome: "Felipe Duarte", equipe: "Verona Corse", equipe_cor: "#4f8fd8", posicao: 4, pontos: 12, is_player: false },
    // Sem cor de equipe: a lista miúda cai no traço neutro em vez de sumir.
    { id: "D5", nome: "Sara Lindqvist", equipe: "Verona Corse", equipe_cor: null, posicao: 5, pontos: 6, is_player: false },
  ],
  constructors: [
    {
      nome: "Meridian GT",
      cor: "#F4C752",
      posicao: 1,
      pontos: 61,
      vitorias: 2,
      podios: 3,
      poles: 1,
      voltas_rapidas: 0,
      cumulative: [30, 61],
      pilotos: [
        { id: "D1", nome: "R. Silva", pontos: 43, vitorias: 1, is_player: true },
        { id: "D6", nome: "Otto Vahl", pontos: 18, vitorias: 1, is_player: false },
      ],
      is_player_team: true,
    },
    {
      nome: "Kaido Works",
      cor: "#9aa6b4",
      posicao: 2,
      pontos: 36,
      vitorias: 0,
      podios: 2,
      poles: 1,
      voltas_rapidas: 2,
      cumulative: [18, 36],
      pilotos: [{ id: "D2", nome: "Yuki Tanaka", pontos: 36, vitorias: 0, is_player: false }],
      is_player_team: false,
    },
  ],
  // Cinco prêmios: quatro cabem no destaque e o quinto desce para a faixa miúda.
  awards: [
    {
      id: "grand_chelem",
      who: "R. Silva",
      who_id: "D1",
      is_player: true,
      args: { track: "Spa-Francorchamps" },
    },
    {
      id: "zebra",
      who: "Marco Bianchi",
      who_id: "D3",
      is_player: false,
      args: { grid: "9", track: "Circuit de Lédenon" },
    },
    {
      id: "regularidade",
      who: "Yuki Tanaka",
      who_id: "D2",
      is_player: false,
      args: { best: "2", worst: "4", count: "9" },
    },
    // O "quem" é uma PISTA: sem `who_id`, e por isso sem linha de contexto.
    {
      id: "etapa_do_ano",
      who: "Circuit de Lédenon",
      who_id: null,
      is_player: false,
      args: { count: "14", round: "6" },
    },
    // O "quem" é uma EQUIPE, e o valor interpolado pode vir quebrado (79.5).
    {
      id: "equipe_do_ano",
      who: "Meridian GT",
      who_id: null,
      is_player: true,
      args: { count: "79.5" },
    },
  ],
  records: [
    // Sem unidade no i18n, sem sufixo do backend.
    { id: "vitorias", who: "R. Silva", is_player: true, valor: "1", sufixo: null },
    // Com unidade fixa no i18n ("DNF").
    { id: "abandonos", who: "Yuki Tanaka", is_player: false, valor: "2", sufixo: null },
    // Com sufixo dinâmico do backend (a pista).
    {
      id: "maior_recuperacao",
      who: "Yuki Tanaka",
      is_player: false,
      valor: "+6",
      sufixo: "Circuit de Lédenon",
    },
  ],
};

function renderOverlay() {
  return render(<SeasonChampionOverlay />);
}

function expectClosed(container) {
  expect(container.querySelector(".champ-ov")).not.toBeInTheDocument();
  expect(useCareerStore.getState().championOverlay).toBe(null);
}

describe("SeasonChampionOverlay", () => {
  beforeEach(() => {
    useCareerStore.setState({
      championOverlay: { ...OVERLAY_STATE },
    });
  });

  afterEach(() => {
    cleanup();
    useCareerStore.setState({
      championOverlay: null,
    });
  });

  it("fecha em Continuar", () => {
    const { container } = renderOverlay();

    fireEvent.click(screen.getByRole("button", { name: /Continuar/i }));

    expectClosed(container);
  });

  it("fecha no botão Fechar", () => {
    const { container } = renderOverlay();

    fireEvent.click(screen.getByRole("button", { name: /Fechar/i }));

    expectClosed(container);
  });

  it("fecha ao clicar no backdrop", () => {
    const { container } = renderOverlay();

    fireEvent.click(container.querySelector(".champ-ov"));

    expectClosed(container);
  });

  it("captura Escape e cancela sua propagação", () => {
    const { container } = renderOverlay();
    const bubbleListener = vi.fn();
    const escapeEvent = new KeyboardEvent("keydown", {
      key: "Escape",
      bubbles: true,
      cancelable: true,
    });
    window.addEventListener("keydown", bubbleListener);

    try {
      act(() => {
        window.dispatchEvent(escapeEvent);
      });

      expect(escapeEvent.defaultPrevented).toBe(true);
      expect(bubbleListener).not.toHaveBeenCalled();
      expectClosed(container);
    } finally {
      window.removeEventListener("keydown", bubbleListener);
    }
  });

  it("desenha campeão, pódio, prêmios e recordes a partir do payload", () => {
    const { container } = renderOverlay();

    expect(container.querySelector(".co-cname")).toHaveTextContent("R. Silva");
    expect(container.querySelector(".co-team")).toHaveTextContent("Meridian GT");
    // Vantagem sobre o vice, direto do payload.
    expect(container.querySelector(".co-clinch")).toHaveTextContent("7 pontos");
    expect(screen.getByText("Grand Chelem")).toBeInTheDocument();
    expect(
      screen.getByText("Em Spa-Francorchamps: pole, vitória e volta mais rápida na mesma etapa."),
    ).toBeInTheDocument();
    expect(screen.getByText("Mais vitórias")).toBeInTheDocument();
  });

  it("conta a campanha do campeão ao lado do nome, com a escala do calendário", () => {
    const { container } = renderOverlay();

    const stats = [...container.querySelectorAll(".co-runstat")].map(
      (el) => `${el.querySelector(".n").textContent} ${el.querySelector(".k").textContent}`,
    );
    // Zero é fato e continua na régua — some só quebraria o alinhamento das colunas.
    expect(stats).toEqual([
      "1 Vitórias",
      "2 Pódios",
      "1 Poles",
      "0 V. rápidas",
      // Etapas fecha o conjunto: 1 vitória em 2 etapas não é 1 em 22.
      "2 Etapas",
    ]);
    expect(container.querySelector(".co-bigpts .v")).toHaveTextContent("43");
  });

  it("desenha bandeira e escudo de verdade na linha do campeão", () => {
    const { container } = renderOverlay();

    // Imagem pelo componente da casa, não o emoji: a fonte do Windows não desenha
    // bandeira e o campeão aparecia com "BR" escrito ao lado.
    const flag = container.querySelector("img.co-flag");
    expect(flag).toBeInTheDocument();
    expect(flag.getAttribute("src")).toMatch(/br\.png$/);
    expect(container.querySelector(".co-nat")).toHaveTextContent("Brasileiro");
    expect(
      container.querySelector('[data-testid="season-champion-team-logo"]'),
    ).toBeInTheDocument();
  });

  it("abre no quadro de pilotos e guarda os construtores atrás do alternador", () => {
    const { container } = renderOverlay();

    // Os dois campeonatos não dividem a mesma rolagem: o de pilotos é o que abre.
    expect(container.querySelector(".co-ctable")).not.toBeInTheDocument();
    expect(container.querySelector(".co-cname")).toHaveTextContent("R. Silva");

    fireEvent.click(screen.getByRole("tab", { name: "Construtores" }));

    expect(container.querySelector(".co-ctable")).toBeInTheDocument();
    // O quadro de pilotos sai de cena junto com os prêmios e recordes dele.
    expect(container.querySelector(".co-podium")).not.toBeInTheDocument();
    expect(container.querySelector(".co-cname")).toHaveTextContent("Meridian GT");
  });

  it("monta o campeonato de construtores com o líder e a equipe do jogador marcados", () => {
    const { container } = renderOverlay();
    fireEvent.click(screen.getByRole("tab", { name: "Construtores" }));

    const rows = [...container.querySelectorAll(".co-crow")];
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("Meridian GT");
    expect(rows[0].className).toContain("lead");
    // Mesma equipe: líder E do jogador, as duas marcas convivem.
    expect(rows[0].className).toContain("you");
    expect(rows[0].querySelector(".co-cpts .v")).toHaveTextContent("61");
    expect(rows[0].querySelector(".nm small")).toHaveTextContent("2 vitórias");
    // Zero não vira "0 vitória": em pt-BR o plural do i18next põe 0 no singular.
    expect(rows[1].querySelector(".nm small")).toHaveTextContent("Sem vitórias");
    expect(rows[1].className).not.toContain("lead");
    // A campanha da equipe em números, na ordem da régua.
    const stats = [...rows[0].querySelectorAll(".co-cstat")].map(
      (el) => `${el.querySelector("b").textContent} ${el.querySelector("small").textContent}`,
    );
    expect(stats).toEqual(["2 Vit.", "3 Pód.", "1 Poles", "0 V. rápidas"]);
  });

  it("mostra quem pontuou por cada equipe, com o jogador destacado", () => {
    const { container } = renderOverlay();
    fireEvent.click(screen.getByRole("tab", { name: "Construtores" }));

    const lider = container.querySelector(".co-crow");
    const pilotos = [...lider.querySelectorAll(".co-cdrv span")];
    expect(pilotos.map((el) => el.textContent)).toEqual(["R. Silva43", "Otto Vahl18"]);
    expect(pilotos[0].className).toContain("you");
    expect(pilotos[1].className).not.toContain("you");
  });

  it("volta ao quadro de pilotos quando outra temporada abre o pop-up", () => {
    const { container } = renderOverlay();
    fireEvent.click(screen.getByRole("tab", { name: "Construtores" }));
    expect(container.querySelector(".co-ctable")).toBeInTheDocument();

    act(() => {
      useCareerStore.setState({ championOverlay: { ...OVERLAY_STATE, year: 2027 } });
    });

    expect(container.querySelector(".co-ctable")).not.toBeInTheDocument();
    expect(container.querySelector(".co-cname")).toHaveTextContent("R. Silva");
  });

  it("esconde o alternador quando a temporada não tem construtores", () => {
    useCareerStore.setState({ championOverlay: { ...OVERLAY_STATE, constructors: [] } });
    const { container } = renderOverlay();

    expect(container.querySelector(".co-switch")).not.toBeInTheDocument();
    expect(container.querySelector(".co-podium")).toBeInTheDocument();
  });

  it("resolve as unidades dos recordes sem vazar nome de chave do i18n", () => {
    const { container } = renderOverlay();

    const cards = container.querySelectorAll(".co-super");
    // Recorde sem unidade: só o número, sem <small>.
    expect(cards[0].querySelector(".val")).toHaveTextContent(/^1$/);
    expect(cards[0].querySelector(".val small")).toBeNull();
    // Unidade fixa do i18n.
    expect(cards[1].querySelector(".val small")).toHaveTextContent("DNF");
    // Sufixo do backend vai para a linha de contexto, não para junto do número.
    expect(cards[2].querySelector(".ctx")).toHaveTextContent("Circuit de Lédenon");
    expect(cards[2].querySelector(".val small")).toBeNull();

    // Nenhuma chave crua escapou para a tela.
    expect(container.textContent).not.toMatch(/seasonChampion\./);
  });

  it("lista a classificação do 4º para baixo embaixo do pódio", () => {
    const { container } = renderOverlay();

    const rows = [...container.querySelectorAll(".co-restrow")];
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("Felipe Duarte");
    expect(rows[1].textContent).toContain("Sara Lindqvist");
    // Os três do pódio não se repetem na lista miúda.
    expect(rows.map((el) => el.textContent).join(" ")).not.toContain("R. Silva");
    // Traço da cor da equipe, com queda para o neutro quando o time não tem cor.
    expect(rows[0].querySelector(".cor")).toHaveStyle({ background: "#4f8fd8" });
    expect(rows[1].querySelector(".cor")).toHaveStyle({ background: "#3a4553" });
  });

  it("desenha os prêmios novos com a frase certa", () => {
    renderOverlay();

    expect(screen.getByText("Zebra do ano")).toBeInTheDocument();
    expect(screen.getByText("Venceu em Circuit de Lédenon largando do 9º.")).toBeInTheDocument();
    expect(screen.getByText("Regularidade do ano")).toBeInTheDocument();
    expect(screen.getByText("9 chegadas, todas entre o 2º e o 4º lugar.")).toBeInTheDocument();
    expect(screen.getByText("Equipe do ano")).toBeInTheDocument();
    // Pontuação quebrada vira número (e não texto) para o i18next pluralizar por ela.
    expect(
      screen.getByText("79.5 pontos somados pelos seus pilotos ao longo da temporada."),
    ).toBeInTheDocument();
  });

  it("acende a posição do piloto ao passar o mouse no nome citado num prêmio", () => {
    const { container } = renderOverlay();

    // "R. Silva" aparece como menção no prêmio; ele é o 1º, então quem acende é o pódio.
    const mencao = [...container.querySelectorAll(".co-award .who span")].find((el) =>
      el.textContent.includes("R. Silva"),
    );
    expect(mencao).toBeTruthy();

    fireEvent.mouseEnter(mencao);
    expect(container.querySelector(".co-step.lit .co-who")).toHaveTextContent("R. Silva");

    fireEvent.mouseLeave(mencao);
    expect(container.querySelector(".co-step.lit")).toBeNull();
  });

  it("acende a linha miúda de quem está fora do pódio", () => {
    useCareerStore.setState({
      championOverlay: {
        ...OVERLAY_STATE,
        records: [
          { id: "vitorias", who: "Felipe Duarte", is_player: false, valor: "1", sufixo: null },
        ],
      },
    });
    const { container } = renderOverlay();

    const mencao = [...container.querySelectorAll(".co-super .nm span")].find((el) =>
      el.textContent.includes("Felipe Duarte"),
    );
    fireEvent.mouseEnter(mencao);

    expect(container.querySelector(".co-restrow.lit")).toHaveTextContent("Felipe Duarte");
  });

  it("põe o prêmio do jogador no primeiro destaque e o excedente na faixa miúda", () => {
    const { container } = renderOverlay();

    const destaques = [...container.querySelectorAll(".co-award")];
    expect(destaques).toHaveLength(4);
    // Os dois do jogador sobem; entre eles a ordem autorada pelo backend se mantém.
    expect(destaques[0].querySelector(".t")).toHaveTextContent("Grand Chelem");
    expect(destaques[0].className).toContain("you");
    expect(destaques[1].querySelector(".t")).toHaveTextContent("Equipe do ano");
    // O quinto não some da tela: desce para a faixa de uma linha.
    const faixa = [...container.querySelectorAll(".co-awardmini")];
    expect(faixa).toHaveLength(1);
    expect(faixa[0]).toHaveTextContent("Etapa do ano");
    expect(faixa[0]).toHaveTextContent("Circuit de Lédenon");
  });

  it("mostra equipe e posição de quem levou o prêmio, e só quando é um piloto", () => {
    const { container } = renderOverlay();

    const porTitulo = (titulo) =>
      [...container.querySelectorAll(".co-award")].find((el) =>
        el.querySelector(".t").textContent.includes(titulo),
      );

    expect(porTitulo("Grand Chelem").querySelector(".ctx")).toHaveTextContent(
      "Meridian GT · 1º no campeonato",
    );
    // Prêmio de equipe não aponta para um piloto: nada de linha de contexto.
    expect(porTitulo("Equipe do ano").querySelector(".ctx")).toBeNull();
  });

  it("não repete o marcador (você) ao lado dos nomes", () => {
    const { container } = renderOverlay();

    // A prosa que fala com o jogador continua ("Você conquistou o título", "É você!");
    // o que saiu foi o "(você)" colado em cada nome no pódio, prêmios e recordes.
    expect(container.textContent).not.toMatch(/\(você\)/i);
  });

  it("não renderiza nada sem pilotos no payload", () => {
    useCareerStore.setState({ championOverlay: { ...OVERLAY_STATE, drivers: [] } });

    const { container } = renderOverlay();

    expect(container.querySelector(".champ-ov")).not.toBeInTheDocument();
  });

  it("dá precedência ao overlay sobre o menu de pausa ao pressionar Escape", () => {
    const { container } = render(
      <MemoryRouter>
        <PauseMenu />
        <SeasonChampionOverlay />
      </MemoryRouter>,
    );

    fireEvent.keyDown(window, { key: "Escape", bubbles: true, cancelable: true });

    expect(container.querySelector(".champ-ov")).not.toBeInTheDocument();
    expect(screen.queryByText("Pausa")).not.toBeInTheDocument();
    expect(container.querySelector(".glass-strong")).not.toBeInTheDocument();
    expect(useCareerStore.getState().championOverlay).toBe(null);
  });
});
