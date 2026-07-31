import { useEffect, useId, useMemo, useState } from "react";
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
  Flag,
  Layers,
  LayoutGrid,
  Medal,
  Rows3,
  TrendingUp,
  Trophy,
  X,
} from "lucide-react";

import goldTrophy from "../../../assets/utilities/trophies/ouro.png";
import TeamLogoMark from "../TeamLogoMark";
import FlagIcon from "../../ui/FlagIcon";
import {
  buildTeamHistoryDossier,
  operationHealthTone,
  orderTeamsForHistoryNavigation,
} from "../TeamHistoryDrawer";
import {
  SPORT_LAYOUT_ARRANGED,
  SPORT_LAYOUT_CLASSIC,
  readSportLayout,
  writeSportLayout,
} from "./sportLayoutPreference";
import i18n from "../../../i18n/index.js";
import { getCategoryColor } from "../../../utils/categoryColors";
import { getVividTeamColor } from "../../../utils/teamColors";

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
const TEAM_HISTORY_SECTIONS = [
  { id: "records", Icon: Trophy },
  { id: "sport", Icon: Flag },
  { id: "identity", Icon: Fingerprint },
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

    setHistoryStatus("loading");
    setHistoryError("");
    setHistoryDossier(null);
    invoke("get_team_history_dossier", {
      careerId,
      teamId: team.id,
      category: activeCategory ?? playerTeam?.categoria ?? team?.categoria ?? "",
    })
      .then((payload) => {
        if (!mounted) return;
        setHistoryDossier(payload);
        setHistoryStatus("ready");
      })
      .catch((invokeError) => {
        if (!mounted) return;
        setHistoryError(typeof invokeError === "string" ? invokeError : i18n.t("myTeamTab.history.loadError"));
        setHistoryStatus("error");
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
        className="animate-scale-in relative flex max-h-[92vh] w-full flex-col overflow-hidden rounded-[28px] border border-white/15 bg-[#07101d] shadow-[0_30px_90px_rgba(0,0,0,0.72)]"
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
            <div
              key={anchor.key}
              data-anchor={anchor.key}
              data-highlighted={highlighted ? "true" : undefined}
              title={highlighted ? t("myTeamTab.history.records.bestRankAria", { rank: anchor.rankPosition }) : undefined}
              className={`min-w-[86px] rounded-xl border px-3 py-2 text-center ${
                highlighted
                  ? "border-[color-mix(in_srgb,var(--team)_55%,transparent)] bg-[color-mix(in_srgb,var(--team)_12%,#0f1c2b)]"
                  : "border-transparent bg-[#0f1c2b]"
              }`}
            >
              <span className="flex items-center justify-center gap-1.5 text-[10px] uppercase tracking-[0.12em] text-text-secondary">
                <MetricIcon name={anchor.icon} />
                <span className="truncate">{anchor.label}</span>
              </span>
              <AnchorValue value={anchor.value} />
            </div>
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

      {dossier.highlights?.length > 0 && (
        <div className="mt-2.5 grid gap-2.5 sm:grid-cols-3">
          {dossier.highlights.map((item) => (
            <div
              key={item.label}
              className="relative overflow-hidden rounded-xl border border-status-yellow/25 bg-[#1c1808]/95 px-3.5 py-3"
            >
              {/* O troféu é marca-d'água: fica atrás do texto, recortado pela
                  borda do card, e é o que separa "destaque" de "mais um card". */}
              <HighlightTrophy />
              <div className="relative">
                <span className="block text-[10px] font-semibold uppercase tracking-[0.13em] text-status-yellow/80">{item.label}</span>
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
      />

      <TitleGallery titles={dossier.titleCategories} seasons={dossier.seasonResults} />
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
function TitleGallery({ titles, seasons }) {
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
          return (
            <span
              key={celula.year}
              data-year={celula.year}
              data-title={celula.title ? "true" : undefined}
              data-double={celula.title?.championIsTeam ? "true" : undefined}
              title={
                celula.title
                  ? `${celula.year} · ${celula.title.category}`
                  : t("myTeamTab.history.records.titleRailEmpty", { year: celula.year })
              }
              className="h-5 min-w-[10px] flex-1 rounded"
              style={{
                backgroundColor: cor || "#141f2c",
                // O anel dourado é a dobradinha. Como ele fica DENTRO da célula,
                // não empurra as vizinhas nem desalinha os rótulos de ano.
                boxShadow: celula.title?.championIsTeam
                  ? `inset 0 0 0 1.5px ${MEDAL_COLORS.first}`
                  : undefined,
              }}
            />
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
        <span key={year} className="min-w-[10px] flex-1 text-center font-mono text-[9px] text-text-muted">
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
        <div className="grid grid-cols-[52px_60px_34px_minmax(0,1fr)] gap-x-3 bg-[#0f1c2b] px-3.5 py-1.5 text-[9px] font-semibold uppercase tracking-[0.12em] text-text-muted">
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
      <span className="block truncate pr-7 text-[10px] font-semibold uppercase tracking-[0.13em] text-text-muted">{record.label}</span>
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
// O 4º-5º é cinza-azulado apagado de propósito: precisa ficar VISÍVEL sem
// competir com os metais, e não pode ser a cor da equipe — há equipes amarelas,
// e uma barra amarela ao lado do ouro viraria adivinhação.
const MEDAL_COLORS = {
  first: "#f2c46d",
  second: "#c2ccd8",
  third: "#c07f4a",
  nearMiss: "#46586d",
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
// baixo, na mesma ordem. O `title` nativo quebra linha no \n.
function seasonTooltip(t, { row, races, topFive, steps }) {
  const base = "myTeamTab.history.records.seasonTooltip";
  const header = row.category ? `${row.year} · ${row.category}` : String(row.year);
  const hasPosition = row.position && row.position !== "—";
  const lines = [
    header,
    hasPosition
      ? t(`${base}.meta`, { position: row.position, races, topFive })
      : t(`${base}.metaNoPosition`, { races, topFive }),
    "",
  ];
  if (steps.length) {
    for (const step of steps) {
      // `value` e não `count`: `count` é palavra reservada do i18next e ligaria
      // a máquina de plural, mandando procurar chaves `..._one`/`..._other`.
      lines.push(t(`${base}.count`, { value: step.count, label: t(`myTeamTab.history.records.medals.${step.id}`) }));
    }
  } else {
    lines.push(t(`${base}.empty`));
  }
  return lines.join("\n");
}

function SeasonTrajectory({ seasons, worldFirstYear, worldLastYear, outsideSeasons }) {
  const { t } = useTranslation();
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
      const steps = [
        { id: "first", count: wins, color: MEDAL_COLORS.first },
        { id: "second", count: seconds, color: MEDAL_COLORS.second },
        { id: "third", count: thirds, color: MEDAL_COLORS.third },
        { id: "nearMiss", count: nearMiss, color: MEDAL_COLORS.nearMiss },
      ].filter((step) => step.count > 0);
      raced.set(Number(row.year), {
        year: String(row.year),
        raced: true,
        topFiveRate: (topFive / races) * 100,
        steps,
        categoryId: row.categoryId || "",
        categoryLabel: row.category || "",
        title: seasonTooltip(t, { row, races, topFive, steps }),
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
        title: outra
          ? t("myTeamTab.history.records.seasonTooltip.elsewhere", {
              year,
              category: outra.category,
            })
          : t("myTeamTab.history.records.seasonTooltip.absent", { year }),
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
              className="absolute right-0 -translate-y-1/2 font-mono text-[9px] text-text-muted"
              style={{ top: `${100 - tick}%` }}
            >
              {`${tick}%`}
            </span>
          ))}
        </div>
        <div className="relative min-w-0 flex-1 overflow-x-auto">
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
                className="relative h-full min-w-[24px] max-w-[64px] flex-1"
                title={bar.title}
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
                    {bar.steps.map((step) => (
                      <div
                        key={step.id}
                        data-step={step.id}
                        className="w-full"
                        style={{
                          flexGrow: step.count,
                          flexBasis: 0,
                          minHeight: "3px",
                          // Gradiente sutil: dá volume à barra sem apagar a cor
                          // da colocação, que é o que precisa ser reconhecida.
                          backgroundImage: `linear-gradient(180deg, ${step.color}, color-mix(in srgb, ${step.color} 72%, #0b1524))`,
                        }}
                      />
                    ))}
                  </div>
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
                title={bar.categoryLabel || undefined}
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
        <span>{t("myTeamTab.history.records.topFivePerRaceLegend")}</span>
      </div>
      {categorias.length > 0 && (
        <div className="mt-1.5 flex flex-wrap items-center gap-3 text-[10px] text-text-muted" data-testid="team-history-trajectory-legend">
          <span className="uppercase tracking-[0.12em] text-text-muted/80">
            {t("myTeamTab.history.records.categoryBand")}
          </span>
          {categorias.map((cat) => (
            <MedalKey key={cat.id} color={getCategoryColor(cat.id)} label={cat.label} />
          ))}
        </div>
      )}
    </div>
  );
}

// Ordem de leitura quando dois fatos caem no mesmo ano — a estreia vem antes do
// primeiro pódio, que vem antes da primeira vitória, e assim por diante.
const HISTORY_KIND_ORDER = ["first_race", "first_podium", "first_win", "first_title", "last_record"];

// Marcos e linha do tempo eram DOIS blocos, e os dois contavam a primeira
// vitória — um como "Primeira vitória / 2017", o outro como "Primeira vitória
// real em Mazda Championship, rodada 1". Aqui viram uma linha do tempo só, em
// ordem cronológica, e o fato repetido fica com a versão que traz categoria e
// rodada. A fusão é por `kind` e não por texto: prosa traduzida não é chave.
//
// A linha é VERTICAL: na horizontal o rótulo comprido tinha que ser truncado
// para caber na coluna, e o fio entre os pontos não existia — eram marcos
// soltos, sem a leitura de sequência que é o assunto do bloco.
function HistoryRail({ milestones, timeline, espacado = true }) {
  const { t } = useTranslation();
  const eventos = useMemo(() => {
    const daLinha = Array.isArray(timeline) ? timeline : [];
    const kinds = new Set(daLinha.map((item) => item.kind).filter(Boolean));
    const itens = [
      ...daLinha.map((item) => ({
        kind: item.kind || "",
        year: String(item.year ?? ""),
        text: item.text ?? "",
      })),
      ...(Array.isArray(milestones) ? milestones : [])
        .filter((item) => !item.kind || !kinds.has(item.kind))
        .map((item) => ({
          kind: item.kind || "",
          year: String(item.year ?? ""),
          text: item.label ?? "",
        })),
    ];
    return itens.sort((a, b) => {
      const anos = Number(a.year) - Number(b.year);
      if (anos) return anos;
      return HISTORY_KIND_ORDER.indexOf(a.kind) - HISTORY_KIND_ORDER.indexOf(b.kind);
    });
  }, [milestones, timeline]);

  if (!eventos.length) return null;
  return (
    <div className={espacado ? "mt-5" : ""}>
      <BlockLabel>{t("myTeamTab.history.timeline.title")}</BlockLabel>
      {/* A trilha corre para o LADO, na direção em que o tempo já corre no resto
          da aba — a forma recente e a curva de campeonato leem da esquerda para a
          direita, e a cronologia lia de cima para baixo. Empilhada ela ainda
          custava uma linha por marco e esticava o dossiê conforme a equipe
          tivesse dois ou cinco.

          O texto QUEBRA na coluna, não é truncado: foi por truncar que a versão
          horizontal antiga foi abandonada — "Primeira vitória real em Mazda
          Rookie, rodada 1." vira "Primeira vitória re…" e deixa de ser um fato.
          Colunas de no mínimo 150px numa faixa que rola de lado quando a janela
          é estreita demais para todas. */}
      <ol
        className="mt-3 grid auto-cols-[minmax(150px,1fr)] grid-flow-col gap-x-4 overflow-x-auto pb-1"
        data-testid="team-history-milestones"
      >
        {eventos.map((evento, index) => (
          <li key={`${evento.kind}-${evento.year}-${index}`} className="relative min-w-0 pt-[15px]">
            {/* O fio só segue até o PRÓXIMO ponto: no último ele viraria uma
                ponta solta apontando para nada. Ele atravessa o vão entre as
                colunas, então soma o gap à largura. */}
            {index < eventos.length - 1 ? (
              <span className="absolute left-[5px] top-[4px] h-px w-[calc(100%+16px-5px)] bg-white/15" />
            ) : null}
            <span className="absolute left-0 top-0 h-2.5 w-2.5 rounded-full bg-[color:var(--team)]" />
            <strong className="block font-mono text-xs leading-none text-[color:var(--team)]">{evento.year}</strong>
            <span className="mt-1.5 block text-[11px] leading-[15px] text-text-secondary">{evento.text}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}

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
function RecentForm({ races, espacado = true }) {
  const { t } = useTranslation();
  if (!races?.length) return null;
  const primeira = races[0];
  const ultima = races[races.length - 1];
  // Troca de categoria no meio da fita é a explicação de uma queda que, sem ela,
  // se leria como perda de forma.
  const trocou = primeira.categoryId && ultima.categoryId && primeira.categoryId !== ultima.categoryId;
  return (
    <div className={espacado ? "mt-5" : ""}>
      <BlockLabel>{t("myTeamTab.history.sport.recentForm")}</BlockLabel>
      <div className="mt-2.5 flex gap-1.5" data-testid="team-history-recent-form">
        {races.map((race, index) => {
          const pos = Number(race.position) || 0;
          return (
            <span
              key={`${race.year}-${race.round}-${index}`}
              data-position={pos || undefined}
              title={
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
              className="grid h-9 flex-1 place-items-center rounded-md font-mono text-[11px]"
              style={{ backgroundColor: placementTone(pos || 99), color: placementInk(pos || 99) }}
            >
              {pos || "—"}
            </span>
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
const RUN_MODE_POSITION = "posicao";
const RUN_MODE_POINTS = "pontos";
// Cinza das outras equipes. Elas precisam existir — sem o campo, a linha da
// equipe é só um traço subindo, e subir é o que toda linha acumulada faz — mas
// não podem competir: um fio fino, sem marcador e sem etiqueta.
const RUN_FIELD_STROKE = "#3a4d63";

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
function ChampionshipRun({ run, espacado = true }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  // O modo é o que o eixo MEDE, e é a diferença entre um gráfico com nuance e um
  // feixe de retas paralelas.
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
  const [modo, setModo] = useState(RUN_MODE_POSITION);
  const dados = useMemo(() => {
    const rounds = Array.isArray(run?.rounds) ? run.rounds : [];
    const lines = Array.isArray(run?.lines) ? run.lines : [];
    if (rounds.length < 2 || !lines.length) return null;
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
  // A mancha desce até a base do eixo nos dois modos — é o corpo da linha, não
  // uma medida por si.
  const baseArea = dados.y(dados.baixo);

  return (
    <div className={espacado ? "mt-5" : ""}>
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
        <div className="flex items-baseline gap-2">
          <BlockLabel>{t("myTeamTab.history.sport.championshipRun")}</BlockLabel>
          <span className="font-mono text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.runScope", { year: run.year, category: run.category })}
          </span>
          {run.live ? (
            <span className="rounded-full bg-white/[0.07] px-2 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-text-muted">
              {t("myTeamTab.history.sport.runLive")}
            </span>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {/* Os dois modos ficam à vista, e não num menu: o eixo muda de
              significado entre eles, e um gráfico que mede outra coisa sem
              anunciar é um gráfico que mente. */}
          <div className="flex overflow-hidden rounded-lg border border-white/10" data-testid="team-history-run-mode">
            {[RUN_MODE_POSITION, RUN_MODE_POINTS].map((id) => (
              <button
                key={id}
                type="button"
                onClick={() => setModo(id)}
                data-mode={id}
                data-active={modo === id ? "true" : undefined}
                className={`px-2 py-1 text-[9px] font-bold uppercase tracking-[0.1em] transition-glass ${
                  modo === id ? "bg-white/[0.09] text-text-primary" : "text-text-muted hover:text-text-secondary"
                }`}
              >
                {t(`myTeamTab.history.sport.runMode.${id}`)}
              </button>
            ))}
          </div>
          {/* A colocação vira pílula na cor da equipe: é o veredito do gráfico, e
              procurá-lo contando linhas de cima para baixo seria trabalho. */}
          {selecionada ? (
            <span
              data-testid="team-history-run-position"
              className="flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[9px] font-bold uppercase tracking-[0.12em]"
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

          {/* Régua de rodadas embaixo da linha do eixo: acima é pontuação,
              abaixo é quando. */}
          {dados.rounds.map((round, index) =>
            index % dados.saltoRotulo === 0 || index === dados.rounds.length - 1 ? (
              <text
                key={`rodada-${round}`}
                x={dados.x(index)}
                y={RUN_ROUND_Y}
                textAnchor="middle"
                fontSize="9.5"
                fill="#66788d"
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
const CURVE_LEFT = 44;
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

// Curva de campeonato: a posição FINAL por temporada.
//
// Não repete a faixa de top 5 de Records: aquela mede corrida a corrida, esta
// mede o campeonato. Uma equipe regular pode ter poucos top 5 e ainda terminar
// em P3 — quando os dois gráficos discordam, a discordância É a informação.
function ChampionshipCurve({ seasons, espacado = true }) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  const dados = useMemo(() => {
    const rows = (Array.isArray(seasons) ? seasons : []).filter((row) => Number(row.races) > 0);
    if (rows.length < 2) return null;
    const pontos = rows.map((row, index) => {
      const digitos = String(row.position ?? "").match(/\d+/);
      return {
        index,
        year: String(row.year ?? ""),
        category: row.category || "",
        categoryId: row.categoryId || "",
        position: digitos ? Number(digitos[0]) : null,
      };
    });
    const conhecidas = pontos.map((p) => p.position).filter((p) => p !== null);
    if (!conhecidas.length) return null;
    // O fundo da escala nunca sobe acima de P6: numa equipe que só terminou em
    // P1 e P2, esticar o eixo entre as duas transformaria um degrau em abismo.
    const pior = Math.max(6, ...conhecidas);
    const passo = pontos.length > 1 ? (CURVE_RIGHT - CURVE_LEFT) / (pontos.length - 1) : 0;
    const y = (pos) => CURVE_TOP + ((pos - 1) / (pior - 1)) * (CURVE_BOTTOM - CURVE_TOP);
    const comXY = pontos.map((p) => ({
      ...p,
      x: CURVE_LEFT + p.index * passo,
      y: p.position === null ? null : y(p.position),
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
    return { pontos: comXY, trechos, pior, passo, y, meio: y(3) };
  }, [seasons]);

  if (!dados) return null;
  const { pontos, trechos, pior, passo, y, meio } = dados;
  const rotulos = pontos.length > 8 ? 2 : 1;
  // Ids de gradiente precisam ser únicos no documento: o dossiê pode estar aberto
  // ao lado de outro gráfico com os mesmos nomes, e o `url(#...)` pega o primeiro.
  const areaId = `${uid}-area`;
  const podiumId = `${uid}-podium`;
  const glowId = `${uid}-glow`;
  // O chip por temporada só cabe quando as colunas são largas. Numa carreira
  // longa sobram os que carregam informação sozinhos: os títulos e a última
  // temporada já fechada.
  const ultimoFechado = [...pontos].reverse().find((ponto) => ponto.y !== null);
  const chipEmTodos = passo >= CHIP_MIN_STEP;
  return (
    <div className={espacado ? "mt-5" : ""}>
      <div className="flex items-center justify-between gap-3">
        <BlockLabel>{t("myTeamTab.history.sport.championshipCurve")}</BlockLabel>
        {/* A legenda do pódio é uma pílula, não um texto solto dentro do desenho:
            lá dentro ela disputava espaço com a curva justamente nas temporadas
            boas, que são as que sobem até a faixa. Aqui em cima nunca colide. */}
        <span className="flex items-center gap-1.5 rounded-full border border-[#3fbf7f]/35 bg-[#3fbf7f]/[0.07] px-2.5 py-1 text-[9px] font-bold uppercase tracking-[0.12em] text-[#63c795]">
          <Trophy size={11} strokeWidth={2.4} className="text-status-yellow" />
          {t("myTeamTab.history.sport.championshipPodium")}
        </span>
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
            {/* O pódio era um bloco verde chapado com aresta dura no meio da tela —
                lia como um segundo gráfico. Agora ele desbota de cima para baixo e
                é fechado por uma divisa em P3: continua sendo uma zona, e não uma
                peça sobreposta. */}
            <linearGradient id={podiumId} x1="0" y1={CURVE_TOP} x2="0" y2={meio} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="#3fbf7f" stopOpacity="0.13" />
              <stop offset="100%" stopColor="#3fbf7f" stopOpacity="0.02" />
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

          {/* Faixa do pódio do campeonato: P1 a P3. É a régua que diz se a
              temporada foi boa sem o leitor ter que ler o eixo. */}
          <rect
            x={CURVE_LEFT}
            y={CURVE_TOP}
            width={CURVE_RIGHT - CURVE_LEFT}
            height={meio - CURVE_TOP}
            fill={`url(#${podiumId})`}
            rx="3"
          />
          <line
            x1={CURVE_LEFT}
            y1={meio}
            x2={CURVE_RIGHT}
            y2={meio}
            stroke="#3fbf7f"
            strokeOpacity="0.24"
            strokeDasharray="4 4"
          />

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

          {[1, Math.ceil((pior + 1) / 2), pior].map((tick) => (
            <g key={tick}>
              <line
                x1={CURVE_LEFT}
                y1={y(tick)}
                x2={CURVE_RIGHT}
                y2={y(tick)}
                stroke="#ffffff"
                strokeOpacity={tick === 1 ? 0.1 : 0.05}
                strokeDasharray={tick === 1 ? undefined : "2 5"}
              />
              <text
                x={CURVE_LEFT - 9}
                y={y(tick) + 3.4}
                textAnchor="end"
                fontSize="10"
                fontWeight={tick === 1 ? 700 : 500}
                fill={tick === 1 ? "#c2d2e2" : "#66788d"}
              >
                {`P${tick}`}
              </text>
            </g>
          ))}
          {/* O troféu marca P1 no eixo. Sem ele, o topo da escala é só mais um
              número — e o topo da escala é o objetivo do jogo inteiro. */}
          <image href={goldTrophy} x={CURVE_LEFT - 40} y={y(1) - 6.5} width="13" height="13" opacity="0.9" />

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
                  <title>
                    {t("myTeamTab.history.sport.curveTooltip", {
                      year: ponto.year,
                      category: ponto.category,
                      position: ponto.position,
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
            const texto = `P${ponto.position}`;
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

function ResultSpread({ spread, espacado = true }) {
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
    <div className={espacado ? "mt-5" : ""}>
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.resultSpread")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.spreadRaces", { value: spread.races })}
        </span>
      </div>
      <div className="mt-2.5 flex h-7 overflow-hidden rounded-md" data-testid="team-history-spread">
        {faixas.map((faixa) => (
          <span
            key={faixa.id}
            data-band={faixa.id}
            title={`${t(`myTeamTab.history.sport.spread.${faixa.id}`)} · ${rotuloFaixa(t, faixa)}`}
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
function ReliabilityPanel({ reliability, espacado = true, compacto = false }) {
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
              <span
                key={faixa.id}
                data-band={faixa.id}
                title={rotulo(faixa)}
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
    <div className={espacado ? "mt-5" : ""} data-testid="team-history-reliability">
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
          <span className="mt-1 block text-[10px] uppercase tracking-[0.12em] text-text-muted">
            {t("myTeamTab.history.sport.finishRate")}
          </span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex h-5 overflow-hidden rounded-md">
            {faixas.map((faixa) => (
              <span
                key={faixa.id}
                data-band={faixa.id}
                title={`${t(`myTeamTab.history.sport.rel${faixa.id[0].toUpperCase()}${faixa.id.slice(1)}`)} · ${faixa.value}`}
                style={{ flexGrow: faixa.value, flexBasis: 0, backgroundColor: RELIABILITY_COLORS[faixa.id] }}
              />
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

// Pilotos que passaram pela equipe, repartidos pelas DUAS vagas. É o único bloco
// do dossiê que fala de gente — todo o resto trata a equipe como um carro só — e,
// em duas colunas, também responde quanto essa casa troca de piloto: uma que
// manteve o mesmo titular por seis anos e outra que troca todo ano desenham
// diferente antes de qualquer número ser lido.
function TeamLineup({ lineup, espacado = true }) {
  const { t } = useTranslation();
  if (!lineup?.length) return null;
  // Vagas 1 e 2 sempre lado a lado; a faixa de "outras passagens" só existe
  // quando alguém correu sem constar como titular de temporada arquivada.
  const colunas = [1, 2]
    .map((slot) => ({ slot, itens: lineup.filter((item) => item.slot === slot) }))
    .filter((coluna) => coluna.itens.length > 0);
  const avulsos = lineup.filter((item) => item.slot !== 1 && item.slot !== 2);
  return (
    <div className={espacado ? "mt-5" : ""} data-testid="team-history-lineup">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.alumni")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.lineupCount", { value: lineup.length })}
        </span>
      </div>
      <div className="mt-2.5 grid gap-x-3 gap-y-3 sm:grid-cols-2">
        {colunas.map((coluna) => (
          <div key={coluna.slot} data-slot={coluna.slot}>
            <span className="block text-[9px] font-bold uppercase tracking-[0.14em] text-text-muted">
              {t(`myTeamTab.history.sport.lineupSlot${coluna.slot}`)}
            </span>
            <LineupColumn itens={coluna.itens} />
          </div>
        ))}
        {avulsos.length ? (
          <div data-slot="0" className="sm:col-span-2">
            <span className="block text-[9px] font-bold uppercase tracking-[0.14em] text-text-muted">
              {t("myTeamTab.history.sport.lineupSlotOther")}
            </span>
            <LineupColumn itens={avulsos} />
          </div>
        ) : null}
      </div>
    </div>
  );
}

// Uma coluna da galeria. As passagens vêm em ordem cronológica do backend, e a
// coluna só se lê como sucessão se essa ordem sobreviver ao desenho.
function LineupColumn({ itens }) {
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
              // Quem está na equipe HOJE é o fim da coluna e o começo da leitura:
              // ganha faixa lateral e fundo na cor da casa. Passagem encerrada
              // fica em cinza — a diferença entre as duas é a informação, e
              // "ainda na equipe" escrito em texto era fácil demais de perder no
              // meio de oito linhas iguais.
              className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-0.5 rounded-lg border px-3 py-2 ${
                piloto.stillHere
                  ? "border-l-2 border-[color-mix(in_srgb,var(--team)_50%,transparent)] border-l-[color:var(--team)] bg-[color-mix(in_srgb,var(--team)_10%,#0f1c2b)]"
                  : piloto.isPlayer
                    ? "border-[color-mix(in_srgb,var(--team)_45%,transparent)] bg-[color-mix(in_srgb,var(--team)_12%,#0f1c2b)]"
                    : "border-white/[0.06] bg-[#0f1c2b]"
              }`}
            >
              {/* A bandeira ocupa as duas linhas do cartão, à esquerda de tudo:
                  é o retrato do piloto que a galeria não tem. Vem do país porque
                  é o único traço visual que o save guarda dele — e é ele que faz
                  a coluna se ler como gente, e não como oito linhas de texto. */}
              <span
                className="row-span-2 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-white/[0.04] ring-1 ring-inset ring-white/[0.07]"
                data-nationality={piloto.nationality || undefined}
                title={piloto.nationality || undefined}
              >
                <FlagIcon nacionalidade={piloto.nationality} />
              </span>
              <span className="flex min-w-0 items-center gap-1.5">
                <strong className="truncate text-xs font-semibold text-text-primary">{piloto.name}</strong>
                {piloto.isPlayer ? (
                  <span className="shrink-0 rounded px-1 py-px text-[9px] font-bold uppercase tracking-[0.1em] text-[color:var(--team)] ring-1 ring-[color:var(--team)]/50">
                    {t("myTeamTab.history.sport.alumniYou")}
                  </span>
                ) : null}
                {piloto.stillHere ? (
                  <span className="shrink-0 rounded bg-[color:var(--team)] px-1 py-px text-[9px] font-bold uppercase tracking-[0.1em] text-[#07101d]">
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

// A seção Esportivo desenha em dois arranjos. O "arrumado" é o padrão; o
// "clássico" é o empilhamento original, a um clique de distância. Os dois leem os
// mesmos campos do dossiê — trocar de layout nunca esconde nem inventa dado.
function SportSection({ dossier }) {
  const { t } = useTranslation();
  const [layout, setLayout] = useState(readSportLayout);
  const arrumado = layout !== SPORT_LAYOUT_CLASSIC;

  function alternar() {
    const proximo = arrumado ? SPORT_LAYOUT_CLASSIC : SPORT_LAYOUT_ARRANGED;
    setLayout(proximo);
    writeSportLayout(proximo);
  }

  return (
    <section>
      {/* O botão fica acima de tudo e à direita, fora da coluna de leitura: é
          controle da tela, não conteúdo dela. O rótulo diz para ONDE o clique
          leva, nunca onde você está — "Layout clássico" num botão que já está no
          clássico é a ambiguidade clássica dos alternadores. */}
      <div className="mb-1 flex justify-end">
        <button
          type="button"
          onClick={alternar}
          data-testid="team-history-sport-layout"
          data-layout={arrumado ? SPORT_LAYOUT_ARRANGED : SPORT_LAYOUT_CLASSIC}
          className="flex items-center gap-1.5 rounded-lg border border-white/10 px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.1em] text-text-muted transition-glass hover:border-white/20 hover:bg-white/[0.05] hover:text-text-secondary"
        >
          {arrumado ? <Rows3 size={12} strokeWidth={2} aria-hidden="true" /> : <LayoutGrid size={12} strokeWidth={2} aria-hidden="true" />}
          {arrumado ? t("myTeamTab.history.sport.layoutToClassic") : t("myTeamTab.history.sport.layoutToArranged")}
        </button>
      </div>
      {dossier.historyStatus !== "ready" ? <HistoryStateMessage dossier={dossier} /> : null}
      {arrumado ? <SportArranged dossier={dossier} /> : <SportClassic dossier={dossier} />}
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
//   • Quem correu por ela — a galeria de pilotos e a cronologia dos marcos.
//
// Os títulos de grupo são o nível que faltava. Os rótulos de bloco continuam
// exatamente onde estavam, agora lendo como subtítulo do grupo em vez de
// competirem entre si.
function SportArranged({ dossier }) {
  const { t } = useTranslation();
  const temTermino = dossier.reliability?.races > 0 || dossier.resultSpread?.races > 0;
  const temEvolucao =
    Boolean(dossier.championshipRun) ||
    (Array.isArray(dossier.seasonResults) ? dossier.seasonResults : []).filter((row) => Number(row.races) > 0).length >= 2 ||
    dossier.recentForm?.length > 0;
  const temGente =
    dossier.lineup?.length > 0 || dossier.timeline?.length > 0 || dossier.milestones?.length > 0;

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
              <ReliabilityPanel reliability={dossier.reliability} espacado={false} compacto />
            </div>
            <div className="min-w-0 flex-1 basis-[300px] empty:hidden">
              <ResultSpread spread={dossier.resultSpread} espacado={false} />
            </div>
          </div>
        </SportGroup>
      ) : null}

      {temEvolucao ? (
        <SportGroup title={t("myTeamTab.history.sport.groupTrajectory")}>
          {/* A campanha do campeonato no lugar da curva de posição por
              temporada. As duas respondem coisas diferentes — a curva diz ONDE a
              equipe terminou cada ano, a campanha diz COMO a última foi
              disputada — e é a segunda que conversa com a fita logo abaixo: as
              mesmas corridas, agora contra o campo.

              A curva sobrevive como reserva, para quando não há campanha para
              desenhar: equipe com uma corrida só na última temporada, ou save
              antigo cujo backend ainda não manda o recorte. Ela também continua
              inteira no arranjo clássico. */}
          {dossier.championshipRun ? (
            <ChampionshipRun run={dossier.championshipRun} espacado={false} />
          ) : (
            <ChampionshipCurve seasons={dossier.seasonResults} espacado={false} />
          )}
          {/* A fita entra DEPOIS do gráfico aqui, invertendo a ordem do clássico:
              lá ela vinha antes porque era o bloco do presente e abria a seção;
              aqui ela é o zoom final do mesmo eixo, e o zoom vem depois do
              panorama. */}
          {/* `first:mt-0` porque o gráfico some quando não há campanha nem duas
              temporadas fechadas: aí a fita vira o primeiro filho do cartão e o
              respiro de cima seria um vão sem nada acima dele. */}
          <div className="mt-4 first:mt-0">
            <RecentForm races={dossier.recentForm} espacado={false} />
          </div>
        </SportGroup>
      ) : null}

      {temGente ? (
        <SportGroup title={t("myTeamTab.history.sport.groupPeople")}>
          <TeamLineup lineup={dossier.lineup} espacado={false} />
          <div className="mt-5 empty:hidden first:mt-0">
            <HistoryRail milestones={dossier.milestones} timeline={dossier.timeline} espacado={false} />
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

function SportClassic({ dossier }) {
  return (
    <>

      {/* Nem temporadas disputadas nem taxa de pódio/vitória se repetem aqui.
          Temporadas é âncora do cabeçalho, visível em qualquer seção; as taxas
          são dois dos cinco cards de Records, e lá vêm com a média do grupo e a
          posição no ranking. Repetir o número cru aqui era a versão pior do
          mesmo dado.

          As duas sequências (atual e melhor) saíram pelo mesmo motivo, com um
          agravante: elas ocupavam a primeira dobra inteira da seção com prosa
          que os três gráficos abaixo já contam melhor. "7 temporadas seguidas no
          nível Rookie" é a tira de categoria da curva; "2 pódios consecutivos" é
          a fita de forma recente — e nas duas dá para ver QUANDO aconteceu. */}

      {/* A tabela temporada a temporada saiu daqui. Depois que Records ganhou a
          faixa de top 5 por corrida com a tira de categoria, ela era o mesmo
          conteúdo em números — ano, categoria, V e P já estão desenhados lá. Só
          duas colunas eram exclusivas: POS, que virou a curva de campeonato
          abaixo, e PTS, que é incomparável entre calendários e categorias (o
          mesmo motivo que tirou os pontos da faixa de Records).

          O que ficou no lugar responde o que agregado nenhum responde: como a
          equipe corre AGORA, onde ela termina o campeonato, e qual é a forma da
          distribuição por trás da taxa de pódio. */}
      {/* Confiabilidade abre a seção: é a pergunta anterior a todas as outras.
          Colocação, forma e curva só querem dizer alguma coisa depois de saber
          se o carro chega ao fim — 14º com o carro inteiro e abandono na volta 2
          são histórias opostas que todos os outros blocos contam igual. */}
      <ReliabilityPanel reliability={dossier.reliability} />

      <RecentForm races={dossier.recentForm} />
      <ChampionshipCurve seasons={dossier.seasonResults} />
      <ResultSpread spread={dossier.resultSpread} />

      {/* Os pilotos fecham a leitura esportiva: a seção sobe de "como o carro
          andou" para "quem estava dentro dele". */}
      <TeamLineup lineup={dossier.lineup} />

      {/* A cronologia fecha a seção, e não em Records: ela tem de zero a cinco
          itens conforme a equipe, então a altura de Records mudava a cada equipe
          e a tela pulava ao navegar com as setas. Aqui embaixo a variação não
          desloca nada — e a leitura da seção fica do agora para o sempre. */}
      <HistoryRail milestones={dossier.milestones} timeline={dossier.timeline} />

      {/* A linha do tempo não se repete no fim da seção: ela é o HistoryRail lá
          em cima, agora fundido com os marcos. */}
    </>
  );
}

function IdentitySection({ dossier }) {
  const { t } = useTranslation();
  return (
    <section className="grid gap-2.5">
      <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_38%,transparent)] bg-[#0c1626] p-4">
        <BlockLabel>{t("myTeamTab.history.identity.profileLabel")}</BlockLabel>
        <strong className="mt-1.5 block text-xl font-semibold leading-none tracking-[-0.02em] text-text-primary">
          {dossier.identity.profile}
        </strong>
        <p className="mt-2 text-[11px] leading-5 text-text-secondary">{dossier.identity.summary}</p>
      </div>
      <div className="grid gap-2.5 md:grid-cols-2">
        <InfoCard
          label={t("myTeamTab.history.identity.originLabel")}
          value={dossier.identity.origin}
          detail={t("myTeamTab.history.identity.originDetail")}
        />
        <InfoCard
          label={t("myTeamTab.history.identity.currentLabel")}
          value={dossier.identity.current}
          detail={t("myTeamTab.history.identity.currentDetail")}
        />
      </div>
      <div className="grid gap-2.5 md:grid-cols-2">
        <div className="rounded-xl border border-status-yellow/25 bg-[#201a0b]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.identity.rivalLabel")}</BlockLabel>
          <strong className="mt-1.5 block text-sm font-semibold text-status-yellow">{dossier.identity.rival.name}</strong>
          <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">
            {t("myTeamTab.history.identity.rivalToday", { category: dossier.identity.rival.currentCategory })} {dossier.identity.rival.note}
          </p>
        </div>
        <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_32%,transparent)] bg-[#0c1626]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.identity.symbolLabel")}</BlockLabel>
          <strong className="mt-1.5 block text-sm font-semibold text-text-primary">{dossier.identity.symbolDriver}</strong>
          <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{dossier.identity.symbolDriverDetail}</p>
        </div>
      </div>
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

function ManagementSection({ dossier }) {
  const { t } = useTranslation();
  // A saúde da operação é o único bloco que muda de cor por conteúdo — vermelho
  // para pressionada/crise, amarelo para estável, verde para saudável. A regra é
  // a mesma do v1, importada em vez de recopiada.
  const tone = operationHealthTone(dossier.management.operationHealth);
  return (
    <section className="grid gap-2.5">
      <div className={`rounded-xl border p-4 ${tone.card}`}>
        <BlockLabel>{t("myTeamTab.history.management.operationHealth")}</BlockLabel>
        <strong className={`mt-1.5 block text-xl font-semibold ${tone.text}`}>{dossier.management.operationHealth}</strong>
        <p className="mt-2 text-[11px] leading-5 text-text-secondary">{dossier.management.summary}</p>
      </div>
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
  return (
    <section>
      <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-4">
        <MiniMetric label={t("myTeamTab.history.categories.promotions")} value={dossier.movement.promotions} />
        <MiniMetric label={t("myTeamTab.history.categories.relegations")} value={dossier.movement.relegations} />
        <MiniMetric label={t("myTeamTab.history.categories.bestCategory")} value={dossier.movement.bestCategory} />
        <MiniMetric label={t("myTeamTab.history.categories.hardestCategory")} value={dossier.movement.hardestCategory} />
      </div>
      <div className="mt-2.5">
        <InfoCard label={t("myTeamTab.history.categories.timeByCategory")} value={dossier.movement.timeByCategory} />
      </div>
      <div className="mt-5">
        <BlockLabel>{t("myTeamTab.history.categories.ladder")}</BlockLabel>
        <div className="mt-2.5 grid gap-2 md:grid-cols-2">
          {dossier.categoryPath.map((step, index) => {
            const move = categoryMovementBadge(step.movement);
            return (
              <div
                key={`${step.category}-${index}`}
                className="rounded-lg border-l-4 bg-[#0c1626]/95 px-3.5 py-3"
                style={{ borderLeftColor: step.color }}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className={`font-mono text-xs font-bold ${move.tone}`} title={move.label}>{move.icon}</span>
                    <strong className="truncate text-xs text-text-primary">{step.category}</strong>
                  </div>
                  <span className="shrink-0 font-mono text-[11px] font-semibold" style={{ color: step.color }}>{step.years}</span>
                </div>
                <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{step.detail}</p>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function categoryMovementBadge(movement) {
  switch (movement) {
    case "promotion":
      return { icon: "▲", tone: "text-status-green", label: i18n.t("myTeamTab.history.categories.movement.promotion") };
    case "relegation":
      return { icon: "▼", tone: "text-status-red", label: i18n.t("myTeamTab.history.categories.movement.relegation") };
    case "start":
      return { icon: "●", tone: "text-[color:var(--team)]", label: i18n.t("myTeamTab.history.categories.movement.start") };
    default:
      return { icon: "—", tone: "text-text-muted", label: i18n.t("myTeamTab.history.categories.movement.same") };
  }
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

function BlockLabel({ children }) {
  return <span className="block text-[10px] font-semibold uppercase tracking-[0.15em] text-text-muted">{children}</span>;
}

function MiniMetric({ label, value }) {
  return (
    <div className="rounded-xl bg-[#0f1c2b] px-3.5 py-3">
      <span className="block truncate text-[10px] font-semibold uppercase tracking-[0.13em] text-text-muted">{label}</span>
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
    <button
      type="button"
      aria-label={label}
      title={team?.nome ?? label}
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
