import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import {
  Award,
  BarChart3,
  Briefcase,
  CalendarDays,
  ChevronDown,
  ChevronUp,
  Crown,
  Fingerprint,
  Layers,
  Medal,
  Swords,
  TrendingUp,
  Trophy,
  X,
} from "lucide-react";

import goldTrophy from "../../../assets/utilities/trophies/ouro.png";
import TeamLogoMark from "../TeamLogoMark";
import FlagIcon from "../../ui/FlagIcon";
import Tooltip from "../../ui/Tooltip";
import {
  buildTeamHistoryDossier,
  operationHealthTone,
  orderTeamsForHistoryNavigation,
} from "../TeamHistoryDrawer";
import i18n from "../../../i18n/index.js";
import { getCategoryColor } from "../../../utils/categoryColors";
import { getVividTeamColor } from "../../../utils/teamColors";
import { formatMoney, formatMoneyCompact } from "../../../utils/formatters";
import { pisoDeAbertura } from "../../ui/aberturaDePainel.js";
import {
  EVOLUTION_VIEW_RUN,
  EVOLUTION_VIEW_SEASONS,
  RUN_MODE_POINTS,
  RUN_MODE_POSITION,
  guardarModoEvolucao,
  guardarVistaEvolucao,
  lerModoEvolucao,
  lerVistaEvolucao,
} from "./evolutionPreferences.js";

// Dossiê de equipe v2.
//
// Mesmos dados do v1 (get_team_history_dossier, mesmo `buildTeamHistoryDossier`) —
// o que muda é a composição. O v1 era um painel de borda de 720px com abas em
// pílula e uma lista label/valor; aqui a tela abre CENTRALIZADA e larga, com:
//
//   • cabeçalho-herói com os números-âncora sempre visíveis (no v1 eles só
//     apareciam se você estivesse na seção certa);
//   • seções numa coluna lateral, liberando a largura toda para o conteúdo;
//   • records como cards com barra de posição e média do grupo — o rank deixa
//     de ser um número entre parênteses e vira a informação com mais peso;
//   • trajetória por temporada e marcos ancorados no MESMO eixo de anos.
//
// Para voltar ao v1 basta mudar TEAM_HISTORY_VERSION em ../history/index.js.
// Nenhum arquivo do v1 é editado por este redesenho.
// Ícones vêm do lucide-react: traço de 1.5px numa grade de 24, igual para os
// onze. Os SVGs desenhados à mão que estavam aqui variavam de espessura entre si
// e ficavam sujos a 12px, que é o tamanho em que a maioria aparece.
// Os IDs divergem dos rótulos de propósito. `sport` é a aba que hoje se chama
// "Identidade" (o retrato esportivo virou o retrato da equipe) e `identity` é a
// que se chama "Rival". Renomear os ids arrastaria o v1, o estado persistido de
// aba e os testes por uma troca que é só de vocabulário — o rótulo mora no i18n,
// que é onde ele deve morar.
const TEAM_HISTORY_SECTIONS = [
  { id: "records", Icon: Trophy },
  { id: "sport", Icon: Fingerprint },
  { id: "identity", Icon: Swords },
  { id: "management", Icon: Briefcase },
  { id: "categories", Icon: Layers },
];

// Ícone por métrica, escolhido pelo `id` do record — nunca pelo rótulo, que é
// texto traduzido. Métrica desconhecida não desenha nada: um ícone genérico
// seria pior que nenhum.
const METRIC_ICONS = {
  titles: Award,
  wins: Crown,
  podiums: Medal,
  podium_rate: BarChart3,
  win_rate: TrendingUp,
  seasons: CalendarDays,
};

