import i18n from "../../i18n/index.js";
import { FILTRO_TODOS } from "./globalDriverRanking";

// Barra de filtros do ranking global de pilotos. Sem estado próprio: recebe os
// valores atuais e devolve as mudanças por `onChange(chave, valor)`.
export function FilterBar({ filters, options, onChange, onReset }) {
  return (
    // Nove colunas, e não oito: os seis seletores + o par de idades ocupavam a
    // grade inteira e empurravam o "Limpar filtros" para uma segunda linha que
    // era quase toda vazia. Com a coluna extra ele cabe na mesma fileira, e o
    // `items-end` alinha o botão pela base dos campos.
    <div className="grid items-end gap-x-3 gap-y-2 md:grid-cols-2 xl:grid-cols-9">
      <FilterSelect
        label={i18n.t("globalDrivers.filter.status")}
        value={filters.status}
        onChange={(value) => onChange("status", value)}
        options={[
          [FILTRO_TODOS, i18n.t("globalDrivers.filter.all")],
          ["Ativo", i18n.t("globalDrivers.filter.statusActive")],
          ["Livre", i18n.t("globalDrivers.filter.statusFree")],
          ["Aposentado", i18n.t("globalDrivers.filter.statusRetired")],
        ]}
      />
      <FilterSelect
        label={i18n.t("globalDrivers.filter.category")}
        value={filters.category}
        onChange={(value) => onChange("category", value)}
        options={[[FILTRO_TODOS, i18n.t("globalDrivers.filter.allF")]]}
        groups={options.categoryGroups}
      />
      <FilterSelect
        label={i18n.t("globalDrivers.filter.nationality")}
        value={filters.nationality}
        onChange={(value) => onChange("nationality", value)}
        options={[
          [FILTRO_TODOS, i18n.t("globalDrivers.filter.allF")],
          ...options.nationalities.map(({ code, label }) => [code, label]),
        ]}
      />
      <FilterSelect
        label={i18n.t("globalDrivers.filter.champions")}
        value={filters.champions}
        onChange={(value) => onChange("champions", value)}
        options={[
          ["all", i18n.t("globalDrivers.filter.all")],
          ["champions", i18n.t("globalDrivers.filter.onlyChampions")],
        ]}
      />
      <FilterSelect
        label={i18n.t("globalDrivers.filter.injured")}
        value={filters.injured}
        onChange={(value) => onChange("injured", value)}
        options={[
          ["all", i18n.t("globalDrivers.filter.all")],
          ["injured", i18n.t("globalDrivers.filter.onlyInjured")],
        ]}
      />
      <FilterSelect
        label={i18n.t("globalDrivers.filter.favorites")}
        value={filters.favorites}
        onChange={(value) => onChange("favorites", value)}
        options={[
          ["all", i18n.t("globalDrivers.filter.all")],
          ["only", i18n.t("globalDrivers.filter.onlyFavorites")],
        ]}
      />
      <div className="grid grid-cols-2 gap-2 xl:col-span-2">
        <FilterInput
          label={i18n.t("globalDrivers.filter.minAge")}
          value={filters.minAge}
          onChange={(value) => onChange("minAge", value)}
        />
        <FilterInput
          label={i18n.t("globalDrivers.filter.maxAge")}
          value={filters.maxAge}
          onChange={(value) => onChange("maxAge", value)}
        />
      </div>
      <button
        type="button"
        onClick={onReset}
        className="rounded-xl border border-white/10 bg-white/[0.04] px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-text-secondary transition-glass hover:text-text-primary"
      >
        Limpar filtros
      </button>
    </div>
  );
}

function FilterSelect({ label, value, onChange, options = [], groups = null }) {
  return (
    <label className="text-[10px] font-semibold uppercase tracking-[0.14em] text-text-muted">
      <span>{label}</span>
      <select
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="mt-1.5 w-full rounded-xl border border-white/10 bg-app-card px-3 py-2 text-xs normal-case tracking-normal text-text-primary outline-none transition-glass focus:border-accent-primary/60"
      >
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue} value={optionValue} className="bg-app-card text-text-primary">
            {optionLabel}
          </option>
        ))}
        {groups?.map((group) => (
          <optgroup key={group.key} label={group.label} className="bg-app-card text-text-primary">
            {group.options.map(([optionValue, optionLabel]) => (
              <option key={optionValue} value={optionValue} className="bg-app-card text-text-primary">
                {optionLabel}
              </option>
            ))}
          </optgroup>
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
        className="mt-1.5 w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-xs normal-case tracking-normal text-text-primary outline-none transition-glass focus:border-accent-primary/60"
      />
    </label>
  );
}

export default FilterBar;
