import { useEffect, useMemo, useState } from "react";

import GlassButton from "../ui/GlassButton";
import LoadingOverlay from "../ui/LoadingOverlay";
import useCareerStore from "../../stores/useCareerStore";
import SeasonSectionHeader from "./SeasonSectionHeader";
import TeamLogoMark from "../team/TeamLogoMark";

const CATEGORY_LABELS = {
  production_challenger: "Production",
  endurance: "Endurance",
};

const CATEGORY_LOGOS = {
  production_challenger: "/categorias/recortadas/PRODUCTION.png",
  endurance: "/categorias/recortadas/ENDURANCE.png",
};

const CATEGORY_COLORS = {
  all: "rgba(255,255,255,0.35)",
  production_challenger: "#9030E0",
  endurance: "#30D010",
};

const CATEGORY_FILTERS = [
  { id: "all", label: "Todas", color: CATEGORY_COLORS.all },
  {
    id: "production_challenger",
    label: "Production",
    color: CATEGORY_COLORS.production_challenger,
  },
  { id: "endurance", label: "Endurance", color: CATEGORY_COLORS.endurance },
];

const CATEGORY_ORDER = ["production_challenger", "endurance"];
const CANDIDATE_GROUP_ORDER = ["gt3", "gt4", "bmw_m2", "toyota_amador", "mazda_amador"];

const CANDIDATE_GROUP_LABELS = {
  gt3: "GT3 Championship",
  gt4: "GT4 Series",
  bmw_m2: "BMW M2 Cup",
  toyota_amador: "Toyota Cup",
  mazda_amador: "Mazda Cup",
};

const CLASS_COLORS = {
  mazda: "#C8102E",
  toyota: "#E8841A",
  bmw: "#6B4FBB",
  gt4: "#58a6ff",
  gt3: "#f85149",
  lmp2: "#d29922",
  geral: "#8b949e",
};

const CANDIDATE_GROUP_COLORS = {
  gt3: "#f85149",
  gt4: "#58a6ff",
  bmw_m2: "#6B4FBB",
  toyota_amador: "#E8841A",
  mazda_amador: "#C8102E",
};

const DAILY_LOG_CLASS_ORDER = ["gt3", "gt4", "lmp2", "bmw", "toyota", "mazda", "geral"];
const TEAM_CLASS_ORDER = {
  production_challenger: ["bmw", "toyota", "mazda"],
  endurance: ["lmp2", "gt3", "gt4"],
};

const LICENSE_COLORS = {
  R: { text: "#9ba3ae", bg: "rgba(155,163,174,0.12)" },
  A: { text: "#3fb950", bg: "rgba(63,185,80,0.12)" },
  P: { text: "#58a6ff", bg: "rgba(88,166,255,0.12)" },
  SP: { text: "#FF8000", bg: "rgba(255,128,0,0.12)" },
  E: { text: "#bc8cff", bg: "rgba(188,140,255,0.12)" },
  SE: { text: "#ffd700", bg: "rgba(255,215,0,0.12)" },
};

function roleLabel(role) {
  if (role === "Numero1") return "Piloto principal";
  if (role === "Numero2") return "Segundo piloto";
  return "Convocado";
}

function countTeamVacancies(team) {
  let total = 0;
  if (!team.piloto_1_nome) total += 1;
  if (!team.piloto_2_nome) total += 1;
  return total;
}

function categorySortValue(categoryId) {
  const index = CATEGORY_ORDER.indexOf(categoryId);
  return index === -1 ? 999 : index;
}

function classAccentColor(className) {
  return CLASS_COLORS[className] ?? "#8b949e";
}

function sortByConfiguredOrder(items, getValue, preferredOrder = []) {
  return [...items].sort((left, right) => {
    const leftValue = getValue(left);
    const rightValue = getValue(right);
    const leftIndex = preferredOrder.indexOf(leftValue);
    const rightIndex = preferredOrder.indexOf(rightValue);
    const normalizedLeft = leftIndex === -1 ? 999 : leftIndex;
    const normalizedRight = rightIndex === -1 ? 999 : rightIndex;

    if (normalizedLeft !== normalizedRight) {
      return normalizedLeft - normalizedRight;
    }

    return String(leftValue ?? "").localeCompare(String(rightValue ?? ""));
  });
}