export function TeamHistoryDrawerV2({
  careerId,
  team,
  teams,
  playerTeam,
  activeCategory,
  activeTab,
  onTabChange,
  onSelectTeam,
  // Abre a tabela de recordes de equipes no grupo desta ficha. Sem o callback
  // (dossiê aberto fora do Dashboard, como no overlay de pré-temporada) os cards
  // continuam sendo cards, em vez de virarem botões que não levam a lugar
  // nenhum.
  onOpenRecordsRanking,
  onClose,
}) {
  const { t } = useTranslation();
  // Sentido do último passo entre equipes, só para escolher de que lado a ficha
  // nova entra. Fica aqui em cima porque quem clica é a seta e quem anima é o
  // conteúdo, dois pontos distantes da árvore.
  const [stepDirection, setStepDirection] = useState("down");
  // Só a primeira carga é uma ABERTURA. Passar de equipe para equipe com o
  // dossiê já na tela é navegação, e navegação não espera.
  const primeiraCargaRef = useRef(true);
  // Se já existe dossiê desenhado. Separado de `primeiraCargaRef` porque as duas
  // perguntas são diferentes: aquela decide o compasso de abertura e é gasta na
  // primeira ENTREGA; esta decide se o miolo pode ficar vazio.
  const temDossieRef = useRef(false);
  // A ABERTURA esconde o dossiê inteiro, e não só o miolo. O drawer monta a
  // moldura na hora — a equipe já vem no `prop`, então o cabeçalho e as abas
  // desenham antes do `invoke` —, e por isso ele abria de estalo enquanto a
  // ficha do piloto, que depende do payload para tudo, tinha a sequência.
  const [abrindo, setAbrindo] = useState(true);
  const [historyDossier, setHistoryDossier] = useState(null);
  const [historyStatus, setHistoryStatus] = useState("loading");
  const [historyError, setHistoryError] = useState("");
  const dossier = buildTeamHistoryDossier(
    team,
    teams,
    playerTeam,
    activeCategory,
    historyDossier,
    historyStatus,
    historyError,
  );
  const orderedTeams = orderTeamsForHistoryNavigation(teams);
  const currentTeamIndex = orderedTeams.findIndex((entry) => entry.id === team?.id);
  const previousTeam = currentTeamIndex > 0 ? orderedTeams[currentTeamIndex - 1] : null;
  const nextTeam = currentTeamIndex >= 0 && currentTeamIndex < orderedTeams.length - 1
    ? orderedTeams[currentTeamIndex + 1]
    : null;

  useEffect(() => {
    let mounted = true;
    if (!careerId || !team?.id) {
      setHistoryStatus("error");
      setHistoryError(i18n.t("myTeamTab.history.unavailable"));
      return undefined;
    }

    setHistoryError("");
    // Esvaziar o dossiê só na ABERTURA — mesma regra da ficha do piloto. Trocar
    // de equipe com o painel na tela é navegação: sem isto, `historyStatus` volta
    // a "loading", o payload some, e as seções caem no aviso de carga e nos
    // números-placeholder. O painel não fecha, mas o miolo inteiro pisca — o que
    // se vê é uma tela fechando e abrindo.
    if (!temDossieRef.current) {
      setHistoryStatus("loading");
      setHistoryDossier(null);
    }
    const piso = pisoDeAbertura(primeiraCargaRef.current);

    Promise.all([
      invoke("get_team_history_dossier", {
        careerId,
        teamId: team.id,
        category: activeCategory ?? playerTeam?.categoria ?? team?.categoria ?? "",
      }),
      piso,
    ])
      .then(([payload]) => {
        if (!mounted) return;
        // A abertura só é gasta por quem CHEGA a entregar — ver o mesmo ponto na
        // ficha do piloto. Em dev o StrictMode monta, desmonta e remonta o
        // efeito; consumir a bandeira lá em cima entregava o piso à passagem
        // descartada e o dossiê abria de estalo, imune ao valor de ABERTURA_MS.
        primeiraCargaRef.current = false;
        temDossieRef.current = true;
        setHistoryDossier(payload);
        setHistoryStatus("ready");
        setAbrindo(false);
      })
      .catch((invokeError) => {
        if (!mounted) return;
        primeiraCargaRef.current = false;
        setHistoryError(typeof invokeError === "string" ? invokeError : i18n.t("myTeamTab.history.loadError"));
        setHistoryStatus("error");
        setAbrindo(false);
      });

    return () => {
      mounted = false;
    };
  }, [activeCategory, careerId, team?.id, team?.categoria, playerTeam?.categoria]);

  useEffect(() => {
    function handleKeyDown(event) {
      if (event.key === "Escape") onClose?.();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const layer = (
    <div
      className="fixed inset-0 z-[90] flex items-center justify-center px-[120px] max-lg:px-[84px]"
      data-testid="team-history-layer"
      aria-hidden={false}
    >
      <button
        type="button"
        aria-label={t("myTeamTab.history.closeAria")}
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/70 backdrop-blur-[3px]"
      />

      {/* O wrapper existe só para dar às setas uma âncora que NÃO seja o conteúdo
          do dossiê: ele tem a largura do painel, então a coluna de setas fica
          sempre na mesma calha à direita, no meio da altura, independente do que
          a aba de dentro esteja desenhando. O padding do layer reserva o espaço
          dessa calha, então elas nunca cobrem o painel. */}
      <div className="relative z-10 flex w-[min(100%,1180px)] justify-center">
        <div
          data-testid="team-history-step-rail"
          className="animate-team-rail-out absolute left-full top-1/2 ml-3 flex -translate-y-1/2 flex-col gap-2.5 max-lg:gap-2"
        >
          <TeamStepButton
            label={t("myTeamTab.history.nav.previous")}
            direction="up"
            team={previousTeam}
            onSelectTeam={onSelectTeam}
            onStep={setStepDirection}
          />
          <TeamStepButton
            label={t("myTeamTab.history.nav.next")}
            direction="down"
            team={nextTeam}
            onSelectTeam={onSelectTeam}
            onStep={setStepDirection}
          />
        </div>
        <aside
          role="dialog"
        aria-modal="true"
        aria-labelledby="team-history-title"
        data-testid="team-history-drawer"
        // O teto em pixels é o que mantém as abas do mesmo tamanho. Só com `vh`,
        // a ficha crescia até quase a tela inteira na aba mais longa e voltava a
        // uns 700px nas curtas — trocar de aba mexia a moldura, o cabeçalho subia
        // e descia, e o painel dava a impressão de ser outra tela a cada clique.
        // 780px é onde Records e Rival já param sozinhas; a partir daí a aba longa
        // rola por dentro em vez de esticar o quadro.
        //
        // O `vh` continua como piso de segurança para janela baixa, onde 780px não
        // caberiam.
        className="animate-scale-in relative flex max-h-[min(88vh,780px)] w-full flex-col overflow-hidden rounded-[28px] border border-white/15 bg-[#07101d] shadow-[0_30px_90px_rgba(0,0,0,0.72)]"
        style={{
          // `--team` é a cor da equipe JÁ LEGÍVEL sobre o fundo escuro do dossiê, e não a
          // cor crua. Toda a identidade visual do drawer sai desta variável — a faixa do
          // topo, a curva de campeonato, as barras, os anos, os chips de forma —, então
          // normalizar aqui conserta todas de uma vez, no único ponto onde a cor entra.
          //
          // Sem isto, equipe de cor muito escura (Thunderline Academy é o caso que o
          // próprio `getTeamGlow` cita) pinta gráfico e barras quase da cor do fundo, e a
          // tela fica ilegível.
          //
          // Só CLAREAR não bastava, e o drawer viveu um tempo assim: o #2f3542 da
          // Thunderline clareado vira cinza claro — legível, e igual à grade, aos
          // rótulos e a todo o resto do cromo, que também é cinza. A linha de dados
          // sumia no meio da interface mesmo estando visível. `getVividTeamColor`
          // sobe a saturação junto, então o azul que a equipe TEM reaparece; para
          // quem já era vivo e contrastado (o ciano da Track Day Heroes) é no-op.
          "--team": getVividTeamColor(dossier.color),
          backgroundImage:
            "radial-gradient(circle at 8% 0%, color-mix(in srgb, var(--team) 14%, transparent), transparent 26rem), linear-gradient(180deg, rgba(12,22,38,0.98), rgba(5,11,20,0.995))",
        }}
      >
        <div className="h-1 shrink-0 bg-[color:var(--team)]" />

        {abrindo ? (
          <div
            className="flex min-h-[260px] flex-1 flex-col items-center justify-center gap-3"
            data-testid="team-history-loading"
          >
            <span className="animate-pulse text-4xl">🏁</span>
            <p className="text-sm text-text-secondary">{t("myTeamTab.history.loading")}</p>
          </div>
        ) : (
          <>
        {/* `key` na equipe é o gatilho da animação: ao trocar de ficha o React
            monta um bloco novo e a CSS toca do zero. A moldura do drawer fica de
            fora do wrapper de propósito — ela não pisca, só o conteúdo desliza. */}
        <div
          key={team?.id ?? "sem-equipe"}
          data-step-direction={stepDirection}
          className={`flex min-h-0 flex-1 flex-col ${
            stepDirection === "up" ? "animate-team-step-up" : "animate-team-step-down"
          }`}
        >
        <TeamHistoryHero dossier={dossier} onClose={onClose} />

        <div className="grid min-h-0 flex-1 grid-cols-[184px_minmax(0,1fr)] max-lg:grid-cols-1">
          <nav
            role="tablist"
            aria-label={t("myTeamTab.history.tablistAria")}
            className="border-r border-white/10 p-3 max-lg:flex max-lg:gap-2 max-lg:overflow-x-auto max-lg:border-b max-lg:border-r-0"
          >
            {TEAM_HISTORY_SECTIONS.map((section) => (
              <button
                key={section.id}
                type="button"
                role="tab"
                aria-selected={activeTab === section.id}
                onClick={() => onTabChange(section.id)}
                className={`flex w-full shrink-0 items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-xs font-semibold transition-glass ${
                  activeTab === section.id
                    ? "bg-[color-mix(in_srgb,var(--team)_22%,transparent)] text-text-primary"
                    : "text-text-secondary hover:bg-white/[0.05] hover:text-text-primary"
                }`}
              >
                <section.Icon size={18} strokeWidth={1.5} aria-hidden="true" className="shrink-0" />
                <span className="truncate">{t(`myTeamTab.history.tabs.${section.id}`)}</span>
                {section.id === "categories" && dossier.categoryPath?.length > 0 ? (
                  <span className="ml-auto rounded-full bg-white/10 px-2 py-0.5 font-mono text-[10px] text-text-secondary">
                    {dossier.categoryPath.length}
                  </span>
                ) : null}
              </button>
            ))}
          </nav>

          <div className="min-h-0 overflow-y-auto px-6 py-5">
            {activeTab === "records" ? (
              <RecordsSection
                dossier={dossier}
                // A categoria vai junto do clique porque é ela que define o
                // RECORTE: o card diz "11º de 19" dentro do grupo desta ficha, e
                // a tabela tem de abrir no mesmo grupo para o 11º continuar
                // sendo o 11º. Vem das props do drawer, e não do dossiê montado,
                // porque é o id cru — o dossiê só carrega os rótulos.
                onOpenRecord={
                  onOpenRecordsRanking
                    ? (metric) =>
                        onOpenRecordsRanking({
                          metric,
                          category: activeCategory ?? team?.categoria ?? "",
                          // A classe do carro. Numa categoria multiclasse — a
                          // Production tem três campeonatos dentro dela — é ela
                          // que diz em qual a equipe corre; sem ela a tabela
                          // abriria numa das três, escolhida por sorteio.
                          teamClass: team?.classe ?? "",
                          teamId: team?.id ?? null,
                        })
                    : null
                }
              />
            ) : null}
            {activeTab === "sport" ? <SportSection dossier={dossier} /> : null}
            {activeTab === "identity" ? <IdentitySection dossier={dossier} /> : null}
            {activeTab === "management" ? <ManagementSection dossier={dossier} /> : null}
            {activeTab === "categories" ? <CategoriesSection dossier={dossier} /> : null}
          </div>
        </div>

        <footer className="flex shrink-0 items-center gap-3 border-t border-white/10 px-6 py-2.5 text-[11px] text-text-secondary">
          <span className="truncate">
            {t("myTeamTab.history.records.compareIntro")}
            <strong className="text-text-primary">{dossier.recordScope}</strong>
            {t("myTeamTab.history.records.compareOutro")}
          </span>
        </footer>
        </div>
          </>
        )}
        </aside>
      </div>
    </div>
  );

  return createPortal(layer, document.body);
}

// Cabeçalho-herói: identidade à esquerda, números-âncora à direita. Os âncoras
// são os três primeiros records (títulos, vitórias, pódios) mais as temporadas
// disputadas — os quatro que o jogador procura antes de qualquer outra coisa.
function TeamHistoryHero({ dossier, onClose }) {
  const { t } = useTranslation();
  const anchors = [
    ...dossier.records.slice(0, 3).map((record) => ({
      key: record.id || record.label,
      icon: record.id,
      label: record.label,
      value: record.value,
      rankPosition: record.rankPosition,
    })),
    {
      key: "seasons",
      icon: "seasons",
      label: t("myTeamTab.history.sport.seasonsPlayed"),
      value: dossier.sport.seasons,
      rankPosition: 0,
    },
  ];
  // O âncora em destaque é a MELHOR colocação da equipe entre os três records —
  // a moldura responde "no que essa equipe é boa?" antes de você ler os números.
  // Sem rank (histórico ainda carregando) ninguém acende.
  const bestRank = anchors
    .filter((anchor) => anchor.rankPosition > 0)
    .reduce((best, anchor) => (best === null || anchor.rankPosition < best.rankPosition ? anchor : best), null);

  return (
    <header className="flex shrink-0 items-center gap-4 border-b border-white/10 px-6 py-4">
      <TeamLogoMark teamName={dossier.name} color={dossier.color} size="lg" testId="team-history-logo" />
      <div className="min-w-0 flex-1">
        <h2 id="team-history-title" className="min-w-0 truncate text-2xl font-semibold leading-none tracking-[-0.03em] text-text-primary">
          {dossier.name}
        </h2>
        <div className="mt-2.5 flex flex-wrap gap-2">
          <HeroBadge>{dossier.state}</HeroBadge>
          {dossier.founded ? <HeroBadge>{t("myTeamTab.history.foundedIn", { year: dossier.founded })}</HeroBadge> : null}
          <HeroBadge>{dossier.currentCategory}</HeroBadge>
        </div>
      </div>
      <div className="flex shrink-0 gap-2 max-md:hidden">
        {anchors.map((anchor) => {
          const highlighted = bestRank !== null && bestRank.key === anchor.key;
          // O card é discreto de propósito: fundo escuro sem borda, para não
          // virar quatro caixas competindo com o nome da equipe. O que dá vida
          // ao bloco é o TEXTO — rótulo e ícone em text-secondary, não no
          // text-muted que os deixava ilegíveis.
          return (
            <Tooltip
              key={anchor.key}
              texto={highlighted ? t("myTeamTab.history.records.bestRankAria", { rank: anchor.rankPosition }) : undefined}
            >
              <div
                data-anchor={anchor.key}
                data-highlighted={highlighted ? "true" : undefined}
                className={`min-w-[86px] rounded-xl border px-3 py-2 text-center ${
                  highlighted
                    ? "border-[color-mix(in_srgb,var(--team)_55%,transparent)] bg-[color-mix(in_srgb,var(--team)_12%,#0f1c2b)]"
                    : "border-transparent bg-[#0f1c2b]"
                }`}
              >
                <span className="flex items-center justify-center gap-1.5 text-[10px] text-text-secondary">
                  <MetricIcon name={anchor.icon} />
                  <span className="truncate">{anchor.label}</span>
                </span>
                <AnchorValue value={anchor.value} />
              </div>
            </Tooltip>
          );
        })}
      </div>
      <button
        type="button"
        onClick={onClose}
        aria-label={t("myTeamTab.history.close")}
        className="grid h-8 w-8 shrink-0 place-items-center rounded-lg border border-white/15 bg-[#0d1727] text-text-secondary transition-glass hover:bg-[#14233a] hover:text-text-primary"
      >
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </header>
  );
}

// Número grande, unidade pequena: o backend manda "7 Temporadas" numa string só,
// e jogar tudo em fonte de número faz a palavra competir com o valor. Quebra no
// primeiro trecho não-numérico; sem número à frente, imprime como veio.
function AnchorValue({ value }) {
  const match = String(value ?? "").match(/^(\d+[.,]?\d*)\s*(.*)$/);
  if (!match) {
    return <strong className="mt-1 block truncate text-sm leading-tight text-text-primary">{value}</strong>;
  }
  return (
    <strong className="mt-1 flex items-baseline justify-center gap-1 leading-none">
      <span className="font-mono text-lg text-text-primary">{match[1]}</span>
      {match[2] ? <span className="truncate text-[10px] font-normal text-text-secondary">{match[2]}</span> : null}
    </strong>
  );
}

function HeroBadge({ children }) {
  return (
    <span className="rounded-full border border-white/15 bg-[#08111f] px-2.5 py-1 text-[11px] text-text-secondary">{children}</span>
  );
}

function RecordsSection({ dossier, onOpenRecord = null }) {
  // O ano sob o cursor, compartilhado pela faixa de top 5 e pela régua de
  // títulos. Os dois desenham o MESMO eixo de anos em escalas diferentes, e sem
  // o elo era preciso contar coluna com o dedo para descobrir qual ano da régua
  // corresponde àquele pico — que é justamente a pergunta que as duas juntas
  // deveriam responder de graça.
  //
  // O estado mora aqui porque este é o pai comum mais próximo; guardado dentro
  // de qualquer um dos dois, o outro não teria como saber.
  const [anoAceso, setAnoAceso] = useState(null);

  return (
    <section>
      {dossier.historyStatus !== "ready" ? <HistoryStateMessage dossier={dossier} /> : null}

      {/* Grid assimétrico: as três CONTAGENS na primeira linha, as TAXAS numa
          segunda linha de cards mais largos. Não é capricho — separa o que é
          acumulado numa carreira do que é proporção de aproveitamento, e dá às
          taxas o espaço que a barra de posição precisa para ser lida. */}
      <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
        {dossier.records.slice(0, 3).map((record) => (
          <RecordCard key={record.id || record.label} record={record} onOpen={onOpenRecord} />
        ))}
      </div>
      {dossier.records.length > 3 && (
        <div className="mt-2.5 grid gap-2.5 sm:grid-cols-2">
          {dossier.records.slice(3).map((record) => (
            <RecordCard key={record.id || record.label} record={record} onOpen={onOpenRecord} />
          ))}
        </div>
      )}

      {/* O backend já corta em 3 (ou 6): quatro cards deixavam um órfão sozinho na
          linha de baixo. Aqui só resta o caso de quem tem HISTÓRIA de menos para
          três — a grade encolhe junto em vez de abrir buraco. */}
      {dossier.highlights?.length > 0 && (
        <div
          className={`mt-2.5 grid gap-2.5 ${
            dossier.highlights.length === 1
              ? ""
              : dossier.highlights.length === 2
                ? "sm:grid-cols-2"
                : "sm:grid-cols-3"
          }`}
        >
          {dossier.highlights.map((item) => (
            <div
              key={item.label}
              className="relative overflow-hidden rounded-xl border border-status-yellow/25 bg-[#1c1808]/95 px-3.5 py-3"
            >
              {/* O troféu é marca-d'água: fica atrás do texto, recortado pela
                  borda do card, e é o que separa "destaque" de "mais um card". */}
              <HighlightTrophy />
              <div className="relative">
                <span className="block text-[11px] font-semibold text-status-yellow">{item.label}</span>
                <strong className="mt-1.5 block text-base font-semibold text-status-yellow">{item.value}</strong>
                <p className="mt-1 text-[11px] leading-4 text-text-secondary">{item.detail}</p>
              </div>
            </div>
          ))}
        </div>
      )}

      <SeasonTrajectory
        seasons={dossier.seasonResults}
        worldFirstYear={dossier.worldFirstYear}
        worldLastYear={dossier.worldLastYear}
        outsideSeasons={dossier.outsideScopeSeasons}
        anoAceso={anoAceso}
        onAcenderAno={setAnoAceso}
      />

      <TitleGallery
        titles={dossier.titleCategories}
        seasons={dossier.seasonResults}
        anoAceso={anoAceso}
        onAcenderAno={setAnoAceso}
      />
    </section>
  );
}

// Galeria de títulos.
//
// A versão em cards repetia em SEIS cards a mesma categoria, a mesma contagem de
// vitórias e a mesma frase sobre o mesmo piloto — e, de tanto repetir, escondia
// o que estava acontecendo: seis títulos SEGUIDOS, quatro deles com dobradinha
// do mesmo piloto. Isso é uma dinastia, e o layout contava como seis fatos
// soltos.
//
// Aqui o que se repete virou cabeçalho (a categoria, uma vez, com o resumo do
// reinado) e o que varia virou coluna. Repetição é ilegível espalhada em cards e
// legível empilhada numa coluna: a de pontos passa a mostrar a equipe ganhando
// por menos a cada ano, que nos cards era impossível de ver.
function TitleGallery({ titles, seasons, anoAceso = null, onAcenderAno = null }) {
  const { t } = useTranslation();
  const dados = useMemo(() => {
    const lista = (Array.isArray(titles) ? titles : []).filter((item) => item.year);
    if (!lista.length) return null;

    const anosTitulo = new Map();
    for (const titulo of lista) {
      anosTitulo.set(Number(titulo.year), titulo);
    }
    // A régua cobre TODAS as temporadas da equipe, não só as de título: sem os
    // anos vazios em volta, seis títulos seguidos desenhariam igual a seis
    // títulos espalhados, que é a diferença entre um reinado e uma coleção.
    const anosCorridos = (Array.isArray(seasons) ? seasons : [])
      .filter((row) => Number(row.races) > 0)
      .map((row) => Number(row.year));
    const todos = [...anosTitulo.keys(), ...anosCorridos];
    const inicio = Math.min(...todos);
    const fim = Math.max(...todos);
    const regua = [];
    for (let ano = inicio; ano <= fim; ano += 1) {
      regua.push({ year: ano, title: anosTitulo.get(ano) ?? null });
    }

    // Um grupo por categoria, na ordem do primeiro título de cada uma.
    const grupos = [];
    for (const titulo of [...lista].sort((a, b) => Number(a.year) - Number(b.year))) {
      const chave = titulo.categoryId || titulo.category;
      let grupo = grupos.find((item) => item.key === chave);
      if (!grupo) {
        grupo = { key: chave, category: titulo.category, categoryId: titulo.categoryId, rows: [] };
        grupos.push(grupo);
      }
      grupo.rows.push(titulo);
    }

    return { regua, grupos };
  }, [titles, seasons]);

  if (!dados) return null;

  // Um título só ganha a MESMA tela de quem tem seis: régua, cabeçalho e tabela.
  //
  // A versão anterior colapsava o caso de um título numa linha, com o argumento
  // de que três níveis de moldura para um fato é mais chrome que conteúdo. Estava
  // errado por dois motivos. O primeiro é que a régua é MAIS informativa aí: ela
  // mostra o único ano que importou dentro de treze temporadas, e um título
  // isolado numa carreira longa é uma história melhor que seis seguidos. O
  // segundo é que dar a tela boa só para a dinastia premia quem já tem muito, e
  // o dossiê existe para contar a história de qualquer equipe do grid.
  return (
    <div className="mt-5" data-testid="team-history-title-gallery">
      <BlockLabel>{t("myTeamTab.history.records.titleGallery")}</BlockLabel>
      <div className="mt-2.5 flex gap-1" data-testid="team-history-title-rail">
        {dados.regua.map((celula) => {
          const cor = celula.title
            ? getCategoryColor(celula.title.categoryId) || celula.title.color
            : null;
          const ano = String(celula.year);
          const aceso = anoAceso === ano;
          // Os dois anéis convivem numa propriedade só: o dourado por DENTRO
          // continua sendo a dobradinha, o branco por fora é o ano aceso. Como
          // o de fora não ocupa espaço de layout, a régua não se mexe ao acender.
          const aneis = [
            celula.title?.championIsTeam ? `inset 0 0 0 1.5px ${MEDAL_COLORS.first}` : null,
            aceso ? "0 0 0 1px rgba(255,255,255,0.55)" : null,
          ].filter(Boolean);
          return (
            <Tooltip
              key={celula.year}
              texto={
                celula.title
                  ? `${celula.year} · ${celula.title.category}`
                  : t("myTeamTab.history.records.titleRailEmpty", { year: celula.year })
              }
            >
              <span
                data-year={celula.year}
                data-title={celula.title ? "true" : undefined}
                data-double={celula.title?.championIsTeam ? "true" : undefined}
                data-aceso={aceso ? "true" : undefined}
                onMouseEnter={() => onAcenderAno?.(ano)}
                onMouseLeave={() => onAcenderAno?.(null)}
                className="h-5 min-w-[10px] flex-1 rounded transition-[box-shadow]"
                style={{
                  backgroundColor: cor || "#141f2c",
                  boxShadow: aneis.length ? aneis.join(", ") : undefined,
                }}
              />
            </Tooltip>
          );
        })}
      </div>
      <TitleRailYears years={dados.regua.map((celula) => celula.year)} />
      <div className="mt-2 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        <MedalKey color={dados.grupos[0].categoryId ? getCategoryColor(dados.grupos[0].categoryId) : "#8020D0"} label={t("myTeamTab.history.records.titleRailKey")} />
        <span className="flex items-center gap-1.5">
          <span
            className="h-2.5 w-2.5 rounded-[3px]"
            style={{
              backgroundColor: dados.grupos[0].categoryId ? getCategoryColor(dados.grupos[0].categoryId) : "#8020D0",
              boxShadow: `inset 0 0 0 1.5px ${MEDAL_COLORS.first}`,
            }}
          />
          {t("myTeamTab.history.records.titleRailDoubleKey")}
        </span>
      </div>

      {dados.grupos.map((grupo) => (
        <TitleGroup key={grupo.key} grupo={grupo} />
      ))}
    </div>
  );
}

// Rótulos de ano da régua: um a cada N células, para caber sem virar borrão.
function TitleRailYears({ years }) {
  const passo = Math.max(1, Math.ceil(years.length / 7));
  return (
    <div className="mt-1 flex gap-1">
      {years.map((year, index) => (
        <span key={year} className="min-w-[10px] flex-1 text-center font-mono text-[10px] text-text-muted">
          {index % passo === 0 ? year : ""}
        </span>
      ))}
    </div>
  );
}

function TitleGroup({ grupo }) {
  const { t } = useTranslation();
  const cor = grupo.categoryId ? getCategoryColor(grupo.categoryId) : "#58a6ff";
  const anos = grupo.rows.map((row) => Number(row.year));
  const dobradinhas = grupo.rows.filter((row) => row.championIsTeam).length;
  const span = anos.length > 1 ? `${Math.min(...anos)}–${Math.max(...anos)}` : String(anos[0]);
  return (
    <div className="mt-4" data-testid="team-history-title-group" data-category={grupo.categoryId || undefined}>
      <div className="flex flex-wrap items-baseline gap-x-2 border-l-[3px] pl-2.5" style={{ borderLeftColor: cor }}>
        <strong className="text-sm font-semibold text-text-primary">{grupo.category}</strong>
        <span className="text-[11px] text-text-secondary">
          {t("myTeamTab.history.records.titleCount", { count: grupo.rows.length })}
          {" · "}
          {span}
          {dobradinhas > 0
            ? ` · ${t("myTeamTab.history.records.titleDoubleCount", { count: dobradinhas })}`
            : ""}
        </span>
      </div>
      <div className="mt-2 overflow-hidden rounded-lg border border-white/10">
        <div className="grid grid-cols-[52px_60px_34px_minmax(0,1fr)] gap-x-3 bg-[#0f1c2b] px-3.5 py-1.5 text-[10px] font-semibold text-text-secondary">
          <span>{t("myTeamTab.history.sport.cols.year")}</span>
          <span className="text-right">{t("myTeamTab.history.sport.cols.points")}</span>
          <span className="text-right">{t("myTeamTab.history.sport.cols.wins")}</span>
          <span>{t("myTeamTab.history.records.titleChampionCol")}</span>
        </div>
        {grupo.rows.map((row) => (
          <div
            key={row.year}
            data-title-year={row.year}
            data-double={row.championIsTeam ? "true" : undefined}
            className="grid grid-cols-[52px_60px_34px_minmax(0,1fr)] items-center gap-x-3 border-t border-white/[0.06] px-3.5 py-1.5 text-xs"
          >
            <span className="font-mono font-bold text-[color:var(--team)]">{row.year}</span>
            {/* Aqui os pontos fazem sentido, ao contrário do gráfico: são de uma
                temporada só, e a coluna os empilha para comparação direta. */}
            <span className="text-right font-mono text-text-primary">{row.points}</span>
            <span className="text-right font-mono text-text-primary">{row.wins}</span>
            <ChampionLine title={row} />
          </div>
        ))}
      </div>
    </div>
  );
}

// O campeão de PILOTOS daquele ano — outro campeonato, que pode ter ido para
// outra casa. A frase "campeão de pilotos" não se repete linha a linha: o
// cabeçalho da coluna já diz isso, e o que sobra é o nome. A coroa acesa em
// dourado faz o trabalho que a frase fazia.
function ChampionLine({ title }) {
  if (!title.championDriver) return <span />;
  const dobradinha = title.championIsTeam;
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <Crown
        size={12}
        strokeWidth={1.8}
        aria-hidden="true"
        className="shrink-0"
        style={{ color: dobradinha ? MEDAL_COLORS.first : MEDAL_COLORS.nearMiss }}
      />
      <span className={`truncate ${dobradinha ? "text-status-yellow" : "text-text-secondary"}`}>
        {title.championDriver}
        {!dobradinha && title.championTeam ? (
          <span className="text-text-muted">{` · ${title.championTeam}`}</span>
        ) : null}
      </span>
    </span>
  );
}

// Card de record: valor, média do grupo ao lado, barra de posição e o rank por
// extenso. A barra enche na PROPORÇÃO INVERSA da posição — 1º de 24 enche tudo,
// 24º de 24 fica quase vazia. Sem `rankTotal` (backend antigo ou payload
// incompleto) a barra some em vez de inventar um denominador.
function RecordCard({ record, onOpen = null }) {
  const { t } = useTranslation();
  const hasScale = record.rankTotal > 0 && record.rankPosition > 0;
  const fill = hasScale ? ((record.rankTotal - record.rankPosition + 1) / record.rankTotal) * 100 : 0;
  // O card só vira botão quando há métrica para ordenar E destino para abrir. Um
  // record sem `id` (payload antigo) não tem por onde ordenar a tabela, e clicar
  // levaria a uma lista arbitrária — pior que não clicar.
  const clicavel = Boolean(onOpen && record.id);

  return (
    <div
      className={`relative rounded-xl bg-[#0f1c2b] px-3.5 py-3 ${
        clicavel
          ? "cursor-pointer transition-glass hover:bg-[#132436] focus-within:ring-1 focus-within:ring-accent-primary/50"
          : ""
      }`}
      data-record={record.id || undefined}
    >
      {/* O botão cobre o card inteiro em vez de embrulhá-lo: assim a área de
          clique é a mesma que o olho vê, e a barra de posição, o ícone e os
          números continuam sendo o que eram, sem herdar estilo de botão. O
          `sr-only` é o que dá ao alvo um nome para leitor de tela — o texto
          visível está tudo embaixo dele, e não é lido como rótulo. */}
      {clicavel ? (
        <button
          type="button"
          onClick={() => onOpen(record.id)}
          data-testid={`team-history-record-open-${record.id}`}
          className="absolute inset-0 z-10 rounded-xl"
        >
          <span className="sr-only">{t("myTeamTab.history.records.openRanking", { metric: record.label })}</span>
        </button>
      ) : null}
      {/* Ícone da métrica no canto, apagado: identifica o card na varredura sem
          disputar atenção com o número. Vem do `id` do record, não do rótulo —
          rótulo é texto traduzido. */}
      <span className="pointer-events-none absolute right-3 top-3 text-white/15">
        <MetricIcon name={record.id} size={24} />
      </span>
      <span className="block truncate pr-7 text-[11px] font-semibold text-text-secondary">{record.label}</span>
      <div className="mt-1 flex items-baseline gap-2">
        <strong className="font-mono text-xl leading-none text-text-primary">{record.value}</strong>
        {record.groupAverage ? (
          <span className="truncate text-[11px] text-text-secondary">
            {t("myTeamTab.history.records.groupAverage", { value: record.groupAverage })}
          </span>
        ) : null}
      </div>
      {hasScale ? (
        <>
          <div className="mt-2.5 h-[3px] overflow-hidden rounded-full bg-white/10">
            <div className="h-full rounded-full bg-[color:var(--team)]" style={{ width: `${fill}%` }} />
          </div>
          <span className="mt-1.5 block text-[11px] text-text-secondary">
            {t("myTeamTab.history.records.rankOf", { rank: record.rank, total: record.rankTotal })}
          </span>
        </>
      ) : (
        <span className="mt-2.5 block text-[11px] text-text-secondary">{record.rank}</span>
      )}
    </div>
  );
}

// Cores das colocações. Ouro reaproveita o amarelo de status que a UI já usa
// para vitória; prata e bronze são novos e existem só aqui — são metal, não
// estado, e por isso não entram na paleta semântica do app.
//
// O 4º e o 5º são UMA faixa só, no mesmo cinza-azulado apagado. Separá-los em
// dois tons foi um erro de leitura: quem olha a coluna não precisa saber se
// aquele fim de semana terminou em 4º ou em 5º, precisa saber quantas vezes a
// equipe chegou PERTO. Dois azuis vizinhos só devolveram a dúvida de qual bloco
// era qual — e a resposta não mudava nada.
//
// O abandono é o único VERMELHO da faixa, e é o vermelho de estado que o app já
// usa para erro. Ele não é uma colocação — é o oposto de uma — e por isso é a
// única entrada que não tem parentesco com as outras.
const MEDAL_COLORS = {
  first: "#f2c46d",
  second: "#c2ccd8",
  third: "#c07f4a",
  nearMiss: "#46586d",
  dnf: "#ef4444",
};

// Altura da área de plotagem da faixa e as marcas do eixo Y, em % do top 5.
// Três marcas: sem a de 50% a barra não tem meio de referência; com mais que
// isso a grade compete com as colunas.
const TRAJECTORY_HEIGHT = 92;
const AXIS_TICKS = [0, 50, 100];

// Janela da faixa: as últimas 15 temporadas do mundo. Um save antigo chega a
// 25+ anos, e desenhar tudo espremia as temporadas recentes — que são as que o
// jogador está olhando — em colunas de poucos pixels. Quem quiser a história
// completa tem a tabela da aba Esportivo, que não recorta nada.
const TRAJECTORY_WINDOW_YEARS = 15;

// Trajetória: uma coluna por temporada, altura = % das corridas terminadas no
// top 5, repartida em 1º, 2º, 3º e 4º-5º.
//
// A escala é POR CORRIDA de propósito. Somar pontos misturava coisas que não se
// comparam — uma temporada de 12 corridas rende mais pontos que uma de 6 sem que
// a campanha tenha sido melhor, e categorias diferentes pontuam diferente. "9 no
// top 5 em 13 corridas" atravessa temporada e categoria sem essa distorção.
//
// O top 5 (e não só o pódio) porque uma equipe de meio de grid vivia como faixa
// vazia: sem pódio, o gráfico não dizia NADA sobre ela. O 4º-5º apagado mostra a
// temporada que quase foi, e é a diferença entre "não competiu" e "faltou pouco".
//
// O eixo vai de 0 a 100%: coluna cheia é top 5 em toda corrida do ano.
// Tooltip da coluna, em linhas.
//
// A versão anterior era uma frase única com tudo separado por "·", incluindo as
// colocações zeradas ("0× 2º") — ilegível justamente onde o jogador para o mouse
// para entender a barra. Aqui cada coisa tem sua linha, e só aparece o que
// aconteceu: a lista de colocações espelha os blocos desenhados, de cima para
// baixo, na mesma ordem — com a mesma cor do bloco ao lado do texto.
//
// Sai estruturado, e não como string de `\n`, porque quem desenha é o balão do
// app (`TrajectoryTooltip`) e não o `title` do sistema. O `texto` continua
// existindo por baixo: é o nome acessível da coluna, para quem lê por leitor de
// tela e para o teste.
function seasonTooltip(t, { row, races, topFive, steps, dnfs }) {
  const base = "myTeamTab.history.records.seasonTooltip";
  const header = row.category ? `${row.year} · ${row.category}` : String(row.year);
  const hasPosition = row.position && row.position !== "—";
  const meta = hasPosition
    ? t(`${base}.meta`, { position: row.position, races, topFive })
    : t(`${base}.metaNoPosition`, { races, topFive });
  const linhas = steps.length
    ? steps.map((step) => ({
        id: step.id,
        color: step.color,
        // Na tela, só a contagem: o quadradinho ao lado JÁ é a colocação, na
        // mesma cor do bloco da barra e da legenda embaixo do gráfico. Repetir
        // "1º" ao lado do ouro é dizer duas vezes a mesma coisa num balão que
        // tem quatro linhas.
        //
        // `value` e não `count`: `count` é palavra reservada do i18next e ligaria
        // a máquina de plural, mandando procurar chaves `..._one`/`..._other`.
        texto: t(`${base}.countShort`, { value: step.count }),
        // Para o leitor de tela a cor não existe — ali a colocação continua
        // escrita por extenso.
        textoAcessivel: t(`${base}.count`, {
          value: step.count,
          label: t(`myTeamTab.history.records.medals.${step.id}`),
        }),
      }))
    : [{ id: "empty", color: null, texto: t(`${base}.empty`) }];
  // O abandono entra por último e SÓ quando existe. Ele guarda o rótulo "DNF"
  // porque não é uma colocação: as linhas de cima contam onde a equipe terminou,
  // esta conta o fim de semana em que ela não terminou — e a unidade é CARRO,
  // não corrida (os dois carros podem abandonar no mesmo domingo).
  if (dnfs > 0) {
    linhas.push({
      id: "dnf",
      color: MEDAL_COLORS.dnf,
      texto: t(`${base}.count`, { value: dnfs, label: t("myTeamTab.history.records.medals.dnf") }),
    });
  }
  return montarDica(header, meta, linhas);
}

// O par header/meta das temporadas fora do recorte vem colado num só valor de
// i18n, separado por "\n" — herança de quando o balão era o do sistema. Separar
// aqui evita duplicar a chave só para mudar quem desenha.
function dicaDeTexto(texto) {
  const [header, ...resto] = String(texto).split("\n");
  return montarDica(header, resto.join(" ").trim(), []);
}

function montarDica(header, meta, linhas) {
  return {
    header,
    meta,
    linhas,
    texto: [
      header,
      meta,
      ...(linhas.length ? ["", ...linhas.map((linha) => linha.textoAcessivel ?? linha.texto)] : []),
    ].join("\n"),
  };
}

// O balão da coluna, no estilo do app.
//
// O `title` nativo desenhava a caixa BRANCA do Windows no meio de um gráfico
// escuro, com a fonte do sistema e o atraso do sistema — meio segundo em que a
// informação simplesmente não existe. Aqui é a mesma casca dos balões dos
// gráficos de corrida: borda clara, fundo quase preto, blur e sombra.
//
// Vai num portal porque a faixa rola no eixo X (`overflow-x-auto`): um painel
// absoluto dentro dela seria recortado na borda da calha.
const TOOLTIP_MARGEM = 12;
const TOOLTIP_FOLGA = 8;

function TrajectoryTooltip({ rect, dica }) {
  const painelRef = useRef(null);
  const [medida, setMedida] = useState({ width: 0, height: 0 });

  // Mede depois de montar para saber se cabe acima da coluna. Enquanto a medida
  // não existe o painel fica invisível — um quadro pulando de posição no
  // primeiro frame se lê como falha de desenho.
  useLayoutEffect(() => {
    if (!painelRef.current) return;
    const next = { width: painelRef.current.offsetWidth, height: painelRef.current.offsetHeight };
    setMedida((atual) => (atual.width === next.width && atual.height === next.height ? atual : next));
  }, [dica]);

  if (!rect || !dica) return null;

  // Abre para cima por padrão — é de onde a coluna cresce e onde o cursor não
  // está. Vira para baixo quando o topo da janela não dá espaço.
  const cabeAcima = rect.top - medida.height - TOOLTIP_FOLGA >= TOOLTIP_MARGEM;
  const topo = cabeAcima ? rect.top - medida.height - TOOLTIP_FOLGA : rect.bottom + TOOLTIP_FOLGA;
  const esquerda = Math.min(
    Math.max(TOOLTIP_MARGEM, rect.left + rect.width / 2 - medida.width / 2),
    Math.max(TOOLTIP_MARGEM, window.innerWidth - medida.width - TOOLTIP_MARGEM),
  );

  return createPortal(
    <div
      ref={painelRef}
      data-testid="team-history-trajectory-tooltip"
      style={{
        position: "fixed",
        top: topo,
        left: esquerda,
        zIndex: 95,
        opacity: medida.height ? 1 : 0,
      }}
      className="pointer-events-none w-max max-w-[280px] rounded-lg border border-white/15 bg-[#0a0f16]/95 px-3 py-2 text-[11px] shadow-[0_12px_32px_rgba(0,0,0,0.55)] backdrop-blur"
    >
      <span className="block font-semibold leading-tight text-text-primary">{dica.header}</span>
      {dica.meta ? <span className="mt-1 block leading-snug text-text-secondary">{dica.meta}</span> : null}
      {dica.linhas.length ? (
        <ul className="mt-1.5 space-y-1 border-t border-white/[0.08] pt-1.5">
          {dica.linhas.map((linha) => (
            <li
              key={linha.id}
              // Sem o rótulo, a linha é só um número — e número em fonte de
              // número, para as contagens ficarem uma embaixo da outra.
              className={`flex items-center gap-1.5 leading-none ${
                linha.color ? "font-mono text-text-primary" : "text-text-muted"
              }`}
            >
              {linha.color ? (
                // O mesmo quadradinho da legenda embaixo do gráfico, na cor do
                // bloco — é o que liga a linha do balão à fatia da barra.
                <span className="h-2 w-2 shrink-0 rounded-sm" style={{ backgroundColor: linha.color }} />
              ) : null}
              {linha.texto}
            </li>
          ))}
        </ul>
      ) : null}
    </div>,
    document.body,
  );
}

function SeasonTrajectory({
  seasons,
  worldFirstYear,
  worldLastYear,
  outsideSeasons,
  anoAceso = null,
  onAcenderAno = null,
}) {
  const { t } = useTranslation();
  // A coluna sob o cursor, com o retângulo dela medido na hora do hover: o balão
  // vive num portal e não tem como se posicionar pelo pai.
  const [dicaAberta, setDicaAberta] = useState(null);

  const abrirDica = useCallback(
    (event, bar) => {
      setDicaAberta({ rect: event.currentTarget.getBoundingClientRect(), dica: bar.dica });
      onAcenderAno?.(bar.year);
    },
    [onAcenderAno],
  );
  const fecharDica = useCallback(() => {
    setDicaAberta(null);
    onAcenderAno?.(null);
  }, [onAcenderAno]);

  const bars = useMemo(() => {
    // Anos em que a equipe correu, mas em outra escada de categorias. O dossiê
    // recorta os fatos ao grupo comparável ("Grupo GT3"), então esses anos não
    // chegam em `seasons` — e sem eles a coluna virava "×", afirmando que a
    // equipe não disputou nada num ano em que ela disputou outro campeonato.
    const fora = new Map(
      (Array.isArray(outsideSeasons) ? outsideSeasons : []).map((item) => [Number(item.year), item]),
    );
    const rows = Array.isArray(seasons) ? seasons : [];
    const raced = new Map();
    for (const row of rows) {
      if (Number(row.races) <= 0) continue;
      const races = Number(row.races);
      const wins = Number(row.wins) || 0;
      const seconds = Number(row.seconds) || 0;
      const thirds = Number(row.thirds) || 0;
      const nearMiss = (Number(row.fourths) || 0) + (Number(row.fifths) || 0);
      const topFive = wins + seconds + thirds + nearMiss;
      // De cima para baixo, como um pódio se lê: 1º no alto, o "quase" na base.
      // 4º e 5º entram somados: a pergunta que a faixa responde é quantas vezes
      // a equipe chegou perto de pontuar alto, não em qual das duas casas.
      const steps = [
        { id: "first", count: wins, color: MEDAL_COLORS.first },
        { id: "second", count: seconds, color: MEDAL_COLORS.second },
        { id: "third", count: thirds, color: MEDAL_COLORS.third },
        { id: "nearMiss", count: nearMiss, color: MEDAL_COLORS.nearMiss },
      ].filter((step) => step.count > 0);
      const topFiveRate = (topFive / races) * 100;
      const dnfs = Number(row.dnfs) || 0;
      // O vermelho desce do TETO da coluna, enquanto o top 5 sobe do chão: são
      // as duas pontas do ano, e o meio vazio é o que sobrou — as corridas em
      // que a equipe terminou sem chegar perto.
      //
      // O teto é o espaço livre acima do top 5. A conta do abandono é por CARRO
      // sobre corridas, então em tese ela passa de 100% (dois carros, uma
      // corrida) — e nesse caso o bloco para onde o top 5 começa em vez de
      // invadi-lo. O número exato continua no balão, que é onde ele é lido.
      const dnfRate = Math.min((dnfs / races) * 100, 100 - topFiveRate);
      raced.set(Number(row.year), {
        year: String(row.year),
        raced: true,
        topFiveRate,
        dnfRate,
        steps,
        categoryId: row.categoryId || "",
        categoryLabel: row.category || "",
        dica: seasonTooltip(t, { row, races, topFive, steps, dnfs }),
      });
    }

    if (!raced.size) return [];

    // O eixo é o do MUNDO, não o da equipe, e recortado nas últimas
    // TRAJECTORY_WINDOW_YEARS temporadas. Os anos dentro da janela em que ela não
    // correu viram coluna de ausência, com um "×" no lugar da barra: é o que faz
    // uma equipe de 2024 num mundo de 2012 ocupar o gráfico inteiro e mostrar,
    // de relance, que ela chegou tarde.
    const anos = [...raced.keys()];
    const fim = Math.max(worldLastYear || 0, ...anos);
    // A janela nunca abre antes do primeiro ano do mundo: coluna de ausência num
    // ano em que o campeonato não existia seria ausência de mentira.
    const inicio = Math.max(
      Math.min(worldFirstYear || fim, ...anos),
      fim - TRAJECTORY_WINDOW_YEARS + 1,
    );
    const colunas = [];
    for (let year = inicio; year <= fim; year += 1) {
      const noRecorte = raced.get(year);
      if (noRecorte) {
        colunas.push(noRecorte);
        continue;
      }
      const outra = fora.get(year);
      colunas.push({
        year: String(year),
        raced: false,
        elsewhere: Boolean(outra),
        topFiveRate: 0,
        steps: [],
        categoryId: outra?.categoryId || "",
        categoryLabel: outra?.category || "",
        dica: dicaDeTexto(
          outra
            ? t("myTeamTab.history.records.seasonTooltip.elsewhere", {
                year,
                category: outra.category,
              })
            : t("myTeamTab.history.records.seasonTooltip.absent", { year }),
        ),
      });
    }
    return colunas;
  }, [seasons, t, worldFirstYear, worldLastYear, outsideSeasons]);

  if (!bars.length) return null;

  // A barra diz o quão bem a temporada foi, e não dizia NADA sobre onde ela foi
  // — 40% de top 5 na categoria de entrada e 40% na GT3 são campanhas de peso
  // completamente diferente. A tira colorida sob as colunas carrega essa camada
  // sem gastar altura: mesma paleta de categorias do resto do app, e um degrau
  // na escada aparece como troca de cor no ano exato.
  const categorias = [];
  for (const bar of bars) {
    if (!bar.categoryId || categorias.some((cat) => cat.id === bar.categoryId)) continue;
    categorias.push({ id: bar.categoryId, label: bar.categoryLabel || bar.categoryId });
  }

  return (
    <div className="mt-5">
      {/* O intervalo desenhado fica anunciado ao lado do título: a faixa recorta
          as últimas 15 temporadas, e recorte silencioso se lê como "está tudo
          aqui". */}
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.records.topFivePerRace")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {bars.length > 1 ? `${bars[0].year}–${bars[bars.length - 1].year}` : bars[0].year}
        </span>
      </div>
      {/* Rola no eixo X em vez de espremer: uma carreira longa tem 40 temporadas,
          e coluna de 3px não é leitura, é ruído. A calha do eixo Y fica FORA da
          área rolável, senão os rótulos 0/50/100% deslizariam junto com as
          barras e deixariam de ser régua. */}
      <div className="mt-2.5 flex gap-2 rounded-xl bg-[#0f1c2b] px-3.5 py-3" data-testid="team-history-trajectory">
        <div className="relative w-7 shrink-0" style={{ height: TRAJECTORY_HEIGHT }}>
          {AXIS_TICKS.map((tick) => (
            <span
              key={tick}
              className="absolute right-0 -translate-y-1/2 font-mono text-[10px] text-text-muted"
              style={{ top: `${100 - tick}%` }}
            >
              {`${tick}%`}
            </span>
          ))}
        </div>
        {/* Rolar com o balão aberto deslocaria a coluna por baixo dele — o
            retângulo foi medido antes do scroll. Fecha, e o próximo hover mede
            de novo. */}
        <div className="relative min-w-0 flex-1 overflow-x-auto" onScroll={fecharDica}>
          {/* Linhas de grade tracejadas atrás das colunas — é o que transforma
              "a barra é alta" em "a barra é 70%". Não capturam o mouse para não
              roubar o tooltip da coluna. */}
          <div className="pointer-events-none absolute inset-x-0 top-0" style={{ height: TRAJECTORY_HEIGHT }}>
            {AXIS_TICKS.map((tick) => (
              <div
                key={tick}
                className="absolute inset-x-0 border-t border-dashed border-white/10"
                style={{ top: `${100 - tick}%` }}
              />
            ))}
          </div>
          {/* Piso de 24px e teto de 64px por coluna. O piso protege a carreira de
              40 temporadas (coluna de 3px não é leitura); o teto protege a de 3,
              em que `flex-1` esticava cada barra para ~290px e ela deixava de
              parecer barra. Sobra vazio à direita — o que é honesto: a equipe tem
              mesmo pouca história. */}
          <div className="relative flex min-w-full items-end gap-1.5" style={{ height: TRAJECTORY_HEIGHT }}>
            {bars.map((bar) => (
              <div
                key={bar.year}
                data-year={bar.year}
                data-absent={bar.raced ? undefined : "true"}
                data-aceso={anoAceso === bar.year ? "true" : undefined}
                // Anel branco fino, e não mudança de cor: as cores da coluna são
                // a informação, e acender apagando-as seria trocar o que se
                // quer ler pelo destaque de onde ler.
                className={`relative h-full min-w-[24px] max-w-[64px] flex-1 rounded-md transition-[box-shadow] ${
                  anoAceso === bar.year ? "ring-1 ring-white/45" : ""
                }`}
                aria-label={bar.dica.texto}
                onMouseEnter={(event) => abrirDica(event, bar)}
                onMouseLeave={fecharDica}
              >
                {/* Trilho: a coluna VAZIA, sempre desenhada, de 0 a 100%.
                    Sem ele, uma temporada sem nenhum top 5 não tinha pixel
                    algum e ficava idêntica a uma temporada que não existiu — que
                    era exatamente o que se via numa equipe de meio de grid. Com
                    o trilho, o ano sempre aparece e o que varia é o quanto dele
                    está preenchido.

                    Ano em que a equipe não correu não ganha trilho: ganha um
                    "×". Trilho vazio quer dizer "correu e não pontuou", e as
                    duas coisas não podem desenhar igual.

                    E há um terceiro estado: correu, mas em outra escada de
                    categorias, fora do recorte comparável deste dossiê. Esse ano
                    ganha o ícone de categorias na cor da escada em que ela
                    estava — a tira colorida embaixo fecha a leitura. Antes ele
                    caía no "×" e o gráfico afirmava que a equipe tinha sumido do
                    mundo. */}
                {bar.raced ? (
                  <div className="absolute inset-0 rounded-md bg-white/[0.045]" />
                ) : bar.elsewhere ? (
                  <div
                    className="absolute inset-0 grid place-items-center rounded-md border border-dashed"
                    style={{
                      borderColor: `color-mix(in srgb, ${getCategoryColor(bar.categoryId)} 30%, transparent)`,
                    }}
                  >
                    <Layers
                      size={14}
                      strokeWidth={1.5}
                      aria-hidden="true"
                      style={{ color: `color-mix(in srgb, ${getCategoryColor(bar.categoryId)} 65%, transparent)` }}
                    />
                  </div>
                ) : (
                  <div className="absolute inset-0 grid place-items-center rounded-md border border-dashed border-white/[0.07]">
                    <X size={14} strokeWidth={1.5} aria-hidden="true" className="text-white/20" />
                  </div>
                )}
                {/* As colocações dividem a barra por `flex-grow`, não por altura
                    em porcentagem: assim preenchem exatamente a altura do top 5
                    mesmo que a contagem por colocação e o total divirjam. O piso
                    de 3px garante que um único 2º lugar numa temporada cheia de
                    4ºs continue visível — "quase invisível" e "não tem" são a
                    mesma coisa para quem olha. */}
                {bar.steps.length ? (
                  <div
                    className="absolute inset-x-0 bottom-0 flex flex-col overflow-hidden rounded-md"
                    style={{ height: `${bar.topFiveRate}%`, minHeight: "4px" }}
                  >
                    {bar.steps.map((step, index) => (
                      <div
                        key={step.id}
                        data-step={step.id}
                        className="w-full"
                        style={{
                          flexGrow: step.count,
                          flexBasis: 0,
                          minHeight: "3px",
                          // Gradiente quase reto: ele dá volume à barra, mas o
                          // que era 28% de escurecimento fazia o pé de um bloco
                          // chegar na cor do topo do bloco de baixo — e era
                          // metade do motivo de não dar para dizer se aquilo era
                          // 2º, 4º ou 5º. Volume é enfeite; a cor é a leitura.
                          backgroundImage: `linear-gradient(180deg, ${step.color}, color-mix(in srgb, ${step.color} 88%, #0b1524))`,
                          // Fio escuro entre os blocos: sem ele, dois vizinhos de
                          // brilho parecido encostam e viram uma faixa só.
                          borderTop: index > 0 ? "1px solid rgba(8,15,25,0.65)" : undefined,
                        }}
                      />
                    ))}
                  </div>
                ) : null}
                {/* Abandonos, pendurados no teto da coluna. Piso de 3px pelo
                    mesmo motivo dos degraus: um abandono solo numa temporada
                    longa vale menos de um pixel, e "quase invisível" se lê como
                    "não teve". */}
                {bar.dnfRate > 0 ? (
                  <div
                    data-dnf={bar.year}
                    className="absolute inset-x-0 top-0 rounded-md"
                    style={{
                      height: `${bar.dnfRate}%`,
                      minHeight: "3px",
                      backgroundImage: `linear-gradient(180deg, ${MEDAL_COLORS.dnf}, color-mix(in srgb, ${MEDAL_COLORS.dnf} 88%, #0b1524))`,
                    }}
                  />
                ) : null}
              </div>
            ))}
          </div>
          {/* Tira de categoria: uma célula por ano, com os MESMOS limites da
              coluna, então ela fica alinhada por construção — sem agrupar anos
              em faixas contínuas, que desalinhavam (uma faixa de N anos come N-1
              gaps, e o espaço livre distribuído pelo flex deixava de bater). Anos
              seguidos na mesma categoria já se leem como um trilho único. */}
          <div className="flex min-w-full gap-1.5" data-testid="team-history-trajectory-categories">
            {bars.map((bar) => (
              <span
                key={bar.year}
                data-category={bar.categoryId || undefined}
                // Sem `title`: a categoria já é o segundo termo do cabeçalho do
                // balão da coluna, e um segundo balão do sistema no mesmo
                // gráfico era a caixa branca de volta, 3px abaixo.
                aria-label={bar.categoryLabel || undefined}
                className="mt-1.5 h-[3px] min-w-[24px] max-w-[64px] flex-1 rounded-full"
                style={{ backgroundColor: bar.categoryId ? getCategoryColor(bar.categoryId) : "transparent" }}
              />
            ))}
          </div>
          {/* Mesmos limites da coluna acima — é o que mantém o ano embaixo da sua
              própria barra. */}
          <div className="flex min-w-full gap-1.5">
            {bars.map((bar) => (
              <span key={bar.year} className="mt-1 min-w-[24px] max-w-[64px] flex-1 text-center font-mono text-[10px] text-text-muted">
                {bar.year}
              </span>
            ))}
          </div>
        </div>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        <MedalKey color={MEDAL_COLORS.first} label={t("myTeamTab.history.records.medals.first")} />
        <MedalKey color={MEDAL_COLORS.second} label={t("myTeamTab.history.records.medals.second")} />
        <MedalKey color={MEDAL_COLORS.third} label={t("myTeamTab.history.records.medals.third")} />
        <MedalKey color={MEDAL_COLORS.nearMiss} label={t("myTeamTab.history.records.medals.nearMiss")} />
        <MedalKey color={MEDAL_COLORS.dnf} label={t("myTeamTab.history.records.medals.dnf")} />
        <span>{t("myTeamTab.history.records.topFivePerRaceLegend")}</span>
      </div>
      {categorias.length > 0 && (
        <div className="mt-1.5 flex flex-wrap items-center gap-3 text-[10px] text-text-muted" data-testid="team-history-trajectory-legend">
          <span className="text-text-muted/80">
            {t("myTeamTab.history.records.categoryBand")}
          </span>
          {categorias.map((cat) => (
            <MedalKey key={cat.id} color={getCategoryColor(cat.id)} label={cat.label} />
          ))}
        </div>
      )}
      <TrajectoryTooltip rect={dicaAberta?.rect} dica={dicaAberta?.dica} />
    </div>
  );
}

// A cronologia de marcos ("Momentos-chave") viveu aqui e saiu: cinco linhas de
// prosa que diziam o ano da estreia, do primeiro pódio e do último registro —
// datas de cartório, nenhuma delas sobre QUEM correu. O espaço é do ranking de
// pilotos abaixo, que responde a pergunta que o grupo faz no título.

// Cor de uma colocação, na mesma paleta dos degraus da faixa de Records: ouro,
// prata, bronze, o "quase" apagado e dois tons de fundo para o resto. Um número
// só muda de significado entre as telas se mudar de cor — então não muda.
const PLACEMENT_COLORS = {
  first: MEDAL_COLORS.first,
  second: MEDAL_COLORS.second,
  third: MEDAL_COLORS.third,
  nearMiss: MEDAL_COLORS.nearMiss,
  topTen: "#22303f",
  outside: "#141f2c",
};

function placementTone(position) {
  if (position === 1) return PLACEMENT_COLORS.first;
  if (position === 2) return PLACEMENT_COLORS.second;
  if (position === 3) return PLACEMENT_COLORS.third;
  if (position >= 4 && position <= 5) return PLACEMENT_COLORS.nearMiss;
  if (position >= 6 && position <= 10) return PLACEMENT_COLORS.topTen;
  return PLACEMENT_COLORS.outside;
}

// Texto legível sobre o quadrado: os tons claros (ouro, prata, bronze) precisam
// de tinta escura, os escuros de tinta clara.
function placementInk(position) {
  return position >= 1 && position <= 3 ? "#0b1524" : "#8fa3bb";
}

// Fita de forma recente: as últimas corridas, da mais antiga à mais nova.
//
// É o único bloco do dossiê que fala do PRESENTE. Todo o resto é história
// agregada, e agregado de 87 corridas não mostra que a equipe subiu de categoria
// no ano passado e não anda mais perto do pódio — que é exatamente a pergunta de
// quem abre o dossiê numa janela de transferências.
function RecentForm({ races, rodadaAcesa = null, onAcenderRodada = null }) {
  const { t } = useTranslation();
  if (!races?.length) return null;
  const primeira = races[0];
  const ultima = races[races.length - 1];
  // Troca de categoria no meio da fita é a explicação de uma queda que, sem ela,
  // se leria como perda de forma.
  const trocou = primeira.categoryId && ultima.categoryId && primeira.categoryId !== ultima.categoryId;
  return (
    <div>
      <BlockLabel>{t("myTeamTab.history.sport.recentForm")}</BlockLabel>
      <div className="mt-2.5 flex gap-1.5" data-testid="team-history-recent-form">
        {races.map((race, index) => {
          const pos = Number(race.position) || 0;
          const chave = chaveDaRodada(race.year, race.round);
          const aceso = chave != null && chave === rodadaAcesa;
          return (
            <Tooltip
              key={`${race.year}-${race.round}-${index}`}
              texto={
                pos
                  ? t("myTeamTab.history.sport.formTooltip", {
                      year: race.year,
                      round: race.round,
                      category: race.category,
                      position: pos,
                    })
                  : t("myTeamTab.history.sport.formTooltipNoPosition", {
                      year: race.year,
                      round: race.round,
                      category: race.category,
                    })
              }
            >
              <span
                data-position={pos || undefined}
                data-round={chave || undefined}
                data-aceso={aceso ? "true" : undefined}
                onMouseEnter={() => onAcenderRodada?.(chave)}
                onMouseLeave={() => onAcenderRodada?.(null)}
                // Anel branco, igual ao da faixa de top 5: o quadrado já é
                // colorido pela colocação, e trocar a cor apagaria o dado.
                className={`grid h-9 flex-1 place-items-center rounded-md font-mono text-[11px] transition-[box-shadow] ${
                  aceso ? "ring-1 ring-white/60" : ""
                }`}
                style={{ backgroundColor: placementTone(pos || 99), color: placementInk(pos || 99) }}
              >
                {pos || "—"}
              </span>
            </Tooltip>
          );
        })}
      </div>
      <div className="mt-1.5 flex items-center justify-between gap-3 font-mono text-[10px] text-text-muted">
        <span>{t("myTeamTab.history.sport.formRound", { year: primeira.year, round: primeira.round })}</span>
        {trocou ? (
          <span className="truncate font-sans text-text-secondary">
            {t("myTeamTab.history.sport.formMoved", { category: ultima.category })}
          </span>
        ) : null}
        <span>{t("myTeamTab.history.sport.formRound", { year: ultima.year, round: ultima.round })}</span>
      </div>
    </div>
  );
}

// Geometria da campanha do campeonato. O eixo aqui NÃO é invertido: pontos são
// pontos, mais é mais alto, e a linha que sobe é a equipe que está ganhando.
const RUN_WIDTH = 640;
const RUN_HEIGHT = 186;
const RUN_LEFT = 46;
// A direita para antes da borda porque a etiqueta da equipe fica FORA do
// desenho, no fim da linha — dentro, ela cobriria justamente o trecho decisivo.
const RUN_RIGHT = 596;
const RUN_TOP = 16;
const RUN_BOTTOM = 132;
const RUN_AXIS = 146;
const RUN_ROUND_Y = 166;
const RUN_SURFACE = "#0f1c2b";
// Cinza das outras equipes. Elas precisam existir — sem o campo, a linha da
// equipe é só um traço subindo, e subir é o que toda linha acumulada faz — mas
// não podem competir: um fio fino, sem marcador e sem etiqueta.
const RUN_FIELD_STROKE = "#3a4d63";

// Se há campanha para desenhar. Mora fora do componente porque o seletor de
// vistas precisa saber disso ANTES de escolher o que renderizar — e uma segunda
// cópia da regra derivaria da primeira no primeiro save antigo que aparecesse.
function campanhaTemDados(run) {
  return (Array.isArray(run?.rounds) ? run.rounds.length : 0) >= 2 && Boolean(run?.lines?.length);
}

// Campanha do campeonato: a pontuação ACUMULADA rodada a rodada, da equipe do
// dossiê contra todas as outras do mesmo campeonato.
//
// Substitui a curva de posição final por temporada no arranjo arrumado, e a
// diferença é a pergunta: a curva dizia ONDE a equipe terminou cada ano, esta
// diz COMO a temporada mais recente foi disputada. Vinte pontos abertos na
// primeira metade e defendidos até o fim, ou uma virada na última rodada,
// terminam ambos em "P1" — e desenham completamente diferente.
//
// É também o bloco que conversa com a fita de forma recente logo abaixo: as
// mesmas corridas, agora somadas contra quem estava de fato na pista.
function ChampionshipRun({
  run,
  seletor = null,
  seletorModo = null,
  modo = RUN_MODE_POSITION,
  rodadaAcesa = null,
  onAcenderRodada = null,
}) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  // O modo é o que o eixo MEDE, e é a diferença entre um gráfico com nuance e um
  // feixe de retas paralelas. Ele vem de fora porque é a métrica do BLOCO, não
  // deste gráfico: a curva entre campeonatos mede as mesmas duas coisas, e a
  // escolha atravessa a troca de vista.
  //
  // Em pontos acumulados todo mundo sobe — é o que acumulado faz — e a subida
  // comum a todas as linhas domina o desenho. Pior é o alcance: o eixo tem de
  // caber o líder, então numa temporada de 243 contra 74 o pelotão inteiro se
  // espreme em menos de um terço da altura e as diferenças que decidem o
  // campeonato viram espessura de traço.
  //
  // Descontar o líder não resolve: subtrair a linha dele é uma transformação
  // afim, tira a inclinação comum e mantém a compressão intacta.
  //
  // A COLOCAÇÃO resolve, porque troca o espaço. Em pontos, um líder disparado
  // come o eixo sozinho; em colocação, cada equipe ocupa exatamente uma faixa da
  // altura, por construção — nenhum outlier pode espremer ninguém. E é onde a
  // temporada acontece: as linhas se cruzam quando uma equipe passa a outra, que
  // é o evento que o acumulado esconde atrás de dois traços quase paralelos.
  const dados = useMemo(() => {
    if (!campanhaTemDados(run)) return null;
    const rounds = run.rounds;
    const lines = run.lines;
    const porPosicao = modo === RUN_MODE_POSITION;
    const pontosEm = (line, index) => Number(line.points?.[index] ?? 0);
    // A classificação RODADA A RODADA, refeita a cada uma: é a colocação de
    // então, não a do fim. Empate cai para o id — arbitrário, mas estável, para
    // a linha não trocar de faixa sozinha entre aberturas da mesma tela.
    const classificacao = rounds.map((_, index) => {
      const ordem = [...lines].sort(
        (a, b) => pontosEm(b, index) - pontosEm(a, index) || a.teamId.localeCompare(b.teamId),
      );
      return new Map(ordem.map((line, posicao) => [line.teamId, posicao + 1]));
    });
    const valor = (line, index) =>
      porPosicao ? classificacao[index].get(line.teamId) : pontosEm(line, index);

    // Em colocação o eixo é fixo: P1 no topo, a última do grid embaixo. Em
    // pontos vai de zero ao líder.
    const alto = porPosicao ? 1 : Math.max(1, ...lines.map((line) => pontosEm(line, rounds.length - 1)));
    const baixo = porPosicao ? Math.max(2, lines.length) : 0;
    const x = (index) => RUN_LEFT + ((RUN_RIGHT - RUN_LEFT) * index) / (rounds.length - 1);
    const y = (v) => RUN_BOTTOM - ((RUN_BOTTOM - RUN_TOP) * (v - baixo)) / (alto - baixo);
    const traco = (line) =>
      rounds
        .map((_, index) => `${index ? "L" : "M"} ${x(index).toFixed(1)},${y(valor(line, index)).toFixed(1)}`)
        .join(" ");
    const selecionada = lines.find((line) => line.selected) ?? null;
    // Uma etiqueta a cada N rodadas quando elas não cabem. Abaixo de 34px os
    // rótulos "R10" começam a se encostar e a régua vira tarja.
    const passo = (RUN_RIGHT - RUN_LEFT) / (rounds.length - 1);
    const saltoRotulo = Math.max(1, Math.ceil(34 / Math.max(passo, 1)));
    return {
      rounds,
      porPosicao,
      alto,
      baixo,
      x,
      y,
      traco,
      selecionada,
      outras: lines.filter((line) => !line.selected),
      saltoRotulo,
      pontosSelecionada: selecionada
        ? rounds.map((_, index) => {
            const v = valor(selecionada, index);
            return { index, valor: v, cx: x(index), cy: y(v) };
          })
        : [],
    };
  }, [run, modo]);

  if (!dados) return null;
  const areaId = `run-area-${uid}`;
  const glowId = `run-glow-${uid}`;
  const { selecionada } = dados;
  // Três níveis só. Uma linha por colocação, num grid de dez, faria uma malha
  // que disputa atenção com as próprias linhas.
  const ticks = [...new Set([dados.alto, Math.round((dados.alto + dados.baixo) / 2), dados.baixo])];
  const rotuloTick = (tick) => (dados.porPosicao ? `P${tick}` : `${tick}`);
  const ultima = dados.pontosSelecionada[dados.pontosSelecionada.length - 1];
  // A rodada acesa, traduzida para índice do eixo. Rodada de OUTRO ano (a fita
  // recente atravessa temporadas) simplesmente não acha índice aqui — e não
  // acender é a resposta certa: aquela corrida não está neste gráfico.
  const indiceAceso = rodadaAcesa
    ? dados.rounds.findIndex((round) => chaveDaRodada(run.year, round) === rodadaAcesa)
    : -1;
  const pontoAceso = indiceAceso >= 0 ? dados.pontosSelecionada[indiceAceso] : null;
  // Meia distância entre rodadas: é a largura da faixa invisível que captura o
  // mouse. Menos que isso deixaria vãos mortos entre as rodadas.
  const meiaFaixa = dados.rounds.length > 1 ? (RUN_RIGHT - RUN_LEFT) / (dados.rounds.length - 1) / 2 : 12;
  // A mancha desce até a base do eixo nos dois modos — é o corpo da linha, não
  // uma medida por si.
  const baseArea = dados.y(dados.baixo);

  return (
    <div>
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5">
          <BlockLabel>{t("myTeamTab.history.sport.championshipTitle")}</BlockLabel>
          {seletor}
          <span className="font-mono text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.runScope", { year: run.year, category: run.category })}
          </span>
          {run.live ? (
            <span className="rounded-full bg-white/[0.07] px-2 py-0.5 text-[10px] font-semibold text-text-muted">
              {t("myTeamTab.history.sport.runLive")}
            </span>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {/* Os dois modos ficam à vista, e não num menu: o eixo muda de
              significado entre eles, e um gráfico que mede outra coisa sem
              anunciar é um gráfico que mente. */}
          {seletorModo}
          {/* A colocação vira pílula na cor da equipe: é o veredito do gráfico, e
              procurá-lo contando linhas de cima para baixo seria trabalho. */}
          {selecionada ? (
            <span
              data-testid="team-history-run-position"
              className="flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.12em]"
              style={{
                borderColor: "color-mix(in srgb, var(--team) 45%, transparent)",
                backgroundColor: "color-mix(in srgb, var(--team) 10%, transparent)",
                color: "var(--team)",
              }}
            >
              {t("myTeamTab.history.sport.runStanding", {
                position: selecionada.position,
                points: selecionada.total,
              })}
            </span>
          ) : null}
        </div>
      </div>

      <div className="mt-2.5 rounded-xl border border-white/[0.06] bg-[#0f1c2b] px-3 py-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
        <svg
          viewBox={`0 0 ${RUN_WIDTH} ${RUN_HEIGHT}`}
          className="h-auto w-full"
          data-testid="team-history-championship-run"
        >
          <defs>
            <linearGradient id={areaId} x1="0" y1={RUN_TOP} x2="0" y2={RUN_BOTTOM} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.34" />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.01" />
            </linearGradient>
            <filter id={glowId} x="-12%" y="-40%" width="124%" height="180%">
              <feGaussianBlur stdDeviation="3.2" result="borrao" />
              <feMerge>
                <feMergeNode in="borrao" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>

          {/* Grade: só três níveis. Uma linha por rodada faria uma malha que
              disputa atenção com as vinte linhas do campo. */}
          {ticks.map((tick, index) => (
            <g key={`tick-${index}`}>
              <line
                x1={RUN_LEFT}
                y1={dados.y(tick)}
                x2={RUN_RIGHT}
                y2={dados.y(tick)}
                stroke="#ffffff"
                strokeOpacity={index === 0 ? 0.1 : 0.05}
                strokeDasharray={index === 0 ? undefined : "2 5"}
              />
              <text x={RUN_LEFT - 8} y={dados.y(tick) + 3.4} textAnchor="end" fontSize="10" fill="#66788d">
                {rotuloTick(tick)}
              </text>
            </g>
          ))}
          <text x={RUN_LEFT - 8} y={RUN_TOP - 5} textAnchor="end" fontSize="8.5" fill="#4d5f74" letterSpacing="0.08em">
            {t(dados.porPosicao ? "myTeamTab.history.sport.runPositionAxis" : "myTeamTab.history.sport.runPointsAxis")}
          </text>

          {/* Em colocação, o topo do eixo é a liderança do campeonato. Marcá-lo
              dá o que ler contra: uma linha que encosta ali está brigando pelo
              título, e sem a marca P1 é só mais um valor da escala. */}
          {dados.porPosicao ? (
            <>
              <line
                x1={RUN_LEFT}
                y1={dados.y(1)}
                x2={RUN_RIGHT}
                y2={dados.y(1)}
                stroke="#e2c96a"
                strokeOpacity="0.4"
                strokeDasharray="5 4"
              />
              <text x={RUN_RIGHT - 2} y={dados.y(1) - 5} textAnchor="end" fontSize="8.5" fill="#e2c96a" fillOpacity="0.7" letterSpacing="0.1em">
                {t("myTeamTab.history.sport.runLeaderRef")}
              </text>
            </>
          ) : null}

          <line x1={RUN_LEFT} y1={RUN_TOP - 6} x2={RUN_LEFT} y2={RUN_AXIS} stroke="#ffffff" strokeOpacity="0.12" />
          <line x1={RUN_LEFT} y1={RUN_AXIS} x2={RUN_WIDTH - 8} y2={RUN_AXIS} stroke="#ffffff" strokeOpacity="0.08" />

          {/* Faixas invisíveis, uma por rodada, para o mouse ter onde pousar: os
              únicos alvos do gráfico eram as linhas, e mirar um traço de 1px
              para achar uma rodada não é um alvo.

              Vêm ANTES das linhas de propósito. O que é pintado depois fica por
              cima e continua capturando o mouse — então a linha do campo mantém
              o balão dela, e a faixa pega todo o resto da coluna. */}
          {onAcenderRodada
            ? dados.rounds.map((round, index) => (
                <rect
                  key={`faixa-${round}`}
                  data-round-band={chaveDaRodada(run.year, round) || undefined}
                  x={dados.x(index) - meiaFaixa}
                  y={RUN_TOP - 6}
                  width={meiaFaixa * 2}
                  height={RUN_AXIS - (RUN_TOP - 6)}
                  fill="transparent"
                  onMouseEnter={() => onAcenderRodada(chaveDaRodada(run.year, round))}
                  onMouseLeave={() => onAcenderRodada(null)}
                />
              ))
            : null}

          {/* O campo primeiro, para a linha da equipe passar POR CIMA de todas
              elas — é a única que não pode ser cruzada e escondida. */}
          {dados.outras.map((line) => (
            <path
              key={line.teamId}
              data-line={line.teamId}
              d={dados.traco(line)}
              fill="none"
              stroke={RUN_FIELD_STROKE}
              strokeWidth="1"
              strokeOpacity="0.7"
              strokeLinejoin="round"
            >
              <title>
                {t("myTeamTab.history.sport.runTooltip", {
                  team: line.team,
                  position: line.position,
                  points: line.total,
                })}
              </title>
            </path>
          ))}

          {selecionada ? (
            <>
              <path
                d={`${dados.traco(selecionada)} L ${RUN_RIGHT},${baseArea} L ${RUN_LEFT},${baseArea} Z`}
                fill={`url(#${areaId})`}
              />
              <path
                data-line={selecionada.teamId}
                data-selected="true"
                d={dados.traco(selecionada)}
                fill="none"
                stroke="var(--team)"
                strokeWidth="2.6"
                strokeLinecap="round"
                strokeLinejoin="round"
                filter={`url(#${glowId})`}
              >
                <title>
                  {t("myTeamTab.history.sport.runTooltip", {
                    team: selecionada.team,
                    position: selecionada.position,
                    points: selecionada.total,
                  })}
                </title>
              </path>
              {/* Marcadores só na equipe do dossiê, e vazados na cor do cartão
                  para a linha não passar por dentro deles. */}
              {dados.pontosSelecionada.map((ponto) => (
                <circle
                  key={`ponto-${ponto.index}`}
                  cx={ponto.cx}
                  cy={ponto.cy}
                  r="2.6"
                  fill={RUN_SURFACE}
                  stroke="var(--team)"
                  strokeWidth="1.8"
                />
              ))}
              {ultima ? (
                <g>
                  <circle cx={ultima.cx} cy={ultima.cy} r="4.4" fill="var(--team)" filter={`url(#${glowId})`} />
                  <text
                    x={ultima.cx + 10}
                    y={ultima.cy + 3.6}
                    fontSize="11"
                    fontWeight="700"
                    fill="var(--team)"
                  >
                    {/* A etiqueta mostra o que o EIXO mede. Repetir o total em
                        pontos com a linha desenhada em colocação seria um número
                        que não bate com a altura em que ele está. */}
                    {dados.porPosicao ? `P${ultima.valor}` : selecionada.total}
                  </text>
                </g>
              ) : null}
            </>
          ) : null}

          {/* A rodada acesa: um fio vertical atravessando o desenho e, sobre a
              linha da equipe, o marcador cheio. O fio é quem faz a ponte com a
              fita lá embaixo — ele diz ONDE no eixo aquela corrida caiu, que é
              o que uma fita de quadradinhos não tem como dizer.

              Vem depois das linhas para não ficar por baixo do campo, e é
              `pointer-events-none` para não roubar o mouse das faixas. */}
          {indiceAceso >= 0 ? (
            <g data-testid="team-history-run-round-mark" className="pointer-events-none">
              <line
                x1={dados.x(indiceAceso)}
                y1={RUN_TOP - 6}
                x2={dados.x(indiceAceso)}
                y2={RUN_AXIS}
                stroke="#ffffff"
                strokeOpacity="0.28"
                strokeDasharray="3 3"
              />
              {pontoAceso ? (
                <circle cx={pontoAceso.cx} cy={pontoAceso.cy} r="4.2" fill="var(--team)" />
              ) : null}
            </g>
          ) : null}

          {/* Régua de rodadas embaixo da linha do eixo: acima é pontuação,
              abaixo é quando. */}
          {dados.rounds.map((round, index) =>
            // A régua é esparsa quando as rodadas não cabem — e a acesa aparece
            // mesmo fora do salto: perguntaram por ela, esconder o número dela
            // seria acender sem responder.
            index % dados.saltoRotulo === 0 || index === dados.rounds.length - 1 || index === indiceAceso ? (
              <text
                key={`rodada-${round}`}
                x={dados.x(index)}
                y={RUN_ROUND_Y}
                textAnchor="middle"
                fontSize="9.5"
                fill={index === indiceAceso ? "#e3ebf3" : "#66788d"}
                fontWeight={index === indiceAceso ? "700" : undefined}
              >
                {t("myTeamTab.history.sport.runRound", { value: round })}
              </text>
            ) : null,
          )}
        </svg>
      </div>
      <p className="mt-1.5 text-[10px] text-text-muted">
        {t(
          dados.porPosicao
            ? "myTeamTab.history.sport.runFieldNotePosition"
            : "myTeamTab.history.sport.runFieldNote",
          { value: dados.outras.length },
        )}
      </p>
    </div>
  );
}

// Geometria da curva. O eixo é INVERTIDO — P1 no topo — porque no automobilismo
// "subir" é diminuir o número, e um gráfico em que a campanha campeã desce
// contraria a leitura antes de qualquer rótulo.
const CURVE_WIDTH = 640;
const CURVE_HEIGHT = 178;
// A calha da esquerda tem que caber o rótulo do eixo E o chip da primeira
// temporada, que é centrado no ponto e portanto invade meia largura de chip para
// fora do desenho. Com a calha estreita, "P4" ficava atrás do chip "P5".
const CURVE_LEFT = 54;
const CURVE_RIGHT = 622;
// O topo reserva a altura de um chip de posição: o ponto de P1 encosta em
// CURVE_TOP, e a etiqueta dele fica acima sem sair do quadro.
const CURVE_TOP = 30;
const CURVE_BOTTOM = 128;
// Onde o desenho de dados acaba e a régua do tempo começa. A linha do eixo separa
// as duas coisas: acima é posição, abaixo é quando aquilo aconteceu.
const CURVE_AXIS = 142;
const CURVE_STRIP_Y = 150;
const CURVE_YEAR_Y = 168;
// Mesma tinta do card que envolve o gráfico. Os pontos são vazados nessa cor para
// a linha não passar por dentro do marcador — sem isso, com quatro temporadas
// seguidas na mesma posição, o ponto some dentro do próprio traço.
const CURVE_SURFACE = "#0f1c2b";
// Chip de posição sobre o ponto. Largura estimada pelo número de caracteres —
// "P1" e "P12" não podem dividir a mesma caixa fixa.
const CHIP_HEIGHT = 17;
const CHIP_GAP = 13;
function chipWidth(texto) {
  return 12 + String(texto).length * 6.4;
}
// Abaixo desta distância entre temporadas os chips começam a se encostar, e a
// etiqueta que devia acelerar a leitura vira uma tarja. Aí só os títulos e a
// última temporada fechada continuam rotulados.
const CHIP_MIN_STEP = 52;

function temporadasDisputadas(seasons) {
  return (Array.isArray(seasons) ? seasons : []).filter((row) => Number(row.races) > 0);
}

// Se há curva para desenhar: duas temporadas disputadas, e pelo menos uma com
// colocação conhecida. Mesmo papel do `campanhaTemDados` — o seletor de vistas
// pergunta antes de desenhar.
function curvaTemDados(seasons) {
  const rows = temporadasDisputadas(seasons);
  return rows.length >= 2 && rows.some((row) => /\d/.test(String(row.position ?? "")));
}

// Curva de campeonato: a posição FINAL por temporada.
//
// Não repete a faixa de top 5 de Records: aquela mede corrida a corrida, esta
// mede o campeonato. Uma equipe regular pode ter poucos top 5 e ainda terminar
// em P3 — quando os dois gráficos discordam, a discordância É a informação.
function ChampionshipCurve({ seasons, seletor = null, seletorModo = null, modo = RUN_MODE_POSITION }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const dados = useMemo(() => {
    if (!curvaTemDados(seasons)) return null;
    const porPosicao = modo === RUN_MODE_POSITION;
    const rows = temporadasDisputadas(seasons);
    const pontos = rows.map((row, index) => {
      const digitos = String(row.position ?? "").match(/\d+/);
      const somados = Number(String(row.points ?? "").replace(/[^\d.-]/g, ""));
      return {
        index,
        year: String(row.year ?? ""),
        category: row.category || "",
        categoryId: row.categoryId || "",
        position: digitos ? Number(digitos[0]) : null,
        points: Number.isFinite(somados) ? somados : null,
      };
    });
    // O valor que o eixo desenha. Em colocação é a posição final; em pontos, o
    // total somado no ano. Pontos de temporadas diferentes NÃO são comparáveis
    // entre categorias — a régua de categorias embaixo do gráfico é o que diz
    // isso, e é por ela que a colocação continua sendo o padrão.
    const valor = (p) => (porPosicao ? p.position : p.points);
    const conhecidos = pontos.map(valor).filter((v) => v !== null && Number.isFinite(v));
    if (!conhecidos.length) return null;

    // `alto` é o topo do desenho e `baixo` o fundo — em colocação o eixo é
    // invertido (P1 no alto), em pontos não. O resto da geometria não precisa
    // saber qual dos dois está em jogo.
    //
    // Em colocação o fundo nunca sobe acima de P6: numa equipe que só terminou
    // em P1 e P2, esticar o eixo entre as duas transformaria um degrau em abismo.
    const pior = Math.max(6, ...conhecidos);
    const alto = porPosicao ? 1 : Math.max(1, ...conhecidos);
    const baixo = porPosicao ? pior : 0;
    const passo = pontos.length > 1 ? (CURVE_RIGHT - CURVE_LEFT) / (pontos.length - 1) : 0;
    const y = (v) => CURVE_TOP + ((v - alto) / (baixo - alto)) * (CURVE_BOTTOM - CURVE_TOP);
    const comXY = pontos.map((p) => ({
      ...p,
      valor: valor(p),
      x: CURVE_LEFT + p.index * passo,
      y: valor(p) === null || !Number.isFinite(valor(p)) ? null : y(valor(p)),
    }));
    // A linha quebra em cada temporada sem posição conhecida (campeonato em
    // andamento, arquivo incompleto): ligar por cima do buraco inventaria um
    // resultado que não existe.
    const trechos = [];
    let atual = [];
    for (const ponto of comXY) {
      if (ponto.y === null) {
        if (atual.length > 1) trechos.push(atual);
        atual = [];
      } else {
        atual.push(ponto);
      }
    }
    if (atual.length > 1) trechos.push(atual);
    return { pontos: comXY, trechos, porPosicao, alto, baixo, pior, passo, y };
  }, [seasons, modo]);

  if (!dados) return null;
  const { pontos, trechos, porPosicao, alto, baixo, passo, y } = dados;
  // As três marcas da régua, no mesmo lugar nos dois modos: topo, meio e fundo
  // da escala. `Set` porque numa escala curta o meio pode coincidir com a ponta.
  const marcas = [...new Set([alto, Math.round((alto + baixo) / 2), baixo])];
  const rotuloMarca = (marca) => (porPosicao ? `P${marca}` : String(Math.round(marca)));
  // O veredito do gráfico, na mesma pílula da campanha: a última temporada
  // fechada. Sem ela, trocar de vista fazia a pílula sumir junto com o resto do
  // cabeçalho, e as duas vistas pareciam blocos diferentes.
  const fechada = [...pontos].reverse().find((ponto) => ponto.position !== null);
  const rotulos = pontos.length > 8 ? 2 : 1;
  // Ids de gradiente precisam ser únicos no documento: o dossiê pode estar aberto
  // ao lado de outro gráfico com os mesmos nomes, e o `url(#...)` pega o primeiro.
  const areaId = `${uid}-area`;
  const glowId = `${uid}-glow`;
  // O chip por temporada só cabe quando as colunas são largas. Numa carreira
  // longa sobram os que carregam informação sozinhos: os títulos e a última
  // temporada já fechada.
  const ultimoFechado = [...pontos].reverse().find((ponto) => ponto.y !== null);
  const chipEmTodos = passo >= CHIP_MIN_STEP;
  return (
    <div>
      {/* O cabeçalho é o MESMO da campanha, slot a slot: rótulo, seletor de
          escala, recorte, e à direita o seletor de métrica com a pílula do
          veredito. Antes cada vista trazia o seu, então trocar de vista renomeava
          o bloco e fazia chrome aparecer do nada — duas telas em vez de duas
          vistas. */}
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5">
          <BlockLabel>{t("myTeamTab.history.sport.championshipTitle")}</BlockLabel>
          {seletor}
          <span className="font-mono text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.curveScope", {
              from: pontos[0]?.year ?? "",
              to: pontos[pontos.length - 1]?.year ?? "",
            })}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {seletorModo}
          {/* A pílula verde do pódio saiu junto com a faixa que ela legendava. Era
              uma terceira cor num gráfico que já tem duas — a da equipe e o ouro
              do título — para marcar uma zona que o eixo com P1 no topo já
              entrega. Esta é outra coisa: o mesmo veredito que a campanha mostra,
              na mesma pílula, aqui referido à última temporada fechada. */}
          {fechada ? (
            <span
              data-testid="team-history-curve-standing"
              className="flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.12em]"
              style={{
                borderColor: "color-mix(in srgb, var(--team) 45%, transparent)",
                backgroundColor: "color-mix(in srgb, var(--team) 10%, transparent)",
                color: "var(--team)",
              }}
            >
              {t("myTeamTab.history.sport.curveStanding", {
                position: fechada.position,
                year: fechada.year,
              })}
            </span>
          ) : null}
        </div>
      </div>
      <div className="mt-2.5 rounded-xl border border-white/[0.06] bg-[#0f1c2b] px-3 py-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
        <svg
          viewBox={`0 0 ${CURVE_WIDTH} ${CURVE_HEIGHT}`}
          className="h-auto w-full"
          data-testid="team-history-curve"
        >
          <defs>
            {/* A área sob a linha é o que dá corpo ao gráfico: sem ela, quatro
                pontos ligados por um fio flutuavam num retângulo vazio. Ela desce
                até o fundo da escala, não até o eixo — a faixa de baixo é a régua
                do tempo, e a mancha da equipe não invade a régua. */}
            <linearGradient id={areaId} x1="0" y1={CURVE_TOP} x2="0" y2={CURVE_BOTTOM} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.42" />
              <stop offset="60%" stopColor="var(--team)" stopOpacity="0.12" />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.01" />
            </linearGradient>
            {/* Brilho da linha. A cor da equipe é o assunto do gráfico, e um traço
                de 2px chapado num fundo escuro não sustenta esse papel. */}
            <filter id={glowId} x="-25%" y="-25%" width="150%" height="150%">
              <feGaussianBlur stdDeviation="2.4" result="borrao" />
              <feMerge>
                <feMergeNode in="borrao" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>

          {/* Uma guia vertical por temporada. É o que transforma o vazio entre os
              pontos em colunas legíveis — sem elas o olho não sabe a qual ano
              pertence cada vértice quando a linha corre reta. */}
          {pontos.map((ponto) => (
            <line
              key={`guia-${ponto.year}`}
              x1={ponto.x}
              y1={CURVE_TOP}
              x2={ponto.x}
              y2={CURVE_AXIS}
              stroke="#ffffff"
              strokeOpacity="0.06"
              strokeDasharray="3 5"
            />
          ))}

          {/* Temporada sem posição fechada (campeonato em andamento, arquivo
              incompleto): a coluna fica marcada, mas nenhum ponto é inventado. */}
          {pontos.map((ponto) =>
            ponto.y === null ? (
              <g key={`aberta-${ponto.year}`}>
                <rect
                  x={ponto.x - Math.max(passo / 2, 6)}
                  y={CURVE_TOP}
                  width={Math.max(passo, 12)}
                  height={CURVE_AXIS - CURVE_TOP}
                  fill="#ffffff"
                  fillOpacity="0.022"
                />
                <line
                  x1={ponto.x}
                  y1={CURVE_TOP}
                  x2={ponto.x}
                  y2={CURVE_AXIS}
                  stroke="#7f93a8"
                  strokeOpacity="0.28"
                  strokeDasharray="2 4"
                />
              </g>
            ) : null,
          )}

          {marcas.map((tick) => (
            <g key={tick}>
              <line
                x1={CURVE_LEFT}
                y1={y(tick)}
                x2={CURVE_RIGHT}
                y2={y(tick)}
                stroke="#ffffff"
                strokeOpacity={tick === alto ? 0.1 : 0.05}
                strokeDasharray={tick === alto ? undefined : "2 5"}
              />
              {/* O rótulo recua o suficiente para o chip da primeira temporada
                  passar por fora dele. Todos com o mesmo peso: isto é a régua do
                  desenho, e régua não disputa atenção com a linha. O troféu que
                  ficava ao lado do P1 saiu — ele anunciava um marco onde só há
                  escala, e o título de verdade já é o ponto dourado na curva. */}
              <text
                x={CURVE_LEFT - 17}
                y={y(tick) + 3.4}
                textAnchor="end"
                fontSize="10"
                fontWeight="500"
                fill="#7c8ea3"
              >
                {rotuloMarca(tick)}
              </text>
            </g>
          ))}

          {/* Eixo vertical: fecha o desenho à esquerda e separa a escala da área
              de dados. */}
          <line x1={CURVE_LEFT} y1={CURVE_TOP - 6} x2={CURVE_LEFT} y2={CURVE_AXIS} stroke="#ffffff" strokeOpacity="0.12" />

          {/* Preenchimento e traço vêm do MESMO trecho: onde a linha quebra por
              falta de dado, a mancha quebra junto. */}
          {trechos.map((trecho) => (
            <path
              key={`area-${trecho[0].year}-${trecho.length}`}
              d={`M ${trecho[0].x},${CURVE_BOTTOM} ${trecho.map((p) => `L ${p.x},${p.y}`).join(" ")} L ${
                trecho[trecho.length - 1].x
              },${CURVE_BOTTOM} Z`}
              fill={`url(#${areaId})`}
            />
          ))}
          {trechos.map((trecho) => (
            <polyline
              key={`${trecho[0].year}-${trecho.length}`}
              points={trecho.map((p) => `${p.x},${p.y}`).join(" ")}
              fill="none"
              stroke="var(--team)"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              filter={`url(#${glowId})`}
            />
          ))}

          {pontos.map((ponto) => {
            if (ponto.y === null) return null;
            const campeao = ponto.position === 1;
            const cor = campeao ? MEDAL_COLORS.first : "var(--team)";
            return (
              <g key={ponto.year}>
                {/* Halo em todos, forte no título: o ponto tem que se destacar da
                    própria linha, mas só o campeão merece ser achado de longe. */}
                <circle cx={ponto.x} cy={ponto.y} r={campeao ? 9 : 7} fill={cor} fillOpacity={campeao ? 0.22 : 0.12} />
                {/* Recorte na cor do card — o marcador fica por cima da linha em
                    vez de se dissolver nela. */}
                <circle cx={ponto.x} cy={ponto.y} r={campeao ? 5.8 : 4.8} fill={CURVE_SURFACE} />
                <circle
                  data-season={ponto.year}
                  cx={ponto.x}
                  cy={ponto.y}
                  r={campeao ? 4.4 : 3.4}
                  fill={cor}
                  stroke={cor}
                  strokeOpacity="0.4"
                  strokeWidth="2"
                >
                  {/* O balão traz as DUAS leituras em qualquer modo: a métrica
                      escolhida manda no eixo, não no que se pode perguntar de um
                      ponto. */}
                  <title>
                    {t("myTeamTab.history.sport.curveTooltip", {
                      year: ponto.year,
                      category: ponto.category,
                      position: ponto.position ?? "—",
                      points: ponto.points ?? 0,
                    })}
                  </title>
                </circle>
              </g>
            );
          })}

          {/* Chip de posição sobre o ponto. Ler a colocação exata dependia de mirar
              o vértice contra a grade, e a grade só tem três marcas — entre P4 e P6
              não havia como saber se aquilo era P5. */}
          {pontos.map((ponto) => {
            if (ponto.y === null) return null;
            const campeao = ponto.position === 1;
            if (!chipEmTodos && !campeao && ponto !== ultimoFechado) return null;
            const texto = porPosicao ? `P${ponto.position}` : String(Math.round(ponto.points ?? 0));
            const largura = chipWidth(texto);
            // O chip fica acima do ponto; onde não há teto — P1 encosta no topo —
            // ele desce para baixo do marcador em vez de sair do quadro.
            const acima = ponto.y - CHIP_GAP - CHIP_HEIGHT / 2 >= CHIP_HEIGHT / 2;
            const cy = acima ? ponto.y - CHIP_GAP - CHIP_HEIGHT / 2 : ponto.y + CHIP_GAP + CHIP_HEIGHT / 2;
            const cx = Math.min(Math.max(ponto.x, largura / 2 + 2), CURVE_WIDTH - largura / 2 - 2);
            const cor = campeao ? MEDAL_COLORS.first : "var(--team)";
            return (
              <g key={`chip-${ponto.year}`} data-chip={ponto.year}>
                <rect
                  x={cx - largura / 2}
                  y={cy - CHIP_HEIGHT / 2}
                  width={largura}
                  height={CHIP_HEIGHT}
                  rx={4.5}
                  fill={CURVE_SURFACE}
                  fillOpacity="0.95"
                  stroke={cor}
                  strokeOpacity="0.55"
                />
                <text
                  x={cx}
                  y={cy + 3.6}
                  textAnchor="middle"
                  fontSize="10"
                  fontWeight="700"
                  letterSpacing="0.02em"
                  fill={cor}
                >
                  {texto}
                </text>
              </g>
            );
          })}

          {/* Eixo: o corte entre "onde terminou" e "quando foi". */}
          <line x1={CURVE_LEFT} y1={CURVE_AXIS} x2={CURVE_RIGHT} y2={CURVE_AXIS} stroke="#ffffff" strokeOpacity="0.09" />

          {/* Mesma tira de categoria da faixa de Records, aqui embaixo da curva:
              a queda de uma temporada quase sempre tem a promoção como causa, e
              as duas coisas precisam ser lidas juntas. O vão entre os blocos é
              largo de propósito — colada, a tira virava uma barra contínua
              atravessando o gráfico e competia com a própria curva. */}
          {pontos.map((ponto) => {
            // A tira vive DENTRO do eixo: nas pontas ela é meio bloco, senão
            // avançava por cima dos rótulos de posição à esquerda e sangrava para
            // fora do card à direita.
            const esquerda = Math.max(ponto.x - Math.max(passo / 2 - 5, 5), CURVE_LEFT);
            const direita = Math.min(ponto.x + Math.max(passo / 2 - 5, 5), CURVE_RIGHT);
            return (
              <rect
                key={`cat-${ponto.year}`}
                data-category={ponto.categoryId || undefined}
                x={esquerda}
                y={CURVE_STRIP_Y}
                width={Math.max(direita - esquerda, 8)}
                height={3}
                rx={1.5}
                fill={ponto.categoryId ? getCategoryColor(ponto.categoryId) : "transparent"}
                fillOpacity={ponto.y === null ? 0.3 : 0.85}
              />
            );
          })}
          {pontos.map((ponto) =>
            ponto.index % rotulos === 0 ? (
              <text
                key={`ano-${ponto.year}`}
                x={ponto.x}
                y={CURVE_YEAR_Y}
                textAnchor="middle"
                fontSize="10.5"
                fontWeight="600"
                fill={ponto.y === null ? "#4f6076" : "#93a7bb"}
              >
                {ponto.year}
              </text>
            ) : null,
          )}
        </svg>
      </div>
    </div>
  );
}

