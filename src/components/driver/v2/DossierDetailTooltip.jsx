import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";

import FlagIcon from "../../ui/FlagIcon";
import Tooltip from "../../ui/Tooltip";
import TeamLogoMark from "../../team/TeamLogoMark";
import { extractNationalityLabel } from "../../../utils/formatters";
import { formatCategoryLabel } from "../detalhes/formatadores";
import { getCategoryColor } from "../../../utils/categoryColors";
import { getReadableTeamColor } from "../../../utils/teamColors";

// O detalhe por trás de cada número do dossiê.
//
// O dossiê é uma parede de contadores: "4 equipes", "2 promoções", "19 poles",
// "74ª corrida". O número responde QUANTO e esconde o resto — quais equipes, de
// qual degrau para qual, por qual time, em que ano. Este painel é onde esse
// resto mora, e ele abre no hover porque a informação é de segunda ordem: ela
// não pode competir com o número, só estar a um movimento de distância.
//
// Vai num portal e posicionado por `getBoundingClientRect` porque os cards do
// dossiê vivem num grid com `overflow` recortado — um painel absoluto dentro do
// card seria cortado na borda.

const LARGURA = 320;
const MARGEM = 12;
// A bolinha, no espírito dos tooltips da Paradox. Enquanto ela fecha o círculo o
// painel é volátil — tirar o mouse da linha fecha na hora. Quando ela fecha, o
// painel PRENDE: passa a viver por conta própria e a lista com barra de rolagem
// finalmente fica alcançável. É o que o fade tentava resolver e resolvia mal,
// porque um painel que já está sumindo é um alvo em movimento.
//
// O círculo só corre com o cursor PARADO: qualquer movimento sobre a linha
// reinicia. É essa regra que faz o gesto ser deliberado em vez de acidental —
// sem ela, atravessar o dossiê prende tudo por onde o mouse passou.
const FIXAR_MS = 700;
// Preso, ele só solta depois de o mouse ficar esse tempo longe da linha E do
// painel. Voltar para qualquer um dos dois cancela a contagem. Mas o tempo é a
// última saída, não a única: o × e o Esc soltam na hora.
const SOLTAR_MS = 5000;
// Movimento abaixo disso (em px, distância de quarteirão) é tremor de mão, não
// intenção de sair da linha.
const PARADO_PX = 2;

const RAIO = 6;
const PERIMETRO = 2 * Math.PI * RAIO;
const COR_BOLINHA = "#58a6ff";

// Um painel preso por vez. Prender o segundo deixava o primeiro na tela pelos
// 5 segundos inteiros — e como o painel preso cobre as linhas vizinhas, prender
// virava empilhar, que foi exatamente o que apareceu na tela.
let soltarOPresoAtual = null;

function assumirOPino(soltar) {
  if (soltarOPresoAtual && soltarOPresoAtual !== soltar) soltarOPresoAtual();
  soltarOPresoAtual = soltar;
}

function largarOPino(soltar) {
  if (soltarOPresoAtual === soltar) soltarOPresoAtual = null;
}

// O anel enchendo, no canto do painel. Some assim que o painel prende: ali o
// lugar passa a ser do ×, que é a única affordance visível de soltar.
function Bolinha() {
  const [fechado, setFechado] = useState(false);

  useEffect(() => {
    const id = requestAnimationFrame(() => setFechado(true));
    return () => cancelAnimationFrame(id);
  }, []);

  return (
    <svg viewBox="0 0 20 20" width="18" height="18" aria-hidden="true" data-testid="dossier-detail-bolinha">
      <circle cx="10" cy="10" r={RAIO} fill="#0b1522" stroke="rgba(255,255,255,0.16)" strokeWidth="2" />
      <circle
        cx="10"
        cy="10"
        r={RAIO}
        fill="none"
        stroke={COR_BOLINHA}
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray={PERIMETRO}
        strokeDashoffset={fechado ? 0 : PERIMETRO}
        transform="rotate(-90 10 10)"
        style={{ transition: `stroke-dashoffset ${FIXAR_MS}ms linear` }}
      />
    </svg>
  );
}