function normalizeCategorySections(entries = []) {
  const grouped = new Map();

  for (const team of entries) {
    const category = team.categoria ?? team._categoria ?? "especial";
    if (!grouped.has(category)) {
      grouped.set(category, []);
    }
    grouped.get(category).push(team);
  }

  return [...grouped.entries()]
    .sort(([left], [right]) => categorySortValue(left) - categorySortValue(right))
    .map(([category, teams]) => ({
      category,
      label: CATEGORY_LABELS[category] ?? category,
      color: CATEGORY_COLORS[category] ?? "#58a6ff",
      teams: [...teams].sort((left, right) => {
        const classDiff = (left.classe ?? "").localeCompare(right.classe ?? "");
        if (classDiff !== 0) return classDiff;
        return (left.nome ?? "").localeCompare(right.nome ?? "");
      }),
    }));
}

function buildClassGroups(teams = [], category) {
  const grouped = new Map();

  for (const team of teams) {
    const className = team.classe ?? "geral";
    if (!grouped.has(className)) {
      grouped.set(className, []);
    }
    grouped.get(className).push(team);
  }

  return sortByConfiguredOrder(
    [...grouped.entries()],
    ([className]) => className,
    TEAM_CLASS_ORDER[category] ?? [],
  )
    .map(([className, classTeams]) => ({
      className,
      teams: classTeams,
    }));
}

function filterEligibleCandidates(candidates = [], selectedCategory = "all") {
  if (selectedCategory === "production_challenger") {
    return candidates.filter((candidate) => candidate.production_eligible);
  }
  if (selectedCategory === "endurance") {
    return candidates.filter((candidate) => candidate.endurance_eligible);
  }
  return candidates.filter(
    (candidate) => candidate.production_eligible || candidate.endurance_eligible,
  );
}

function buildCandidateGroups(candidates = []) {
  const grouped = new Map();

  for (const candidate of candidates) {
    const category = candidate.origin_category || "sem_categoria";
    if (!grouped.has(category)) {
      grouped.set(category, []);
    }
    grouped.get(category).push(candidate);
  }

  return [...grouped.entries()]
    .sort(([left], [right]) => {
      const leftIndex = CANDIDATE_GROUP_ORDER.indexOf(left);
      const rightIndex = CANDIDATE_GROUP_ORDER.indexOf(right);
      const normalizedLeft = leftIndex === -1 ? 999 : leftIndex;
      const normalizedRight = rightIndex === -1 ? 999 : rightIndex;
      if (normalizedLeft !== normalizedRight) {
        return normalizedLeft - normalizedRight;
      }
      return left.localeCompare(right);
    })
    .map(([category, entries]) => ({
      category,
      label: CANDIDATE_GROUP_LABELS[category] ?? category,
      color: CANDIDATE_GROUP_COLORS[category] ?? "rgba(255,255,255,0.35)",
      entries,
    }));
}

function buildDailyLogGroups(entries = []) {
  const grouped = new Map();

  for (const entry of entries) {
    const specialCategory = entry.special_category || "especial";
    const className = entry.class_name || "geral";
    const key = `${specialCategory}:${className}`;
    if (!grouped.has(key)) {
      grouped.set(key, {
        key,
        specialCategory,
        className,
        label: `${CATEGORY_LABELS[specialCategory] ?? specialCategory} - ${className.toUpperCase()}`,
        color: classAccentColor(className),
        entries: [],
      });
    }
    grouped.get(key).entries.push(entry);
  }

  return [...grouped.values()].sort((left, right) => {
    const categoryDiff =
      categorySortValue(left.specialCategory) - categorySortValue(right.specialCategory);
    if (categoryDiff !== 0) return categoryDiff;

    const leftIndex = DAILY_LOG_CLASS_ORDER.indexOf(left.className);
    const rightIndex = DAILY_LOG_CLASS_ORDER.indexOf(right.className);
    const normalizedLeft = leftIndex === -1 ? 999 : leftIndex;
    const normalizedRight = rightIndex === -1 ? 999 : rightIndex;
    if (normalizedLeft !== normalizedRight) return normalizedLeft - normalizedRight;
    return left.className.localeCompare(right.className);
  });
}

