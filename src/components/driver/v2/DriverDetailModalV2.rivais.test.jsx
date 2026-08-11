import { fireEvent, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import {
  abrirRivais,
  detail,
  fingeQueRola,
  renderFicha,
  restauraLayout,
  rival,
} from "./driverDetailV2TestKit.jsx";

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

describe("DriverDetailModalV2 — rivais", () => {
  beforeEach(() => {
    mockState = { careerId: "career-1" };
    invoke.mockReset();
    fingeQueRola(true);
  });

  afterEach(restauraLayout);

  // ── Rivais ────────────────────────────────────────────────────────────────
  //
  // A aba mostrava três números numa escala invisível e o valor cru do enum do
  // motor. O que ela precisa provar agora é que o CONFRONTO manda: placar,
  // último encontro e nível nomeado.

  it("abre o confronto direto do rival principal com placar e ultimo encontro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const hero = await screen.findByTestId("driver-detail-rival-hero");
    expect(within(hero).getByText("Tiago Sousa")).toBeInTheDocument();
    // 13 a 10 em 23: o placar e o total sao coisas diferentes, e a diferenca
    // (as duas corridas sem duelo) e informacao, nao arredondamento.
    const duelo = within(hero).getByTestId("driver-detail-duel");
    expect(within(duelo).getByText("13")).toBeInTheDocument();
    expect(within(duelo).getByText("10")).toBeInTheDocument();
    expect(within(duelo).getByText("23 corridas juntos")).toBeInTheDocument();
  });

  // A faixa é o que responde "em QUAIS corridas" — o placar sozinho só diz
  // quanto. O ano vai no lugar do número da temporada porque "T27" não é uma
  // data que alguém reconheça.
  it("desenha a faixa do confronto agrupada por ano e comeca pelo ultimo encontro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    const anos = within(faixa)
      .getAllByText(/^20\d\d$/)
      .map((no) => no.textContent);
    expect(anos).toEqual(["2026", "2027"]);
    // Uma marca por corrida, com a cor dizendo quem ganhou o dia.
    expect(faixa.querySelectorAll("[data-duel-mark]")).toHaveLength(3);
    expect(faixa.querySelectorAll('[data-duel-mark="piloto"]')).toHaveLength(2);

    // Em repouso a legenda mostra o ENCONTRO MAIS RECENTE: quem não passar o
    // mouse em nada leva daqui a última vez que os dois se cruzaram.
    expect(within(faixa).getByText("Interlagos · 2027")).toBeInTheDocument();
    expect(within(faixa).getByTestId("driver-detail-duel-winner")).toHaveTextContent("Ana");
  });

  // Marcas de altura igual desenhavam a mesma coisa para "ele me ganhou por tres
  // decimos" e "ele me tomou uma volta".
  it("da altura as barras conforme o tempo entre os dois, e lado conforme quem venceu", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    const barras = [...faixa.querySelectorAll('[data-duel-band="gap"] rect')];
    const altura = (indice) => Number(barras[indice].getAttribute("height"));
    const topo = (indice) => Number(barras[indice].getAttribute("y"));

    // 0.4s, 12s e 3s: a ordem das alturas e a ordem dos tempos.
    expect(altura(1)).toBeGreaterThan(altura(2));
    expect(altura(2)).toBeGreaterThan(altura(0));
    // Mas a raiz impede que a corrida de 12s achate a de 0.4s contra a linha:
    // trinta vezes mais tempo nao vira trinta vezes mais barra.
    expect(altura(1) / altura(0)).toBeLessThan(30);

    // Quem venceu cresce para CIMA da linha central, quem perdeu para baixo.
    expect(topo(0)).toBeLessThan(20);
    expect(topo(2)).toBeLessThan(20);
    expect(topo(1)).toBe(20);
  });

  it("mostra o tempo da corrida em foco ao lado do nome de quem venceu", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    // Em repouso, o ultimo encontro: Interlagos, 3s de vantagem para a Ana. O
    // espaco entre os dois e margem e nao texto, entao a asserção olha as duas
    // partes.
    const legenda = within(faixa).getByTestId("driver-detail-duel-winner");
    expect(legenda).toHaveTextContent("Ana");
    expect(legenda).toHaveTextContent("3.0s");
  });

  it("troca a legenda da faixa para a corrida sob o ponteiro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    fireEvent.mouseEnter(faixa.querySelectorAll("[data-duel-mark]")[1]);

    expect(within(faixa).getByText("Spa · 2026")).toBeInTheDocument();
    // Só o nome de quem venceu, e não as duas colocações: a pergunta é quem
    // ganhou o dia, e comparar P6 com P1 é o caminho longo para a resposta.
    expect(within(faixa).getByTestId("driver-detail-duel-winner")).toHaveTextContent("Tiago");
  });

  it("marca como sem duelo a corrida em que alguem abandonou", async () => {
    renderFicha(
      {},
      detail({
        rivais: {
          itens: [
            rival({
              encontros: [
                { ano: 2027, season_number: 3, rodada: 8, pista: "Sebring", vencedor: "nenhum" },
              ],
            }),
          ],
        },
      }),
    );
    await abrirRivais();

    const faixa = await screen.findByTestId("driver-detail-duel-timeline");
    expect(within(faixa).getByTestId("driver-detail-duel-winner")).toHaveTextContent("sem duelo");
    expect(faixa.querySelectorAll('[data-duel-mark="nenhum"]')).toHaveLength(1);
  });

  it("nomeia a faixa de intensidade em vez de mostrar o numero cru", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival({ nivel_chave: "forte", intensidade: 66 })] } }));
    await abrirRivais();

    const hero = await screen.findByTestId("driver-detail-rival-hero");
    expect(within(hero).getByText("Rivalidade forte")).toBeInTheDocument();
    expect(within(hero).queryByText("66")).not.toBeInTheDocument();
  });

  it("vai para a ficha do rival por um alvo proprio, e nao pelo card inteiro", async () => {
    const onSelectDriver = vi.fn();
    renderFicha({ onSelectDriver }, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    // O card inteiro nao navega mais: o clique grande e o gesto de abrir a
    // rivalidade, e sequestra-lo para trocar de piloto era o que impedia ver o
    // confronto de qualquer rival que nao fosse o primeiro.
    fireEvent.click(await screen.findByTestId("driver-detail-rival-hero"));
    expect(onSelectDriver).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("driver-detail-rival-open"));
    expect(onSelectDriver).toHaveBeenCalledWith("D9");
  });

  // Num grid fechado todo mundo divide o mesmo numero de corridas, e um piloto
  // de meio de pelotao perde para os tres de cima quase na mesma proporcao: o
  // placar de carreira e justamente o numero que NAO separa esses rivais.
  it("separa rivais de placar parecido pelo recorte recente, pela sequencia e pelo sabado", async () => {
    const corrida = (ano, rodada, vencedor) => ({
      ano,
      season_number: ano === 2025 ? 2 : 3,
      rodada,
      pista: "Spa",
      vencedor,
    });
    const encontros = [
      // Doze antigas de mao unica para o rival...
      ...Array.from({ length: 12 }, (_, indice) => corrida(2025, indice + 1, "rival")),
      // ...e as dez recentes quase empatadas, terminando em quatro seguidas para
      // o dono da ficha.
      ...["rival", "rival", "rival", "piloto", "rival", "piloto", "piloto"].map(
        (vencedor, indice) => corrida(2026, indice + 1, vencedor),
      ),
      // Abandono no meio da sequencia: nao decide nada e nao a quebra.
      corrida(2026, 8, "nenhum"),
      corrida(2026, 9, "piloto"),
      corrida(2026, 10, "piloto"),
    ];
    renderFicha(
      {},
      detail({
        rivais: { itens: [rival({ encontros, confrontos: 22, vitorias: 5, derrotas: 16 })] },
      }),
    );
    await abrirRivais();

    const fatos = await screen.findByTestId("driver-detail-duel-facts");
    const fato = (chave) => fatos.querySelector(`[data-duel-fact="${chave}"]`);

    // Na vida ele apanha de 5 a 16; nas ultimas dez esta 4 a 5. Sao dois
    // retratos diferentes do mesmo par de pessoas, e so o recorte mostra o
    // segundo. O par vem na ordem do placar grande: rival a esquerda, dono da
    // ficha a direita.
    expect(fato("recente")).toHaveTextContent("Últimas 10");
    expect(fato("recente")).toHaveTextContent("4–5");
    // Quatro seguidas, com o abandono atravessado no meio contando como
    // transparente em vez de quebrar a serie.
    expect(fato("sequencia")).toHaveTextContent("4 p/ Ana");
    // O sabado e outro esporte: da para perder o domingo a vida inteira e ainda
    // assim largar na frente.
    expect(fato("quali")).toHaveTextContent("14–9");
  });

  it("mede a distancia do confronto, e nao so a contagem", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const fatos = await screen.findByTestId("driver-detail-duel-facts");
    // O gap diz se a derrota e fotografia ou surra: 13–10 pode ser meio segundo
    // ou meia volta, e o placar nao distingue.
    expect(fatos.querySelector('[data-duel-fact="gap"]')).toHaveTextContent("2.4s");
  });

  // A unica comparacao da ficha sem o carro no meio.
  it("destaca o periodo de box dividido e junta anos seguidos num intervalo", async () => {
    renderFicha(
      {},
      detail({
        rivais: {
          itens: [
            rival({
              companheirismo: {
                equipe: "Aures Racing",
                anos: [2024, 2025, 2026, 2028],
                vitorias: 21,
                derrotas: 15,
              },
            }),
          ],
        },
      }),
    );
    await abrirRivais();

    const dupla = await screen.findByTestId("driver-detail-duel-teammate");
    // Tres anos seguidos sao um periodo, e lista-los item a item contaria como
    // tres fatos o que e um so.
    expect(dupla).toHaveTextContent("Dividiram box na Aures Racing em 2024–2026, 2028.");
    expect(dupla).toHaveTextContent("Mesmo carro: 21 a 15 para Ana sobre Tiago.");
  });

  it("cala o box dividido para quem nunca foi companheiro", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    await screen.findByTestId("driver-detail-rival-hero");
    expect(screen.queryByTestId("driver-detail-duel-teammate")).toBeNull();
  });

  it("cala o recorte recente quando ele seria a carreira inteira de novo", async () => {
    renderFicha({}, detail({ rivais: { itens: [rival()] } }));
    await abrirRivais();

    const fatos = await screen.findByTestId("driver-detail-duel-facts");
    // Tres encontros no total: "ultimas 10" seria o mesmo numero com outro nome.
    expect(fatos.querySelector('[data-duel-fact="recente"]')).toBeNull();
    expect(fatos.querySelector('[data-duel-fact="quali"]')).not.toBeNull();
  });

  it("resume a linha fechada pelo saldo, que ja chega comparavel", async () => {
    renderFicha(
      {},
      detail({
        rivais: {
          itens: [
            rival(),
            rival({ driver_id: "D8", nome: "Eli Green", confrontos: 45, vitorias: 12, derrotas: 32 }),
          ],
        },
      }),
    );
    await abrirRivais();

    const linha = await screen.findByTestId("driver-detail-rival-row");
    // "12–32" e "12–33" obrigam a subtrair para comparar duas linhas.
    expect(linha).toHaveTextContent("−20");
    expect(linha).not.toHaveTextContent("12–32");
  });

  it("rebaixa os rivais seguintes a linha, com o principal em destaque", async () => {
    renderFicha(
      {},
      detail({
        rivais: { itens: [rival(), rival({ driver_id: "D8", nome: "Eli Green", confrontos: 4 })] },
      }),
    );
    await abrirRivais();

    await screen.findByTestId("driver-detail-rival-hero");
    const linhas = screen.getAllByTestId("driver-detail-rival-row");
    expect(linhas).toHaveLength(1);
    expect(linhas[0]).toHaveTextContent("Eli Green");
  });

  // Era isto que estava quebrado: tocar num rival secundario trocava de piloto,
  // entao a unica rivalidade que dava para ler era a primeira.
  it("abre a rivalidade da linha tocada e fecha a que estava aberta", async () => {
    const onSelectDriver = vi.fn();
    renderFicha(
      { onSelectDriver },
      detail({
        rivais: {
          itens: [
            rival(),
            rival({
              driver_id: "D8",
              nome: "Eli Green",
              confrontos: 4,
              vitorias: 1,
              derrotas: 3,
              encontros: [
                { ano: 2027, season_number: 3, rodada: 9, pista: "Suzuka", vencedor: "rival" },
              ],
            }),
          ],
        },
      }),
    );
    await abrirRivais();

    fireEvent.click(await screen.findByTestId("driver-detail-rival-row"));

    // A lista nao reordena: o card aberto nasce onde estava a linha tocada, e o
    // que estava aberto vira linha no lugar dele.
    expect(await screen.findByTestId("driver-detail-rival-hero")).toHaveTextContent("Eli Green");
    expect(screen.getByTestId("driver-detail-rival-row")).toHaveTextContent("Tiago Sousa");
    // Abrir uma rivalidade nao e navegar.
    expect(onSelectDriver).not.toHaveBeenCalled();
    // E o confronto mostrado passa a ser o do rival aberto.
    expect(screen.getByTestId("driver-detail-duel-timeline")).toHaveTextContent("Suzuka · 2027");
  });

  it("ensina como nasce uma rivalidade quando o piloto nao tem nenhuma", async () => {
    renderFicha({}, detail({ rivais: { itens: [] } }));
    await abrirRivais();

    const vazio = await screen.findByTestId("driver-detail-rivals-empty");
    expect(vazio).toHaveTextContent("Ainda ninguém");
    expect(vazio).toHaveTextContent("dividir o mesmo box");
  });

  it("avisa quando os dois nao dividem mais o grid", async () => {
    renderFicha(
      {},
      detail({
        rivais: { itens: [rival({ mesma_categoria: false, categoria_atual: "F3" })] },
      }),
    );
    await abrirRivais();

    expect(await screen.findByTestId("driver-detail-rival-hero")).toHaveTextContent(
      "Corre na F3 — não dividem mais o grid.",
    );
  });

  it("navega para o piloto vizinho pela calha de setas", async () => {
    const onSelectDriver = vi.fn();
    renderFicha({ onSelectDriver });

    await screen.findByTestId("driver-detail-hero");
    fireEvent.click(screen.getByTestId("driver-detail-step-down"));
    expect(onSelectDriver).toHaveBeenCalledWith("D2");
  });
});
