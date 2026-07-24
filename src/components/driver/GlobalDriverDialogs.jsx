import TeamLogoMark from "../team/TeamLogoMark";
import i18n from "../../i18n/index.js";
import { titleCategoryLabel } from "./globalDriverFormatters";

// Detalhe dos títulos de um piloto, por categoria.
export function TitleBreakdownDialog({ row, onClose }) {
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
            <p className="text-[10px] uppercase tracking-[0.18em] text-accent-primary">{i18n.t("globalDrivers.titles.eyebrow")}</p>
            <h3 id={titleId} className="mt-1 text-xl font-semibold text-text-primary">
              {i18n.t("globalDrivers.titles.ofDriver", { name: row.nome })}
            </h3>
            <p className="mt-1 text-sm text-text-secondary">{i18n.t("globalDrivers.titles.total", { count: row.titulos })}</p>
          </div>
          <button
            type="button"
            aria-label={i18n.t("globalDrivers.titles.close")}
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
                  {i18n.t("globalDrivers.champions.titlesCount", { count: entry.titulos })}
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

// Galeria de campeões de um campeonato (grupo do ChampionshipChampionPanel).
export function ChampionshipChampionsDialog({ group, onClose }) {
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
              {i18n.t("globalDrivers.champions.ofGroup", { group: group.label })}
            </h3>
            <p className="mt-1 text-sm text-text-secondary">
              {i18n.t("globalDrivers.champions.championsCount", { count: group.championCount })}
            </p>
          </div>
          <button
            type="button"
            aria-label={i18n.t("globalDrivers.champions.close")}
            onClick={onClose}
            className="rounded-lg border border-white/10 px-3 py-1 text-sm text-text-secondary transition-glass hover:border-accent-primary/40 hover:text-text-primary"
          >
            X
          </button>
        </div>

        <div className="mt-5 max-h-[58vh] overflow-y-auto pr-2">
          <div className="divide-y divide-white/8">
          {group.champions.map((champion) => (
            <div key={champion.id} className="flex items-start justify-between gap-4 py-3">
              <div className="min-w-0">
                <p className="font-semibold text-text-primary">{champion.name}</p>
                <ChampionTitleYears champion={champion} />
              </div>
              <span className="shrink-0 font-mono text-sm text-accent-secondary">
                {i18n.t("globalDrivers.champions.titlesCount", { count: champion.titles })}
              </span>
            </div>
          ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function ChampionTitleYears({ champion }) {
  if (champion.yearTeams.length > 0) {
    return (
      <div className="mt-2 flex flex-wrap gap-1.5">
        {champion.yearTeams.map((yearTeam) => (
          <span
            key={yearTeam.ano}
            title={yearTeam.equipe ?? undefined}
            className="inline-flex items-center gap-1.5 rounded-md border border-white/8 bg-black/20 px-2 py-1"
          >
            <span className="font-mono text-xs text-text-muted">{yearTeam.ano}</span>
            {yearTeam.equipe ? (
              <TeamLogoMark teamName={yearTeam.equipe} color={yearTeam.equipe_cor} size="xs" />
            ) : null}
          </span>
        ))}
      </div>
    );
  }

  return (
    <p className="mt-1 font-mono text-xs text-text-muted">
      {champion.years.length > 0 ? champion.years.join(", ") : i18n.t("globalDrivers.row.yearsUnavailable")}
    </p>
  );
}