// Assinatura de resultados: TODAS as corridas repartidas por faixa de colocação.
//
// Records dá a taxa de pódio; isto dá a forma dela. Duas equipes com 60% de
// pódio desenham diferente aqui — uma converte em vitória, a outra vive em
// terceiro —, e a taxa sozinha não separa as duas.
// Fatia mínima, em % do total, para o número caber dentro da barra. Abaixo
// disso a caixa tem menos que a largura de "1 (1%)" e o texto sai cortado.
const FAIXA_MIN_ROTULO = 5;

function rotuloFaixa(t, faixa) {
  return t("myTeamTab.history.sport.spreadValue", { value: faixa.value, percent: faixa.percent });
}

// Preto sobre ouro e prata, branco sobre os azuis escuros. A paleta das faixas
// vai de #f2c46d a #141f2c, e um texto de cor fixa some em metade delas.
function corDeTextoSobre(hex) {
  const cor = String(hex).replace("#", "");
  if (cor.length !== 6) return "#eaf1f8";
  const r = parseInt(cor.slice(0, 2), 16);
  const g = parseInt(cor.slice(2, 4), 16);
  const b = parseInt(cor.slice(4, 6), 16);
  const luminancia = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luminancia > 0.6 ? "#0d1622" : "#eaf1f8";
}

