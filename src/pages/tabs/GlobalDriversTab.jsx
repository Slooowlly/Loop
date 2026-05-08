import { Fragment, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import DriverDetailModal from "../../components/driver/DriverDetailModal";
import GlassCard from "../../components/ui/GlassCard";
import FlagIcon from "../../components/ui/FlagIcon";
import TeamLogoMark from "../../components/team/TeamLogoMark";
import useCareerStore from "../../stores/useCareerStore";
import { getCategoryTier } from "../../utils/formatters";

const DEFAULT_SORT = { key: "historical_index", direction: "desc" };
const DEFAULT_FILTERS = {
  status: "Todos",
  category: "Todas",
  nationality: "Todas",
  minAge: "",
  maxAge: "",
  champions: "all",
  injured: "all",
};

const SORTERS = {
  historical_rank: (row) => row.historical_rank ?? 9999,
  nome: (row) => row.nome ?? "",
  status: (row) => row.status ?? "",
  team_category: (row) => (row.status === "Aposentado" ? row.anos_aposentado ?? -1 : -1),
  idade: (row) => row.idade ?? 0,
  anos_carreira: (row) => row.anos_carreira ?? 0,
  salario_anual: (row) => row.salario_anual ?? 0,
  historical_index: (row) => row.historical_index ?? 0,
  titulos: (row) => row.titulos ?? 0,
  vitorias: (row) => row.vitorias ?? 0,
  podios: (row) => row.podios ?? 0,
  poles: (row) => row.poles ?? 0,
  pontos: (row) => row.pontos ?? 0,
  corridas: (row) => row.corridas ?? 0,
  dnfs: (row) => row.dnfs ?? 0,
  lesoes: (row) => row.lesoes ?? 0,
};

function GlobalDriversTab({ selectedDriverId, onBack }) {
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
        setError("Carreira não carregada.");
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
          setError(typeof invokeError === "string" ? invokeError : "Nao foi possivel carregar o panorama de pilotos.");
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

  if (loading) {
    return <GlobalDriversLoading onBack={onBack} />;
  }

  return (
    <div className="space-y-5">
      <header className="flex flex-wrap items-start justify-between gap-4 px-1">
        <div>
          <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">Ranking mundial</p>
          <h2 className="mt-2 text-3xl font-semibold text-text-primary">Panorama global de pilotos</h2>
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
        <section className="grid items-stretch gap-4 lg:grid-cols-[minmax(0,1.22fr)_minmax(330px,0.78fr)]" aria-label="Resumo do ranking mundial">
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
            <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">Todos os contratos e historicos</p>
            <h3 className="mt-2 text-xl font-semibold text-text-primary">Ranking mundial de pilotos</h3>
          </div>
          <p className="text-sm text-text-secondary">{`${filteredRows.length} de ${rows.length} pilotos`}</p>
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
          <table className="min-w-full text-left text-sm" aria-label="Ranking mundial de pilotos">
            <thead>
              <tr className="border-b border-white/8 text-[10px] uppercase tracking-[0.16em] text-text-muted">
                <SortableHeader label="#" sortKey="historical_rank" sort={sort} onSort={handleSort} className="py-3 pr-4" />
                <SortableHeader label="Piloto" sortKey="nome" sort={sort} onSort={handleSort} />
                <SortableHeader label="Status" sortKey="status" sort={sort} onSort={handleSort} />
                <SortableHeader label="Equipe/Categoria" sortKey="team_category" sort={sort} onSort={handleSort} />
                <SortableHeader label="Idade" sortKey="idade" sort={sort} onSort={handleSort} />
                <SortableHeader label="Carreira" sortKey="anos_carreira" sort={sort} onSort={handleSort} />
                <SortableHeader label="Salario" sortKey="salario_anual" sort={sort} onSort={handleSort} />
                <SortableHeader label="Indice" sortKey="historical_index" sort={sort} onSort={handleSort} />
                <SortableHeader label="Titulos" sortKey="titulos" sort={sort} onSort={handleSort} />
                <SortableHeader label="Vit." sortKey="vitorias" sort={sort} onSort={handleSort} />
                <SortableHeader label="Pod." sortKey="podios" sort={sort} onSort={handleSort} />
                <SortableHeader label="Poles" sortKey="poles" sort={sort} onSort={handleSort} />
                <SortableHeader label="Pts" sortKey="pontos" sort={sort} onSort={handleSort} />
                <SortableHeader label="Corr." sortKey="corridas" sort={sort} onSort={handleSort} />
                <SortableHeader label="DNFs" sortKey="dnfs" sort={sort} onSort={handleSort} />
                <SortableHeader label="Lesoes" sortKey="lesoes" sort={sort} onSort={handleSort} />
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
                      focusedDriverId={focusedDriver?.id}
                      detailDriverId={selectedDetailDriverId}
                      onFocus={setFocusedDriverId}
                      onOpenDriverDetail={setSelectedDetailDriverId}
                      onOpenTitles={setTitleModalDriver}
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
          onClose={() => setSelectedDetailDriverId(null)}
        />
      ) : null}
    </div>
  );
}

