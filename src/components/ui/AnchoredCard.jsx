import {
  cloneElement,
  isValidElement,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

// A ficha ancorada — o painel que abre no clique, preso ao que o explica.
//
// É o degrau entre o `Tooltip` e a gaveta inteira. O tooltip é uma linha e não
// recebe ponteiro; a gaveta cobre a tela que o jogador está comparando e custa
// um Esc para voltar. No mercado, onde a mesma pergunta ("quem é esse, como está
// esse time") se repete quarenta vezes numa tela, o que falta é um painel que
// responda sem tirar o grid de baixo dele.
//
// Três decisões o separam do `Tooltip`:
//
// 1. ABRE NO CLIQUE, não no hover. Varrer um grid com o mouse passa por dez
//    alvos até chegar no que interessa; um painel deste tamanho aparecendo em
//    cada um seria um estroboscópio. O clique é o gesto que diz "este".
//
// 2. RECEBE PONTEIRO. Tem conteúdo selecionável e um botão de fechar — é painel,
//    e não balão de leitura. Por isso também precisa fechar no clique de fora,
//    no Esc e na rolagem: ancorado a um retângulo medido, ele não acompanha a
//    página, ficaria flutuando longe do que explica.
//
// 3. VAI NUM PORTAL. Os cards do grid recortam o `overflow`, e um painel
//    absoluto dentro do card seria decepado na borda.
//
// O CLONE do filho, em vez de um `<button>` por fora, é a mesma razão do
// Tooltip: os pontos de uso são linhas de grid com `truncate` e `flex-1`, e um
// nó a mais mudaria o layout que a suíte estrutural lê como texto.

const MARGEM = 12;
const AFASTAMENTO = 8;

// Uma ficha por vez. Sem isso, clicar num segundo alvo deixava as duas na tela,
// e a de baixo cobria justamente a linha que a de cima explicava.
let fecharAAberta = null;

function mesclarRefs(...refs) {
  return (no) => {
    for (const ref of refs) {
      if (!ref) continue;
      if (typeof ref === "function") ref(no);
      else ref.current = no;
    }
  };
}

function encadear(original, nosso) {
  if (!original) return nosso;
  return (evento) => {
    original(evento);
    nosso(evento);
  };
}

/// Uma faixa de números da ficha. O rótulo em miúdo embaixo, o número grande em
/// cima — a leitura de varredura é pelo número, e o rótulo só desempata.
export function NumeroDaFicha({ rotulo, valor, destaque = false }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col items-center gap-0.5">
      <span
        className="text-[15px] font-black leading-none tabular-nums"
        style={{ color: destaque && valor > 0 ? "#ffd700" : "var(--text-primary)" }}
      >
        {valor ?? 0}
      </span>
      <span className="truncate text-[8.5px] font-bold uppercase tracking-[0.1em] text-[color:var(--text-muted)]">
        {rotulo}
      </span>
    </div>
  );
}

/// Um bloco da ficha, com a cinta de título que separa um assunto do próximo.
///
/// `cor` tinge a cinta e acende uma barra à esquerda. É para o bloco que fala de
/// UMA entidade já nomeada acima na ficha: repetir "Na Roadster Touring Club"
/// três centímetros abaixo do cabeçalho que diz "Roadster Touring Club" gasta a
/// linha inteira para não dizer nada — a cor faz o vínculo sem escrever.
export function SecaoDaFicha({ titulo, cor, children }) {
  return (
    <div className="border-t border-white/[0.08] px-3 py-2.5">
      <p
        className="mb-1.5 flex items-center gap-1.5 text-[8.5px] font-bold uppercase tracking-[0.18em]"
        style={{ color: cor ?? "var(--text-muted)" }}
      >
        {cor && (
          <span
            aria-hidden="true"
            className="h-2.5 w-[3px] shrink-0 rounded-full"
            style={{ background: cor }}
          />
        )}
        {titulo}
      </p>
      {children}
    </div>
  );
}