function formatSafeChampionshipPosition(position, totalDrivers) {
  if (!position) {
    return null;
  }
  return `${position}º`;
}

function formatChampionshipPosition(position, totalDrivers) {
  if (!position) {
    return null;
  }
  return `${position}\u00ba`;
}

function TeamPilotRow({ name, fallback, isNew = false, accentColor = "rgba(88,166,255,0.9)" }) {
  const empty = !name;

  return (
    <div className="flex items-center gap-3 py-2 first:pt-0 last:pb-0">
      <div
        className="h-2 w-2 shrink-0 rounded-full"
        style={{ background: empty ? "rgba(255,255,255,0.16)" : accentColor }}
      />
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <p
          className={`min-w-0 flex-1 truncate text-[11px] leading-[1.2] ${empty ? "italic text-[color:var(--text-muted)]" : "font-semibold text-[color:var(--text-primary)]"}`}
        >
          {name || fallback}
        </p>
        {!empty && isNew && (
          <span className="rounded-full border border-[#58a6ff55] bg-[#58a6ff1a] px-2 py-0.5 text-[9px] font-black uppercase tracking-[0.18em] text-[#8cc8ff]">
            NEW
          </span>
        )}
      </div>
    </div>
  );
}

function DailyLogMovement({ entry, color }) {
  const isStructured =
    entry.driver_name && entry.team_name && (entry.event_type === "convocado" || entry.event_type === "player_selected");

  if (!isStructured) {
    return (
      <p className="rounded-lg border border-white/8 bg-white/[0.03] px-3 py-2 text-body text-[color:var(--text-secondary)]">
        {entry.message}
      </p>
    );
  }

  return (
    <article
      className="rounded-lg border px-2.5 py-2"
      style={{
        borderColor: `${color}26`,
        background: `linear-gradient(135deg, ${color}0f 0%, rgba(255,255,255,0.02) 100%)`,
      }}
    >
      <div className="flex min-w-0 items-center gap-2.5">
        <span
          className="w-8 shrink-0 text-right text-[13px] font-black leading-none"
          style={{ color }}
        >
          {formatChampionshipPosition(entry.championship_position, entry.championship_total_drivers) ?? "--"}
        </span>
        <p className="min-w-0 flex-1 truncate text-[13px] font-extrabold leading-[1.05] text-[color:var(--text-primary)]">
          {entry.driver_name}
        </p>
      </div>
    </article>
  );
}

