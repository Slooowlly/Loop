import { useEffect, useMemo, useState } from "react";
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
  Medal,
  TrendingUp,
  Trophy,
  X,
} from "lucide-react";

import goldTrophy from "../../../assets/utilities/trophies/ouro.png";
import TeamLogoMark from "../TeamLogoMark";
import {
  buildTeamHistoryDossier,
  operationHealthTone,
  orderTeamsForHistoryNavigation,
} from "../TeamHistoryDrawer";
import i18n from "../../../i18n/index.js";
import { getCategoryColor } from "../../../utils/categoryColors";

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
  onClose,
}) {
  const { t } = useTranslation();
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
    <div className="fixed inset-0 z-[90] flex items-center justify-center" data-testid="team-history-layer" aria-hidden={false}>
      <button
        type="button"
        aria-label={t("myTeamTab.history.closeAria")}
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/70 backdrop-blur-[3px]"
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-labelledby="team-history-title"
        data-testid="team-history-drawer"
        className="animate-scale-in relative z-10 flex max-h-[92vh] w-[min(94vw,1180px)] flex-col overflow-hidden rounded-[28px] border border-white/15 bg-[#07101d] shadow-[0_30px_90px_rgba(0,0,0,0.72)]"
        style={{
          "--team": dossier.color,
          backgroundImage:
            "radial-gradient(circle at 8% 0%, color-mix(in srgb, var(--team) 14%, transparent), transparent 26rem), linear-gradient(180deg, rgba(12,22,38,0.98), rgba(5,11,20,0.995))",
        }}
      >
        <div className="h-1 shrink-0 bg-[color:var(--team)]" />

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
            {activeTab === "records" ? <RecordsSection dossier={dossier} /> : null}
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
          <div className="ml-auto flex shrink-0 items-center gap-2">
            <TeamStepButton
              label={t("myTeamTab.history.nav.previous")}
              direction="up"
              team={previousTeam}
              onSelectTeam={onSelectTeam}
            />
            <TeamStepButton
              label={t("myTeamTab.history.nav.next")}
              direction="down"
              team={nextTeam}
              onSelectTeam={onSelectTeam}
            />
          </div>
        </footer>
      </aside>
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

function RecordsSection({ dossier }) {
  return (
    <section>
      {dossier.historyStatus !== "ready" ? <HistoryStateMessage dossier={dossier} /> : null}

      {/* Grid assimétrico: as três CONTAGENS na primeira linha, as TAXAS numa
          segunda linha de cards mais largos. Não é capricho — separa o que é
          acumulado numa carreira do que é proporção de aproveitamento, e dá às
          taxas o espaço que a barra de posição precisa para ser lida. */}
      <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
        {dossier.records.slice(0, 3).map((record) => (
          <RecordCard key={record.id || record.label} record={record} />
        ))}
      </div>
      {dossier.records.length > 3 && (
        <div className="mt-2.5 grid gap-2.5 sm:grid-cols-2">
          {dossier.records.slice(3).map((record) => (
            <RecordCard key={record.id || record.label} record={record} />
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

    return { regua, grupos, unico: lista.length === 1 ? lista[0] : null };
  }, [titles, seasons]);

  if (!dados) return null;

  // Um título só não sustenta régua, cabeçalho e tabela — seria mais moldura que
  // conteúdo. Vira uma linha que diz tudo.
  if (dados.unico) {
    const titulo = dados.unico;
    const cor = titulo.categoryId ? getCategoryColor(titulo.categoryId) : titulo.color;
    return (
      <div className="mt-5">
        <BlockLabel>{t("myTeamTab.history.records.titleGallery")}</BlockLabel>
        <div
          data-testid="team-history-title-gallery"
          data-single="true"
          className="mt-2.5 rounded-lg border-l-4 bg-[#0c1626]/95 px-3.5 py-2.5"
          style={{ borderLeftColor: cor }}
        >
          <div className="flex items-center justify-between gap-3">
            <strong className="truncate text-xs text-text-primary">{titulo.category}</strong>
            <span className="shrink-0 font-mono text-xs font-bold text-status-yellow">{titulo.year}</span>
          </div>
          <div className="mt-1 text-[11px] leading-tight text-text-secondary">
            <span className="font-mono text-text-muted">
              {t("myTeamTab.history.records.titleCampaign", { points: titulo.points, wins: titulo.wins })}
            </span>
            {titulo.championDriver ? <ChampionLine title={titulo} inline /> : null}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="mt-5">
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
function ChampionLine({ title, inline = false }) {
  if (!title.championDriver) return <span />;
  const dobradinha = title.championIsTeam;
  return (
    <span className={`flex min-w-0 items-center gap-1.5 ${inline ? "mt-1" : ""}`}>
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
function RecordCard({ record }) {
  const { t } = useTranslation();
  const hasScale = record.rankTotal > 0 && record.rankPosition > 0;
  const fill = hasScale ? ((record.rankTotal - record.rankPosition + 1) / record.rankTotal) * 100 : 0;

  return (
    <div className="relative rounded-xl bg-[#0f1c2b] px-3.5 py-3" data-record={record.id || undefined}>
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
function HistoryRail({ milestones, timeline }) {
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
    <div className="mt-5">
      <BlockLabel>{t("myTeamTab.history.timeline.title")}</BlockLabel>
      <ol className="mt-3 grid gap-3" data-testid="team-history-milestones">
        {eventos.map((evento, index) => (
          <li key={`${evento.kind}-${evento.year}-${index}`} className="relative grid grid-cols-[14px_minmax(0,1fr)] gap-3">
            <span className="relative flex justify-center">
              <span className="mt-[3px] h-2.5 w-2.5 shrink-0 rounded-full bg-[color:var(--team)]" />
              {/* O fio só desce ENTRE os pontos: no último ele sumiria numa
                  ponta solta apontando para nada. */}
              {index < eventos.length - 1 ? (
                <span className="absolute left-1/2 top-[15px] h-[calc(100%+12px-15px)] w-px -translate-x-1/2 bg-white/15" />
              ) : null}
            </span>
            <div className="min-w-0 pb-0.5">
              <strong className="block font-mono text-sm text-[color:var(--team)]">{evento.year}</strong>
              <span className="block text-[11px] leading-tight text-text-secondary">{evento.text}</span>
            </div>
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
function RecentForm({ races }) {
  const { t } = useTranslation();
  if (!races?.length) return null;
  const primeira = races[0];
  const ultima = races[races.length - 1];
  // Troca de categoria no meio da fita é a explicação de uma queda que, sem ela,
  // se leria como perda de forma.
  const trocou = primeira.categoryId && ultima.categoryId && primeira.categoryId !== ultima.categoryId;
  return (
    <div className="mt-5">
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

// Geometria da curva. O eixo é INVERTIDO — P1 no topo — porque no automobilismo
// "subir" é diminuir o número, e um gráfico em que a campanha campeã desce
// contraria a leitura antes de qualquer rótulo.
const CURVE_WIDTH = 640;
const CURVE_LEFT = 34;
const CURVE_RIGHT = 626;
const CURVE_TOP = 16;
const CURVE_BOTTOM = 120;

// Curva de campeonato: a posição FINAL por temporada.
//
// Não repete a faixa de top 5 de Records: aquela mede corrida a corrida, esta
// mede o campeonato. Uma equipe regular pode ter poucos top 5 e ainda terminar
// em P3 — quando os dois gráficos discordam, a discordância É a informação.
function ChampionshipCurve({ seasons }) {
  const { t } = useTranslation();
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
  return (
    <div className="mt-5">
      <BlockLabel>{t("myTeamTab.history.sport.championshipCurve")}</BlockLabel>
      <div className="mt-2.5 rounded-xl bg-[#0f1c2b] px-3 py-3">
        <svg viewBox={`0 0 ${CURVE_WIDTH} 150`} className="h-auto w-full" data-testid="team-history-curve">
          {/* Faixa do pódio do campeonato: P1 a P3. É a régua que diz se a
              temporada foi boa sem o leitor ter que ler o eixo. */}
          <rect x={CURVE_LEFT} y={CURVE_TOP} width={CURVE_RIGHT - CURVE_LEFT} height={meio - CURVE_TOP} fill="#16281f" />
          <text x={CURVE_RIGHT - 4} y={CURVE_TOP + 11} textAnchor="end" fontSize="9" fill="#4e7f63">
            {t("myTeamTab.history.sport.championshipPodium")}
          </text>
          {[1, Math.ceil((pior + 1) / 2), pior].map((tick) => (
            <g key={tick}>
              <line
                x1={CURVE_LEFT}
                y1={y(tick)}
                x2={CURVE_RIGHT}
                y2={y(tick)}
                stroke="#22303f"
                strokeDasharray={tick === 1 ? undefined : "3 3"}
              />
              <text x={CURVE_LEFT - 8} y={y(tick) + 3} textAnchor="end" fontSize="9" fill="#6f8299">
                {`P${tick}`}
              </text>
            </g>
          ))}
          {trechos.map((trecho) => (
            <polyline
              key={`${trecho[0].year}-${trecho.length}`}
              points={trecho.map((p) => `${p.x},${p.y}`).join(" ")}
              fill="none"
              stroke="var(--team)"
              strokeWidth="2"
            />
          ))}
          {pontos.map((ponto) => {
            if (ponto.y === null) return null;
            const campeao = ponto.position === 1;
            return (
              <circle
                key={ponto.year}
                data-season={ponto.year}
                cx={ponto.x}
                cy={ponto.y}
                r={campeao ? 4.5 : 3.5}
                fill={campeao ? MEDAL_COLORS.first : "var(--team)"}
              >
                <title>
                  {t("myTeamTab.history.sport.curveTooltip", {
                    year: ponto.year,
                    category: ponto.category,
                    position: ponto.position,
                  })}
                </title>
              </circle>
            );
          })}
          {/* Mesma tira de categoria da faixa de Records, aqui embaixo da curva:
              a queda de uma temporada quase sempre tem a promoção como causa, e
              as duas coisas precisam ser lidas juntas. */}
          {pontos.map((ponto) => (
            <rect
              key={`cat-${ponto.year}`}
              data-category={ponto.categoryId || undefined}
              x={ponto.x - Math.max(passo / 2 - 1, 4)}
              y={132}
              width={Math.max(passo - 2, 8)}
              height={3}
              rx={1.5}
              fill={ponto.categoryId ? getCategoryColor(ponto.categoryId) : "transparent"}
            />
          ))}
          {pontos.map((ponto) =>
            ponto.index % rotulos === 0 ? (
              <text key={`ano-${ponto.year}`} x={ponto.x} y={147} textAnchor="middle" fontSize="9" fill="#6f8299">
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
function ResultSpread({ spread }) {
  const { t } = useTranslation();
  if (!spread || spread.races <= 0) return null;
  const faixas = [
    { id: "first", value: spread.first, color: PLACEMENT_COLORS.first },
    { id: "podium", value: spread.podium, color: PLACEMENT_COLORS.second },
    { id: "nearMiss", value: spread.nearMiss, color: PLACEMENT_COLORS.nearMiss },
    { id: "topTen", value: spread.topTen, color: PLACEMENT_COLORS.topTen },
    { id: "outside", value: spread.outside, color: PLACEMENT_COLORS.outside },
  ].filter((faixa) => faixa.value > 0);
  if (!faixas.length) return null;
  return (
    <div className="mt-5">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.resultSpread")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.spreadRaces", { value: spread.races })}
        </span>
      </div>
      <div className="mt-2.5 flex h-6 overflow-hidden rounded-md" data-testid="team-history-spread">
        {faixas.map((faixa) => (
          <span
            key={faixa.id}
            data-band={faixa.id}
            title={`${t(`myTeamTab.history.sport.spread.${faixa.id}`)} · ${faixa.value}`}
            style={{ flexGrow: faixa.value, flexBasis: 0, backgroundColor: faixa.color }}
          />
        ))}
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        {faixas.map((faixa) => (
          <MedalKey
            key={faixa.id}
            color={faixa.color}
            label={`${t(`myTeamTab.history.sport.spread.${faixa.id}`)} · ${faixa.value}`}
          />
        ))}
      </div>
    </div>
  );
}

function SportSection({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      {dossier.historyStatus !== "ready" ? <HistoryStateMessage dossier={dossier} /> : null}
      {/* Nem temporadas disputadas nem taxa de pódio/vitória se repetem aqui.
          Temporadas é âncora do cabeçalho, visível em qualquer seção; as taxas
          são dois dos cinco cards de Records, e lá vêm com a média do grupo e a
          posição no ranking. Repetir o número cru aqui era a versão pior do
          mesmo dado. */}
      <div className="grid gap-2.5 sm:grid-cols-2">
        <InfoCard label={t("myTeamTab.history.sport.currentStreak")} value={dossier.sport.currentStreak} />
        <InfoCard label={t("myTeamTab.history.sport.bestStreak")} value={dossier.sport.bestStreak} />
      </div>

      {/* A tabela temporada a temporada saiu daqui. Depois que Records ganhou a
          faixa de top 5 por corrida com a tira de categoria, ela era o mesmo
          conteúdo em números — ano, categoria, V e P já estão desenhados lá. Só
          duas colunas eram exclusivas: POS, que virou a curva de campeonato
          abaixo, e PTS, que é incomparável entre calendários e categorias (o
          mesmo motivo que tirou os pontos da faixa de Records).

          O que ficou no lugar responde o que agregado nenhum responde: como a
          equipe corre AGORA, onde ela termina o campeonato, e qual é a forma da
          distribuição por trás da taxa de pódio. */}
      <RecentForm races={dossier.recentForm} />
      <ChampionshipCurve seasons={dossier.seasonResults} />
      <ResultSpread spread={dossier.resultSpread} />

      {/* A cronologia fecha a seção, e não em Records: ela tem de zero a cinco
          itens conforme a equipe, então a altura de Records mudava a cada equipe
          e a tela pulava ao navegar com as setas. Aqui embaixo a variação não
          desloca nada — e a leitura da seção fica do agora para o sempre. */}
      <HistoryRail milestones={dossier.milestones} timeline={dossier.timeline} />

      {/* A linha do tempo não se repete no fim da seção: ela é o HistoryRail lá
          em cima, agora fundido com os marcos. */}
    </section>
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

// Navegação entre equipes: no v1 as setas flutuavam na borda da tela, longe do
// painel. Aqui elas moram no rodapé, ao lado do escopo do comparativo.
function TeamStepButton({ label, direction, team, onSelectTeam }) {
  const Chevron = direction === "up" ? ChevronUp : ChevronDown;
  return (
    <button
      type="button"
      aria-label={label}
      title={team?.nome ?? label}
      disabled={!team}
      onClick={() => team && onSelectTeam(team)}
      className={`grid h-7 w-7 place-items-center rounded-lg border transition-glass ${
        team
          ? "border-white/15 bg-[#0d1727] text-text-secondary hover:bg-[#14233a] hover:text-text-primary"
          : "cursor-not-allowed border-white/[0.06] bg-[#0b111a] text-[#4a525d]"
      }`}
    >
      <Chevron size={16} strokeWidth={1.8} aria-hidden="true" />
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
