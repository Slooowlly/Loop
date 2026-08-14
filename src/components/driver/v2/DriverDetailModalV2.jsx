import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import {
  ArrowUpRight,
  Award,
  ChevronDown,
  ChevronsUp,
  ChevronUp,
  Flag,
  Globe,
  Sparkles,
  Star,
  TrendingUp,
  Trophy,
  X,
} from "lucide-react";

import FlagIcon from "../../ui/FlagIcon";
import Tooltip from "../../ui/Tooltip";
import TeamLogoMark, { getTeamLogoSrc, HALO_FILTER } from "../../team/TeamLogoMark";
import useCareerStore from "../../../stores/useCareerStore";
import i18n from "../../../i18n/index.js";
import { ordinal } from "../../../i18n/format.js";
import { pisoDeAbertura } from "../../ui/aberturaDePainel.js";
import { getCategoryColor } from "../../../utils/categoryColors";
import { getVividTeamColor } from "../../../utils/teamColors";
import { comprimeSequenciasDeAnos } from "../../../utils/sequenciaDeAnos";
import { formatSalaryAnnual } from "../../../utils/formatters";
import { PlayerSkillSection } from "../detalhes/PlayerSkillSection.jsx";
import { technicalToneClass } from "../detalhes/primitivos.jsx";
import { DossierDetailTooltip } from "./DossierDetailTooltip.jsx";
import { CurvaDeCampeonato } from "./CurvaDeCampeonato.jsx";
import { DuelTimeline, MiniTimeline } from "./FaixaDeConfronto.jsx";
import { MarketSection } from "./MarketSection.jsx";
import { RecentFormStrip } from "./RecentFormStrip.jsx";
import {
  Block,
  BlockLabel,
  DataRow,
  HeroBadge,
  MetricIcon,
  MotivationBar,
} from "./primitivosDaFicha.jsx";
import {
  formatAttributeName,
  formatAverage,
  formatAverageGrid,
  formatDuel,
  formatCareerYears,
  formatContractPeriod,
  formatContractRole,
  formatInjuryOccurrence,
  formatInjuryRecovery,
  formatRaceMilestone,
  formatRetirementRate,
  formatSeasonMilestone,
  formatSeasonWithResult,
  formatSpecialCampaign,
  formatSpecialEventEntry,
  formatStreakRaces,
  formatUnemploymentYears,
  formatWorldAverage,
  formatYearsAverage,
} from "../detalhes/formatadores.js";
import {
  DUEL_LOSS_COLOR,
  DUEL_WIN_COLOR,
  MEDAL_COLORS,
  TONE_HEX,
  corDoSaldo,
  formataSaldo,
  groupTitlesByTeam,
  listaDeAnos,
  naRegua,
  ordenarPorNivel,
  primeiroNome,
  sequenciaAtual,
} from "./driverDetailV2Logic";

// Ficha do piloto v2.
//
// Mesmos dados do v1 (get_driver_detail, mesmo payload) — o que muda é a
// composição. O v1 era um drawer colado na borda direita: metade da tela de
// largura e, mesmo assim, tudo empilhado numa coluna só, com o cabeçalho
// espremido em 300px e o resto rolando para fora da vista.
//
// Aqui a ficha abre CENTRALIZADA e larga o bastante para o cabeçalho carregar,
// de uma vez, as três coisas que definem um piloto e que no v1 estavam separadas
// ou escondidas: quem ele é (nome, equipe, papel, licença), como ele pensa
// (personalidade) e como ele está (momento, motivação, campeonato).
//
// O que este arquivo deliberadamente NÃO copia do dossiê de equipe v2, apesar de
// dividir a paleta e a moldura com ele:
//
//   • números de carreira no cabeçalho, em caixa ou em faixa. Uma equipe tem 37
//     pódios de história e isso a define; um estreante de 16 anos tem quatro
//     zeros. Corridas, vitórias e pódios moram no Histórico, onde há um rank
//     para dar escala a eles. O que sobe ao topo é o TÍTULO, que não é um
//     acumulado a mais: é o fato que muda como se lê o piloto inteiro.
//   • coluna lateral de seções. A equipe tem cinco seções cheias; o piloto tem
//     quatro, e o rail cobrava 184px de largura para desenhar quatro palavras.
//     As abas voltaram a ser pílulas horizontais, que é o que o v1 já acertava.
//
// O Histórico abre primeiro, e é também a primeira pílula: a ficha é aberta para
// julgar um piloto, e quem ele é ao longo da carreira pesa mais que como ele foi
// nas últimas cinco corridas. Deixar a aba padrão fora da primeira posição faria
// a ficha abrir com a segunda pílula acesa, que se lê como estado errado.
const DRIVER_SECTIONS = ["historico", "temporada", "perfil", "rivais", "mercado"];
const DEFAULT_SECTION = DRIVER_SECTIONS[0];

// Piloto aposentado não tem temporada corrente, contrato, rival ativo nem forma
// recente: sobra a história. Mostrar as outras abas vazias seria prometer
// conteúdo que o payload não tem.
const RETIRED_SECTIONS = ["historico"];

// Tom do momento atual. As chaves (`forte`, `estavel`, `em_baixa`) vêm do
// backend e viram sufixo da chave `driverDetail.momentBuilder.<chave>` — mudar
// uma delas aqui exige mudar a chave de i18n do mesmo nome.
const MOMENT_TONES = {
  forte: { key: "forte", color: "#3fb950" },
  estavel: { key: "estavel", color: "#d29922" },
  em_baixa: { key: "em_baixa", color: "#f85149" },
  sem_dados: { key: "sem_dados", color: "#8b949e" },
};

// Quantas EQUIPES o card de campeão desenha antes de resumir o resto. Quatro
// linhas é o que cabe sem o card dobrar de altura em relação aos vizinhos —
// acima disso a linha de destaques deixa de ser uma linha.
const MAX_TITLE_TEAMS = 4;