function GlobalDriversLoading({ onBack }) {
  return (
    <div className="space-y-5">
      <GlassCard hover={false} className="rounded-[30px]">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">Ranking mundial</p>
            <h2 className="mt-2 text-3xl font-semibold text-text-primary">Ranking mundial de pilotos</h2>
          </div>
          <button
            type="button"
            onClick={onBack}
            className="rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] text-text-secondary transition-glass hover:text-text-primary"
          >
            Voltar para Classificacao
          </button>
        </div>
        <div className="mt-8 rounded-[24px] border border-accent-primary/25 bg-accent-primary/10 p-6 text-center">
          <div className="mx-auto mb-5 h-14 w-14 animate-spin rounded-full border-4 border-white/10 border-t-accent-primary" />
          <p className="text-[10px] uppercase tracking-[0.2em] text-accent-primary">Panorama global de pilotos</p>
          <h3 className="mt-3 text-2xl font-semibold text-text-primary">Montando ranking mundial</h3>
          <p className="mt-3 text-sm text-text-secondary">Reunindo pilotos ativos, livres e aposentados.</p>
          <div className="mt-5 flex flex-wrap justify-center gap-2">
            {["Histórico", "Contratos", "Aposentadorias", "Índice"].map((label) => (
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

function FocusedDriverCard({ row, ranks, userRow, userRanks }) {
  const metrics = [
    { label: "Indice", value: formatIndex(row.historical_index), rank: row.historical_rank },
    { label: "Corridas", value: row.corridas, rank: ranks.races },
    { label: "Vitorias", value: row.vitorias, rank: ranks.wins },
    { label: "Podios", value: row.podios, rank: ranks.podiums },
    { label: "Carreira", value: formatYears(row.anos_carreira), rank: ranks.careerYears },
  ];

  return (
    <GlassCard hover={false} as="article" className="flex h-full flex-col overflow-hidden rounded-[28px] border-accent-primary/25 p-0">
      <div className="flex flex-1 flex-col p-5 sm:p-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-[10px] uppercase tracking-[0.2em] text-accent-primary">Piloto em foco</p>
            <h3 className="mt-2 text-2xl font-semibold text-text-primary">{row.nome}</h3>
          </div>
          <span className="rounded-full border border-accent-primary/25 bg-accent-primary/10 px-3 py-1 font-mono text-xs text-accent-primary">
            Rank #{row.historical_rank}
          </span>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <span className={statusClass(row)}>{row.status}</span>
          <span className="rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-xs text-text-secondary">
            {teamCategoryLabel(row)}
          </span>
        </div>
        <div className="mt-5 grid flex-1 content-end gap-2 sm:grid-cols-5">
          {metrics.map((metric) => (
            <FocusStat key={metric.label} {...metric} />
          ))}
        </div>
      </div>
      {userRow ? <UserDriverFocusCard row={userRow} ranks={userRanks} /> : null}
    </GlassCard>
  );
}

function UserDriverFocusCard({ row, ranks }) {
  const stats = [
    { label: "Indice", value: formatIndex(row.historical_index) },
    { label: "Vitorias", value: row.vitorias ?? 0 },
    { label: "Titulos", value: row.titulos ?? 0 },
    { label: "Carreira", value: formatYears(row.anos_carreira) },
  ];

  return (
    <section className="border-t border-white/10 bg-black/15 px-5 py-4 sm:px-6" aria-label="Seu piloto no ranking mundial">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">Seu piloto</p>
          <h4 className="mt-2 text-lg font-semibold text-text-primary">{row.nome}</h4>
          <p className="mt-1 text-sm text-text-secondary">{teamCategoryLabel(row)}</p>
        </div>
        <span className="rounded-full border border-accent-primary/25 bg-accent-primary/10 px-3 py-1 font-mono text-xs text-accent-primary">
          Rank #{row.historical_rank}
        </span>
      </div>
      <div className="mt-3 grid gap-2 sm:grid-cols-4">
        {stats.map((stat) => (
          <div key={stat.label} className="min-h-14 border-l border-white/10 pl-3">
            <p className="text-[10px] uppercase tracking-[0.12em] text-text-muted">{stat.label}</p>
            <p className="mt-1 font-mono text-sm font-semibold text-text-primary">{stat.value}</p>
          </div>
        ))}
      </div>
      <p className="mt-3 text-xs text-text-muted">
        Top #{row.historical_rank || "--"} geral
        {ranks.wins ? ` / Top #${ranks.wins} em vitorias` : ""}
      </p>
    </section>
  );
}

function FocusStat({ label, value, rank }) {
  return (
    <div className="min-h-24 rounded-2xl border border-white/8 bg-black/10 p-3">
      <p className="text-[10px] uppercase tracking-[0.14em] text-text-muted">{label}</p>
      <p className="mt-2 font-mono text-lg font-semibold text-text-primary">{value ?? 0}</p>
      <p className="mt-1 text-xs text-accent-primary">Top #{rank || "--"}</p>
    </div>
  );
}

function ChampionshipChampionPanel({ sections, onOpenChampionship }) {
  const totalGroups = sections.reduce((total, section) => total + section.groups.length, 0);

  return (
    <GlassCard hover={false} as="aside" className="flex max-h-[430px] flex-col overflow-hidden rounded-[28px] p-5 sm:p-6">
      <div className="flex shrink-0 items-start justify-between gap-3">
        <div>
          <p className="text-[10px] uppercase tracking-[0.2em] text-accent-primary">Campeoes</p>
          <h3 className="mt-2 text-xl font-semibold text-text-primary">Campeoes por campeonato</h3>
          <p className="mt-2 text-sm text-text-secondary">Categorias com historico de campeoes no ranking.</p>
        </div>
        <span className="rounded-full border border-white/10 px-3 py-1 font-mono text-xs text-text-muted">
          {totalGroups} grupos
        </span>
      </div>

      <div className="scroll-area mt-4 min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
        {totalGroups > 0 ? (
          sections.map((section) => (
            <div key={section.key} className="space-y-2">
              {section.label ? (
                <div className="flex items-center gap-3 py-1">
                  <span className="h-px flex-1 bg-white/10" />
                  <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-text-muted">
                    {section.label}
                  </span>
                  <span className="h-px flex-1 bg-white/10" />
                </div>
              ) : null}
              <div className="grid gap-2 sm:grid-cols-2">
                {section.groups.map((group) => (
                  <button
                    key={group.key}
                    type="button"
                    aria-label={`Ver campeoes de ${group.label}`}
                    onClick={() => onOpenChampionship(group)}
                    className="flex min-h-20 items-center justify-between gap-4 rounded-2xl border border-white/8 bg-black/10 px-4 py-3 text-left transition-glass hover:border-accent-primary/40 hover:bg-accent-primary/10"
                  >
                    <span>
                      <span className="block text-sm font-semibold text-text-primary">{group.label}</span>
                      <span className="mt-1 block text-xs text-text-muted">
                        {group.champions.slice(0, 2).map((champion) => champion.name).join(", ") || "Sem nomes"}
                      </span>
                    </span>
                    <span className="rounded-full border border-accent-secondary/25 bg-accent-secondary/10 px-3 py-1 font-mono text-xs text-accent-secondary">
                      {group.championCount}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          ))
        ) : (
          <p className="rounded-2xl border border-white/8 bg-black/10 px-4 py-3 text-sm text-text-secondary">
            Sem campeoes registrados.
          </p>
        )}
      </div>
    </GlassCard>
  );
}

function FilterBar({ filters, options, onChange, onReset }) {
  return (
    <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-7">
      <FilterSelect
        label="Status"
        value={filters.status}
        onChange={(value) => onChange("status", value)}
        options={[
          ["Todos", "Todos"],
          ["Ativo", "Ativos"],
          ["Livre", "Livres"],
          ["Aposentado", "Aposentados"],
        ]}
      />
      <FilterSelect
        label="Categoria"
        value={filters.category}
        onChange={(value) => onChange("category", value)}
        options={[
          ["Todas", "Todas"],
          ...options.categories.map((category) => [category, categoryLabel(category)]),
        ]}
      />
      <FilterSelect
        label="Nacionalidade"
        value={filters.nationality}
        onChange={(value) => onChange("nationality", value)}
        options={[
          ["Todas", "Todas"],
          ...options.nationalities.map((nationality) => [nationality, nationality]),
        ]}
      />
      <FilterSelect
        label="Campeões"
        value={filters.champions}
        onChange={(value) => onChange("champions", value)}
        options={[
          ["all", "Todos"],
          ["champions", "Apenas campeões"],
        ]}
      />
      <FilterSelect
        label="Lesionados"
        value={filters.injured}
        onChange={(value) => onChange("injured", value)}
        options={[
          ["all", "Todos"],
          ["injured", "Apenas lesionados"],
        ]}
      />
      <div className="grid grid-cols-2 gap-2 xl:col-span-2">
        <FilterInput
          label="Idade mínima"
          value={filters.minAge}
          onChange={(value) => onChange("minAge", value)}
        />
        <FilterInput
          label="Idade máxima"
          value={filters.maxAge}
          onChange={(value) => onChange("maxAge", value)}
        />
      </div>
      <button
        type="button"
        onClick={onReset}
        className="rounded-xl border border-white/10 bg-white/[0.04] px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-text-secondary transition-glass hover:text-text-primary xl:col-start-7"
      >
        Limpar filtros
      </button>
    </div>
  );
}

function FilterSelect({ label, value, onChange, options }) {
  return (
    <label className="text-[10px] font-semibold uppercase tracking-[0.14em] text-text-muted">
      <span>{label}</span>
      <select
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="mt-2 w-full rounded-xl border border-white/10 bg-app-card px-3 py-2 text-xs normal-case tracking-normal text-text-primary outline-none transition-glass focus:border-accent-primary/60"
      >
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue} value={optionValue} className="bg-app-card text-text-primary">
            {optionLabel}
          </option>
        ))}
      </select>
    </label>
  );
}

function FilterInput({ label, value, onChange }) {
  return (
    <label className="text-[10px] font-semibold uppercase tracking-[0.14em] text-text-muted">
      <span>{label}</span>
      <input
        aria-label={label}
        type="number"
        min="0"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="mt-2 w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-xs normal-case tracking-normal text-text-primary outline-none transition-glass focus:border-accent-primary/60"
      />
    </label>
  );
}

function CategorySectionRow({ label }) {
  return (
    <tr className="border-y border-accent-primary/15 bg-accent-primary/[0.06]">
      <td colSpan={16} className="px-4 py-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent-primary">
        {label}
      </td>
    </tr>
  );
}

function DriverRankingRow({ row, focusedDriverId, detailDriverId, onFocus, onOpenDriverDetail, onOpenTitles }) {
  const isDetailDriver = row.id === detailDriverId;
  return (
    <tr
      onClick={() => onFocus(row.id)}
      onDoubleClick={() => {
        onFocus(row.id);
        onOpenDriverDetail(row.id);
      }}
      className={[
        "cursor-pointer border-b border-white/6 last:border-0 transition-glass hover:bg-white/[0.04]",
        row.id === focusedDriverId ? "bg-accent-primary/12 ring-1 ring-accent-primary/40" : "",
        isDetailDriver ? "bg-accent-secondary/12 ring-2 ring-accent-secondary/60 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]" : "",
        row.is_jogador ? "border-l-2 border-l-accent-primary/70" : "",
        row.status === "Livre" && !isDetailDriver ? "opacity-60" : "",
        row.status === "Aposentado" && !isDetailDriver ? "opacity-50" : "",
      ].join(" ")}
    >
      <td className="py-3 pr-4 font-mono text-xs text-text-muted">
        <RankCell rank={row.historical_rank} delta={row.historical_rank_delta} />
      </td>
      <td className="px-4 py-3">
        <div className="flex min-w-[190px] items-center gap-2">
          {row.nacionalidade ? <FlagIcon nacionalidade={row.nacionalidade} /> : null}
          <span
            onDoubleClick={(event) => {
              event.stopPropagation();
              onOpenDriverDetail(row.id);
            }}
            className={row.is_jogador ? "font-semibold text-accent-primary" : "font-semibold text-text-primary"}
          >
            {row.nome}
          </span>
          {row.is_jogador ? (
            <span className="rounded-full border border-accent-primary/30 bg-accent-primary/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-accent-primary">
              Voce
            </span>
          ) : null}
          {row.is_lesionado ? (
            <span
              title={row.lesao_ativa_tipo ? `Lesionado: ${row.lesao_ativa_tipo}` : "Lesionado"}
              className="rounded-full border border-status-red/25 bg-status-red/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-status-red"
            >
              Lesionado
            </span>
          ) : null}
        </div>
      </td>
      <td className="px-4 py-3">
        <span className={statusClass(row.status)}>
          {row.status}
        </span>
      </td>
      <td className="px-4 py-3">
        <div className="flex min-w-[170px] items-center gap-2">
          {row.equipe_nome ? (
            <TeamLogoMark teamName={row.equipe_nome} color={row.equipe_cor_primaria} size="xs" />
          ) : null}
          <span className="truncate text-text-secondary" title={statusTitle(row)}>
            {teamCategoryLabel(row)}
          </span>
        </div>
      </td>
      <MetricCell value={row.idade || "-"} />
      <td className="px-4 py-3 font-mono text-text-primary">{formatYears(row.anos_carreira)}</td>
      <td className="px-4 py-3 font-mono text-text-primary">{formatMoney(row.salario_anual)}</td>
      <td className="px-4 py-3 font-mono text-text-primary">{formatIndex(row.historical_index)}</td>
      <TitleMetricCell row={row} onOpenTitles={onOpenTitles} />
      <MetricCell value={row.vitorias} />
      <MetricCell value={row.podios} />
      <MetricCell value={row.poles} />
      <MetricCell value={row.pontos} />
      <MetricCell value={row.corridas} />
      <MetricCell value={row.dnfs} />
      <MetricCell value={row.lesoes} />
    </tr>
  );
}

function TitleMetricCell({ row, onOpenTitles }) {
  if (!row.titulos || row.titulos <= 0) {
    return <MetricCell value={row.titulos} />;
  }

  return (
    <td className="px-4 py-3 font-mono text-text-primary">
      <button
        type="button"
        aria-label={`Ver titulos de ${row.nome}`}
        onClick={(event) => {
          event.stopPropagation();
          onOpenTitles(row);
        }}
        onDoubleClick={(event) => event.stopPropagation()}
        className="rounded-md px-2 py-1 font-mono text-accent-primary underline decoration-accent-primary/40 underline-offset-4 transition-glass hover:bg-accent-primary/10 hover:text-accent-secondary"
      >
        {row.titulos}
      </button>
    </td>
  );
}

function TitleBreakdownDialog({ row, onClose }) {
  const categories = Array.isArray(row.titulos_por_categoria) ? row.titulos_por_categoria : [];
  const titleId = `title-breakdown-${row.id}`;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      role="presentation"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-md rounded-2xl border border-white/10 bg-app-card p-5 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-[10px] uppercase tracking-[0.18em] text-accent-primary">Titulos</p>
            <h3 id={titleId} className="mt-1 text-xl font-semibold text-text-primary">
              Titulos de {row.nome}
            </h3>
            <p className="mt-1 text-sm text-text-secondary">Total: {row.titulos}</p>
          </div>
          <button
            type="button"
            aria-label="Fechar titulos"
            onClick={onClose}
            className="rounded-lg border border-white/10 px-3 py-1 text-sm text-text-secondary transition-glass hover:border-accent-primary/40 hover:text-text-primary"
          >
            X
          </button>
        </div>

        <div className="mt-5 divide-y divide-white/8">
          {categories.length > 0 ? (
            categories.map((entry) => (
              <div key={`${entry.categoria}-${entry.classe ?? "geral"}`} className="flex items-center justify-between py-3">
                <span className="font-semibold text-text-primary">{titleCategoryLabel(entry)}</span>
                <span className="font-mono text-sm text-accent-secondary">
                  {entry.titulos} {entry.titulos === 1 ? "titulo" : "titulos"}
                </span>
              </div>
            ))
          ) : (
            <p className="py-3 text-sm text-text-secondary">Sem detalhes por categoria.</p>
          )}
        </div>
      </div>
    </div>
  );
}

function ChampionshipChampionsDialog({ group, onClose }) {
  const titleId = `championship-champions-${group.key}`;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      role="presentation"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="max-h-[85vh] w-full max-w-lg overflow-hidden rounded-2xl border border-white/10 bg-app-card p-5 shadow-2xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-[10px] uppercase tracking-[0.18em] text-accent-primary">Campeonato</p>
            <h3 id={titleId} className="mt-1 text-xl font-semibold text-text-primary">
              Campeoes de {group.label}
            </h3>
            <p className="mt-1 text-sm text-text-secondary">
              {group.championCount} {group.championCount === 1 ? "campeao" : "campeoes"}
            </p>
          </div>
          <button
            type="button"
            aria-label="Fechar campeoes"
            onClick={onClose}
            className="rounded-lg border border-white/10 px-3 py-1 text-sm text-text-secondary transition-glass hover:border-accent-primary/40 hover:text-text-primary"
          >
            X
          </button>
        </div>

        <div className="mt-5 max-h-[58vh] overflow-y-auto pr-2">
          <div className="divide-y divide-white/8">
          {group.champions.map((champion) => (
            <div key={champion.id} className="flex items-center justify-between gap-4 py-3">
              <div>
                <p className="font-semibold text-text-primary">{champion.name}</p>
                <p className="mt-1 font-mono text-xs text-text-muted">
                  {champion.years.length > 0 ? champion.years.join(", ") : "Anos indisponiveis"}
                </p>
              </div>
              <span className="font-mono text-sm text-accent-secondary">
                {champion.titles} {champion.titles === 1 ? "titulo" : "titulos"}
              </span>
            </div>
          ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function SortableHeader({ label, sortKey, sort, onSort, className = "px-4 py-3" }) {
  const active = sort.key === sortKey;
  const marker = active ? (sort.direction === "asc" ? "↑" : "↓") : "↕";
  return (
    <th className={className}>
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        className="inline-flex items-center gap-1 rounded-lg text-left transition-glass hover:text-text-primary"
      >
        <span>{label}</span>
        <span className={active ? "text-accent-primary" : "text-text-muted"}>{marker}</span>
      </button>
    </th>
  );
}

function MetricCell({ value }) {
  return <td className="px-4 py-3 font-mono text-text-primary">{value ?? 0}</td>;
}

function RankCell({ rank, delta }) {
  const numericDelta = Number(delta ?? 0);
  if (!numericDelta) {
    return <span>{formatRank(rank)}</span>;
  }

  const gained = numericDelta > 0;
  const amount = Math.abs(numericDelta);
  const label = `${gained ? "↑" : "↓"}${amount}`;
  const title = `${gained ? "Subiu" : "Desceu"} ${amount} ${amount === 1 ? "posição" : "posições"} desde a última corrida`;

  return (
    <span className="inline-flex items-center gap-2">
      <span>{formatRank(rank)}</span>
      <span
        title={title}
        className={[
          "whitespace-nowrap rounded-full border px-1.5 py-0.5 text-[10px] font-semibold leading-none",
          gained
            ? "border-status-green/25 bg-status-green/10 text-status-green"
            : "border-status-red/25 bg-status-red/10 text-status-red",
        ].join(" ")}
      >
        {label}
      </span>
    </span>
  );
}

function sortRows(rows, sort) {
  const getter = SORTERS[sort.key] ?? SORTERS.historical_index;
  return [...rows].sort((a, b) => {
    const aValue = getter(a);
    const bValue = getter(b);
    const direction = sort.direction === "asc" ? 1 : -1;
    if (typeof aValue === "string" || typeof bValue === "string") {
      return String(aValue).localeCompare(String(bValue), "pt-BR") * direction;
    }
    return ((aValue > bValue ? 1 : 0) - (aValue < bValue ? 1 : 0)) * direction || a.nome.localeCompare(b.nome, "pt-BR");
  });
}

function filterRows(rows, filters) {
  const minAge = parseOptionalNumber(filters.minAge);
  const maxAge = parseOptionalNumber(filters.maxAge);

  return rows.filter((row) => {
    if (filters.status !== "Todos" && row.status !== filters.status) return false;
    if (filters.category !== "Todas" && !rowCategories(row).includes(filters.category)) return false;
    if (filters.nationality !== "Todas" && row.nacionalidade !== filters.nationality) return false;
    if (filters.champions === "champions" && (row.titulos ?? 0) <= 0) return false;
    if (filters.injured === "injured" && !row.is_lesionado) return false;
    if (minAge != null && (row.idade ?? 0) < minAge) return false;
    if (maxAge != null && (row.idade ?? 0) > maxAge) return false;
    return true;
  });
}

function buildTableSections(rows, selectedCategory) {
  if (selectedCategory === "Todas") {
    return [{ key: "all", label: null, rows }];
  }

  const currentRows = [];
  const pastRows = [];

  rows.forEach((row) => {
    if (row.status === "Ativo" && row.categoria_atual === selectedCategory) {
      currentRows.push(row);
    } else {
      pastRows.push(row);
    }
  });

  return [
    {
      key: "current",
      label: currentRows.length > 0 ? `Atualmente em ${categoryLabel(selectedCategory)}` : null,
      rows: currentRows,
    },
    {
      key: "past",
      label: pastRows.length > 0 ? `Ja passaram por ${categoryLabel(selectedCategory)}` : null,
      rows: pastRows,
    },
  ].filter((section) => section.rows.length > 0);
}

function buildFilterOptions(rows) {
  return {
    categories: uniqueSortedCategories(rows.flatMap(rowCategories)),
    nationalities: uniqueSorted(rows.map((row) => row.nacionalidade).filter(Boolean)),
  };
}

function buildFocusedDriverRanks(rows, focusedDriver) {
  if (!focusedDriver) {
    return {};
  }

  return {
    races: metricRank(rows, "corridas", focusedDriver.id),
    wins: metricRank(rows, "vitorias", focusedDriver.id),
    titles: metricRank(rows, "titulos", focusedDriver.id),
    podiums: metricRank(rows, "podios", focusedDriver.id),
    careerYears: metricRank(rows, "anos_carreira", focusedDriver.id),
  };
}

function metricRank(rows, key, targetId) {
  const sorted = [...rows]
    .filter((row) => Number(row?.[key] ?? 0) > 0)
    .sort((left, right) =>
      Number(right?.[key] ?? 0) - Number(left?.[key] ?? 0)
      || String(left.nome ?? "").localeCompare(String(right.nome ?? ""), "pt-BR"),
    );
  let rank = 0;
  let previousValue = null;

  for (let index = 0; index < sorted.length; index += 1) {
    const value = Number(sorted[index]?.[key] ?? 0);
    if (previousValue == null || value !== previousValue) {
      rank = index + 1;
      previousValue = value;
    }
    if (sorted[index].id === targetId) {
      return rank;
    }
  }

  return null;
}

function buildChampionshipChampionSections(rows) {
  const groups = buildChampionshipChampionGroups(rows);
  const normal = groups.filter((group) => !group.special).sort(compareChampionshipGroups);
  const special = groups.filter((group) => group.special).sort(compareChampionshipGroups);

  return [
    { key: "normal", label: null, groups: normal },
    { key: "special", label: "Eventos especiais", groups: special },
  ].filter((section) => section.groups.length > 0);
}

function buildChampionshipChampionGroups(rows) {
  const groups = new Map();

  rows.forEach((row) => {
    const titleEntries = Array.isArray(row.titulos_por_categoria) ? row.titulos_por_categoria : [];
    titleEntries.forEach((entry) => {
      const titles = Number(entry?.titulos ?? 0);
      if (titles <= 0) return;

      const key = titleGroupKey(entry);
      const existing = groups.get(key) ?? {
        key,
        label: titleCategoryLabel(entry),
        category: entry?.categoria ?? "",
        className: entry?.classe ?? entry?.class_name ?? "",
        special: isSpecialTitleEntry(entry),
        totalTitles: 0,
        champions: [],
      };

      const years = titleEntryYears(entry);
      existing.totalTitles += titles;
      existing.champions.push({
        id: row.id,
        name: row.nome,
        rank: row.historical_rank,
        titles,
        years,
        latestYear: years[0] ?? 0,
      });
      groups.set(key, existing);
    });
  });

  return [...groups.values()]
    .map((group) => ({
      ...group,
      championCount: group.champions.length,
      champions: group.champions.sort((left, right) =>
        right.titles - left.titles
        || right.latestYear - left.latestYear
        || (left.rank ?? 9999) - (right.rank ?? 9999)
        || left.name.localeCompare(right.name, "pt-BR"),
      ),
    }))
    .sort(compareChampionshipGroups);
}

function titleGroupKey(entry) {
  return `${entry?.categoria ?? "unknown"}::${entry?.classe ?? entry?.class_name ?? ""}`;
}

function titleEntryYears(entry) {
  const years = Array.isArray(entry?.anos) ? entry.anos : [];
  return [...new Set(years.map(Number).filter((year) => Number.isFinite(year) && year > 0))]
    .sort((left, right) => right - left);
}

function isSpecialTitleEntry(entry) {
  return ["endurance", "production_challenger"].includes(entry?.categoria);
}

function compareChampionshipGroups(left, right) {
  return championshipGroupOrder(left) - championshipGroupOrder(right)
    || right.championCount - left.championCount
    || right.totalTitles - left.totalTitles
    || String(left.label).localeCompare(String(right.label), "pt-BR");
}

function championshipGroupOrder(group) {
  if (group.special) {
    const specialOrder = {
      "endurance:lmp2": 10,
      "endurance:gt3": 20,
      "endurance:gt4": 30,
      "production_challenger:bmw": 40,
      "production_challenger:toyota": 50,
      "production_challenger:mazda": 60,
    };
    return specialOrder[`${group.category}:${String(group.className ?? "").toLowerCase()}`] ?? 999;
  }

  const normalOrder = {
    lmp2: 10,
    gt3: 20,
    gt4: 30,
    bmw_m2: 40,
    mazda_amador: 50,
    toyota_amador: 60,
    mazda_rookie: 70,
    toyota_rookie: 80,
  };
  return normalOrder[group.category] ?? 900 + categoryTierOrder(group.category);
}

function rowCategories(row) {
  const categories = Array.isArray(row.categorias_historicas) ? row.categorias_historicas : [];
  return uniqueSortedCategories([...categories, row.categoria_atual].filter(Boolean));
}

function uniqueSorted(values) {
  return [...new Set(values)].sort((a, b) => categoryLabel(a).localeCompare(categoryLabel(b), "pt-BR"));
}

function uniqueSortedCategories(values) {
  return [...new Set(values)].sort(compareCategoriesByProgression);
}

function compareCategoriesByProgression(a, b) {
  const aTier = categoryTierOrder(a);
  const bTier = categoryTierOrder(b);
  return aTier - bTier || categoryLabel(a).localeCompare(categoryLabel(b), "pt-BR");
}

function categoryTierOrder(category) {
  const tier = getCategoryTier(category);
  return tier > 0 ? tier : 999;
}

function parseOptionalNumber(value) {
  if (value === "" || value == null) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function defaultDirection(key) {
  return key === "nome" || key === "status" || key === "historical_rank" ? "asc" : "desc";
}

function statusClass(status) {
  if (status === "Aposentado") return "rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-xs text-text-muted";
  if (status === "Livre") return "rounded-full border border-status-yellow/20 bg-status-yellow/10 px-3 py-1 text-xs text-status-yellow";
  return "rounded-full border border-status-green/20 bg-status-green/10 px-3 py-1 text-xs text-status-green";
}

function statusTitle(row) {
  if (row.status === "Aposentado" && row.temporada_aposentadoria) {
    return `Aposentado em ${row.temporada_aposentadoria}`;
  }
  return undefined;
}

function teamCategoryLabel(row) {
  const category = categoryLabel(row.categoria_atual);
  if (row.equipe_nome) return `${row.equipe_nome} / ${category}`;
  if (row.status === "Aposentado") {
    const retiredLabel = row.anos_aposentado != null ? `Há ${row.anos_aposentado} anos` : "Aposentado";
    return `${retiredLabel} / ${category}`;
  }
  if (row.status === "Livre" && category !== "-") return `Livre / ${category}`;
  if (row.status === "Livre") return "Livre";
  return category;
}

function categoryLabel(category) {
  if (!category) return "-";
  return category
    .split("_")
    .map((part) => {
      const upper = part.toUpperCase();
      if (["GT3", "GT4", "BMW", "M2"].includes(upper)) return upper;
      return part.charAt(0).toUpperCase() + part.slice(1);
    })
    .join(" ");
}

function titleCategoryLabel(entry) {
  const category = titleBaseCategoryLabel(entry?.categoria);
  const className = classLabel(entry?.classe ?? entry?.class_name);
  return className ? `${category}/${className}` : category;
}

function titleBaseCategoryLabel(category) {
  if (category === "production_challenger") return "Production";
  if (category === "endurance") return "Endurance";
  if (category === "lmp2") return "LMP2";
  if (category === "bmw_m2") return "BMW";
  if (category === "mazda_amador") return "Mazda Cup";
  if (category === "toyota_amador") return "Toyota Cup";
  if (category === "mazda_rookie") return "Mazda Rookie";
  if (category === "toyota_rookie") return "Toyota Rookie";
  return categoryLabel(category);
}

function classLabel(className) {
  if (!className) return "";
  const normalized = String(className).trim().toLowerCase();
  const labels = {
    mazda: "Mazda",
    toyota: "Toyota",
    bmw: "BMW",
    gt4: "GT4",
    gt3: "GT3",
    lmp2: "LMP2",
  };
  return labels[normalized] ?? categoryLabel(normalized);
}

function formatRank(rank) {
  return rank ? String(rank).padStart(2, "0") : "--";
}

function formatIndex(value) {
  return Number(value ?? 0).toLocaleString("pt-BR", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
}

function formatYears(value) {
  return value == null || value < 0 ? "-" : `${value} anos`;
}

function formatMoney(value) {
  if (value == null || value <= 0) return "-";
  if (value >= 1000000) {
    return `$${(value / 1000000).toLocaleString("pt-BR", {
      maximumFractionDigits: 1,
    })}M`;
  }
  return `$${Math.round(value / 1000).toLocaleString("pt-BR")}k`;
}

export default GlobalDriversTab;
