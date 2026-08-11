import { render, screen } from "@testing-library/react";
import { renderHook, act } from "@testing-library/react";

import { ChatBlockedBanner, EngineerRadioCard, PlayerWarningCard, useMaisRecente } from "./EngineerRadio";

// O rádio do engenheiro: o card que aparece por cima do iRacing durante a corrida. A vistoria
// de 10/08/2026 marcou a cadeia inteira do overlay (rádio, PTT, posição) como sem nenhum
// teste, e a memória do projeto já registra um episódio de spotter mudo nessa região.
//
// A peça de risco aqui é o `useMaisRecente`: ele funde três feeds independentes (quebra,
// ritmo, avisos) e a ordenação NÃO pode usar o `id` de cada um — cada feed tem o próprio
// cursor, e o id 3 de um não é mais novo que o id 7 do outro. O que os ordena é a CHEGADA.
// Errar isso não quebra nada visível: só faz o rádio anunciar a mensagem errada, no meio de
// uma corrida, onde ninguém consegue conferir.

// `anunciar` devolve promessa: o card encadeia `.catch` para que falha de áudio não derrube
// a exibição. Um mock que devolve `undefined` quebraria ali — e por um motivo que não existe
// no app.
vi.mock("../lib/engenheiroVoz", () => ({
  anunciar: vi.fn(() => Promise.resolve()),
  pausasDoRadio: vi.fn(() => []),
}));

const msg = (text, extra = {}) => ({ id: 1, text, ...extra });

/// O card revela o texto PALAVRA A PALAVRA, um `<span>` por palavra, então `getByText` com a
/// frase inteira não acha nada. Comparar o texto concatenado é o que corresponde ao que o
/// jogador enxerga.
const textoDoCard = (container) => container.textContent.replace(/\s+/g, " ");

describe("useMaisRecente", () => {
  it("começa sem mensagem", () => {
    const { result } = renderHook(() => useMaisRecente(null, null));
    expect(result.current).toBeNull();
  });

  it("adota a primeira mensagem que aparecer em qualquer feed", () => {
    const { result, rerender } = renderHook(({ a, b }) => useMaisRecente(a, b), {
      initialProps: { a: null, b: null },
    });
    const nova = msg("bateu");
    rerender({ a: null, b: nova });
    expect(result.current).toBe(nova);
  });

  it("a mensagem mais nova substitui a atual, venha do feed que vier", () => {
    const primeira = msg("quebra", { id: 9 });
    const segunda = msg("ritmo", { id: 1 });
    const { result, rerender } = renderHook(({ a, b }) => useMaisRecente(a, b), {
      initialProps: { a: primeira, b: null },
    });
    expect(result.current).toBe(primeira);
    // O id 1 do segundo feed é MAIS NOVO que o id 9 do primeiro: os cursores são
    // independentes. Ordenar por id aqui mostraria a mensagem velha.
    rerender({ a: primeira, b: segunda });
    expect(result.current).toBe(segunda);
  });

  it("segura a mensagem enquanto os feeds não mudam", () => {
    const atual = msg("ficou");
    const { result, rerender } = renderHook(({ a, b }) => useMaisRecente(a, b), {
      initialProps: { a: atual, b: null },
    });
    rerender({ a: atual, b: null });
    rerender({ a: atual, b: null });
    expect(result.current).toBe(atual);
  });

  it("não reexibe a mensagem quando o feed volta a null", () => {
    // O feed zera o cursor depois de entregar. Se `null` apagasse a mensagem atual, o card
    // sumiria da tela um tique depois de aparecer.
    const atual = msg("segura");
    const { result, rerender } = renderHook(({ a }) => useMaisRecente(a), {
      initialProps: { a: atual },
    });
    rerender({ a: null });
    expect(result.current).toBe(atual);
  });

  it("compara por REFERÊNCIA, não por conteúdo", () => {
    // Dois avisos idênticos em sequência são dois eventos, e o rádio precisa reanunciar o
    // segundo. Comparar por texto engoliria a repetição — que é justo o caso do carro que
    // quebra a mesma peça duas vezes.
    const um = msg("perdeu o motor");
    const dois = msg("perdeu o motor");
    const { result, rerender } = renderHook(({ a }) => useMaisRecente(a), {
      initialProps: { a: um },
    });
    rerender({ a: dois });
    expect(result.current).toBe(dois);
  });

  it("prefere o primeiro feed quando dois mudam no mesmo tique", () => {
    // Empate de chegada é resolvido pela ORDEM DOS ARGUMENTOS, e a chamada real passa a
    // quebra antes do ritmo: perder o motor importa mais que um aviso de ritmo.
    const quebra = msg("quebra");
    const ritmo = msg("ritmo");
    const { result, rerender } = renderHook(({ a, b }) => useMaisRecente(a, b), {
      initialProps: { a: null, b: null },
    });
    rerender({ a: quebra, b: ritmo });
    expect(result.current).toBe(quebra);
  });

  it("aguenta três feeds", () => {
    const c = msg("terceiro");
    const { result, rerender } = renderHook(({ a, b, x }) => useMaisRecente(a, b, x), {
      initialProps: { a: null, b: null, x: null },
    });
    rerender({ a: null, b: null, x: c });
    expect(result.current).toBe(c);
  });
});

