import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import DriverDetailModal from "../../components/driver";
import GlassCard from "../../components/ui/GlassCard";
import { TeamHistoryDrawer } from "../../components/team/history";
import useCareerStore from "../../stores/useCareerStore";
import { FilterBar } from "../../components/driver/GlobalDriverFilters";
import {
  CategorySectionRow,
  DriverRankingRow,
  SortableHeader,
} from "../../components/driver/DriverRankingRow";
import {
  ChampionshipChampionsDialog,
  TitleBreakdownDialog,
} from "../../components/driver/GlobalDriverDialogs";
import {
  ChampionshipChampionPanel,
  FocusedDriverCard,
} from "../../components/driver/GlobalDriverSummary";
import { statusFilterLabel, defaultDirection } from "../../components/driver/globalDriverFormatters";
import {
  DEFAULT_FILTERS,
  DEFAULT_SORT,
  FILTRO_TODOS,
  buildChampionshipChampionSections,
  buildFilterOptions,
  buildFocusedDriverRanks,
  buildRelativeRanks,
  buildTableSections,
  filterRows,
  filterableCategory,
  sortForMetric,
  sortRows,
} from "../../components/driver/globalDriverRanking";

// Os três pulsos de `.animate-driver-row-glow` (800ms cada). A classe sai da
// linha quando eles acabam para não sobrar animação parada no DOM e para o
// próximo pedido poder reacender o mesmo nome.
const GLOW_MS = 2400;

// Folga para a tabela FILTRADA existir antes de medir onde a linha está. Não é
// espera de animação: é um tique de macrotarefa, o bastante para o React ter
// pintado o `setFilters` que o efeito da categoria acabou de disparar.
const ESPERA_DO_FILTRO_MS = 60;

// Folga para o `scrollIntoView` suave terminar antes do halo acender. É um
// palpite calibrado, e não uma medida: a rolagem suave não avisa quando chega
// (`scrollend` não é confiável em toda WebView), e errar para mais só atrasa o
// brilho — errar para menos o desperdiça no meio do movimento.
const ESPERA_DA_ROLAGEM_MS = 420;

