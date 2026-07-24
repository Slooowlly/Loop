import { Fragment, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import DriverDetailModal from "../../components/driver/DriverDetailModal";
import GlassCard from "../../components/ui/GlassCard";
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
  buildChampionshipChampionSections,
  buildFilterOptions,
  buildFocusedDriverRanks,
  buildRelativeRanks,
  buildTableSections,
  filterRows,
  sortRows,
} from "../../components/driver/globalDriverRanking";

function GlobalDriversTab({ selectedDriverId, onBack }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const [payload, setPayload] = useState({ rows: [], leaders: {} });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [sort, setSort] = useState(DEFAULT_SORT);
  const [filters, setFilters] = useState(DEFAULT_FILTERS);
  const [focusedDriverId, setFocusedDriverId] = useState(selectedDriverId ?? null);
  const [titleModalDriver, setTitleModalDriver] = useState(null);
  const [championshipModal, setChampionshipModal] = useState(null);
  const [selectedDetailDriverId, setSelectedDetailDriverId] = useState(null);

  useEffect(() => {
    setFocusedDriverId(selectedDriverId ?? null);
  }, [selectedDriverId]);

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
    () => buildRelativeRanks(filteredRows, filters.status !== "Todos"),
    [filteredRows, filters.status],
  );
  const sortedRows = useMemo(() => sortRows(filteredRows, sort), [filteredRows, sort]);
  const tableSections = useMemo(
    () => buildTableSections(sortedRows, filters.category),
    [sortedRows, filters.category],
  );

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
          Voltar para Classificacao
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
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">{i18n.t("globalDrivers.allContracts")}</p>
            <h3 className="mt-2 text-xl font-semibold text-text-primary">{t("globalDrivers.worldRankingDrivers")}</h3>
          </div>
          <div className="text-right">
            <p className="text-sm text-text-secondary">{`${filteredRows.length} de ${rows.length} pilotos`}</p>
            {relativeRankById ? (
              <p className="mt-1 text-[10px] uppercase tracking-[0.14em] text-accent-primary">
                {`# recalculado entre ${statusFilterLabel(filters.status)}`}
              </p>
            ) : null}
          </div>
        </div>

        <div className="mt-5 border-t border-white/10 pt-4">
          <FilterBar
            filters={filters}
            options={filterOptions}
            onChange={updateFilter}
            onReset={() => setFilters(DEFAULT_FILTERS)}
          />
        </div>

        <div className="mt-5 overflow-x-auto">
          <table className="min-w-full text-left text-sm" aria-label={i18n.t("globalDrivers.rankingAria")}>
            <thead>
              <tr className="border-b border-white/8 text-[10px] uppercase tracking-[0.16em] text-text-muted">
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
          onClose={() => setSelectedDetailDriverId(null)}
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
