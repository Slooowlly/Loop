import { useEffect, useLayoutEffect, useRef, useState } from "react";

const SPECIAL_SCOPE_LABEL = "Rankings";
const SHARED_SCOPE_IDS = new Set(["production_challenger", "endurance"]);
const DRAWER_GAP = 8;
const FAMILY_CONFIG = [
  {
    id: "mazda",
    label: "Mazda",
    defaultScopeId: "mazda_rookie",
    items: [
      { type: "scope", scopeId: "mazda_rookie", label: "Rookie" },
      { type: "scope", scopeId: "mazda_amador", label: "Cup" },
      { type: "scope", scopeId: "production_challenger", scopeClass: "mazda", label: "Production" },
    ],
  },
  {
    id: "toyota",
    label: "Toyota",
    defaultScopeId: "toyota_rookie",
    items: [
      { type: "scope", scopeId: "toyota_rookie", label: "Rookie" },
      { type: "scope", scopeId: "toyota_amador", label: "Cup" },
      { type: "scope", scopeId: "production_challenger", scopeClass: "toyota", label: "Production" },
    ],
  },
  {
    id: "bmw",
    label: "BMW",
    defaultScopeId: "bmw_m2",
    items: [
      { type: "scope", scopeId: "bmw_m2", label: "Rookie" },
      { type: "scope", scopeId: "production_challenger", scopeClass: "bmw", label: "Production" },
    ],
  },
  {
    id: "gt4",
    label: "GT4",
    defaultScopeId: "gt4",
    items: [
      { type: "scope", scopeId: "gt4", label: "Rookie" },
      { type: "scope", scopeId: "endurance", scopeClass: "gt4", label: "Endurance" },
    ],
  },
  {
    id: "gt3",
    label: "GT3",
    defaultScopeId: "gt3",
    items: [
      { type: "scope", scopeId: "gt3", label: "Rookie" },
      { type: "scope", scopeId: "endurance", scopeClass: "gt3", label: "Endurance" },
    ],
  },
  {
    id: "lmp2",
    label: "LMP2",
    defaultScopeId: "endurance",
    defaultScopeClass: "lmp2",
    items: [
      { type: "label", label: "Rookie" },
      { type: "scope", scopeId: "endurance", scopeClass: "lmp2", label: "Endurance" },
    ],
  },
];

