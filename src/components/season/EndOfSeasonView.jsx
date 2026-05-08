import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";

import useCareerStore from "../../stores/useCareerStore";
import {
  extractFlag,
  extractNationalityLabel,
  formatAttributeName,
  formatCategoryName,
  formatLicenseLevel,
  getCategoryTier,
} from "../../utils/formatters";
import GlassButton from "../ui/GlassButton";
import LoadingOverlay from "../ui/LoadingOverlay";
import TeamLogoMark from "../team/TeamLogoMark";

const PILOT_STATUS_META = {
  promoted: {
    badge: "Promovido",
    badgeClassName: "text-status-green bg-status-green/10 border border-status-green/20",
    borderClassName: "border-status-green/20 bg-status-green/5",
    accentClassName: "text-status-green",
    icon: "↑",
    sectionTitle: "Promovidos",
  },
  relegated: {
    badge: "Rebaixado",
    badgeClassName: "text-status-red bg-status-red/10 border border-status-red/20",
    borderClassName: "border-status-red/20 bg-status-red/5",
    accentClassName: "text-status-red",
    icon: "↓",
    sectionTitle: "Rebaixados",
  },
  unchanged: {
    badge: "Não alterou",
    badgeClassName: "text-text-secondary bg-white/5 border border-white/10",
    borderClassName: "border-white/10 bg-white/[0.03]",
    accentClassName: "text-text-secondary",
    icon: "•",
    sectionTitle: "Não alterou",
  },
};

const LICENSE_ORDER = [4, 3, 2, 1, 0];
const LICENSE_GROUP_ORDER = ["promoted", "relegated", "unchanged"];
const CATEGORY_TO_LICENSE_LEVEL = {
  mazda_rookie: 0,
  toyota_rookie: 0,
  mazda_amador: 1,
  toyota_amador: 1,
  bmw_m2: 2,
  production_challenger: 2,
  gt4: 3,
  gt3: 4,
  lmp2: 4,
  endurance: 4,
};

function normalizeLicenseLevel(level) {
  if (typeof level !== "number" || Number.isNaN(level)) return 0;
  return Math.max(0, Math.min(4, level));
}

function licenseLevelFromCategory(category) {
  return normalizeLicenseLevel(CATEGORY_TO_LICENSE_LEVEL[category] ?? 0);
}

function formatRetainedLicenseLabel(category, licenseLevel) {
  if (category === "mazda_rookie" || category === "toyota_rookie") {
    return "Rookie";
  }

  return formatLicenseLevel(licenseLevel);
}

function buildPilotSummary(entry) {
  if (entry.status === "promoted") {
    return `Conquistou Licença ${formatLicenseLevel(entry.licenseLevel)} em ${formatCategoryName(entry.category || "Sem Categoria")}.`;
  }

  if (entry.status === "relegated") {
    return entry.reason || "Perdeu a licença da categoria ao fim da temporada.";
  }

  return `Manteve a Licença ${formatRetainedLicenseLabel(entry.category, entry.licenseLevel)} para a próxima temporada.`;
}

function buildPilotEntries({ reports, licenses, promotionEffects, movements, driverCategoriesById }) {
  const reportById = new Map((reports || []).map((report) => [report.driver_id, report]));
  const licenseById = new Map((licenses || []).map((license) => [license.driver_id, license]));
  const relegationById = new Map(
    (promotionEffects || [])
      .filter((effect) => effect.effect === "FreedNoLicense")
      .map((effect) => [effect.driver_id, effect]),
  );
  const movementByTeamId = new Map((movements || []).map((movement) => [movement.team_id, movement]));

  return Array.from(
    new Set([
      ...reportById.keys(),
      ...licenseById.keys(),
      ...relegationById.keys(),
    ]),
  )
    .map((driverId) => {
      const report = reportById.get(driverId);
      const license = licenseById.get(driverId);
      const relegation = relegationById.get(driverId);
      const driverCategory = driverCategoriesById[driverId];
      const movement = relegation ? movementByTeamId.get(relegation.team_id) : null;
      const fallbackCategory = license?.category || driverCategory || movement?.from_category || null;

      let status = "unchanged";
      let licenseLevel = licenseLevelFromCategory(fallbackCategory);

      if (license) {
        status = "promoted";
        licenseLevel = normalizeLicenseLevel(license.license_level);
      } else if (relegation) {
        status = "relegated";
        licenseLevel = licenseLevelFromCategory(fallbackCategory);
      }

      return {
        driverId,
        driverName: report?.driver_name || license?.driver_name || relegation?.driver_name || "Piloto sem nome",
        status,
        licenseLevel,
        category: fallbackCategory,
        summary: buildPilotSummary({
          status,
          licenseLevel,
          category: fallbackCategory,
          reason: relegation?.reason,
        }),
        changes: report?.changes || [],
        overallDelta: report?.overall_delta || 0,
      };
    })
    .sort((a, b) => a.driverName.localeCompare(b.driverName, "pt-BR"));
}

