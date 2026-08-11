import { useTranslation } from "react-i18next";

import GlassCard from "../ui/GlassCard";
import Tooltip from "../ui/Tooltip";
import i18n from "../../i18n/index.js";
import {
  formatIndex,
  formatYears,
  podiumBreakdownTitle,
  statusClass,
  teamCategoryLabel,
} from "./globalDriverFormatters";

// Cartão do piloto em foco (o selecionado na tabela) com o rodapé fixo do piloto do
// jogador — o "onde eu estou" fica sempre visível, mesmo olhando outro piloto.
export function FocusedDriverCard({ row, ranks, userRow, userRanks }) {
  const { t } = useTranslation();
  const metrics = [
    { label: i18n.t("globalDrivers.stat.index"), value: formatIndex(row.historical_index), rank: row.historical_rank },
    { label: i18n.t("globalDrivers.stat.races"), value: row.corridas, rank: ranks.races },
    { label: i18n.t("globalDrivers.stat.wins"), value: row.vitorias, rank: ranks.wins },
    { label: i18n.t("globalDrivers.stat.podiums"), value: row.podios, rank: ranks.podiums, title: podiumBreakdownTitle(row) },
    { label: i18n.t("globalDrivers.stat.career"), value: formatYears(row.anos_carreira), rank: ranks.careerYears },
  ];

  return (
    <GlassCard hover={false} as="article" className="flex h-full flex-col overflow-hidden rounded-[28px] border-accent-primary/25 p-0">
      <div className="flex flex-1 flex-col p-5 sm:p-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-[10px] uppercase tracking-[0.2em] text-accent-primary">{t("globalDrivers.focusEyebrow")}</p>
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
  const { t } = useTranslation();
  const stats = [
    { label: i18n.t("globalDrivers.stat.index"), value: formatIndex(row.historical_index) },
    { label: i18n.t("globalDrivers.stat.wins"), value: row.vitorias ?? 0 },
    { label: i18n.t("globalDrivers.stat.titles"), value: row.titulos ?? 0 },
    { label: i18n.t("globalDrivers.stat.career"), value: formatYears(row.anos_carreira) },
  ];

  return (
    <section className="border-t border-white/10 bg-black/15 px-5 py-4 sm:px-6" aria-label={t("globalDrivers.yourDriverAria")}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{t("globalDrivers.yourDriver")}</p>
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

function FocusStat({ label, value, rank, title }) {
  return (
    <Tooltip texto={title || undefined}>
      <div className={`min-h-24 rounded-2xl border border-white/8 bg-black/10 p-3${title ? " cursor-help" : ""}`}>
        <p className="text-[10px] uppercase tracking-[0.14em] text-text-muted">{label}</p>
        <p className="mt-2 font-mono text-lg font-semibold text-text-primary">{value ?? 0}</p>
        <p className="mt-1 text-xs text-accent-primary">Top #{rank || "--"}</p>
      </div>
    </Tooltip>
  );
}

// Painel lateral com os campeonatos que já tiveram campeão; clicar num grupo abre o
// diálogo com a galeria de campeões daquele campeonato.
export function ChampionshipChampionPanel({ sections, onOpenChampionship }) {
  const totalGroups = sections.reduce((total, section) => total + section.groups.length, 0);

  return (
    <GlassCard hover={false} as="aside" className="flex max-h-[430px] flex-col overflow-hidden rounded-[28px] p-5 sm:p-6">
      <div className="flex shrink-0 items-start justify-between gap-3">
        <div>
          <p className="text-[10px] uppercase tracking-[0.2em] text-accent-primary">{i18n.t("globalDrivers.champions.eyebrow")}</p>
          <h3 className="mt-2 text-xl font-semibold text-text-primary">{i18n.t("globalDrivers.champions.title")}</h3>
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
                    aria-label={i18n.t("globalDrivers.viewChampionsAria", { group: group.label })}
                    onClick={() => onOpenChampionship(group)}
                    className="flex min-h-20 items-center justify-between gap-4 rounded-2xl border border-white/8 bg-black/10 px-4 py-3 text-left transition-glass hover:border-accent-primary/40 hover:bg-accent-primary/10"
                  >
                    <span>
                      <span className="block text-sm font-semibold text-text-primary">{group.label}</span>
                      <span className="mt-1 block text-xs text-text-muted">
                        {group.champions.slice(0, 2).map((champion) => champion.name).join(", ") || i18n.t("globalDrivers.row.noNames")}
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
            {i18n.t("globalDrivers.noChampionsRecorded")}
          </p>
        )}
      </div>
    </GlassCard>
  );
}