function ResultSpread({ spread }) {
  const { t } = useTranslation();
  if (!spread || spread.races <= 0) return null;
  const faixas = [
    { id: "first", value: spread.first, color: PLACEMENT_COLORS.first },
    { id: "podium", value: spread.podium, color: PLACEMENT_COLORS.second },
    { id: "nearMiss", value: spread.nearMiss, color: PLACEMENT_COLORS.nearMiss },
    { id: "topTen", value: spread.topTen, color: PLACEMENT_COLORS.topTen },
    { id: "outside", value: spread.outside, color: PLACEMENT_COLORS.outside },
  ]
    .filter((faixa) => faixa.value > 0)
    .map((faixa) => {
      // A proporção é o que a barra desenha; o número é o que ela esconde. "24"
      // não diz se é a equipe toda ou um terço dela, e "73%" não diz de quantas
      // corridas — os dois juntos fecham a leitura sem exigir a legenda.
      const share = (faixa.value / spread.races) * 100;
      return { ...faixa, percent: Math.round(share), cabeNaBarra: share >= FAIXA_MIN_ROTULO };
    });
  if (!faixas.length) return null;
  return (
    <div>
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.resultSpread")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.spreadRaces", { value: spread.races })}
        </span>
      </div>
      <div className="mt-2.5 flex h-7 overflow-hidden rounded-md" data-testid="team-history-spread">
        {faixas.map((faixa) => (
          <Tooltip
            key={faixa.id}
            texto={`${t(`myTeamTab.history.sport.spread.${faixa.id}`)} · ${rotuloFaixa(t, faixa)}`}
          >
            <span
              data-band={faixa.id}
              className="flex items-center justify-center overflow-hidden whitespace-nowrap px-1 font-mono text-[10px] font-semibold"
              style={{
                flexGrow: faixa.value,
                flexBasis: 0,
                backgroundColor: faixa.color,
                color: corDeTextoSobre(faixa.color),
              }}
            >
              {/* Faixa estreita demais recorta o número no meio e vira sujeira.
                  Abaixo do limite ela fica só como cor, e a contagem aparece na
                  legenda — nenhum dado se perde por não caber na barra. */}
              {faixa.cabeNaBarra ? rotuloFaixa(t, faixa) : null}
            </span>
          </Tooltip>
        ))}
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        {faixas.map((faixa) => (
          <MedalKey
            key={faixa.id}
            color={faixa.color}
            label={
              faixa.cabeNaBarra
                ? t(`myTeamTab.history.sport.spread.${faixa.id}`)
                : `${t(`myTeamTab.history.sport.spread.${faixa.id}`)} · ${rotuloFaixa(t, faixa)}`
            }
          />
        ))}
      </div>
    </div>
  );
}