function AccordionPilotRow({ entry, expanded, onClick }) {
  const statusMeta = PILOT_STATUS_META[entry.status];

  return (
    <div className={`rounded-2xl border overflow-hidden ${statusMeta.borderClassName}`}>
      <button
        type="button"
        className="w-full p-4 text-left flex items-center justify-between gap-4 hover:bg-white/[0.03] transition"
        onClick={onClick}
      >
        <div className="min-w-0">
          <div className="flex items-center gap-3 min-w-0">
            <span className={`${statusMeta.accentClassName} text-lg font-bold`} aria-hidden="true">
              {statusMeta.icon}
            </span>
            <p className="text-lg font-bold leading-tight text-text-primary truncate">{entry.driverName}</p>
          </div>
          <p className="text-xs text-text-secondary mt-2 leading-relaxed">{entry.summary}</p>
        </div>

        <div className="flex items-center gap-3 shrink-0">
          <span className={`text-[10px] font-bold uppercase tracking-widest px-2.5 py-1 rounded-full ${statusMeta.badgeClassName}`}>
            {statusMeta.badge}
          </span>
          <span className={`text-white/50 text-sm transition-transform ${expanded ? "rotate-180" : ""}`} aria-hidden="true">
            ▼
          </span>
        </div>
      </button>

      {expanded && (
        <div className="p-5 border-t border-white/5 bg-black/20">
          <p className="text-xs text-text-secondary uppercase tracking-widest mb-3 font-semibold">
            Evolução de atributos
          </p>

          {entry.changes.length > 0 ? (
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              {entry.changes.map((change, index) => {
                const isPositive = change.delta >= 0;
                const valueClassName = isPositive ? "text-status-green" : "text-status-red";

                return (
                  <div
                    key={`${entry.driverId}-${change.attribute}-${index}`}
                    className="rounded p-3 text-center border border-white/10 bg-white/[0.03]"
                  >
                    <p className="text-xs text-text-secondary mb-1">{formatAttributeName(change.attribute)}</p>
                    <p className={`text-lg font-bold ${valueClassName}`}>
                      {change.delta > 0 ? `+${change.delta}` : change.delta}
                    </p>
                  </div>
                );
              })}
            </div>
          ) : (
            <p className="text-sm text-text-secondary">Sem alterações de atributos registradas para este piloto.</p>
          )}
        </div>
      )}
    </div>
  );
}