describe("EngineerRadioCard", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("não desenha nada sem mensagem", () => {
    const { container } = render(<EngineerRadioCard message={null} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("mostra o texto da mensagem", () => {
    const { container } = render(<EngineerRadioCard message={msg("o motor foi")} />);
    expect(textoDoCard(container)).toContain("o motor foi");
  });

  it("acrescenta o detalhe depois do texto, com a inicial em maiúscula", () => {
    // O detalhe chega do Rust em caixa baixa (é fragmento de frase) e entra na tela depois de
    // um travessão, como oração própria. Sem o `capDetail` a linha sai com duas minúsculas
    // seguidas no meio do card.
    const { container } = render(
      <EngineerRadioCard message={msg("o motor foi", { detail: "sem pressão de óleo" })} />,
    );
    expect(textoDoCard(container)).toContain("Sem pressão de óleo");
  });

  it("marca a severidade na classe do card", () => {
    // O CSS pinta a moldura pela severidade. Um valor fora do trio conhecido precisa cair em
    // "light" em vez de gerar uma classe que não existe e deixar o card sem estilo nenhum.
    const classe = (severity) =>
      render(<EngineerRadioCard message={msg("x", { severity })} />).container.firstChild.className;
    expect(classe("dnf")).toContain("sev-dnf");
    expect(classe("heavy")).toContain("sev-heavy");
    expect(classe("qualquer-coisa")).toContain("sev-light");
  });

  it("inicia a saída sozinho depois do tempo de retenção", () => {
    // O card precisa sair sem clique: o jogador está pilotando. Um card que fica preso na
    // tela tapa a pista, que é o pior desfecho possível para um overlay.
    //
    // A remoção do DOM depende do `onTransitionEnd`, que o jsdom não dispara; o que dá para
    // cobrar aqui é a troca de `in` para `out`, que é quem dispara o fade.
    const { container } = render(<EngineerRadioCard message={msg("mensagem")} holdMs={500} />);
    expect(container.firstChild.className).toContain("in");
    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(container.firstChild.className).toContain("out");
  });
});

describe("PlayerWarningCard", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("não desenha nada sem aviso", () => {
    const { container } = render(<PlayerWarningCard message={null} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("mostra o aviso do jogador", () => {
    const { container } = render(<PlayerWarningCard message={msg("cuidado com o pneu")} />);
    expect(textoDoCard(container)).toContain("cuidado com o pneu");
  });

  it("com `mudo`, desenha o card e NÃO fala", async () => {
    // AO VIVO quem fala é a janela principal (`EngenheiroVozAuto`): a de overlay nasce sem
    // foco e clique-atravessa, então o contexto de áudio dela nunca sai de "suspended" — o
    // engenheiro ficava mudo enquanto o spotter, que já morava na principal, era ouvido.
    // O card continua aqui; se os dois falassem, o piloto ouviria a frase duas vezes.
    const { anunciar } = await import("../lib/engenheiroVoz");
    anunciar.mockClear();
    const { container } = render(
      <PlayerWarningCard message={msg("o carro não tem conserto", { pecas: ["meu_quali_dq"] })} mudo />,
    );
    expect(textoDoCard(container)).toContain("o carro não tem conserto");
    expect(anunciar).not.toHaveBeenCalled();
  });

  it("sem `mudo`, fala as peças (o caminho do demo)", async () => {
    const { anunciar } = await import("../lib/engenheiroVoz");
    anunciar.mockClear();
    render(<PlayerWarningCard message={msg("aviso", { pecas: ["meu_quali_eol"] })} />);
    expect(anunciar).toHaveBeenCalledTimes(1);
  });
});

describe("ChatBlockedBanner", () => {
  it("fica calado quando o chat não está bloqueado", () => {
    const { container } = render(<ChatBlockedBanner blocked={false} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("aparece quando o chat está bloqueado", () => {
    // O bloqueio do chat é o que impede a penalidade de chegar ao sim. Sem o aviso, o
    // jogador acha que a bandeira preta não funciona.
    const { container } = render(<ChatBlockedBanner blocked />);
    expect(container).not.toBeEmptyDOMElement();
  });
});