// Cores da confiabilidade. Chegar ao fim é o estado bom e usa o verde do pódio;
// as duas causas de abandono precisam ser distinguíveis entre si porque a
// diferença entre elas é a pergunta do bloco — carro ruim ou piloto afoito?
const RELIABILITY_COLORS = {
  finished: "#3fbf7f",
  mechanical: "#e5793a",
  driverError: MEDAL_COLORS.nearMiss,
  other: "#3b4b5e",
};

// Confiabilidade: quantas largadas viraram chegada, e o que levou o resto ao box.
//
// É o buraco que a assinatura de resultados deixa. Lá, abandonar na volta 2 e
// terminar em 14º caem os dois em "fora do top 10" — e são coisas opostas: uma é
// o carro quebrando, a outra é o carro andando devagar.
function ReliabilityPanel({ reliability, compacto = false }) {
  const { t } = useTranslation();
  if (!reliability || reliability.races <= 0) return null;
  const faixas = [
    { id: "finished", value: reliability.finished },
    { id: "mechanical", value: reliability.mechanical },
    { id: "driverError", value: reliability.driverError },
    { id: "other", value: reliability.other },
  ].filter((faixa) => faixa.value > 0);
  // A comparação com o grupo é o que dá escala ao número: 88% pode ser ótimo ou
  // medíocre conforme o carro, e só o grupo responde isso.
  const delta = reliability.finishRate - reliability.groupFinishRate;

  // Modo compacto: a MESMA gramática da assinatura de resultados — rótulo,
  // barra de 100% e legenda, nas mesmas alturas.
  //
  // No arranjo arrumado os dois blocos dividem uma linha, e enquanto este aqui
  // era um cartão com moldura e um número de 24px ao lado da barra, os dois liam
  // como dois gráficos de telas diferentes encostados: as barras em alturas
  // distintas, uma dentro de caixa e outra solta. Aqui a taxa de chegadas vira um
  // valor pequeno ao lado do rótulo e a barra desce para a mesma linha da outra —
  // a informação é idêntica, o peso é que deixa de brigar.
  if (compacto) {
    const rotulo = (faixa) => `${t(`myTeamTab.history.sport.rel${faixa.id[0].toUpperCase()}${faixa.id.slice(1)}`)} · ${faixa.value}`;
    return (
      <div data-testid="team-history-reliability">
        <div className="flex items-baseline gap-2">
          <BlockLabel>{t("myTeamTab.history.sport.reliability")}</BlockLabel>
          <strong
            className="font-mono text-[11px] font-semibold leading-none"
            style={{ color: RELIABILITY_COLORS.finished }}
            data-testid="team-history-finish-rate"
          >
            {`${reliability.finishRate}%`}
          </strong>
          <span className="font-mono text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.reliabilityRaces", { value: reliability.races })}
          </span>
        </div>
        <div className="mt-2.5 flex h-7 overflow-hidden rounded-md">
          {faixas.map((faixa) => {
            const share = (faixa.value / reliability.races) * 100;
            return (
              <Tooltip key={faixa.id} texto={rotulo(faixa)}>
                <span
                  data-band={faixa.id}
                  className="flex items-center justify-center overflow-hidden whitespace-nowrap px-1 font-mono text-[10px] font-semibold"
                  style={{
                    flexGrow: faixa.value,
                    flexBasis: 0,
                    backgroundColor: RELIABILITY_COLORS[faixa.id],
                    color: corDeTextoSobre(RELIABILITY_COLORS[faixa.id]),
                  }}
                >
                  {/* Só a faixa que cabe mostra o número, pela mesma regra da
                      assinatura: recortado no meio ele vira sujeira. */}
                  {share >= FAIXA_MIN_ROTULO ? faixa.value : null}
                </span>
              </Tooltip>
            );
          })}
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-text-muted">
          {faixas.map((faixa) => (
            <MedalKey key={faixa.id} color={RELIABILITY_COLORS[faixa.id]} label={rotulo(faixa)} />
          ))}
          <span
            className="ml-auto font-mono"
            style={{ color: delta >= 0 ? RELIABILITY_COLORS.finished : RELIABILITY_COLORS.mechanical }}
          >
            {t("myTeamTab.history.sport.finishRateVs", { value: reliability.groupFinishRate })}
          </span>
        </div>
        {reliability.worstPart ? (
          <p className="mt-1.5 text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.relWorstPart", { part: reliability.worstPart })}
          </p>
        ) : null}
      </div>
    );
  }

  return (
    <div data-testid="team-history-reliability">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.reliability")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.reliabilityRaces", { value: reliability.races })}
        </span>
      </div>
      <div className="mt-2.5 flex items-center gap-4 rounded-xl border border-white/[0.06] bg-[#0f1c2b] px-4 py-3">
        <div className="shrink-0">
          <strong
            className="block font-mono text-2xl leading-none tracking-[-0.02em]"
            style={{ color: RELIABILITY_COLORS.finished }}
            data-testid="team-history-finish-rate"
          >
            {`${reliability.finishRate}%`}
          </strong>
          <span className="mt-1 block text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.finishRate")}
          </span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex h-5 overflow-hidden rounded-md">
            {faixas.map((faixa) => (
              <Tooltip
                key={faixa.id}
                texto={`${t(`myTeamTab.history.sport.rel${faixa.id[0].toUpperCase()}${faixa.id.slice(1)}`)} · ${faixa.value}`}
              >
                <span
                  data-band={faixa.id}
                  style={{ flexGrow: faixa.value, flexBasis: 0, backgroundColor: RELIABILITY_COLORS[faixa.id] }}
                />
              </Tooltip>
            ))}
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-text-muted">
            {faixas.map((faixa) => (
              <MedalKey
                key={faixa.id}
                color={RELIABILITY_COLORS[faixa.id]}
                label={`${t(`myTeamTab.history.sport.rel${faixa.id[0].toUpperCase()}${faixa.id.slice(1)}`)} · ${faixa.value}`}
              />
            ))}
            {/* A média do grupo fica na mesma linha da legenda, e não ao lado do
                número grande: ela é referência, não manchete. O sinal do delta
                vira cor — acima da média é verde, abaixo é laranja. */}
            <span
              className="ml-auto font-mono"
              style={{ color: delta >= 0 ? RELIABILITY_COLORS.finished : RELIABILITY_COLORS.mechanical }}
            >
              {t("myTeamTab.history.sport.finishRateVs", { value: reliability.groupFinishRate })}
            </span>
          </div>
        </div>
      </div>
      {reliability.worstPart ? (
        <p className="mt-1.5 text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.relWorstPart", { part: reliability.worstPart })}
        </p>
      ) : null}
    </div>
  );
}

// O elo entre a galeria de pilotos e o ranking dos melhores.
//
// Os dois blocos listam as MESMAS pessoas por critérios diferentes: a galeria em
// ordem de ano e por vaga, o ranking por currículo. Achar num o nome que está no
// outro é o gesto mais repetido dessa seção — e o mais caro, porque uma equipe
// antiga tem quinze passagens e o ranking corta em dez.
//
// O elo ACENDE e só. Uma versão anterior rolava a página até o par quando ele
// estava fora do quadro, para garantir que o realce fosse visto; a tela se
// mexendo sozinha sob o cursor é pior do que o problema que resolve — quem está
// lendo perde o lugar, e o gesto de passar o mouse deixa de ser inofensivo.

// Pilotos que passaram pela equipe, repartidos pelas DUAS vagas. É o único bloco
// do dossiê que fala de gente — todo o resto trata a equipe como um carro só — e,
// em duas colunas, também responde quanto essa casa troca de piloto: uma que
// manteve o mesmo titular por seis anos e outra que troca todo ano desenham
// diferente antes de qualquer número ser lido.
function TeamLineup({ lineup, pilotoAceso = null, onAcenderPiloto = null }) {
  const { t } = useTranslation();
  if (!lineup?.length) return null;
  // Vagas 1 e 2 sempre lado a lado; a faixa de "outras passagens" só existe
  // quando alguém correu sem constar como titular de temporada arquivada.
  const colunas = [1, 2]
    .map((slot) => ({ slot, itens: lineup.filter((item) => item.slot === slot) }))
    .filter((coluna) => coluna.itens.length > 0);
  const avulsos = lineup.filter((item) => item.slot !== 1 && item.slot !== 2);
  return (
    <div data-testid="team-history-lineup">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.alumni")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.lineupCount", { value: lineup.length })}
        </span>
      </div>
      <div className="mt-2.5 grid gap-x-3 gap-y-3 sm:grid-cols-2">
        {colunas.map((coluna) => (
          <div key={coluna.slot} data-slot={coluna.slot}>
            <span className="block text-[10px] font-semibold text-text-muted">
              {t(`myTeamTab.history.sport.lineupSlot${coluna.slot}`)}
            </span>
            <LineupColumn itens={coluna.itens} pilotoAceso={pilotoAceso} onAcenderPiloto={onAcenderPiloto} />
          </div>
        ))}
        {avulsos.length ? (
          <div data-slot="0" className="sm:col-span-2">
            <span className="block text-[10px] font-semibold text-text-muted">
              {t("myTeamTab.history.sport.lineupSlotOther")}
            </span>
            <LineupColumn itens={avulsos} pilotoAceso={pilotoAceso} onAcenderPiloto={onAcenderPiloto} />
          </div>
        ) : null}
      </div>
    </div>
  );
}