// Vira botão só quando há para onde ir: equipe que sumiu do mundo não tem tela
// no Atlas, e um alvo que não abre nada é pior do que nenhum alvo.
//
// `nomeia` só na porta do LOGO. As duas portas levam à mesma equipe, mas a do
// texto já mostra o nome — um balão ali diria "Rocket Motorsport" por cima de
// "Rocket Motorsport". O logo é que é a marca muda e precisa ser nomeada.
function AberturaDaEquipe({ entrada, abrivel, onAbrirEquipe, nomeia = false, children }) {
  if (!abrivel) return children;

  const botao = (
    <button
      type="button"
      data-testid="dossier-detail-abrir-equipe"
      onClick={() =>
        onAbrirEquipe({
          id: entrada.equipe_id,
          nome: entrada.equipe,
          cor_primaria: entrada.equipe_cor,
        })
      }
      className="flex min-w-0 shrink-0 cursor-pointer items-center rounded outline-none focus-visible:ring-1 focus-visible:ring-[#58a6ff]"
    >
      {children}
    </button>
  );

  return nomeia ? <Tooltip texto={entrada.equipe}>{botao}</Tooltip> : botao;
}

function LinhaDetalhe({ entrada, legenda, reservaEquipe, onAbrirEquipe }) {
  const { t } = useTranslation();
  const abrivel = Boolean(entrada.equipe_id && onAbrirEquipe);

  // A legenda ganha do texto da linha: ela existe justamente nos painéis em que
  // a entrada não se explica sozinha — "Equipe de estreia" e "Equipe atual" nas
  // duas pontas da carreira, que sem rótulo passavam por lista de equipes.
  const titulo =
    legenda ??
    (entrada.categoria_origem
      ? `${formatCategoryLabel(entrada.categoria_origem)} → ${formatCategoryLabel(entrada.categoria)}`
      : entrada.texto);

  const quando = [
    anoDe(entrada),
    Number.isFinite(entrada.rodada) ? t("driverDetail.history.tooltipRound", { round: entrada.rodada }) : null,
    entrada.pista,
  ]
    .filter(Boolean)
    .join(" · ");

  // Sem `categoria_origem` a categoria é um rótulo comum, com a cor da escada.
  const categoria = !entrada.categoria_origem && entrada.categoria ? entrada.categoria : null;
  const corDaEquipe = getReadableTeamColor(entrada.equipe_cor);

  return (
    <li className="flex items-center gap-2.5 border-b border-white/[0.06] py-1.5 last:border-b-0 last:pb-0">
      {entrada.equipe ? (
        // O logo e o nome são a MESMA porta, e não duas. Quem quer ver a equipe
        // mira em qualquer um dos dois — exigir acertar o texto seria um alvo de
        // 12px numa lista que rola.
        <AberturaDaEquipe entrada={entrada} abrivel={abrivel} onAbrirEquipe={onAbrirEquipe} nomeia>
          <TeamLogoMark
            teamName={entrada.equipe}
            color={entrada.equipe_cor}
            size="xs"
            halo
            testId="dossier-detail-logo"
          />
        </AberturaDaEquipe>
      ) : reservaEquipe ? (
        // Sem equipe a linha ficava sem o slot do logo e o nome encostava na
        // borda, desalinhado do resto da lista. O X ocupa a mesma caixa do
        // `xs` e diz o que a ausência significa: hoje ele não corre por ninguém.
        <Tooltip texto={t("driverDetail.history.tooltipNoTeam")}>
          <span
            data-testid="dossier-detail-sem-equipe"
            aria-label={t("driverDetail.history.tooltipNoTeam")}
            className="flex h-6 w-[36px] shrink-0 items-center justify-center rounded-md border border-dashed border-white/12 text-text-muted"
          >
            <svg viewBox="0 0 10 10" width="9" height="9" aria-hidden="true">
              <path
                d="M1.5 1.5 L8.5 8.5 M8.5 1.5 L1.5 8.5"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
                fill="none"
              />
            </svg>
          </span>
        </Tooltip>
      ) : null}
      <span className="min-w-0 flex-1">
        {/* Centro comum, e não linha de base: o rótulo tem 12px, a chave da
            categoria tem 10px, e alinhados pela base a categoria afundava e o
            rótulo parecia flutuar acima do nome da equipe. */}
        <span className="flex flex-wrap items-center gap-x-1.5 gap-y-0.5">
          {titulo ? <strong className="text-xs font-semibold text-text-primary">{titulo}</strong> : null}
          {entrada.equipe ? (
            <AberturaDaEquipe entrada={entrada} abrivel={abrivel} onAbrirEquipe={onAbrirEquipe}>
              <span
                className={`truncate text-xs font-semibold ${abrivel ? "underline-offset-2 hover:underline" : ""}`}
                style={{ color: corDaEquipe }}
              >
                {entrada.equipe}
              </span>
            </AberturaDaEquipe>
          ) : null}
          {categoria ? (
            <span className="text-[10px] font-semibold" style={{ color: getCategoryColor(categoria) }}>
              {formatCategoryLabel(categoria)}
            </span>
          ) : null}
        </span>
        {quando ? <span className="mt-0.5 block text-[11px] text-text-secondary">{quando}</span> : null}
        {entrada.nacionalidade || Number.isFinite(entrada.idade) ? (
          // A bandeira vai como ARTE, e não como o emoji que veio na string: o
          // Windows não tem glifo de bandeira nacional, e o par de indicadores
          // regionais caía para as duas letras cruas ("AU", "SE") na ficha.
          <span className="mt-0.5 flex items-center gap-1.5 text-[11px] text-text-muted">
            {entrada.nacionalidade ? (
              <FlagIcon
                nacionalidade={entrada.nacionalidade}
                variant="natural"
                className="h-3 w-auto shrink-0"
              />
            ) : null}
            <span className="min-w-0 truncate">
              {[
                // Só o gentílico: o emoji que veio colado na string já virou
                // arte no `FlagIcon` ao lado.
                extractNationalityLabel(entrada.nacionalidade),
                Number.isFinite(entrada.idade)
                  ? t("driverDetail.history.tooltipAge", { count: entrada.idade })
                  : null,
              ]
                .filter(Boolean)
                .join(" · ")}
            </span>
          </span>
        ) : null}
      </span>
      <span className="shrink-0 text-right">
        {Number.isFinite(entrada.melhor_resultado) ? (
          <span className="block text-[11px] font-semibold text-text-primary">
            {t("driverDetail.history.tooltipBestFinish", { position: entrada.melhor_resultado })}
          </span>
        ) : null}
        {entrada.resumo ? (
          <span className="block font-mono text-[11px] text-text-primary">{entrada.resumo}</span>
        ) : null}
        {Number.isFinite(entrada.contagem) ? (
          <span className="block text-[10px] text-text-muted">{entrada.contagem}</span>
        ) : null}
      </span>
    </li>
  );
}

