import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import i18n from "../../../i18n/index.js";
import { formatCompactDate } from "../../../i18n/format.js";
import useCareerStore from "../../../stores/useCareerStore";
import { TeamHistoryDrawer } from "../../../components/team/history";
import { AtlasChart } from "../../../components/team/v2/AtlasChart";
import { AtlasRankings } from "../../../components/team/v2/AtlasRankings";
import { AtlasChampionsPanel } from "../../../components/team/v2/AtlasChampionsPanel";
import { AtlasFooter, AtlasHeader, AtlasLegend } from "../../../components/team/v2/AtlasChrome";
import { useElementSize } from "../../../components/team/v2/useElementSize";
import {
  YEAR_HEADER_HEIGHT,
  atlasDivisions,
  buildAtlasTracks,
  buildAtlasVerticalGeometry,
  buildAtlasYears,
  buildRankingCards,
  isLivePayload,
} from "../../../components/team/v2/atlasV2Geometry";
import {
  DEFAULT_START_YEAR,
  familyFromTeamContext,
  flattenTeams,
  normalizePayload,
  teamRowToTeam,
  teamToTeamRow,
} from "../../../components/team/worldTeamChartGeometry";

const ZOOM_RECENT_YEARS = 10;
const HISTORY_FETCH_WINDOW_SIZE = 32;
const HISTORY_FETCH_START_YEAR = DEFAULT_START_YEAR;
const TEAM_CLICK_DELAY_MS = 220;
// Largura da coluna lateral e vão entre as colunas. O vão é a distância que o
// pequeno rabicho colorido percorre do fim da linha até a linha do card.
const RANKING_COLUMN_WIDTH = "clamp(410px, 27vw, 445px)";
const CHART_CARD_GAP = 22;

