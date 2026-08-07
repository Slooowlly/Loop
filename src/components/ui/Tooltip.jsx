import {
  cloneElement,
  isValidElement,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

// O tooltip do app — o que substitui o `title=` do HTML.
//
// O `title=` nativo é do Windows, e não do Loop: fonte do sistema, fundo
// amarelado, meio segundo de espera que ninguém controla, e um retângulo que
// aparece colado no cursor em vez de ancorado no que ele explica. Dentro de uma
// tela escura de painéis o efeito é o de uma janela de outro programa passando
// por cima.
//
// Este aqui é o mesmo gesto com a casca do app, e a diferença mora em três
// decisões:
//
// 1. VAI NUM PORTAL. Os tooltips do Loop nascem dentro de tabelas, grids e
//    gavetas com `overflow` recortado — um balão absoluto dentro do container
//    seria cortado na borda. Posicionado por `getBoundingClientRect` contra a
//    janela, ele escapa do recorte e vira para cima quando não cabe embaixo.
//
// 2. NÃO RECEBE PONTEIRO. O balão é informação de leitura, não área de
//    interação: `pointer-events: none` impede o caso clássico de o balão
//    aparecer debaixo do cursor, roubar o `mouseleave` do alvo e piscar. Quem
//    precisa de um painel ALCANÇÁVEL — com lista que rola, link para a equipe —
//    quer o `DossierDetailTooltip`, que é outro mecanismo e resolve outro
//    problema.
//
// 3. TEM JANELA QUENTE. O primeiro balão espera; os seguintes, se vierem logo
//    na sequência, abrem na hora. É o que faz varrer uma coluna de badges ser
//    uma leitura contínua em vez de uma espera por célula — e é justamente o
//    que o `title=` nativo não sabe fazer.
//
// 4. SABE FICAR CALADO (`soSeCortado`). Um tooltip que repete o texto que já
//    está na tela é ruído, e o `title=` nativo não tem como evitar isso: posto
//    num `truncate`, ele dispara igual quando o nome coube inteiro e quando o
//    nome foi cortado. Com `soSeCortado` o balão mede o alvo no hover e só abre
//    quando existe texto escondido para revelar — que era a única razão de
//    aquele `title` existir.

const MARGEM = 8; // respiro contra a borda da janela
const AFASTAMENTO = 8; // distância entre o alvo e o balão
const LARGURA_MAX = 280;
const SETA = 8;
const ATRASO_MS = 400;
// Depois de fechar um balão, o próximo abre sem espera se vier dentro disso.
const JANELA_QUENTE_MS = 300;

// Um balão por vez. Sem isso, sair de um alvo direto para o vizinho deixa os
// dois na tela até o primeiro se resolver.
let fecharOAberto = null;
let quenteAte = 0;

function mesclarRefs(...refs) {
  return (no) => {
    for (const ref of refs) {
      if (!ref) continue;
      if (typeof ref === "function") ref(no);
      else ref.current = no;
    }
  };
}

// Texto que cabe não tem o que revelar. `scrollWidth` maior que o `clientWidth`
// é o sinal de que o `truncate` comeu alguma coisa — a folga de 1px absorve o
// arredondamento subpixel que faz um texto justo se declarar cortado.
function estaCortado(no) {
  return no.scrollWidth > no.clientWidth + 1 || no.scrollHeight > no.clientHeight + 1;
}

function encadear(original, nosso) {
  if (!original) return nosso;
  return (evento) => {
    original(evento);
    nosso(evento);
  };
}

const HORIZONTAIS = new Set(["esquerda", "direita"]);

/// Onde o balão cabe no eixo VERTICAL (acima/abaixo do alvo), com o lado pedido
/// respeitado até ele não caber de jeito nenhum.
function posicaoVertical(rect, tamanho, lado) {
  const { width, height } = tamanho;
  const cabeAbaixo = rect.bottom + height + AFASTAMENTO + MARGEM <= window.innerHeight;
  const cabeAcima = rect.top - height - AFASTAMENTO - MARGEM >= 0;

  let paraCima;
  if (lado === "cima") paraCima = cabeAcima || !cabeAbaixo;
  else paraCima = !cabeAbaixo && cabeAcima;

  const topo = paraCima ? rect.top - height - AFASTAMENTO : rect.bottom + AFASTAMENTO;
  const esquerda = Math.min(
    Math.max(MARGEM, rect.left + rect.width / 2 - width / 2),
    Math.max(MARGEM, window.innerWidth - width - MARGEM),
  );

  // A seta mira o CENTRO DO ALVO, e não o centro do balão: perto da borda da
  // janela o balão desliza para caber e os dois centros deixam de coincidir —
  // uma seta no meio do balão apontaria para o lado errado.
  const centro = rect.left + rect.width / 2 - esquerda;

  return {
    top: Math.round(topo),
    left: Math.round(esquerda),
    lado: paraCima ? "cima" : "baixo",
    seta: Math.round(Math.min(Math.max(centro, SETA + 4), Math.max(width - SETA - 4, SETA + 4))),
  };
}

/// Onde o balão cabe no eixo HORIZONTAL (ao lado do alvo).
///
/// Existe para o alvo LARGO — uma linha de tabela ocupa a coluna inteira, e um
/// balão centrado nela pousa em cima justamente do que se está lendo. Ao lado, o
/// conteúdo que motivou o hover continua visível enquanto o balão explica.
function posicaoHorizontal(rect, tamanho, lado) {
  const { width, height } = tamanho;
  const cabeEsquerda = rect.left - width - AFASTAMENTO - MARGEM >= 0;
  const cabeDireita = rect.right + width + AFASTAMENTO + MARGEM <= window.innerWidth;

  let paraEsquerda;
  if (lado === "esquerda") paraEsquerda = cabeEsquerda || !cabeDireita;
  else paraEsquerda = !cabeDireita && cabeEsquerda;

  const esquerda = paraEsquerda ? rect.left - width - AFASTAMENTO : rect.right + AFASTAMENTO;
  const topo = Math.min(
    Math.max(MARGEM, rect.top + rect.height / 2 - height / 2),
    Math.max(MARGEM, window.innerHeight - height - MARGEM),
  );
  const centro = rect.top + rect.height / 2 - topo;

  return {
    top: Math.round(topo),
    left: Math.round(esquerda),
    lado: paraEsquerda ? "esquerda" : "direita",
    seta: Math.round(Math.min(Math.max(centro, SETA + 4), Math.max(height - SETA - 4, SETA + 4))),
  };
}

/// A seta, colada na borda que aponta para o alvo. O quadrado girado é o mesmo
/// nos quatro lados; muda qual par de bordas fica visível e para onde ele gira.
function estiloDaSeta(pos) {
  switch (pos.lado) {
    case "cima":
      return { left: pos.seta - SETA / 2, bottom: -SETA / 2, transform: "rotate(45deg)" };
    case "baixo":
      return { left: pos.seta - SETA / 2, top: -SETA / 2, transform: "rotate(225deg)" };
    case "esquerda":
      return { top: pos.seta - SETA / 2, right: -SETA / 2, transform: "rotate(315deg)" };
    default:
      return { top: pos.seta - SETA / 2, left: -SETA / 2, transform: "rotate(135deg)" };
  }
}

/// O balão, já posicionado. Separado porque ele mede a si mesmo antes de saber
/// onde fica: renderiza invisível, o `useLayoutEffect` lê o tamanho e só então
/// decide de que lado do alvo abrir.
function Balao({ id, corpo, rect, lado, largura }) {
  const balaoRef = useRef(null);
  const [pos, setPos] = useState(null);

  useLayoutEffect(() => {
    const no = balaoRef.current;
    if (!no || !rect) return;

    const tamanho = no.getBoundingClientRect();
    setPos(
      HORIZONTAIS.has(lado)
        ? posicaoHorizontal(rect, tamanho, lado)
        : posicaoVertical(rect, tamanho, lado),
    );
  }, [rect, lado, corpo]);

  return (
    <div
      ref={balaoRef}
      id={id}
      role="tooltip"
      data-testid="tooltip"
      data-lado={pos?.lado ?? "baixo"}
      style={{
        position: "fixed",
        top: pos?.top ?? 0,
        left: pos?.left ?? 0,
        maxWidth: largura ?? LARGURA_MAX,
        zIndex: 200,
        visibility: pos ? "visible" : "hidden",
        opacity: pos ? 1 : 0,
        transition: "opacity 110ms ease-out",
      }}
      className="pointer-events-none"
    >
      <div className="relative rounded-lg border border-white/10 bg-[rgba(13,19,32,0.96)] px-2.5 py-1.5 text-[11px] leading-snug text-text-primary shadow-[0_10px_28px_rgba(0,0,0,0.5)]">
        {corpo}
        {pos ? (
          <span
            aria-hidden="true"
            className="absolute h-2 w-2 border-b border-r border-white/10 bg-[rgba(13,19,32,0.96)]"
            style={estiloDaSeta(pos)}
          />
        ) : null}
      </div>
    </div>
  );
}

/// Envolve o alvo e explica ele no hover ou no foco.
///
/// `texto` para a linha simples (o caso do `title=`), `conteudo` para um nó
/// montado. O filho é CLONADO em vez de embrulhado: os pontos de uso estão
/// espalhados por tabelas e grids onde um `<span>` a mais muda o layout — e a
/// suíte estrutural lê layout como texto.
///
/// `lado` aceita "cima"/"baixo" (o balão pousa acima ou abaixo, centrado no
/// alvo) e "esquerda"/"direita" (o balão pousa AO LADO, centrado na altura do
/// alvo). Os horizontais existem para o alvo largo — numa linha de tabela, o
/// balão centrado cobre a própria linha que se está lendo. `largura` levanta o
/// teto de 280px para o balão que carrega uma tabelinha em vez de uma frase.
export function Tooltip({
  texto,
  conteudo,
  lado = "auto",
  largura,
  desabilitado = false,
  soSeCortado = false,
  children,
}) {
  const corpo = conteudo ?? texto;
  const [rect, setRect] = useState(null);
  const alvoRef = useRef(null);
  const atrasoRef = useRef(null);
  const id = useId();

  const fechar = useCallback(() => {
    clearTimeout(atrasoRef.current);
    if (fecharOAberto === fechar) {
      fecharOAberto = null;
      quenteAte = Date.now() + JANELA_QUENTE_MS;
    }
    setRect(null);
  }, []);

  const abrir = useCallback(() => {
    if (!alvoRef.current) return;
    // Medido na HORA de abrir, e não ao montar: a largura da coluna muda com a
    // janela, e um nome que cabia às 14h pode estar cortado depois de arrastar
    // a borda.
    if (soSeCortado && !estaCortado(alvoRef.current)) return;
    if (fecharOAberto && fecharOAberto !== fechar) fecharOAberto();
    fecharOAberto = fechar;
    setRect(alvoRef.current.getBoundingClientRect());
  }, [fechar, soSeCortado]);

  const agendar = useCallback(() => {
    clearTimeout(atrasoRef.current);
    if (Date.now() < quenteAte) {
      abrir();
      return;
    }
    atrasoRef.current = setTimeout(abrir, ATRASO_MS);
  }, [abrir]);

  useEffect(
    () => () => {
      clearTimeout(atrasoRef.current);
      if (fecharOAberto === fechar) fecharOAberto = null;
    },
    [fechar],
  );

  // Aberto, o balão está preso a uma medida que qualquer rolagem ou
  // redimensionamento invalida — e ele não acompanha, some. O Esc fecha porque
  // um balão aberto por foco de teclado precisa de uma saída de teclado.
  useEffect(() => {
    if (!rect) return undefined;

    function noTeclado(evento) {
      if (evento.key === "Escape") fechar();
    }

    window.addEventListener("scroll", fechar, true);
    window.addEventListener("resize", fechar);
    window.addEventListener("keydown", noTeclado);
    return () => {
      window.removeEventListener("scroll", fechar, true);
      window.removeEventListener("resize", fechar);
      window.removeEventListener("keydown", noTeclado);
    };
  }, [rect, fechar]);

  if (desabilitado || corpo == null || corpo === "" || !isValidElement(children)) return children;

  const gatilho = cloneElement(children, {
    ref: mesclarRefs(alvoRef, children.ref),
    // O texto fica legível no próprio gatilho, e não só no balão que o hover
    // desenha. É o que dá aos testes uma alça estática no lugar do `title=` que
    // eles perderam — e, de quebra, o que faz o inspetor mostrar a explicação
    // sem precisar prender o balão. Só para texto: `conteudo` é um nó montado e
    // não tem forma serializada honesta.
    "data-tooltip": typeof corpo === "string" ? corpo : undefined,
    "aria-describedby": rect ? id : children.props["aria-describedby"],
    onMouseEnter: encadear(children.props.onMouseEnter, agendar),
    onMouseLeave: encadear(children.props.onMouseLeave, fechar),
    // Clicar já disse o que o balão ia dizer; deixá-lo na tela depois do clique
    // é sujeira que sobrevive à ação.
    onMouseDown: encadear(children.props.onMouseDown, fechar),
    onFocus: encadear(children.props.onFocus, abrir),
    onBlur: encadear(children.props.onBlur, fechar),
  });

  return (
    <>
      {gatilho}
      {rect
        ? createPortal(
            <Balao id={id} corpo={corpo} rect={rect} lado={lado} largura={largura} />,
            document.body,
          )
        : null}
    </>
  );
}

export default Tooltip;