// Uma coluna da galeria. As passagens vêm em ordem cronológica do backend, e a
// coluna só se lê como sucessão se essa ordem sobreviver ao desenho.
function LineupColumn({ itens, pilotoAceso = null, onAcenderPiloto = null }) {
  const { t } = useTranslation();
  return (
    <ul className="mt-1.5 grid gap-1.5">
      {itens.map((piloto) => {
          // A linha é sempre a mesma leitura: quanto correu, até onde chegou.
          // O melhor resultado vale para TODO mundo — era ele que separava quem
          // chegou perto de quem nunca ameaçou, e a contagem de pódios tomava o
          // lugar dele em quem tinha pódio. Pior: quem não tinha nada exibia
          // "0V · 0P", que é ruído com aparência de dado.
          // Titular que ainda não largou (save recém-criado, antes da rodada 1)
          // entra na galeria assim mesmo — mas "0 corridas · " é ruído: o que a
          // linha tem a dizer é que a passagem começou e a pista ainda não veio.
          const feitos =
            piloto.races > 0
              ? [t("myTeamTab.history.sport.alumniRaces", { value: piloto.races })]
              : [t("myTeamTab.history.sport.alumniNoRaces")];
          if (piloto.bestPosition > 0) {
            feitos.push(t("myTeamTab.history.sport.alumniBest", { value: piloto.bestPosition }));
          }
          // Vencer é a única coisa que a colocação sozinha não conta: "melhor P1"
          // não diz se foi uma vez ou dez. Só aparece quando houve vitória.
          if (piloto.wins > 0) {
            feitos.push(t("myTeamTab.history.sport.alumniWins", { value: piloto.wins }));
          }
          return (
            <li
              key={`${piloto.driverId}-${piloto.firstYear}`}
              data-driver={piloto.driverId}
              data-player={piloto.isPlayer ? "true" : undefined}
              data-current={piloto.stillHere ? "true" : undefined}
              data-aceso={pilotoAceso === piloto.driverId ? "true" : undefined}
              onMouseEnter={() => onAcenderPiloto?.(piloto.driverId)}
              onMouseLeave={() => onAcenderPiloto?.(null)}
              // Quem está na equipe HOJE é o fim da coluna e o começo da leitura:
              // ganha faixa lateral e fundo na cor da casa. Passagem encerrada
              // fica em cinza — a diferença entre as duas é a informação, e
              // "ainda na equipe" escrito em texto era fácil demais de perder no
              // meio de oito linhas iguais.
              className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-0.5 rounded-lg border px-3 py-2 transition-[box-shadow] ${
                piloto.stillHere
                  ? "border-l-2 border-[color-mix(in_srgb,var(--team)_50%,transparent)] border-l-[color:var(--team)] bg-[color-mix(in_srgb,var(--team)_10%,#0f1c2b)]"
                  : piloto.isPlayer
                    ? "border-[color-mix(in_srgb,var(--team)_45%,transparent)] bg-[color-mix(in_srgb,var(--team)_12%,#0f1c2b)]"
                    : "border-white/[0.06] bg-[#0f1c2b]"
              } ${pilotoAceso === piloto.driverId ? "ring-1 ring-white/45" : ""}`}
            >
              {/* A bandeira ocupa as duas linhas do cartão, à esquerda de tudo:
                  é o retrato do piloto que a galeria não tem. Vem do país porque
                  é o único traço visual que o save guarda dele — e é ele que faz
                  a coluna se ler como gente, e não como oito linhas de texto. */}
              <Tooltip texto={piloto.nationality || undefined}>
                <span
                  className="row-span-2 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-white/[0.04] ring-1 ring-inset ring-white/[0.07]"
                  data-nationality={piloto.nationality || undefined}
                >
                  <FlagIcon nacionalidade={piloto.nationality} />
                </span>
              </Tooltip>
              <span className="flex min-w-0 items-center gap-1.5">
                <strong className="truncate text-xs font-semibold text-text-primary">{piloto.name}</strong>
                {piloto.isPlayer ? (
                  <span className="shrink-0 rounded px-1 py-px text-[10px] font-semibold text-[color:var(--team)] ring-1 ring-[color:var(--team)]/50">
                    {t("myTeamTab.history.sport.alumniYou")}
                  </span>
                ) : null}
                {piloto.stillHere ? (
                  <span className="shrink-0 rounded bg-[color:var(--team)] px-1 py-px text-[10px] font-semibold text-[#07101d]">
                    {t("myTeamTab.history.sport.lineupCurrent")}
                  </span>
                ) : null}
              </span>
              <span className="shrink-0 font-mono text-[10px] text-text-muted">
                {piloto.firstYear === piloto.lastYear
                  ? t("myTeamTab.history.sport.alumniOneYear", { first: piloto.firstYear })
                  : t("myTeamTab.history.sport.alumniYears", { first: piloto.firstYear, last: piloto.lastYear })}
              </span>
              <span className="truncate font-mono text-[10px] text-text-secondary">{feitos.join(" · ")}</span>
              {/* Quem foi para outra equipe aparece COM a equipe: brasão e cor.
                  "Hoje na GT Pro" dizia a categoria e escondia o que interessa —
                  para onde o piloto que a casa formou acabou indo. */}
              {piloto.currentTeamName ? (
                <span
                  className="flex min-w-0 items-center justify-end gap-1.5 text-[10px]"
                  data-current-team={piloto.currentTeamName}
                  style={{ color: piloto.currentTeamColor || undefined }}
                >
                  {/* Sem `scale`: transform não muda o espaço que o elemento
                      ocupa no fluxo, então o brasão de 36px continuava reservando
                      36px dentro de uma caixa de 20 e vazava por cima do nome. O
                      tamanho vem do próprio TeamLogoMark. */}
                  <TeamLogoMark teamName={piloto.currentTeamName} color={piloto.currentTeamColor} size="xs" />
                  <span className="truncate font-semibold">{piloto.currentTeamName}</span>
                </span>
              ) : piloto.stillHere ? null : (
                // Quem ficou não repete "ainda na equipe" aqui: o selo ao lado do
                // nome já diz, e a mesma frase duas vezes na linha só ocupa o
                // lugar de uma informação que não existe.
                <span className="truncate text-right text-[10px] text-text-muted">{piloto.currentLabel}</span>
              )}
            </li>
          );
        })}
    </ul>
  );
}

// Quantos nomes o pódio da casa aguenta. Dez é o corte clássico de tabela de
// recordes, e numa equipe antiga ele alcança quem correu na década anterior — a
// galeria acima lista todo mundo, mas em ordem de ano, onde ninguém compara.
const BEST_DRIVERS_LIMIT = 10;

// Cor da posição no ranking: as três primeiras na mesma paleta de medalha que os
// degraus de Records usam, o resto apagado. Um número só muda de significado
// entre as telas se mudar de cor — então não muda.
const BEST_RANK_COLORS = [MEDAL_COLORS.first, MEDAL_COLORS.second, MEDAL_COLORS.third];

// O ranking dos pilotos que vestiram a equipe.
//
// A galeria logo acima conta a SUCESSÃO — quem veio depois de quem, em duas
// colunas. Ela não responde quem foi o melhor: os números estão lá, espalhados
// por oito linhas em ordem de ano, e comparar dois deles é trabalho do leitor.
// Aqui a ordem é a resposta.
//
// A conta é por PILOTO, e não por passagem: quem saiu e voltou tem dois mandatos
// na galeria (é assim que a sucessão se lê), mas um só currículo pela casa —
// somar os dois é o que impede a mesma pessoa de aparecer duas vezes no pódio,
// cada metade abaixo de quem ela na verdade supera.
function bestDriversRanking(lineup) {
  const porPiloto = new Map();
  for (const term of lineup) {
    // Contrato anunciado que nunca virou pista não entra: o titular de hoje sem
    // corrida aparece na galeria (é onde se confere quem está no carro), mas um
    // ranking de quem correu não tem o que fazer com quem não correu.
    if (!(term.races > 0)) continue;
    const acumulado = porPiloto.get(term.driverId);
    if (!acumulado) {
      porPiloto.set(term.driverId, {
        driverId: term.driverId,
        name: term.name,
        nationality: term.nationality,
        isPlayer: term.isPlayer,
        stillHere: term.stillHere,
        races: term.races,
        titles: term.titles,
        wins: term.wins,
        podiums: term.podiums,
        bestPosition: term.bestPosition,
        firstYear: term.firstYear,
        lastYear: term.lastYear,
      });
      continue;
    }
    acumulado.races += term.races;
    acumulado.titles += term.titles;
    acumulado.wins += term.wins;
    acumulado.podiums += term.podiums;
    // Zero é "nunca teve colocação", e não a melhor delas.
    if (term.bestPosition > 0 && (acumulado.bestPosition === 0 || term.bestPosition < acumulado.bestPosition)) {
      acumulado.bestPosition = term.bestPosition;
    }
    acumulado.stillHere = acumulado.stillHere || term.stillHere;
    acumulado.nationality = acumulado.nationality || term.nationality;
    if (term.firstYear && term.firstYear < acumulado.firstYear) acumulado.firstYear = term.firstYear;
    if (term.lastYear && term.lastYear > acumulado.lastYear) acumulado.lastYear = term.lastYear;
  }

  // TÍTULO primeiro, e não vitória. Vencer domingo e vencer o ano não são a
  // mesma moeda em escala diferente: um campeão da casa com seis vitórias vale
  // mais para a história dela que um piloto de quinze vitórias que nunca levou
  // o campeonato — e era exatamente isso que a lista dizia ao contrário.
  // Nenhum peso relativo resolveria: quantas vitórias valem um título é uma
  // pergunta sem resposta, e a ordem lexicográfica não precisa dela.
  return [...porPiloto.values()].sort((a, b) => {
    if (b.titles !== a.titles) return b.titles - a.titles;
    if (b.wins !== a.wins) return b.wins - a.wins;
    if (b.podiums !== a.podiums) return b.podiums - a.podiums;
    // A melhor colocação não é mais coluna, mas continua desempatando: entre dois
    // pilotos sem pódio, quem chegou em quarto fez mais que quem nunca passou de
    // décimo, e sem isso o fundo da tabela sairia em ordem alfabética.
    const melhorA = a.bestPosition > 0 ? a.bestPosition : Number.POSITIVE_INFINITY;
    const melhorB = b.bestPosition > 0 ? b.bestPosition : Number.POSITIVE_INFINITY;
    if (melhorA !== melhorB) return melhorA - melhorB;
    if (b.races !== a.races) return b.races - a.races;
    return a.name.localeCompare(b.name);
  });
}

// As três colunas de números do ranking, na ordem em que desempatam a lista.
// Largura fixa e conteúdo alinhado à direita: é o que faz a coluna se ler de
// cima para baixo, que é a leitura que o bloco existe para dar.
// O título abre a fila porque é o primeiro critério de ordem, e leva a taça
// junto do número: as duas colunas de ouro seguidas (título e vitória) seriam
// indistinguíveis de relance, e a diferença entre elas é o assunto do bloco.
//
// A melhor colocação saiu: ela era redundante com as colunas à esquerda — quem
// tem pódio nunca terá "melhor" pior que P3, e quem tem vitória sempre marca P1.
// Só dizia algo de quem não subiu no pódio, e continua dizendo, como critério de
// desempate invisível. No lugar dela entram as CORRIDAS, que dão a escala das
// outras três (seis títulos em quarenta corridas não é seis em duzentas) e, por
// serem o filtro de entrada do ranking, garantem que nenhuma linha fique só com
// travessões.
const BEST_COLUMNS = [
  { id: "titles", label: "bestColTitles", width: "w-9", color: MEDAL_COLORS.first, trophy: true, value: (p) => p.titles },
  { id: "wins", label: "bestColWins", width: "w-8", color: MEDAL_COLORS.first, value: (p) => p.wins },
  { id: "podiums", label: "bestColPodiums", width: "w-8", color: MEDAL_COLORS.second, value: (p) => p.podiums },
  { id: "races", label: "bestColRaces", width: "w-10", color: null, value: (p) => p.races },
];

function BestDrivers({ lineup, pilotoAceso = null, onAcenderPiloto = null }) {
  const { t } = useTranslation();
  const ranking = useMemo(() => bestDriversRanking(lineup ?? []), [lineup]);
  // Um nome sozinho não é ranking: a galeria acima já o mostra, com mais dado.
  if (ranking.length < 2) return null;
  const primeiros = ranking.slice(0, BEST_DRIVERS_LIMIT);

  return (
    <div data-testid="team-history-best-drivers">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.bestDrivers")}</BlockLabel>
        <span className="text-[10px] text-text-muted">{t("myTeamTab.history.sport.bestDriversScope")}</span>
      </div>
      {/* O cabeçalho das colunas paga uma linha e devolve o que a prosa
          "1 vitória · 2 pódios" custava em cada uma das cinco: com ele, os
          números viram três colunas comparáveis de cima para baixo, e a ordem
          da lista fica visível em vez de precisar ser acreditada.
          `pr-[13px]` = o padding da linha mais a borda de 1px dela. */}
      <div className="mt-2.5 flex justify-end gap-3 pr-[13px] text-[9px] uppercase tracking-[0.08em] text-text-muted">
        {BEST_COLUMNS.map((coluna) => (
          <span key={coluna.id} className={`${coluna.width} text-right`}>
            {t(`myTeamTab.history.sport.${coluna.label}`)}
          </span>
        ))}
      </div>
      <ol className="mt-1 grid gap-1.5">
        {primeiros.map((piloto, index) => {
          const cor = BEST_RANK_COLORS[index] || MEDAL_COLORS.nearMiss;
          return (
            <li
              key={piloto.driverId}
              data-driver={piloto.driverId}
              data-rank={index + 1}
              data-player={piloto.isPlayer ? "true" : undefined}
              data-aceso={pilotoAceso === piloto.driverId ? "true" : undefined}
              onMouseEnter={() => onAcenderPiloto?.(piloto.driverId)}
              onMouseLeave={() => onAcenderPiloto?.(null)}
              className={`grid grid-cols-[18px_auto_minmax(0,1fr)_auto] items-center gap-x-3 rounded-lg border px-3 py-2 transition-[box-shadow] ${
                piloto.isPlayer
                  ? "border-[color-mix(in_srgb,var(--team)_45%,transparent)] bg-[color-mix(in_srgb,var(--team)_12%,#0f1c2b)]"
                  : "border-white/[0.06] bg-[#0f1c2b]"
              } ${pilotoAceso === piloto.driverId ? "ring-1 ring-white/45" : ""}`}
            >
              <strong className="text-center font-mono text-sm leading-none" style={{ color: cor }}>
                {index + 1}
              </strong>
              <FlagIcon nacionalidade={piloto.nationality} />
              <span className="flex min-w-0 items-center gap-1.5">
                <strong className="truncate text-xs font-semibold text-text-primary">{piloto.name}</strong>
                {piloto.isPlayer ? (
                  <span className="shrink-0 rounded px-1 py-px text-[10px] font-semibold text-[color:var(--team)] ring-1 ring-[color:var(--team)]/50">
                    {t("myTeamTab.history.sport.alumniYou")}
                  </span>
                ) : null}
                {piloto.stillHere ? (
                  <span className="shrink-0 rounded bg-[color:var(--team)] px-1 py-px text-[10px] font-semibold text-[#07101d]">
                    {t("myTeamTab.history.sport.lineupCurrent")}
                  </span>
                ) : null}
                <span className="shrink-0 font-mono text-[10px] text-text-muted">
                  {piloto.firstYear === piloto.lastYear
                    ? t("myTeamTab.history.sport.alumniOneYear", { first: piloto.firstYear })
                    : t("myTeamTab.history.sport.alumniYears", { first: piloto.firstYear, last: piloto.lastYear })}
                </span>
              </span>
              {/* Números em coluna, e não em prosa. A barra que morava aqui
                  media PÓDIOS, mas a lista ordena por título e vitória antes
                  disso: o 3º colocado, com quatro pódios e nada mais, ganhava a
                  barra mais longa e a figura desmentia a ordem que ela deveria
                  explicar. Nenhum comprimento resolve isso — o que ordena são
                  quatro critérios, e barra tem um eixo só. */}
              <span className="flex justify-self-end gap-3 font-mono text-[11px] text-text-secondary">
                {BEST_COLUMNS.map((coluna) => {
                  const valor = coluna.value(piloto);
                  if (!(valor > 0)) {
                    // Zero vira travessão apagado: "0" alinhado com os outros
                    // números pesa como dado e é ausência de dado.
                    return (
                      <span key={coluna.id} data-col={coluna.id} className={`${coluna.width} text-right text-text-muted/50`}>
                        {t("myTeamTab.history.defaults.dash")}
                      </span>
                    );
                  }
                  return (
                    <span
                      key={coluna.id}
                      data-col={coluna.id}
                      className={`${coluna.width} flex items-center justify-end gap-1`}
                      style={{ color: coluna.color || undefined }}
                    >
                      {coluna.trophy ? <Trophy size={10} strokeWidth={2} aria-hidden="true" /> : null}
                      {valor}
                    </span>
                  );
                })}
              </span>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

// A seção Esportivo tem UM arranjo. O empilhamento original — seis blocos irmãos
// com o mesmo peso — viveu um tempo atrás de um botão "Layout clássico" no canto
// superior, e o botão custava uma faixa inteira no topo da seção para oferecer
// uma tela pior. Os grupos abaixo têm o mesmo conteúdo, item por item.
//
// O que a seção NÃO desenha, e por quê: temporadas disputadas é âncora do
// cabeçalho, visível em qualquer aba; taxa de pódio e de vitória são cards de
// Records, e lá vêm com a média do grupo e a posição no ranking; a tabela
// temporada a temporada era a faixa de top 5 de Records em números, com POS
// virando a curva de campeonato e PTS sendo incomparável entre calendários.
function SportSection({ dossier }) {
  return (
    <section>
      {dossier.historyStatus !== "ready" ? <HistoryStateMessage dossier={dossier} /> : null}
      <SportArranged dossier={dossier} />
    </section>
  );
}

// As duas vistas do mesmo assunto — onde a equipe TERMINOU cada campeonato, e
// COMO o campeonato de agora está sendo disputado.
//
// O panorama entre temporadas é o padrão porque é a pergunta que o dossiê de uma
// equipe responde primeiro: quem é essa equipe ao longo dos anos. A campanha é o
// zoom no ano corrente, e zoom vem depois do panorama — a mesma ordem que a fita
// de forma recente segue logo abaixo.
//
// O seletor entra no cabeçalho do gráfico, à esquerda, colado no rótulo: é ali
// que ele fica no MESMO lugar nas duas vistas. À direita cada vista tem o que é
// dela (a pílula do pódio, o modo do eixo), e o seletor pularia de posição a cada
// troca.
function ChampionshipEvolution({ run, seasons, rodadaAcesa = null, onAcenderRodada = null }) {
  const { t } = useTranslation();
  // As duas escolhas sobrevivem ao desmonte do bloco — ver evolutionPreferences.js.
  // Comparar equipes é o uso principal do gráfico, e o caminho até a próxima
  // equipe passa por trocar de aba ou fechar o dossiê: sem persistir, o gráfico
  // voltava para "entre campeonatos" bem no meio da comparação.
  const [vista, setVistaState] = useState(lerVistaEvolucao);
  // A métrica é escolhida UMA vez e vale nas duas escalas de tempo. Ela morava
  // dentro da campanha, então trocar de vista fazia aparecer um segundo seletor
  // e uma pílula do nada — e o toggle parecia levar a dois blocos diferentes em
  // vez de a duas vistas do mesmo. São dois eixos de escolha independentes:
  // QUANDO (entre campeonatos · campeonato atual) e O QUÊ (colocação · pontos).
  const [modo, setModoState] = useState(lerModoEvolucao);
  // Só o CLIQUE grava. A vista efetiva pode divergir da escolhida quando a
  // equipe da vez não tem campanha (abaixo), e essa queda é circunstância da
  // equipe — não deve reescrever o que o jogador pediu.
  const setVista = (id) => {
    guardarVistaEvolucao(id);
    setVistaState(id);
  };
  const setModo = (id) => {
    guardarModoEvolucao(id);
    setModoState(id);
  };
  const temCampanha = campanhaTemDados(run);
  const temTemporadas = curvaTemDados(seasons);
  if (!temCampanha && !temTemporadas) return null;

  // A vista escolhida pode ficar sem dado sem que ninguém clique em nada: as
  // setas do dossiê trocam de equipe sem desmontar a tela, e a próxima pode não
  // ter campanha. Derivar em vez de guardar em efeito evita o quadro em branco
  // de um frame.
  const efetiva = temCampanha && temTemporadas ? vista : temCampanha ? EVOLUTION_VIEW_RUN : EVOLUTION_VIEW_SEASONS;

  // Com uma vista só o seletor não aparece: um segmentado de um botão é ruído
  // que promete uma escolha inexistente.
  const seletor =
    temCampanha && temTemporadas ? (
      <div className="flex overflow-hidden rounded-lg border border-white/10" data-testid="team-history-evolution-view">
        {[EVOLUTION_VIEW_SEASONS, EVOLUTION_VIEW_RUN].map((id) => (
          <button
            key={id}
            type="button"
            onClick={() => setVista(id)}
            data-view={id}
            data-active={efetiva === id ? "true" : undefined}
            className={`px-2 py-1 text-[10px] font-semibold transition-glass ${
              efetiva === id ? "bg-white/[0.09] text-text-primary" : "text-text-muted hover:text-text-secondary"
            }`}
          >
            {t(`myTeamTab.history.sport.evolutionView.${id}`)}
          </button>
        ))}
      </div>
    ) : null;

  // O seletor de métrica é o MESMO objeto nas duas vistas — mesma posição, mesmo
  // desenho, mesma escolha preservada ao trocar de escala. É o que faz as duas
  // vistas lerem como um sistema, e não como dois blocos que se substituem.
  const seletorModo = (
    <div className="flex overflow-hidden rounded-lg border border-white/10" data-testid="team-history-run-mode">
      {[RUN_MODE_POSITION, RUN_MODE_POINTS].map((id) => (
        <button
          key={id}
          type="button"
          onClick={() => setModo(id)}
          data-mode={id}
          data-active={modo === id ? "true" : undefined}
          className={`px-2 py-1 text-[10px] font-semibold transition-colors duration-150 ${
            modo === id ? "bg-white/[0.09] text-text-primary" : "text-text-muted hover:text-text-secondary"
          }`}
        >
          {t(`myTeamTab.history.sport.runMode.${id}`)}
        </button>
      ))}
    </div>
  );

  return efetiva === EVOLUTION_VIEW_RUN ? (
    <ChampionshipRun
      run={run}
      seletor={seletor}
      seletorModo={seletorModo}
      modo={modo}
      rodadaAcesa={rodadaAcesa}
      onAcenderRodada={onAcenderRodada}
    />
  ) : (
    <ChampionshipCurve seasons={seasons} seletor={seletor} seletorModo={seletorModo} modo={modo} />
  );
}

// Arranjo arrumado: os mesmos seis blocos em TRÊS grupos, cada um respondendo
// uma pergunta inteira.
//
// O problema do clássico não é o conteúdo, é a hierarquia: seis blocos irmãos,
// todos com o mesmo rótulo minúsculo e a mesma largura total, viram uma lista
// onde o olho não tem onde pousar — e dois deles (confiabilidade e assinatura)
// são literalmente a mesma figura, uma barra de 100% com legenda, separadas por
// dois blocos no meio.
//
//   • Como a equipe termina — confiabilidade e assinatura lado a lado. Medem a
//     mesma corrida por ângulos vizinhos: quantas acabaram, e em que lugar.
//     Juntas, uma explica a outra; separadas, cada uma parecia assunto novo.
//   • Como a equipe evolui — a curva de campeonato com a fita de forma recente
//     ancorada logo abaixo. As duas leem tempo da esquerda para a direita, a
//     curva por temporada e a fita por corrida: é o mesmo eixo em dois zooms.
//   • Quem correu por ela — a galeria de pilotos e o ranking dos melhores.
//
// Os títulos de grupo são o nível que faltava. Os rótulos de bloco continuam
// exatamente onde estavam, agora lendo como subtítulo do grupo em vez de
// competirem entre si.
// Chave do elo entre a campanha e a fita: ano + rodada. As duas desenham as
// MESMAS corridas — a campanha somadas contra o grid, a fita uma a uma — e a
// rodada é o que elas têm em comum. O ano entra junto porque a fita atravessa
// temporadas e a campanha é de uma só; sem ele, a rodada 3 do ano passado
// acenderia a rodada 3 deste.
function chaveDaRodada(year, round) {
  const ano = Number(year);
  const rodada = Number(round);
  if (!Number.isFinite(ano) || !Number.isFinite(rodada)) return null;
  return `${ano}-${rodada}`;
}

function SportArranged({ dossier }) {
  const { t } = useTranslation();
  // A rodada sob o cursor, compartilhada pelo gráfico da campanha e pela fita de
  // forma recente. O grupo "Como a equipe evolui" é o pai comum dos dois.
  const [rodadaAcesa, setRodadaAcesa] = useState(null);
  // O piloto sob o cursor, compartilhado pela galeria de passagens e pelo
  // ranking dos melhores.
  const [pilotoAceso, setPilotoAceso] = useState(null);
  const temTermino = dossier.reliability?.races > 0 || dossier.resultSpread?.races > 0;
  const temEvolucao =
    campanhaTemDados(dossier.championshipRun) ||
    curvaTemDados(dossier.seasonResults) ||
    dossier.recentForm?.length > 0;
  const temGente = dossier.lineup?.length > 0;

  return (
    <div className="grid gap-6">
      {temTermino ? (
        <SportGroup title={t("myTeamTab.history.sport.groupFinish")}>
          {/* Flex e não grid de duas colunas: quando um dos dois blocos não tem
              dado (equipe recém-fundada não tem assinatura), o que sobrou ocupa a
              largura inteira em vez de deixar meia tela vazia. O `empty:hidden`
              recolhe a coluna do bloco que se anulou sozinho.

              Os dois entram em modo compacto, que é o que os faz ler como UMA
              linha e não como dois gráficos encostados: mesma estrutura de três
              faixas (rótulo, barra de 100%, legenda), então as barras caem na
              mesma altura sozinhas, sem altura fixa nem alinhamento manual. */}
          <div className="flex flex-wrap items-start gap-x-6 gap-y-5">
            <div className="min-w-0 flex-1 basis-[300px] empty:hidden">
              <ReliabilityPanel reliability={dossier.reliability} compacto />
            </div>
            <div className="min-w-0 flex-1 basis-[300px] empty:hidden">
              <ResultSpread spread={dossier.resultSpread} />
            </div>
          </div>
        </SportGroup>
      ) : null}

      {temEvolucao ? (
        <SportGroup title={t("myTeamTab.history.sport.groupTrajectory")}>
          {/* Um bloco com duas vistas, e não uma escolha nossa entre as duas: a
              curva diz ONDE a equipe terminou cada campeonato, a campanha diz
              COMO o campeonato de agora está sendo disputado. Nenhuma das duas é
              recorte da outra, e qual delas interessa depende da pergunta de
              quem abriu o dossiê.

              A curva abre por padrão — é o panorama, e o panorama vem antes do
              zoom. Quando só uma das duas tem dado, ela aparece sozinha e sem
              seletor. O arranjo clássico continua com a curva fixa. */}
          <ChampionshipEvolution
            run={dossier.championshipRun}
            seasons={dossier.seasonResults}
            rodadaAcesa={rodadaAcesa}
            onAcenderRodada={setRodadaAcesa}
          />
          {/* A fita entra DEPOIS do gráfico aqui, invertendo a ordem do clássico:
              lá ela vinha antes porque era o bloco do presente e abria a seção;
              aqui ela é o zoom final do mesmo eixo, e o zoom vem depois do
              panorama. */}
          {/* `first:mt-0` porque o gráfico some quando não há campanha nem duas
              temporadas fechadas: aí a fita vira o primeiro filho do cartão e o
              respiro de cima seria um vão sem nada acima dele. */}
          <div className="mt-4 first:mt-0">
            <RecentForm
              races={dossier.recentForm}
              rodadaAcesa={rodadaAcesa}
              onAcenderRodada={setRodadaAcesa}
            />
          </div>
        </SportGroup>
      ) : null}

      {temGente ? (
        <SportGroup title={t("myTeamTab.history.sport.groupPeople")}>
          <TeamLineup
            lineup={dossier.lineup}
            pilotoAceso={pilotoAceso}
            onAcenderPiloto={setPilotoAceso}
          />
          {/* O ranking vem DEPOIS da galeria porque depende dela para se ler: a
              galeria apresenta os nomes e a sucessão, o ranking ordena os mesmos
              nomes por currículo. Invertido, ele abriria o grupo com cinco
              pessoas que o leitor ainda não conhece. */}
          <div className="mt-5 empty:hidden first:mt-0">
            <BestDrivers
              lineup={dossier.lineup}
              pilotoAceso={pilotoAceso}
              onAcenderPiloto={setPilotoAceso}
            />
          </div>
        </SportGroup>
      ) : null}
    </div>
  );
}

// Um grupo do arranjo arrumado: título de primeiro nível sobre um cartão.
//
// O título tem a marca da equipe à esquerda e uma régua que atravessa a largura —
// é o que separa um grupo do outro sem precisar de mais uma borda. Dentro, o
// cartão de fundo levemente mais claro que a seção agrupa o que é do mesmo
// assunto: a proximidade sozinha não bastava, porque no clássico os blocos já
// eram próximos e mesmo assim liam como seis coisas.
function SportGroup({ title, children }) {
  return (
    <div>
      <div className="mb-2.5 flex items-center gap-2.5">
        <span className="h-3.5 w-[3px] shrink-0 rounded-full bg-[color:var(--team)]" />
        <h3 className="text-[13px] font-semibold leading-none tracking-[-0.01em] text-text-primary">{title}</h3>
        <span className="h-px min-w-0 flex-1 bg-gradient-to-r from-white/[0.09] to-transparent" />
      </div>
      <div className="rounded-2xl border border-white/[0.06] bg-white/[0.015] px-4 pb-4 pt-3.5">{children}</div>
    </div>
  );
}

function IdentitySection({ dossier }) {
  const { t } = useTranslation();
  const identity = dossier.identity;
  // Origem e atual eram dois cards com a mesma palavra na maioria das equipes —
  // metade da aba gasta para dizer "Mazda Rookie" duas vezes. Viraram um card só,
  // e o caso "nunca saiu" passou a ser afirmação de identidade em vez de repetição.
  const rooted = identity.origin === identity.current;
  const steps = dossier.categoryPath?.length ?? 0;
  const podiumRate = identity.profileRaces
    ? Math.round(((identity.profilePodiums ?? 0) / identity.profileRaces) * 100)
    : null;

  return (
    <section className="grid gap-2.5">
      {/* O duelo abre a aba porque é o assunto dela. Os fatos da equipe descem
          para a faixa de apoio: aqui eles existem para dar contexto a quem está
          do outro lado, não para retratar a casa — isso é papel da aba vizinha. */}
      <DuelPanel dossier={dossier} />

      {/* Perfil ocupa a largura toda: é o único bloco com prosa de verdade, e a
          meia-largura quebrava a frase em quatro linhas enquanto o vizinho tinha
          duas. Os números sobem para a mesma linha do rótulo. */}
      <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_38%,transparent)] bg-[#0c1626] px-4 py-3.5">
        <BlockLabel>{t("myTeamTab.history.identity.profileLabel")}</BlockLabel>
        <strong className="mt-1 block text-lg font-semibold leading-tight tracking-[-0.01em] text-text-primary">
          {identity.profile}
        </strong>
        <p className="mt-1 text-xs leading-5 text-text-secondary">{identity.summary}</p>
        {/* Os números que sustentam o rótulo viram métricas, não uma linha de mono
            solta: cada um ganha rótulo próprio e alinha em coluna com os vizinhos.
            Corridas é o denominador, e sem ele "33% de pódio" não quer dizer nada. */}
        {identity.profileRaces ? (
          <div className="mt-3 grid grid-cols-2 gap-2 md:grid-cols-4">
            <FactTile label={t("myTeamTab.history.identity.tile.races")} value={identity.profileRaces} />
            <FactTile label={t("myTeamTab.history.identity.tile.wins")} value={identity.profileWins ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.podiums")} value={identity.profilePodiums ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.podiumRate")} value={`${podiumRate}%`} />
          </div>
        ) : null}
        {/* Mesma barra de proporção que Records usa em cada card: é ela que põe a
            cor da equipe na tela e dá ao rótulo uma medida em vez de só prosa.
            Aqui enche com a taxa de pódio, que é o número que sustenta o perfil. */}
        {podiumRate != null ? <TeamShareBar value={podiumRate} /> : null}
      </div>

      {/* Três cards do mesmo peso numa linha só. Antes a trajetória dividia a
          fileira com a pilha de duas pistas e esticava até a altura delas — um
          card gigante com duas linhas de texto dentro. */}
      <div className="grid gap-2.5 md:grid-cols-3">
        <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_32%,transparent)] bg-[#0c1626]/95 px-4 py-3.5">
          <div className="flex items-baseline justify-between gap-2">
            <BlockLabel>{t("myTeamTab.history.identity.symbolLabel")}</BlockLabel>
            {identity.symbolDriverYears ? (
              <span className="shrink-0 font-mono text-[11px] font-semibold text-text-secondary">
                {identity.symbolDriverYears}
              </span>
            ) : null}
          </div>
          <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1">
            {identity.symbolDriverNationality ? (
              <FlagIcon nacionalidade={identity.symbolDriverNationality} />
            ) : null}
            <strong className="text-[15px] font-semibold leading-tight text-text-primary">
              {identity.symbolDriver}
            </strong>
            {/* Um símbolo que ficou e um que foi embora contam histórias opostas —
                o card dizia a mesma coisa nos dois casos. */}
            {identity.symbolDriverYears ? (
              <span
                className={`rounded-md px-1.5 py-0.5 text-[10px] font-semibold ${
                  identity.symbolDriverActive
                    ? "bg-status-green/15 text-status-green"
                    : "bg-white/10 text-text-secondary"
                }`}
              >
                {identity.symbolDriverActive
                  ? t("myTeamTab.history.identity.symbolStillHere")
                  : t("myTeamTab.history.identity.symbolGone")}
              </span>
            ) : null}
          </div>
          {/* A prosa "5 corridas, 1 vitória, 2 pódios" virou métrica: os três
              cards da fileira passam a ter a mesma anatomia, e era a falta disso
              que fazia dois deles sobrarem espaço enquanto o terceiro enchia. */}
          <div className="mt-3 grid grid-cols-3 gap-2">
            <FactTile label={t("myTeamTab.history.identity.tile.races")} value={identity.symbolDriverRaces ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.wins")} value={identity.symbolDriverWins ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.podiums")} value={identity.symbolDriverPodiums ?? 0} />
          </div>
        </div>

        <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_32%,transparent)] bg-[#0c1626]/95 px-4 py-3.5">
          <BlockLabel>{t("myTeamTab.history.identity.trajectoryLabel")}</BlockLabel>
          {/* O degrau ATUAL vai na cor da equipe e a origem fica apagada: é a cor
              dizendo onde a equipe está, não enfeitando a linha. */}
          <strong className="mt-1.5 block text-[15px] font-semibold leading-tight text-text-primary">
            {rooted ? null : (
              <>
                <span className="text-text-muted">{identity.origin}</span>
                <span className="px-1.5 text-text-muted">→</span>
              </>
            )}
            <span className="text-[color:var(--team)]">{identity.current}</span>
          </strong>
          {/* Contagem de temporadas vem de `seasonResults`, que é lista:
              `sport.seasons` já chega formatado como prosa ("4 Temporadas"). */}
          <div className="mt-3 grid grid-cols-2 gap-2">
            <FactTile
              label={t("myTeamTab.history.identity.tile.seasons")}
              value={dossier.seasonResults?.length ?? 0}
            />
            <FactTile label={t("myTeamTab.history.identity.tile.steps")} value={steps} />
          </div>
        </div>

        {identity.recruitment ? <RecruitmentCard recruitment={identity.recruitment} /> : null}
      </div>

      {identity.bestTrack && identity.worstTrack ? (
        <div className="grid gap-2.5 md:grid-cols-2">
          <TrackAffinityCard affinity={identity.bestTrack} favourite />
          <TrackAffinityCard affinity={identity.worstTrack} />
        </div>
      ) : null}
      {dossier.ownershipEvents?.length > 0 && (
        <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.identity.erasLabel")}</BlockLabel>
          <ul className="mt-2.5 grid gap-2.5 md:grid-cols-2">
            {dossier.ownershipEvents.map((event, index) => (
              <li key={index} className="flex items-start gap-3">
                <span className="mt-0.5 font-mono text-xs font-bold text-[color:var(--team)]">{event.year}</span>
                <div className="min-w-0">
                  <strong className="block text-xs font-semibold text-text-primary">{event.title}</strong>
                  <p className="text-[11px] leading-5 text-text-secondary">{event.detail}</p>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

// Idade do último encontro em linguagem de calendário. A fonte é em SEMANAS
// porque é assim que o mundo do Loop marca o tempo (`week_of_year`), e a escada
// sobe conforme a distância: semanas viram meses, meses viram anos. `null` só
// acontece em payload antigo, e aí o card cala em vez de inventar "há 0 semanas".
function formatMeetingAge(t, weeksAgo) {
  if (weeksAgo == null) return t("myTeamTab.history.identity.rivalAgeUnknown");
  if (weeksAgo <= 1) return t("myTeamTab.history.identity.rivalAgeNow");
  if (weeksAgo < 9) return t("myTeamTab.history.identity.rivalAgeWeeks", { count: weeksAgo });
  if (weeksAgo < 52) {
    return t("myTeamTab.history.identity.rivalAgeMonths", { count: Math.round(weeksAgo / 4.33) });
  }
  return t("myTeamTab.history.identity.rivalAgeYears", { count: Math.floor(weeksAgo / 52) });
}

// Métrica de card: rótulo em cima, número em mono embaixo. É a mesma anatomia do
// `MiniMetric` do cabeçalho, em corpo menor — e é ela que dá aos cards da fileira
// uma altura comum, porque prosa não alinha em coluna e número alinha.
function FactTile({ label, value }) {
  return (
    <div className="rounded-lg bg-[#0f1c2b] px-2.5 py-2">
      {/* Rótulo QUEBRA em vez de truncar: numa coluna estreita "Anos na chegada"
          virava "Anos na c…", e rótulo cortado não informa nada. Como os tiles
          vivem num grid, a segunda linha estica todos juntos e o alinhamento
          entre eles se mantém. */}
      <span className="block text-[11px] font-semibold leading-tight text-text-secondary">{label}</span>
      <strong className="mt-1 block font-mono text-base leading-none text-text-primary">{value}</strong>
    </div>
  );
}

// O duelo: as duas equipes se encarando, com o placar do confronto direto no meio.
//
// A aba antes empilhava seis cards do mesmo tamanho e nenhum era o assunto. Aqui o
// olho cai no centro, e cada lado carrega a cor de quem representa — a casa em
// `--team`, o adversário em `--rival`. Sem rival consolidado o painel desce para
// um estado enxuto em vez de desenhar um duelo que não existe.
function DuelPanel({ dossier }) {
  const { t } = useTranslation();
  const rival = dossier.identity.rival;
  const hasRival = Boolean(rival.color);
  const hasAxes = rival.historicalIntensity != null && rival.recentActivity != null;
  const clashes = rival.headToHeadWins + rival.headToHeadLosses;
  const rivalColor = hasRival ? getVividTeamColor(rival.color) : "var(--team)";

  if (!hasRival) {
    return (
      <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4">
        <BlockLabel>{t("myTeamTab.history.identity.rivalLabel")}</BlockLabel>
        <strong className="mt-1.5 block text-sm font-semibold text-text-primary">{rival.name}</strong>
        <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{rival.note}</p>
      </div>
    );
  }

  return (
    <div
      className="rounded-xl border border-white/10 bg-[#0c1626] p-4"
      style={{
        "--rival": rivalColor,
        // O painel é dividido ao meio: a metade da casa puxa `--team`, a do
        // adversário puxa `--rival`, e as duas se encontram no centro — onde fica
        // o placar. Antes o fundo inteiro era da cor do rival, então o painel
        // parecia território dele em vez de terreno dividido.
        backgroundImage:
          "linear-gradient(90deg, color-mix(in srgb, var(--team) 20%, transparent) 0%, transparent 46%, transparent 54%, color-mix(in srgb, var(--rival) 20%, transparent) 100%)",
      }}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <BlockLabel>{t("myTeamTab.history.identity.rivalLabel")}</BlockLabel>
        <span className="rounded-md border border-white/15 px-2 py-0.5 text-[10px] font-semibold text-text-secondary">
          {rival.originKind ?? t("myTeamTab.history.identity.rivalHeuristic")}
        </span>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-3">
        <TeamLogoMark teamName={dossier.name} color={dossier.color} size="md" testId="team-history-duel-home" />
        <div className="min-w-0 flex-1">
          <strong className="block truncate text-base font-semibold leading-tight text-[color:var(--team)]">
            {dossier.name}
          </strong>
          {/* Categoria dos dois lados, e não o perfil da casa: o perfil já está
              no card logo abaixo, e simétrico o painel lê como confronto. */}
          <span className="block truncate text-xs text-text-secondary">{dossier.identity.current}</span>
        </div>
        {/* O placar é a informação que uma rivalidade existe para dar. Fica no
            centro geométrico do painel porque é o centro do assunto, e grande o
            bastante para ser a primeira coisa que o olho encontra. */}
        <div className="shrink-0 px-2 text-center">
          <div className="flex items-baseline justify-center gap-2.5 font-mono text-[32px] font-semibold leading-none tracking-[-0.03em]">
            <span className="text-[color:var(--team)]">{rival.headToHeadWins}</span>
            <span className="text-base text-text-muted">×</span>
            <span className="text-[color:var(--rival)]">{rival.headToHeadLosses}</span>
          </div>
          <span className="mt-2 block text-[11px] font-semibold text-text-secondary">
            {clashes > 0
              ? t("myTeamTab.history.identity.rivalHeadToHead")
              : t("myTeamTab.history.identity.rivalNoClash")}
          </span>
        </div>
        <div className="min-w-0 flex-1 text-right">
          <strong className="block truncate text-base font-semibold leading-tight text-[color:var(--rival)]">
            {rival.name}
          </strong>
          <span className="block truncate text-xs text-text-secondary">{rival.currentCategory}</span>
        </div>
        <TeamLogoMark teamName={rival.name} color={rival.color} size="md" testId="team-history-rival-logo" />
      </div>

      {/* Faixa de rodapé em flex, não em grid: quando o motor não registrou os
          eixos sobrava um item só, e o grid de três colunas o esticava por toda a
          largura como se fosse um bloco. */}
      <div className="mt-3.5 flex flex-wrap items-end gap-x-8 gap-y-3 border-t border-white/5 pt-3">
        <div className="min-w-[13rem] flex-1">
          <BlockLabel>
            {rival.lastMeeting
              ? t("myTeamTab.history.identity.rivalLastMeeting")
              : t("myTeamTab.history.identity.rivalScopeNote")}
          </BlockLabel>
          <p className="mt-1 text-xs leading-5 text-text-secondary">
            {/* "Temporada 3, rodada 5" não diz se foi ontem ou há três anos. O
                tempo decorrido diz, e é ele que faz a rivalidade parecer viva ou
                arquivada — o resultado do encontro segue junto. */}
            {rival.lastMeeting
              ? `${formatMeetingAge(t, rival.lastMeeting.weeksAgo)} — ${t(
                  "myTeamTab.history.identity.rivalLastMeetingResult",
                  {
                    position: rival.lastMeeting.position,
                    rivalPosition: rival.lastMeeting.rivalPosition,
                  },
                )}`
              : rival.note}
          </p>
        </div>
        {hasAxes ? (
          <>
            <IntensityBar
              label={t("myTeamTab.history.identity.rivalAxisHistorical")}
              value={rival.historicalIntensity}
            />
            <IntensityBar label={t("myTeamTab.history.identity.rivalAxisRecent")} value={rival.recentActivity} />
          </>
        ) : null}
      </div>
    </div>
  );
}

// Escola ou mercado: os rótulos de desempenho não separam a equipe que forma
// gente da que compra pronto, e as duas podem ganhar igual. Aqui separam.
function RecruitmentCard({ recruitment }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_32%,transparent)] bg-[#0c1626]/95 px-4 py-3.5">
      <div className="flex items-baseline justify-between gap-2">
        <BlockLabel>{t("myTeamTab.history.identity.recruitmentLabel")}</BlockLabel>
        <span className="shrink-0 font-mono text-[11px] font-semibold text-text-secondary">
          {t("myTeamTab.history.identity.recruitmentRatio", {
            rookies: recruitment.rookies,
            drivers: recruitment.drivers,
          })}
        </span>
      </div>
      <strong className="mt-1.5 block text-[15px] font-semibold leading-tight text-[color:var(--team)]">
        {recruitment.profile}
      </strong>
      <div className="mt-3 grid grid-cols-3 gap-2">
        <FactTile
          label={t("myTeamTab.history.identity.tile.rookies")}
          value={`${Math.round(recruitment.rookieShare)}%`}
        />
        <FactTile
          label={t("myTeamTab.history.identity.tile.grid")}
          value={`${Math.round(recruitment.fieldRookieShare)}%`}
        />
        <FactTile
          label={t("myTeamTab.history.identity.tile.onArrival")}
          value={recruitment.averageExperience.toFixed(1)}
        />
      </div>
      {/* A comparação com o grid vira geometria: a barra é a fatia da equipe, o
          traço é a do grid. O rótulo "Escola" ou "Mercado" passa a ser a leitura
          de uma distância que dá para ver, e não uma afirmação para acreditar. */}
      <TeamShareBar value={recruitment.rookieShare} marker={recruitment.fieldRookieShare} />
    </div>
  );
}

// A barra de proporção que Records usa em cada card, trazida para cá — é ela que
// põe a cor da equipe na aba. `marker` desenha a régua de comparação por cima.
function TeamShareBar({ value, marker = null }) {
  const pct = Math.max(0, Math.min(100, Math.round(value)));
  const markerPct = marker == null ? null : Math.max(0, Math.min(100, Math.round(marker)));
  return (
    <div className="relative mt-2.5 h-[3px] overflow-hidden rounded-full bg-white/10">
      <div className="h-full rounded-full bg-[color:var(--team)]" style={{ width: `${pct}%` }} />
      {markerPct == null ? null : (
        <span className="absolute inset-y-0 w-[2px] bg-white/55" style={{ left: `calc(${markerPct}% - 1px)` }} />
      )}
    </div>
  );
}

// Onde a equipe historicamente vai bem e onde apanha. É identidade barata de
// obter e cara de esquecer: o jogador aprende a temer o calendário.
function TrackAffinityCard({ affinity, favourite = false }) {
  const { t } = useTranslation();
  const tone = favourite
    ? "border-status-green/25 bg-[#0b1d19]/95"
    : "border-status-red/25 bg-[#241014]/95";
  const accent = favourite ? "text-status-green" : "text-status-red";
  return (
    <div className={`flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 rounded-xl border px-4 py-3 ${tone}`}>
      <div className="min-w-0">
        <BlockLabel>
          {favourite
            ? t("myTeamTab.history.identity.trackFavouriteLabel")
            : t("myTeamTab.history.identity.trackBogeyLabel")}
        </BlockLabel>
        <strong className={`mt-1 block truncate text-[15px] font-semibold leading-tight ${accent}`}>
          {affinity.track}
        </strong>
      </div>
      <span className="shrink-0 font-mono text-[11px] font-semibold text-text-secondary">
        {t("myTeamTab.history.identity.trackDetail", {
          count: affinity.races,
          average: affinity.averagePosition.toFixed(1),
          best: affinity.bestPosition,
        })}
      </span>
    </div>
  );
}

// Vive dentro do card do rival, então lê `--rival` do pai — as barras são da cor
// de quem a rivalidade descreve, não de um amarelo qualquer.
function IntensityBar({ label, value }) {
  const pct = Math.max(0, Math.min(100, Math.round(value)));
  return (
    <div className="w-[10rem]">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-[11px] font-semibold text-text-secondary">{label}</span>
        <span className="font-mono text-[11px] font-semibold text-[color:var(--rival)]">{pct}</span>
      </div>
      <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-[#0f1c2b]">
        <div
          className="h-full rounded-full bg-[color-mix(in_srgb,var(--rival)_78%,transparent)]"
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

const CASH_WIDTH = 640;
const CASH_HEIGHT = 156;
const CASH_LEFT = 8;
const CASH_RIGHT = 632;
const CASH_TOP = 12;
// Fundo da área de dados. Abaixo dele sobra a faixa da régua do tempo, que os
// rótulos de temporada ocupam sem invadir o desenho.
const CASH_FLOOR = 122;
const CASH_AXIS = 138;

// Curva de caixa da carreira inteira, com a dívida pendurada abaixo da linha do
// zero. São duas séries de propósito: caixa e dívida coexistem (dá para ter $1M em
// caixa e $2M de passivo), e um único traço do líquido esconderia justamente a
// equipe que opera alavancada. A leitura fica imediata — o que está acima do zero
// é dinheiro, o que está abaixo é buraco.
function CashCurve({ ledger }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const dados = useMemo(() => {
    const pontos = ledger?.cashCurve ?? [];
    // Dois pontos é o mínimo para existir uma curva. Com um, o desenho seria um
    // ponto solto anunciando uma trajetória que ainda não aconteceu.
    if (pontos.length < 2) return null;
    const teto = Math.max(0, ...pontos.map((p) => p.cashBalance));
    const piso = Math.max(0, ...pontos.map((p) => p.debtBalance));
    const amplitude = teto + piso;
    if (amplitude <= 0) return null;
    const passo = (CASH_RIGHT - CASH_LEFT) / (pontos.length - 1);
    const y = (valor) => CASH_TOP + ((teto - valor) / amplitude) * (CASH_FLOOR - CASH_TOP);
    const comXY = pontos.map((ponto, index) => ({
      ...ponto,
      x: CASH_LEFT + index * passo,
      yCaixa: y(ponto.cashBalance),
      yDivida: y(-ponto.debtBalance),
    }));
    // Uma guia por virada de temporada — é o que dá escala de tempo ao eixo sem
    // um rótulo por rodada. A primeira coluna também entra: sem ela a carreira
    // começaria sem ano.
    const viradas = comXY.filter(
      (ponto, index) => index === 0 || ponto.seasonNumber !== comXY[index - 1].seasonNumber,
    );
    return { pontos: comXY, viradas, teto, piso, zero: y(0), temDivida: piso > 0 };
  }, [ledger]);

  if (!dados) return null;
  const { pontos, viradas, teto, piso, zero, temDivida } = dados;
  const areaId = `${uid}-caixa`;
  const dividaId = `${uid}-divida`;
  const linha = pontos.map((p) => `${p.x},${p.yCaixa}`).join(" ");
  const areaCaixa = `M ${pontos[0].x},${zero} ${pontos
    .map((p) => `L ${p.x},${p.yCaixa}`)
    .join(" ")} L ${pontos[pontos.length - 1].x},${zero} Z`;
  const areaDivida = `M ${pontos[0].x},${zero} ${pontos
    .map((p) => `L ${p.x},${p.yDivida}`)
    .join(" ")} L ${pontos[pontos.length - 1].x},${zero} Z`;
  // Só as viradas que cabem ganham rótulo: numa carreira longa os anos colariam
  // um no outro e a régua viraria uma mancha.
  const espacoPorVirada = (CASH_RIGHT - CASH_LEFT) / Math.max(viradas.length, 1);
  const rotulaTodas = espacoPorVirada >= 42;

  return (
    <div className="mt-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <BlockLabel>{t("myTeamTab.history.management.cashCurve")}</BlockLabel>
        <span className="font-mono text-[11px] text-text-secondary">
          {t("myTeamTab.history.management.cashCurveScale", {
            peak: formatMoneyCompact(teto),
            debt: formatMoneyCompact(piso),
          })}
        </span>
      </div>
      <div className="mt-2 rounded-xl border border-white/[0.06] bg-[#0b1524] px-3 py-2.5">
        <svg
          viewBox={`0 0 ${CASH_WIDTH} ${CASH_HEIGHT}`}
          className="h-auto w-full"
          data-testid="team-history-cash-curve"
        >
          <defs>
            <linearGradient id={areaId} x1="0" y1={CASH_TOP} x2="0" y2={zero} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.45" />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.03" />
            </linearGradient>
            <linearGradient id={dividaId} x1="0" y1={zero} x2="0" y2={CASH_FLOOR} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--status-red)" stopOpacity="0.06" />
              <stop offset="100%" stopColor="var(--status-red)" stopOpacity="0.5" />
            </linearGradient>
          </defs>

          {/* Guia por temporada. É o que transforma uma linha contínua de rodadas
              em anos legíveis. */}
          {viradas.map((ponto) => (
            <line
              key={`guia-${ponto.seasonNumber}`}
              x1={ponto.x}
              y1={CASH_TOP}
              x2={ponto.x}
              y2={CASH_FLOOR}
              stroke="#ffffff"
              strokeOpacity="0.06"
              strokeDasharray="3 5"
            />
          ))}

          <path d={areaCaixa} fill={`url(#${areaId})`} />
          {temDivida ? <path d={areaDivida} fill={`url(#${dividaId})`} /> : null}

          {/* A linha do zero é a régua moral do gráfico: acima dela é caixa, abaixo
              é dívida. Fica mais forte que as guias porque é o que separa os dois
              lados da história. */}
          <line
            x1={CASH_LEFT}
            y1={zero}
            x2={CASH_RIGHT}
            y2={zero}
            stroke="#ffffff"
            strokeOpacity="0.22"
          />

          <polyline
            points={linha}
            fill="none"
            stroke="var(--team)"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          {temDivida ? (
            <polyline
              points={pontos.map((p) => `${p.x},${p.yDivida}`).join(" ")}
              fill="none"
              stroke="var(--status-red)"
              strokeWidth="1.6"
              strokeOpacity="0.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          ) : null}

          {/* Fechamento de temporada: é onde o prêmio de construtores cai no caixa,
              o degrau mais informativo do desenho. Marcado no ponto, não numa
              legenda que ninguém lê. */}
          {pontos
            .filter((ponto) => ponto.isSeasonClose)
            .map((ponto) => (
              <circle
                key={`fecha-${ponto.seasonNumber}-${ponto.round}`}
                cx={ponto.x}
                cy={ponto.yCaixa}
                r="3"
                fill="var(--team)"
                stroke="#0b1524"
                strokeWidth="1.6"
              />
            ))}

          {viradas.map((ponto, index) =>
            rotulaTodas || index === 0 || index === viradas.length - 1 ? (
              <text
                key={`ano-${ponto.seasonNumber}`}
                x={Math.min(Math.max(ponto.x, CASH_LEFT + 10), CASH_RIGHT - 10)}
                y={CASH_AXIS}
                textAnchor="middle"
                fontSize="10"
                fontWeight="500"
                fill="#8ea0b4"
              >
                {t("myTeamTab.history.management.seasonShort", { season: ponto.seasonNumber })}
              </text>
            ) : null,
          )}
        </svg>
      </div>
    </div>
  );
}

// O viewBox é largo de propósito: escalado para a largura do painel, ele fica perto
// de 1:1 e o texto do gráfico sai do mesmo tamanho do texto do resto do dossiê. Num
// viewBox estreito o mesmo `fontSize` chegaria esticado e o gráfico gritaria.
const FLOW_WIDTH = 1040;
const FLOW_PAD_Y = 24;
// Folga vertical entre dois nós do mesmo lado. É o que abre espaço para o rótulo de
// cada fita — sem ela, duas fitas finas vizinhas teriam os rótulos colados.
const FLOW_GAP = 30;
const FLOW_MIN_BAND = 3;
const FLOW_PILL_W = 5;
// As pontas encostam nas BORDAS do viewBox. Antes sobrava um vão morto de ~130
// unidades à direita — herança de reservar espaço para os rótulos, que na verdade
// flutuam ACIMA das fitas e não precisam de coluna própria. O desenho é a coisa mais
// larga da aba; deixá-lo parar no meio do caminho encolhia justamente as diferenças
// de largura que ele existe para mostrar.
const FLOW_LEFT_X = 0;
const FLOW_RIGHT_X = FLOW_WIDTH - FLOW_PILL_W;
const FLOW_TRUNK_W = 13;
const FLOW_TRUNK_X = (FLOW_WIDTH - FLOW_TRUNK_W) / 2;

// Fluxo de dinheiro da carreira inteira: as linhas de receita convergem no tronco e
// saem repartidas em custos e saldo.
//
// Um Sankey e não dois gráficos separados porque a pergunta é uma só — o dinheiro
// que entrou é o mesmo que saiu, e são as LARGURAS relativas que contam a história:
// a folha salarial como metade do tronco diz mais do que "$6,3M" numa lista.
//
// A conta fecha dos dois lados por construção. Quando a equipe gasta mais do que
// arrecada, a diferença entra como um nó próprio à esquerda — o dinheiro veio de
// algum lugar (reservas ou dívida nova), e o desenho não pode fingir que apareceu.
function MoneyFlow({ ledger }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const dados = useMemo(() => {
    const receita = ledger?.incomeLines ?? [];
    const custos = ledger?.expenseLines ?? [];
    if (!receita.length && !custos.length) return null;
    const saldo = (ledger.incomeTotal ?? 0) - (ledger.expensesTotal ?? 0);
    const cobertura = Math.max(0, -saldo);
    const tronco = (ledger.incomeTotal ?? 0) + cobertura;
    if (tronco <= 0) return null;

    const esquerda = receita.map((line, index) => ({
      key: line.id,
      label: t(`myTeamTab.finance.lines.${line.id}`),
      value: line.value,
      hue: "var(--status-green)",
      fade: Math.max(0.3, 1 - index * 0.15),
    }));
    if (cobertura > 0) {
      esquerda.push({
        key: "coverage",
        label: t("myTeamTab.history.management.flowCoverage"),
        value: cobertura,
        hue: "var(--status-red)",
        fade: 1,
      });
    }
    const direita = custos.map((line, index) => ({
      key: line.id,
      label: t(`myTeamTab.finance.lines.${line.id}`),
      value: line.value,
      hue: "var(--status-yellow)",
      fade: Math.max(0.3, 1 - index * 0.15),
    }));
    if (saldo > 0) {
      direita.push({
        key: "balance",
        label: t("myTeamTab.history.management.flowBalance"),
        value: saldo,
        hue: "var(--status-green)",
        fade: 1,
      });
    }

    // A altura do desenho é derivada, não fixa: o lado com mais nós define quantas
    // folgas cabem, e o tronco fica com o que sobra. Assim o gráfico cresce com o
    // dado em vez de espremer oito fitas numa caixa de altura fixa.
    const folgas = Math.max(esquerda.length, direita.length) - 1;
    const corpo = Math.max(120, 26 * (folgas + 1));
    const altura = FLOW_PAD_Y * 2 + corpo + folgas * FLOW_GAP;
    const banda = (valor) => Math.max(FLOW_MIN_BAND, (valor / tronco) * corpo);

    const empilha = (nos) => {
      const total = nos.reduce((soma, no) => soma + banda(no.value), 0) + (nos.length - 1) * FLOW_GAP;
      let cursor = (altura - total) / 2;
      return nos.map((no) => {
        const h = banda(no.value);
        const topo = cursor;
        cursor += h + FLOW_GAP;
        return { ...no, topo, base: topo + h, share: (no.value / tronco) * 100 };
      });
    };

    const nosEsquerda = empilha(esquerda);
    const nosDireita = empilha(direita);
    // As fitas chegam ao tronco na MESMA ordem em que saem dos nós, empilhadas sem
    // folga: o tronco é contínuo, é o total.
    const troncoTopo = (altura - corpo) / 2;
    let cursorEsq = troncoTopo;
    const fitasEsquerda = nosEsquerda.map((no) => {
      const h = no.base - no.topo;
      const ancora = cursorEsq;
      cursorEsq += h;
      return { ...no, ancoraTopo: ancora, ancoraBase: ancora + h };
    });
    let cursorDir = troncoTopo;
    const fitasDireita = nosDireita.map((no) => {
      const h = no.base - no.topo;
      const ancora = cursorDir;
      cursorDir += h;
      return { ...no, ancoraTopo: ancora, ancoraBase: ancora + h };
    });

    return { fitasEsquerda, fitasDireita, altura, corpo, troncoTopo, tronco };
  }, [ledger, t]);

  // Sem repartição o bloco NÃO some — ele explica. Sumir era o pior estado: o
  // jogador não distinguia "esta equipe não tem economia de rodada" de "o gráfico
  // quebrou", e a frase vem pronta do backend, que é quem sabe a causa.
  if (!dados) {
    if (!ledger?.flowNote) return null;
    return (
      <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4" data-testid="team-history-money-flow">
        <BlockLabel>{t("myTeamTab.history.management.moneyFlow")}</BlockLabel>
        <p className="mt-2 text-[11px] leading-5 text-text-secondary">{ledger.flowNote}</p>
      </div>
    );
  }
  const { fitasEsquerda, fitasDireita, altura, corpo, troncoTopo, tronco } = dados;
  const janela =
    ledger.flowFirstSeason === ledger.flowLastSeason
      ? t("myTeamTab.history.management.flowWindowOne", { season: ledger.flowLastSeason })
      : t("myTeamTab.history.management.flowWindowRange", {
          first: ledger.flowFirstSeason,
          last: ledger.flowLastSeason,
        });

  return (
    <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4" data-testid="team-history-money-flow">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
        <BlockLabel>{t("myTeamTab.history.management.moneyFlow")}</BlockLabel>
        {/* A legenda diz a JANELA, não "da fundação até aqui". O livro-caixa rodada
            a rodada existe só nas temporadas jogadas; as de backstory registram só
            o prêmio de construtores, e prometer a carreira inteira aqui seria
            vender uma soma que a tabela não tem. */}
        <span className="text-[11px] text-text-secondary">{janela}</span>
      </div>
      <svg
        viewBox={`0 0 ${FLOW_WIDTH} ${altura}`}
        className="mt-2 h-auto w-full"
        data-testid="team-history-money-flow-chart"
      >
        <defs>
          {/* Uma fita degrada da cor do NÓ para a cor da equipe no tronco. É o que
              faz o meio do desenho virar uma massa só — a receita da equipe — em
              vez de doze fios coloridos atravessando a tela. */}
          {fitasEsquerda.map((no) => (
            <linearGradient
              key={`ge-${no.key}`}
              id={`${uid}-e-${no.key}`}
              x1={FLOW_LEFT_X}
              x2={FLOW_TRUNK_X}
              y1="0"
              y2="0"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0%" stopColor={no.hue} stopOpacity={0.6 * no.fade} />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.42" />
            </linearGradient>
          ))}
          {fitasDireita.map((no) => (
            <linearGradient
              key={`gd-${no.key}`}
              id={`${uid}-d-${no.key}`}
              x1={FLOW_TRUNK_X + FLOW_TRUNK_W}
              x2={FLOW_RIGHT_X}
              y1="0"
              y2="0"
              gradientUnits="userSpaceOnUse"
            >
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.42" />
              <stop offset="100%" stopColor={no.hue} stopOpacity={0.6 * no.fade} />
            </linearGradient>
          ))}
        </defs>

        {fitasEsquerda.map((no) => (
          <g key={`fe-${no.key}`}>
            <path
              d={ribbonPath(FLOW_LEFT_X + FLOW_PILL_W, no.topo, no.base, FLOW_TRUNK_X, no.ancoraTopo, no.ancoraBase)}
              fill={`url(#${uid}-e-${no.key})`}
            />
            <rect
              x={FLOW_LEFT_X}
              y={no.topo}
              width={FLOW_PILL_W}
              height={no.base - no.topo}
              rx={FLOW_PILL_W / 2}
              fill={no.hue}
              fillOpacity={no.fade}
            />
            <FlowLabel x={FLOW_LEFT_X + FLOW_PILL_W + 8} y={no.topo - 7} anchor="start" node={no} />
          </g>
        ))}

        {fitasDireita.map((no) => (
          <g key={`fd-${no.key}`}>
            <path
              d={ribbonPath(
                FLOW_TRUNK_X + FLOW_TRUNK_W,
                no.ancoraTopo,
                no.ancoraBase,
                FLOW_RIGHT_X,
                no.topo,
                no.base,
              )}
              fill={`url(#${uid}-d-${no.key})`}
            />
            <rect
              x={FLOW_RIGHT_X}
              y={no.topo}
              width={FLOW_PILL_W}
              height={no.base - no.topo}
              rx={FLOW_PILL_W / 2}
              fill={no.hue}
              fillOpacity={no.fade}
            />
            <FlowLabel x={FLOW_RIGHT_X - 8} y={no.topo - 7} anchor="end" node={no} />
          </g>
        ))}

        {/* O tronco por cima das fitas: é ele que fecha a conta, e as pontas das
            fitas não devem vazar por dentro dele. */}
        <rect
          x={FLOW_TRUNK_X}
          y={troncoTopo}
          width={FLOW_TRUNK_W}
          height={corpo}
          rx={FLOW_TRUNK_W / 2}
          fill="var(--team)"
        />
        <text
          x={FLOW_TRUNK_X + FLOW_TRUNK_W / 2}
          y={troncoTopo - 9}
          textAnchor="middle"
          fontSize="11"
          fontWeight="600"
          fill="#e6edf3"
        >
          {t("myTeamTab.history.management.flowTrunk", { value: formatMoney(tronco) })}
        </text>
      </svg>
    </div>
  );
}

// Rótulo de um nó do fluxo: nome, valor e fatia numa linha só, como as etiquetas
// que flutuam ao lado das fitas. `textLength` fica de fora — deixar o SVG espremer
// o texto para caber quebraria a régua tipográfica do resto do dossiê.
function FlowLabel({ x, y, anchor, node }) {
  return (
    <text x={x} y={y} textAnchor={anchor} fontSize="11" fill="#8ea0b4">
      <tspan fill="#e6edf3">{node.label}</tspan>
      <tspan dx="7">{formatMoneyCompact(node.value)}</tspan>
      <tspan dx="7">{`${Math.round(node.share)}%`}</tspan>
    </text>
  );
}

// Fita do Sankey: duas cúbicas espelhadas com os controles no meio do vão, que é o
// que dá a curva em S sem depender de biblioteca.
function ribbonPath(x0, topo0, base0, x1, topo1, base1) {
  const meio = (x0 + x1) / 2;
  return [
    `M ${x0},${topo0}`,
    `C ${meio},${topo0} ${meio},${topo1} ${x1},${topo1}`,
    `L ${x1},${base1}`,
    `C ${meio},${base1} ${meio},${base0} ${x0},${base0}`,
    "Z",
  ].join(" ");
}

function ManagementSection({ dossier }) {
  const { t } = useTranslation();
  // A saúde da operação é o único bloco que muda de cor por conteúdo — vermelho
  // para pressionada/crise, amarelo para estável, verde para saudável. A regra é
  // a mesma do v1, importada em vez de recopiada.
  const tone = operationHealthTone(dossier.management.operationHealth);
  const ledger = dossier.management.ledger;
  return (
    <section className="grid gap-2.5">
      {/* O fluxo abre a aba: é o desenho que responde "de onde vem e para onde vai"
          antes de qualquer rótulo, e a largura das fitas carrega a leitura sozinha.
          Saúde e curva descem para depois dos extremos — a frase da saúde é um
          RESUMO, e resumo depois do dado lê melhor do que antes. */}
      {ledger ? <MoneyFlow ledger={ledger} /> : null}
      <div className="grid gap-2.5 md:grid-cols-2">
        <div className="rounded-xl border border-status-green/25 bg-[#0b1d19]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.management.peakCash")}</BlockLabel>
          <strong className="mt-1.5 block font-mono text-sm text-status-green">{dossier.management.peakCash}</strong>
          <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{dossier.management.peakCashDetail}</p>
        </div>
        <div className="rounded-xl border border-status-red/25 bg-[#241014]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.management.worstCrisis")}</BlockLabel>
          <strong className="mt-1.5 block font-mono text-sm text-status-red">{dossier.management.worstCrisis}</strong>
          <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{dossier.management.worstCrisisDetail}</p>
        </div>
      </div>
      {/* Saúde e curva seguem no MESMO painel: a frase é a leitura do momento e a
          curva é a prova dela. Separadas, o jogador lia "Monitorada" sem nada que
          dissesse se a equipe está subindo ou afundando. */}
      <div className={`rounded-xl border p-4 ${tone.card}`}>
        <BlockLabel>{t("myTeamTab.history.management.operationHealth")}</BlockLabel>
        <strong className={`mt-1.5 block text-xl font-semibold ${tone.text}`}>{dossier.management.operationHealth}</strong>
        <p className="mt-2 text-[11px] leading-5 text-text-secondary">{dossier.management.summary}</p>
        {ledger ? <CashCurve ledger={ledger} /> : null}
      </div>
      <div className="grid gap-2.5 md:grid-cols-2">
        <InfoCard
          label={t("myTeamTab.history.management.healthyYears")}
          value={dossier.management.healthyYears}
          detail={dossier.management.healthyYearsDetail}
        />
        <InfoCard
          label={t("myTeamTab.history.management.biggestInvestment")}
          value={dossier.management.biggestInvestment}
          detail={dossier.management.investmentDetail}
        />
      </div>
      {dossier.ownershipEvents?.length > 0 && (
        <div className="rounded-xl border border-status-yellow/25 bg-[#201a0b]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.management.boardChanges")}</BlockLabel>
          <ul className="mt-2.5 grid gap-2.5 md:grid-cols-2">
            {dossier.ownershipEvents.map((event, index) => (
              <li key={index} className="flex items-start gap-3">
                <span className="mt-0.5 font-mono text-xs font-bold text-status-yellow">{event.year}</span>
                <div className="min-w-0">
                  <strong className="block text-xs font-semibold text-text-primary">{event.title}</strong>
                  <p className="text-[11px] leading-5 text-text-secondary">{event.financialNote}</p>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function CategoriesSection({ dossier }) {
  const { t } = useTranslation();
  const movement = dossier.movement ?? {};
  return (
    <section>
      <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-4">
        <MiniMetric label={t("myTeamTab.history.categories.promotions")} value={movement.promotions} />
        <MiniMetric label={t("myTeamTab.history.categories.relegations")} value={movement.relegations} />
        <MiniMetric label={t("myTeamTab.history.categories.peakCategory")} value={movement.peakCategory} />
        <MiniMetric label={t("myTeamTab.history.categories.homeCategory")} value={movement.homeCategory} />
      </div>
      <CategoryPyramid ladder={movement.ladder} />
      <CategoryTrajectory dossier={dossier} ladder={movement.ladder} />
      <CategoryTimeBars lines={movement.timeLines} fallback={movement.timeByCategory} />
    </section>
  );
}

// A escada INTEIRA do recorte, com os degraus nunca pisados apagados.
//
// A lista de passagens sozinha respondia "onde ela esteve" e escondia a pergunta
// que o jogo levanta o tempo todo: quanto falta para o topo. Uma estreante virava
// um card solitário — sem os dois degraus acima dele, não dava para ver que ela
// está no primeiro. A largura cresce para baixo porque a base é a categoria de
// entrada: o desenho é a pirâmide, não uma lista com título de pirâmide.
function CategoryPyramid({ ladder }) {
  const { t } = useTranslation();
  const degraus = Array.isArray(ladder) ? ladder : [];
  if (degraus.length === 0) return null;

  const doTopo = [...degraus].sort((a, b) => b.tier - a.tier);
  const passo = doTopo.length > 1 ? 42 / (doTopo.length - 1) : 0;

  return (
    <div className="mt-5" data-testid="team-history-category-pyramid">
      <BlockLabel>{t("myTeamTab.history.categories.ladder")}</BlockLabel>
      <div className="mt-2.5 flex flex-col items-center gap-1.5">
        {doTopo.map((degrau, index) => {
          const cor = getCategoryColor(degrau.categoryId) || "#58a6ff";
          const largura = 58 + passo * index;
          return (
            <div
              key={degrau.categoryId || degrau.category}
              data-category={degrau.categoryId || undefined}
              data-visited={degrau.visited ? "1" : "0"}
              className={`flex w-full items-center justify-between gap-3 rounded-lg px-3.5 py-2 ${
                degrau.visited ? "border-l-4" : "border border-dashed border-white/[0.08]"
              }`}
              style={{
                maxWidth: `${largura}%`,
                borderLeftColor: degrau.visited ? cor : undefined,
                backgroundColor: degrau.visited
                  ? `color-mix(in srgb, ${cor} 18%, transparent)`
                  : "transparent",
              }}
            >
              <div className="flex min-w-0 items-center gap-2">
                <strong
                  className={`truncate text-xs ${degrau.visited ? "text-text-primary" : "text-text-muted"}`}
                >
                  {degrau.category}
                </strong>
                {degrau.isCurrent ? (
                  <span
                    className="shrink-0 rounded px-1.5 py-0.5 text-[9px] font-black uppercase tracking-[0.12em]"
                    style={{ backgroundColor: cor, color: "#06101c" }}
                  >
                    {t("myTeamTab.history.categories.rungCurrent")}
                  </span>
                ) : null}
                {degrau.isPeak && !degrau.isCurrent ? (
                  <span className="shrink-0 font-mono text-[10px] text-accent-primary">
                    {t("myTeamTab.history.categories.rungPeak")}
                  </span>
                ) : null}
              </div>
              <span
                className={`shrink-0 font-mono text-[10px] ${degrau.visited ? "" : "text-text-muted"}`}
                style={degrau.visited ? { color: cor } : undefined}
              >
                {degrau.visited
                  ? `${t("myTeamTab.history.categories.rungSeasons", { count: degrau.seasons })} · ${degrau.years}`
                  : t("myTeamTab.history.categories.rungNever")}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// Faixa ano a ano com ALTURA igual ao degrau. A pirâmide diz onde a equipe
// esteve; esta diz quando, e no mesmo gesto separa quem subiu degrau a degrau de
// quem passou dez anos parada na entrada — que na lista de passagens saíam iguais.
function CategoryTrajectory({ dossier, ladder }) {
  const { t } = useTranslation();
  const dados = useMemo(() => {
    const passagens = (dossier.categoryPath ?? []).filter((step) => step.startYear > 0);
    if (passagens.length === 0) return null;
    const foraDoRecorte = new Map(
      (dossier.outsideScopeSeasons ?? []).map((item) => [Number(item.year), item]),
    );

    const anos = passagens.flatMap((step) => [step.startYear, step.endYear]);
    const mundoInicio = Number(dossier.worldFirstYear ?? 0);
    const mundoFim = Number(dossier.worldLastYear ?? 0);
    const inicio = Math.min(...anos, ...(mundoInicio > 0 ? [mundoInicio] : []));
    const fim = Math.max(...anos, ...(mundoFim > 0 ? [mundoFim] : []));
    if (fim < inicio) return null;

    // A régua da altura é a ESCADA do recorte, não os degraus que esta equipe
    // pisou. Normalizar pela própria equipe punha a estreante de tier 0 na altura
    // máxima — desenhando "no topo" justamente quem está na base.
    const degraus = (Array.isArray(ladder) ? ladder : []).map((rung) => rung.tier);
    const tiers = degraus.length > 0 ? degraus : passagens.map((step) => step.tier);
    const tierMin = Math.min(...tiers);
    const tierMax = Math.max(...tiers);
    const faixa = Math.max(1, tierMax - tierMin + 1);

    const celulas = [];
    for (let ano = inicio; ano <= fim; ano += 1) {
      // A passagem MAIS RECENTE do ano ganha a célula: no ano da troca, pintar a
      // que estava saindo esconderia a subida.
      const passagem = [...passagens]
        .reverse()
        .find((step) => step.startYear <= ano && ano <= step.endYear);
      const fora = passagem ? null : foraDoRecorte.get(ano);
      celulas.push({
        year: ano,
        categoryId: passagem?.categoryId ?? null,
        label: passagem?.category ?? fora?.category ?? null,
        outside: Boolean(fora),
        // Altura proporcional ao degrau. O tier de quem está fora do recorte não
        // é conhecido aqui — a célula fica baixa e neutra, e o rótulo diz por quê.
        altura: passagem ? 12 + ((passagem.tier - tierMin + 1) / faixa) * 26 : 10,
      });
    }
    return { celulas };
  }, [dossier.categoryPath, dossier.outsideScopeSeasons, dossier.worldFirstYear, dossier.worldLastYear, ladder]);

  if (!dados) return null;
  const passo = Math.max(1, Math.ceil(dados.celulas.length / 10));

  return (
    <div className="mt-5" data-testid="team-history-category-trajectory">
      <BlockLabel>{t("myTeamTab.history.categories.trajectory")}</BlockLabel>
      <div className="mt-2.5 rounded-xl bg-[#0f1c2b] px-4 py-3.5">
        <div className="flex h-[38px] items-end gap-1">
          {dados.celulas.map((celula) => (
            <Tooltip
              key={celula.year}
              texto={
                celula.outside
                  ? t("myTeamTab.history.categories.trajectoryOutside", {
                      year: celula.year,
                      category: celula.label,
                    })
                  : celula.label
                    ? t("myTeamTab.history.categories.trajectoryYear", {
                        year: celula.year,
                        category: celula.label,
                      })
                    : t("myTeamTab.history.categories.trajectoryEmpty", { year: celula.year })
              }
            >
              <span
                data-year={celula.year}
                data-category={celula.categoryId || undefined}
                className="min-w-[8px] flex-1 rounded-t"
                style={{
                  height: `${celula.altura}px`,
                  backgroundColor: celula.categoryId
                    ? getCategoryColor(celula.categoryId)
                    : celula.outside
                      ? "#2c3a4c"
                      : "#141f2c",
                }}
              />
            </Tooltip>
          ))}
        </div>
        <div className="mt-1 flex gap-1">
          {dados.celulas.map((celula, index) => (
            <span
              key={`ano-${celula.year}`}
              className="min-w-[8px] flex-1 text-center font-mono text-[10px] text-text-muted"
            >
              {index % passo === 0 ? celula.year : ""}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

// Uma linha por categoria: quanto tempo e o que a equipe fez lá.
//
// Era uma string concatenada com "·" — numa equipe de quinze anos e seis
// categorias, uma linha que ninguém lê, e ninguém compara "4 anos" com "9 anos"
// em texto tão rápido quanto em largura. O saldo veio para cá dos cards de
// passagem, que gastavam três linhas cada para repetir o que a pirâmide e a faixa
// ano a ano já desenham. Duas idas ao GT4 são UMA linha, somadas: a viagem é
// assunto da escada; aqui a pergunta é o que ela ganhou em cada degrau.
function CategoryTimeBars({ lines, fallback }) {
  const { t } = useTranslation();
  const linhas = Array.isArray(lines) ? lines : [];
  if (linhas.length === 0) {
    return fallback ? (
      <div className="mt-5">
        <InfoCard label={t("myTeamTab.history.categories.timeByCategory")} value={fallback} />
      </div>
    ) : null;
  }
  const maior = Math.max(...linhas.map((linha) => linha.seasons), 1);

  return (
    <div className="mt-5" data-testid="team-history-category-time">
      <BlockLabel>{t("myTeamTab.history.categories.byCategory")}</BlockLabel>
      <div className="mt-2.5 grid gap-2">
        {linhas.map((linha) => {
          const cor = getCategoryColor(linha.categoryId) || "#58a6ff";
          return (
            <Tooltip
              key={linha.categoryId || linha.category}
              texto={t("myTeamTab.history.categories.timeBarDetail", {
                count: linha.seasons,
                races: linha.races,
              })}
            >
              <div className="flex items-center gap-3">
                <span className="w-[34%] shrink-0 truncate text-[11px] text-text-secondary">
                  {linha.category}
                </span>
                <span className="h-2.5 flex-1 overflow-hidden rounded-full bg-white/[0.06]">
                  <span
                    className="block h-full rounded-full"
                    data-category={linha.categoryId || undefined}
                    style={{ width: `${(linha.seasons / maior) * 100}%`, backgroundColor: cor }}
                  />
                </span>
                <span
                  className="shrink-0 font-mono text-[10px] tabular-nums"
                  data-tally={linha.categoryId || undefined}
                >
                  <span style={{ color: cor }}>
                    {t("myTeamTab.history.categories.tallyWins", { count: linha.wins })}
                  </span>
                  <span className="text-text-muted">
                    {" · "}
                    {t("myTeamTab.history.categories.tallyPodiums", { count: linha.podiums })}
                  </span>
                </span>
              </div>
            </Tooltip>
          );
        })}
      </div>
    </div>
  );
}

function HistoryStateMessage({ dossier }) {
  const message = dossier.historyStatus === "error" ? dossier.historyError : i18n.t("myTeamTab.history.loading");
  return <div className="mb-3 rounded-xl border border-white/10 bg-[#08111f]/95 px-4 py-2.5 text-[11px] text-text-secondary">{message}</div>;
}

function MedalKey({ color, label }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="h-2 w-2 rounded-sm" style={{ backgroundColor: color }} />
      {label}
    </span>
  );
}

// Rótulo de bloco — a receita vale para TODO rótulo do dossiê.
//
// Era `text-[10px] uppercase tracking-[0.15em] text-text-muted`, e essa
// combinação é o pior caso possível de legibilidade: a caixa alta apaga a
// silhueta da palavra (sem ascendente nem descendente o olho perde a forma que
// usa para reconhecê-la), o espaçamento largo desmancha o que sobrou em letras
// soltas, e o cinza apagado num corpo pequeno não tem contraste para compensar.
// Cada um sozinho passaria; os quatro juntos obrigavam a soletrar.
//
// As chaves de i18n já vêm em caixa de frase, então parar de forçar `uppercase`
// devolve a capitalização certa de graça. A hierarquia não depende disso — o
// rótulo continua menor e cinza contra um valor branco e maior.
//
// NÃO reintroduza caixa alta em rótulo pequeno aqui. Se precisar de mais
// separação entre rótulo e valor, mexa no peso ou no espaçamento vertical.
function BlockLabel({ children }) {
  return <span className="block text-[11px] font-semibold text-text-secondary">{children}</span>;
}

function MiniMetric({ label, value }) {
  return (
    <div className="rounded-xl bg-[#0f1c2b] px-3.5 py-3">
      <span className="block truncate text-[11px] font-semibold text-text-secondary">{label}</span>
      <strong className="mt-1 block font-mono text-lg leading-none text-text-primary">{value}</strong>
    </div>
  );
}

function InfoCard({ label, value, detail = "" }) {
  return (
    <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <strong className="text-xs text-text-primary">{label}</strong>
        <span className="text-right font-mono text-[11px] font-semibold text-status-yellow">{value}</span>
      </div>
      {detail ? <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{detail}</p> : null}
    </div>
  );
}

// Navegação entre equipes. As setas moraram um tempo no rodapé do painel, e o
// problema era justamente esse: no rodapé elas andavam junto com o conteúdo do
// dossiê, e o alvo do clique fugia entre uma equipe e outra. Agora vivem numa
// coluna presa à calha à direita do painel, no meio da altura — o ponteiro pode
// ficar parado e só clicar.
function TeamStepButton({ label, direction, team, onSelectTeam, onStep }) {
  const Chevron = direction === "up" ? ChevronUp : ChevronDown;
  return (
    <Tooltip texto={team?.nome ?? label}>
      <button
        type="button"
        aria-label={label}
        disabled={!team}
        onClick={() => {
          if (!team) return;
          onStep?.(direction);
          onSelectTeam(team);
        }}
        data-testid={`team-history-step-${direction}`}
        className={`grid h-[92px] w-[92px] place-items-center rounded-2xl border backdrop-blur-sm transition-glass max-lg:h-16 max-lg:w-16 ${
          team
            ? "border-white/15 bg-[#0d1727]/90 text-text-secondary hover:border-white/30 hover:bg-[#14233a] hover:text-text-primary"
            : "cursor-not-allowed border-white/[0.06] bg-[#0b111a]/70 text-[#4a525d]"
        }`}
      >
        <Chevron size={34} strokeWidth={1.6} aria-hidden="true" className="max-lg:h-6 max-lg:w-6" />
      </button>
    </Tooltip>
  );
}

function MetricIcon({ name, size = 15 }) {
  const Icon = METRIC_ICONS[name];
  if (!Icon) return null;
  return <Icon size={size} strokeWidth={1.5} aria-hidden="true" className="shrink-0" />;
}

// Marca-d'água dos cards de destaque: o troféu de ouro do jogo, o MESMO que a
// classificação usa no TrophyBadge. Antes era o ícone de contorno do Lucide —
// correto como ícone de 16px, magro e sem peso como ornamento de 80px. Arte de
// verdade preenche, e a tela já tinha a dela.
//
// Puro ornamento, então `alt=""` e `aria-hidden`: quem lê por leitor de tela não
// perde nada.
function HighlightTrophy() {
  return (
    <img
      src={goldTrophy}
      alt=""
      aria-hidden="true"
      className="pointer-events-none absolute -right-4 top-1/2 h-[84px] w-[84px] -translate-y-1/2 object-contain opacity-[0.16] [filter:saturate(1.4)]"
    />
  );
}

export default TeamHistoryDrawerV2;