// Atlas Histórico v2.
//
// Mesmo carregamento, mesmos dados e mesmas interações do v1 — o que muda é a
// composição: moldura única ocupando a viewport, gráfico e rankings como DUAS
// colunas reais do grid (no v1 as tabelas flutuavam sobre a plotagem), legenda sob o
// gráfico e rodapé fixo dentro da moldura. Ver src/pages/tabs/atlas/index.js.
function GlobalTeamsTabV2({
  selectedTeamId = null,
  selectedTeamCategory = null,
  selectedTeamClassName = null,
  initialZoomYears = null,
  pinnedTeamId = null,
  drawerPlacement = "center",
  onBack,
  onOpenTeamRecords,
}) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  // Data corrente da carreira — a mesma que o cabeçalho do app mostra. É o único
  // "atualizado em" real que existe aqui; o atlas não fala com nenhum servidor.
  const calendarDisplayDate = useCareerStore((state) => state.calendarDisplayDate);
  const [zoomYears, setZoomYears] = useState(initialZoomYears);
  const [family, setFamily] = useState(() => familyFromTeamContext(selectedTeamCategory, selectedTeamClassName));
  const [payload, setPayload] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [focusedTeamId, setFocusedTeamId] = useState(selectedTeamId);
  const [selectedTeam, setSelectedTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");
  const [championsBand, setChampionsBand] = useState(null);
  const teamClickTimeoutRef = useRef(null);

  useEffect(() => {
    setFocusedTeamId(selectedTeamId);
  }, [selectedTeamId]);

  useEffect(() => {
    setFamily(familyFromTeamContext(selectedTeamCategory, selectedTeamClassName));
  }, [selectedTeamCategory, selectedTeamClassName]);

  useEffect(() => {
    let mounted = true;

    async function load() {
      if (!careerId) {
        setPayload(null);
        setError(i18n.t("globalTeams.notLoaded"));
        setLoading(false);
        return;
      }

      try {
        setLoading(true);
        setError("");
        const data = await invoke("get_global_team_history", {
          careerId,
          family,
          startYear: HISTORY_FETCH_START_YEAR,
          windowSize: HISTORY_FETCH_WINDOW_SIZE,
        });
        if (!mounted) return;
        setPayload(normalizePayload(data));
      } catch (invokeError) {
        if (!mounted) return;
        setError(typeof invokeError === "string" ? invokeError : i18n.t("globalTeams.loadError"));
      } finally {
        if (mounted) setLoading(false);
      }
    }

    load();
    return () => {
      mounted = false;
    };
  }, [careerId, family]);

  useEffect(() => () => {
    if (teamClickTimeoutRef.current) clearTimeout(teamClickTimeoutRef.current);
  }, []);

  // Esc fecha o Atlas. O botão do rodapé é a saída anunciada; esta é a que o dedo
  // já procura sozinho quando uma tela toma a viewport inteira.
  //
  // Só age na camada de cima: com o salão dos campeões ou o dossiê abertos, o Esc
  // é DELES, e cada um fecha o seu. Sem essa guarda, um Esc fecharia os dois de uma
  // vez e jogaria o jogador para fora do Atlas sem ele ter pedido.
  useEffect(() => {
    if (!onBack || championsBand || selectedTeam) return undefined;
    function handleKeyDown(event) {
      if (event.key === "Escape") onBack();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onBack, championsBand, selectedTeam]);

  // O eixo inteiro é desenhado de uma vez: a área de plotagem tem tamanho medido, e
  // as colunas se dividem entre a largura disponível. Sem janela deslizante, o
  // scrubber do v1 deixa de existir — o zoom "recente" continua sendo o recorte.
  const years = useMemo(() => buildAtlasYears(payload, zoomYears), [payload, zoomYears]);
  const tracks = useMemo(() => buildAtlasTracks(payload, years), [payload, years]);
  // Os cards não dependem de `years`: eles falam da temporada atual, esteja ela
  // visível no gráfico ou não (o zoom "recente" não pode mexer na variação).
  const cards = useMemo(() => buildRankingCards(payload), [payload]);
  // Há uma temporada começada e não terminada? É o que separa "o Atlas mostra o
  // agora" de "o Atlas mostra o último campeonato decidido".
  const isLive = isLivePayload(payload);

  // Régua vertical ÚNICA, calculada aqui e entregue às duas colunas.
  //
  // A altura medida é a da PRÓPRIA linha 2 do grid — a faixa que a área de plotagem
  // e a coluna de rankings dividem. Medir a coluna de rankings dá exatamente isso,
  // porque ela é um item dessa linha e estica com ela. Nada de subtrair a barra dos
  // anos nem as bordas do card: quem garante a origem comum é o CSS grid.
  const [laneRef, laneSize] = useElementSize({ width: 430, height: 700 });
  const vertical = useMemo(
    () => buildAtlasVerticalGeometry({ totalHeight: laneSize.height, divisions: atlasDivisions(payload) }),
    [laneSize.height, payload],
  );
  const allTeams = useMemo(() => flattenTeams(payload), [payload]);
  const activeFamily = payload?.families?.find((item) => item.id === payload?.selected_family);

  // O título anuncia exatamente o intervalo desenhado — as duas pontas vêm do mesmo
  // array de anos que vira coluna no gráfico.
  const dataStart = years.length ? years[0] : null;
  const dataEnd = years.length ? years[years.length - 1] : null;
  // A temporada em andamento não é coluna, mas é desenhada: a linha chega na borda
  // direita e o card fala dela. É esse o ano que fecha o intervalo anunciado e o que
  // o rabicho liga ao card — usar `dataEnd` aqui apagaria os dois.
  const edgeYear = isLive ? payload.current_year : dataEnd;

  function clearTeamClickTimeout() {
    if (teamClickTimeoutRef.current) {
      clearTimeout(teamClickTimeoutRef.current);
      teamClickTimeoutRef.current = null;
    }
  }

  // Clique abre o dossiê, duplo clique fixa a equipe em destaque. O atraso curto
  // existe para o duplo clique poder cancelar a abertura do dossiê.
  function handleTeamClick(team) {
    clearTeamClickTimeout();
    teamClickTimeoutRef.current = setTimeout(() => {
      setSelectedTeam(team);
      setActiveHistoryTab("records");
      teamClickTimeoutRef.current = null;
    }, TEAM_CLICK_DELAY_MS);
  }

  function handleTeamDoubleClick(team) {
    clearTeamClickTimeout();
    setFocusedTeamId(team.team_id);
  }

  // Portal para o <body>: a troca de abas do Dashboard envolve a tela num
  // `.tab-pane-fade`, que anima `transform` por 420ms — e um ancestral com
  // transform vira o bloco de contenção de `position: fixed`. Sem o portal, a
  // moldura abria dentro do `main` (com padding e max-width) e só pulava para a
  // viewport quando a animação terminava.
  return createPortal(
    // z-[85] fica ENTRE o backdrop do TeamHistoryOverlay (z-80), que também usa este
    // atlas, e o TeamHistoryDrawer (z-90/91), que precisa abrir por cima.
    <div data-testid="atlas-v2-layer" className="fixed inset-0 z-[85] bg-[#050b14]">
      <div
        data-testid="atlas-v2-shell"
        className="m-[10px] grid h-[calc(100dvh-20px)] w-[calc(100vw-20px)] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden rounded-[18px] border border-[#2a3f5c]/70 bg-[radial-gradient(120%_90%_at_50%_0%,#0d1c30_0%,#060d18_65%)] shadow-[inset_0_0_60px_rgba(30,80,140,0.10)]"
      >
        <AtlasHeader
          title={
            loading && !payload
              ? t("globalTeams.buildingWorldTeamHistory")
              : i18n.t("globalTeams.familyWindow", {
                  family: activeFamily?.label ?? "",
                  start: dataStart ?? "-",
                  end: edgeYear ?? "-",
                })
          }
          families={payload?.families ?? []}
          selectedFamily={payload?.selected_family}
          zoomYears={zoomYears}
          zoomLabel={
            zoomYears != null
              ? i18n.t("globalTeams.zoomRecentLabel", { years: ZOOM_RECENT_YEARS })
              : i18n.t("globalTeams.zoomAll")
          }
          zoomTitle={
            zoomYears != null
              ? i18n.t("globalTeams.zoomFullTitle")
              : i18n.t("globalTeams.zoomRecentTitle", { years: ZOOM_RECENT_YEARS })
          }
          onSelectFamily={setFamily}
          onToggleZoom={() => setZoomYears((current) => (current == null ? ZOOM_RECENT_YEARS : null))}
        />

        {/* A legenda saiu para uma linha própria: assim as duas colunas dividem
            exatamente a mesma faixa vertical, que é o que a régua compartilhada
            pressupõe. */}
        <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] px-5 pb-1">
          {/* Duas colunas, duas linhas. A faixa dos anos ocupa a linha 1 da coluna
              1; a área de plotagem e a coluna de rankings dividem a linha 2. É o
              grid que lhes dá a MESMA origem vertical — nenhum dos dois soma a
              altura do cabeçalho, e a célula superior direita fica vazia. */}
          <div
            className="grid min-h-0"
            style={{
              gridTemplateColumns: `minmax(0, 1fr) ${RANKING_COLUMN_WIDTH}`,
              gridTemplateRows: `${YEAR_HEADER_HEIGHT}px minmax(0, 1fr)`,
              columnGap: CHART_CARD_GAP,
            }}
          >
            {payload && years.length ? (
              <AtlasChart
                payload={payload}
                years={years}
                tracks={tracks}
                vertical={vertical}
                focusedTeamId={focusedTeamId}
                pinnedTeamId={pinnedTeamId}
                onFocus={setFocusedTeamId}
                onTeamClick={handleTeamClick}
                onTeamDoubleClick={handleTeamDoubleClick}
              />
            ) : (
              <div
                className="animate-pulse rounded-2xl border border-[#2a3f5c]/60 bg-white/[0.02]"
                style={{ gridColumn: 1, gridRow: "1 / span 2" }}
              />
            )}

            {error ? (
              <div
                className="self-start rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red"
                style={{ gridColumn: 1, gridRow: 2 }}
              >
                {error}
              </div>
            ) : null}

            <AtlasRankings
              laneRef={laneRef}
              cards={cards}
              vertical={vertical}
              lastAxisYear={edgeYear}
              focusedTeamId={focusedTeamId}
              pinnedTeamId={pinnedTeamId}
              onFocus={setFocusedTeamId}
              onTeamClick={handleTeamClick}
              onTeamDoubleClick={handleTeamDoubleClick}
              // A categoria em que o jogador corre AGORA: é o card dela que
              // acende até ele abrir a coluna pela primeira vez.
              playerCategory={playerTeam?.categoria ?? null}
              onOpenChampions={setChampionsBand}
            />
          </div>

          <AtlasLegend showLive={isLive} />
        </div>

        <AtlasFooter
          teamCount={tracks.length}
          bandCount={cards.length}
          familyLabel={activeFamily?.label ?? ""}
          lastDataYear={dataEnd ?? "-"}
          updatedAt={calendarDisplayDate ? formatCompactDate(calendarDisplayDate) : ""}
          currentSeason={payload?.current_year ?? "-"}
          onBack={onBack}
        />
      </div>

      {championsBand ? (
        <AtlasChampionsPanel careerId={careerId} band={championsBand} onClose={() => setChampionsBand(null)} />
      ) : null}

      {selectedTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={teamRowToTeam(selectedTeam)}
          teams={allTeams.map(teamRowToTeam)}
          playerTeam={playerTeam}
          activeCategory={selectedTeam.category ?? selectedTeam.points?.[0]?.category ?? selectedTeam.band_category ?? ""}
          activeTab={activeHistoryTab}
          placement={drawerPlacement}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={(team) => setSelectedTeam(teamToTeamRow(team, selectedTeam))}
          onOpenRecordsRanking={onOpenTeamRecords}
          onClose={() => setSelectedTeam(null)}
        />
      ) : null}
    </div>,
    document.body,
  );
}

export default GlobalTeamsTabV2;
