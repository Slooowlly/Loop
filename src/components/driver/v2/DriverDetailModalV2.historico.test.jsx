import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import {
  curva,
  detail,
  fingeQueRola,
  renderFicha,
  restauraLayout,
  rival,
} from "./driverDetailV2TestKit.jsx";
import { DriverDetailModalV2 } from "./DriverDetailModalV2";

// O piso não corre debaixo do vitest de qualquer jeito; o espião existe para
// poder afirmar COM QUE ARGUMENTO ele foi pedido, que é onde o bug morava.
vi.mock("../../ui/aberturaDePainel.js", () => ({
  ABERTURA_MS: 0,
  pisoDeAbertura: vi.fn(() => Promise.resolve()),
}));

let mockState = {};

vi.mock("../../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// O dossiê de habilidade do jogador é um bloco fechado do v1 com invoke próprio:
// aqui interessa só se a ABA aparece para o jogador, não o conteúdo dela.
vi.mock("../detalhes/PlayerSkillSection.jsx", () => ({
  PlayerSkillSection: () => <section>dossie-habilidade</section>,
}));

describe("DriverDetailModalV2 — historico", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    fingeQueRola(true);
  });

  afterEach(restauraLayout);

  it("nao desenha mais a fileira de trofeus", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    // Melhor temporada, maior sequência e status repetiam, em ouro, o que os
    // cards do dossiê já respondem logo abaixo — e empurravam o dossiê inteiro
    // para fora da tela.
    expect(document.querySelector("[data-highlight]")).toBeNull();
  });

  it("lista os anos de titulo com a equipe de cada um", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    expect(anos.querySelectorAll("[data-title-year]")).toHaveLength(2);
    expect(anos).toHaveTextContent("2023");
    expect(anos).toHaveTextContent("2021");
    expect(within(anos).getAllByTestId("driver-detail-title-logo")).toHaveLength(2);
  });

  it("abre ano, equipe e categoria de cada conquista no hover dos cards", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");

    // O card conta QUANTAS; o painel conta quando, por quem e em que escada.
    const cartao = (id) => document.querySelector(`[data-record="${id}"]`).closest("[data-has-detail]");

    fireEvent.mouseEnter(cartao("vitorias"));
    const vitorias = screen.getByTestId("dossier-detail-tooltip");
    expect(vitorias).toHaveTextContent("2021");
    expect(vitorias).toHaveTextContent("Aures Racing");
    expect(vitorias).toHaveTextContent("GT4");
    expect(vitorias).toHaveTextContent("2023");
    expect(vitorias).toHaveTextContent("Ferrari");
    // Nada de colocacao no campeonato: "P1" numa lista de vitorias diz o obvio, e
    // numa de podios ("P10") se le como a chegada — que seria mentira.
    expect(vitorias).not.toHaveTextContent(/\bP\d/);
    fireEvent.mouseLeave(cartao("vitorias"));

    fireEvent.mouseEnter(cartao("podios"));
    const podios = screen.getByTestId("dossier-detail-tooltip");
    expect(podios).toHaveTextContent("11");
    expect(podios).not.toHaveTextContent(/\bP\d/);
    fireEvent.mouseLeave(cartao("podios"));

    // Os titulos abrem a temporada inteira de cada um — o card mostra a logo e o
    // ano, o painel diz em que categoria e com que campanha.
    fireEvent.mouseEnter(cartao("titulos"));
    const titulos = screen.getByTestId("dossier-detail-tooltip");
    expect(titulos).toHaveTextContent("240 pts · 4V · 9P");
    // Aqui toda linha e P1 por definicao — repetir a colocacao nao diz nada.
    expect(titulos).not.toHaveTextContent(/\bP\d/);
    fireEvent.mouseLeave(cartao("titulos"));

    // E corridas reaproveita a lista de temporadas, que responde a mesma coisa.
    fireEvent.mouseEnter(cartao("corridas"));
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("P11");
    fireEvent.mouseLeave(cartao("corridas"));
  });

  it("deixa as equipes campeas fluirem lado a lado", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // Uma linha por equipe crescia noventa pixels num tricampeao, e como os
    // quatro cards dividem a linha do grid o vazio sobrava embaixo de Corridas,
    // Vitorias e Podios. Com wrap, tres titulos cabem numa linha so.
    expect(anos.className).toContain("flex-wrap");
    expect(anos.className).not.toContain("flex-col");
  });

  it("agrupa os titulos por equipe em vez de estourar o card", async () => {
    const dinastia = [
      ...[2025, 2024, 2023, 2022, 2021, 2020, 2019, 2018, 2017].map((ano) => ({
        ano,
        categoria: "gt3",
        equipe: "McLaren",
        equipe_cor: "#ff8000",
      })),
      { ano: 2014, categoria: "gt4", equipe: "Aures Racing", equipe_cor: "#3fb950" },
      { ano: 2011, categoria: "gt4", equipe: "Acura", equipe_cor: "#1f6feb" },
      { ano: 2008, categoria: "bmw_m2", equipe: "Ferrari", equipe_cor: "#dc0000" },
    ];
    renderFicha(
      {},
      detail({
        trajetoria: { ...detail().trajetoria, titulos: 12, titulos_detalhe: dinastia },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // Doze títulos viravam doze chips e quatro fileiras; por equipe são quatro
    // linhas, e as nove taças da mesma equipe dividem uma logo só.
    expect(anos.querySelectorAll("[data-title-team]")).toHaveLength(4);
    // Nove anos seguidos viram UM intervalo: nove rotulos custavam duas linhas e
    // obrigavam a somar de cabeca para descobrir que era uma dinastia.
    const mclaren = anos.querySelector('[data-title-team="McLaren"]');
    expect(mclaren.querySelectorAll("[data-title-year]")).toHaveLength(1);
    expect(mclaren).toHaveTextContent("2017 ~ 2025");
    // Ordem: a dinastia mais recente primeiro.
    expect(anos.querySelectorAll("[data-title-team]")[0]).toHaveAttribute(
      "data-title-team",
      "McLaren",
    );
    expect(anos.querySelectorAll("[data-title-year]")[0]).toHaveTextContent("2025");
  });

  it("nao vira intervalo com dois anos seguidos", async () => {
    const bi = [2025, 2024].map((ano) => ({
      ano,
      categoria: "gt3",
      equipe: "McLaren",
      equipe_cor: "#ff8000",
    }));
    renderFicha(
      {},
      detail({ trajetoria: { ...detail().trajetoria, titulos: 2, titulos_detalhe: bi } }),
    );

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // "2024 ~ 2025" e mais largo que "2024 2025" e ainda esconde que sao dois.
    expect(anos.querySelectorAll("[data-title-year]")).toHaveLength(2);
    expect(anos).not.toHaveTextContent("~");
  });

  it("resume as equipes que passam do teto do card", async () => {
    const espalhado = [2025, 2023, 2021, 2019, 2017, 2015].map((ano, index) => ({
      ano,
      categoria: "gt3",
      equipe: `Equipe ${index}`,
      equipe_cor: "#1f6feb",
    }));
    renderFicha(
      {},
      detail({
        trajetoria: { ...detail().trajetoria, titulos: 6, titulos_detalhe: espalhado },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    const anos = await screen.findByTestId("driver-detail-title-years");
    // Seis equipes distintas: quatro linhas e o resto vira uma frase, senão o
    // card volta a crescer sem teto.
    expect(anos.querySelectorAll("[data-title-team]")).toHaveLength(4);
    expect(anos).toHaveTextContent("2 títulos");
  });

  it("mostra so o total quando o piloto nao tem arquivo de titulos", async () => {
    renderFicha({}, detail({ trajetoria: { ...detail().trajetoria, titulos_detalhe: [] } }));

    await screen.findByTestId("driver-detail-hero");
    // Piloto histórico pré-gerado tem o total mas não tem temporada arquivada:
    // a lista some, o card continua dizendo que ele é campeão.
    expect(screen.queryByTestId("driver-detail-title-years")).toBeNull();
    expect(document.querySelector('[data-record="titulos"]')).toHaveTextContent("2");
  });

  it("desenha a barra de posicao com o rank por extenso", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    expect(screen.getByText("3º de 240")).toBeInTheDocument();
    expect(screen.getByText("40º de 240")).toBeInTheDocument();
  });

  it("leva o card de recorde ao ranking mundial da metrica e fecha a ficha", async () => {
    const onOpenRanking = vi.fn();
    const onClose = vi.fn();
    renderFicha({ onOpenRanking, onClose });

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
    fireEvent.click(screen.getByTestId("driver-detail-record-link-vitorias"));

    // Sem categoria: no recorte mundial o destino é o mundo inteiro.
    expect(onOpenRanking).toHaveBeenCalledWith({
      metric: "vitorias",
      driverId: "D1",
      category: null,
    });
    // A tela de destino ocupa o lugar da ficha — deixar o modal aberto por cima
    // esconderia o ranking que o clique pediu.
    expect(onClose).toHaveBeenCalled();
  });

  it("leva a categoria junto quando o recorte e o grid atual", async () => {
    const onOpenRanking = vi.fn();
    renderFicha(
      { onOpenRanking },
      detail({ rankings_grid: { corridas: 4, vitorias: 2, podios: 3, titulos: 1, total: 24 } }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
    fireEvent.click(screen.getByTestId("driver-detail-rank-scope-grid"));
    fireEvent.click(screen.getByTestId("driver-detail-record-link-podios"));

    // A tela de destino tem que responder a MESMA pergunta que a origem: quem
    // leu "3º de 24 no grid" não pode cair na lista dos 610 do mundo.
    expect(onOpenRanking).toHaveBeenCalledWith({
      metric: "podios",
      driverId: "D1",
      category: "gt3",
    });
  });

  it("nao torna o card de recorde clicavel sem destino", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    expect(screen.queryByTestId("driver-detail-record-link-vitorias")).toBeNull();
  });

  it("alterna os numeros de carreira entre o mundo e o grid atual", async () => {
    renderFicha(
      {},
      detail({ rankings_grid: { corridas: 4, vitorias: 2, podios: 3, titulos: 1, total: 24 } }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // ACIMA dos cards: o denominador precisa ser lido antes do "3º de 240", não
    // depois. `DOCUMENT_POSITION_FOLLOWING` = o card vem depois do seletor.
    const seletor = screen.getByTestId("driver-detail-rank-scope-toggle");
    const primeiroCard = document.querySelector('[data-record="corridas"]');
    expect(seletor.compareDocumentPosition(primeiroCard) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    // O mundo é o padrão.
    expect(screen.getByText("3º de 240")).toBeInTheDocument();
    expect(screen.getByTestId("driver-detail-rank-scope-mundo")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    // Com o seletor na tela a frase de recorte sai: ela repetiria o botão aceso
    // e o "de 240" que já está em cada card.
    expect(screen.queryByTestId("driver-detail-rank-scope")).toBeNull();

    fireEvent.click(screen.getByTestId("driver-detail-rank-scope-grid"));

    expect(screen.getByText("2º de 24")).toBeInTheDocument();
    expect(screen.queryByText("3º de 240")).toBeNull();
    expect(screen.getByTestId("driver-detail-rank-scope-grid")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("esconde o seletor de escopo para quem nao tem grid", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // Sem `rankings_grid` no payload não há segundo recorte — e um botão morto
    // seria pior que nenhum.
    expect(screen.queryByTestId("driver-detail-rank-scope-toggle")).toBeNull();
    expect(screen.getByTestId("driver-detail-rank-scope")).toHaveTextContent("do mundo");
  });

  it("esconde a barra quando o payload nao traz o total", async () => {
    renderFicha({}, detail({ rankings_carreira: { vitorias: 3 } }));

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    expect(screen.queryByText(/de 240/)).not.toBeInTheDocument();
    // Preso ao card, e nao ao texto solto: a curva de campeonato escreve a
    // posicao final em cada ponto, e "3º" passou a existir tambem la.
    expect(document.querySelector('[data-record="vitorias"]')).toHaveTextContent("3º");
  });

  it("mostra a queda e a confiabilidade no dossie", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const dossie = screen.getByTestId("driver-detail-career-dossier");
    const queda = dossie.querySelector('[data-group="queda"]');
    // O jejum carrega o periodo: "24" e um numero, "24 (2018-2021)" e uma queda.
    expect(queda).toHaveTextContent("24 (2018–2021)");
    // O jejum de PODIOS e mais fundo que o de vitorias, e e a marca que serve
    // para quem nunca venceu.
    expect(queda).toHaveTextContent("31 (2018–2022)");
    // A pior temporada leva o resultado colado: sem ele, "2019, GT4" seria
    // indistinguivel de uma linha qualquer da escada de categorias.
    expect(queda).toHaveTextContent("2019, GT4 · P11");

    const confiabilidade = dossie.querySelector('[data-group="confiabilidade"]');
    expect(confiabilidade).toHaveTextContent("7.5%");
    expect(confiabilidade).toHaveTextContent("18");
  });

  it("agrupa o dossie em tres linhas tematicas de tres colunas", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const grupos = [...screen.getByTestId("driver-detail-career-dossier").querySelectorAll("[data-group]")]
      .map((node) => node.dataset.group);
    expect(grupos).toEqual([
      // quem ele é
      "presenca",
      "mobilidade",
      "primeiros",
      // o que entrega
      "auge",
      "sabado",
      "duelos",
      // o que custa
      "queda",
      "confiabilidade",
      "lesoes",
    ]);
    // Em tres colunas, cada card da terceira linha cai embaixo do seu par da
    // segunda: auge sobre queda, sabado sobre confiabilidade.
    expect(grupos.indexOf("queda")).toBe(grupos.indexOf("auge") + 3);
    expect(grupos.indexOf("confiabilidade")).toBe(grupos.indexOf("sabado") + 3);
  });

  it("auge e queda sao espelhos, linha por linha", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const dossie = screen.getByTestId("driver-detail-career-dossier");
    const auge = dossie.querySelector('[data-group="auge"]');
    const queda = dossie.querySelector('[data-group="queda"]');
    // Melhor e pior temporada levam o resultado colado. "Melhor campeonato P1"
    // era uma linha inteira repetindo a colocacao da linha de cima.
    expect(auge).toHaveTextContent("2023, GT3 · P1");
    expect(queda).toHaveTextContent("2019, GT4 · P11");
    expect(auge).not.toHaveTextContent("Melhor campeonato");
    // Sequencia e jejum, de vitorias e de podios, com o periodo nos dois lados.
    expect(auge).toHaveTextContent("3 (2023)");
    expect(auge).toHaveTextContent("9 (2022–2023)");
    expect(queda).toHaveTextContent("24 (2018–2021)");
    expect(queda).toHaveTextContent("31 (2018–2022)");
    expect(auge.querySelectorAll("[class*=border-b]")).toHaveLength(
      queda.querySelectorAll("[class*=border-b]").length,
    );
  });

  it("abre o detalhe das equipes ao passar o mouse na linha", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");
    expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();

    fireEvent.mouseEnter(linha);
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Aures Racing");
    expect(painel).toHaveTextContent("Ferrari");
    expect(painel).toHaveTextContent("2019-2021");

    // Sem a bolinha fechada o painel é volátil: sai o mouse, some na hora.
    fireEvent.mouseLeave(linha);
    expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
  });

  it("a bolinha prende o painel, que so solta 5s depois de o mouse ir embora", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      // Segurando o mouse na linha a bolinha fecha o circulo e o painel prende.
      act(() => vi.advanceTimersByTime(700));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "true");

      // Preso, ele sobrevive a saida do mouse — que e o ponto: a lista rolavel
      // so e alcancavel assim.
      fireEvent.mouseLeave(linha);
      act(() => vi.advanceTimersByTime(4000));
      expect(screen.getByTestId("dossier-detail-tooltip")).toBeInTheDocument();

      // Voltar para o proprio painel zera a contagem dos 5s.
      fireEvent.mouseEnter(screen.getByTestId("dossier-detail-tooltip"));
      act(() => vi.advanceTimersByTime(4000));
      expect(screen.getByTestId("dossier-detail-tooltip")).toBeInTheDocument();

      fireEvent.mouseLeave(screen.getByTestId("dossier-detail-tooltip"));
      act(() => vi.advanceTimersByTime(5000));
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("o botao do meio prende na hora e solta na hora", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      fireEvent.mouseDown(linha, { button: 1 });
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "true");

      // E solta sem esperar os 5s, para tirar o painel da frente.
      fireEvent.mouseDown(linha, { button: 1 });
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      // Solto, ele nao volta a prender sozinho com o mouse parado onde esta.
      act(() => vi.advanceTimersByTime(2000));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      fireEvent.mouseLeave(linha);
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("o painel que cabe na tela nao oferece prender", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    fingeQueRola(false);
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      const painel = screen.getByTestId("dossier-detail-tooltip");
      // Sem barra de rolagem nao ha o que alcancar: nem bolinha, nem legenda.
      expect(within(painel).queryByTestId("dossier-detail-bolinha")).not.toBeInTheDocument();
      expect(painel).not.toHaveTextContent("prender");

      act(() => vi.advanceTimersByTime(700));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      fireEvent.mouseLeave(linha);
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("o botao do meio prende ate o painel que nao rola", async () => {
    fingeQueRola(false);
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
    const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

    fireEvent.mouseEnter(linha);
    // A bolinha nao aparece — mas o gesto deliberado vale em qualquer painel.
    fireEvent.mouseDown(linha, { button: 1 });
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveAttribute("data-preso", "true");
    // E preso ele ganha o rodape com a saida, senao ficaria sem porta.
    expect(within(painel).getByTestId("dossier-detail-soltar")).toBeInTheDocument();

    fireEvent.mouseLeave(linha);
    expect(screen.getByTestId("dossier-detail-tooltip")).toBeInTheDocument();
  });

  it("a bolinha so corre com o cursor parado", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      act(() => vi.advanceTimersByTime(500));

      // Andar sobre a linha reinicia o anel: e o que impede atravessar o dossie
      // devagar e prender cada linha do caminho.
      fireEvent.mouseMove(linha, { clientX: 10, clientY: 10 });
      fireEvent.mouseMove(linha, { clientX: 40, clientY: 10 });
      act(() => vi.advanceTimersByTime(500));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "false");

      act(() => vi.advanceTimersByTime(300));
      expect(screen.getByTestId("dossier-detail-tooltip")).toHaveAttribute("data-preso", "true");
    } finally {
      vi.useRealTimers();
    }
  });

  it("so um painel fica preso por vez", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      renderFicha();

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const equipes = screen.getByText("Equipes defendidas").closest("[data-has-detail]");
      const promocoes = screen.getByText("Promoções").closest("[data-has-detail]");

      fireEvent.mouseEnter(equipes);
      act(() => vi.advanceTimersByTime(700));
      fireEvent.mouseLeave(equipes);

      // O painel preso cobre as linhas vizinhas: dois presos ao mesmo tempo e
      // uma pilha, nao uma leitura.
      fireEvent.mouseEnter(promocoes);
      act(() => vi.advanceTimersByTime(700));
      const abertos = screen.getAllByTestId("dossier-detail-tooltip");
      expect(abertos).toHaveLength(1);
      expect(abertos[0]).toHaveTextContent("GT4 → GT3");
    } finally {
      vi.useRealTimers();
    }
  });

  it("o x e o Esc soltam o painel preso, e o Esc nao fecha a ficha por baixo", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const onClose = vi.fn();
    try {
      renderFicha({ onClose });

      await screen.findByTestId("driver-detail-hero");
      fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));
      const linha = screen.getByText("Equipes defendidas").closest("[data-has-detail]");

      fireEvent.mouseEnter(linha);
      act(() => vi.advanceTimersByTime(700));
      fireEvent.click(screen.getByTestId("dossier-detail-soltar"));
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();

      fireEvent.mouseEnter(linha);
      act(() => vi.advanceTimersByTime(700));
      fireEvent.keyDown(window, { key: "Escape" });
      expect(screen.queryByTestId("dossier-detail-tooltip")).not.toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();

      // Sem painel preso o Esc volta a ser da ficha.
      fireEvent.keyDown(window, { key: "Escape" });
      expect(onClose).toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("mostra a logo da equipe e o ano, sem o dia", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Primeira vitória").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(within(painel).getByTestId("dossier-detail-logo")).toBeInTheDocument();
    // O dia exato é precisão que ninguém pediu, e rouba a linha da rodada e da
    // pista, que dizem muito mais.
    expect(painel).toHaveTextContent("2019 · Rodada 6");
    expect(painel).not.toHaveTextContent("mai");
  });

  it("diz de qual categoria para qual na promocao", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Promoções").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    // O backend manda as CHAVES; o rotulo e montado aqui.
    expect(painel).toHaveTextContent("GT4 → GT3");
    // Promocao nao tem equipe por natureza: aqui o X seria um buraco em toda
    // linha anunciando uma ausencia que nao significa nada.
    expect(within(painel).queryByTestId("dossier-detail-sem-equipe")).not.toBeInTheDocument();
  });

  it("mostra a data real da primeira vitoria, e nao so o numero da corrida", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Primeira vitória").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Rodada 6");
    expect(painel).toHaveTextContent("Circuito de Navarra");
    expect(painel).toHaveTextContent("Aures Racing");
    // A data ISO vira data local, com o dia — o ano sozinho ja estava no card.
    expect(painel).toHaveTextContent("2019");
  });

  it("mostra quem sao os companheiros, com equipe de hoje, idade e nacionalidade", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Companheiros enfrentados").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Igor Petrov");
    expect(painel).toHaveTextContent("Vector Racing");
    expect(painel).toHaveTextContent("29 anos");

    // A bandeira vai como ARTE: o Windows nao tem glifo de bandeira nacional, e
    // o emoji do backend caia para as duas letras cruas ("RU") na ficha.
    expect(within(painel).getByAltText("🇷🇺 Russo")).toBeInTheDocument();
    expect(painel).toHaveTextContent("Russo");
    expect(painel).not.toHaveTextContent("🇷🇺");

    // Companheiro sem equipe hoje ocupa o mesmo slot do logo com um X, senao a
    // linha ficava desalinhada do resto da lista.
    expect(within(painel).getByTestId("dossier-detail-sem-equipe")).toBeInTheDocument();
  });

  it("mostra a lesao com a data e a equipe daquele dia", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Leves").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Dor no pescoço");
    expect(painel).toHaveTextContent("Rodada 4");
    expect(painel).toHaveTextContent("Ferrari");
    // Cada gravidade abre SO as suas: "Leves 1" listando a moderada junto era a
    // linha mentindo sobre o proprio numero.
    expect(painel).not.toHaveTextContent("Fratura no punho");
    fireEvent.mouseLeave(screen.getByText("Leves").closest("[data-has-detail]"));

    fireEvent.mouseEnter(screen.getByText("Moderadas").closest("[data-has-detail]"));
    const moderadas = screen.getByTestId("dossier-detail-tooltip");
    expect(moderadas).toHaveTextContent("Fratura no punho");
    expect(moderadas).not.toHaveTextContent("Dor no pescoço");
  });

  it("clicar na equipe do painel abre o Atlas e fecha a ficha", async () => {
    const onOpenTeam = vi.fn();
    const onClose = vi.fn();
    renderFicha({ onOpenTeam, onClose });

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Equipes defendidas").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    // Duas equipes na lista, mas so a que ainda existe no mundo vira porta — e
    // sao DOIS alvos por equipe: o logo e o nome.
    const portas = within(painel).getAllByTestId("dossier-detail-abrir-equipe");
    expect(portas).toHaveLength(2);

    fireEvent.click(portas[0]);
    expect(onOpenTeam).toHaveBeenCalledWith({
      id: "T1",
      nome: "Aures Racing",
      cor_primaria: "#3fb950",
    });
    // As duas telas ocupam o mesmo espaco: a ficha sai da frente do Atlas.
    expect(onClose).toHaveBeenCalled();
  });

  it("o tempo de carreira nomeia as duas pontas, e nao lista as equipes do meio", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Tempo de carreira").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    // Sem os rotulos, duas linhas passavam por uma lista de equipes cortada.
    expect(painel).toHaveTextContent("Equipe de estreia");
    expect(painel).toHaveTextContent("Equipe atual");
  });

  it("o ano parado diz de qual equipe ele saiu e quando voltou", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Anos desempregado").closest("[data-has-detail]"));
    const painel = screen.getByTestId("dossier-detail-tooltip");
    expect(painel).toHaveTextContent("Aures Racing");
    expect(painel).toHaveTextContent("→ 2022");
  });

  it("a temporada sem podio mostra a melhor chegada do ano", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Temporadas sem pódio").closest("[data-has-detail]"));
    // Sem podio, a melhor chegada e o unico resultado que da tamanho ao ano.
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("melhor: P5");
  });

  it("a taxa de abandono e o rival mais duro abrem o proprio detalhe", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    fireEvent.mouseEnter(screen.getByText("Taxa de abandono").closest("[data-has-detail]"));
    // Numerador e denominador a vista: "4,3%" sozinho nao diz se sao 3 em 70.
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("3/70 · 4.3%");
    fireEvent.mouseLeave(screen.getByText("Taxa de abandono").closest("[data-has-detail]"));

    fireEvent.mouseEnter(screen.getByText("Mais duro").closest("[data-has-detail]"));
    expect(screen.getByTestId("dossier-detail-tooltip")).toHaveTextContent("388 x 412");
  });

  it("o toggle de recordes liga a posicao no grid e no mundo", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // Desligado por padrão: a posição é a pergunta SEGUINTE, e ligada sempre
    // dobraria a altura de nove cards.
    expect(screen.queryAllByTestId("dossier-rank")).toHaveLength(0);

    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    const marcas = screen.getAllByTestId("dossier-rank");
    // Poles tem as duas posições; taxa de abandono não tem grid (aposentado,
    // sem pelotão de domingo) e mostra só a do mundo.
    expect(marcas[0]).toHaveTextContent("2º");
    expect(marcas[0]).toHaveTextContent("41º");
    expect(marcas.some((marca) => marca.textContent.trim() === "7º")).toBe(true);

    // Os denominadores vão UMA vez, na legenda — repetidos em vinte linhas eles
    // quebravam cada uma em duas e dobravam a altura dos nove cards.
    const escopo = screen.getByTestId("driver-detail-records-scope");
    expect(escopo).toHaveTextContent("Grid de 24");
    expect(escopo).toHaveTextContent("mundo de 610");
    // O número exato de cada linha, que varia, fica no title do próprio ordinal.
    expect(within(marcas[0]).getByText("2º")).toHaveAttribute("data-tooltip", "2º de 24 no grid");

    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    expect(screen.queryAllByTestId("dossier-rank")).toHaveLength(0);
    expect(screen.queryByTestId("driver-detail-records-scope")).toBeNull();
  });

  it("nao esvazia a ficha ao trocar de piloto", async () => {
    const primeiro = detail();
    const tela = renderFicha();
    await screen.findByTestId("driver-detail-hero");
    expect(screen.getByText(primeiro.nome)).toBeInTheDocument();

    // O proximo piloto demora a responder: e nessa janela que o painel de carga
    // aparecia, piscando entre uma ficha e outra.
    let entregar;
    invoke.mockImplementation((command) => {
      if (command === "get_driver_world_rank") return Promise.resolve(null);
      return new Promise((resolve) => {
        entregar = resolve;
      });
    });

    await act(async () => {
      tela.rerender(
        <DriverDetailModalV2
          driverId="D2"
          driverIds={["D0", "D1", "D2"]}
          onSelectDriver={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    // A ficha anterior segue inteira no lugar — sem painel de carga no meio.
    expect(screen.queryByTestId("driver-detail-loading")).toBeNull();
    expect(screen.getByText(primeiro.nome)).toBeInTheDocument();

    await act(async () => {
      entregar(detail({ id: "D2", nome: "Outro Piloto" }));
    });
    expect(await screen.findByText("Outro Piloto")).toBeInTheDocument();
    expect(screen.queryByText(primeiro.nome)).toBeNull();
  });

  it("so busca os recordes quando o toggle liga", async () => {
    // O payload da ficha nao traz mais o mapa: monta-lo exige varrer o mundo
    // inteiro (503ms num save de 27 mil resultados) e era cobrado de toda
    // abertura e de toda troca de piloto para alimentar um botao desligado.
    const payload = detail();
    delete payload.trajetoria.historico.recordes;
    const ranks = {
      poles: { grid: 2, grid_total: 24, mundo: 41, mundo_total: 610 },
    };
    invoke.mockImplementation((command) => {
      if (command === "get_driver_world_rank") return Promise.resolve(null);
      if (command === "get_driver_dossier_ranks") return Promise.resolve(ranks);
      return Promise.resolve(payload);
    });

    render(
      <DriverDetailModalV2 driverId="D1" driverIds={["D1"]} onSelectDriver={vi.fn()} onClose={vi.fn()} />,
    );
    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const buscou = () => invoke.mock.calls.some(([cmd]) => cmd === "get_driver_dossier_ranks");
    expect(buscou()).toBe(false);
    // O botao aparece mesmo sem mapa nenhum em maos — quem monta o mapa e o clique.
    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    expect(buscou()).toBe(true);

    const marcas = await screen.findAllByTestId("dossier-rank");
    expect(marcas[0]).toHaveTextContent("2º");
    expect(marcas[0]).toHaveTextContent("41º");
    expect(screen.getByTestId("driver-detail-records-scope")).toHaveTextContent("Grid de 24");

    // Desligar e ligar de novo nao repete a busca: o mapa ja esta em maos.
    const antes = invoke.mock.calls.filter(([cmd]) => cmd === "get_driver_dossier_ranks").length;
    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    fireEvent.click(screen.getByTestId("driver-detail-records-toggle"));
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "get_driver_dossier_ranks")).toHaveLength(antes);
  });

  it("nao abre painel na linha que o backend nao detalhou", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    // Ele nunca foi rebaixado, entao o backend nem manda a chave — e sem linhas
    // o wrapper nao vira alvo de hover.
    expect(screen.getByText("Rebaixamentos").closest("[data-has-detail]")).toBeNull();
  });

  it("mostra o sabado, que a ficha inteira ignorava, com a media do mundo", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const sabado = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="sabado"]');
    expect(sabado).toHaveTextContent("18");
    // Largar na frente e converter sao habilidades diferentes.
    expect(sabado).toHaveTextContent("11");
    expect(sabado).toHaveTextContent("P4.2");
    // "P4.2" sozinho nao diz se e frente ou fundo; a media do mundo diz.
    expect(sabado).toHaveTextContent("média P10.5");
  });

  it("mostra o confronto com o companheiro de equipe", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const duelos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="duelos"]');
    expect(duelos).toHaveTextContent("7 de 9");
    expect(duelos).toHaveTextContent("Igor Petrov 1-2");
  });

  it("nao inventa confronto para quem nunca dividiu box", async () => {
    const base = detail();
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...base.trajetoria,
          historico: {
            ...base.trajetoria.historico,
            duelos: { companheiros: 0, temporadas: 0, temporadas_vencidas: 0, rival_mais_duro: null },
          },
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const duelos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="duelos"]');
    // "0 de 0" leria como derrota; sem companheiro a comparacao nao existe.
    expect(duelos).not.toHaveTextContent("de 0");
    expect(duelos).toHaveTextContent("-");
  });

  it("lista os primeiros marcos do mais alto para o mais baixo", async () => {
    renderFicha();

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const marcos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="primeiros"]');
    const linhas = [...marcos.querySelectorAll("span")].map((n) => n.textContent);
    const ordem = ["Primeiro título", "Primeira vitória", "Primeiro pódio", "Primeiro DNF"];
    const posicoes = ordem.map((rotulo) => linhas.indexOf(rotulo));
    expect(posicoes.every((p) => p >= 0)).toBe(true);
    expect([...posicoes].sort((a, b) => a - b)).toEqual(posicoes);
    // O titulo e o unico marco que nao cabe num numero de corrida.
    expect(marcos).toHaveTextContent("2021, GT4");
  });

  it("diz Nunca, e nao um traco, para quem jamais foi campeao", async () => {
    const base = detail();
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...base.trajetoria,
          historico: {
            ...base.trajetoria.historico,
            primeiros_marcos: { ...base.trajetoria.historico.primeiros_marcos, primeiro_titulo: null },
          },
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const marcos = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="primeiros"]');
    // Os vizinhos dizem "Nunca"; um "-" solto no meio da coluna sairia como
    // dado faltando, e nao como marca que nao aconteceu.
    expect(marcos).toHaveTextContent("Nunca");
    expect(marcos).not.toHaveTextContent("2021, GT4");
  });

  it("nao inventa taxa de abandono para quem nunca largou", async () => {
    renderFicha(
      {},
      detail({
        trajetoria: {
          ...detail().trajetoria,
          historico: {
            ...detail().trajetoria.historico,
            confiabilidade: { abandonos: 0, corridas: 0, taxa_abandono: null, maior_sequencia_chegadas: 0 },
          },
        },
      }),
    );

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-tab-historico"));

    const confiabilidade = screen
      .getByTestId("driver-detail-career-dossier")
      .querySelector('[data-group="confiabilidade"]');
    // "0%" se leria como "nunca abandona" — um estreante sem largada nenhuma nao
    // e o piloto mais confiavel do grid.
    expect(confiabilidade).toHaveTextContent("-");
    expect(confiabilidade).not.toHaveTextContent("0%");
  });
});
