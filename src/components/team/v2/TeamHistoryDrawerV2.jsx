import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import i18n from "../../../i18n/index.js";
import Tooltip from "../../ui/Tooltip";
import TeamLogoMark from "../TeamLogoMark";
import {
  Briefcase,
  ChevronDown,
  ChevronUp,
  Fingerprint,
  Layers,
  Swords,
  Trophy,
  X,
} from "lucide-react";
import { buildTeamHistoryDossier, orderTeamsForHistoryNavigation } from "../teamHistoryDossier";
import { getVividTeamColor } from "../../../utils/teamColors";
import { pisoDeAbertura } from "../../ui/aberturaDePainel.js";
import { campanhaTemDados, curvaTemDados } from "./teamHistoryV2Logic";
import {
  HistoryStateMessage,
  MetricIcon,
  HighlightTrophy,
} from "./teamHistoryV2Primitives.jsx";
import { TitleGallery } from "./TeamHistoryTitles.jsx";
import { SeasonTrajectory, RecentForm } from "./TeamHistoryTrajectory.jsx";
import { ChampionshipEvolution } from "./TeamHistoryChampionship.jsx";
import { ResultSpread, ReliabilityPanel } from "./TeamHistoryResults.jsx";
import { TeamLineup, BestDrivers } from "./TeamHistoryLineup.jsx";
import { IdentitySection } from "./TeamHistoryIdentity.jsx";
import { ManagementSection } from "./TeamHistoryMoney.jsx";
import { CategoriesSection } from "./TeamHistoryCategories.jsx";

// Dossiê de equipe.
//
// Os dados vêm de get_team_history_dossier e são normalizados por
// `buildTeamHistoryDossier` (../teamHistoryDossier.js). A tela abre CENTRALIZADA
// e larga, com:
//
//   • cabeçalho-herói com os números-âncora sempre visíveis;
//   • seções numa coluna lateral, liberando a largura toda para o conteúdo;
//   • records como cards com barra de posição e média do grupo — o rank deixa
//     de ser um número entre parênteses e vira a informação com mais peso;
//   • trajetória por temporada e marcos ancorados no MESMO eixo de anos.
//
// Ícones vêm do lucide-react: traço de 1.5px numa grade de 24, igual para os
// onze. Os SVGs desenhados à mão que estavam aqui variavam de espessura entre si
// e ficavam sujos a 12px, que é o tamanho em que a maioria aparece.
//
// O que mora AQUI é a composição: o carregamento do dossiê, o estado de alto
// nível (seção aberta, realce entre blocos irmãos, navegação entre equipes), o
// cabeçalho-herói e as seções Records e Identidade, que são só arranjo dos
// painéis. Cada painel pesado mora no irmão da sua área — [TeamHistoryTitles],
// [TeamHistoryTrajectory], [TeamHistoryChampionship], [TeamHistoryResults],
// [TeamHistoryLineup], [TeamHistoryIdentity], [TeamHistoryMoney] e
// [TeamHistoryCategories] —, com o vocabulário comum a todos em
// [teamHistoryV2Primitives.jsx]. O arquivo tinha 4.298 linhas em 11/08/2026 e a
// vistoria marcou o tamanho como [Alta]: o mesmo caminho que já havia tirado
// daqui `atlasV2Geometry.js`, `teamHistoryV2Logic.js` e `teamHistoryV2Labels.js`.
//
// Os IDs divergem dos rótulos de propósito. `sport` é a seção que hoje se chama
// "Identidade" (o retrato esportivo virou o retrato da equipe) e `identity` é a
// que se chama "Rival". Renomear os ids arrastaria o estado persistido de seção
// e os testes por uma troca que é só de vocabulário — o rótulo mora no i18n, que
// é onde ele deve morar.
const TEAM_HISTORY_SECTIONS = [
  { id: "records", Icon: Trophy },
  { id: "sport", Icon: Fingerprint },
  { id: "identity", Icon: Swords },
  { id: "management", Icon: Briefcase },
  { id: "categories", Icon: Layers },
];

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

export default TeamHistoryDrawerV2;