// `initialMetric` é a métrica de carreira apontada pela ficha do piloto
// (corridas, vitórias, pódios, títulos): a tela abre já ordenada por ela, com o
// piloto em foco, para responder o "205º de 610" que foi clicado lá.
// `initialCategory` chega junto quando a ficha estava no recorte "grid atual":
// a lista abre filtrada pela categoria dele, para o destino responder a mesma
// pergunta que a origem respondia.
function GlobalDriversTab({ selectedDriverId, initialMetric = null, initialCategory = null, onBack }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const [payload, setPayload] = useState({ rows: [], leaders: {} });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [sort, setSort] = useState(() => sortForMetric(initialMetric) ?? DEFAULT_SORT);
  const [filters, setFilters] = useState(DEFAULT_FILTERS);
  const [focusedDriverId, setFocusedDriverId] = useState(selectedDriverId ?? null);
  const [titleModalDriver, setTitleModalDriver] = useState(null);
  const [championshipModal, setChampionshipModal] = useState(null);
  const [selectedDetailDriverId, setSelectedDetailDriverId] = useState(null);
  // A ficha da equipe aberta a partir do dossiê de um piloto. Aqui não há tabela
  // de equipes por trás, então ela vem com o mínimo (id, nome, cor) e o resto
  // sai do `get_team_history_dossier`.
  const [equipeAberta, setEquipeAberta] = useState(null);
  const [abaDaEquipe, setAbaDaEquipe] = useState("records");
  const tabelaRef = useRef(null);
  // Armado só quando o foco vem de FORA da tabela (um card de recorde da ficha).
  // Clicar numa linha daqui também muda o foco, e rolar nesse caso arrastaria a
  // tabela debaixo do cursor de quem acabou de clicar.
  const pedidoDeRolagemRef = useRef(false);
  // Qual `initialCategory` já foi aplicada. Sem esta marca o filtro voltaria
  // sozinho toda vez que as opções fossem recalculadas — favoritar um piloto
  // refaz a lista de linhas — e desfaria a categoria que o jogador tivesse
  // trocado à mão depois de chegar aqui.
  const categoriaAplicadaRef = useRef(null);
  // O piloto que a rolagem acabou de entregar, aceso por dois pulsos. É estado, e
  // não uma classe fixa na linha em foco, porque o brilho é o FIM de um gesto:
  // ele nasce com a chegada e morre sozinho.
  const [glowDriverId, setGlowDriverId] = useState(null);
  const glowTimerRef = useRef(null);

  useEffect(() => {
    setFocusedDriverId(selectedDriverId ?? null);
    if (selectedDriverId) pedidoDeRolagemRef.current = true;
  }, [selectedDriverId]);

  // Uma métrica nova chegando é sempre um pedido novo, mesmo que o piloto seja o
  // mesmo de antes — voltar à ficha e clicar em "Títulos" depois de "Vitórias"
  // não muda `selectedDriverId`, e sem isto a tela ficaria na ordem anterior.
  useEffect(() => {
    const sortDaMetrica = sortForMetric(initialMetric);
    if (!sortDaMetrica) return;
    setSort(sortDaMetrica);
    pedidoDeRolagemRef.current = true;
  }, [initialMetric]);

  useEffect(() => {
    let mounted = true;

    async function load() {
      if (!careerId) {
        setPayload({ rows: [], leaders: {} });
        setError(i18n.t("globalDrivers.notLoaded"));
        setLoading(false);
        return;
      }
      try {
        setLoading(true);
        setError("");
        const data = await invoke("get_global_driver_rankings", {
          careerId,
          selectedDriverId,
        });
        if (mounted) {
          setPayload({
            rows: Array.isArray(data?.rows) ? data.rows : [],
            leaders: data?.leaders ?? {},
            selected_driver_id: data?.selected_driver_id,
            player_driver: data?.player_driver ?? null,
          });
        }
      } catch (invokeError) {
        if (mounted) {
          setError(typeof invokeError === "string" ? invokeError : i18n.t("globalDrivers.loadError"));
        }
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    load();
    return () => {
      mounted = false;
    };
  }, [careerId, selectedDriverId]);

  useEffect(() => {
    if (!titleModalDriver && !championshipModal) return undefined;

    function handleKeyDown(event) {
      if (event.key === "Escape") {
        setTitleModalDriver(null);
        setChampionshipModal(null);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [titleModalDriver, championshipModal]);

  const rows = payload.rows ?? [];
  const focusedDriver =
    rows.find((row) => row.id === focusedDriverId)
    ?? rows.find((row) => row.id === selectedDriverId)
    ?? rows[0]
    ?? null;
  const userDriver =
    payload.player_driver
    ?? rows.find((row) => row.is_jogador)
    ?? null;
  const filterOptions = useMemo(() => buildFilterOptions(rows), [rows]);
  const championshipChampionSections = useMemo(() => buildChampionshipChampionSections(rows), [rows]);
  const focusedDriverRanks = useMemo(
    () => buildFocusedDriverRanks(rows, focusedDriver),
    [rows, focusedDriver],
  );
  const userDriverRanks = useMemo(
    () => buildFocusedDriverRanks(rows, userDriver),
    [rows, userDriver],
  );
  const filteredRows = useMemo(() => filterRows(rows, filters), [rows, filters]);
  const relativeRankById = useMemo(
    () => buildRelativeRanks(filteredRows, filters.status !== FILTRO_TODOS),
    [filteredRows, filters.status],
  );
  const sortedRows = useMemo(() => sortRows(filteredRows, sort), [filteredRows, sort]);
  const tableSections = useMemo(
    () => buildTableSections(sortedRows, filters.category),
    [sortedRows, filters.category],
  );

  // A categoria pedida pela ficha entra depois que as linhas chegam: as opções
  // do filtro saem delas, e antes disso não há como saber se a chave é uma
  // categoria que a tela sabe representar. Aplica UMA vez por pedido.
  useEffect(() => {
    if (!initialCategory || categoriaAplicadaRef.current === initialCategory) return;
    const categoria = filterableCategory(filterOptions, initialCategory);
    if (!categoria) return;
    categoriaAplicadaRef.current = initialCategory;
    setFilters((atual) => ({ ...atual, category: categoria }));
    pedidoDeRolagemRef.current = true;
  }, [initialCategory, filterOptions]);

  // Rola até a linha do piloto em foco depois que a ordem nova já pintou. Sem
  // isto a tela abre no topo do ranking e o piloto de quem se clicou o card fica
  // 200 linhas abaixo da dobra — a resposta certa, fora de vista.
  useEffect(() => {
    if (!pedidoDeRolagemRef.current || loading) return;
    pedidoDeRolagemRef.current = false;
    if (!focusedDriverId) return;
    const alvo = focusedDriverId;
    // Apagar antes de acender: sem o nulo no meio, chegar duas vezes no mesmo
    // piloto não reinicia a animação — o React não repinta uma classe que já
    // estava lá, e o segundo clique não brilharia.
    //
    // E o brilho espera a rolagem ASSENTAR. Aceso junto com o `scrollIntoView`,
    // ele queimava enquanto a tabela ainda estava em movimento e a linha fora do
    // centro: o olho estava seguindo o deslocamento, não lendo nomes, e quando
    // parava o halo já tinha ido embora. Acender no fim do trajeto é o que faz
    // dele um apontar de dedo em vez de um efeito colateral da rolagem.
    setGlowDriverId(null);
    if (glowTimerRef.current) clearTimeout(glowTimerRef.current);

    // A rolagem sai num timer, e não aqui dentro, porque quando o pedido traz
    // CATEGORIA o filtro entra numa renderização seguinte: o efeito que aplica a
    // categoria roda logo antes deste, na mesma leva, mas o `setFilters` dele só
    // vira DOM depois. Medindo aqui, o `scrollIntoView` mirava a posição da
    // linha na tabela de 610 e a tabela encolhia para 31 logo em seguida — a
    // página ficava rolada muito abaixo do piloto, quase sempre no fim.
    glowTimerRef.current = setTimeout(() => {
      const linha = tabelaRef.current?.querySelector(`[data-driver-id="${alvo}"]`);
      linha?.scrollIntoView?.({ block: "center", behavior: "smooth" });
      glowTimerRef.current = setTimeout(() => {
        setGlowDriverId(alvo);
        glowTimerRef.current = setTimeout(() => setGlowDriverId(null), GLOW_MS);
      }, ESPERA_DA_ROLAGEM_MS);
    }, ESPERA_DO_FILTRO_MS);
  }, [focusedDriverId, loading, tableSections]);

  useEffect(() => () => clearTimeout(glowTimerRef.current), []);

  function handleSort(key) {
    setSort((current) => {
      if (current.key === key) {
        return { key, direction: current.direction === "asc" ? "desc" : "asc" };
      }
      return { key, direction: defaultDirection(key) };
    });
  }

  function updateFilter(key, value) {
    setFilters((current) => ({ ...current, [key]: value }));
  }

  // Aplica o novo estado de favorito numa linha (e no player_driver, se for ele) —
  // mantém estrela + filtro "Favoritos" em sincronia sem refazer o ranking inteiro.
  function patchRowFavorite(driverId, favorite) {
    setPayload((current) => ({
      ...current,
      rows: (current.rows ?? []).map((row) =>
        row.id === driverId ? { ...row, is_favorito: favorite } : row,
      ),
      player_driver:
        current.player_driver && current.player_driver.id === driverId
          ? { ...current.player_driver, is_favorito: favorite }
          : current.player_driver,
    }));
  }

  async function handleToggleFavorite(driverId) {
    if (!careerId || !driverId) return;
    try {
      const nowFavorite = await invoke("toggle_driver_favorite", { careerId, driverId });
      patchRowFavorite(driverId, nowFavorite);
    } catch {
      // Silencioso — favoritar nunca pode quebrar a lista.
    }
  }

  if (loading) {
    return <GlobalDriversLoading onBack={onBack} />;
  }

  return (
    <div className="space-y-5">
      <header className="flex flex-wrap items-start justify-between gap-4 px-1">
        <div>
          <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">{t("globalDrivers.worldRanking")}</p>
          <h2 className="mt-2 text-3xl font-semibold text-text-primary">{t("globalDrivers.globalPanorama")}</h2>
        </div>
        <button
          type="button"
          onClick={onBack}
          className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary transition-glass hover:border-accent-primary/40 hover:bg-accent-primary/10 hover:text-text-primary"
        >
          {t("globalDrivers.backToStandings")}
        </button>
      </header>

      {focusedDriver ? (
        <section className="grid items-stretch gap-4 lg:grid-cols-[minmax(0,1.22fr)_minmax(330px,0.78fr)]" aria-label={i18n.t("globalDrivers.summaryAria")}>
          <FocusedDriverCard
            row={focusedDriver}
            ranks={focusedDriverRanks}
            userRow={userDriver}
            userRanks={userDriverRanks}
          />
          <ChampionshipChampionPanel
            sections={championshipChampionSections}
            onOpenChampionship={setChampionshipModal}
          />
        </section>
      ) : null}

      {error ? (
        <div className="rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
          {error}
        </div>
      ) : null}

      <GlassCard hover={false} className="rounded-[28px]">
        {/* Título, contagem e filtros moram DENTRO do rolador junto com a
            tabela: quem gruda no topo é só a régua de colunas, e o resto sobe e
            sai de cena. Sem isso o bloco de cima ficaria plantado na tela o
            tempo todo, comendo um terço da altura útil em 509 pilotos.
            A rolagem é desta caixa, e não da página, porque `position: sticky`
            mira o scrollport mais próximo — e a tabela já precisa ser um, para
            rolar as 17 colunas na horizontal. `sticky left-0` nos blocos de
            cima é o que impede que essa rolagem horizontal os arraste para
            fora: eles têm a largura visível da caixa e ficam colados na
            esquerda enquanto as colunas passam por baixo. */}
        <div className="max-h-[calc(100vh-220px)] overflow-auto" ref={tabelaRef}>
          <div className="sticky left-0 flex flex-wrap items-end justify-between gap-4">
            <div>
              <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">{i18n.t("globalDrivers.allContracts")}</p>
              <h3 className="mt-1 text-xl font-semibold text-text-primary">{t("globalDrivers.worldRankingDrivers")}</h3>
            </div>
            <div className="text-right">
              <p className="text-sm text-text-secondary">
                {t("globalDrivers.countOfDrivers", { shown: filteredRows.length, total: rows.length })}
              </p>
              {relativeRankById ? (
                <p className="mt-1 text-[10px] uppercase tracking-[0.14em] text-accent-primary">
                  {t("globalDrivers.rankRecalculated", { scope: statusFilterLabel(filters.status) })}
                </p>
              ) : null}
            </div>
          </div>

          <div className="sticky left-0 mt-4 border-t border-white/10 pt-3">
            <FilterBar
              filters={filters}
              options={filterOptions}
              onChange={updateFilter}
              onReset={() => setFilters(DEFAULT_FILTERS)}
            />
          </div>

          <table className="mt-4 min-w-full text-left text-sm" aria-label={i18n.t("globalDrivers.rankingAria")}>
            <thead className="sticky top-0 z-10 bg-[#0b1524]">
              <tr className="border-b border-white/[0.08] text-[10px] uppercase tracking-[0.16em] text-text-muted">
                <SortableHeader label="#" sortKey="historical_rank" sort={sort} onSort={handleSort} className="py-3 pr-4" />
                <SortableHeader label={t("globalDrivers.col.driver")} sortKey="nome" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.status")} sortKey="status" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.teamCategory")} sortKey="team_category" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.age")} sortKey="idade" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.career")} sortKey="anos_carreira" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.salaryYear")} sortKey="salario_anual" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.fame")} sortKey="fama" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.index")} sortKey="historical_index" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.titles")} sortKey="titulos" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.wins")} sortKey="vitorias" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.podiums")} sortKey="podios" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.poles")} sortKey="poles" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.points")} sortKey="pontos" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.races")} sortKey="corridas" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.dnfs")} sortKey="dnfs" sort={sort} onSort={handleSort} />
                <SortableHeader label={t("globalDrivers.col.injuries")} sortKey="lesoes" sort={sort} onSort={handleSort} />
              </tr>
            </thead>
            <tbody>
              {tableSections.map((section) => (
                <Fragment key={section.key}>
                  {section.label ? <CategorySectionRow label={section.label} /> : null}
                  {section.rows.map((row) => (
                    <DriverRankingRow
                      key={row.id}
                      row={row}
                      relativeEntry={relativeRankById?.get(row.id)}
                      focusedDriverId={focusedDriver?.id}
                      glowing={row.id === glowDriverId}
                      detailDriverId={selectedDetailDriverId}
                      onFocus={setFocusedDriverId}
                      onOpenDriverDetail={setSelectedDetailDriverId}
                      onOpenTitles={setTitleModalDriver}
                      onToggleFavorite={handleToggleFavorite}
                    />
                  ))}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      </GlassCard>
      {titleModalDriver ? (
        <TitleBreakdownDialog
          row={titleModalDriver}
          onClose={() => setTitleModalDriver(null)}
        />
      ) : null}
      {championshipModal ? (
        <ChampionshipChampionsDialog
          group={championshipModal}
          onClose={() => setChampionshipModal(null)}
        />
      ) : null}
      {selectedDetailDriverId ? (
        <DriverDetailModal
          driverId={selectedDetailDriverId}
          driverIds={rows.map((row) => row.id)}
          onSelectDriver={setSelectedDetailDriverId}
          onFavoriteChange={patchRowFavorite}
          // A ficha aberta AQUI DENTRO não precisa navegar: o destino do card de
          // recorde é esta mesma tela. Reordena no lugar e foca o piloto — a
          // ficha já se fechou sozinha antes de chamar.
          onOpenRanking={({ metric, driverId, category }) => {
            const sortDaMetrica = sortForMetric(metric);
            if (sortDaMetrica) setSort(sortDaMetrica);
            const categoria = filterableCategory(filterOptions, category);
            if (categoria) setFilters((atual) => ({ ...atual, category: categoria }));
            setFocusedDriverId(driverId);
            pedidoDeRolagemRef.current = true;
          }}
          onOpenTeam={(equipe) => {
            if (!equipe?.id) return;
            setAbaDaEquipe("records");
            setEquipeAberta(equipe);
          }}
          onClose={() => setSelectedDetailDriverId(null)}
        />
      ) : null}
      {equipeAberta ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={equipeAberta}
          teams={[]}
          playerTeam={playerTeam}
          activeCategory={null}
          placement="center"
          activeTab={abaDaEquipe}
          onTabChange={setAbaDaEquipe}
          onSelectTeam={setEquipeAberta}
          onClose={() => setEquipeAberta(null)}
        />
      ) : null}
    </div>
  );
}

