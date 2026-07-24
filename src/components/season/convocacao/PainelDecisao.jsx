import { useTranslation } from "react-i18next";

import { CATEGORY_LABELS } from "./constantes.js";
import { formatChampionshipPosition, roleLabel } from "./agrupamentos.js";

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

export default function PainelDecisao({
  acceptedSpecialOffer,
  specialWindowState,
  dailyLogGroups,
}) {
  const { t } = useTranslation();

  return (
    <aside className="glass scroll-area animate-drawer-in self-start overflow-y-auto rounded-2xl px-4 py-4 lg:px-5 lg:py-5 xl:max-h-[calc(100vh-96px)]">
      <div className="mb-4 flex h-6 items-center gap-2">
        <span className="relative inline-flex h-2.5 w-2.5">
          {acceptedSpecialOffer && (
            <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-[#58a6ff]/80" />
          )}
          <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-[color:var(--accent-primary)]" />
        </span>
        <p className="text-body-sm font-bold uppercase tracking-[0.22em] text-[color:var(--accent-primary)]">
          {t("convocation.decision.title")}
        </p>
      </div>

      {acceptedSpecialOffer ? (
        <div className="glass-light rounded-xl border px-4 py-4">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--accent-primary)]">
            {t("convocation.decision.highlightLabel")}
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
          {t("convocation.decision.empty")}
        </div>
      )}

      <div
        data-testid="daily-log-market"
        className="mt-4 rounded-xl border border-white/8 bg-black/18 px-4 py-4"
      >
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--text-muted)]">
          {t("convocation.dailyLog.title")}
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
            {t("convocation.dailyLog.empty")}
          </p>
        )}
      </div>

      <div className="mt-4 rounded-xl border border-white/8 bg-black/18 px-4 py-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-[color:var(--text-muted)]">
          {t("convocation.nextStep.title")}
        </p>
        <p className="mt-2 text-body text-[color:var(--text-secondary)]">
          {specialWindowState?.is_finished
            ? t("convocation.nextStep.finished")
            : t("convocation.nextStep.inProgress")}
        </p>
      </div>
    </aside>
  );
}
