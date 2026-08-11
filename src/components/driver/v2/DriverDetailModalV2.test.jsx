import { StrictMode } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import { pisoDeAbertura } from "../../ui/aberturaDePainel.js";
import {
  abrirPerfil,
  abrirTemporada,
  contrato,
  curva,
  detail,
  fingeQueRola,
  renderFicha,
  restauraLayout,
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

describe("DriverDetailModalV2 — cabecalho, temporada e perfil", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    fingeQueRola(true);
  });

  afterEach(restauraLayout);

  it("nao gasta a abertura na passagem descartada do StrictMode", async () => {
    pisoDeAbertura.mockClear();
    invoke.mockImplementation((command) =>
      Promise.resolve(command === "get_driver_world_rank" ? null : detail()),
    );

    render(
      <DriverDetailModalV2 driverId="D1" driverIds={["D1"]} onSelectDriver={vi.fn()} onClose={vi.fn()} />,
      { wrapper: StrictMode },
    );
    await screen.findByTestId("driver-detail-hero");

    // Em dev o StrictMode monta, desmonta e remonta o efeito. Enquanto a
    // bandeira da primeira carga era baixada ao PEDIR o piso, quem a gastava era
    // a passagem descartada — e a passagem que de fato desenha pedia o piso como
    // se fosse navegação entre pilotos, sem esperar nada.
    expect(pisoDeAbertura).toHaveBeenCalled();
    expect(pisoDeAbertura.mock.calls.every(([ehAbertura]) => ehAbertura === true)).toBe(true);
  });

  it("carrega titulo, personalidade e estado no cabecalho, e nao numeros de carreira", async () => {
    renderFicha();

    const header = await screen.findByTestId("driver-detail-hero");
    expect(header.querySelector('[data-personality="primary"]')).toHaveTextContent("Calculista");
    expect(within(header).getByTestId("driver-detail-motivation")).toHaveTextContent("72%");
    // A posição no campeonato saiu do cabeçalho: é da temporada corrente e já
    // abre em número grande na aba Temporada atual.
    expect(within(header).getByTestId("driver-detail-state")).not.toHaveTextContent("P2");
    // Título tem faixa própria; no meio dos chips cinzas ele saía com o peso
    // visual de uma licença.
    expect(within(header).getByTestId("driver-detail-titles")).toHaveTextContent("2");
    // As 120 corridas de carreira vivem no Histórico, onde há rank para dar
    // escala a elas — no cabeçalho seriam números sem referência, e zeros na
    // ficha de um estreante.
    expect(header).not.toHaveTextContent("120");
  });

  it("desenha a posicao no ranking mundial ao lado da estrela", async () => {
    renderFicha({}, detail(), { indice: 4210.4, posicao: 12, total: 240, delta: 3 });

    const mark = await screen.findByTestId("driver-detail-world-rank");
    expect(mark).toHaveTextContent("12º");
    // O índice vai arredondado: 4.210,4 e 4.210 dizem a mesma coisa no topo.
    expect(mark).toHaveTextContent("4.210");
    expect(mark).toHaveTextContent("▲3");
  });

  it("nao desenha a marca do ranking para quem esta fora dele", async () => {
    renderFicha({}, detail(), null);

    await screen.findByTestId("driver-detail-hero");
    expect(screen.queryByTestId("driver-detail-world-rank")).toBeNull();
  });

  it("some com a faixa de titulo quem nunca foi campeao", async () => {
    renderFicha({}, detail({ trajetoria: { ...detail().trajetoria, titulos: 0, foi_campeao: false } }));

    const header = await screen.findByTestId("driver-detail-hero");
    expect(within(header).queryByTestId("driver-detail-titles")).toBeNull();
  });

  it("mostra o que um estreante TEM em vez de um vazio", async () => {
    renderFicha(
      {},
      detail({
        stats_temporada: { corridas: 0, pontos: 0, vitorias: 0, podios: 0, poles: 0, dnfs: 0 },
        stats_carreira: { corridas: 0, pontos: 0, vitorias: 0, podios: 0, poles: 0, dnfs: 0 },
        resumo_atual: { posicao_campeonato: 1, pontos: 0, vitorias: 0, podios: 0 },
        contrato_mercado: {
          contrato: {
            equipe_nome: "Sunday Speed Club",
            papel: "Numero2",
            salario_anual: 120000,
            ano_inicio: 2026,
            ano_fim: 2027,
            anos_restantes: 2,
            status: "ativo",
          },
          mercado: null,
        },
      }),
    );

    await abrirTemporada();
    expect(await screen.findByTestId("driver-detail-rookie-banner")).toBeInTheDocument();
    // O contrato é o que ainda cabe aqui; traços e leitura técnica mudaram de
    // casa para a aba "Perfil", que é a que responde por um piloto sem temporada.
    expect(screen.getByText("Sunday Speed Club")).toBeInTheDocument();
    // Antes da primeira largada o grid inteiro está zerado e a ordem é desempate
    // alfabético: anunciar "Campeonato P1" para quem nunca correu é mentira.
    expect(screen.getByTestId("driver-detail-drawer")).not.toHaveTextContent("P1");
  });

  it("abre a temporada com a posicao e a distancia, e nao com quatro caixas", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: {
          veredito: "Em alta",
          tom: "success",
          posicao_campeonato: 3,
          gap_lider: 8,
          gap_proximo: 2,
          pontos: 15,
          vitorias: 0,
          podios: 1,
          media_recente: 3,
          tendencia: "->",
        },
      }),
    );

    await abrirTemporada();
    const faixa = await screen.findByTestId("driver-detail-verdict");
    // A posição aparece UMA vez, e grande — não mais repartida entre o card de
    // veredito e um MiniMetric "Campeonato".
    expect(screen.getByTestId("driver-detail-championship")).toHaveTextContent("P3");
    expect(faixa).toHaveTextContent("Em alta");

    // P3 a 8 pontos do líder e P3 a 80 são temporadas diferentes: é a distância
    // que diz qual das duas, e ela ocupa o lugar que os zeros ocupavam.
    const linha = screen.getByTestId("driver-detail-standings-line");
    expect(linha).toHaveTextContent("15 pontos");
    expect(linha).toHaveTextContent("8 pontos do líder");
    expect(linha).toHaveTextContent("+2 sobre o P4");
    expect(linha).toHaveTextContent("1 pódio");
    // Zero vitória não é notícia — a ausência já se lê na faixa de forma.
    expect(linha).not.toHaveTextContent("vitória");
  });

  it("chama de lider quem lidera, em vez de dizer que esta a zero do lider", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: { veredito: "Em alta", tom: "success", posicao_campeonato: 1, gap_lider: 0, gap_proximo: 12, pontos: 96 },
      }),
    );

    await abrirTemporada();
    const linha = await screen.findByTestId("driver-detail-standings-line");
    expect(linha).toHaveTextContent("líder do campeonato");
    expect(linha).not.toHaveTextContent("do líder");
    expect(linha).toHaveTextContent("+12 sobre o P2");
  });

  it("quebra a classificacao em linhas com ancora, e nao numa fita de texto", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: {
          veredito: "Bom",
          tom: "success",
          posicao_campeonato: 1,
          gap_lider: 0,
          gap_proximo: 8,
          pontos: 26,
          vitorias: 1,
          podios: 1,
          media_recente: 1,
          tendencia: "->",
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-standings-line");
    // Quatro fatos, quatro linhas: pontos+situação, vantagem, conquistas, média.
    // Num `join(" · ")` os cinco pesavam igual e nenhum era achável de relance.
    expect(bloco.children).toHaveLength(4);
    // A posição virou o assunto da faixa, em corpo grande.
    expect(screen.getByTestId("driver-detail-championship")).toHaveTextContent("P1");
  });

  it("nao gasta uma linha para anunciar que nao houve vitoria nem podio", async () => {
    renderFicha(
      {},
      detail({
        resumo_atual: {
          veredito: "Regular",
          tom: "warning",
          posicao_campeonato: 9,
          gap_lider: 40,
          gap_proximo: 2,
          pontos: 6,
          vitorias: 0,
          podios: 0,
          media_recente: 9,
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-standings-line");
    expect(bloco.children).toHaveLength(3);
    expect(bloco).not.toHaveTextContent("pódio");
  });

  it("desenha o delta contra o pacote dentro da propria faixa", async () => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: {
          entregue_posicao: 2,
          esperado_posicao: 6,
          delta_posicao: 4,
          piloto_pontos: 96,
          companheiro_pontos: 48,
          leitura: "Entrega acima do pacote atual.",
        },
      }),
    );

    await abrirTemporada();
    // "P2 é bom?" só se responde contra o que o pacote prometia — a resposta
    // agora mora ao lado do P2, no espaço que a faixa deixava em branco.
    const faixa = await screen.findByTestId("driver-detail-verdict");
    const delta = within(faixa).getByTestId("driver-detail-performance");
    expect(delta).toHaveTextContent("+4");
    expect(delta).toHaveTextContent("P2 entregue");
    expect(delta).toHaveTextContent("P6 esperado");
    expect(delta).toHaveTextContent("Entrega acima do pacote atual.");
  });

  it("cala o delta quando o pacote nao prometeu nada", async () => {
    renderFicha(
      {},
      detail({ leitura_desempenho: { piloto_pontos: 96, companheiro_pontos: 48, leitura: "-" } }),
    );

    await abrirTemporada();
    await screen.findByTestId("driver-detail-verdict");
    // Sem `esperado_posicao` um "+0" pendurado na faixa afirmaria que o piloto
    // entregou o previsto quando ninguém previu nada.
    expect(screen.queryByTestId("driver-detail-performance")).toBeNull();
  });

  // O zero é o CENTRO da barra. Enquanto ele era a borda e o preenchimento media
  // a fatia do par, qualquer x × 0 dava "100% contra 0%": vencer por 2 e vencer
  // por 15 desenhavam idêntico. Ancorada no meio, a barra cresce com a margem.
  it.each([
    { meus: 2, dele: 0, fill: "5", side: "piloto", ponta: "esquerda" },
    { meus: 15, dele: 0, fill: "38", side: "piloto", ponta: "esquerda" },
    { meus: 20, dele: 0, fill: "50", side: "piloto", ponta: "esquerda" },
    { meus: 40, dele: 0, fill: "50", side: "piloto", ponta: "esquerda" },
    { meus: 0, dele: 15, fill: "38", side: "companheiro", ponta: "direita" },
  ])("cresce do meio com a margem ($meus x $dele)", async ({ meus, dele, fill, side, ponta }) => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: {
          piloto_pontos: meus,
          companheiro_pontos: dele,
          companheiro_nome: "Romain Fournier",
        },
      }),
    );

    await abrirTemporada();
    const barra = await screen.findByTestId("driver-detail-duel-bar");
    expect(barra).toHaveAttribute("data-fill", fill);
    expect(barra).toHaveAttribute("data-side", side);
    // A direção é a PONTA do próprio preenchimento. Como chevron solto ao lado da
    // barra ela boiava no trilho vazio, e na margem saturada escapava para fora.
    expect(within(barra).getByTestId("driver-detail-duel-fill")).toHaveAttribute(
      "data-tip",
      ponta,
    );
    // A margem é lida junto com o desenho, pendurada sobre a ponta — a frase que
    // ficava embaixo virou `aria-label`, para quem não vê o desenho.
    expect(within(barra).getByTestId("driver-detail-duel-margin")).toHaveTextContent(
      `+${Math.abs(meus - dele)}`,
    );
    expect(barra).toHaveAttribute("aria-label", expect.stringContaining("pontos"));
  });

  it("diz em que altura do campeonato o duelo interno acontece", async () => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: {
          piloto_pontos: 18,
          companheiro_pontos: 1,
          companheiro_nome: "Rasmus Thomsen",
          entregue_posicao: 2,
          companheiro_posicao: 14,
          companheiro_nacionalidade: "Dinamarca",
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-teammate");
    // 18 × 1 na briga do título e 18 × 1 no fundo do grid desenham a mesma barra
    // e são duas temporadas diferentes.
    expect(within(bloco).getByTestId("driver-detail-duel-pos-piloto")).toHaveTextContent("P2");
    expect(within(bloco).getByTestId("driver-detail-duel-pos-companheiro")).toHaveTextContent(
      "P14",
    );
    // Uma bandeira por lado — a do piloto aberto e a do companheiro.
    expect(within(bloco).getAllByRole("img", { name: /Brasil|Dinamarca/ })).toHaveLength(2);
  });

  it("nao anuncia posicao no duelo interno antes da primeira largada", async () => {
    renderFicha(
      {},
      detail({
        stats_temporada: { corridas: 0, pontos: 0, vitorias: 0, podios: 0, poles: 0, dnfs: 0 },
        leitura_desempenho: {
          piloto_pontos: 0,
          companheiro_pontos: 0,
          companheiro_nome: "Rasmus Thomsen",
          entregue_posicao: 1,
          companheiro_posicao: 2,
        },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-teammate");
    // Com o grid inteiro zerado a ordem é desempate alfabético — mesmo motivo
    // pelo qual a faixa lá em cima também cala a posição.
    expect(within(bloco).queryByTestId("driver-detail-duel-pos-piloto")).toBeNull();
    expect(within(bloco).queryByTestId("driver-detail-duel-pos-companheiro")).toBeNull();
  });

  it("nao inventa vencedor no duelo interno quando ninguem pontuou", async () => {
    renderFicha(
      {},
      detail({
        leitura_desempenho: { piloto_pontos: 0, companheiro_pontos: 0, companheiro_nome: "X" },
      }),
    );

    await abrirTemporada();
    const bloco = await screen.findByTestId("driver-detail-teammate");
    const barra = within(bloco).getByTestId("driver-detail-duel-bar");
    expect(barra).toHaveAttribute("data-side", "empate");
    expect(barra).toHaveAttribute("data-fill", "0");
    expect(barra).toHaveAttribute("aria-label", "Empatados");
    // Sem margem o rótulo é "0" sobre o zero — nem "+0", que sugeriria vantagem.
    expect(within(barra).getByTestId("driver-detail-duel-margin")).toHaveTextContent("0");
    // E sem vantagem não há direção: uma ponta apontando para algum lado no
    // empate seria a única informação errada da barra.
    expect(within(barra).queryByTestId("driver-detail-duel-fill")).toBeNull();
    // A marca do zero fica mesmo sem preenchimento: é ela que faz a barra
    // significar margem em vez de território.
    expect(within(barra).getByTestId("driver-detail-duel-zero")).toBeInTheDocument();
  });

  // Nada do perfil sai das corridas do ano: traços e níveis técnicos saem dos
  // atributos do piloto e dizem o mesmo em janeiro e em dezembro. Encostados nos
  // números da temporada eles se liam como veredito de forma.
  it("tira o perfil da aba de temporada", async () => {
    renderFicha();

    await abrirTemporada();
    await screen.findByTestId("driver-detail-verdict");
    expect(screen.queryByTestId("driver-detail-profile-strip")).toBeNull();
    expect(screen.queryByTestId("driver-detail-technical")).toBeNull();
  });

  // A fita comanda o realce de tudo o que vem depois: embaixo do que comanda,
  // ela obrigaria a olhar para cima e para baixo ao mesmo tempo.
  it("abre a aba de perfil pelos tracos", async () => {
    renderFicha();

    await abrirPerfil();
    const strip = await screen.findByTestId("driver-detail-profile-strip");
    const blocos = [...strip.parentElement.children].map((node) =>
      node.getAttribute("data-testid"),
    );
    expect(blocos[0]).toBe("driver-detail-profile-strip");
    expect(blocos).toContain("driver-detail-technical");
  });

  it("agrupa a leitura tecnica por onde o eixo se manifesta", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    // Doze eixos numa fita única viram um paredão: cada coluna responde a uma
    // pergunta inteira, e a ordem acompanha o fim de semana.
    const grupos = [...tecnica.querySelectorAll("[data-technical-group]")].map((node) =>
      node.getAttribute("data-technical-group"),
    );
    expect(grupos).toEqual(["volta_seca", "corrida", "condicoes"]);
    expect(tecnica.querySelector("[data-technical='ritmo']")).toHaveTextContent("Elite");
    // Estilo saiu da leitura técnica: vizinho de eixo com nota, o marcador no
    // meio da régua era lido como nota média.
    expect(tecnica.querySelector("[data-technical='agressividade']")).toBeNull();
  });

  // Agressividade não tem lado bom: um piloto agressivo ganha na largada e paga
  // em pneu e em incidente. O eixo É o par de polos — duas palavras e um
  // marcador, e nem nome de eixo nem faixa do meio repetindo a mesma coisa.
  it("reduz o estilo a dois polos e um marcador", async () => {
    renderFicha();

    await abrirPerfil();
    const estilo = await screen.findByTestId("driver-detail-style");
    const agressividade = estilo.querySelector("[data-technical='agressividade']");
    expect(agressividade).toHaveTextContent("Calculista");
    expect(agressividade).toHaveTextContent("Agressivo");
    // O nome do eixo e a faixa do meio saem da tela: eram um terceiro e um quarto
    // rótulo dizendo o que os dois polos e a posição já dizem.
    expect(agressividade).not.toHaveTextContent("Agressividade");
    expect(agressividade.querySelector("[data-technical-marker='estilo']")).toHaveStyle({
      left: "40%",
    });
    // Estilo não ganha barra preenchida em lugar nenhum: preencher diz "mais é
    // melhor", que é exatamente o julgamento que este bloco existe para não fazer.
    expect(agressividade.querySelector("div[style*='width']")).toBeNull();
    // Duas marcas soltas obrigam a medir com o olho; o vão entre elas desenha a
    // distância até o grid como comprimento — 40 contra mediana 62.
    expect(agressividade.querySelector("[data-technical-vao]")).toHaveStyle({
      left: "40%",
      width: "22%",
    });
  });

  // Confiança é o terceiro do trio que sai no roster de IA do iRacing
  // (aggression, smoothness, optimism), e "Fraco em confiança" seria um defeito
  // onde só existe um jeito de correr.
  it("le a confianca como polo e nao como nota", async () => {
    renderFicha();

    await abrirPerfil();
    const estilo = await screen.findByTestId("driver-detail-style");
    const confianca = estilo.querySelector("[data-technical='confianca']");
    expect(confianca).toHaveTextContent("Cauteloso");
    expect(confianca).toHaveTextContent("Confiante");
    expect(screen.getByTestId("driver-detail-technical").querySelector("[data-technical='confianca']")).toBeNull();
  });

  // A régua é o que separa "Forte" de "Muito forte" sem obrigar a decorar a
  // escala de cor.
  it("desenha a regua de cada eixo", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const ritmo = tecnica.querySelector("[data-technical='ritmo'] div[style*='width']");
    expect(ritmo).toHaveStyle({ width: "92%" });
  });

  // Mentalidade era um dos dois atributos que a ficha inteira nunca mostrava — só
  // vazava como chip em Traços quando era extremo — e decide corrida: é ela que
  // resolve clutch/choke sob pressão de campeonato e de duelo. Fica na CORRIDA,
  // que é onde a pressão se manifesta.
  it("mostra a mentalidade, que a ficha nunca mostrou", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const corrida = tecnica.querySelector("[data-technical-group='corrida']");
    expect(corrida.querySelector("[data-technical='mentalidade']")).toHaveTextContent("Forte");
  });

  // A régua se explica OLHANDO — a mediana é linha de referência, mais alta que a
  // régua e mais apagada que o dado. O hover só nomeia essa linha, e num painel
  // com a casca do tooltip do dossiê em vez do balão do sistema operacional.
  it("nomeia a linha da mediana no hover", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    fireEvent.mouseEnter(tecnica.querySelector("[data-technical='ritmo'] [data-technical-regua]"));

    const painel = await screen.findByTestId("driver-detail-regua-tooltip");
    expect(painel).toHaveTextContent("Mediana do grid");
    // Nada de valor bruto: "72 de racecraft" não é linguagem do jogo, e a régua
    // já mostra a magnitude sem precisar soletrá-la.
    expect(painel.textContent).not.toMatch(/\d/);
  });

  // Sem grid para comparar não há linha na régua, e sem linha não há o que
  // explicar: o eixo sai sem alvo de hover em vez de abrir um painel vazio.
  it("nao abre painel no eixo sem mediana", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    fireEvent.mouseEnter(
      tecnica.querySelector("[data-technical='classificacao'] [data-technical-regua]"),
    );

    expect(screen.queryByTestId("driver-detail-regua-tooltip")).toBeNull();
  });

  // "Instável" contra quem, e desde quando: as duas âncoras que a régua abriu.
  it("ancora o eixo na mediana do grid e no ano passado", async () => {
    renderFicha();

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const ritmo = tecnica.querySelector("[data-technical='ritmo']");
    expect(ritmo.querySelector("[data-technical-median]")).toHaveStyle({ left: "51%" });
    expect(ritmo.querySelector("[data-technical-delta='subiu']")).toHaveTextContent("+4");

    const classificacao = tecnica.querySelector("[data-technical='classificacao']");
    expect(classificacao.querySelector("[data-technical-delta='caiu']")).toHaveTextContent("-3");
    // Sem mediana no payload não se inventa um traço no meio da régua.
    expect(classificacao.querySelector("[data-technical-median]")).toBeNull();
  });

  // Payload antigo (save aberto por build anterior) não traz a régua: a linha
  // volta a ser rótulo e nível em vez de desenhar uma barra zerada, que se leria
  // como "este piloto é zero em ritmo".
  it("nao inventa regua quando o payload nao tem escala", async () => {
    renderFicha(
      {},
      detail({
        leitura_tecnica: {
          itens: [{ chave: "ritmo", grupo: "volta_seca", label: "Ritmo", nivel: "Elite", tom: "elite" }],
        },
      }),
    );

    await abrirPerfil();
    const tecnica = await screen.findByTestId("driver-detail-technical");
    expect(tecnica.querySelector("[data-technical='ritmo']")).toHaveTextContent("Elite");
    expect(tecnica.querySelector("[data-technical='ritmo'] div[style*='width']")).toBeNull();
  });

  // A tag JÁ nomeia o eixo em quase todo caso ("Bom Defensor" é defesa): o eixo
  // fica no `title`, e a pílula diz a palavra e mais nada.
  it("mantem os tracos como chips com o eixo so no title", async () => {
    renderFicha();

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const traco = within(perfil).getByText("Rapida").closest("[data-trait]");
    expect(traco).toHaveAttribute("data-trait", "strength");
    expect(traco).toHaveTextContent(/^Rapida$/);
    expect(traco).toHaveAttribute("data-tooltip", "Velocidade");
  });

  // O traço é o pico de um eixo, e a fita não dizia de qual. Passar o mouse
  // responde no lugar onde o dado já está desenhado: o eixo fica, o resto recua.
  it("acende o eixo de origem ao passar por cima do traco", async () => {
    renderFicha();

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const tecnica = await screen.findByTestId("driver-detail-technical");
    const traco = within(perfil).getByText("Rapida").closest("[data-trait]");
    // "Rapida" e uma tag de skill, e skill se chama "ritmo" na leitura tecnica.
    expect(traco).toHaveAttribute("data-trait-eixo", "ritmo");

    const ritmo = tecnica.querySelector("[data-technical='ritmo']");
    const mentalidade = tecnica.querySelector("[data-technical='mentalidade']");
    expect(ritmo.className).not.toMatch(/opacity-25/);
    expect(mentalidade.className).not.toMatch(/opacity-25/);

    fireEvent.mouseEnter(traco);
    // Os DOIS lados: o entorno recua E o alvo acende. Só apagar deixava o olho
    // procurando o buraco no escuro — treze reguas mudavam e a que interessa
    // continuava igual a si mesma.
    expect(ritmo.className).not.toMatch(/opacity-25/);
    expect(ritmo).toHaveAttribute("data-em-foco", "true");
    expect(ritmo.querySelector("div[style*='box-shadow']")).not.toBeNull();
    expect(mentalidade.className).toMatch(/opacity-25/);
    expect(mentalidade).not.toHaveAttribute("data-em-foco");
    // O estilo recua junto: o realce vale para a aba inteira, nao so para o
    // painel em que o eixo caiu.
    const estilo = await screen.findByTestId("driver-detail-style");
    expect(estilo.querySelector("[data-technical='confianca']").className).toMatch(/opacity-25/);

    fireEvent.mouseLeave(traco);
    expect(mentalidade.className).not.toMatch(/opacity-25/);
  });

  // Experiencia, desenvolvimento e midia nao tem regua na leitura tecnica: sem
  // alvo no arco e no estrelato, um terco dos tracos de um novato ("Calouro",
  // "Em Ascensao") teria hover morto.
  it("acende o arco quando o traco nao tem regua propria", async () => {
    renderFicha(
      {},
      detail({
        competitivo: {
          qualidades: [],
          defeitos: [
            { attribute_name: "experiencia", tag_text: "Calouro", level: "defeito_grave", color: "#f85149" },
          ],
          neutro: false,
        },
      }),
    );

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const arco = await screen.findByTestId("driver-detail-arc");
    const traco = within(perfil).getByText("Calouro").closest("[data-trait]");
    expect(traco).toHaveAttribute("data-trait-eixo", "arco:experiencia");

    fireEvent.mouseEnter(traco);
    expect(arco.querySelector("[data-arc='experiencia']").className).not.toMatch(/opacity-25/);
    expect(arco.querySelector("[data-arc='margem']").className).toMatch(/opacity-25/);
  });

  // A fita se lê como um degradê: do que ele tem de melhor ao que ele tem de
  // pior. Sem isso, o nível só existiria como cor, e duas qualidades de peso
  // muito diferente sairiam lado a lado na ordem em que o payload as listou.
  it("ordena os tracos do nivel mais alto ao mais baixo", async () => {
    renderFicha(
      {},
      detail({
        competitivo: {
          qualidades: [
            { attribute_name: "consistencia", tag_text: "Metronomo", level: "qualidade", color: "#3fb950" },
            { attribute_name: "skill", tag_text: "Fenomeno", level: "elite", color: "#bc8cff" },
          ],
          defeitos: [
            { attribute_name: "experiencia", tag_text: "Cru", level: "defeito_grave", color: "#f85149" },
            { attribute_name: "fitness", tag_text: "Sedentario", level: "defeito", color: "#db6d28" },
          ],
          neutro: false,
        },
      }),
    );

    await abrirPerfil();
    const perfil = await screen.findByTestId("driver-detail-profile-strip");
    const niveis = [...perfil.querySelectorAll("[data-trait-level]")].map((no) =>
      no.getAttribute("data-trait-level"),
    );
    expect(niveis).toEqual(["elite", "qualidade", "defeito", "defeito_grave"]);
  });

  // A única pergunta de contratação que a ficha não respondia: sobra estrada?
  it("diz onde o piloto esta na propria curva", async () => {
    renderFicha();

    await abrirPerfil();
    const arco = await screen.findByTestId("driver-detail-arc");
    expect(within(arco).getByTestId("driver-detail-arc-phase")).toHaveTextContent("Em ascensao");
    expect(arco.querySelector("[data-arc='margem']")).toHaveTextContent("Boa");
    expect(arco.querySelector("[data-arc='desenvolvimento']")).toHaveTextContent("Rapida");
    expect(arco.querySelector("[data-arc='experiencia']")).toHaveTextContent("Rodado");
  });

  // "Em ascensão" em corpo grande diz onde ele está e não quanto ainda falta. A
  // faixa das cinco fases responde as duas de uma vez.
  it("situa a fase atual na faixa das cinco", async () => {
    renderFicha();

    await abrirPerfil();
    const faixa = await screen.findByTestId("driver-detail-arc-track");
    const fases = [...faixa.querySelectorAll("[data-arc-phase]")].map((node) =>
      node.getAttribute("data-arc-phase"),
    );
    expect(fases).toEqual(["formacao", "ascensao", "auge", "plato", "crepusculo"]);
    expect(faixa.querySelectorAll("[data-arc-current]")).toHaveLength(1);
    expect(faixa.querySelector("[data-arc-current]")).toHaveAttribute("data-arc-phase", "ascensao");
  });

  // Sem `fase_chave` (payload antigo) a faixa some inteira — melhor nenhuma do
  // que uma com a fase errada acesa.
  it("some com a faixa quando o payload nao diz a fase", async () => {
    renderFicha({}, detail({ arco: { fase: "Em ascensao", tom_fase: "info", resumo: "" } }));

    await abrirPerfil();
    await screen.findByTestId("driver-detail-arc");
    expect(screen.queryByTestId("driver-detail-arc-track")).toBeNull();
  });

  // Teto 0.0 é teto NÃO DERIVADO (jogador e saves antigos), e não teto no chão.
  it("cala o teto quando ninguem mediu o teto", async () => {
    renderFicha(
      {},
      detail({
        arco: {
          idade: 34,
          fase: "No plato",
          tom_fase: "neutral",
          nivel_experiencia: "Veterano",
          nivel_desenvolvimento: "Lenta",
          resumo: "Nao cresce mais, mas ainda entrega.",
        },
      }),
    );

    await abrirPerfil();
    const arco = await screen.findByTestId("driver-detail-arc");
    expect(arco.querySelector("[data-arc='margem']")).toBeNull();
    expect(arco.querySelector("[data-arc='experiencia']")).toHaveTextContent("Veterano");
  });

  // Fama e carisma são traços da PESSOA — no Mercado eles se liam como cláusula.
  it("traz o estrelato do mercado para o perfil", async () => {
    renderFicha(
      {},
      detail({
        estrelato: {
          fama: 74,
          carisma: 61,
          nivel_fama: "Estrela",
          tom_fama: "success",
          nivel_carisma: "Magnetico",
          tom_carisma: "info",
          resumo: "O publico responde.",
        },
      }),
    );

    await abrirPerfil();
    expect(await screen.findByTestId("driver-detail-stardom")).toHaveTextContent("Estrela");

    fireEvent.click(screen.getByTestId("driver-detail-tab-mercado"));
    expect(screen.queryByTestId("driver-detail-stardom")).toBeNull();
  });
});