function GlobalDriversLoading({ onBack }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-5">
      <GlassCard hover={false} className="rounded-[30px]">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">{t("globalDrivers.worldRanking")}</p>
            <h2 className="mt-2 text-3xl font-semibold text-text-primary">{t("globalDrivers.worldRankingDrivers")}</h2>
          </div>
          <button
            type="button"
            onClick={onBack}
            className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary transition-glass hover:text-text-primary"
          >
            {i18n.t("globalDrivers.backToStandings")}
          </button>
        </div>
        <div className="mt-8 rounded-[24px] border border-accent-primary/25 bg-accent-primary/10 p-6 text-center">
          <div className="mx-auto mb-5 h-14 w-14 animate-spin rounded-full border-4 border-white/10 border-t-accent-primary" />
          <p className="text-[10px] uppercase tracking-[0.2em] text-accent-primary">{i18n.t("globalDrivers.loadingEyebrow")}</p>
          <h3 className="mt-3 text-2xl font-semibold text-text-primary">{i18n.t("globalDrivers.loadingTitle")}</h3>
          <p className="mt-3 text-sm text-text-secondary">{i18n.t("globalDrivers.loadingDesc")}</p>
          <div className="mt-5 flex flex-wrap justify-center gap-2">
            {[i18n.t("globalDrivers.tab.history"), i18n.t("globalDrivers.tab.contracts"), i18n.t("globalDrivers.tab.retirements"), i18n.t("globalDrivers.tab.index")].map((label) => (
              <span
                key={label}
                className="rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-[10px] font-semibold uppercase tracking-[0.14em] text-text-secondary"
              >
                {label}
              </span>
            ))}
          </div>
        </div>
      </GlassCard>
    </div>
  );
}

export default GlobalDriversTab;