// O ANO, e não o dia. Saber que a primeira vitória foi em 2008 responde a
// pergunta; saber que foi em 3 de julho é precisão que ninguém pediu e que rouba
// a linha da rodada e da pista, que dizem muito mais.
function anoDe(entrada) {
  if (entrada.data) {
    const ano = String(entrada.data).slice(0, 4);
    if (/^\d{4}$/.test(ano)) return ano;
  }
  return entrada.periodo ?? null;
}

export function DossierDetailTooltip({ entradas, legendas, onAbrirEquipe, className = "", children }) {
  const { t } = useTranslation();
  const [rect, setRect] = useState(null);
  const [preso, setPreso] = useState(false);
  const [carregando, setCarregando] = useState(false);
  const [altura, setAltura] = useState(0);
  // Prender só existe onde há o que alcançar. A maioria dos painéis do dossiê
  // cabe inteira na tela e não rola — neles a bolinha e a legenda são um
  // mecanismo oferecido para um problema que aquele painel não tem.
  const [rolavel, setRolavel] = useState(false);
  // O mesmo valor em ref: a bolinha arma no hover, ANTES de o painel existir
  // para ser medido, e o `setTimeout` congelaria um `rolavel` que ainda era
  // falso. O estado desenha; a ref é quem decide no fim da contagem.
  const rolavelRef = useRef(false);
  const alvoRef = useRef(null);
  const painelRef = useRef(null);
  const listaRef = useRef(null);
  const fixarRef = useRef(null);
  const soltarRef = useRef(null);
  const posRef = useRef(null);
  // Depois de soltar no botão do meio a bolinha não rearma enquanto o mouse não
  // sair de vez: senão soltar seria inútil, o painel prenderia de novo em 700ms.
  const rearmarRef = useRef(true);
  // Reinicia o anel a cada movimento do cursor, remontando o SVG.
  const [ciclo, setCiclo] = useState(0);

  const lista = Array.isArray(entradas) ? entradas : [];

  useLayoutEffect(() => {
    if (painelRef.current) setAltura(painelRef.current.offsetHeight);
    const lista = listaRef.current;
    const rola = !!lista && lista.scrollHeight > lista.clientHeight + 1;
    rolavelRef.current = rola;
    setRolavel(rola);
    // `rolavel` e `preso` entram nas dependências porque os dois fazem a legenda
    // aparecer, e a legenda muda a altura de que o posicionamento para cima
    // depende — um painel curto preso no botão do meio ganha rodapé.
  }, [rect, entradas, rolavel, preso]);

  const fechar = useCallback(() => {
    clearTimeout(fixarRef.current);
    clearTimeout(soltarRef.current);
    largarOPino(fechar);
    setRect(null);
    setPreso(false);
    setCarregando(false);
    rearmarRef.current = true;
  }, []);

  // Desmontar com o pino na mão deixaria o registro global apontando para um
  // componente que não existe mais — e o próximo a prender chamaria esse fantasma.
  useEffect(
    () => () => {
      clearTimeout(fixarRef.current);
      clearTimeout(soltarRef.current);
      largarOPino(fechar);
    },
    [fechar],
  );

  // Quem decide se a BOLINHA vale é o FIM da contagem, e não o começo dela — ela
  // só existe onde há lista para alcançar. O botão do meio passa por cima disso
  // (`forcado`): prender um painel curto para ler com calma é uma vontade
  // legítima, e o gesto é deliberado o bastante para nunca acontecer sozinho.
  const prender = useCallback(
    (forcado = false) => {
      setCarregando(false);
      if (!forcado && !rolavelRef.current) return;
      assumirOPino(fechar);
      setPreso(true);
    },
    [fechar],
  );

  const armar = useCallback(() => {
    clearTimeout(fixarRef.current);
    setCarregando(true);
    setCiclo((n) => n + 1);
    fixarRef.current = setTimeout(prender, FIXAR_MS);
  }, [prender]);

  // Vale para a linha e para o próprio painel: os dois contam como "perto".
  const entrar = useCallback(() => {
    clearTimeout(soltarRef.current);
    if (alvoRef.current) setRect(alvoRef.current.getBoundingClientRect());
    if (preso || carregando || !rearmarRef.current) return;
    armar();
  }, [preso, carregando, armar]);

  // A regra da Paradox: o anel só corre com o cursor parado. Sem isso, atravessar
  // o dossiê devagar prende cada linha do caminho.
  const mover = useCallback(
    (event) => {
      if (preso || !rolavelRef.current || !rearmarRef.current) return;
      const anterior = posRef.current;
      posRef.current = { x: event.clientX, y: event.clientY };
      if (!anterior) return;
      const andou = Math.abs(anterior.x - event.clientX) + Math.abs(anterior.y - event.clientY);
      if (andou >= PARADO_PX) armar();
    },
    [preso, armar],
  );

  const sair = useCallback(() => {
    clearTimeout(fixarRef.current);
    setCarregando(false);
    posRef.current = null;
    if (!preso) {
      fechar();
      return;
    }
    clearTimeout(soltarRef.current);
    soltarRef.current = setTimeout(fechar, SOLTAR_MS);
  }, [preso, fechar]);

  // O atalho de quem não quer esperar a bolinha, nos dois sentidos.
  const alternarPreso = useCallback(
    (event) => {
      if (event.button !== 1) return;
      event.preventDefault(); // mata o autoscroll do navegador
      clearTimeout(fixarRef.current);
      clearTimeout(soltarRef.current);
      setCarregando(false);
      if (preso) {
        rearmarRef.current = false;
        largarOPino(fechar);
        setPreso(false);
        return;
      }
      if (alvoRef.current) setRect(alvoRef.current.getBoundingClientRect());
      rearmarRef.current = true;
      prender(true);
    },
    [preso, prender, fechar],
  );

  // Preso, o painel é uma janelinha — e janelinha fecha no Esc e no clique fora.
  // O Esc precisa vir na captura e cortar a propagação: quem escuta depois é o
  // `keydown` da própria ficha, que fecharia o modal inteiro por baixo.
  useEffect(() => {
    if (!preso) return undefined;

    function noTeclado(event) {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      fechar();
    }

    function noPonteiro(event) {
      if (event.button === 1) return; // o botão do meio tem dono
      const dentro =
        painelRef.current?.contains(event.target) || alvoRef.current?.contains(event.target);
      if (!dentro) fechar();
    }

    window.addEventListener("keydown", noTeclado, true);
    window.addEventListener("mousedown", noPonteiro, true);
    return () => {
      window.removeEventListener("keydown", noTeclado, true);
      window.removeEventListener("mousedown", noPonteiro, true);
    };
  }, [preso, fechar]);

  const linhas = useMemo(() => {
    // O X só faz sentido onde a equipe É a informação e falta em ALGUMA linha —
    // categorias disputadas e promoções não têm equipe por natureza, e ali um
    // slot vazio em toda linha seria um buraco anunciando nada.
    const reservaEquipe = lista.some((entrada) => entrada.equipe);
    return lista.map((entrada, index) => (
      <LinhaDetalhe
        key={`${entrada.periodo ?? ""}-${entrada.texto ?? ""}-${index}`}
        entrada={entrada}
        legenda={legendas?.[index]}
        reservaEquipe={reservaEquipe}
        onAbrirEquipe={onAbrirEquipe}
      />
    ));
  }, [lista, legendas, onAbrirEquipe]);

  // Sem detalhe o alvo sai do caminho por inteiro — nem o `cursor-help` nem a
  // parada de tabulação sobram anunciando um painel que não existe. É o que
  // permite envelopar o card de conquista sem risco: piloto histórico
  // pré-gerado tem o número e não tem o corrida-a-corrida por trás dele.
  if (!lista.length) return children;

  // Abre para baixo por padrão e vira para cima quando não cabe — o dossiê tem
  // nove cards e as linhas de baixo estão sempre perto da borda da janela.
  let estilo = null;
  if (rect) {
    const cabeAbaixo = rect.bottom + altura + MARGEM <= window.innerHeight;
    const topo = cabeAbaixo ? rect.bottom + 6 : Math.max(MARGEM, rect.top - altura - 6);
    const esquerda = Math.min(
      Math.max(MARGEM, rect.right - LARGURA),
      window.innerWidth - LARGURA - MARGEM,
    );
    estilo = {
      position: "fixed",
      top: topo,
      left: esquerda,
      width: LARGURA,
      zIndex: 90,
    };
  }

  return (
    <div
      ref={alvoRef}
      onMouseEnter={entrar}
      onMouseMove={mover}
      onMouseLeave={sair}
      onMouseDown={alternarPreso}
      onFocus={entrar}
      onBlur={sair}
      tabIndex={0}
      className={`cursor-help rounded outline-none focus-visible:ring-1 focus-visible:ring-[color:var(--team)] ${className}`}
      data-has-detail="true"
    >
      {children}
      {rect
        ? createPortal(
            <div
              ref={painelRef}
              style={estilo}
              data-testid="dossier-detail-tooltip"
              data-preso={preso ? "true" : "false"}
              onMouseEnter={entrar}
              onMouseLeave={sair}
              onMouseDown={alternarPreso}
            >
              <div
                className={`rounded-xl border bg-[#0b1522] shadow-[0_12px_32px_rgba(0,0,0,0.55)] ${
                  preso ? "border-[#58a6ff]/40" : "border-white/10"
                }`}
              >
                <ul ref={listaRef} className="max-h-[320px] overflow-y-auto px-3 py-2">
                  {linhas}
                </ul>
                {/* Legenda, e não texto: centralizada, em caixa alta e espaçada,
                    igual aos títulos dos cards do dossiê. Assim ela se lê como
                    rodapé do painel e não disputa com a primeira linha da lista.
                    Altura fixa nos DOIS estados de preso — o painel virado para
                    cima é posicionado pela altura e daria um pulo ao prender. */}
                {rolavel || preso ? (
                  <div className="border-t border-white/[0.06] px-3 py-1.5 text-center text-[9px] uppercase leading-none tracking-[0.06em] text-text-muted">
                    {t(preso ? "driverDetail.history.tooltipPinned" : "driverDetail.history.tooltipPinHint")}
                  </div>
                ) : null}
              </div>
              {preso ? (
                <button
                  type="button"
                  onClick={fechar}
                  aria-label={t("driverDetail.history.tooltipRelease")}
                  data-testid="dossier-detail-soltar"
                  className="absolute -right-2 -top-2 flex h-[18px] w-[18px] items-center justify-center rounded-full border border-[#58a6ff]/60 bg-[#0b1522] text-[#58a6ff] transition-colors hover:border-[#58a6ff] hover:bg-[#58a6ff] hover:text-[#0b1522]"
                >
                  <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true">
                    <path
                      d="M1 1 L9 9 M9 1 L1 9"
                      stroke="currentColor"
                      strokeWidth="1.8"
                      strokeLinecap="round"
                      fill="none"
                    />
                  </svg>
                </button>
              ) : carregando && rolavel ? (
                <div className="pointer-events-none absolute -right-2 -top-2">
                  <Bolinha key={ciclo} />
                </div>
              ) : null}
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}

export default DossierDetailTooltip;