function NewsScopeDrawers({
  scopeTabs,
  requestState,
  showDrawer = true,
  onScopeChange,
  renderPrimaryFilters,
  renderContextualFilters,
}) {
  const families = buildFamilies(scopeTabs);
  const specialScope =
    scopeTabs.find((scope) => scope.scope_type === "famous") ??
    scopeTabs.find((scope) => scope.special);
  const [openFamilyId, setOpenFamilyId] = useState(null);

  useEffect(() => {
    if (families.length === 0) {
      setOpenFamilyId(null);
      return;
    }

    setOpenFamilyId((current) => {
      const inferred = inferUniqueFamilyId(
        requestState.scopeType,
        requestState.scopeId,
        requestState.scopeClass,
        families,
      );
      if (inferred) return inferred;
      if (current && families.some((family) => family.id === current)) return current;
      return families[0].id;
    });
  }, [families, requestState.scopeType, requestState.scopeId, requestState.scopeClass]);

  const openFamily = families.find((family) => family.id === openFamilyId) ?? families[0] ?? null;
  const shouldRenderFamilyDrawer = showDrawer && requestState.scopeType === "category" && Boolean(openFamily);

  function handleFamilyClick(family) {
    setOpenFamilyId(family.id);
    if (family.defaultSelection) {
      onScopeChange(family.defaultSelection);
    }
  }

  function handleFamilyItemClick(family, item) {
    setOpenFamilyId(family.id);
    if (item.type === "scope" && item.scope) {
      onScopeChange({
        scope: item.scope,
        scopeClass: item.scopeClass ?? null,
      });
    }
  }

  return (
    <section data-news-section="dashboard" className="bg-gradient-to-b from-white/[0.02] to-transparent p-5 sm:p-6 lg:p-8 space-y-6">
      
      {/* Nível 1: Abas Globais Macros e Filtros Primários */}
      <div className="flex flex-col xl:flex-row xl:items-center justify-between gap-6">
        
        {/* Categorias (GT3, LMP2...) */}
        <div className="inline-flex rounded-full border border-white/10 bg-black/20 p-1 overflow-x-auto max-w-full no-scrollbar">
          {families.map((family) => {
            const isOpen = requestState.scopeType === "category" && openFamily?.id === family.id;
            return (
              <button
                key={family.id}
                type="button"
                onClick={() => handleFamilyClick(family)}
                className={[
                  "rounded-full px-5 lg:px-6 py-2 text-sm transition-all whitespace-nowrap",
                  isOpen
                    ? "font-bold bg-white/10 text-white shadow-sm border border-white/10"
                    : "font-medium text-text-secondary hover:text-white border border-transparent",
                ].join(" ")}
                aria-label={family.label}
              >
                {family.label}
              </button>
            );
          })}

          {specialScope ? (
            <button
              type="button"
              onClick={() => onScopeChange(specialScope)}
              className={[
                "rounded-full px-5 lg:px-6 py-2 text-sm transition-all whitespace-nowrap",
                requestState.scopeType === specialScope.scope_type && requestState.scopeId === specialScope.id
                  ? "font-bold bg-white/10 text-accent-gold shadow-sm border border-white/10"
                  : "font-medium text-text-secondary hover:text-white border border-transparent",
              ].join(" ")}
              aria-label={SPECIAL_SCOPE_LABEL}
            >
              {SPECIAL_SCOPE_LABEL}
            </button>
          ) : null}
        </div>

        {/* Primary Filters (Corridas, Pilotos...) */}
        <div className="flex items-center gap-1.5 flex-wrap flex-none">
          {renderPrimaryFilters && renderPrimaryFilters()}
        </div>
      </div>

      {/* Nível 2: O Escopo Físico & Contexto */}
      {showDrawer ? (
        <div className="rounded-2xl border border-white/10 bg-black/40 p-4 flex flex-col md:flex-row md:items-center gap-6 shadow-[inset_0_2px_18px_rgba(0,0,0,0.5)]">
          
          {/* Seleção de Copa (Rookie/Endurance) */}
          {shouldRenderFamilyDrawer && openFamily ? (
            <div className="flex items-center bg-white/5 rounded-xl p-1 border border-white/5 w-fit">
              {openFamily.items.map((item, index) => {
                const isActive = isFamilyItemActive(item, requestState);
                return (
                  <div key={`${openFamily.id}-${item.label}-${index}`} className="flex items-center">
                    {item.type === "scope" ? (
                      <button
                        type="button"
                        onClick={() => handleFamilyItemClick(openFamily, item)}
                        className={[
                          "px-4 lg:px-5 py-2 rounded-lg text-sm transition-all",
                          isActive
                            ? "font-bold text-accent-primary bg-accent-primary/10 shadow-[inset_0_0_0_1px_rgba(88,166,255,0.2)]"
                            : "font-medium text-text-secondary hover:text-white",
                        ].join(" ")}
                      >
                        {item.label}
                      </button>
                    ) : (
                      <span className="px-5 py-2 rounded-lg text-sm font-bold text-text-secondary">
                        {item.label}
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          ) : null}

          {/* Separador */}
          <div className="h-8 w-px bg-white/10 hidden md:block"></div>

          {/* Pílulas de Contexto do Filtro Ativo */}
          <div className="flex items-center gap-2 flex-wrap flex-1">
            {renderContextualFilters && renderContextualFilters()}
          </div>
        </div>
      ) : null}
    </section>
  );
}

function buildFamilies(scopeTabs) {
  const scopeMap = new Map(
    scopeTabs
      .filter((scope) => scope.scope_type === "category")
      .map((scope) => [scope.id, scope]),
  );

  return FAMILY_CONFIG.map((family) => {
    const items = family.items
      .map((item) => {
        if (item.type !== "scope") return item;
        const scope = scopeMap.get(item.scopeId);
        if (!scope) return null;
        return { ...item, scope };
      })
      .filter(Boolean);

    const defaultScope = scopeMap.get(family.defaultScopeId) ?? null;
    if (!defaultScope || items.length === 0) return null;

    return {
      ...family,
      defaultScope,
      defaultSelection: {
        scope: defaultScope,
        scopeClass: family.defaultScopeClass ?? null,
      },
      items,
      memberScopeIds: items.filter((item) => item.type === "scope").map((item) => item.scope.id),
    };
  }).filter(Boolean);
}

function inferUniqueFamilyId(scopeType, scopeId, scopeClass, families) {
  if (scopeType !== "category" || !scopeId) return null;
  if (!SHARED_SCOPE_IDS.has(scopeId)) {
    return families.find((family) => family.memberScopeIds.includes(scopeId))?.id ?? null;
  }

  return families.find((family) =>
    family.items.some(
      (item) =>
        item.type === "scope"
        && item.scope.id === scopeId
        && (item.scopeClass ?? null) === (scopeClass ?? null),
    ),
  )?.id ?? null;
}

function isFamilyItemActive(item, requestState) {
  if (item.type !== "scope") return false;
  if (requestState.scopeType !== "category" || requestState.scopeId !== item.scope.id) return false;
  if (!SHARED_SCOPE_IDS.has(item.scope.id)) return true;
  return requestState.scopeClass === (item.scopeClass ?? null);
}

export default NewsScopeDrawers;