// O painel posicionado. Ele mede a si mesmo antes de saber onde fica: renderiza
// invisível, o `useLayoutEffect` lê a altura e só então decide entre abrir para
// baixo ou virar para cima.
function Painel({ rect, largura, rotuloFechar, testId, onClose, children }) {
  const painelRef = useRef(null);
  const [pos, setPos] = useState(null);

  // Remedido a cada troca de conteúdo: a ficha que busca dados nasce com a
  // altura do "carregando" e cresce quando eles chegam. Sem remedir, uma ficha
  // aberta perto do rodapé cresceria para fora da janela.
  useLayoutEffect(() => {
    const no = painelRef.current;
    if (!no || !rect) return;

    const { height } = no.getBoundingClientRect();
    const cabeAbaixo = rect.bottom + height + AFASTAMENTO + MARGEM <= window.innerHeight;
    const cabeAcima = rect.top - height - AFASTAMENTO - MARGEM >= 0;
    const paraCima = !cabeAbaixo && cabeAcima;

    const topo = paraCima
      ? rect.top - height - AFASTAMENTO
      : Math.min(rect.bottom + AFASTAMENTO, Math.max(MARGEM, window.innerHeight - height - MARGEM));
    const esquerda = Math.min(
      Math.max(MARGEM, rect.left + rect.width / 2 - largura / 2),
      Math.max(MARGEM, window.innerWidth - largura - MARGEM),
    );

    setPos({ top: Math.round(topo), left: Math.round(esquerda) });
  }, [rect, largura, children]);

  return (
    <div
      ref={painelRef}
      role="dialog"
      data-testid={testId}
      style={{
        position: "fixed",
        top: pos?.top ?? 0,
        left: pos?.left ?? 0,
        width: largura,
        zIndex: 210,
        visibility: pos ? "visible" : "hidden",
        opacity: pos ? 1 : 0,
        transition: "opacity 120ms ease-out",
      }}
      onMouseDown={(evento) => evento.stopPropagation()}
    >
      <div className="relative overflow-hidden rounded-xl border border-white/[0.12] bg-[rgba(13,19,32,0.98)] shadow-[0_18px_48px_rgba(0,0,0,0.6)]">
        <button
          type="button"
          onClick={onClose}
          aria-label={rotuloFechar}
          className="absolute right-1.5 top-1.5 z-10 flex h-5 w-5 items-center justify-center rounded-md text-[12px] leading-none text-[color:var(--text-muted)] transition-colors hover:bg-white/10 hover:text-[color:var(--text-primary)]"
        >
          ×
        </button>
        {children}
      </div>
    </div>
  );
}

/// Envolve o alvo e abre a ficha no clique.
///
/// `desabilitado` devolve o filho intacto — assento sem ocupante resolvido não
/// tem ficha para abrir, e um alvo que parece clicável e não abre nada é pior do
/// que um alvo que nunca prometeu abrir.
export default function AnchoredCard({
  largura = 320,
  rotuloFechar,
  testId = "anchored-card",
  desabilitado = false,
  realce = "hover:underline focus-visible:underline",
  conteudo,
  children,
}) {
  const [rect, setRect] = useState(null);
  const alvoRef = useRef(null);

  const fechar = useCallback(() => {
    if (fecharAAberta === fechar) fecharAAberta = null;
    setRect(null);
  }, []);

  const alternar = useCallback(
    (evento) => {
      // O alvo mora dentro de cards que já têm gesto próprio (duplo clique abre
      // o histórico da equipe, o card inteiro é clicável). Abrir a ficha não
      // pode disparar nada disso junto.
      evento.stopPropagation();
      if (!alvoRef.current) return;
      if (fecharAAberta && fecharAAberta !== fechar) fecharAAberta();
      setRect((atual) => {
        if (atual) {
          fecharAAberta = null;
          return null;
        }
        fecharAAberta = fechar;
        return alvoRef.current.getBoundingClientRect();
      });
    },
    [fechar],
  );

  useEffect(
    () => () => {
      if (fecharAAberta === fechar) fecharAAberta = null;
    },
    [fechar],
  );

  useEffect(() => {
    if (!rect) return undefined;

    function noTeclado(evento) {
      if (evento.key === "Escape") fechar();
    }

    function foraDaFicha(evento) {
      if (alvoRef.current?.contains(evento.target)) return;
      if (evento.target.closest?.(`[data-testid='${testId}']`)) return;
      fechar();
    }

    document.addEventListener("mousedown", foraDaFicha);
    window.addEventListener("scroll", fechar, true);
    window.addEventListener("resize", fechar);
    window.addEventListener("keydown", noTeclado);
    return () => {
      document.removeEventListener("mousedown", foraDaFicha);
      window.removeEventListener("scroll", fechar, true);
      window.removeEventListener("resize", fechar);
      window.removeEventListener("keydown", noTeclado);
    };
  }, [rect, fechar, testId]);

  if (desabilitado || !isValidElement(children)) return children;

  const gatilho = cloneElement(children, {
    ref: mesclarRefs(alvoRef, children.ref),
    role: "button",
    tabIndex: 0,
    // Um alvo que abre uma ficha nunca é decorativo. O logo da equipe nasce
    // `aria-hidden` — marca muda ao lado do nome escrito —, e virar gatilho sem
    // desfazer isso deixaria um elemento focável escondido do leitor de tela.
    "aria-hidden": undefined,
    "aria-haspopup": "dialog",
    "aria-expanded": Boolean(rect),
    className: `${children.props.className ?? ""} cursor-pointer outline-none ${realce}`.trim(),
    onClick: encadear(children.props.onClick, alternar),
    // O duplo clique é do card da equipe; no alvo ele só repetiria o gesto de
    // abrir e fechar a ficha duas vezes e ainda levaria o histórico junto.
    onDoubleClick: encadear(children.props.onDoubleClick, (evento) => evento.stopPropagation()),
    onKeyDown: encadear(children.props.onKeyDown, (evento) => {
      if (evento.key === "Enter" || evento.key === " ") {
        evento.preventDefault();
        alternar(evento);
      }
    }),
  });

  return (
    <>
      {gatilho}
      {rect
        ? createPortal(
            <Painel
              rect={rect}
              largura={largura}
              rotuloFechar={rotuloFechar}
              testId={testId}
              onClose={fechar}
            >
              {conteudo}
            </Painel>,
            document.body,
          )
        : null}
    </>
  );
}