function SectionLicenses({ careerId, reports, licenses, promotionEffects, movements }) {
  const [openId, setOpenId] = useState(null);
  const [openLicenseLevels, setOpenLicenseLevels] = useState([]);
  const [driverCategoriesById, setDriverCategoriesById] = useState({});

  const relevantDriverIds = useMemo(() => {
    return Array.from(
      new Set([
        ...(reports || []).map((report) => report.driver_id),
        ...(licenses || []).map((license) => license.driver_id),
        ...((promotionEffects || [])
          .filter((effect) => effect.effect === "FreedNoLicense")
          .map((effect) => effect.driver_id)),
      ]),
    );
  }, [licenses, promotionEffects, reports]);

  useEffect(() => {
    let isMounted = true;

    async function loadDriverCategories() {
      if (!careerId || relevantDriverIds.length === 0) {
        if (isMounted) setDriverCategoriesById({});
        return;
      }

      const results = await Promise.allSettled(
        relevantDriverIds.map(async (driverId) => {
          try {
            const detail = await invoke("get_driver_detail", { careerId, driverId });
            return [driverId, detail?.trajetoria?.categoria_atual || null];
          } catch {
            return [driverId, null];
          }
        }),
      );

      if (!isMounted) return;

      const nextCategories = {};
      results.forEach((result) => {
        if (result.status !== "fulfilled") return;
        const [driverId, category] = result.value;
        nextCategories[driverId] = category;
      });

      setDriverCategoriesById(nextCategories);
    }

    void loadDriverCategories();

    return () => {
      isMounted = false;
    };
  }, [careerId, relevantDriverIds]);

  const pilotEntries = useMemo(() => {
    return buildPilotEntries({
      reports,
      licenses,
      promotionEffects,
      movements,
      driverCategoriesById,
    });
  }, [driverCategoriesById, licenses, movements, promotionEffects, reports]);

  const groupedByLicense = useMemo(() => {
    return LICENSE_ORDER.map((licenseLevel) => ({
      licenseLevel,
      entries: pilotEntries.filter((entry) => entry.licenseLevel === licenseLevel),
    })).filter((group) => group.entries.length > 0);
  }, [pilotEntries]);

  function toggleLicenseGroup(licenseLevel) {
    const groupEntries = groupedByLicense.find((group) => group.licenseLevel === licenseLevel)?.entries || [];

    setOpenLicenseLevels((currentLevels) => (
      currentLevels.includes(licenseLevel)
        ? currentLevels.filter((level) => level !== licenseLevel)
        : [...currentLevels, licenseLevel]
    ));

    if (groupEntries.some((entry) => entry.driverId === openId)) {
      setOpenId(null);
    }
  }

  return (
    <div className="max-w-4xl space-y-10 animate-fade-in pb-10">
      <div>
        <h2 className="text-3xl font-bold mb-2">Licenças de Pilotos</h2>
        <p className="text-text-secondary">
          Panorama completo das licenças para a próxima temporada, com promoções, rebaixamentos e pilotos que mantiveram seu nível.
        </p>
      </div>

      {pilotEntries.length === 0 ? (
        <p className="text-sm text-text-secondary">Nenhuma alteração de licença registrada este ano.</p>
      ) : (
        <div className="space-y-10">
          {groupedByLicense.map((group) => {
            const isOpen = openLicenseLevels.includes(group.licenseLevel);

            return (
              <section key={group.licenseLevel} className="rounded-2xl border border-white/10 bg-white/[0.02] overflow-hidden">
                <button
                  type="button"
                  className="w-full px-5 py-4 flex items-center justify-between gap-4 text-left hover:bg-white/[0.03] transition"
                  onClick={() => toggleLicenseGroup(group.licenseLevel)}
                >
                  <div className="flex items-center gap-4 min-w-0">
                    <span className={`text-white/50 text-sm transition-transform ${isOpen ? "rotate-180" : ""}`} aria-hidden="true">
                      ▼
                    </span>
                    <h3 className="text-lg font-bold text-text-primary">{formatLicenseLevel(group.licenseLevel)}</h3>
                  </div>
                  <span className="text-[11px] font-bold uppercase tracking-widest text-text-secondary shrink-0">
                    {group.entries.length} pilotos
                  </span>
                </button>

                {isOpen && (
                  <div className="space-y-5 p-5 pt-0 border-t border-white/10">
                    {LICENSE_GROUP_ORDER.map((status) => {
                      const entries = group.entries.filter((entry) => entry.status === status);
                      if (entries.length === 0) return null;

                      return (
                        <div key={`${group.licenseLevel}-${status}`} className="space-y-3">
                          <div className="flex items-center gap-3">
                            <h4 className="text-xs font-bold uppercase tracking-[0.2em] text-text-secondary">
                              {PILOT_STATUS_META[status].sectionTitle}
                            </h4>
                            <span className="text-[10px] font-bold px-2 py-1 rounded-full bg-white/5 border border-white/10 text-text-secondary">
                              {entries.length}
                            </span>
                          </div>

                          <div className="space-y-3">
                            {entries.map((entry) => (
                              <AccordionPilotRow
                                key={entry.driverId}
                                entry={entry}
                                expanded={openId === entry.driverId}
                                onClick={() => setOpenId(openId === entry.driverId ? null : entry.driverId)}
                              />
                            ))}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}

function SectionTeams({ movements }) {
  if (!movements || movements.length === 0) {
    return (
      <div className="max-w-4xl animate-fade-in">
        <h2 className="text-3xl font-bold mb-2">Movimentações de Equipes</h2>
        <p className="text-text-secondary mb-10">Tudo pacífico. Organizações mantiveram suas categorias no campeonato.</p>
      </div>
    );
  }

  const categories = useMemo(() => {
    const map = new Map();

    movements.forEach((movement) => {
      const category = movement.from_category || "Sem Categoria";
      if (!map.has(category)) map.set(category, { promotions: [], relegations: [] });

      if (movement.movement_type === "Promocao") {
        map.get(category).promotions.push(movement);
      } else {
        map.get(category).relegations.push(movement);
      }
    });

    return Array.from(map.entries()).sort((a, b) => getCategoryTier(b[0]) - getCategoryTier(a[0]));
  }, [movements]);

  return (
    <div className="max-w-4xl space-y-10 animate-fade-in pb-10">
      <div>
        <h2 className="text-3xl font-bold mb-2">Movimentações de Equipes</h2>
        <p className="text-text-secondary">Organizações promovidas ao nível superior e aquelas rebaixadas nas divisões, separadas por categoria.</p>
      </div>

      <div className="space-y-10">
        {categories.map(([category, { promotions, relegations }]) => (
          <div key={category}>
            <h3 className="text-lg font-bold text-text-primary mb-4 pb-2 border-b border-white/10 flex items-center gap-3">
              {formatCategoryName(category)}
            </h3>

            <div className="grid sm:grid-cols-2 gap-4">
              {promotions.map((movement, index) => (
                <div key={`${movement.team_id}-promo-${index}`} className="rounded-xl border bg-status-green/5 border-status-green/20 p-5 relative overflow-hidden">
                  <div className="absolute right-0 top-0 bottom-0 opacity-10 bg-gradient-to-l from-status-green w-1/3 pointer-events-none" />
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex min-w-0 items-center gap-3">
                      <TeamLogoMark
                        teamName={movement.team_name}
                        color="#238636"
                        size="md"
                        testId="end-season-team-logo"
                      />
                      <p className="min-w-0 truncate font-bold text-lg text-white">{movement.team_name}</p>
                    </div>
                    <span className="font-bold px-3 py-1 rounded-full text-xs text-status-green bg-status-green/10">
                      ↑ Promovida
                    </span>
                  </div>
                  <p className="text-xs text-text-secondary">{movement.reason}</p>
                  <p className="text-[10px] uppercase text-text-muted mt-3 font-semibold">
                    Moveu para {formatCategoryName(movement.to_category)}
                  </p>
                </div>
              ))}

              {relegations.map((movement, index) => (
                <div key={`${movement.team_id}-rel-${index}`} className="rounded-xl border bg-status-red/5 border-status-red/20 p-5 relative overflow-hidden">
                  <div className="absolute right-0 top-0 bottom-0 opacity-10 bg-gradient-to-l from-status-red w-1/3 pointer-events-none" />
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex min-w-0 items-center gap-3">
                      <TeamLogoMark
                        teamName={movement.team_name}
                        color="#da3633"
                        size="md"
                        testId="end-season-team-logo"
                      />
                      <p className="min-w-0 truncate font-bold text-lg text-white">{movement.team_name}</p>
                    </div>
                    <span className="font-bold px-3 py-1 rounded-full text-xs text-status-red bg-status-red/10">
                      ↓ Rebaixada
                    </span>
                  </div>
                  <p className="text-xs text-text-secondary">{movement.reason}</p>
                  <p className="text-[10px] uppercase text-text-muted mt-3 font-semibold">
                    Moveu para {formatCategoryName(movement.to_category)}
                  </p>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function SectionRookies({ rookies }) {
  if (!rookies || rookies.length === 0) {
    return (
      <div className="max-w-4xl animate-fade-in">
        <h2 className="text-3xl font-bold mb-2">Novos Talentos</h2>
        <p className="text-text-secondary">Nenhum talento vindo das academias confirmou idade esse ano.</p>
      </div>
    );
  }

  return (
    <div className="max-w-4xl space-y-10 animate-fade-in">
      <div>
        <h2 className="text-3xl font-bold mb-2">Novos Talentos Formados</h2>
        <p className="text-text-secondary">Eles acabaram de conseguir idade nas categorias de base, estando soltos no mercado de transferências como agentes livres.</p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {rookies.map((rookie) => {
          const isGenius = rookie.tipo === "Gênio";
          const isTalent = rookie.tipo === "Talento";
          const borderColor = isGenius ? "border-t-accent-primary" : (isTalent ? "border-t-white/30" : "border-t-white/10");
          const typeBadge = isGenius
            ? <div className="bg-accent-primary/10 text-accent-primary text-xs font-bold uppercase tracking-wider py-1.5 rounded-lg border border-accent-primary/20">Gênio</div>
            : isTalent
              ? <div className="bg-white/10 text-white/80 text-xs font-bold uppercase tracking-wider py-1.5 rounded-lg border border-white/10">Talento</div>
              : <div className="bg-white/5 text-white/50 text-xs font-bold uppercase tracking-wider py-1.5 rounded-lg border border-white/5">Normal</div>;

          return (
            <div key={rookie.driver_id} className={`rounded-xl bg-white/[0.03] border border-white/5 p-6 text-center border-t-4 ${borderColor}`}>
              <div className="text-4xl mb-3">{isGenius ? "🌟" : isTalent ? "⭐" : "👤"}</div>
              <h3 className="text-lg font-bold mb-1">{rookie.driver_name}</h3>
              <p className="text-[10px] uppercase tracking-widest text-text-secondary mb-4 border-b border-white/10 pb-4">
                {extractNationalityLabel(rookie.nationality) || "Sem País"} • {rookie.age} Anos
              </p>
              {typeBadge}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function SectionRetirements({ retirements }) {
  if (!retirements || retirements.length === 0) {
    return (
      <div className="max-w-4xl animate-fade-in">
        <h2 className="text-3xl font-bold mb-2">Aposentadorias</h2>
        <p className="text-text-secondary">Nenhum piloto se despediu das pistas ao final desse campeonato.</p>
      </div>
    );
  }

  return (
    <div className="max-w-4xl space-y-10 animate-fade-in">
      <div>
        <h2 className="text-3xl font-bold mb-2">Aposentadorias Confirmadas</h2>
        <p className="text-text-secondary">Lendas deixam as pistas e abrem espaço no grid. Suas histórias estarão no Hall da Fama.</p>
      </div>

      <div className="space-y-4">
        {retirements.map((retirement) => (
          <div key={retirement.driver_id} className="rounded-xl border border-white/5 bg-white/[0.03] p-6 flex flex-col sm:flex-row gap-6 items-center sm:justify-start">
            <div className="w-16 h-16 bg-white/5 border border-white/10 rounded-full flex items-center justify-center text-2xl grayscale">
              {extractFlag(retirement.nationality) || "🏁"}
            </div>
            <div>
              <h3 className="text-xl font-bold">
                {retirement.driver_name}
                <span className="ml-2 text-sm text-text-secondary font-normal">{retirement.age} anos</span>
              </h3>
              <p className="text-sm mt-2 text-white/80 leading-relaxed max-w-2xl">
                {retirement.reason || "O piloto declarou que pendurará as luvas após a bateria final deste campeonato."}
              </p>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function EndOfSeasonView() {
  const result = useCareerStore((state) => state.endOfSeasonResult);
  const enterPreseason = useCareerStore((state) => state.enterPreseason);
  const isEnteringPreseason = useCareerStore((state) => state.isEnteringPreseason);
  const careerId = useCareerStore((state) => state.careerId);

  const [activeTab, setActiveTab] = useState("licenses");

  if (!result) return null;

  const pilotLicenseCount = new Set([
    ...(result.growth_reports || []).map((report) => report.driver_id),
    ...(result.licenses_earned || []).map((license) => license.driver_id),
    ...((result.promotion_result?.pilot_effects || [])
      .filter((effect) => effect.effect === "FreedNoLicense")
      .map((effect) => effect.driver_id)),
  ]).size;

  const tabs = [
    { id: "licenses", icon: "📜", label: "Licenças de Pilotos", count: pilotLicenseCount },
    { id: "teams", icon: "🏎️", label: "Movimentações de Equipes", count: result.promotion_result?.movements?.length || 0 },
    { id: "rookies", icon: "🎓", label: "Novos Talentos", count: result.rookies_generated?.length || 0 },
    { id: "retirements", icon: "👴", label: "Aposentadorias", count: result.retirements?.length || 0 },
  ];

  return (
    <div className="app-shell flex h-screen flex-col overflow-hidden bg-[#0A0F1C] text-text-primary relative pt-12 items-center justify-center">
      <div className="app-backdrop" />
      <LoadingOverlay
        open={isEnteringPreseason}
        title="Abrindo mercado de transferências"
        message="Carregando equipes, propostas e pilotos disponíveis."
      />

      <div className="flex flex-col w-full h-full max-w-[1600px] mx-auto z-10 px-8 pb-8">
        <nav className="border border-white/10 px-6 py-4 flex justify-between items-center bg-[#0E0E10]/80 backdrop-blur-md shrink-0 mb-6 rounded-2xl shadow-lg mt-4 h-24">
          <div>
            <h1 className="text-2xl font-bold tracking-tighter text-white">O Paddock Virou a Página</h1>
            <p className="text-xs text-accent-primary uppercase tracking-widest mt-2 font-bold">Resumo da Temporada {result.new_year - 1}</p>
          </div>
          <div className="flex items-center gap-5">
            <span className="text-xs border border-white/20 bg-white/5 px-5 py-3 rounded-full font-bold hidden sm:block">
              Temporada {result.new_year} Preparada no Mercado
            </span>
            <GlassButton
              variant="primary"
              disabled={isEnteringPreseason}
              className="rounded-full shadow-[0_0_20px_rgba(88,166,255,0.4)] !px-8 !py-3 font-semibold text-sm transition-transform hover:scale-105"
              onClick={() => {
                if (!isEnteringPreseason) {
                  void enterPreseason();
                }
              }}
            >
              {isEnteringPreseason ? "Aguarde..." : "Iniciar Pré-Temporada"}
            </GlassButton>
          </div>
        </nav>

        <div className="flex flex-1 overflow-hidden w-full gap-8">
          <aside className="w-[340px] shrink-0 overflow-y-auto space-y-3 pr-4 custom-scrollbar">
            {tabs.map((tab) => {
              const isActive = activeTab === tab.id;
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex items-center justify-between w-full text-left p-5 rounded-2xl transition-all ${isActive ? "bg-accent-primary/10 border border-accent-primary/30 shadow-inner" : "hover:bg-white/5 border border-white/5 bg-white/[0.02]"}`}
                >
                  <div className="flex items-center gap-4">
                    <span className="text-2xl opacity-90">{tab.icon}</span>
                    <span className={`font-semibold tracking-wide ${isActive ? "text-accent-primary" : "text-text-primary/70"}`}>{tab.label}</span>
                  </div>
                  {tab.count > 0 && (
                    <span className={`${isActive ? "bg-accent-primary/20 text-accent-primary border border-accent-primary/30" : "bg-white/10 text-white/50"} text-[11px] px-2.5 py-1 rounded-full font-bold`}>
                      {tab.count}
                    </span>
                  )}
                </button>
              );
            })}
          </aside>

          <main className="flex-1 overflow-y-auto pr-8 custom-scrollbar">
            <div className="glass-light p-10 rounded-3xl min-h-full">
              {activeTab === "licenses" && (
                <SectionLicenses
                  careerId={careerId}
                  reports={result.growth_reports}
                  licenses={result.licenses_earned}
                  promotionEffects={result.promotion_result?.pilot_effects}
                  movements={result.promotion_result?.movements}
                />
              )}
              {activeTab === "teams" && <SectionTeams movements={result.promotion_result?.movements} />}
              {activeTab === "rookies" && <SectionRookies rookies={result.rookies_generated} />}
              {activeTab === "retirements" && <SectionRetirements retirements={result.retirements} />}
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}

export default EndOfSeasonView;