export function DriverDetailModalV2({
  driverId,
  driverIds = [],
  onSelectDriver = null,
  onFavoriteChange = null,
  onOpenTeam = null,
  onOpenRanking = null,
  onClose,
}) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const [detail, setDetail] = useState(null);
  const [status, setStatus] = useState("loading");
  const [error, setError] = useState("");
  const [activeSection, setActiveSection] = useState(DEFAULT_SECTION);
  const [injuryAcknowledged, setInjuryAcknowledged] = useState(false);
  const [favoritePending, setFavoritePending] = useState(false);
  const [worldRank, setWorldRank] = useState(null);
  // Sentido do último passo entre pilotos, só para escolher de que lado a ficha
  // nova entra. Mora aqui em cima porque quem clica é a seta e quem anima é o
  // conteúdo, dois pontos distantes da árvore.
  const [stepDirection, setStepDirection] = useState("down");
  const contentRef = useRef(null);
  const primeiraCargaRef = useRef(true);
  // Se já existe uma ficha desenhada. Separado de `primeiraCargaRef` porque as
  // duas perguntas são diferentes: aquela decide o compasso de abertura e é
  // gasta na primeira ENTREGA; esta decide se a tela pode ficar vazia, e uma
  // abertura que falhou continua podendo.
  const temFichaRef = useRef(false);

  useEffect(() => {
    let active = true;

    if (!driverId || !careerId) {
      setStatus("error");
      setError(i18n.t("driverDetail.profile.loadError"));
      return undefined;
    }

    setError("");
    // Esvaziar a ficha só na ABERTURA. Trocar de piloto com o painel na tela é
    // navegação: a ficha anterior fica inteira no lugar até a próxima chegar, e
    // a troca vira um corte seco em vez de um piscar para o painel de carga e de
    // volta. A animação de passo continua no ponto certo porque ela é disparada
    // pelo `key` do piloto lá embaixo — que só muda quando o payload novo entra.
    //
    // Vale a pena porque o `invoke` deixou de custar meio segundo (os recordes
    // saíram de dentro dele): a ficha antiga fica visível por poucos quadros.
    if (!temFichaRef.current) {
      setStatus("loading");
      setDetail(null);
    }
    const piso = pisoDeAbertura(primeiraCargaRef.current);

    Promise.all([invoke("get_driver_detail", { careerId, driverId }), piso])
      .then(([payload]) => {
        if (!active) return;
        // A abertura só é gasta por quem CHEGA a entregar. Baixar a bandeira
        // aqui em vez de junto do `pisoDeAbertura` é o que faz o compasso
        // sobreviver ao StrictMode: em dev o React monta, desmonta e remonta o
        // efeito, e a primeira passagem — descartada, com `active` já falso —
        // consumia a bandeira. A segunda, a que de fato desenha, pedia o piso
        // como se fosse navegação e não esperava nada.
        primeiraCargaRef.current = false;
        temFichaRef.current = true;
        setDetail(payload);
        setStatus("ready");
      })
      .catch((invokeError) => {
        if (!active) return;
        primeiraCargaRef.current = false;
        setError(
          typeof invokeError === "string"
            ? invokeError
            : invokeError?.toString?.() ?? i18n.t("driverDetail.profile.loadError"),
        );
        setStatus("error");
      });

    return () => {
      active = false;
    };
  }, [careerId, driverId]);

  // Ranking mundial em uma busca à parte, e não dentro do payload da ficha: a
  // posição no mundo só existe em relação aos outros 200+ pilotos, então ela
  // custa o ranking inteiro. Junto, ela atrasaria a abertura da ficha e cada
  // passo entre pilotos; separada, a ficha abre na hora e a marca aparece
  // quando chega. Falha aqui não vira erro de tela — a marca só não existe.
  //
  // A marca viaja CARIMBADA com o piloto a que pertence. Sem isso ela seria a
  // única coisa fora de sincronia agora que a ficha anterior fica na tela
  // durante a troca: limpar na hora apagaria a marca do piloto que ainda está
  // desenhado, e não limpar mostraria a posição dele no cabeçalho do próximo —
  // este ranking custa mais que a ficha e chega sempre depois.
  useEffect(() => {
    let active = true;
    if (!driverId || !careerId) return undefined;

    invoke("get_driver_world_rank", { careerId, driverId })
      .then((payload) => {
        if (active) setWorldRank(payload ? { id: driverId, dados: payload } : null);
      })
      .catch(() => {});

    return () => {
      active = false;
    };
  }, [careerId, driverId]);

  useEffect(() => {
    function handleKeyDown(event) {
      if (event.key === "Escape") onClose?.();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  // A lesão volta a pedir confirmação a cada piloto: é um aviso sobre AQUELE
  // piloto, e carregar o "já vi" de um para o outro esconderia a lesão do
  // segundo. O scroll volta ao topo pelo mesmo motivo.
  useEffect(() => {
    setInjuryAcknowledged(false);
    if (contentRef.current) contentRef.current.scrollTop = 0;
  }, [driverId]);

  // Abrir a equipe FECHA a ficha. As duas telas ocupam o mesmo espaço, e deixar
  // o modal por cima do Atlas deixaria o jogador olhando para a ficha que ele
  // acabou de pedir para trocar.
  const abrirEquipe = onOpenTeam
    ? (equipe) => {
        onClose?.();
        onOpenTeam(equipe);
      }
    : null;

  // Um card de número de carreira leva ao ranking mundial ORDENADO por aquele
  // número. O card já diz "205º de 610" em vitórias; o clique é a pergunta
  // seguinte — quem são os 204 na frente. Fecha a ficha pelo mesmo motivo que
  // abrir a equipe fecha: o destino ocupa a tela inteira atrás do modal.
  const abrirRanking = onOpenRanking
    ? (metric, category = null) => {
        onClose?.();
        onOpenRanking({ metric, driverId, category });
      }
    : null;

  async function toggleFavorite() {
    if (!careerId || !driverId || favoritePending) return;
    setFavoritePending(true);
    try {
      const nowFavorite = await invoke("toggle_driver_favorite", { careerId, driverId });
      setDetail((current) => (current ? { ...current, is_favorito: nowFavorite } : current));
      onFavoriteChange?.(driverId, nowFavorite);
    } catch {
      // Silencioso — favoritar nunca pode quebrar a ficha.
    } finally {
      setFavoritePending(false);
    }
  }

  const perfil = detail?.perfil;
  const isRetired = detail?.status === "aposentado" || perfil?.status === "aposentado";
  const sections = useMemo(() => {
    const base = DRIVER_SECTIONS.filter((id) => !isRetired || RETIRED_SECTIONS.includes(id));
    // A aba de habilidade é do JOGADOR: são atributos inferidos do desempenho
    // real na pista, e não existem para um piloto de IA.
    return detail?.is_jogador ? [...base, "habilidade"] : base;
  }, [detail?.is_jogador, isRetired]);
  const effectiveSection = sections.includes(activeSection) ? activeSection : sections[0];

  const currentIndex = driverIds.indexOf(driverId);
  const previousDriverId = currentIndex > 0 ? driverIds[currentIndex - 1] : null;
  const nextDriverId =
    currentIndex >= 0 && currentIndex < driverIds.length - 1 ? driverIds[currentIndex + 1] : null;

  const activeInjury = detail?.saude?.lesao_ativa ?? null;
  const showInjury = Boolean(activeInjury && !injuryAcknowledged);
  const teamColor = getVividTeamColor(
    detail?.equipe_cor_primaria || perfil?.equipe_cor_primaria || "",
  );

  const layer = (
    <div
      className="fixed inset-0 z-[90] flex items-center justify-center px-[110px] max-lg:px-[76px]"
      data-testid="driver-detail-layer"
    >
      <button
        type="button"
        aria-label={t("driverDetail.profile.closeSheet")}
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/70 backdrop-blur-[3px]"
      />

      {/* O wrapper existe só para dar às setas uma âncora que NÃO seja o
          conteúdo da ficha: ele tem a largura do painel, então a coluna de setas
          fica sempre na mesma calha à direita, no meio da altura, independente
          do que a seção de dentro esteja desenhando. */}
      {/* 1280, e não 1100: o dossiê de histórico são nove cards em três colunas
          e, na largura antiga, a última fileira ficava sempre abaixo da dobra —
          o jogador tinha que rolar para ver "Confiabilidade" e "Lesões". */}
      <div className="relative z-10 flex w-[min(100%,1280px)] justify-center">
        {onSelectDriver ? (
          <div
            data-testid="driver-detail-step-rail"
            className="animate-team-rail-out absolute left-full top-1/2 ml-3 flex -translate-y-1/2 flex-col gap-2"
          >
            <DriverStepButton
              label={t("driverDetail.navigator.previous")}
              direction="up"
              driverId={previousDriverId}
              onSelectDriver={onSelectDriver}
              onStep={setStepDirection}
            />
            <DriverStepButton
              label={t("driverDetail.navigator.next")}
              direction="down"
              driverId={nextDriverId}
              onSelectDriver={onSelectDriver}
              onStep={setStepDirection}
            />
          </div>
        ) : null}

        <aside
          role="dialog"
          aria-modal="true"
          aria-labelledby="driver-detail-title"
          data-testid="driver-detail-drawer"
          // Teto em pixels, e não só em `vh`: sem ele a ficha crescia até quase a
          // tela inteira na seção mais longa e voltava a uns 600px nas curtas —
          // trocar de aba mexia a moldura e o painel parecia outra tela a cada
          // clique. O `vh` fica como piso de segurança para janela baixa.
          //
          // Não há altura MÍNIMA de propósito: a ficha de um estreante tem pouco
          // a dizer, e esticá-la até um teto fixo só produziria o vazio que ela
          // não tem por que ter. Já foi tentado fixar a altura para estabilizar
          // a pintura na troca de aba — o vazio embaixo das abas curtas é feio
          // demais, e não era ele o culpado da lentidão.
          //
          // O teto subiu de 900px para 968: a aba Perfil é a mais longa das
          // cinco e sobrava dela só a faixa de fases do arco, cortada rente ao
          // rodapé — meia dúzia de pixels a mais de moldura poupa um scroll na
          // aba que mais se abre.
          className="animate-scale-in relative flex max-h-[min(94vh,968px)] w-full flex-col overflow-hidden rounded-[28px] border border-white/15 bg-[#07101d] shadow-[0_30px_90px_rgba(0,0,0,0.72)]"
          style={{
            // `--team` é a cor da equipe do piloto JÁ LEGÍVEL sobre o fundo
            // escuro, e não a cor crua. Toda a identidade visual da ficha sai
            // desta variável. Piloto sem equipe cai no cinza do próprio helper.
            "--team": teamColor,
            backgroundImage:
              "radial-gradient(circle at 8% 0%, color-mix(in srgb, var(--team) 14%, transparent), transparent 26rem), linear-gradient(180deg, rgba(12,22,38,0.98), rgba(5,11,20,0.995))",
          }}
        >
          <div className="h-1 shrink-0 bg-[color:var(--team)]" />

          {status === "loading" ? (
            <div
              className="flex min-h-[260px] flex-1 flex-col items-center justify-center gap-3"
              data-testid="driver-detail-loading"
            >
              <span className="animate-pulse text-4xl">🏎️</span>
              <p className="text-sm text-text-secondary">{t("driverDetail.profile.loading")}</p>
            </div>
          ) : null}

          {status === "error" ? (
            <div className="flex min-h-[260px] flex-1 flex-col items-center justify-center gap-4 px-8 text-center">
              <p className="text-sm text-status-red">{error}</p>
              <button
                type="button"
                onClick={onClose}
                className="rounded-xl border border-white/15 bg-[#0d1727] px-4 py-2 text-xs font-semibold text-text-secondary transition-glass hover:bg-[#14233a] hover:text-text-primary"
              >
                {t("driverDetail.profile.closeButton")}
              </button>
            </div>
          ) : null}

          {status === "ready" && detail ? (
            // `key` no piloto é o gatilho da animação: ao trocar de ficha o React
            // monta um bloco novo e a CSS toca do zero. A moldura fica de fora do
            // wrapper de propósito — ela não pisca, só o conteúdo desliza.
            <div
              key={driverId}
              data-step-direction={stepDirection}
              className={`flex min-h-0 flex-1 flex-col ${
                stepDirection === "up" ? "animate-team-step-up" : "animate-team-step-down"
              } ${showInjury ? "pointer-events-none select-none blur-[5px]" : ""}`}
            >
              <DriverHero
                detail={detail}
                worldRank={worldRank?.id === detail.id ? worldRank.dados : null}
                favoritePending={favoritePending}
                onToggleFavorite={toggleFavorite}
                onClose={onClose}
              />

              <nav
                role="tablist"
                aria-label={t("driverDetail.v2.tablistAria")}
                // `safe center`, e não `center` puro: quando as pílulas não
                // cabem (janela estreita, jogador com a aba de habilidade), o
                // centramento comum empurra a primeira para fora da área
                // rolável e ela fica inalcançável. Com `safe` o excesso volta a
                // alinhar à esquerda e a rolagem alcança tudo.
                className="flex shrink-0 gap-1.5 overflow-x-auto border-b border-white/10 px-6 pb-3 pt-3 [justify-content:safe_center]"
              >
                {sections.map((id) => (
                  <button
                    key={id}
                    type="button"
                    role="tab"
                    aria-selected={effectiveSection === id}
                    onClick={() => setActiveSection(id)}
                    data-testid={`driver-detail-tab-${id}`}
                    // `transition-colors 150ms`, e não a `transition-glass` da
                    // casa: aquela é `transition: all 300ms`, então a pílula
                    // levava 300ms só para assentar a cor do clique — e `all`
                    // ainda deixa aberta a porta para animar propriedade de
                    // layout sem querer.
                    className={`shrink-0 rounded-lg px-4 py-2 text-[13px] font-semibold transition-colors duration-150 ${
                      effectiveSection === id
                        ? "bg-[color-mix(in_srgb,var(--team)_26%,transparent)] text-text-primary"
                        : "text-text-secondary hover:bg-white/[0.05] hover:text-text-primary"
                    }`}
                  >
                    {t(`driverDetail.v2.sections.${id}`)}
                  </button>
                ))}
              </nav>

              {/* `contain: layout paint` diz ao navegador o que a troca de aba
                  NÃO afeta: nada aqui dentro pode mexer no tamanho da moldura
                  (ela é fixa) nem pintar fora desta caixa (ela já corta). Com
                  isso o repintar fica confinado ao miolo, em vez de subir até o
                  painel inteiro. Os painéis do Clausewitz são portais no
                  `body`, então não são cortados por isto. */}
              <div ref={contentRef} className="min-h-0 flex-1 overflow-y-auto px-6 py-5 [contain:layout_paint]">
                {effectiveSection === "temporada" ? <SeasonSection detail={detail} /> : null}
                {effectiveSection === "historico" ? (
                  <HistorySection
                    detail={detail}
                    onAbrirEquipe={abrirEquipe}
                    onAbrirRanking={abrirRanking}
                    careerId={careerId}
                  />
                ) : null}
                {effectiveSection === "perfil" ? <ProfileSection detail={detail} /> : null}
                {effectiveSection === "rivais" ? (
                  <RivalsSection detail={detail} onSelectDriver={onSelectDriver} />
                ) : null}
                {effectiveSection === "mercado" ? <MarketSection detail={detail} /> : null}
                {effectiveSection === "habilidade" ? (
                  <PlayerSkillSection SectionComponent={Block} careerId={careerId} />
                ) : null}
              </div>
            </div>
          ) : null}

          {showInjury ? (
            <InjuryOverlay injury={activeInjury} onConfirm={() => setInjuryAcknowledged(true)} />
          ) : null}
        </aside>
      </div>
    </div>
  );

  if (typeof document === "undefined") return null;
  return createPortal(layer, document.body);
}

// Cabeçalho: as camadas da identidade do piloto lado a lado.
//
// No v1 elas estavam separadas — o nome e os chips no topo, a personalidade
// numa coluna de 300px que só cabia em tela grande, a motivação numa barra à
// direita e o momento atual escondido dentro da aba Resumo. A largura da ficha
// centralizada existe justamente para isso: quem é, o que já fez, como pensa,
// como está.
//
// A hierarquia é deliberada e vai do permanente ao volátil: o título (que ele
// leva para sempre) na linha do nome, a personalidade (que muda devagar) e o
// estado atual (que muda toda corrida) na base.
function DriverHero({ detail, worldRank, favoritePending, onToggleFavorite, onClose }) {
  const { t } = useTranslation();
  const perfil = detail.perfil ?? {};
  const competitivo = detail.competitivo ?? {};
  const stardom = detail.estrelato;
  const personalities = [
    competitivo.personalidade_primaria,
    competitivo.personalidade_secundaria,
  ].filter(Boolean);
  const role = formatContractRole(detail.papel);
  const teamLabel = perfil.equipe_nome || detail.equipe_nome || "";
  // CAMPEAO sai da fileira de chips cinzas: vira a faixa dourada ao lado do
  // nome. Ser campeão é o fato mais alto da ficha e estava saindo com o mesmo
  // peso visual da licença — escrever isso duas vezes na mesma linha vale menos
  // que escrever uma vez com o destaque certo.
  const badges = (perfil.badges ?? []).filter(
    (badge) => badge.label !== "ROOKIE" && badge.label !== "CAMPEAO",
  );
  const titles = detail.trajetoria?.titulos ?? 0;
  const moment = MOMENT_TONES[detail.forma?.momento] ?? MOMENT_TONES.sem_dados;
  // Fama só aparece de "Conhecido" (>30) para cima. Carimbar "Anônimo" na testa
  // do estreante é gastar um chip para dizer que não há o que dizer.
  const fameLevel = (stardom?.fama ?? 0) > 30 ? stardom?.nivel_fama : null;

  return (
    <header
      data-testid="driver-detail-hero"
      className="relative shrink-0 overflow-hidden border-b border-white/10 px-6 pb-5 pt-5"
    >
      {/* Lavagem diagonal na cor da equipe. O gradiente da moldura nasce no
          canto e já morreu quando chega ao nome; aqui a cor entra por trás da
          identidade, que é onde ela quer dizer alguma coisa. */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0"
        style={{
          backgroundImage:
            "linear-gradient(104deg, color-mix(in srgb, var(--team) 15%, transparent), transparent 58%)",
        }}
      />

      {/* GRADE de três colunas, não flex: com `flex-1` dos dois lados a faixa
          da equipe caía onde sobrava espaço, e como a coluna direita é bem mais
          larga que a do nome ela nascia deslocada para a direita. Com
          `1fr auto 1fr` a coluna do meio fica no eixo do cabeçalho por
          construção, independente do que as laterais tenham dentro. O `minmax(0,…)`
          é o que permite as laterais encolherem em vez de empurrar o centro. */}
      <div className="relative grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-start gap-4">
        <div className="flex min-w-0 items-start gap-4">
          {/* PLACA 3:2 (42x28) PREENCHIDA pela arte. Só a altura fixa não
              resolvia: as proporções oficiais vão de 1:1 (Suíça) a 2:1 (Canadá,
              Reino Unido), então a mesma altura devolvia larguras de 28 a 56px e
              as fichas pareciam ter bandeiras de tamanhos diferentes. Conter a
              arte dentro da placa uniformizava a caixa, mas deixava uma tarja
              transparente em volta de quem não é 3:2 — a moldura ficava com
              folga de um lado só, e isso lê como defeito.
              Daí `cover`: 3:2 é a proporção de 11 das 25 artes (e a mediana das
              outras), então a maioria preenche sem perder nada; o corte pesa nas
              quatro 2:1 (12,5% de cada lateral, com as diagonais do Union Jack
              ainda inteiras) e na suíça (16,7% de topo e base, com a cruz — que
              para bem antes da borda — intacta). Na TABELA continua `contain`:
              a 16px o corte some, mas a tarja também, e lá as bandeiras estão
              empilhadas numa coluna onde a caixa é que alinha.
              Os 28px batem com a linha do nome (24px de nome + ~2px de baseline
              da idade), então a placa nasce centrada sem margem manual. */}
          <span className="flex h-7 w-[42px] shrink-0 items-center justify-center overflow-hidden rounded-md bg-white/[0.06] shadow-[0_0_0_1px_rgba(255,255,255,0.10)]">
            <FlagIcon
              variant="natural"
              nacionalidade={
                perfil.bandeira && perfil.nacionalidade
                  ? `${perfil.bandeira} ${perfil.nacionalidade}`
                  : perfil.nacionalidade || detail.nacionalidade || ""
              }
              className="h-full w-full object-cover text-base"
            />
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
              <h2
                id="driver-detail-title"
                className="min-w-0 truncate text-2xl font-semibold leading-none tracking-[-0.03em] text-text-primary"
              >
                {detail.nome}
              </h2>
              {/* A idade saía em 14px cinza-secundário ao lado de um nome de
                  24px: legível se você fosse procurar, invisível se não fosse. É
                  um dos três números que decidem se vale contratar o piloto. */}
              <span className="shrink-0 font-mono text-base font-semibold text-text-secondary">
                {t("driverDetail.profile.age", { count: perfil.idade ?? detail.idade })}
              </span>
              {titles > 0 ? (
                <span
                  data-testid="driver-detail-titles"
                  className="flex shrink-0 items-center gap-1.5 self-center rounded-full border px-2.5 py-1 text-xs font-semibold"
                  style={{
                    borderColor: `color-mix(in srgb, ${MEDAL_COLORS.first} 45%, transparent)`,
                    backgroundColor: `color-mix(in srgb, ${MEDAL_COLORS.first} 14%, transparent)`,
                    color: MEDAL_COLORS.first,
                  }}
                >
                  <Award size={13} strokeWidth={2} aria-hidden="true" />
                  {t("driverDetail.v2.titleBadge", { count: titles })}
                </span>
              ) : null}
            </div>
            {/* A fileira de chips ficou só com o que qualifica o PILOTO — licença,
                fama, alertas. A equipe saiu daqui para a faixa do meio: espremida
                numa pílula de 12px ela competia com a licença, e é a primeira
                coisa que o jogador procura quando abre a ficha de um rival. */}
            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              {perfil.licenca?.nivel ? <HeroBadge>{perfil.licenca.nivel}</HeroBadge> : null}
              {fameLevel ? (
                <Tooltip texto={t("driverDetail.stardom.fame")}>
                  <span
                    data-testid="driver-detail-fame"
                    className="flex items-center gap-1.5 rounded-full border border-white/15 bg-[#08111f] px-2.5 py-1 text-xs"
                  >
                    <Sparkles
                      size={12}
                      strokeWidth={2}
                      aria-hidden="true"
                      className={technicalToneClass[stardom?.tom_fama] ?? technicalToneClass.neutral}
                    />
                    <span className="text-text-secondary">{fameLevel}</span>
                  </span>
                </Tooltip>
              ) : null}
              {badges.map((badge) => (
                <HeroBadge key={`${badge.label}-${badge.variant}`}>{badge.label}</HeroBadge>
              ))}
            </div>
          </div>
        </div>

        {/* A EQUIPE, no meio e no tamanho que ela tem no jogo. A lavagem
            diagonal da moldura já é da cor dela; aqui o nome nasce em cima do
            ponto mais forte do gradiente em vez de ficar num chip de 12px
            perdido entre licença e fama. */}
        <div
          data-testid="driver-detail-team-banner"
          className="flex min-w-0 items-center justify-center gap-3 self-center px-2"
        >
          {teamLabel ? (
            <>
              {/* `md` (36px), não `sm`: o bloco ao lado tem ~42px de altura
                  (26px de nome + a linha do papel), e a placa de 28px lia como
                  um ícone perdido em vez da marca da equipe. */}
              <TeamLogoMark teamName={teamLabel} size="md" halo testId="driver-detail-team-logo" />
              <div className="min-w-0 text-center">
                <div className="truncate text-[26px] font-semibold leading-none tracking-[-0.02em] text-[color:var(--team)]">
                  {teamLabel}
                </div>
                {role !== "-" ? (
                  <div className="mt-1.5 truncate text-[10px] font-bold uppercase tracking-[0.2em] text-text-secondary">
                    {role}
                  </div>
                ) : null}
              </div>
            </>
          ) : (
            <HeroBadge>{t("driverDetail.profile.noTeam")}</HeroBadge>
          )}
        </div>

        {/* Faixa direita, tudo em UMA linha: como o piloto está (momento e
            motivação), onde ele está no MUNDO (ranking) e o que dá para fazer
            com ele (favoritar, fechar).

            Empilhado, o estado descia por baixo dos botões e puxava o
            cabeçalho para baixo — e como a coluna da esquerda acaba nos chips,
            sobrava uma faixa morta de largura inteira ao lado dele. Deitado, o
            bloco cabe na altura que os botões já ocupavam e o buraco some.

            A posição no campeonato não está aqui: ela é da temporada corrente
            e já abre em número grande na aba Temporada atual. */}
        <div className="flex shrink-0 items-center justify-self-end gap-3">
          <div className="flex items-center gap-3" data-testid="driver-detail-state">
            <strong
              className="flex items-center gap-1.5 whitespace-nowrap text-sm font-semibold leading-none"
              style={{ color: moment.color }}
            >
              <span
                className="h-1.5 w-1.5 shrink-0 rounded-full"
                style={{ backgroundColor: moment.color }}
              />
              {t(`driverDetail.momentBuilder.${moment.key}`)}
            </strong>
            <MotivationBar value={competitivo.motivacao} />
          </div>
          <div className="flex items-center gap-2">
            <WorldRankMark rank={worldRank} />
            {detail.is_jogador ? null : (
              <button
                type="button"
                onClick={onToggleFavorite}
                disabled={favoritePending}
                aria-pressed={Boolean(detail.is_favorito)}
                aria-label={
                  detail.is_favorito
                    ? t("driverDetail.favorite.remove")
                    : t("driverDetail.favorite.add")
                }
                data-testid="driver-detail-favorite"
                className={`grid h-8 w-8 place-items-center rounded-lg border transition-glass ${
                  detail.is_favorito
                    ? "border-status-yellow/50 bg-status-yellow/15 text-status-yellow"
                    : "border-white/15 bg-[#0d1727] text-text-secondary hover:text-status-yellow"
                } ${favoritePending ? "cursor-not-allowed opacity-60" : ""}`}
              >
                <Star
                  size={16}
                  strokeWidth={1.8}
                  aria-hidden="true"
                  fill={detail.is_favorito ? "currentColor" : "none"}
                />
              </button>
            )}
            <button
              type="button"
              onClick={onClose}
              aria-label={t("driverDetail.profile.closeModal")}
              className="grid h-8 w-8 place-items-center rounded-lg border border-white/15 bg-[#0d1727] text-text-secondary transition-glass hover:bg-[#14233a] hover:text-text-primary"
            >
              <X size={18} strokeWidth={1.8} aria-hidden="true" />
            </button>
          </div>
        </div>
      </div>

      {/* A personalidade ocupa a largura inteira. Ela dividia a linha com o card
          de estado e sobrava espaço morto dentro de cada caixa: duas linhas de
          texto de 12px ao lado de um emoji de 16px. Com o dobro de largura o
          emoji pode ser o que ele é — a ilustração do traço, não uma bolinha ao
          lado do título.

          Já tentamos encolher isto para chips na fileira dos badges, com a
          descrição no `title`. Ganhava ~85px em todas as abas e não vale: o
          traço é metade do retrato do piloto, e a descrição é o que diferencia
          "Consolidador" de "Ambicioso" para quem ainda não decorou os nomes.
          Hover não é leitura. */}
      <div className="relative mt-4 grid gap-3 sm:grid-cols-2">
        {personalities.length ? (
          personalities.map((personality, index) => (
            <div
              key={`${personality.tipo}-${index}`}
              data-personality={index === 0 ? "primary" : "secondary"}
              className="flex items-center gap-4 rounded-xl bg-[#0f1c2b] px-4 py-3"
            >
              <span className="shrink-0 text-[34px] leading-none">{personality.emoji}</span>
              <div className="min-w-0">
                <strong className="block truncate text-sm font-semibold text-text-primary">
                  {personality.tipo}
                </strong>
                <p className="mt-1 line-clamp-2 text-xs leading-5 text-text-secondary">
                  {personality.descricao}
                </p>
              </div>
            </div>
          ))
        ) : (
          <div className="rounded-xl bg-[#0f1c2b] px-4 py-3 text-xs text-text-secondary">
            {t("driverDetail.personality.empty")}
          </div>
        )}
      </div>
    </header>
  );
}

// Marca do ranking mundial, ao lado da estrela.
//
// Chega depois do resto (comando separado, ver `get_driver_world_rank`), então
// ela simplesmente não existe até o número voltar — e continua não existindo
// para quem está fora do ranking. Um esqueleto piscando aqui chamaria atenção
// para o canto errado do cabeçalho.
function WorldRankMark({ rank }) {
  const { t } = useTranslation();
  if (!rank || !rank.posicao) return null;
  const delta = rank.delta ?? 0;

  return (
    <Tooltip texto={t("driverDetail.v2.worldRankTitle", { total: rank.total })}>
      <span
        data-testid="driver-detail-world-rank"
        className="flex items-center gap-2 py-1 pl-2.5 pr-3"
      >
        <Globe size={13} strokeWidth={1.8} aria-hidden="true" className="text-text-muted" />
        <span className="leading-none">
          <span className="flex items-baseline gap-1">
            <strong className="font-mono text-sm font-semibold text-text-primary">
              {ordinal(rank.posicao)}
            </strong>
            {delta ? (
              <span
                className={`font-mono text-[10px] ${delta > 0 ? "text-status-green" : "text-status-red"}`}
              >
                {delta > 0 ? `▲${delta}` : `▼${Math.abs(delta)}`}
              </span>
            ) : null}
          </span>
          <span className="mt-1 block font-mono text-[10px] leading-none text-text-muted">
            {t("driverDetail.v2.worldRankIndex", { value: formatWorldIndex(rank.indice) })}
          </span>
        </span>
      </span>
    </Tooltip>
  );
}

// O índice passa dos milhares num piloto de carreira longa e não tem por que
// carregar a fração no cabeçalho — 4.210,4 e 4.210 dizem a mesma coisa aqui.
function formatWorldIndex(value) {
  if (!Number.isFinite(value)) return "0";
  return Math.round(value).toLocaleString(i18n.language || "pt-BR");
}

// ─────────────────────────────── Resumo ───────────────────────────────

function SeasonSection({ detail }) {
  const rookie = (detail.stats_carreira?.corridas ?? 0) === 0;
  return rookie ? <RookieSummary detail={detail} /> : <RacedSummary detail={detail} />;
}

// Resumo do estreante.
//
// O v1 respondia a um piloto sem passado com um painel vazio de 200px dizendo
// "sem passado competitivo para comparar" — verdadeiro e inútil: o jogador abriu
// a ficha para decidir se contrata o garoto, e a tela não dava nada com que
// decidir. Um estreante TEM tudo isto: traços, leitura técnica, licença e
// contrato. É só que nada disso é histórico — e hoje o grosso mora na aba
// "Perfil", que é justamente a que responde por um piloto sem temporada.
function RookieSummary({ detail }) {
  const { t } = useTranslation();
  const contract = detail.contrato_mercado?.contrato;

  return (
    <section>
      <div
        className="flex flex-wrap items-baseline gap-x-2 gap-y-1 rounded-xl border border-accent-primary/20 bg-accent-primary/[0.08] px-3.5 py-2.5"
        data-testid="driver-detail-rookie-banner"
      >
        <strong className="text-base font-semibold text-accent-primary">
          {t("driverDetail.summary.rookie")}
        </strong>
        <span className="text-xs text-text-secondary">
          {t("driverDetail.summary.unknownExpectation")} ·{" "}
          {t("driverDetail.summary.formStartsAfter")}
        </span>
      </div>

      {contract ? (
        <div className="mt-5">
          <BlockLabel>{t("driverDetail.moment.contractStatus")}</BlockLabel>
          {/* Duas colunas: quatro linhas curtas empilhadas na largura toda da
              ficha viravam uma tabela alta e quase vazia, com o valor a meio
              metro do rótulo. */}
          {/* Em duas colunas quem fecha o bloco são as DUAS últimas linhas, e não
              só a última — sem isto a coluna da esquerda terminava com um risco
              solto embaixo. */}
          <div className="mt-2.5 grid gap-x-6 rounded-xl bg-[#0f1c2b] px-4 py-3.5 sm:grid-cols-2 sm:[&>*:nth-last-child(-n+2)]:border-b-0">
            <DataRow label={t("driverDetail.moment.team")} value={contract.equipe_nome} />
            <DataRow
              label={t("driverDetail.market.role")}
              value={formatContractRole(contract.papel)}
            />
            <DataRow
              label={t("driverDetail.moment.salary")}
              value={formatSalaryAnnual(contract.salario_anual)}
            />
            <DataRow label={t("driverDetail.moment.term")} value={formatContractPeriod(contract)} />
          </div>
        </div>
      ) : null}
    </section>
  );
}

// Quem o piloto É saiu daqui para a aba "Perfil": traços e leitura técnica saem
// dos ATRIBUTOS (`technical_level_for_value` no backend), não das corridas do
// ano. Encostados nos números da temporada eles se liam como veredito de forma
// ("Instável" depois de uma corrida), quando dizem a mesma coisa em janeiro e em
// dezembro. Esta aba responde "como vai a temporada"; a outra, "quem é ele".
function RacedSummary({ detail }) {
  const resumo = detail.resumo_atual ?? {};
  const forma = detail.forma ?? {};

  return (
    <section>
      <SeasonHeadline
        resumo={resumo}
        forma={forma}
        leitura={detail.leitura_desempenho ?? {}}
        raced={(detail.stats_temporada?.corridas ?? 0) > 0}
      />

      <TeammateCompare detail={detail} />

      <RecentFormStrip
        seasons={forma.temporadas}
        entries={forma.ultimas_10 ?? forma.ultimas_5 ?? []}
        context={forma.contexto}
      />

    </section>
  );
}

// A faixa que abre a temporada: uma caixa, e não cinco.
//
// O v2 abria com o card de veredito ao lado de quatro MiniMetrics, e a conta não
// fechava: "Média recente: 3.0", "Campeonato P3", a coluna rotulada P3 na faixa
// de forma e a "média P3.0" no canto dela diziam o MESMO número em quatro
// tipografias — enquanto "Vitórias 0" ganhava um card do tamanho da posição no
// campeonato. Aqui a posição aparece uma vez, grande, e o resto é a linha de
// contexto dela.
//
// O que entra no lugar dos zeros é a DISTÂNCIA: P3 a 8 pontos do líder e P3 a 80
// são temporadas diferentes, e a tela antiga desenhava as duas igual. A posição é
// o degrau; o gap é o campeonato.
//
// A metade direita era espaço morto — a faixa escrevia quatro linhas curtas e
// deixava dois terços da largura em branco, enquanto logo abaixo um card gastava
// quatro linhas de tabela para dizer uma comparação só. O delta contra o pacote
// mudou de casa: é a resposta para "P2 é bom?", e agora mora ao lado do P2.
function SeasonHeadline({ resumo, forma, leitura, raced }) {
  const { t } = useTranslation();
  const tone = SUMMARY_TONES[resumo.tom] ?? SUMMARY_TONES.info;
  // Posição no campeonato só vale para quem já largou nesta temporada. Antes da
  // primeira corrida o grid inteiro está com zero ponto e a ordem é desempate
  // alfabético — anunciar "P1" para quem não correu é a mentira mais convincente
  // que a tela poderia contar. O backend cala os gaps pelo mesmo motivo.
  const posicao = raced ? resumo.posicao_campeonato : null;

  // Cada linha ganha um ícone e uma âncora em negrito. A versão anterior juntava
  // tudo num `join(" · ")` de 12px: cinco fatos sem hierarquia numa fita cinza,
  // em que "26 pontos" e "líder do campeonato" pesavam igual e nenhum dos dois
  // era achável de relance. O ícone dá o assunto antes da leitura; o negrito
  // separa o dado do que o qualifica.
  const linhas = [];

  const situacao =
    posicao === 1
      ? t("driverDetail.summary.leadingChampionship")
      : Number.isFinite(resumo.gap_lider) && resumo.gap_lider > 0
        ? t("driverDetail.summary.gapToLeader", { count: resumo.gap_lider })
        : null;
  linhas.push({
    key: "pontos",
    icon: Trophy,
    forte: t("driverDetail.summary.pointsShort", { count: resumo.pontos ?? 0 }),
    resto: situacao,
  });

  if (posicao && Number.isFinite(resumo.gap_proximo)) {
    linhas.push({
      key: "gap",
      icon: ChevronsUp,
      forte: t("driverDetail.summary.gapToNext", {
        gap: resumo.gap_proximo,
        position: posicao + 1,
      }),
    });
  }

  // Zero vitória e zero pódio não são notícia: a ausência já se lê na faixa de
  // forma logo abaixo, e a linha inteira desaparece em vez de anunciar zeros.
  const conquistas = [];
  if ((resumo.vitorias ?? 0) > 0) {
    conquistas.push(t("driverDetail.summary.winsCount", { count: resumo.vitorias }));
  }
  if ((resumo.podios ?? 0) > 0) {
    conquistas.push(t("driverDetail.summary.podiumsCount", { count: resumo.podios }));
  }
  if (conquistas.length) {
    linhas.push({ key: "conquistas", icon: Flag, forte: conquistas.join(" · ") });
  }

  linhas.push({
    key: "media",
    icon: TrendingUp,
    resto: `${t("driverDetail.summary.recentAverage")}: ${formatAverage(resumo.media_recente)} · ${
      resumo.tendencia || forma.tendencia || "->"
    }`,
  });

  const delta = leitura?.delta_posicao;
  const temDelta = Number.isFinite(delta) && Boolean(leitura?.esperado_posicao);

  // A faixa não usa `justify-between`: com duas colunas só — piloto sem equipe,
  // sem expectativa contra a qual medir — ele atirava a linha de campeonato no
  // canto direito, a meio metro do P5 que ela explica. Quem empurra o delta para
  // a borda é a coluna do meio, que cresce (`flex-1`); sem delta ela só ocupa a
  // sobra, e o conteúdo continua encostado no P5.
  return (
    <div
      data-summary-tone={resumo.tom || "info"}
      data-testid="driver-detail-verdict"
      className="flex flex-wrap items-stretch gap-x-6 gap-y-4 rounded-xl border bg-[#0c1726] px-5 py-4"
      style={{
        // A faixa é da EQUIPE, não do humor. Antes ela se pintava com o tom do
        // veredito: um piloto "Regular" abria a temporada dentro de uma moldura
        // amarela, que na ficha inteira é a cor de alerta — e a moldura gritava
        // mais alto que o próprio veredito, que é quem tem o direito de julgar.
        // O julgamento desceu para a palavra (ver `tone.label` abaixo) e a
        // identidade subiu para a moldura.
        borderColor: "color-mix(in srgb, var(--team) 30%, transparent)",
        backgroundImage:
          "linear-gradient(100deg, color-mix(in srgb, var(--team) 14%, transparent), transparent 58%)",
      }}
    >
      {/* A posição é o número que o jogador veio buscar. Em 20px ela era só mais
          um dado numa fita de dados; no corpo grande ela vira o assunto da faixa,
          e o veredito passa a legendá-la em vez de disputar com ela. */}
      <div className="flex shrink-0 flex-col justify-center">
        <span className="block text-[11px] font-semibold uppercase tracking-[0.14em] text-[color:var(--team)]">
          {t("driverDetail.summary.now")}
        </span>
        {posicao ? (
          <strong
            data-testid="driver-detail-championship"
            className="mt-1.5 block text-[56px] font-semibold leading-[0.85] tracking-[-0.04em] text-text-primary"
          >
            P{posicao}
          </strong>
        ) : null}
        {/* O veredito herdou o tom que era da moldura: "Crítico" em vermelho e
            "Bom" em verde continuam se lendo de longe, mas agora sem sequestrar
            a faixa inteira para dizer isso. */}
        <strong
          className={`block font-semibold leading-tight ${tone.label} ${
            posicao ? "mt-2.5 text-xl" : "mt-1.5 text-3xl"
          }`}
        >
          {resumo.veredito || t("driverDetail.momentBuilder.sem_dados")}
        </strong>
      </div>

      <div
        data-testid="driver-detail-standings-line"
        className="grid min-w-0 flex-1 gap-y-2 self-center border-l border-white/10 pl-6"
      >
        {linhas.map(({ key, icon: Icone, forte, resto }) => (
          <div key={key} className="flex min-w-0 items-center gap-2.5 text-sm leading-5">
            <Icone size={15} strokeWidth={1.8} aria-hidden="true" className="shrink-0 text-text-muted" />
            {forte ? <strong className="font-semibold text-text-primary">{forte}</strong> : null}
            {forte && resto ? <span className="text-text-muted">·</span> : null}
            {resto ? <span className="min-w-0 truncate text-text-secondary">{resto}</span> : null}
          </div>
        ))}
      </div>

      {/* Sem `esperado_posicao` não há contra o quê comparar: um "+0" pendurado
          na direita da faixa afirmaria que o piloto entregou exatamente o
          previsto quando ninguém previu nada. */}
      {temDelta ? (
        <div
          data-testid="driver-detail-performance"
          className="flex min-w-0 max-w-[19rem] shrink-0 flex-col justify-center border-l border-white/10 pl-6"
        >
          <span className="block text-[11px] font-semibold uppercase tracking-[0.14em] text-text-secondary">
            {t("driverDetail.performance.vsExpected")}
          </span>
          {/* Mesmo corpo do P6: os dois números que respondem "como vai a
              temporada" — onde ele está e quanto isso vale contra o carro —
              agora emolduram a faixa em vez de um deles ser nota de rodapé. */}
          <strong
            className={`mt-1.5 block text-[56px] font-semibold leading-[0.85] tracking-[-0.04em] ${
              delta >= 0 ? "text-status-green" : "text-status-red"
            }`}
          >
            {delta > 0 ? `+${delta}` : `${delta}`}
          </strong>
          <div className="mt-2.5 text-sm leading-5 text-text-secondary">
            {t("driverDetail.performance.deliveredShort", { position: leitura.entregue_posicao })} ·{" "}
            {t("driverDetail.performance.expectedShort", { position: leitura.esperado_posicao })}
          </div>
          <p className="text-xs leading-5 text-text-muted">{leitura.leitura}</p>
        </div>
      ) : null}
    </div>
  );
}

// Margem que enche METADE da barra do duelo, ou seja, a barra inteira de um lado
// só. Vinte pontos é a distância a partir da qual o duelo interno deixou de ser
// duelo: quem abre isso no companheiro não está ganhando, está em outra corrida.
// Acima disso a barra satura em vez de continuar crescendo — a diferença entre
// 20 e 60 não muda nada do que o jogador precisa decidir.
const DUEL_FULL_MARGIN = 20;

// O duelo com o companheiro é a régua mais antiga do automobilismo: mesmo carro,
// mesma equipe, mesma temporada — a única comparação da ficha em que o pacote não
// tem desculpa.
//
// Três tentativas anteriores morreram aqui, e vale registrar por quê:
//
// 1. Duas barras, cada uma contra o SEU próprio máximo, com trilho de 1px sobre
//    `bg-white/10`. Preenchimento e vazio viravam a mesma linha cinza: 0 e 15
//    desenhavam idêntico, em 130px de altura.
//
// 2. Uma barra dividida em que o box inteiro era 100% dos pontos DOS DOIS. Lia
//    bem quando os dois pontuavam e mentia no resto: a fatia normaliza a
//    magnitude para fora, então 2 × 0 e 15 × 0 eram ambos "100% contra 0%" — uma
//    vantagem ridícula e uma goleada com o mesmo peso na tela.
//
// 3. Duas barras contra o total do líder do campeonato. Honesto, mas responde
//    outra pergunta: "esses dois estão na briga?" em vez de "quem está ganhando
//    o duelo, e por quanto?".
//
// Agora o ZERO é o centro da barra, e não a borda. O preenchimento nasce no meio
// e cresce para o lado de quem está na frente, proporcional à MARGEM — que é o
// único número que o duelo produz. Dois pontos empurram um dedo para o lado;
// quinze quase encostam na ponta; vinte enchem o lado inteiro. Cabo de guerra, e
// não fatia de pizza.
//
// A cor é VEREDITO, não identidade: verde quando a margem corre para o piloto
// aberto na ficha, vermelho quando corre para o companheiro. A cor da equipe saiu
// daqui de propósito — ela responde "de quem é essa barra?", e a pergunta desta
// barra é "isso é bom ou ruim para quem eu estou olhando?".
//
// Sem companheiro de equipe o bloco não existe: uma barra sozinha contra o vazio
// afirma que o piloto ganhou de alguém.
function TeammateCompare({ detail }) {
  const { t } = useTranslation();
  const leitura = detail.leitura_desempenho ?? {};
  if (!Number.isFinite(leitura.companheiro_pontos)) return null;

  const meus = leitura.piloto_pontos ?? 0;
  const dele = leitura.companheiro_pontos ?? 0;
  // `entregue_posicao` é a posição do próprio piloto no campeonato — o mesmo
  // número que a faixa lá em cima abre em corpo grande.
  const minhaPosicao = leitura.entregue_posicao;
  const posicaoDele = leitura.companheiro_posicao;
  const correu = (detail.stats_temporada?.corridas ?? 0) > 0;
  // Mesma montagem do cabeçalho: com `bandeira` o `FlagIcon` acha a arte, sem ela
  // o nome da nacionalidade sozinho ainda resolve pelo código.
  const perfil = detail.perfil ?? {};
  const bandeiraPiloto =
    perfil.bandeira && perfil.nacionalidade
      ? `${perfil.bandeira} ${perfil.nacionalidade}`
      : perfil.nacionalidade || detail.nacionalidade || "";
  const margem = meus - dele;
  const empate = margem === 0;
  const favoravel = margem > 0;
  // Fração do lado, em porcentagem da barra INTEIRA — daí o 50: cada lado é
  // metade do trilho.
  const largura = Math.min(Math.abs(margem) / DUEL_FULL_MARGIN, 1) * 50;
  const cor = favoravel ? "#3fb950" : "#f85149";
  // Onde o preenchimento termina — e onde o rótulo da margem fica pendurado.
  const ponta = empate ? 50 : favoravel ? 50 - largura : 50 + largura;
  // A frase deixou de ser desenhada: virou `aria-label` da barra. O rótulo "+6"
  // diz a mesma coisa em dois caracteres para quem enxerga, mas para um leitor de
  // tela "+6" sozinho, fora do desenho, não diz de quem é a vantagem.
  const veredito = empate
    ? t("driverDetail.performance.duelTied")
    : favoravel
      ? t("driverDetail.performance.duelAhead", { count: margem })
      : t("driverDetail.performance.duelBehind", { count: -margem });

  return (
    <div className="mt-5" data-testid="driver-detail-teammate">
      <BlockLabel>{t("driverDetail.performance.internalCompare")}</BlockLabel>
      <div className="mt-2.5 rounded-xl bg-[#0f1c2b] px-4 py-3">
        {/* A posição de cada um entra entre o nome e os pontos, espelhada nos dois
            lados. Pontos sozinhos não dizem ONDE o duelo acontece: 18 × 1 na
            briga do título e 18 × 1 no fundo do grid são a mesma barra e duas
            temporadas diferentes.

            Some antes da primeira largada pelo mesmo motivo que a posição some da
            faixa lá em cima: com o grid inteiro zerado, a ordem é desempate
            alfabético. */}
        {/* Três pesos, e não dois: bandeira e nome identificam, a POSIÇÃO é o
            contexto (11px, apagada) e os PONTOS são o dado que a barra mede
            (16px, na cor do lado). Em corpos iguais, "P4" e "12" viravam dois
            números soltos lado a lado e era preciso ler o "P" para saber qual era
            qual. */}
        <div className="flex items-center justify-between gap-4 text-sm leading-5">
          <span className="flex min-w-0 items-center gap-2">
            <FlagIcon nacionalidade={bandeiraPiloto} className="shrink-0" />
            <strong className="truncate font-semibold text-text-primary">{detail.nome}</strong>
            {correu && minhaPosicao ? (
              <span
                data-testid="driver-detail-duel-pos-piloto"
                className="shrink-0 font-mono text-[11px] text-text-muted"
              >
                P{minhaPosicao}
              </span>
            ) : null}
            <strong className="shrink-0 font-mono text-base font-semibold text-[color:var(--team)]">
              {meus}
            </strong>
          </span>
          <span className="flex min-w-0 items-center gap-2">
            <strong className="shrink-0 font-mono text-base font-semibold text-text-secondary">
              {dele}
            </strong>
            {correu && posicaoDele ? (
              <span
                data-testid="driver-detail-duel-pos-companheiro"
                className="shrink-0 font-mono text-[11px] text-text-muted"
              >
                P{posicaoDele}
              </span>
            ) : null}
            <span className="truncate text-text-secondary">
              {leitura.companheiro_nome || t("driverDetail.performance.teammate")}
            </span>
            <FlagIcon nacionalidade={leitura.companheiro_nacionalidade || ""} className="shrink-0" />
          </span>
        </div>

        <div
          data-testid="driver-detail-duel-bar"
          data-fill={Math.round(largura)}
          data-side={empate ? "empate" : favoravel ? "piloto" : "companheiro"}
          role="img"
          aria-label={veredito}
          className="relative mt-6 h-3.5"
        >
          {/* A margem em cima da PONTA, e não numa frase embaixo. Pendurada ali
              ela é lida junto com o desenho: o número, a cor e o lado para onde a
              barra aponta chegam de uma vez só.

              O `translateX(-ponta%)` alinha o rótulo sozinho nas três situações:
              em 0% ele encosta a borda esquerda na ponta, em 50% fica centrado no
              zero, em 100% encosta a borda direita. Sem isso um "+40" centrado
              sobre uma ponta saturada vazava metade para fora do card. */}
          <span
            data-testid="driver-detail-duel-margin"
            className={`pointer-events-none absolute bottom-full mb-1.5 whitespace-nowrap font-mono text-[13px] font-semibold leading-none ${
              empate ? "text-text-secondary" : ""
            }`}
            style={{
              left: `${ponta}%`,
              transform: `translateX(-${ponta}%)`,
              color: empate ? undefined : cor,
            }}
          >
            {empate ? "0" : `+${Math.abs(margem)}`}
          </span>

          <div className="absolute inset-0 overflow-hidden rounded-full bg-white/[0.07] ring-1 ring-inset ring-white/10">
            {empate ? null : (
              <div
                data-testid="driver-detail-duel-fill"
                data-tip={favoravel ? "esquerda" : "direita"}
                className="absolute inset-y-0 overflow-hidden"
                style={{
                  width: `${largura}%`,
                  // Cabo de guerra: o preenchimento parte do meio e vai para o
                  // lado de quem está na frente — o lado onde o nome dele está.
                  left: favoravel ? `${50 - largura}%` : "50%",
                  backgroundColor: cor,
                  // A DIREÇÃO é a própria ponta da barra, e não um chevron
                  // posicionado ao lado dela. A seta solta ficava 13px antes do
                  // preenchimento, boiando no trilho vazio — lia como elemento
                  // perdido, e na margem saturada ainda escapava para fora do
                  // trilho. Recortada aqui, a ponta não tem como se descolar do
                  // que ela aponta, e some junto no empate.
                  clipPath: favoravel
                    ? "polygon(9px 0, 100% 0, 100% 100%, 9px 100%, 0 50%)"
                    : "polygon(0 0, calc(100% - 9px) 0, 100% 50%, calc(100% - 9px) 100%, 0 100%)",
                }}
              >
                {/* O brilho corre no sentido da vantagem. Parado, o preenchimento
                    é um bloco ao lado de uma marca e não diz para que lado está
                    empurrando. */}
                <span
                  aria-hidden="true"
                  className={`absolute inset-y-0 left-0 w-2/5 ${
                    favoravel ? "animate-duel-flow-reverse" : "animate-duel-flow"
                  }`}
                  style={{
                    backgroundImage:
                      "linear-gradient(90deg, transparent, rgba(255,255,255,0.55), transparent)",
                  }}
                />
              </div>
            )}
          </div>

          {/* O ZERO, desenhado por cima e MAIS ALTO que o trilho. Era uma linha de
              2px contida na altura da barra, e sumia justamente quando o
              preenchimento encostava nela — o momento em que ela mais precisa
              aparecer. Transbordando em cima e embaixo, vira marca de régua: é
              ela que faz o preenchimento significar margem, e não território. */}
          <span
            aria-hidden="true"
            data-testid="driver-detail-duel-zero"
            className="pointer-events-none absolute left-1/2 top-1/2 h-[22px] w-0.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white/80"
          />
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────── Perfil ───────────────────────────────

// Quem o piloto É — a aba que faltava.
//
// Isto era um rodapé da "Temporada atual": sete dados de uma palavra espremidos
// embaixo de tudo o que fala de corrida, num lugar onde ninguém procura o que
// não muda. E o que ficava de fora era mais do que o que entrava — o backend
// guardava treze eixos e a tela mostrava quatro, com fitness e gestão de pneus
// misturados num número só porque não cabia mais nada.
//
// Depois dos traços, a ordem vai do que ele FAZ para o que ele É: leitura
// técnica (como corre), estilo (de que jeito), estrelato (o que o público vê) e
// arco (onde está na própria curva). O arco fecha a aba porque é a única leitura
// que aponta para FORA do presente — os três de cima retratam o piloto de hoje,
// e ele diz quanto disso ainda está por vir.
//
// TRAÇOS ABRE A ABA, e é o hover que ganhou o lugar. A fita é um resumo do que
// vem abaixo — cada pílula é o pico de um eixo desenhado adiante —, e resumo
// depois do detalhe é errata. Enquanto acender o eixo era só um extra ela podia
// ficar no meio; agora que é ela quem comanda o realce, ficar embaixo do que
// comanda obrigava a olhar para cima e para baixo ao mesmo tempo. No topo, o
// alvo do hover está sempre à vista e o que responde está sempre abaixo dele.
function ProfileSection({ detail }) {
  const competitivo = detail.competitivo ?? {};
  const stardom = detail.estrelato;
  const [eixoEmFoco, setEixoEmFoco] = useState(null);

  return (
    // Contexto e não prop: o eixo em foco atravessa cinco blocos e duas camadas
    // de aninhamento, e passá-lo à mão obrigaria TechnicalRead e StyleRead a
    // carregar um dado que não é deles só para entregá-lo adiante.
    <EixoEmFocoContext.Provider value={eixoEmFoco}>
      <section>
        <TraitStrip competitivo={competitivo} onFocarEixo={setEixoEmFoco} />
        <TechnicalRead itens={detail.leitura_tecnica?.itens} />
        <StyleRead itens={detail.leitura_tecnica?.itens} />
        {stardom ? <StardomBlock stardom={stardom} /> : null}
        <CareerArc arco={detail.arco} />
      </section>
    </EixoEmFocoContext.Provider>
  );
}

// O LAÇO ENTRE O TRAÇO E O EIXO.
//
// Um traço é o pico de um eixo, e a fita nunca dizia de qual — "Muro na Pista"
// não se liga sozinho a "Defesa", e o `title` só responde a quem já desconfia
// que há o que perguntar. Passar o mouse no traço acende o eixo onde ele mora,
// que é a mesma resposta mostrada no lugar em que o dado já está desenhado, em
// vez de escrita de novo dentro da pílula.
//
// O realce tem DOIS lados, e por um tempo teve só um. Apagar o entorno diz onde
// não olhar, e sozinho isso deixa o olho procurando o buraco no escuro — de
// catorze réguas, treze mudaram e a que interessa continuou igual a si mesma.
// Então o alvo também acende: placa de fundo, rótulo em branco e a régua com
// brilho na própria cor.
//
// Um dos dois lados sem o outro não serviria. Só acender exigiria um destaque
// forte o bastante para vencer treze vizinhos em brilho normal, e isso vira
// outra cor disputando a tela; só apagar deixa o alvo mudo. Juntos, um realce
// discreto basta — o contraste com o entorno é que faz o trabalho.
const EixoEmFocoContext = createContext(null);

const REALCE_BASE = "transition-opacity duration-150";
// Margem negativa igual ao respiro que a placa acrescenta: ela ganha corpo sem
// empurrar o eixo de baixo, e a coluna inteira fica parada durante o hover.
const REALCE_PLACA = "-mx-2 -my-1 rounded-md bg-white/[0.07] px-2 py-1";

function classesDeRealce(foco) {
  if (foco === "apagado") return `${REALCE_BASE} opacity-25`;
  if (foco === "aceso") return `${REALCE_BASE} ${REALCE_PLACA}`;
  return REALCE_BASE;
}

// "neutro" quando nada está em foco: o painel em repouso não é nem aceso nem
// apagado, e tratá-lo como uma das duas pontas pintaria a tela toda de estado.
function useFoco(chave) {
  const emFoco = useContext(EixoEmFocoContext);
  if (!emFoco) return "neutro";
  return chave && chave === emFoco ? "aceso" : "apagado";
}

// De que eixo cada traço veio. As chaves da esquerda são as do backend
// (`attribute_name` da tag); as da direita, as da leitura técnica (`chave` do
// item) — que NÃO são as mesmas palavras: skill vira "ritmo", fitness vira
// "preparo". `scripts/tests/driver-detail-tracos-eixos.test.mjs` cruza este mapa
// com as tabelas do Rust para o dia em que uma das duas pontas for renomeada.
//
// Os três últimos não têm régua: experiência e desenvolvimento são níveis do
// arco, e mídia É a fama do estrelato (`fama = atributos.midia` em leitura.rs —
// carisma é outro atributo, e esse não gera traço). Sem eles, um terço dos
// traços de um piloto novato ("Calouro", "Em Ascensão") teria hover morto.
const TRAIT_AXIS = {
  skill: "ritmo",
  ritmo_classificacao: "classificacao",
  consistencia: "consistencia",
  racecraft: "racecraft",
  defesa: "defesa",
  habilidade_largada: "largada",
  mentalidade: "mentalidade",
  fator_chuva: "chuva",
  gestao_pneus: "pneus",
  adaptabilidade: "adaptabilidade",
  fitness: "preparo",
  aggression: "agressividade",
  smoothness: "suavidade",
  confianca: "confianca",
  experiencia: "arco:experiencia",
  desenvolvimento: "arco:desenvolvimento",
  midia: "estrelato:fama",
};

// Uma coluna por grupo, na ordem do fim de semana e não na do payload: sábado,
// domingo, e por último o que só aparece quando a pista muda.
const TECHNICAL_COLUMNS = [["volta_seca"], ["corrida"], ["condicoes"]];
// Estilo é grupo conhecido mas NÃO entra em coluna: ele tem bloco próprio. Sem
// estar nesta lista os dois eixos cairiam na regra do órfão e apareceriam duas
// vezes — uma no bloco de estilo e outra empurrados para a volta seca.
const STYLE_GROUP = "estilo";
const TECHNICAL_GROUPS = [...TECHNICAL_COLUMNS.flat(), STYLE_GROUP];

// Uma fita única de eixos virava um paredão: nada agrupa, nada ancora, e a
// leitura vira busca. Em colunas cada uma responde a uma pergunta inteira — "como
// ele é numa volta só", "e no meio da briga", "e quando a pista muda".
function TechnicalRead({ itens }) {
  const { t } = useTranslation();
  const lista = Array.isArray(itens) ? itens : [];
  if (!lista.length) return null;

  const colunas = TECHNICAL_COLUMNS.map((grupos) =>
    grupos
      .map((grupo) => ({
        grupo,
        // Eixo sem grupo (payload antigo, antes da aba existir) não some da tela:
        // cai no primeiro bloco em vez de ser engolido pelo filtro.
        itens: lista.filter(
          (item) =>
            item.grupo === grupo ||
            (grupo === TECHNICAL_GROUPS[0] && !TECHNICAL_GROUPS.includes(item.grupo)),
        ),
      }))
      .filter((bloco) => bloco.itens.length),
  ).filter((coluna) => coluna.length);

  return (
    // `first:mt-0` para o dia em que o piloto não tem traço nenhum e a fita não
    // é desenhada: sem isso a aba abriria com uma margem órfã no topo.
    <div className="mt-4 first:mt-0" data-testid="driver-detail-technical">
      <BlockLabel>{t("driverDetail.profileTab.technicalTitle")}</BlockLabel>
      <div className="mt-2.5 grid rounded-xl bg-[#0f1c2b] py-3.5 sm:grid-cols-3">
        {colunas.map((coluna, index) => (
          <div
            key={coluna[0].grupo}
            // A divisória entre colunas existe para a coluna que ainda termina
            // cedo poder terminar cedo sem parecer truncada.
            className={`grid content-start gap-y-4 px-4 ${
              index
                ? "mt-4 border-t border-white/[0.06] pt-4 sm:mt-0 sm:border-l sm:border-t-0 sm:pt-0"
                : ""
            }`}
          >
            {coluna.map(({ grupo, itens: doGrupo }) => (
              <div key={grupo} data-technical-group={grupo}>
                {/* O título do grupo recua junto com os eixos que ele encabeça:
                    aceso sobre uma coluna inteira apagada, ele apontaria para o
                    lugar errado. */}
                <TechnicalGroupLabel itens={doGrupo}>
                  {t(`driverDetail.profileTab.groups.${grupo}`)}
                </TechnicalGroupLabel>
                <div className="mt-2.5 grid gap-y-2.5">
                  {doGrupo.map((item) => (
                    <TechnicalAxis key={item.chave || item.label} item={item} />
                  ))}
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

// O título do grupo não ACENDE — ele só deixa de apagar. A placa é do eixo, e
// dois retângulos claros na mesma coluna disputariam a atenção que o realce
// existe para dirigir a um só lugar.
function TechnicalGroupLabel({ itens, children }) {
  const emFoco = useContext(EixoEmFocoContext);
  const apagado = Boolean(emFoco) && !itens.some((item) => item.chave === emFoco);

  return (
    <span
      className={`block text-[11px] font-semibold uppercase tracking-[0.14em] text-[color:var(--team)] ${classesDeRealce(apagado ? "apagado" : "neutro")}`}
    >
      {children}
    </span>
  );
}

// A LINHA DA MEDIANA, e o que ela precisa parecer.
//
// Ela era um traço da mesma altura e do mesmo peso do dado — dois riscos
// parecidos na mesma régua, e nada dizendo qual é o piloto e qual é o resto do
// grid. Legenda escrita resolvia lendo; isto resolve OLHANDO: a linha ficou mais
// alta que a régua e mais apagada que o dado, que é a gramática de linha de
// referência de qualquer gráfico — o fundo não compete com a série.
//
// O número exato continua a um hover de distância, no `title` do eixo.
function MedianaDoGrid({ posicao }) {
  return (
    <span
      data-technical-median=""
      className="pointer-events-none absolute top-1/2 h-[13px] w-px -translate-x-1/2 -translate-y-1/2 bg-white/25"
      style={{ left: `${posicao}%` }}
    />
  );
}

// O painel de hover da régua.
//
// O `title` nativo estava errado por dois motivos: é o balão do sistema
// operacional — fonte do SO, meio segundo de espera, nenhuma relação com o resto
// da tela — e o app já tem tooltip próprio, o painel do dossiê. Este aqui usa a
// MESMA casca (mesma borda, mesmo fundo, mesma sombra, mesmo portal) e larga o
// que era daquele problema e não deste: o mecanismo de prender existe para
// alcançar uma lista que rola, e aqui são duas linhas.
//
// Portal porque a ficha rola dentro de um contêiner com `contain: layout paint`:
// um painel absoluto morreria recortado na borda da área de conteúdo.
//
// Ele diz UMA coisa: que aquela linha é a mediana do grid. Números soltos
// não cabem aqui — o valor bruto de um eixo não é linguagem do jogo (ninguém
// pensa "72 de racecraft"), e a régua já mostra a magnitude sem eles. O único
// símbolo que não se explica sozinho é a linha, e é dela que o painel trata.
const REGUA_TOOLTIP_LARGURA = 190;

function ReguaTooltip({ mediana, children }) {
  const { t } = useTranslation();
  const [rect, setRect] = useState(null);
  const alvoRef = useRef(null);

  const mostrar = useCallback(() => {
    if (alvoRef.current) setRect(alvoRef.current.getBoundingClientRect());
  }, []);
  const esconder = useCallback(() => setRect(null), []);

  // Sem mediana não há linha na régua, e sem linha não há o que explicar: o eixo
  // sai sem alvo de hover em vez de abrir um painel vazio.
  if (mediana === null) return children;

  // Abre para cima por padrão: a régua é a última linha do eixo, e um painel
  // abaixo dela cobriria o eixo seguinte, que é justamente com quem se compara.
  let estilo = null;
  if (rect) {
    const cabeAcima = rect.top - 8 >= 76;
    estilo = {
      position: "fixed",
      top: cabeAcima ? rect.top - 8 : rect.bottom + 8,
      left: Math.min(
        Math.max(8, rect.left + rect.width / 2 - REGUA_TOOLTIP_LARGURA / 2),
        window.innerWidth - REGUA_TOOLTIP_LARGURA - 8,
      ),
      width: REGUA_TOOLTIP_LARGURA,
      transform: cabeAcima ? "translateY(-100%)" : undefined,
      zIndex: 90,
    };
  }

  return (
    <div
      ref={alvoRef}
      onMouseEnter={mostrar}
      onMouseLeave={esconder}
      onFocus={mostrar}
      onBlur={esconder}
      tabIndex={0}
      className="cursor-help rounded outline-none focus-visible:ring-1 focus-visible:ring-[color:var(--team)]"
    >
      {children}
      {rect
        ? createPortal(
            <div style={estilo} data-testid="driver-detail-regua-tooltip">
              <div className="flex items-center gap-2 rounded-xl border border-white/10 bg-[#0b1522] px-3 py-2 text-[11px] text-text-secondary shadow-[0_12px_32px_rgba(0,0,0,0.55)]">
                {/* O SÍMBOLO, e não o nome dele: a legenda que serve é a que se
                    liga ao que está desenhado na régua sem intermediário. Mesmas
                    classes do traço lá em cima, para não poderem divergir. */}
                <span className="h-[11px] w-px shrink-0 bg-white/25" />
                {t("driverDetail.profileTab.tooltipMedian")}
              </div>
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}

// Um eixo: rótulo, nível e a régua de 0–100 de onde o nível saiu.
//
// O texto continua sendo a resposta — a régua é o que deixa a coluna ser LIDA de
// relance em vez de soletrada. Doze palavras empilhadas não têm magnitude: nada
// ali dizia que "Forte" está três degraus abaixo de "Elite" e um acima de
// "Competente", e descobrir isso exigia conhecer a escala de cor de antemão.
//
// Eixo de ESTILO não ganha barra cheia, e sim um marcador na régua: preencher
// diria "mais é melhor", e agressividade não tem lado bom — é a mesma razão de o
// backend deixar o tom dele neutro.
function TechnicalAxis({ item }) {
  const { t } = useTranslation();
  const cor = TONE_HEX[item.tom] || TONE_HEX.neutral;
  const posicao = naRegua(item.escala);
  const mediana = naRegua(item.referencia);
  const delta = Number(item.delta);
  const variacao = Number.isFinite(delta) && delta !== 0 ? delta : null;
  const foco = useFoco(item.chave);
  const aceso = foco === "aceso";

  return (
    <div
      data-technical={item.chave || undefined}
      data-em-foco={aceso || undefined}
      className={classesDeRealce(foco)}
    >
      <div className="flex items-baseline justify-between gap-2 text-[13px] leading-5">
        <span
          className={`min-w-0 truncate ${aceso ? "font-medium text-text-primary" : "text-text-secondary"}`}
        >
          {item.label}
        </span>
        <span className="flex shrink-0 items-baseline gap-1.5">
          {variacao === null ? null : (
            // Verde e vermelho só em eixo de QUALIDADE. Ficar mais agressivo não
            // é piorar, e pintar de vermelho diria que é — a mesma razão de o
            // eixo de estilo não ganhar barra cheia.
            <Tooltip texto={t("driverDetail.profileTab.sinceLastSeason")}>
              <span
                data-technical-delta={variacao > 0 ? "subiu" : "caiu"}
                className={`font-mono text-[10px] ${
                  item.estilo
                    ? "text-text-muted"
                    : variacao > 0
                      ? "text-status-green"
                      : "text-status-red"
                }`}
              >
                {variacao > 0 ? `+${variacao}` : variacao}
              </span>
            </Tooltip>
          )}
          <strong
            className={`font-semibold ${
              technicalToneClass[item.tom] || technicalToneClass.neutral
            }`}
          >
            {item.nivel}
          </strong>
        </span>
      </div>
      <ReguaTooltip mediana={mediana}>
        <div
          data-technical-regua=""
          className="relative mt-1 h-[3px] rounded-full bg-white/[0.07]"
        >
          {item.estilo ? (
            <span
              data-technical-marker="estilo"
              className={`absolute top-1/2 h-[7px] w-[7px] -translate-x-1/2 -translate-y-1/2 rounded-full border border-[#0f1c2b] ${
                aceso ? "bg-text-primary" : "bg-text-secondary"
              }`}
              style={{
                left: `${posicao}%`,
                boxShadow: aceso ? "0 0 8px rgba(255,255,255,0.55)" : undefined,
              }}
            />
          ) : (
            // A barra cruza ou não cruza a linha da mediana, e é isso que se lê
            // de relance: "Instável" contra quem era a pergunta que a régua abria
            // e não fechava — 45 de ritmo na F4 e 45 na GT3 desenham a mesma barra
            // e descrevem pilotos que não se parecem.
            //
            // O brilho no foco é o mesmo do medidor de estrelato — mesma fórmula,
            // para os dois realces da ficha não divergirem em alfa e raio.
            <div
              className="h-full rounded-full"
              style={{
                width: `${posicao}%`,
                backgroundColor: cor,
                boxShadow: aceso ? `0 0 10px ${cor}` : undefined,
              }}
            />
          )}
          {mediana === null ? null : <MedianaDoGrid posicao={mediana} />}
        </div>
      </ReguaTooltip>
    </div>
  );
}

// ESTILO em bloco próprio, e não como mais uma coluna da leitura técnica.
//
// Agressividade, suavidade e confiança não têm lado bom: um piloto agressivo
// ganha na largada e paga em pneu e em incidente. O backend já se recusava a dar
// nota aos três (tom neutro, marcador em vez de barra cheia), mas de pouco
// adiantava — vizinhos de eixos com nota, o marcador no meio da régua era lido
// como nota média, e o julgamento voltava pela companhia.
function StyleRead({ itens }) {
  const { t } = useTranslation();
  const lista = Array.isArray(itens) ? itens : [];
  const doEstilo = lista.filter((item) => item.grupo === STYLE_GROUP);
  if (!doEstilo.length) return null;

  return (
    // O rótulo sai da tela mas não do documento, como nos eixos: os três pares de
    // polos já dizem que ali se fala de jeito e não de nota, e a linha de título
    // custava uma altura que a aba não tem de sobra. `aria-label` mantém o bloco
    // nomeado para quem chega por leitor de tela.
    <div
      className="mt-4 grid gap-x-8 gap-y-5 rounded-xl bg-[#0f1c2b] px-4 py-3.5 sm:grid-cols-3"
      data-testid="driver-detail-style"
      role="group"
      aria-label={t("driverDetail.profileTab.styleTitle")}
    >
      {doEstilo.map((item) => (
        <StyleAxis key={item.chave || item.label} item={item} />
      ))}
    </div>
  );
}

// DUAS palavras e um marcador. Nada mais.
//
// A primeira versão deste bloco mostrava o nome do eixo, a faixa e os dois
// extremos — "Agressividade / Calculista / Cirúrgico…Beligerante": quatro
// rótulos para uma informação só, três deles dizendo a mesma coisa em graus
// diferentes. O eixo É o par. Ou pende para calculista, ou pende para agressivo,
// e o quanto está na posição do marcador.
//
// O polo para o qual ele pende vem aceso e o outro apagado: o realce faz o
// trabalho que a palavra do meio fazia, sem gastar uma linha nem uma palavra.
function StyleAxis({ item }) {
  const { t } = useTranslation();
  const posicao = naRegua(item.escala);
  const mediana = naRegua(item.referencia);
  const delta = Number(item.delta);
  const variacao = Number.isFinite(delta) && delta !== 0 ? delta : null;
  const pendeParaMax = posicao !== null && posicao >= 50;
  const foco = useFoco(item.chave);
  // `emEvidencia` e não `aceso`: `poloClass` já usa "aceso" para o polo para o
  // qual o eixo pende, que é outra coisa e some no meio se as duas se chamarem
  // igual.
  const emEvidencia = foco === "aceso";
  const poloClass = (aceso) =>
    `truncate text-[11px] uppercase tracking-[0.08em] ${
      aceso ? "font-semibold text-text-primary" : "text-text-muted"
    }`;

  return (
    // O nome do eixo sai da tela mas não do documento: quem navega por leitor de
    // tela precisa saber que este par é "agressividade" em vez de adivinhar pelas
    // duas palavras soltas.
    <div
      data-technical={item.chave || undefined}
      data-em-foco={emEvidencia || undefined}
      aria-label={item.label}
      className={classesDeRealce(foco)}
    >
      <ReguaTooltip mediana={mediana}>
        <div data-technical-regua="" className="relative h-[3px] rounded-full bg-white/[0.07]">
          {mediana === null ? null : (
            // O VÃO entre a mediana e o piloto, desenhado como comprimento. Duas
            // marcas soltas na régua obrigam a medir com o olho; o vão preenchido
            // já é a resposta — "ele está deste lado do grid, e por isto".
            <span
              data-technical-vao=""
              className="absolute top-1/2 h-[3px] -translate-y-1/2 rounded-full bg-white/20"
              style={{
                left: `${Math.min(posicao, mediana)}%`,
                width: `${Math.abs(posicao - mediana)}%`,
              }}
            />
          )}
          {mediana === null ? null : <MedianaDoGrid posicao={mediana} />}
          <span
            data-technical-marker="estilo"
            className="absolute top-1/2 h-[9px] w-[9px] -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-[#0f1c2b] bg-text-primary"
            style={{
              left: `${posicao}%`,
              boxShadow: emEvidencia ? "0 0 9px rgba(255,255,255,0.6)" : undefined,
            }}
          />
        </div>
      </ReguaTooltip>
      <div className="mt-2 flex items-baseline justify-between gap-2">
        <span className={poloClass(!pendeParaMax)}>{item.polo_min}</span>
        {variacao === null ? null : (
          // Cinza, e não verde ou vermelho: ficar mais agressivo não é melhorar
          // nem piorar, é mudar. Só a direção interessa.
          <Tooltip texto={t("driverDetail.profileTab.sinceLastSeason")}>
            <span
              data-technical-delta={variacao > 0 ? "subiu" : "caiu"}
              className="shrink-0 font-mono text-[10px] text-text-muted"
            >
              {variacao > 0 ? `+${variacao}` : variacao}
            </span>
          </Tooltip>
        )}
        <span className={poloClass(pendeParaMax)}>{item.polo_max}</span>
      </div>
    </div>
  );
}

// TRAÇOS: o pico de um eixo virado palavra.
//
// Eram pílulas soltas boiando num painel da largura da tela, e com dois sistemas
// de cor brigando dentro da mesma pílula: a borda dizia força ou fraqueza, a
// bolinha dizia o nível. "Atleta" saía com borda verde e bolinha azul, e nenhuma
// das duas cores queria dizer o que a outra dizia. Some a isso o dado que ficava
// de fora — de que eixo veio a palavra — escondido num `title`, que é o lugar
// onde a informação vai para não ser vista.
//
// Agora é um sistema só. A cor do NÍVEL manda em tudo (roxo elite, azul, verde,
// laranja, vermelho), porque ela já carrega a direção que a borda tentava
// carregar de novo. E a ordem é a do próprio nível, de elite a defeito grave, o
// que faz a fita inteira virar um degradê: forças à esquerda, atenção à direita,
// sem custar os dois títulos de painel que essa separação custava antes.
//
// O eixo de origem continua só no `title`, e é o certo: quase toda tag JÁ diz o
// eixo — "Bom Defensor" com "Defesa" embaixo é a mesma palavra duas vezes. As
// poucas opacas ("Alien", "Camaleão", "Atleta") não pagam a redundância das
// outras doze. O painel encolheu para o tamanho do conteúdo em vez de tentar
// preencher a largura: era daí que vinha o ar de inacabado, não da falta de dado.
function TraitStrip({ competitivo, onFocarEixo }) {
  const { t } = useTranslation();
  const qualidades = Array.isArray(competitivo?.qualidades) ? competitivo.qualidades : [];
  const defeitos = Array.isArray(competitivo?.defeitos) ? competitivo.defeitos : [];
  if (!qualidades.length && !defeitos.length && !competitivo?.neutro) return null;

  const tracos = [
    ...ordenarPorNivel(qualidades, "strength"),
    ...ordenarPorNivel(defeitos, "weakness"),
  ];

  return (
    // Sem painel de fundo, sem título e sem legenda — só as pílulas e a linha que
    // as separa do resto. Cada uma dessas três coisas era uma linha de altura
    // gasta para dizer o que a fita já diz sozinha: um "Traços" em cima de três
    // palavras coloridas nomeia o óbvio, e no topo da aba a faixa é claramente um
    // cabeçalho sem precisar se anunciar como um.
    //
    // `justify-center` porque três pílulas encostadas à esquerda numa faixa da
    // largura da tela parecem conteúdo que faltou carregar; centradas, a fita
    // curta é curta de propósito.
    <div
      data-testid="driver-detail-profile-strip"
      className="flex flex-wrap justify-center gap-2 border-b border-white/[0.06] pb-3"
    >
      {tracos.map(({ tag, tone }) => (
        <TraitChip
          key={`${tone}-${tag.attribute_name}-${tag.level}`}
          tag={tag}
          tone={tone}
          onFocarEixo={onFocarEixo}
        />
      ))}
      {tracos.length ? null : (
        <span className="text-xs leading-6 text-text-secondary">
          {competitivo?.neutro
            ? t("driverDetail.prosCons.balanced")
            : t("driverDetail.prosCons.noStrengths")}
        </span>
      )}
    </div>
  );
}

// ARCO: a única pergunta de contratação que a ficha inteira não respondia.
//
// Tudo o mais aqui diz quão bom o piloto é HOJE. Ninguém contrata só por isso —
// contrata-se pelo que sobra de estrada. A fase abre em corpo grande porque é a
// resposta; os três rótulos ao lado são o que a sustenta.
// A ordem das cinco fases, que é a da própria vida do piloto. Serve para a faixa
// no rodapé do bloco: "Em ascensão" em corpo grande diz onde ele está, e não
// quanto ainda falta — a faixa responde as duas de uma vez.
const ARC_PHASES = ["formacao", "ascensao", "auge", "plato", "crepusculo"];

function CareerArc({ arco }) {
  const { t } = useTranslation();
  if (!arco?.fase) return null;

  const faseAtual = ARC_PHASES.indexOf(arco.fase_chave);
  const corFase = TONE_HEX[arco.tom_fase] || TONE_HEX.neutral;
  const linhas = [
    { key: "margem", label: t("driverDetail.profileTab.ceiling"), valor: arco.nivel_margem },
    {
      key: "desenvolvimento",
      label: t("driverDetail.profileTab.development"),
      valor: arco.nivel_desenvolvimento,
    },
    {
      key: "experiencia",
      label: t("driverDetail.profileTab.experience"),
      valor: arco.nivel_experiencia,
    },
  ].filter((linha) => linha.valor);

  return (
    // Sem rótulo de bloco: "Em ascensão" em corpo grande, com a faixa das cinco
    // fases embaixo, já se apresenta — "Arco da carreira" acima só repetia em
    // miúdo o que a própria peça diz.
    <div className="mt-4" data-testid="driver-detail-arc">
      <div className="rounded-xl bg-[#0f1c2b] px-4 py-3.5">
        <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
          <div className="min-w-0 flex-1">
            <strong
              data-testid="driver-detail-arc-phase"
              // `technicalToneClass` e nao `SUMMARY_TONES`: o platô sai com tom
              // `neutral`, que só existe no primeiro mapa — no outro ele cairia no
              // azul do fallback e se leria igual a "Em ascensão".
              className={`block text-2xl font-semibold leading-tight ${
                technicalToneClass[arco.tom_fase] || technicalToneClass.neutral
              }`}
            >
              {arco.fase}
            </strong>
            {arco.resumo ? (
              // `text-secondary` e nao `text-muted`: a frase é a única prosa do
              // bloco, e no tom de rodapé ela ficava mais apagada que os rótulos
              // que só a repetem em uma palavra.
              <p className="mt-1 max-w-[52ch] text-xs leading-5 text-text-secondary">
                {arco.resumo}
              </p>
            ) : null}
          </div>
          <div className="grid shrink-0 gap-y-1">
            {linhas.map(({ key, label, valor }) => (
              <ArcRow key={key} chave={key} label={label} valor={valor} />
            ))}
          </div>
        </div>
        {faseAtual < 0 ? null : (
          <div
            data-testid="driver-detail-arc-track"
            className="mt-4 grid grid-cols-5 gap-1.5 border-t border-white/[0.06] pt-3.5"
          >
            {ARC_PHASES.map((fase, index) => {
              const atual = index === faseAtual;
              return (
                <div key={fase} data-arc-phase={fase} data-arc-current={atual || undefined}>
                  <div
                    className="h-[3px] rounded-full"
                    style={{
                      // O que já passou fica visível, e não apagado: a estrada
                      // andada é metade da resposta de quanto ainda sobra.
                      backgroundColor: atual
                        ? corFase
                        : index < faseAtual
                          ? "rgba(255,255,255,0.22)"
                          : "rgba(255,255,255,0.07)",
                    }}
                  />
                  <span
                    className={`mt-1.5 block truncate text-[10px] uppercase tracking-[0.1em] ${
                      atual ? "font-semibold" : "text-text-muted"
                    }`}
                    style={atual ? { color: corFase } : undefined}
                  >
                    {t(`driverDetail.profileTab.phases.${fase}`)}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

// `arco:` no prefixo porque "experiencia" é ao mesmo tempo uma linha do arco e um
// atributo do piloto: sem o prefixo, o traço "Veterano Sábio" acenderia aqui e,
// se um dia a leitura técnica ganhar um eixo de mesmo nome, nos dois lugares.
function ArcRow({ chave, label, valor }) {
  const foco = useFoco(`arco:${chave}`);
  const aceso = foco === "aceso";

  return (
    <div
      data-arc={chave}
      data-em-foco={aceso || undefined}
      className={`flex items-baseline justify-between gap-3 text-[13px] leading-5 ${classesDeRealce(foco)}`}
    >
      <span className={aceso ? "font-medium text-text-primary" : "text-text-secondary"}>
        {label}
      </span>
      <strong className="shrink-0 font-semibold text-text-primary">{valor}</strong>
    </div>
  );
}

// Estrelato saiu da aba Mercado. Fama e carisma são traços da PESSOA — o que o
// contrato faz com eles é consequência, não o contrário; ali eles eram lidos
// como cláusula.
function StardomBlock({ stardom }) {
  const { t } = useTranslation();

  return (
    // Sem rótulo: as duas medidas já se chamam "Fama" e "Carisma" dentro do
    // painel, e a linha de prosa embaixo diz o que elas fazem uma com a outra.
    <div className="mt-4" data-testid="driver-detail-stardom">
      <div className="rounded-xl bg-[#0f1c2b] px-4 py-3.5">
        <div className="grid gap-3 sm:grid-cols-2">
          <StardomMeter
            eixo="estrelato:fama"
            label={t("driverDetail.stardom.fame")}
            value={stardom.fama}
            level={stardom.nivel_fama}
            tone={stardom.tom_fama}
          />
          <StardomMeter
            eixo="estrelato:carisma"
            label={t("driverDetail.stardom.charisma")}
            value={stardom.carisma}
            level={stardom.nivel_carisma}
            tone={stardom.tom_carisma}
          />
        </div>
        {stardom.resumo ? (
          // Centralizada: a frase fecha um painel de duas colunas simétricas, e
          // alinhada à esquerda ela se pendurava só na de fama.
          <p className="mt-3 text-center text-xs leading-5 text-text-secondary">{stardom.resumo}</p>
        ) : null}
      </div>
    </div>
  );
}

function StardomMeter({ eixo, label, value, level, tone }) {
  const color = TONE_HEX[tone] || TONE_HEX.neutral;
  const width = Math.max(0, Math.min(Number(value) || 0, 100));
  const foco = useFoco(eixo);
  const aceso = foco === "aceso";
  return (
    <div data-stardom={eixo} data-em-foco={aceso || undefined} className={classesDeRealce(foco)}>
      <div className="flex items-baseline justify-between gap-3">
        <span
          className={`text-xs font-semibold ${aceso ? "text-text-primary" : "text-text-secondary"}`}
        >
          {label}
        </span>
        <span className="flex shrink-0 items-baseline gap-1.5">
          {/* O número subiu para a linha do nível: embaixo da barra ele ocupava
              uma terceira linha por medidor para repetir o que a barra desenha, e
              alinhado à direita sozinho não se ligava a coisa nenhuma. */}
          <span className="font-mono text-[10px] text-text-muted">{width}</span>
          <span className="text-xs font-semibold" style={{ color }}>
            {level}
          </span>
        </span>
      </div>
      <div className="mt-1.5 h-1.5 rounded-full bg-white/[0.07]">
        <div
          className="h-full rounded-full"
          style={{
            width: `${width}%`,
            backgroundColor: color,
            // O medidor já nasce com brilho; no foco ele fica em alfa cheio, que
            // é a mesma diferença que a régua da leitura técnica faz ao acender.
            boxShadow: aceso ? `0 0 12px ${color}` : `0 0 10px ${color}59`,
          }}
        />
      </div>
    </div>
  );
}

// A bolinha some e a cor vai para a PALAVRA. Ela era o único portador do nível
// numa pílula que já tinha borda de outra cor, e 6px de diâmetro é pouco pano
// para separar cinco tons — sem ela, o roxo do elite passa a ter o tamanho do
// texto. Borda e fundo saem do mesmo tom em alfas baixos, para a pílula continuar
// sendo uma peça só em vez de três decisões de cor.
const TRAIT_FALLBACK_COLOR = "#8b949e";

function TraitChip({ tag, tone, onFocarEixo }) {
  const cor = tag.color || TRAIT_FALLBACK_COLOR;
  const eixo = TRAIT_AXIS[tag.attribute_name] || null;
  const acender = useCallback(() => onFocarEixo?.(eixo), [onFocarEixo, eixo]);
  const apagar = useCallback(() => onFocarEixo?.(null), [onFocarEixo]);

  return (
    // `focus`/`blur` além do mouse, e `tabIndex` para que existam: quem chega de
    // teclado percorre a fita e vê a mesma resposta que o ponteiro dá.
    <Tooltip texto={formatAttributeName(tag.attribute_name)}>
      <span
        data-trait={tone}
        data-trait-level={tag.level || undefined}
        data-trait-eixo={eixo || undefined}
        tabIndex={eixo ? 0 : undefined}
        onMouseEnter={acender}
        onMouseLeave={apagar}
        onFocus={acender}
        onBlur={apagar}
        className="max-w-full truncate rounded-full border px-3 py-1 text-[13px] font-semibold leading-5 outline-none focus-visible:ring-1 focus-visible:ring-current"
        style={{
          color: cor,
          borderColor: `${cor}59`,
          backgroundColor: `${cor}14`,
        }}
      >
        {tag.tag_text}
      </span>
    </Tooltip>
  );
}

// ────────────────────────────── Histórico ──────────────────────────────

function HistorySection({ detail, onAbrirEquipe, onAbrirRanking, careerId }) {
  const { t } = useTranslation();
  const trajetoria = detail.trajetoria ?? {};
  const historico = trajetoria.historico ?? {};
  const career = detail.stats_carreira ?? {};
  const peak = historico.auge ?? {};
  const bestSeason = peak.melhor_temporada;
  // Contra quem os números de carreira são medidos. O mundo é o padrão porque é
  // a leitura que não muda de significado — o grid é o recorte de HOJE, e some
  // quando o piloto fica sem contrato. O backend manda `rankings_grid: null`
  // nesse caso, e aí não há o que alternar.
  const ranksMundo = detail.rankings_carreira ?? {};
  const ranksGrid = detail.rankings_grid ?? null;
  const [escopo, setEscopo] = useState("mundo");
  const noGrid = Boolean(escopo === "grid" && ranksGrid);
  const ranks = noGrid ? ranksGrid : ranksMundo;

  if ((career.corridas ?? 0) === 0) {
    return (
      <div className="rounded-xl bg-[#0f1c2b] px-4 py-3.5 text-xs text-text-secondary">
        {t("driverDetail.summary.noCompetitivePastRead")}
      </div>
    );
  }

  // O clique leva o RECORTE junto. Quem está lendo "2º de 12 no grid atual" e
  // clica no card espera cair na lista da categoria dele, e não nos 610 do
  // mundo — a tela de destino tem que responder a mesma pergunta que a origem
  // estava respondendo. No recorte mundial não vai categoria nenhuma.
  const abrirRankingNoEscopo = onAbrirRanking
    ? (metric) => onAbrirRanking(metric, noGrid ? trajetoria.categoria_atual ?? null : null)
    : null;

  // O detalhe que abre no hover de cada card, na mesma chave do dossiê. Corridas
  // reaproveita a lista de temporadas — é a mesma pergunta ("em que ano, por qual
  // equipe") e a resposta já estava no payload, com pontos e posição no
  // campeonato de brinde.
  const detalhes = historico.detalhes ?? {};
  const records = [
    {
      id: "corridas",
      label: t("driverDetail.history.races"),
      value: career.corridas ?? 0,
      rank: ranks.corridas,
      detalhe: "temporadas",
    },
    {
      id: "vitorias",
      label: t("driverDetail.history.wins"),
      value: career.vitorias ?? 0,
      rank: ranks.vitorias,
      detalhe: "vitorias",
    },
    {
      id: "podios",
      label: t("driverDetail.history.podiums"),
      value: career.podios ?? 0,
      rank: ranks.podios,
      detalhe: "podios",
    },
    {
      id: "titulos",
      label: t("driverDetail.history.titles"),
      value: trajetoria.titulos ?? 0,
      rank: ranks.titulos,
      detalhe: "titulos",
      // COM QUEM ele foi campeão mora no card que conta os títulos. Era um card
      // de troféu à parte, e o número e a resposta ficavam em lugares
      // diferentes: "3" num card, "com a Ferrari em 2019, 2020 e 2023" no
      // outro, uma fileira abaixo.
      titleGroups: groupTitlesByTeam(trajetoria.titulos_detalhe),
    },
  ];

  return (
    <section>
      {/* O recorte das posições fica COLADO nos cards que as mostram — e ACIMA
          deles: quem lê "2º de 12" precisa saber de que conta esse 12 saiu antes
          de ler o número, não depois. No rodapé da ficha, como já esteve, a
          frase valia para a tela inteira e o "40º" da aba de trás continuava sem
          denominador.

          Com o seletor na tela a frase vira eco: "de 12" já está em cada card e
          "Grid atual" já está no botão aceso. Ela só sobrevive para quem NÃO tem
          grid — aí não há botão dizendo contra quem a conta foi feita. */}
      <div className="mb-2 flex justify-center">
        {ranksGrid ? (
          <SeletorDeEscopo escopo={escopo} onEscopo={setEscopo} />
        ) : ranks.total > 0 ? (
          <p className="text-[11px] text-text-muted" data-testid="driver-detail-rank-scope">
            {t("driverDetail.v2.worldScope", { count: ranks.total })}
          </p>
        ) : null}
      </div>

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {records.map((record) => (
          // O número responde QUANTO e cala o resto: em que ano, por qual equipe,
          // em que categoria. O hover abre exatamente isso — a mesma resposta que
          // o dossiê já dá para os contadores dele, no mesmo painel, agora nos
          // quatro cards que abrem a aba. Uma linha por temporada, e não por
          // corrida: 54 pódios viram catorze linhas com o ano de cada uma.
          <DossierDetailTooltip
            key={record.id}
            entradas={detalhes[record.detalhe]}
            onAbrirEquipe={onAbrirEquipe}
            // O invólucro vira o item do grid, e sem isso o card para de esticar
            // junto com os irmãos — o de títulos é mais alto que os outros três.
            className="h-full"
          >
            <RecordCard record={record} total={ranks.total} onAbrirRanking={abrirRankingNoEscopo} />
          </DossierDetailTooltip>
        ))}
      </div>

      {/* No lugar da escada de categorias, que era esta mesma carreira pintada
          em blocos de cor: a curva já traz a categoria como coluna de fundo e a
          equipe na fita do rodapé, e ainda responde a pergunta que a escada não
          respondia — ONDE ele terminou cada ano. Manter as duas poria a mesma
          faixa de anos duas vezes na mesma tela, uma delas dizendo menos.

          A moldura é a mesma da curva de mercado, na aba ao lado. */}
      <div className="mt-5">
        <BlockLabel>{t("driverDetail.history.championshipCurve.blockLabel")}</BlockLabel>
        <CurvaDeCampeonato
          pontos={trajetoria.curva_campeonato}
          equipeDeEstreia={trajetoria.equipe_estreia}
        />
      </div>

      <CareerDossier
        historico={historico}
        ativo={detail.status === "ativo"}
        onAbrirEquipe={onAbrirEquipe}
        careerId={careerId}
        driverId={detail.id}
      />
    </section>
  );
}

// Contra quem os quatro números de carreira são medidos: o mundo inteiro ou o
// grid de hoje. As duas leituras respondem perguntas diferentes — "que carreira
// é essa" e "onde eu estou entre os caras de domingo" — e a segunda era
// impossível de fazer com "570º de 610" na tela.
//
// Segmentado, e não um checkbox de "só o meu grid": com dois rótulos visíveis o
// jogador lê o recorte atual sem precisar clicar para descobrir o que a opção
// desligada significa.
function SeletorDeEscopo({ escopo, onEscopo }) {
  const { t } = useTranslation();
  const opcoes = [
    ["mundo", t("driverDetail.v2.scope.world")],
    ["grid", t("driverDetail.v2.scope.grid")],
  ];

  return (
    <div
      role="group"
      aria-label={t("driverDetail.v2.scope.aria")}
      data-testid="driver-detail-rank-scope-toggle"
      className="inline-flex rounded-full border border-white/10 bg-white/[0.04] p-px"
    >
      {opcoes.map(([chave, label]) => {
        const ativo = escopo === chave;
        return (
          <button
            key={chave}
            type="button"
            aria-pressed={ativo}
            onClick={() => onEscopo(chave)}
            data-testid={`driver-detail-rank-scope-${chave}`}
            className={[
              "rounded-full px-2.5 py-0.5 text-[10px] font-semibold transition-glass",
              ativo
                ? "bg-[color-mix(in_srgb,var(--team)_26%,transparent)] text-text-primary"
                : "text-text-muted hover:text-text-secondary",
            ].join(" ")}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}

// Card de número de carreira: valor, barra de posição e o rank por extenso. A
// barra enche na PROPORÇÃO INVERSA da posição — 1º de 610 enche tudo, 610º de
// 610 fica quase vazia. Sem o total (payload de build antiga) a barra some em
// vez de inventar um denominador, e sobra o ordinal solto.
function RecordCard({ record, total, onAbrirRanking = null }) {
  const { t } = useTranslation();
  const hasScale = total > 0 && record.rank > 0;
  const fill = hasScale ? ((total - record.rank + 1) / total) * 100 : 0;
  // O card INTEIRO é o alvo, e não um pedaço dele. Quando só o miolo era botão,
  // a única coisa que reagia ao mouse era a linha da posição — um card de 200px
  // de largura anunciando o clique num rótulo de 11px, e as bordas, o número
  // grande e o ícone ficando mortos ao passar por cima.
  //
  // O botão é uma camada por cima (`inset-0`), e não o container: os títulos por
  // equipe precisam continuar recebendo mouse para abrir as próprias tooltips, e
  // eles sobem para cima da camada com `relative`. Assim o card todo acende e
  // clica, e as tooltips de dentro continuam vivas.
  const clicavel = Boolean(onAbrirRanking);
  const rotuloDoLink = t("driverDetail.v2.openRanking", { label: record.label });

  return (
    <div
      data-record={record.id}
      className={[
        "group relative h-full rounded-xl bg-[#0f1c2b] px-4 py-3.5 transition-glass",
        clicavel
          // A cor da equipe com alpha vem de `color-mix`, e não do modificador
          // `/50` do Tailwind: sobre uma var CSS ele não gera regra nenhuma, e o
          // realce simplesmente não apareceria.
          ? "cursor-pointer hover:bg-[#14263a] hover:ring-1 hover:ring-[color-mix(in_srgb,var(--team)_55%,transparent)] hover:shadow-[0_0_26px_-6px_var(--team)]"
          : "",
      ].join(" ")}
    >
      {clicavel ? (
        <button
          type="button"
          onClick={() => onAbrirRanking(record.id)}
          // Sem `title`: o balão nativo do navegador chega atrasado, com a
          // moldura do sistema no meio de uma tela desenhada à mão, e repete o
          // que o card já mostra. O `aria-label` continua — ele não desenha
          // nada, só nomeia o botão para quem não vê o card.
          aria-label={rotuloDoLink}
          data-testid={`driver-detail-record-link-${record.id}`}
          className="absolute inset-0 rounded-xl focus:outline-none focus-visible:ring-2 focus-visible:ring-[color:var(--team)]"
        />
      ) : null}
      <span className="pointer-events-none absolute right-3 top-3 text-white/15 transition-glass group-hover:text-[color-mix(in_srgb,var(--team)_75%,transparent)]">
        <MetricIcon name={record.id} size={22} />
      </span>
      <span className="pointer-events-none block truncate pr-7 text-xs font-semibold text-text-secondary transition-glass group-hover:text-text-primary">
        {record.label}
      </span>
      <strong className="pointer-events-none mt-1 block font-mono text-2xl leading-none text-text-primary">
        {record.value}
      </strong>
      {hasScale ? (
        <div className="pointer-events-none">
          <div className="mt-2.5 h-1 overflow-hidden rounded-full bg-white/10">
            <div
              className="h-full rounded-full bg-[color:var(--team)] transition-glass group-hover:shadow-[0_0_10px_var(--team)]"
              style={{ width: `${fill}%` }}
            />
          </div>
          <span className="mt-1.5 flex items-center gap-1 text-xs text-text-secondary transition-glass group-hover:text-text-primary">
            {t("driverDetail.v2.rankOf", { rank: ordinal(record.rank), total })}
            {clicavel ? (
              <span aria-hidden="true" className="opacity-0 transition-glass group-hover:opacity-100">
                ›
              </span>
            ) : null}
          </span>
        </div>
      ) : (
        <span className="pointer-events-none mt-2.5 block text-xs text-text-secondary transition-glass group-hover:text-text-primary">
          {record.rank > 0 ? ordinal(record.rank) : "-"}
        </span>
      )}
      {/* Os títulos, agrupados POR EQUIPE. O número responde quantos e esconde
          as duas coisas que se quer saber em seguida: quando, e com quem.

          Um chip por título quebrava no campeão de verdade: 12 títulos viravam
          quatro fileiras e o card estourava. E era repetição — nove daqueles
          chips eram a mesma equipe, com a mesma logo, um ao lado do outro. Por
          equipe, a mesma carreira cabe em três linhas e ainda ganha o que a
          lista plana não dava: quanto da dinastia foi construída onde. Some
          sozinho em piloto histórico pré-gerado, que tem o total mas não tem
          arquivo de temporada.

          As equipes fluem LADO A LADO, e não uma por linha. Uma linha por equipe
          fazia o tricampeão da imagem crescer noventa pixels sobre os outros
          três cards da fileira — e como os quatro dividem a mesma linha do grid,
          o vazio aparecia embaixo de Corridas, Vitórias e Pódios, não aqui. Com
          o wrap, três títulos em três equipes cabem numa linha só. A dinastia
          continua legível porque os anos de cada equipe têm a própria caixa ao
          lado da própria logo: quando eles quebram, quebram DENTRO dela. */}
      {record.titleGroups?.length ? (
        // Sem balão próprio nos grupos: quem responde "em que categoria foi cada
        // um desses títulos" agora é o painel do card inteiro, que abre no mesmo
        // gesto e traz o ano, a equipe e a categoria de cada título em vez de
        // uma linha só com as categorias somadas. Dois balões no mesmo hover se
        // cobriam.
        <div
          className="mt-2.5 flex flex-wrap items-start gap-x-3 gap-y-1.5"
          data-testid="driver-detail-title-years"
        >
          {record.titleGroups.slice(0, MAX_TITLE_TEAMS).map((group) => (
            <span
              key={group.key}
              data-title-team={group.equipe ?? ""}
              className="flex min-w-0 items-start gap-2"
            >
              {group.equipe ? (
                <TeamLogoMark
                  teamName={group.equipe}
                  size="xs"
                  halo
                  testId="driver-detail-title-logo"
                />
              ) : (
                // Equipe que não existe mais: o espaço da logo fica reservado
                // para os anos não desalinharem da coluna.
                <span aria-hidden="true" className="h-6 w-9 shrink-0" />
              )}
              <span className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5 pt-1 font-mono text-[11px] font-semibold text-status-yellow">
                {group.blocos.map((bloco, index) => (
                  // O índice entra na chave porque dois títulos no mesmo ano
                  // produzem dois blocos de rótulo idêntico.
                  <span key={`${bloco.key}-${index}`} data-title-year={bloco.key}>
                    {bloco.label}
                  </span>
                ))}
              </span>
            </span>
          ))}
          {record.titleGroups.length > MAX_TITLE_TEAMS ? (
            // `basis-full` para o resumo cair numa linha só dele: encaixado ao
            // lado da última logo ele viraria mais um grupo de títulos.
            <span className="basis-full text-[11px] text-text-muted">
              {t("driverDetail.v2.titlesMoreTeams", {
                count: record.titleGroups
                  .slice(MAX_TITLE_TEAMS)
                  .reduce((soma, group) => soma + group.anos.length, 0),
              })}
            </span>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

// Dossiê de carreira: os mesmos seis grupos do v1, mas em cartões de três
// colunas em vez de uma lista única de trinta linhas. O v1 empilhava tudo num
// painel só e o jogador rolava procurando a linha; aqui cada grupo é um bloco
// com título e a varredura para no que interessa.
function CareerDossier({ historico, ativo = true, onAbrirEquipe, careerId, driverId }) {
  const { t } = useTranslation();
  // Antes do `return null` de propósito: hook não pode ficar atrás de saída
  // antecipada, e a ficha sem histórico é um caminho real (estreante).
  const [recordesLigados, setRecordesLigados] = useState(false);
  // Os recordes NÃO vêm no payload da ficha. Montá-los exige varrer o mundo
  // inteiro — 503ms num save de 27 mil resultados, medido em debug, contra
  // 512ms do bloco de histórico completo. Enquanto viajavam junto, esse meio
  // segundo era cobrado de toda abertura e de TODA troca de piloto para
  // alimentar um toggle que nasce desligado. Agora quem liga o toggle paga.
  //
  // Não precisa de limpeza ao trocar de piloto: o bloco inteiro é remontado
  // pelo `key` do piloto lá em cima, e o estado nasce zerado junto.
  const [recordesBuscados, setRecordesBuscados] = useState(null);
  const [buscandoRecordes, setBuscandoRecordes] = useState(false);
  // Quem já foi buscado, marcado pelo piloto. É ref, e não estado, de propósito:
  // como estado ele entraria nas deps do efeito, o efeito re-rodaria ao ser
  // marcado, e a limpeza da passagem anterior cancelaria a busca que estava no
  // ar — o botão ficava pulsando para sempre.
  const buscaFeitaRef = useRef(null);

  useEffect(() => {
    if (!recordesLigados || !careerId || !driverId) return undefined;
    if (buscaFeitaRef.current === driverId) return undefined;
    buscaFeitaRef.current = driverId;

    // `vivo`, e não `ativo`: `ativo` já é o prop que diz se o piloto ainda corre.
    let vivo = true;
    setBuscandoRecordes(true);
    invoke("get_driver_dossier_ranks", { careerId, driverId })
      .then((payload) => {
        if (!vivo) return;
        setRecordesBuscados(payload ?? {});
      })
      .catch(() => {
        // Recorde é enfeite: falhar aqui não pode manchar a ficha. O mapa vazio
        // desliga os ordinais e o resto do dossiê segue igual.
        if (vivo) setRecordesBuscados({});
      })
      .finally(() => {
        if (vivo) setBuscandoRecordes(false);
      });

    return () => {
      vivo = false;
    };
  }, [recordesLigados, careerId, driverId]);

  if (!historico) return null;

  const presenca = historico.presenca ?? {};
  const marcos = historico.primeiros_marcos ?? {};
  const auge = historico.auge ?? {};
  const queda = historico.queda ?? {};
  const confiabilidade = historico.confiabilidade ?? {};
  const sabado = historico.sabado ?? {};
  const duelos = historico.duelos ?? {};
  const referencias = historico.referencias ?? {};
  // O detalhe de cada linha, indexado pela chave que a linha declara em
  // `detalhe`. Linha sem chave — ou com chave que o backend não preencheu —
  // simplesmente não abre painel nenhum.
  const detalhes = historico.detalhes ?? {};
  // A posição de cada número, na MESMA chave do detalhe. Nem toda linha tem —
  // "melhor temporada" e "companheiro mais duro" não são número comparável.
  //
  // O que o comando trouxe manda; `historico.recordes` fica como retaguarda para
  // payload de build antiga, que ainda os mandava embutidos.
  const recordes = recordesBuscados ?? historico.recordes ?? {};
  const entradasDeRecorde = Object.values(recordes);
  // O tamanho das duas populações. É o MAIOR denominador, e não o de uma linha
  // qualquer: cada métrica exclui quem não tem aquele número (grid médio só
  // conta quem tem largada registrada), então nenhuma linha sozinha representa
  // o mundo. O número exato de cada uma fica no `title` do próprio ordinal.
  const totalGrid = Math.max(0, ...entradasDeRecorde.map((r) => r.grid_total ?? 0));
  const totalMundo = Math.max(0, ...entradasDeRecorde.map((r) => r.mundo_total ?? 0));
  const mobilidade = historico.mobilidade ?? {};
  const lesoes = historico.lesoes ?? {};
  const especiais = historico.eventos_especiais ?? {};

  // A ordem é a de uma carreira lida de fora para dentro, e cada LINHA do grid
  // de três colunas é um tema:
  //
  //   quem ele é       PRESENÇA · MOBILIDADE · PRIMEIROS MARCOS
  //   o que entrega    AUGE · SÁBADO · DUELOS
  //   o que custa      QUEDA · CONFIABILIDADE · LESÕES
  //
  // As colunas também pareiam: auge fica exatamente em cima de queda, e sábado
  // em cima de confiabilidade. A ordem anterior era a ordem em que os cards
  // foram escritos, e o leitor tinha que garimpar.
  const grupos = [
    {
      key: "presenca",
      title: t("driverDetail.history.groupPresence"),
      rows: [
        {
          label: t("driverDetail.history.careerTime"),
          value: formatCareerYears(presenca.tempo_carreira),
          detalhe: "tempo_carreira",
          // Duas linhas sem nome lêem como uma lista de equipes cortada. São as
          // duas PONTAS da carreira, e é o rótulo que diz isso — as equipes do
          // meio estão em "Equipes defendidas". Para quem já parou de correr a
          // segunda ponta é a última equipe, e não a atual.
          legendas: [
            t("driverDetail.history.tooltipDebutTeam"),
            t(ativo ? "driverDetail.history.tooltipCurrentTeam" : "driverDetail.history.tooltipLastTeam"),
          ],
        },
        { label: t("driverDetail.history.seasonsPlayed"), value: presenca.temporadas_disputadas ?? 0, detalhe: "temporadas" },
        { label: t("driverDetail.history.yearsUnemployed"), value: formatUnemploymentYears(presenca), detalhe: "anos_parados" },
        { label: t("driverDetail.history.categoriesContested"), value: presenca.categorias_disputadas ?? 0, detalhe: "categorias" },
      ],
    },
    {
      key: "mobilidade",
      title: t("driverDetail.history.groupMobility"),
      rows: [
        { label: t("driverDetail.history.promotions"), value: mobilidade.promocoes ?? 0, detalhe: "promocoes" },
        { label: t("driverDetail.history.relegations"), value: mobilidade.rebaixamentos ?? 0, detalhe: "rebaixamentos" },
        { label: t("driverDetail.history.teamsDefended"), value: mobilidade.equipes_defendidas ?? 0, detalhe: "equipes" },
        { label: t("driverDetail.history.avgTimePerTeam"), value: formatYearsAverage(mobilidade.tempo_medio_por_equipe), detalhe: "tempo_medio_por_equipe" },
      ],
    },
    {
      key: "primeiros",
      title: t("driverDetail.history.groupFirstMarks"),
      // Do marco mais alto para o mais baixo, e o DNF por último — é o único que
      // não é conquista. A ordem antiga era cronológica (pódio antes de vitória,
      // porque quase sempre vem antes), mas ler uma carreira de cima para baixo
      // é o que a coluna faz nos outros cards.
      rows: [
        { label: t("driverDetail.history.firstTitle"), value: formatSeasonMilestone(marcos.primeiro_titulo), detalhe: "primeiro_titulo" },
        { label: t("driverDetail.history.firstWin"), value: formatRaceMilestone(marcos.primeira_vitoria_corrida), detalhe: "primeira_vitoria" },
        { label: t("driverDetail.history.firstPodium"), value: formatRaceMilestone(marcos.primeiro_podio_corrida), detalhe: "primeiro_podio" },
        { label: t("driverDetail.history.firstDnf"), value: formatRaceMilestone(marcos.primeiro_dnf_corrida), detalhe: "primeiro_dnf" },
      ],
    },
    // AUGE e QUEDA são espelhos, linha por linha: melhor/pior temporada,
    // sequência/jejum de vitórias, sequência/jejum de pódios, temporadas no
    // pódio do campeonato / sem pódio nenhum. Um card só diz o que o outro
    // responde, e na mesma ordem.
    //
    // A colocação saiu da linha própria e foi colada na temporada: "Melhor
    // campeonato P1" era a posição da melhor temporada repetida — uma linha
    // inteira gasta para dizer de novo o que a de cima já dizia.
    {
      key: "auge",
      title: t("driverDetail.history.groupPeak"),
      rows: [
        { label: t("driverDetail.history.bestSeason"), value: formatSeasonWithResult(auge.melhor_temporada), detalhe: "melhor_temporada" },
        {
          label: t("driverDetail.history.longestWinStreak"),
          value: formatStreakRaces(
            auge.maior_sequencia_vitorias,
            auge.sequencia_ano_inicio,
            auge.sequencia_ano_fim,
          ),
          detalhe: "sequencia_vitorias",
        },
        {
          label: t("driverDetail.history.longestPodiumStreak"),
          value: formatStreakRaces(
            auge.maior_sequencia_podios,
            auge.sequencia_podios_ano_inicio,
            auge.sequencia_podios_ano_fim,
          ),
          detalhe: "sequencia_podios",
        },
        { label: t("driverDetail.history.seasonsInTop3"), value: auge.temporadas_no_top3 ?? 0, detalhe: "temporadas_no_top3" },
      ],
    },
    {
      key: "sabado",
      title: t("driverDetail.history.groupQualifying"),
      rows: [
        { label: t("driverDetail.history.poles"), value: sabado.poles ?? 0, detalhe: "poles" },
        // Largar na frente e converter são habilidades diferentes; a distância
        // entre as duas linhas é o retrato do piloto.
        { label: t("driverDetail.history.polesConverted"), value: sabado.poles_convertidas ?? 0, detalhe: "poles_convertidas" },
        {
          label: t("driverDetail.history.averageGrid"),
          value: formatAverageGrid(sabado.grid_medio),
          hint: formatWorldAverage(referencias.grid_medio, formatAverageGrid),
          detalhe: "grid_medio",
        },
        { label: t("driverDetail.history.fastestLaps"), value: sabado.voltas_rapidas ?? 0, detalhe: "voltas_rapidas" },
      ],
    },
    {
      key: "duelos",
      title: t("driverDetail.history.groupTeammates"),
      rows: [
        { label: t("driverDetail.history.teammatesFaced"), value: duelos.companheiros ?? 0, detalhe: "companheiros" },
        {
          label: t("driverDetail.history.duelSeasonsWon"),
          value: duelos.temporadas
            ? t("driverDetail.history.duelSeasonsOf", {
                won: duelos.temporadas_vencidas ?? 0,
                total: duelos.temporadas,
              })
            : "-",
          detalhe: "temporadas_vencidas",
        },
        { label: t("driverDetail.history.toughestTeammate"), value: formatDuel(duelos.rival_mais_duro), detalhe: "rival_mais_duro" },
      ],
    },
    {
      key: "queda",
      title: t("driverDetail.history.groupDrought"),
      rows: [
        { label: t("driverDetail.history.worstSeason"), value: formatSeasonWithResult(queda.pior_temporada), detalhe: "pior_temporada" },
        {
          label: t("driverDetail.history.longestWinless"),
          value: formatStreakRaces(queda.maior_seca_vitorias, queda.seca_ano_inicio, queda.seca_ano_fim),
          detalhe: "jejum_vitorias",
        },
        {
          label: t("driverDetail.history.longestPodiumless"),
          value: formatStreakRaces(
            queda.maior_seca_podios,
            queda.seca_podios_ano_inicio,
            queda.seca_podios_ano_fim,
          ),
          detalhe: "jejum_podios",
        },
        { label: t("driverDetail.history.seasonsWithoutPodium"), value: queda.temporadas_sem_podio ?? 0, detalhe: "temporadas_sem_podio" },
      ],
    },
    {
      key: "confiabilidade",
      title: t("driverDetail.history.groupReliability"),
      rows: [
        { label: t("driverDetail.history.retirements"), value: confiabilidade.abandonos ?? 0, detalhe: "abandonos" },
        {
          label: t("driverDetail.history.retirementRate"),
          value: formatRetirementRate(confiabilidade.taxa_abandono),
          hint: formatWorldAverage(referencias.taxa_abandono, formatRetirementRate),
          detalhe: "taxa_abandono",
        },
        {
          label: t("driverDetail.history.longestFinishStreak"),
          value: confiabilidade.maior_sequencia_chegadas ?? 0,
          detalhe: "sequencia_chegadas",
        },
      ],
    },
    {
      key: "lesoes",
      title: t("driverDetail.history.groupInjuries"),
      rows: [
        { label: t("driverDetail.history.injuriesLight"), value: lesoes.leves ?? 0, detalhe: "lesoes_leves" },
        { label: t("driverDetail.history.injuriesModerate"), value: lesoes.moderadas ?? 0, detalhe: "lesoes_moderadas" },
        { label: t("driverDetail.history.injuriesSevere"), value: lesoes.graves ?? 0, detalhe: "lesoes_graves" },
      ],
    },
    // Eventos especiais só aparecem para quem esteve em algum: o bloco com seis
    // zeros era a regra, e não a exceção, na maior parte do grid.
    especiais.participacoes > 0
      ? {
          key: "especiais",
          title: t("driverDetail.history.groupSpecialEvents"),
          rows: [
            { label: t("driverDetail.history.participations"), value: especiais.participacoes ?? 0, detalhe: "especiais" },
            { label: t("driverDetail.history.callUps"), value: especiais.convocacoes ?? 0, detalhe: "especiais" },
            { label: t("driverDetail.history.wins"), value: especiais.vitorias ?? 0, detalhe: "especiais_vitorias" },
            { label: t("driverDetail.history.bestCampaign"), value: formatSpecialCampaign(especiais.melhor_campanha) },
            { label: t("driverDetail.history.lastEvent"), value: formatSpecialEventEntry(especiais.ultimo_evento) },
          ],
        }
      : null,
  ].filter(Boolean);

  return (
    <div className="mt-5" data-testid="driver-detail-career-dossier">
      {/* O rótulo se centraliza na largura TODA do bloco, então o botão de
          recordes sai do fluxo — em `justify-between` ele empurrava o título
          para a esquerda, e o eixo do cabeçalho não batia com o dos cards. */}
      <div className="relative flex min-h-[26px] items-center justify-center gap-3">
        <BlockLabel>{t("driverDetail.history.title")}</BlockLabel>
        {/* Desligado por padrão: a posição é a pergunta SEGUINTE. Ligada sempre,
            ela dobraria a altura de nove cards para responder algo que ninguém
            perguntou ainda — e o dossiê existe para contar a carreira, não para
            classificá-la.

            O botão também não espera mais existir recorde para aparecer: quem os
            monta é o clique, e era justamente esse mapa, viajando dentro do
            payload, que custava meio segundo em toda abertura de ficha. */}
        <span className="absolute right-0 top-1/2 flex -translate-y-1/2 items-center gap-2">
          {recordesLigados && entradasDeRecorde.length > 0 ? (
            <span className="text-[10px] text-text-muted" data-testid="driver-detail-records-scope">
              <span className="font-medium text-[color:var(--team)]">
                {t("driverDetail.history.rankScopeGrid", { count: totalGrid })}
              </span>
              {" · "}
              {t("driverDetail.history.rankScopeWorld", { count: totalMundo })}
            </span>
          ) : null}
          <button
            type="button"
            data-testid="driver-detail-records-toggle"
            aria-pressed={recordesLigados}
            aria-busy={buscandoRecordes}
            onClick={() => setRecordesLigados((atual) => !atual)}
            className={`shrink-0 rounded-full border px-3 py-1 text-[10px] font-semibold uppercase tracking-[0.08em] transition-colors ${
              buscandoRecordes ? "animate-pulse " : ""
            }${
              recordesLigados
                ? "border-[color:var(--team)] bg-[color:var(--team)]/15 text-[color:var(--team)]"
                : "border-white/[0.12] text-text-muted hover:border-white/25 hover:text-text-secondary"
            }`}
          >
            {t("driverDetail.history.recordsToggle")}
          </button>
        </span>
      </div>
      <div className="mt-2.5 grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {grupos.map((grupo) => (
          <div key={grupo.key} data-group={grupo.key} className="rounded-xl bg-[#0f1c2b] px-4 py-3.5">
            <span className="block text-xs font-semibold text-[color:var(--team)]">
              {grupo.title}
            </span>
            <div className="mt-1.5">
              {grupo.rows.map((row) => (
                <DossierDetailTooltip
                  key={row.label}
                  entradas={detalhes[row.detalhe]}
                  legendas={row.legendas}
                  onAbrirEquipe={onAbrirEquipe}
                >
                  <DataRow
                    label={row.label}
                    value={row.value}
                    hint={row.hint}
                    recorde={recordesLigados ? recordes[row.detalhe] : null}
                  />
                </DossierDetailTooltip>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─────────────────────────────── Rivais ───────────────────────────────

// A aba inteira eram três números e o valor cru de um enum: "6", "Histórico 4",
// "Recente 8", "Colisao". Nenhuma escala à vista para o 6 significar coisa
// alguma, e nenhum FATO — o motor de rivalidade sabe quanto um rival pesa, mas
// não conta nada que tenha acontecido, e uma aba sobre inimizade sem um episódio
// dentro é uma ficha de relacionamento sem relacionamento.
//
// O confronto direto era o que faltava, e já estava em `race_results`: os dois
// têm linha na mesma corrida sempre que dividiram o grid. "13 a 10 em 23
// corridas" é a única frase desta tela que se entende sem legenda, e é ela quem
// manda agora — os dois eixos do motor viraram rodapé.

// Os cinco níveis de `rivalry::intensity_level`, do cinza ao vermelho. A escala
// é de TEMPERATURA e não de qualidade: rivalidade intensa não é boa nem ruim,
// é quente — por isso a rampa não passa por verde em ponto nenhum.
const RIVAL_LEVEL_COLORS = {
  atrito_leve: "#8b949e",
  inicial: "#d29922",
  clara: "#db6d28",
  forte: "#f85149",
  intensa: "#ff3b30",
};

// O enum do motor vem em CamelCase e a chave de tradução é minúscula. O mapa
// explícito evita que um `toLowerCase()` silencioso invente chave para um tipo
// novo que ninguém traduziu.
const RIVAL_ORIGIN_KEYS = {
  Colisao: "colisao",
  Companheiros: "companheiros",
  Campeonato: "campeonato",
  Pista: "pista",
};

// A ABA TEM UMA COR SÓ.
//
// O card e a listrinha da linha se pintavam com a cor da equipe DO RIVAL, para
// que duas lajes vizinhas não parecessem a mesma pessoa. O preço era alto: a
// lista virava um mosaico de laranja, vermelho e azul que não falava de
// rivalidade nenhuma — falava de uniforme — e o card mudava de cor quando o
// sujeito trocava de equipe, como se a briga tivesse mudado junto.
//
// Aqui o assunto é o confronto, e o confronto já tem as suas duas cores (o verde
// e o vermelho do placar). Esta é a terceira e última: neutra, igual em todo
// rival, para que a única cor que varia na tela seja a que significa algo.
const RIVAL_ACCENT = "#8b949e";

// O EIXO FRIO PRECISA PESAR TANTO QUANTO O QUENTE.
//
// A memória usava emprestado o #46586d das medalhas — a cor do "resto do
// pelotão", escolhida justamente para sumir. Ao lado do vermelho do calor, o
// número e a barra dela viravam um borrão escuro num fundo escuro: dava para ver
// que havia algo escrito e não para ler o quê.
//
// Este azul-aço tem peso de leitura equivalente ao do vermelho e continua lendo
// como frio, que é o que o eixo conta — briga velha, não briga de agora. E é
// dessaturado o bastante para não se confundir com o azul de interação.
const RIVAL_MEMORY_COLOR = "#7ea8d4";

// UMA RIVALIDADE ABERTA POR VEZ, e o clique na linha ABRE em vez de navegar.
//
// A primeira versão mandava todo card para a ficha do rival: dava para ver o
// confronto do rival principal e de mais ninguém, porque tocar num secundário
// trocava de piloto e a aba inteira virava outra. Ir para a ficha continua sendo
// possível, mas por um alvo próprio dentro do card aberto — o clique grande, que
// é o barato, faz a coisa barata.
function RivalsSection({ detail, onSelectDriver }) {
  const { t } = useTranslation();
  const rivals = detail.rivais?.itens ?? [];
  const [abertoId, setAbertoId] = useState(null);

  if (!rivals.length) {
    return (
      <div
        className="rounded-xl bg-[#0f1c2b] px-5 py-6 text-center"
        data-testid="driver-detail-rivals-empty"
      >
        <strong className="block text-sm font-semibold text-text-secondary">
          {t("driverDetail.rivals.emptyTitle")}
        </strong>
        {/* O vazio ENSINA a mecânica em vez de só constatar a ausência: um
            estreante não tem rival nenhum, e "sem rivalidades consolidadas" é
            uma porta fechada onde cabia a regra que abre a porta. */}
        <p className="mx-auto mt-1.5 max-w-[46ch] text-xs leading-5 text-text-muted">
          {t("driverDetail.rivals.emptyBody")}
        </p>
      </div>
    );
  }

  const dono = primeiroNome(detail.nome);
  // O aberto sai da lista, e não de um `useEffect` que ressincroniza: trocar de
  // piloto troca a lista inteira, e um id que não está mais nela volta sozinho
  // para o primeiro rival sem passar por um estado intermediário errado.
  const aberto = rivals.some((rival) => rival.driver_id === abertoId)
    ? abertoId
    : rivals[0]?.driver_id;

  return (
    <section className="grid gap-2">
      {rivals.map((rival) =>
        rival.driver_id === aberto ? (
          <RivalHero
            key={rival.driver_id}
            rival={rival}
            dono={dono}
            onSelectDriver={onSelectDriver}
          />
        ) : (
          <RivalRow
            key={rival.driver_id}
            rival={rival}
            onAbrir={() => setAbertoId(rival.driver_id)}
          />
        ),
      )}
    </section>
  );
}

function RivalHero({ rival, dono, onSelectDriver }) {
  const { t } = useTranslation();

  const podeAbrirFicha = typeof onSelectDriver === "function";

  const dupla = rival.companheirismo;
  const foraDaCategoria = Boolean(rival.categoria_atual && !rival.mesma_categoria);
  // Só o box divide box: a categoria atual subiu para a legenda do banner.
  const temProsa = Boolean(dupla?.anos?.length);

  return (
    // `div` e não `button`: o card inteiro deixou de ser um alvo só quando passou
    // a conter alvos próprios, e botão dentro de botão não é markup válido.
    <div
      data-rival={rival.driver_id}
      data-testid="driver-detail-rival-hero"
      className="w-full rounded-xl border border-white/10 bg-[#0f1c2b] px-4 py-3.5"
    >
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <div className="min-w-0">
          <strong className="block truncate text-lg font-semibold leading-tight text-text-primary">
            {rival.nome}
          </strong>
          {rival.equipe_nome ? (
            <span className="mt-1 flex items-center gap-1.5 text-[11px] text-text-secondary">
              <TeamLogoMark teamName={rival.equipe_nome} size="xs" />
              <span className="truncate">{rival.equipe_nome}</span>
            </span>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <RivalLevelChip nivel={rival.nivel_chave} />
          {podeAbrirFicha ? (
            // O caminho para a ficha do rival virou um alvo pequeno e nomeado.
            // Como clique do card inteiro ele sequestrava o gesto mais óbvio da
            // tela — tocar num rival para ver a rivalidade dele.
            <button
              type="button"
              onClick={() => onSelectDriver(rival.driver_id)}
              data-testid="driver-detail-rival-open"
              className="flex items-center gap-1 rounded-full border border-white/10 px-2.5 py-1 text-[11px] font-medium leading-4 text-text-secondary transition-glass hover:border-white/25 hover:text-text-primary"
            >
              {t("driverDetail.rivals.openProfile")}
              <ArrowUpRight size={12} strokeWidth={2} aria-hidden="true" />
            </button>
          ) : null}
        </div>
      </div>

      {/* O MOTIVO, EM BANNER.
          É a única frase da tela que responde "por que estes dois?", e estava em
          corpo 11 no canto, do tamanho do nome da equipe. Centrado e grande,
          entre a identidade e o placar, ele vira a manchete que o card sempre
          teve e nunca mostrou: quem, POR QUÊ, e só então quantas. */}
      <p className="mt-3.5 text-center text-base leading-6 text-text-primary">
        {t(`driverDetail.rivals.origins.${RIVAL_ORIGIN_KEYS[rival.tipo] || "pista"}`)}
      </p>
      {foraDaCategoria ? (
        // O fim de um arco: rivalidade que morreu porque um dos dois subiu conta
        // história diferente da que só esfriou.
        //
        // Como faixa própria lá embaixo, esta única frase custava um fio, dois
        // respiros e uma altura inteira. Aqui ela é a segunda linha da legenda —
        // de onde a rivalidade veio, e onde o sujeito dela está agora — e não
        // paga nada além da própria linha.
        <p className="mt-1 text-center text-[11px] leading-4 text-text-muted">
          {t("driverDetail.rivals.awayCategory", { category: rival.categoria_atual })}
        </p>
      ) : null}

      <DuelScore rival={rival} dono={dono} />
      <DuelTimeline rival={rival} dono={dono} />

      {/* O RODAPÉ NUMA ALTURA SÓ, com os dois eixos de sobrecapa.
          Memória numa ponta, calor na outra, e no meio o que aconteceu na pista.
          A laje é larga demais para uma fileira centrada de cinco: com os eixos
          nos extremos, o vazio que sobrava vira moldura em vez de sobra, e os
          três fatos ficam de fato no centro — a coluna do meio é `auto` entre
          duas de `1fr`, então eles não deslizam quando um eixo passa de 9 para
          100 ou quando "últimas 10" não aparece.
          O div do meio existe mesmo quando não há fato nenhum: sem ele o grid
          promoveria o calor à casa central. */}
      <div className="mt-3 grid grid-cols-1 justify-items-center gap-3 border-t border-white/[0.06] pt-3 sm:grid-cols-[1fr_auto_1fr] sm:items-start sm:justify-items-stretch sm:gap-x-4">
        <EixoDaRivalidade
          chave="memoria"
          label={t("driverDetail.rivals.memory")}
          value={rival.intensidade_historica}
          color={RIVAL_MEMORY_COLOR}
        />
        <div>
          <DuelFacts rival={rival} dono={dono} />
        </div>
        <EixoDaRivalidade
          chave="calor"
          label={t("driverDetail.rivals.heat")}
          value={rival.atividade_recente}
          color={DUEL_LOSS_COLOR}
          alinhaDireita
        />
      </div>

      {temProsa ? (
        <div className="mx-auto mt-3 max-w-2xl border-t border-white/[0.06] pt-3 text-center text-xs leading-5 text-text-secondary">
          <TeammateSpell dupla={dupla} dono={dono} rival={rival} />
        </div>
      ) : null}
    </div>
  );
}

// OS DOIS EIXOS DO MOTOR, NO MOLDE DOS FATOS DO DUELO.
//
// Em pé — rótulo em cima, barra de largura total, número numa coluna de 44px na
// outra ponta — o número acabava a meia laje de distância do próprio rótulo, e a
// barra virava um fio de 600px com um toco no começo.
//
// Agora tem a mesma forma dos fatos vizinhos, versalete em cima e valor embaixo,
// para que os cinco assentem na mesma altura sem calço. A diferença é a barra,
// que fica ao lado do número e não em volta dele: ela desiste de fingir que é
// gráfico e vira o que sempre foi aqui, um adjetivo do número ao lado.
function EixoDaRivalidade({ chave, label, value, color, alinhaDireita = false }) {
  const normalizado = Number.isFinite(value) ? Math.max(0, Math.min(value, 100)) : 0;
  return (
    <div className={alinhaDireita ? "text-right" : undefined} data-rival-eixo={chave}>
      <span className="block text-[10px] uppercase leading-3 tracking-[0.1em] text-text-muted">
        {label}
      </span>
      <div className={`mt-1 flex items-center gap-2 ${alinhaDireita ? "justify-end" : ""}`}>
        <div className="h-1 w-20 shrink-0 overflow-hidden rounded-full bg-white/10">
          <div
            className="h-full rounded-full"
            style={{ width: `${normalizado}%`, backgroundColor: color }}
          />
        </div>
        <span className="font-mono text-sm leading-none tabular-nums" style={{ color }}>
          {normalizado}
        </span>
      </div>
    </div>
  );
}

// O PLACAR, que é a razão de a aba existir.
//
// Duas barras crescendo uma contra a outra a partir das pontas, e o vão do meio
// são as corridas que não decidiram nada (abandono de um dos dois). O vão é
// informação: uma rivalidade de 20 corridas com metade de vão é uma rivalidade
// que quase nunca chegou ao fim junta.
//
// O RIVAL FICA À ESQUERDA, e o dono da ficha à direita. A ordem é a do resto da
// aba: o nome do rival titula o card aberto, titula cada linha fechada e é o
// assunto da lista inteira — tê-lo à direita só aqui obrigava o leitor a
// descobrir, a cada card, que a pessoa nomeada em cima é a do número de lá.
// Quem manda no lado é o rival; o verde e o vermelho continuam mandando em quem
// é quem.
function DuelScore({ rival, dono }) {
  const { t } = useTranslation();
  const total = Math.max(rival.confrontos || 0, 0);

  if (!total) {
    return (
      <p className="mt-3 text-xs text-text-muted" data-testid="driver-detail-duel-empty">
        {t("driverDetail.rivals.noSharedRaces")}
      </p>
    );
  }

  const vitorias = Math.max(rival.vitorias || 0, 0);
  const derrotas = Math.max(rival.derrotas || 0, 0);
  const escala = Math.max(total, vitorias + derrotas);

  return (
    <div className="mt-3" data-testid="driver-detail-duel">
      <div className="flex items-end justify-between gap-3">
        <DuelSide nome={rival.nome ? primeiroNome(rival.nome) : ""} valor={derrotas} cor={DUEL_LOSS_COLOR} />
        {/* O denominador do placar. Em 11px na cor mais apagada da paleta ele
            estava espremido entre dois números de 24px e não se lia — e sem ele
            "28 a 13" não tem escala. */}
        <span className="pb-1 text-xs text-text-secondary">
          {t("driverDetail.rivals.sharedRaces", { count: total })}
        </span>
        <DuelSide nome={dono} valor={vitorias} cor={DUEL_WIN_COLOR} alinhaDireita />
      </div>
      <div className="mt-1.5 flex h-1.5 overflow-hidden rounded-full bg-white/[0.07]">
        <span
          data-duel-share="derrotas"
          className="h-full rounded-full"
          style={{ width: `${(derrotas / escala) * 100}%`, backgroundColor: DUEL_LOSS_COLOR }}
        />
        <span className="flex-1" />
        <span
          data-duel-share="vitorias"
          className="h-full rounded-full"
          style={{ width: `${(vitorias / escala) * 100}%`, backgroundColor: DUEL_WIN_COLOR }}
        />
      </div>
    </div>
  );
}

function DuelSide({ nome, valor, cor, alinhaDireita = false }) {
  return (
    <div className={`min-w-0 ${alinhaDireita ? "text-right" : ""}`}>
      <span className="block truncate text-[10px] uppercase tracking-[0.1em] text-text-muted">
        {nome}
      </span>
      <strong className="block font-mono text-2xl leading-none" style={{ color: cor }}>
        {valor}
      </strong>
    </div>
  );
}

// O QUE SEPARA TRÊS RIVAIS COM O MESMO PLACAR.
//
// Num grid fechado, quem corre a temporada inteira divide o mesmo número de
// corridas com todo mundo — e um piloto de meio de pelotão perde para os três de
// cima quase na mesma proporção. O placar de carreira, que é o número mais
// importante da tela, é justamente o que NÃO distingue essas pessoas: 12–33,
// 12–32, 12–32.
//
// Estes três fatos distinguem. O recorte recente separa quem está subindo de
// quem está caindo; a sequência nomeia o que a faixa desenha; e a classificação
// é outro esporte — dá para perder o domingo a vida inteira e ganhar o sábado.
const DUEL_RECENT_RACES = 10;
// Abaixo disto o recorte recente é quase a carreira inteira, e mostrar os dois
// seria escrever o mesmo número duas vezes com nomes diferentes.
const DUEL_RECENT_MIN_HISTORY = 14;
// Duas seguidas é coincidência. A partir de três há um período.
const DUEL_STREAK_MIN = 3;

function DuelFacts({ rival, dono }) {
  const { t } = useTranslation();
  const encontros = Array.isArray(rival.encontros) ? rival.encontros : [];
  const rivalNome = primeiroNome(rival.nome);

  const fatos = [];

  if (encontros.length >= DUEL_RECENT_MIN_HISTORY) {
    const recorte = encontros.slice(-DUEL_RECENT_RACES);
    fatos.push({
      chave: "recente",
      label: t("driverDetail.rivals.lastRaces", { count: recorte.length }),
      conteudo: (
        <DuelPair
          rival={recorte.filter((corrida) => corrida.vencedor === "rival").length}
          dono={recorte.filter((corrida) => corrida.vencedor === "piloto").length}
        />
      ),
    });
  }

  const sequencia = sequenciaAtual(encontros);
  if (sequencia && sequencia.total >= DUEL_STREAK_MIN) {
    fatos.push({
      chave: "sequencia",
      label: t("driverDetail.rivals.streak"),
      conteudo: (
        <span
          className="font-mono text-sm leading-none"
          style={{ color: sequencia.vencedor === "piloto" ? DUEL_WIN_COLOR : DUEL_LOSS_COLOR }}
        >
          {t("driverDetail.rivals.streakValue", {
            count: sequencia.total,
            name: sequencia.vencedor === "piloto" ? dono : rivalNome,
          })}
        </span>
      ),
    });
  }

  const quali = (rival.vitorias_quali || 0) + (rival.derrotas_quali || 0);
  if (quali > 0) {
    fatos.push({
      chave: "quali",
      label: t("driverDetail.rivals.qualifying"),
      conteudo: <DuelPair rival={rival.derrotas_quali} dono={rival.vitorias_quali} />,
    });
  }

  if (Number.isFinite(rival.gap_medio)) {
    // O sinal aqui é o inverso do resto da tela — gap POSITIVO é o rival na
    // frente, porque a conta é "quanto tempo atrás dele". Por isso a cor
    // também inverte, e não é engano.
    const atras = rival.gap_medio > 0;
    fatos.push({
      chave: "gap",
      label: t("driverDetail.rivals.averageGap"),
      conteudo: (
        <span
          className="font-mono text-sm leading-none"
          style={{ color: atras ? DUEL_LOSS_COLOR : DUEL_WIN_COLOR }}
        >
          {t("driverDetail.rivals.gapValue", {
            value: Math.abs(rival.gap_medio).toFixed(1),
          })}
        </span>
      ),
    });
  }

  if (!fatos.length) return null;

  return (
    // Sem fio e sem margem própria: quem posiciona esta faixa agora é o rodapé
    // do RivalHero, que a carrega junto com os eixos.
    <div
      className="flex flex-wrap items-start justify-center gap-x-6 gap-y-2"
      data-testid="driver-detail-duel-facts"
    >
      {fatos.map((fato) => (
        <div key={fato.chave} data-duel-fact={fato.chave} className="min-w-0">
          <span className="block text-[10px] uppercase leading-3 tracking-[0.1em] text-text-muted">
            {fato.label}
          </span>
          <span className="mt-1 block">{fato.conteudo}</span>
        </div>
      ))}
    </div>
  );
}

// O par vermelho–verde na mesma ordem do placar grande: o rival sempre à
// esquerda, o dono da ficha sempre à direita. Trocar a ordem entre um fato e
// outro faria o leitor conferir de quem é cada número toda vez.
function DuelPair({ rival, dono }) {
  return (
    <span className="font-mono text-sm leading-none">
      <span style={{ color: DUEL_LOSS_COLOR }}>{rival}</span>
      <span className="text-text-muted">–</span>
      <span style={{ color: DUEL_WIN_COLOR }}>{dono}</span>
    </span>
  );
}

// DIVIDIRAM BOX: a única comparação da ficha sem o carro no meio.
//
// Em todo o resto, "ele terminou à frente" pode ser o pacote falando — dois
// pilotos de equipes diferentes nunca correram o mesmo carro. No box, correram.
// É por isso que esta linha vale mais que o placar geral, e por isso ela é a
// única com destaque de cor no bloco de prosa.
function TeammateSpell({ dupla, dono, rival }) {
  const { t } = useTranslation();
  if (!dupla?.anos?.length) return null;

  const decidiu = dupla.vitorias + dupla.derrotas > 0;
  return (
    <span data-testid="driver-detail-duel-teammate" className="text-text-secondary">
      <span className="font-medium" style={{ color: RIVAL_ACCENT }}>
        {t("driverDetail.rivals.teammateSpell", {
          team: dupla.equipe || t("driverDetail.rivals.teammateNoTeam"),
          years: listaDeAnos(dupla.anos),
        })}
      </span>
      {decidiu ? (
        <>
          {" "}
          {t("driverDetail.rivals.teammateScore", {
            driver: dono,
            wins: dupla.vitorias,
            rival: primeiroNome(rival.nome),
            losses: dupla.derrotas,
          })}
        </>
      ) : null}
    </span>
  );
}

function RivalLevelChip({ nivel }) {
  const { t } = useTranslation();
  const chave = RIVAL_LEVEL_COLORS[nivel] ? nivel : "atrito_leve";
  const cor = RIVAL_LEVEL_COLORS[chave];

  return (
    // O número 0–100 sai da tela: sem a escala à vista ele nunca disse nada, e
    // um "6" vermelho em corpo grande lia como nota baixa em vez de briga fria.
    <span
      data-rival-level={chave}
      className="shrink-0 rounded-full border px-2.5 py-1 text-[11px] font-semibold leading-4"
      style={{
        color: cor,
        borderColor: `color-mix(in srgb, ${cor} 40%, transparent)`,
        backgroundColor: `color-mix(in srgb, ${cor} 12%, transparent)`,
      }}
    >
      {t(`driverDetail.rivals.levels.${chave}`)}
    </span>
  );
}

// A rivalidade FECHADA: nome, nível, a faixa em miniatura e o placar. Clicar
// abre esta e fecha a que estava aberta — a lista nunca reordena, então o card
// que abre nasce exatamente onde estava a linha que foi tocada.
function RivalRow({ rival, onAbrir }) {
  const { t } = useTranslation();
  const nivel = RIVAL_LEVEL_COLORS[rival.nivel_chave] ? rival.nivel_chave : "atrito_leve";

  return (
    <button
      type="button"
      onClick={onAbrir}
      aria-expanded={false}
      data-rival={rival.driver_id}
      data-testid="driver-detail-rival-row"
      className="flex w-full items-center gap-3 rounded-xl border border-white/[0.06] bg-[#0f1c2b] px-4 py-2.5 text-left transition-glass hover:border-white/20 hover:bg-[#13243a]"
    >
      <span
        aria-hidden="true"
        className="h-8 w-[3px] shrink-0 rounded-full"
        style={{ backgroundColor: RIVAL_ACCENT }}
      />
      <div className="min-w-0 flex-1">
        <strong className="block truncate text-[13px] font-semibold leading-5 text-text-primary">
          {rival.nome}
        </strong>
        <span className="block truncate text-[11px] leading-4 text-text-muted">
          {t(`driverDetail.rivals.levels.${nivel}`)}
          {rival.categoria_atual && !rival.mesma_categoria ? ` · ${rival.categoria_atual}` : ""}
        </span>
      </div>
      {rival.confrontos ? (
        <>
          {/* A MESMA faixa do card principal, em escala de linha. O rival
              secundário não vira um resumo de outra natureza — vira o mesmo
              gráfico menor, e os dois se comparam de relance. */}
          <MiniTimeline encontros={rival.encontros} />
          <span className="shrink-0 text-right">
            {/* O SALDO, e não o par. "12–32" e "12–33" obrigam a subtrair para
                comparar duas linhas; "−20" e "−21" já chegam comparáveis. O par
                completo continua a um clique, dentro do card aberto. */}
            <span
              className="font-mono text-sm leading-none"
              data-duel-saldo={rival.vitorias - rival.derrotas}
              style={{ color: corDoSaldo(rival.vitorias - rival.derrotas) }}
            >
              {formataSaldo(rival.vitorias - rival.derrotas)}
            </span>
            <span className="mt-0.5 block text-[10px] leading-3 text-text-muted">
              {t("driverDetail.rivals.sharedRaces", { count: rival.confrontos })}
            </span>
          </span>
        </>
      ) : null}
      {/* A seta é o que diz que a linha ABRE em vez de levar embora. Sem ela o
          gesto é o mesmo do card anterior, que navegava. */}
      <ChevronDown
        size={16}
        strokeWidth={1.8}
        aria-hidden="true"
        className="shrink-0 text-text-muted"
      />
    </button>
  );
}

// ────────────────────────────── Primitivos ──────────────────────────────

// Aviso de lesão ativa. Cobre a ficha inteira (que fica desfocada atrás) porque
// é a única informação da tela que muda o que o jogador pode fazer com o piloto
// — deixá-la como mais um card seria enterrá-la entre trinta números.
function InjuryOverlay({ injury, onConfirm }) {
  const { t } = useTranslation();
  // `injury.tipo` é a CHAVE do fio que o Rust manda (`InjuryType::chave`:
  // "light"/"moderate"/"severe"/"critical"), não mais a grafia do banco em
  // português. Chave desconhecida cai no traço em vez de imprimir a chave crua no
  // meio de uma frase traduzida.
  const gravidade = injury?.tipo
    ? t(`driverDetail.injury.severityValue.${injury.tipo}`, { defaultValue: "-" })
    : "-";
  return (
    <div
      className="absolute inset-0 z-30 grid place-items-center bg-[#05070d]/90 px-6"
      role="dialog"
      aria-modal="true"
      aria-label={t("driverDetail.injury.ariaLabel")}
      data-testid="driver-detail-injury"
    >
      <div className="w-full max-w-[420px] rounded-2xl border border-status-red/30 bg-[#0b1018] p-5 shadow-[0_30px_90px_rgba(0,0,0,0.62)]">
        <span className="block text-xs font-semibold text-status-red">
          {t("driverDetail.injury.title")}
        </span>
        <strong className="mt-1 block text-2xl font-semibold text-text-primary">
          {injury?.nome || gravidade}
        </strong>
        <div className="mt-5">
          <DataRow label={t("driverDetail.injury.occurred")} value={formatInjuryOccurrence(injury)} />
          <DataRow
            label={t("driverDetail.injury.recoveryLabel")}
            value={formatInjuryRecovery(injury)}
          />
          <DataRow
            label={t("driverDetail.injury.severity")}
            value={gravidade}
            valueClassName="text-status-red"
          />
        </div>
        <button
          type="button"
          onClick={onConfirm}
          className="mt-5 h-10 w-full rounded-xl border border-status-red/35 bg-status-red/15 text-xs font-semibold text-[#ffd7d4] transition-glass hover:bg-status-red/25"
        >
          OK
        </button>
      </div>
    </div>
  );
}

// Navegação entre pilotos. As setas moram numa coluna presa à calha à direita do
// painel, no meio da altura — o ponteiro pode ficar parado e só clicar, em vez
// de perseguir um alvo que muda de lugar a cada ficha.
//
// São menores que as do dossiê de equipe (56px contra 92px) porque a ficha
// também é: um botão do tamanho do cabeçalho do piloto disputaria atenção com
// ele.
function DriverStepButton({ label, direction, driverId, onSelectDriver, onStep }) {
  const Chevron = direction === "up" ? ChevronUp : ChevronDown;
  return (
    <button
      type="button"
      aria-label={label}
      disabled={!driverId}
      onClick={() => {
        if (!driverId) return;
        onStep?.(direction);
        onSelectDriver(driverId);
      }}
      data-testid={`driver-detail-step-${direction}`}
      className={`grid h-16 w-16 place-items-center rounded-2xl border backdrop-blur-sm transition-glass max-lg:h-12 max-lg:w-12 ${
        driverId
          ? "border-white/15 bg-[#0d1727]/90 text-text-secondary hover:border-white/30 hover:bg-[#14233a] hover:text-text-primary"
          : "cursor-not-allowed border-white/[0.06] bg-[#0b111a]/70 text-[#4a525d]"
      }`}
    >
      <Chevron size={24} strokeWidth={1.6} aria-hidden="true" />
    </button>
  );
}

const SUMMARY_TONES = {
  danger: { card: "border-status-red/25 bg-status-red/10", label: "text-status-red" },
  warning: { card: "border-status-yellow/25 bg-status-yellow/10", label: "text-status-yellow" },
  info: { card: "border-accent-primary/20 bg-accent-primary/[0.08]", label: "text-accent-primary" },
  success: { card: "border-status-green/25 bg-status-green/10", label: "text-status-green" },
};

export default DriverDetailModalV2;