export default function ConvocationView() {
  const careerId = useCareerStore((state) => state.careerId);
  const season = useCareerStore((state) => state.season);
  const specialWindowState = useCareerStore((state) => state.specialWindowState);
  const playerSpecialOffers = useCareerStore((state) => state.playerSpecialOffers);
  const acceptedSpecialOffer = useCareerStore((state) => state.acceptedSpecialOffer);
  const isConvocating = useCareerStore((state) => state.isConvocating);
  const error = useCareerStore((state) => state.error);
  const loadSpecialWindowState = useCareerStore((state) => state.loadSpecialWindowState);
  const acceptSpecialOfferForDay = useCareerStore((state) => state.acceptSpecialOfferForDay);
  const advanceSpecialWindowDay = useCareerStore((state) => state.advanceSpecialWindowDay);
  const confirmSpecialBlock = useCareerStore((state) => state.confirmSpecialBlock);

  const [selectedCategory, setSelectedCategory] = useState("all");

  useEffect(() => {
    if (!careerId || specialWindowState) {
      return;
    }
    void loadSpecialWindowState();
  }, [careerId, loadSpecialWindowState, specialWindowState]);

  const groupedOffers = useMemo(() => {
    const grouped = new Map();

    for (const offer of playerSpecialOffers) {
      const category = offer.special_category ?? "especial";
      if (!grouped.has(category)) {
        grouped.set(category, []);
      }
      grouped.get(category).push(offer);
    }

    return [...grouped.entries()].sort(
      ([left], [right]) => categorySortValue(left) - categorySortValue(right),
    );
  }, [playerSpecialOffers]);

  const filteredSections = useMemo(() => {
    const teamSections = (specialWindowState?.team_sections ?? []).map((section) => ({
      ...section,
      label: section.label ?? CATEGORY_LABELS[section.category] ?? section.category,
      color: CATEGORY_COLORS[section.category] ?? "#58a6ff",
    }));
    if (selectedCategory === "all") {
      return teamSections;
    }
    return teamSections.filter((section) => section.category === selectedCategory);
  }, [selectedCategory, specialWindowState]);

  const candidateGroups = useMemo(() => {
    const candidates = filterEligibleCandidates(
      specialWindowState?.eligible_candidates ?? [],
      selectedCategory,
    );
    return buildCandidateGroups(candidates);
  }, [selectedCategory, specialWindowState]);

  const dailyLogGroups = useMemo(
    () => buildDailyLogGroups(specialWindowState?.last_day_log ?? []),
    [specialWindowState],
  );

  const totalVisibleTeams = filteredSections.reduce((sum, section) => sum + section.teams.length, 0);
  const currentDay = specialWindowState?.current_day ?? 1;
  const totalDays = specialWindowState?.total_days ?? 7;
  const primaryCtaLabel = specialWindowState?.is_finished
    ? acceptedSpecialOffer
      ? "Entrar no bloco especial"
      : "Seguir sem entrar no especial"
    : "Avançar dia";

  return (
    <div
      data-testid="convocation-page"
      className="app-shell relative h-screen w-full overflow-hidden text-[color:var(--text-primary)]"
    >
      <div className="app-backdrop pointer-events-none absolute inset-0" />

      <LoadingOverlay
        open={isConvocating}
        title="Processando convocação"
        message="Atualizando ofertas do jogador e preparando o bloco especial."
      />

      <div className="relative z-10 mx-auto flex h-full max-w-[1680px] flex-col px-3 pb-3 pt-3 sm:px-4 lg:px-5 xl:px-6">
        <header className="glass-strong animate-fade-in mb-3 rounded-2xl px-5 py-2 lg:px-6">
          <div className="grid items-start gap-3 lg:grid-cols-[1fr_auto]">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <p className="text-body-sm font-bold uppercase tracking-[0.28em] text-[color:var(--accent-primary)]">
                  Janela especial
                </p>
                {acceptedSpecialOffer && specialWindowState?.is_finished && (
                  <span className="glass-light rounded-full px-2.5 py-1 text-body-sm font-bold tracking-[0.14em] text-[color:var(--accent-primary)]">
                    Convocação aceita
                  </span>
                )}
              </div>
              <h1 className="mt-1 text-[20px] font-bold leading-[1.05] tracking-[-0.02em] text-[color:var(--text-primary)] lg:text-[26px]">
                Mercado de Convocações
              </h1>

              <div className="mt-2 max-w-full overflow-x-auto">
                <div className="glass inline-flex w-fit items-center gap-0.5 whitespace-nowrap rounded-full p-1">
                  {CATEGORY_FILTERS.map((category) => {
                    const active = selectedCategory === category.id;
                    return (
                      <button
                        key={category.id}
                        onClick={() => setSelectedCategory(category.id)}
                        className={`transition-glass cursor-pointer rounded-full border px-2.5 py-1 text-body-sm font-semibold ${
                          active
                            ? "border-white/30 bg-white/14 text-[color:var(--accent-primary)]"
                            : "border-transparent bg-white/3 text-[color:var(--text-secondary)] hover:bg-white/8 hover:text-[color:var(--text-primary)]"
                        }`}
                      >
                        <span
                          className="mr-2 inline-block h-1.5 w-1.5 rounded-full"
                          style={{ backgroundColor: category.color }}
                        />
                        {category.label}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>

            <div className="flex items-center gap-3 self-center lg:justify-self-end">
              <span className="rounded-full border border-[#58a6ff66] bg-[#58a6ff1a] px-2.5 py-1 text-body-sm font-bold uppercase tracking-[0.14em] text-[color:var(--accent-primary)]">
                Bloco especial
              </span>

              <div className="w-[220px] px-1 lg:w-[280px]">
                <div className="mb-1 flex items-center justify-between gap-2">
                  <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
                    Dia{" "}
                    <span className="text-[color:var(--text-primary)]">
                      {currentDay}/{totalDays}
                    </span>
                  </p>
                  <p className="text-body-sm text-[color:var(--text-secondary)]">
                    {specialWindowState?.status ?? season?.fase ?? "Janela de Convocação"}
                  </p>
                </div>
                <div className="h-[3px] w-full rounded-full bg-[#2a3240]">
                  <div
                    className="h-full rounded-full bg-[color:var(--accent-primary)]"
                    style={{ width: `${Math.max(14, Math.round((currentDay / totalDays) * 100))}%` }}
                  />
                </div>
              </div>

              <GlassButton
                variant="primary"
                disabled={isConvocating}
                className="rounded-full px-6 py-2.5 text-body-lg font-bold uppercase tracking-[0.16em]"
                onClick={() =>
                  void (specialWindowState?.is_finished
                    ? confirmSpecialBlock()
                    : advanceSpecialWindowDay())
                }
              >
                {primaryCtaLabel}
              </GlassButton>
            </div>
          </div>

          {error && (
            <p className="mt-2 text-center text-body-sm text-[color:var(--status-red)]">{error}</p>
          )}
        </header>

        <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 xl:grid-cols-[20%_62%_18%]">
          <aside className="glass-strong scroll-area animate-edge-rail-in min-h-0 overflow-y-auto rounded-2xl px-3 py-4 lg:px-4 lg:py-5">
            <div className="mb-5">
              <div className="mb-3 flex h-6 items-center justify-between">
                <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
                  Pilotos elegíveis
                </p>
              </div>

              {candidateGroups.length === 0 ? (
                <div className="py-8 text-center text-body text-[color:var(--text-muted)]">
                  Nenhum piloto elegivel restante neste filtro.
                </div>
              ) : (
                <div className="space-y-4">
                  {candidateGroups.map((group) => (
                    <section key={group.category}>
                      <div className="mb-2 flex items-center gap-2">
                        <span
                          className="text-[9px] font-bold uppercase tracking-[0.2em]"
                          style={{ color: group.color }}
                        >
                          {group.label}
                        </span>
                        <div
                          className="h-px flex-1"
                          style={{
                            background: `linear-gradient(to right, ${group.color}66, transparent)`,
                          }}
                        />
                      </div>

                      <div className="space-y-2">
                        {group.entries.map((candidate) => {
                          const licenseColors =
                            LICENSE_COLORS[candidate.license_sigla] ?? LICENSE_COLORS.R;

                          return (
                          <article
                            key={candidate.driver_id}
                            className="glass-light rounded-xl border px-3 py-3"
                            style={{
                              borderColor: `${group.color}30`,
                              background: `linear-gradient(180deg, ${group.color}12 0%, rgba(255,255,255,0.03) 100%)`,
                            }}
                          >
                            <div className="flex items-center gap-3">
                              <span
                                className="shrink-0 text-[13px] font-black tracking-[-0.02em]"
                                style={{
                                  color: group.color,
                                }}
                              >
                                {formatChampionshipPosition(
                                  candidate.championship_position,
                                  candidate.championship_total_drivers,
                                ) ?? "—"}
                              </span>
                              <p className="min-w-0 flex-1 truncate text-[15px] font-extrabold leading-[1.05] text-[color:var(--text-primary)]">
                                {candidate.driver_name}
                              </p>
                              <span
                                className="shrink-0 rounded-md px-2 py-1 text-[11px] font-bold uppercase tracking-[0.08em]"
                                style={{
                                  background: licenseColors.bg,
                                  color: licenseColors.text,
                                }}
                                title={candidate.license_nivel}
                              >
                                {candidate.license_sigla}
                              </span>
                            </div>
                          </article>
                          );
                        })}
                      </div>
                    </section>
                  ))}
                </div>
              )}
            </div>

            <div className="border-t border-white/8 pt-4">
              <div className="mb-4 flex h-6 items-center justify-between">
                <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
                  Suas propostas
                </p>
                <span className="text-body-sm text-[color:var(--text-muted)]">
                  {playerSpecialOffers.length}
                </span>
              </div>

              {groupedOffers.length === 0 ? (
                <div className="py-6 text-center text-body text-[color:var(--text-muted)]">
                  Nenhuma proposta visivel neste dia.
                </div>
              ) : (
                <div className="space-y-5">
                  {groupedOffers.map(([category, offers]) => {
                    const color = CATEGORY_COLORS[category] ?? "#58a6ff";
                    return (
                      <section key={category}>
                        <div className="mb-2 flex items-center gap-2">
                          <span
                            className="text-[9px] font-bold uppercase tracking-[0.2em]"
                            style={{ color }}
                          >
                            {CATEGORY_LABELS[category] ?? category}
                          </span>
                          <div
                            className="h-px flex-1"
                            style={{ background: `linear-gradient(to right, ${color}55, transparent)` }}
                          />
                        </div>

                        <div className="space-y-2">
                          {offers.map((offer) => (
                            <article
                              key={offer.id}
                              className="glass-light rounded-xl border border-white/8 px-3 py-3"
                            >
                              <div className="flex items-center justify-between gap-3">
                                <p className="text-body font-bold text-[color:var(--text-primary)]">
                                  {offer.team_name}
                                </p>
                                <span className="text-[10px] font-bold uppercase tracking-[0.08em] text-[color:var(--text-muted)]">
                                  dia {offer.available_from_day}
                                </span>
                              </div>
                              <p className="mt-1 text-body-sm text-[color:var(--text-secondary)]">
                                Classe {offer.class_name.toUpperCase()}
                              </p>
                              <p className="text-body-sm text-[color:var(--text-secondary)]">
                                {roleLabel(offer.papel)}
                              </p>
                              <p className="mt-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-[color:var(--text-muted)]">
                                {offer.status}
                              </p>

                              <div className="mt-3">
                                <GlassButton
                                  variant="primary"
                                  disabled={isConvocating || !offer.is_available_today || specialWindowState?.is_finished}
                                  className="min-h-9 w-full rounded-lg px-3 py-2 text-[11px] font-bold tracking-[0.08em]"
                                  onClick={() => void acceptSpecialOfferForDay(offer.id)}
                                >
                                  Escolher hoje
                                </GlassButton>
                              </div>
                            </article>
                          ))}
                        </div>
                      </section>
                    );
                  })}
                </div>
              )}
            </div>
          </aside>

          <main className="glass scroll-area animate-fade-in min-h-0 overflow-y-auto rounded-2xl px-5 py-4 lg:px-6 lg:py-5">
            <div className="mb-5 flex h-6 items-center justify-between">
              <p className="text-body-sm font-bold uppercase tracking-[0.2em] text-[color:var(--text-secondary)]">
                Mapeamento das equipes
              </p>
              <p className="text-body text-[color:var(--text-muted)]">
                {totalVisibleTeams} equipe{totalVisibleTeams === 1 ? "" : "s"}
              </p>
            </div>

            {filteredSections.length === 0 ? (
              <div className="py-20 text-center text-body text-[color:var(--text-muted)]">
                Nenhuma equipe encontrada.
              </div>
            ) : (
              <div className="space-y-3">
                {filteredSections.map((section, index) => (
                  <section key={section.category} className={index > 0 ? "mt-10" : ""}>
                    <div
                      data-testid={`convocation-category-header-${section.category}`}
                      className="mb-5 flex flex-col items-center justify-center gap-3 rounded-xl px-4 py-6 text-center"
                      style={{
                        background: `linear-gradient(135deg, ${section.color}22 0%, ${section.color}0a 100%)`,
                        borderLeft: `3px solid ${section.color}`,
                        boxShadow: `0 0 18px ${section.color}18`,
                      }}
                    >
                      {CATEGORY_LOGOS[section.category] ? (
                        <img
                          src={CATEGORY_LOGOS[section.category]}
                          alt={section.label}
                          className="h-48 w-auto max-w-full object-contain drop-shadow-[0_18px_36px_rgba(0,0,0,0.38)] lg:h-52"
                          draggable={false}
                        />
                      ) : (
                        <span
                          className="text-[17px] font-bold uppercase tracking-[0.18em]"
                          style={{ color: section.color }}
                        >
                          {section.label}
                        </span>
                      )}
                      <span
                        data-testid="convocation-category-count"
                        className="shrink-0 rounded-full border px-3 py-1 text-[11px] font-bold uppercase tracking-[0.12em]"
                        style={{
                          color: section.color,
                          borderColor: `${section.color}55`,
                          backgroundColor: `${section.color}14`,
                        }}
                      >
                        {section.teams.length} equipes
                      </span>
                    </div>

                    <div className="space-y-8">
                      {buildClassGroups(section.teams, section.category).map((classGroup) => (
                        <div key={`${section.category}:${classGroup.className}`} className="space-y-3">
                          <SeasonSectionHeader
                            title={classGroup.className.toUpperCase()}
                            color={classAccentColor(classGroup.className)}
                            detail={`${classGroup.teams.length} equipe${classGroup.teams.length > 1 ? "s" : ""}`}
                            titleTestId="convocation-class-title"
                          />

                          <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                            {classGroup.teams.map((team) => {
                              const vacancies = countTeamVacancies(team);
                              const accentColor = classAccentColor(classGroup.className);
                              const isOddCount = classGroup.teams.length % 2 === 1;
                              const isLastOddCard =
                                isOddCount && classGroup.teams[classGroup.teams.length - 1]?.id === team.id;

                              return (
                                <article
                                  key={team.id}
                                  className={`glass transition-glass relative overflow-hidden rounded-xl border p-3 hover:-translate-y-0.5 hover:scale-[1.01] ${
                                    isLastOddCard ? "lg:col-span-2 lg:mx-auto lg:w-[calc(50%-0.375rem)]" : ""
                                  }`}
                                  style={{ borderColor: "rgba(255,255,255,0.11)" }}
                                >
                                  <div className="relative mb-3 flex items-start gap-3">
                                    <div className="flex min-w-0 flex-1 items-center gap-3">
                                      <TeamLogoMark
                                        teamName={team.nome}
                                        color={accentColor}
                                        size="md"
                                        testId="convocation-team-logo"
                                      />
                                      <div className="min-w-0 flex-1">
                                        <p className="truncate text-[19px] font-bold leading-[1.05]">
                                          {team.nome}
                                        </p>
                                      </div>
                                      {vacancies > 0 && (
                                        <span className="ml-auto shrink-0 rounded-md border border-white/12 bg-white/5 px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em] text-[color:var(--text-secondary)]">
                                          {vacancies} vaga{vacancies > 1 ? "s" : ""}
                                        </span>
                                      )}
                                    </div>
                                  </div>

                                  <div className="relative divide-y divide-white/8">
                                    <TeamPilotRow
                                      name={team.piloto_1_nome}
                                      fallback="Piloto 1 em aberto"
                                      isNew={team.piloto_1_new_badge_day === currentDay}
                                      accentColor={accentColor}
                                    />
                                    <TeamPilotRow
                                      name={team.piloto_2_nome}
                                      fallback="Piloto 2 em aberto"
                                      isNew={team.piloto_2_new_badge_day === currentDay}
                                      accentColor={accentColor}
                                    />
                                  </div>
                                </article>
                              );
                            })}
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                ))}
              </div>
            )}
          </main>

          <aside className="glass scroll-area animate-drawer-in self-start overflow-y-auto rounded-2xl px-4 py-4 lg:px-5 lg:py-5 xl:max-h-[calc(100vh-96px)]">
            <div className="mb-4 flex h-6 items-center gap-2">
              <span className="relative inline-flex h-2.5 w-2.5">
                {acceptedSpecialOffer && (
                  <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#58a6ff]/80" />
                )}
                <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-[color:var(--accent-primary)]" />
              </span>
              <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
                Sua decisao
              </p>
            </div>

            {acceptedSpecialOffer ? (
              <div className="glass-light rounded-xl border px-4 py-4">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--accent-primary)]">
                  Convocação em destaque
                </p>
                <p className="mt-2 text-[19px] font-bold text-[color:var(--text-primary)]">
                  {acceptedSpecialOffer.team_name}
                </p>
                <p className="mt-1 text-body text-[color:var(--text-secondary)]">
                  {CATEGORY_LABELS[acceptedSpecialOffer.special_category] ??
                    acceptedSpecialOffer.special_category}
                </p>
                <p className="mt-1 text-body-sm text-[color:var(--text-muted)]">
                  {acceptedSpecialOffer.class_name.toUpperCase()} |{" "}
                  {roleLabel(acceptedSpecialOffer.papel)}
                </p>
              </div>
            ) : (
              <div className="glass-light rounded-xl border-dashed p-4 text-body text-[color:var(--text-secondary)]">
                Nenhuma vaga especial aceita ate agora.
              </div>
            )}

            <div
              data-testid="daily-log-market"
              className="mt-4 rounded-xl border border-white/8 bg-black/18 px-4 py-4"
            >
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--text-muted)]">
                Fechamento do dia
              </p>
              {specialWindowState?.last_day_log?.length ? (
                <div className="mt-3 space-y-3">
                  {dailyLogGroups.map((group) => (
                    <section key={group.key} className="space-y-2">
                      <div
                        className="flex items-center justify-center rounded-lg border px-3 py-2"
                        style={{
                          borderColor: `${group.color}30`,
                          background: `linear-gradient(135deg, ${group.color}16 0%, rgba(255,255,255,0.025) 100%)`,
                        }}
                      >
                        <p
                          className="text-center text-[11px] font-black uppercase tracking-[0.16em]"
                          style={{ color: group.color }}
                        >
                          {group.label}
                        </p>
                      </div>
                      <div className="space-y-2">
                        {group.entries.map((entry, index) => (
                          <DailyLogMovement
                            key={`${entry.day}-${entry.event_type}-${entry.team_id ?? "team"}-${entry.driver_id ?? index}`}
                            entry={entry}
                            color={group.color}
                          />
                        ))}
                      </div>
                    </section>
                  ))}
                </div>
              ) : (
                <p className="mt-2 text-body text-[color:var(--text-secondary)]">
                  As movimentacoes do mercado vao aparecer aqui ao final de cada dia.
                </p>
              )}
            </div>

            <div className="mt-4 rounded-xl border border-white/8 bg-black/18 px-4 py-4">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--text-muted)]">
                Proximo passo
              </p>
              <p className="mt-2 text-body text-[color:var(--text-secondary)]">
                {specialWindowState?.is_finished
                  ? "A janela terminou. Agora voc? pode confirmar sua entrada no bloco especial ou seguir sem participar."
                  : "Escolha no maximo uma proposta para o dia e use o botao principal do topo para avancar o mercado."}
              </p>
            </div>
          </aside>
        </div>
      </div>
    </div>
  );
}
