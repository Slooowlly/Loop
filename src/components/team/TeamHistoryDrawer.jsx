import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import TeamLogoMark from "./TeamLogoMark";
import { financialState } from "./teamFinanceLabels";
import i18n from "../../i18n/index.js";
import { categoryLabel, formatMoney } from "../../utils/formatters";

const TEAM_HISTORY_TABS = [
  { id: "records" },
  { id: "sport" },
  { id: "identity" },
  { id: "management" },
  { id: "categories" },
];

function TeamNavChevron({ direction }) {
  const path = direction === "up" ? "M2 7.5 6 3.5l4 4" : "M2 4.5l4 4 4-4";
  return (
    <svg
      viewBox="0 0 12 12"
      aria-hidden="true"
      className="h-3.5 w-3.5 flex-shrink-0"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={path} />
    </svg>
  );
}

function TeamNavigatorButton({ label, direction, disabled, onClick }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className={[
        "flex h-10 w-10 items-center justify-center rounded-2xl border backdrop-blur-md transition-all duration-200 ease-out",
        disabled
          ? "cursor-not-allowed border-white/[0.05] bg-[#11151b]/90 text-[#5b616b]"
          : "border-white/[0.12] bg-[#111d31]/95 text-text-secondary shadow-[0_14px_34px_rgba(0,0,0,0.34)] hover:border-white/[0.18] hover:bg-[#18263d] hover:text-text-primary focus-visible:border-white/[0.18] focus-visible:bg-[#18263d] focus-visible:text-text-primary",
      ].join(" ")}
    >
      <TeamNavChevron direction={direction} />
    </button>
  );
}

function TeamHistoryEdgeNavigator({ previousTeam, nextTeam, onSelectTeam, placement = "right" }) {
  const { t } = useTranslation();
  return (
    <div
      className={[
        "pointer-events-auto fixed top-24 z-[91] flex flex-col gap-2 max-lg:hidden sm:top-28",
        placement === "left" ? "animate-edge-rail-in-right" : "animate-edge-rail-in",
      ].join(" ")}
      style={
        placement === "center"
          ? { right: "18px" }
          : placement === "left"
            ? { left: "calc(min(50vw, 720px) + 14px)" }
            : { right: "calc(min(50vw, 720px) + 14px)" }
      }
    >
      <TeamNavigatorButton
        label={t("myTeamTab.history.nav.previous")}
        direction="up"
        disabled={!previousTeam}
        onClick={() => previousTeam && onSelectTeam(previousTeam)}
      />
      <TeamNavigatorButton
        label={t("myTeamTab.history.nav.next")}
        direction="down"
        disabled={!nextTeam}
        onClick={() => nextTeam && onSelectTeam(nextTeam)}
      />
    </div>
  );
}

export function TeamHistoryDrawer({
  careerId,
  team,
  teams,
  playerTeam,
  activeCategory,
  activeTab,
  onTabChange,
  onSelectTeam,
  onClose,
  placement = "right",
}) {
  const { t } = useTranslation();
  const [historyDossier, setHistoryDossier] = useState(null);
  const [historyStatus, setHistoryStatus] = useState("loading");
  const [historyError, setHistoryError] = useState("");
  const dossier = buildTeamHistoryDossier(
    team,
    teams,
    playerTeam,
    activeCategory,
    historyDossier,
    historyStatus,
    historyError,
  );
  const orderedTeams = orderTeamsForHistoryNavigation(teams);
  const currentTeamIndex = orderedTeams.findIndex((entry) => entry.id === team?.id);
  const previousTeam = currentTeamIndex > 0 ? orderedTeams[currentTeamIndex - 1] : null;
  const nextTeam = currentTeamIndex >= 0 && currentTeamIndex < orderedTeams.length - 1
    ? orderedTeams[currentTeamIndex + 1]
    : null;

  useEffect(() => {
    let mounted = true;
    if (!careerId || !team?.id) {
      setHistoryStatus("error");
      setHistoryError(i18n.t("myTeamTab.history.unavailable"));
      return undefined;
    }

    setHistoryStatus("loading");
    setHistoryError("");
    setHistoryDossier(null);
    invoke("get_team_history_dossier", {
      careerId,
      teamId: team.id,
      category: activeCategory ?? playerTeam?.categoria ?? team?.categoria ?? "",
    })
      .then((payload) => {
        if (!mounted) return;
        setHistoryDossier(payload);
        setHistoryStatus("ready");
      })
      .catch((invokeError) => {
        if (!mounted) return;
        setHistoryError(typeof invokeError === "string" ? invokeError : i18n.t("myTeamTab.history.loadError"));
        setHistoryStatus("error");
      });

    return () => {
      mounted = false;
    };
  }, [activeCategory, careerId, team?.id, team?.categoria, playerTeam?.categoria]);

  const drawerLayer = (
    <div className="fixed inset-0 z-[90] flex items-center justify-center" data-testid="team-history-layer" aria-hidden={false}>
      <button
        type="button"
        aria-label={t("myTeamTab.history.closeAria")}
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/70 backdrop-blur-[3px]"
      />
      <TeamHistoryEdgeNavigator
        previousTeam={previousTeam}
        nextTeam={nextTeam}
        onSelectTeam={onSelectTeam}
        placement={placement}
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-labelledby="team-history-title"
        className={[
          "overflow-y-auto border-white/15 bg-[#07101d]",
          placement === "center"
            ? "animate-scale-in relative z-10 max-h-[88vh] w-[min(92vw,760px)] rounded-[24px] border shadow-[0_30px_90px_rgba(0,0,0,0.72)]"
            : [
                placement === "left" ? "animate-drawer-in-left left-0 border-r shadow-[28px_0_80px_rgba(0,0,0,0.72)]" : "animate-drawer-in right-0 border-l shadow-[-28px_0_80px_rgba(0,0,0,0.72)]",
                "absolute inset-y-0 w-[min(50vw,720px)] max-lg:w-full",
              ].join(" "),
        ].join(" ")}
        data-testid="team-history-drawer"
        style={{
          "--team": dossier.color,
          backgroundImage:
            "radial-gradient(circle at 10% 4%, color-mix(in srgb, var(--team) 16%, transparent), transparent 18rem), linear-gradient(180deg, rgba(12,22,38,0.98), rgba(5,11,20,0.995))",
        }}
      >
        <div className="h-1.5 bg-[linear-gradient(90deg,var(--team),rgba(255,255,255,0.1))]" />
        <button
          type="button"
          onClick={onClose}
          aria-label={t("myTeamTab.history.close")}
          className="absolute right-4 top-4 grid h-9 w-9 place-items-center rounded-xl border border-white/15 bg-[#0d1727] text-text-secondary transition-glass hover:bg-[#14233a] hover:text-text-primary"
        >
          x
        </button>

        <div className="px-6 pb-7 pt-6">
          <section className="rounded-[26px] border border-[color-mix(in_srgb,var(--team)_42%,transparent)] bg-[#0c1626]/95 p-5 shadow-[0_18px_55px_rgba(0,0,0,0.32)]">
            <div className="grid min-w-0 gap-5 pr-10 sm:grid-cols-[168px_minmax(0,1fr)] sm:items-center">
              <TeamLogoMark
                teamName={dossier.name}
                color={dossier.color}
                size="hero"
                testId="team-history-logo"
              />
              <div className="min-w-0">
                <h2 id="team-history-title" className="min-w-0 truncate text-3xl font-semibold leading-none tracking-[-0.04em] text-text-primary">
                  {dossier.name}
                </h2>
                <div className="mt-4 flex flex-wrap gap-2">
                  <span className="rounded-full border border-white/15 bg-[#08111f] px-3 py-1 text-xs text-text-primary">
                    {dossier.state}
                  </span>
                  {dossier.founded ? (
                    <span className="rounded-full border border-white/15 bg-[#08111f] px-3 py-1 text-xs text-text-primary">
                      {t("myTeamTab.history.foundedIn", { year: dossier.founded })}
                    </span>
                  ) : null}
                </div>
              </div>
            </div>
          </section>

          <div role="tablist" aria-label={t("myTeamTab.history.tablistAria")} className="mt-4 flex gap-2 overflow-x-auto pb-1">
            {TEAM_HISTORY_TABS.map((tab) => (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={activeTab === tab.id}
                onClick={() => onTabChange(tab.id)}
                className={`shrink-0 rounded-full border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
                  activeTab === tab.id
                    ? "border-[color-mix(in_srgb,var(--team)_55%,transparent)] bg-[color-mix(in_srgb,var(--team)_18%,transparent)] text-text-primary"
                    : "border-white/12 bg-[#0b1524] text-text-secondary hover:border-white/20 hover:bg-[#111d31] hover:text-text-primary"
                }`}
              >
                {t(`myTeamTab.history.tabs.${tab.id}`)}
              </button>
            ))}
          </div>

          <div className="mt-4">
            {activeTab === "records" ? <TeamHistoryRecords dossier={dossier} /> : null}
            {activeTab === "sport" ? <TeamHistorySport dossier={dossier} /> : null}
            {activeTab === "identity" ? <TeamHistoryIdentity dossier={dossier} /> : null}
            {activeTab === "management" ? <TeamHistoryManagement dossier={dossier} /> : null}
            {activeTab === "categories" ? <TeamHistoryCategories dossier={dossier} /> : null}
          </div>
        </div>
      </aside>
    </div>
  );

  return createPortal(drawerLayer, document.body);
}

function TeamHistoryRecords({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.records.title")}</h3>
      <p className="mt-2 rounded-2xl border border-white/12 bg-[#08111f]/95 px-4 py-3 text-xs leading-5 text-text-secondary">
        {t("myTeamTab.history.records.compareIntro")}<strong className="text-text-primary">{dossier.recordScope}</strong>{t("myTeamTab.history.records.compareOutro")}
      </p>
      {dossier.historyStatus !== "ready" ? (
        <HistoryStateMessage dossier={dossier} />
      ) : null}
      <div className="mt-4 space-y-1">
        {dossier.records.map((record) => (
          <div key={record.label} className="flex items-center justify-between gap-3 border-b border-white/8 py-3 last:border-0">
            <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-text-secondary">
              {record.label} <em className="not-italic font-bold text-accent-primary">({record.rank})</em>
            </span>
            <strong className="font-mono text-lg text-text-primary">{record.value}</strong>
          </div>
        ))}
      </div>
      {dossier.highlights?.length > 0 && (
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          {dossier.highlights.map((item) => (
            <div key={item.label} className="rounded-[18px] border border-status-yellow/30 bg-[#1c1808]/95 p-4">
              <span className="text-[10px] font-black uppercase tracking-[0.15em] text-status-yellow/80">{item.label}</span>
              <strong className="mt-2 block text-lg font-semibold text-status-yellow">{item.value}</strong>
              <p className="mt-1 text-xs leading-5 text-text-secondary">{item.detail}</p>
            </div>
          ))}
        </div>
      )}
      {dossier.milestones?.length > 0 && (
        <div className="mt-4">
          <span className="text-[10px] font-black uppercase tracking-[0.15em] text-text-secondary">{t("myTeamTab.history.records.milestones")}</span>
          <div className="mt-3 grid gap-2 sm:grid-cols-3">
            {dossier.milestones.map((milestone) => (
              <div key={milestone.label} className="rounded-[14px] border border-white/12 bg-[#0c1626]/95 px-3 py-2.5 text-center">
                <span className="block text-[10px] font-semibold uppercase tracking-[0.12em] text-text-secondary">{milestone.label}</span>
                <strong className="mt-1 block font-mono text-lg text-[color:var(--team)]">{milestone.year}</strong>
              </div>
            ))}
          </div>
        </div>
      )}
      {dossier.titleCategories?.length > 0 && (
        <div className="mt-4">
          <span className="text-[10px] font-black uppercase tracking-[0.15em] text-text-secondary">{t("myTeamTab.history.records.titleGallery")}</span>
          <div className="mt-3 grid gap-2">
            {dossier.titleCategories.map((item) => (
              <div key={`${item.category}-${item.year}`} className="rounded-2xl border border-l-4 border-white/12 bg-[#0c1626]/95 px-4 py-3" style={{ borderLeftColor: item.color }}>
                <div className="flex items-center justify-between gap-3">
                  <strong className="text-sm text-text-primary">{item.category}</strong>
                  <span className="font-mono text-xs font-bold text-status-yellow">{item.year}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function TeamHistorySport({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.sport.title")}</h3>
      {dossier.historyStatus !== "ready" ? (
        <HistoryStateMessage dossier={dossier} />
      ) : null}
      <div className="mt-4 grid gap-3">
        <HistoryInfoCard label={t("myTeamTab.history.sport.seasonsPlayed")} value={dossier.sport.seasons} detail={t("myTeamTab.history.sport.withinScope", { scope: dossier.recordScope })} />
        <HistoryInfoCard label={t("myTeamTab.history.sport.currentStreak")} value={dossier.sport.currentStreak} />
        <HistoryInfoCard label={t("myTeamTab.history.sport.bestStreak")} value={dossier.sport.bestStreak} />
      </div>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <HistoryMiniMetric label={t("myTeamTab.history.sport.podiumRate")} value={dossier.sport.podiumRate} />
        <HistoryMiniMetric label={t("myTeamTab.history.sport.winRate")} value={dossier.sport.winRate} />
      </div>
      {dossier.seasonResults?.length > 0 && (
        <div className="mt-5">
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.sport.seasonBySeason")}</span>
          <div className="mt-3 overflow-hidden rounded-[18px] border border-white/10 bg-[#08111f]/95">
            <div className="grid grid-cols-[auto_1fr_auto_auto_auto_auto] gap-x-3 border-b border-white/10 px-4 py-2 text-[9px] font-black uppercase tracking-[0.14em] text-text-muted">
              <span>{t("myTeamTab.history.sport.cols.year")}</span><span>{t("myTeamTab.history.sport.cols.category")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.pos")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.wins")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.podiums")}</span><span className="text-right">{t("myTeamTab.history.sport.cols.points")}</span>
            </div>
            {dossier.seasonResults.map((season) => (
              <div key={season.year} className="grid grid-cols-[auto_1fr_auto_auto_auto_auto] items-center gap-x-3 border-b border-white/6 px-4 py-2 text-xs last:border-0">
                <span className="font-mono font-black text-[color:var(--team)]">{season.year}</span>
                <span className="truncate text-text-secondary">{season.category}</span>
                <span className="text-right font-mono font-semibold text-text-primary">{season.position}</span>
                <span className="text-right font-mono text-status-yellow">{season.wins}</span>
                <span className="text-right font-mono text-text-primary">{season.podiums}</span>
                <span className="text-right font-mono text-text-secondary">{season.points}</span>
              </div>
            ))}
          </div>
        </div>
      )}
      <TimelineBlock items={dossier.timeline} />
    </section>
  );
}

function HistoryStateMessage({ dossier }) {
  const message = dossier.historyStatus === "error"
    ? dossier.historyError
    : i18n.t("myTeamTab.history.loading");
  return (
    <div className="mt-4 rounded-2xl border border-white/10 bg-[#08111f]/95 px-4 py-3 text-xs text-text-secondary">
      {message}
    </div>
  );
}

function TeamHistoryIdentity({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.identity.title")}</h3>
      <div className="mt-4 grid gap-3">
        <div className="rounded-[22px] border border-[color-mix(in_srgb,var(--team)_38%,transparent)] bg-[#0c1626] bg-[radial-gradient(circle_at_10%_8%,color-mix(in_srgb,var(--team)_20%,transparent),transparent_12rem),linear-gradient(145deg,rgba(14,26,44,0.96),rgba(7,16,29,0.99))] p-4">
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.profileLabel")}</span>
          <strong className="mt-2 block text-2xl font-semibold leading-none tracking-[-0.03em] text-text-primary">{dossier.identity.profile}</strong>
          <p className="mt-3 text-xs leading-5 text-text-secondary">{dossier.identity.summary}</p>
        </div>
        <div className="grid items-stretch gap-3 md:grid-cols-[1fr_auto_1fr]">
          <HistoryInfoCard label={t("myTeamTab.history.identity.originLabel")} value={dossier.identity.origin} detail={t("myTeamTab.history.identity.originDetail")} />
          <div className="hidden place-items-center font-mono font-black text-[color:var(--team)] md:grid">-&gt;</div>
          <HistoryInfoCard label={t("myTeamTab.history.identity.currentLabel")} value={dossier.identity.current} detail={t("myTeamTab.history.identity.currentDetail")} />
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <div className="rounded-[18px] border border-status-yellow/30 bg-[#201a0b]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.rivalLabel")}</span>
            <strong className="mt-2 block text-base font-semibold text-status-yellow">{dossier.identity.rival.name}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">
              {t("myTeamTab.history.identity.rivalToday", { category: dossier.identity.rival.currentCategory })} {dossier.identity.rival.note}
            </p>
          </div>
          <div className="rounded-[18px] border border-[color-mix(in_srgb,var(--team)_35%,transparent)] bg-[#0c1626]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.symbolLabel")}</span>
            <strong className="mt-2 block text-base font-semibold text-text-primary">{dossier.identity.symbolDriver}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.identity.symbolDriverDetail}</p>
          </div>
        </div>
        {dossier.ownershipEvents?.length > 0 && (
          <div className="rounded-[18px] border border-[color-mix(in_srgb,var(--team)_35%,transparent)] bg-[#0c1626]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.identity.erasLabel")}</span>
            <ul className="mt-3 grid gap-2.5">
              {dossier.ownershipEvents.map((event, index) => (
                <li key={index} className="flex items-start gap-3">
                  <span className="mt-0.5 font-mono text-xs font-black text-[color:var(--team)]">{event.year}</span>
                  <div>
                    <strong className="block text-sm font-semibold text-text-primary">{event.title}</strong>
                    <p className="text-xs leading-5 text-text-secondary">{event.detail}</p>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </section>
  );
}

function TeamHistoryManagement({ dossier }) {
  const { t } = useTranslation();
  const operationTone = operationHealthTone(dossier.management.operationHealth);

  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.management.title")}</h3>
      <div className="mt-4 grid gap-3">
        <div className={`rounded-[22px] border p-4 ${operationTone.card}`}>
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.operationHealth")}</span>
          <strong className={`mt-2 block text-2xl font-semibold ${operationTone.text}`}>{dossier.management.operationHealth}</strong>
          <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.summary}</p>
        </div>
        <div className="grid items-stretch gap-3 md:grid-cols-[1fr_auto_1fr]">
          <div className="rounded-[18px] border border-status-green/30 bg-[#0b1d19]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.peakCash")}</span>
            <strong className="mt-2 block font-mono text-base text-status-green">{dossier.management.peakCash}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.peakCashDetail}</p>
          </div>
          <div className="hidden place-items-center font-mono font-black text-text-muted md:grid">&lt;&gt;</div>
          <div className="rounded-[18px] border border-status-red/30 bg-[#241014]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.worstCrisis")}</span>
            <strong className="mt-2 block font-mono text-base text-status-red">{dossier.management.worstCrisis}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.worstCrisisDetail}</p>
          </div>
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <HistoryInfoCard label={t("myTeamTab.history.management.healthyYears")} value={dossier.management.healthyYears} detail={dossier.management.healthyYearsDetail} />
          <HistoryInfoCard label={t("myTeamTab.history.management.recordBalance")} value={dossier.management.peakCash} detail={t("myTeamTab.history.management.recordBalanceDetail")} />
        </div>
        <HistoryInfoCard label={t("myTeamTab.history.management.biggestInvestment")} value={dossier.management.biggestInvestment} detail={dossier.management.investmentDetail} />
        {dossier.ownershipEvents?.length > 0 && (
          <div className="rounded-[18px] border border-status-yellow/30 bg-[#201a0b]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.management.boardChanges")}</span>
            <ul className="mt-3 grid gap-2.5">
              {dossier.ownershipEvents.map((event, index) => (
                <li key={index} className="flex items-start gap-3">
                  <span className="mt-0.5 font-mono text-xs font-black text-status-yellow">{event.year}</span>
                  <div>
                    <strong className="block text-sm font-semibold text-text-primary">{event.title}</strong>
                    <p className="text-xs leading-5 text-text-secondary">{event.financialNote}</p>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </section>
  );
}

function TeamHistoryCategories({ dossier }) {
  const { t } = useTranslation();
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">{t("myTeamTab.history.categories.title")}</h3>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <HistoryMiniMetric label={t("myTeamTab.history.categories.promotions")} value={dossier.movement.promotions} />
        <HistoryMiniMetric label={t("myTeamTab.history.categories.relegations")} value={dossier.movement.relegations} />
      </div>
      <div className="mt-4 grid gap-3">
        <HistoryInfoCard label={t("myTeamTab.history.categories.timeByCategory")} value={dossier.movement.timeByCategory} />
        <HistoryInfoCard label={t("myTeamTab.history.categories.bestCategory")} value={dossier.movement.bestCategory} />
        <HistoryInfoCard label={t("myTeamTab.history.categories.hardestCategory")} value={dossier.movement.hardestCategory} />
      </div>
      <span className="mt-5 block text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">{t("myTeamTab.history.categories.ladder")}</span>
      <div className="mt-3 grid gap-2.5">
        {dossier.categoryPath.map((step, index) => {
          const move = categoryMovementBadge(step.movement);
          return (
            <div key={`${step.category}-${index}`} className="rounded-2xl border border-l-4 border-white/12 bg-[#0c1626]/95 p-4" style={{ borderLeftColor: step.color }}>
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-2">
                  <span className={`font-mono text-sm font-black ${move.tone}`} title={move.label}>{move.icon}</span>
                  <strong className="text-sm text-text-primary">{step.category}</strong>
                </div>
                <span className="font-mono text-xs font-semibold" style={{ color: step.color }}>{step.years}</span>
              </div>
              <p className="mt-2 text-xs leading-5 text-text-secondary">{step.detail}</p>
            </div>
          );
        })}
      </div>
    </section>
  );
}

function categoryMovementBadge(movement) {
  switch (movement) {
    case "promotion":
      return { icon: "▲", tone: "text-status-green", label: i18n.t("myTeamTab.history.categories.movement.promotion") };
    case "relegation":
      return { icon: "▼", tone: "text-status-red", label: i18n.t("myTeamTab.history.categories.movement.relegation") };
    case "start":
      return { icon: "●", tone: "text-[color:var(--team)]", label: i18n.t("myTeamTab.history.categories.movement.start") };
    default:
      return { icon: "—", tone: "text-text-muted", label: i18n.t("myTeamTab.history.categories.movement.same") };
  }
}

function HistoryInfoCard({ label, value, detail = "" }) {
  return (
    <div className="rounded-[18px] border border-white/12 bg-[#0c1626]/95 p-4">
      <div className="flex items-start justify-between gap-3">
        <strong className="text-sm text-text-primary">{label}</strong>
        <span className="text-right font-mono text-xs font-semibold text-status-yellow">{value}</span>
      </div>
      {detail ? <p className="mt-2 text-xs leading-5 text-text-secondary">{detail}</p> : null}
    </div>
  );
}

function HistoryMiniMetric({ label, value }) {
  return (
    <div className="rounded-2xl border border-white/12 bg-[#08111f]/95 p-3">
      <span className="text-[9px] font-black uppercase tracking-[0.15em] text-text-muted">{label}</span>
      <strong className="mt-2 block font-mono text-lg text-text-primary">{value}</strong>
    </div>
  );
}

function TimelineBlock({ items }) {
  const { t } = useTranslation();
  return (
    <div className="mt-5 rounded-[22px] border border-white/12 bg-[#0c1626]/95 p-4">
      <h4 className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{t("myTeamTab.history.timeline.title")}</h4>
      <div className="mt-4 space-y-3">
        {items.map((item) => (
          <div key={item.year} className="grid grid-cols-[52px_1fr] gap-3 border-b border-white/6 pb-3 last:border-0 last:pb-0">
            <span className="font-mono text-xs font-semibold text-accent-primary">{item.year}</span>
            <p className="text-xs leading-5 text-text-secondary">{item.text}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

// Exportados para o v2 (src/components/team/v2/TeamHistoryDrawerV2.jsx) consumir
// o MESMO dossiê. Nada além do `export` mudou aqui: o v1 continua desenhando
// exatamente o que desenhava, e a regra de reaproveitar o v1 por importação
// (v2/README.md) evita duas cópias da normalização do payload.
export function orderTeamsForHistoryNavigation(teams) {
  return [...(Array.isArray(teams) ? teams : [])].sort((a, b) => {
    const positionDiff = (a.posicao ?? 999) - (b.posicao ?? 999);
    if (positionDiff !== 0) return positionDiff;
    return String(a.nome ?? "").localeCompare(String(b.nome ?? ""), "pt-BR");
  });
}

export function buildTeamHistoryDossier(
  team,
  teams,
  playerTeam,
  activeCategory,
  historyDossier,
  historyStatus = "ready",
  historyError = "",
) {
  const mergedTeam = team?.id === playerTeam?.id
    ? { ...team, ...playerTeam, posicao: team.posicao, pontos: team.pontos ?? playerTeam.pontos }
    : team;
  const category = activeCategory ?? playerTeam?.categoria ?? mergedTeam?.categoria ?? "gt4";
  const categoryName = categoryLabel(category);
  const rankedTeams = Array.isArray(teams) ? teams : [];
  const realHistory = normalizeTeamHistoryPayload(historyDossier);
  const rival = findHistoricRival(mergedTeam, rankedTeams);
  const founded = resolveTeamFoundedYear(mergedTeam);
  const categoryGroup = categoryGroupLabel(category);

  // Fallbacks NEUTROS: quando o histórico real ainda não carregou (ou o backend não
  // tem dados), mostramos "—"/estado honesto — nunca um número inventado. O histórico
  // real (get_team_history_dossier) substitui tudo isto quando pronto.
  return {
    name: mergedTeam?.nome ?? i18n.t("myTeamTab.team.fallbackName"),
    color: mergedTeam?.cor_primaria ?? "#58a6ff",
    state: realHistory?.identity?.heritage ?? teamHeritageLabel(founded),
    founded,
    currentCategory: categoryName,
    recordScope: realHistory?.recordScope ?? categoryGroup,
    historyStatus,
    historyError,
    hasHistory: realHistory?.hasHistory ?? false,
    records: realHistory?.records ?? [],
    titleCategories: realHistory?.titleCategories ?? [],
    sport: realHistory?.sport ?? emptyRealSport(),
    identity: realHistory?.identity ?? {
      origin: i18n.t("myTeamTab.history.defaults.dash"),
      current: categoryName,
      profile: i18n.t("myTeamTab.history.defaults.profileForming"),
      summary: i18n.t("myTeamTab.history.defaults.identityFormingSummary"),
      rival: {
        name: rival?.nome ?? i18n.t("myTeamTab.history.defaults.noRival"),
        currentCategory: categoryName,
        note: rival
          ? i18n.t("myTeamTab.history.defaults.rivalClosest", { group: categoryGroup })
          : i18n.t("myTeamTab.history.defaults.rivalNoDuel"),
      },
      symbolDriver: mergedTeam?.piloto_1_nome ?? i18n.t("myTeamTab.history.defaults.mainDriver"),
      symbolDriverDetail: i18n.t("myTeamTab.history.defaults.symbolDriverDetail"),
    },
    management: realHistory?.management ?? emptyRealManagement(mergedTeam),
    movement: realHistory?.movement ?? {
      promotions: i18n.t("myTeamTab.history.defaults.dash"),
      relegations: i18n.t("myTeamTab.history.defaults.dash"),
      timeByCategory: i18n.t("myTeamTab.history.defaults.unavailable"),
      bestCategory: categoryName,
      hardestCategory: i18n.t("myTeamTab.history.defaults.dash"),
    },
    categoryPath: realHistory?.categoryPath ?? [],
    timeline: realHistory?.timeline ?? [],
    ownershipEvents: realHistory?.ownershipEvents ?? [],
    highlights: realHistory?.highlights ?? [],
    milestones: realHistory?.milestones ?? [],
    seasonResults: realHistory?.seasonResults ?? [],
    recentForm: realHistory?.recentForm ?? [],
    resultSpread: realHistory?.resultSpread ?? normalizeResultSpread(null),
    championshipRun: realHistory?.championshipRun ?? null,
    lineup: realHistory?.lineup ?? [],
    reliability: realHistory?.reliability ?? normalizeReliability(null),
    outsideScopeSeasons: realHistory?.outsideScopeSeasons ?? [],
    // Intervalo de anos do MUNDO — só o v2 usa, para marcar na faixa os anos em
    // que a equipe não correu. Zero quando o backend não informa.
    worldFirstYear: realHistory?.worldFirstYear ?? 0,
    worldLastYear: realHistory?.worldLastYear ?? 0,
  };
}

function normalizeResultSpread(spread) {
  return {
    races: Number(spread?.races ?? 0),
    first: Number(spread?.first ?? 0),
    podium: Number(spread?.podium ?? 0),
    nearMiss: Number(spread?.near_miss ?? spread?.nearMiss ?? 0),
    topTen: Number(spread?.top_ten ?? spread?.topTen ?? 0),
    outside: Number(spread?.outside ?? 0),
  };
}

// Campanha do campeonato. As rodadas e as linhas vêm cruas do backend; o
// desenho é que decide escala e cor. Um recorte sem pelo menos duas rodadas ou
// sem linha nenhuma vira `null` aqui e não chega à tela como gráfico vazio.
function normalizeChampionshipRun(run) {
  if (!run) return null;
  const rounds = (run.rounds ?? []).map((value) => Number(value ?? 0));
  const lines = (run.lines ?? []).map((line) => ({
    teamId: line.team_id ?? line.teamId ?? "",
    team: line.team ?? "",
    selected: Boolean(line.selected),
    position: Number(line.position ?? 0),
    total: String(line.total ?? "0"),
    points: (line.points ?? []).map((value) => Number(value ?? 0)),
  }));
  if (rounds.length < 2 || !lines.length) return null;
  return {
    year: String(run.year ?? ""),
    category: run.category ?? "",
    categoryId: run.category_id ?? run.categoryId ?? "",
    live: Boolean(run.live),
    rounds,
    lines,
  };
}

// Confiabilidade: contagens cruas, sem prosa. As faixas se somam — quem soma é o
// desenho, então o normalizador só garante que tudo é número.
function normalizeReliability(reliability) {
  return {
    races: Number(reliability?.races ?? 0),
    finished: Number(reliability?.finished ?? 0),
    finishRate: Number(reliability?.finish_rate ?? reliability?.finishRate ?? 0),
    groupFinishRate: Number(reliability?.group_finish_rate ?? reliability?.groupFinishRate ?? 0),
    mechanical: Number(reliability?.mechanical ?? 0),
    driverError: Number(reliability?.driver_error ?? reliability?.driverError ?? 0),
    other: Number(reliability?.other ?? 0),
    worstPart: reliability?.worst_part ?? reliability?.worstPart ?? "",
  };
}

function normalizeTeamHistoryPayload(payload) {
  if (!payload) return null;
  const sport = payload.sport ?? {};
  const identity = payload.identity ?? {};
  const rival = identity.rival ?? {};
  const management = payload.management ?? {};
  return {
    recordScope: payload.record_scope ?? payload.recordScope ?? i18n.t("myTeamTab.history.defaults.recordScope"),
    hasHistory: Boolean(payload.has_history ?? payload.hasHistory),
    records: (payload.records ?? []).map((record) => ({
      // Identificador estável da métrica; o v2 escolhe ícone e layout por ele.
      id: record.id ?? "",
      label: record.label,
      rank: record.rank,
      value: String(record.value),
      // Campos que só o v2 desenha (barra de posição e média do grupo). O v1
      // ignora — carregá-los aqui mantém uma única normalização do payload.
      rankPosition: Number(record.rank_position ?? record.rankPosition ?? 0),
      rankTotal: Number(record.rank_total ?? record.rankTotal ?? 0),
      groupAverage: record.group_average ?? record.groupAverage ?? "",
    })),
    sport: {
      seasons: sport.seasons ?? i18n.t("myTeamTab.history.defaults.noSeasons"),
      currentStreak: sport.current_streak ?? sport.currentStreak ?? i18n.t("myTeamTab.history.defaults.noStreak"),
      bestStreak: sport.best_streak ?? sport.bestStreak ?? i18n.t("myTeamTab.history.defaults.noStreak"),
      podiumRate: sport.podium_rate ?? sport.podiumRate ?? "0%",
      winRate: sport.win_rate ?? sport.winRate ?? "0%",
      races: sport.races ?? 0,
      wins: sport.wins ?? 0,
      podiums: sport.podiums ?? 0,
    },
    timeline: payload.timeline ?? [],
    titleCategories: (payload.title_categories ?? payload.titleCategories ?? []).map((item) => ({
      category: item.category ?? "",
      year: String(item.year ?? ""),
      color: item.color ?? "",
      // Campos que só o v2 desenha: a galeria dele conta como o título foi
      // ganho e quem pilotava, não só em que ano.
      categoryId: item.category_id ?? item.categoryId ?? "",
      points: String(item.points ?? ""),
      wins: Number(item.wins ?? 0),
      championDriver: item.champion_driver ?? item.championDriver ?? "",
      championTeam: item.champion_team ?? item.championTeam ?? "",
      championIsTeam: Boolean(item.champion_is_team ?? item.championIsTeam ?? false),
    })),
    categoryPath: (payload.category_path ?? payload.categoryPath ?? []).map((step) => ({
      category: step.category,
      years: step.years,
      detail: step.detail,
      color: step.color,
      movement: step.movement ?? "same",
    })),
    worldFirstYear: Number(payload.world_first_year ?? payload.worldFirstYear ?? 0),
    worldLastYear: Number(payload.world_last_year ?? payload.worldLastYear ?? 0),
    movement: payload.movement
      ? {
          promotions: payload.movement.promotions ?? 0,
          relegations: payload.movement.relegations ?? 0,
          timeByCategory: payload.movement.time_by_category ?? payload.movement.timeByCategory ?? "",
          bestCategory: payload.movement.best_category ?? payload.movement.bestCategory ?? i18n.t("myTeamTab.history.defaults.dash"),
          hardestCategory: payload.movement.hardest_category ?? payload.movement.hardestCategory ?? i18n.t("myTeamTab.history.defaults.dash"),
        }
      : null,
    ownershipEvents: (payload.ownership_events ?? payload.ownershipEvents ?? []).map((event) => ({
      year: String(event.year ?? ""),
      eventType: event.event_type ?? event.eventType ?? "sale",
      title: event.title ?? i18n.t("myTeamTab.history.defaults.newBoard"),
      detail: event.detail ?? "",
      financialNote: event.financial_note ?? event.financialNote ?? "",
    })),
    highlights: (payload.highlights ?? []).map((item) => ({
      label: item.label,
      value: String(item.value ?? ""),
      detail: item.detail ?? "",
    })),
    milestones: (payload.milestones ?? []).map((item) => ({
      label: item.label,
      year: String(item.year ?? ""),
      // Identidade do fato, para o v2 fundir marcos e linha do tempo sem casar
      // prosa traduzida. Ver TeamHistoryMilestone::kind no backend.
      kind: item.kind ?? "",
    })),
    seasonResults: (payload.season_results ?? payload.seasonResults ?? []).map((item) => ({
      year: String(item.year ?? ""),
      category: item.category ?? "",
      // Id cru da categoria — o rótulo acima é traduzido e não serve de chave na
      // paleta de categorias. Só o v2 usa, na faixa de pódios por corrida.
      categoryId: item.category_id ?? item.categoryId ?? "",
      position: String(item.position ?? "—"),
      wins: item.wins ?? 0,
      podiums: item.podiums ?? 0,
      points: String(item.points ?? "0"),
      // Denominador das taxas por temporada e os degraus do pódio — só o v2
      // desenha (ver a faixa de pódios por corrida em v2/TeamHistoryDrawerV2.jsx).
      races: Number(item.races ?? 0),
      seconds: Number(item.seconds ?? 0),
      thirds: Number(item.thirds ?? 0),
      fourths: Number(item.fourths ?? 0),
      fifths: Number(item.fifths ?? 0),
    })),
    // Fita de forma recente e distribuição por faixa de colocação — só o v2
    // desenha (ver a aba Esportivo em v2/TeamHistoryDrawerV2.jsx).
    recentForm: (payload.recent_form ?? payload.recentForm ?? []).map((item) => ({
      year: String(item.year ?? ""),
      round: Number(item.round ?? 0),
      category: item.category ?? "",
      categoryId: item.category_id ?? item.categoryId ?? "",
      position: item.position ?? null,
    })),
    resultSpread: normalizeResultSpread(payload.result_spread ?? payload.resultSpread),
    // Campanha do campeonato rodada a rodada, com a linha de todas as equipes —
    // só o v2 desenha. `null` quando o backend não tem recorte para mandar
    // (equipe com uma corrida só na última temporada, ou save anterior ao
    // campo), e aí o v2 cai na curva de posição por temporada.
    championshipRun: normalizeChampionshipRun(payload.championship_run ?? payload.championshipRun),
    lineup: (payload.lineup ?? []).map((item) => ({
      slot: Number(item.slot ?? 0),
      driverId: item.driver_id ?? item.driverId ?? "",
      name: item.name ?? "",
      nationality: item.nationality ?? "",
      firstYear: String(item.first_year ?? item.firstYear ?? ""),
      lastYear: String(item.last_year ?? item.lastYear ?? ""),
      races: Number(item.races ?? 0),
      wins: Number(item.wins ?? 0),
      podiums: Number(item.podiums ?? 0),
      bestPosition: Number(item.best_position ?? item.bestPosition ?? 0),
      currentTeamName: item.current_team_name ?? item.currentTeamName ?? "",
      currentTeamColor: item.current_team_color ?? item.currentTeamColor ?? "",
      isPlayer: Boolean(item.is_player ?? item.isPlayer ?? false),
      stillHere: Boolean(item.still_here ?? item.stillHere ?? false),
      currentLabel: item.current_label ?? item.currentLabel ?? "",
    })),
    reliability: normalizeReliability(payload.reliability),
    outsideScopeSeasons: (payload.outside_scope_seasons ?? payload.outsideScopeSeasons ?? []).map((item) => ({
      year: String(item.year ?? ""),
      category: item.category ?? "",
      categoryId: item.category_id ?? item.categoryId ?? "",
    })),
    identity: {
      origin: identity.origin ?? i18n.t("myTeamTab.history.defaults.noOrigin"),
      current: identity.current ?? i18n.t("myTeamTab.history.defaults.noCurrentCategory"),
      heritage: identity.heritage ?? null,
      profile: identity.profile ?? i18n.t("myTeamTab.history.defaults.profileForming"),
      summary: identity.summary ?? i18n.t("myTeamTab.history.defaults.identityInsufficient"),
      rival: {
        name: rival.name ?? i18n.t("myTeamTab.history.defaults.noRival"),
        currentCategory: rival.current_category ?? rival.currentCategory ?? i18n.t("myTeamTab.history.defaults.noCurrentCategory"),
        note: rival.note ?? i18n.t("myTeamTab.history.defaults.noRivalry"),
      },
      symbolDriver: identity.symbol_driver ?? identity.symbolDriver ?? i18n.t("myTeamTab.history.defaults.noSymbolDriver"),
      symbolDriverDetail: identity.symbol_driver_detail ?? identity.symbolDriverDetail ?? i18n.t("myTeamTab.history.defaults.insufficientResults"),
    },
    management: {
      operationHealth: management.operation_health ?? management.operationHealth ?? i18n.t("myTeamTab.history.defaults.monitored"),
      peakCash: management.peak_cash ?? management.peakCash ?? i18n.t("myTeamTab.history.defaults.noBalance"),
      worstCrisis: management.worst_crisis ?? management.worstCrisis ?? i18n.t("myTeamTab.history.defaults.noCrisis"),
      healthyYears: management.healthy_years ?? management.healthyYears ?? i18n.t("myTeamTab.history.defaults.noSeasons"),
      efficiency: management.efficiency ?? i18n.t("myTeamTab.history.defaults.efficiencyZero"),
      biggestInvestment: management.biggest_investment ?? management.biggestInvestment ?? i18n.t("myTeamTab.history.defaults.noInvestment"),
      summary: management.summary ?? i18n.t("myTeamTab.history.defaults.managementUnread"),
      peakCashDetail: management.peak_cash_detail ?? management.peakCashDetail ?? i18n.t("myTeamTab.history.defaults.noBalanceDetail"),
      worstCrisisDetail: management.worst_crisis_detail ?? management.worstCrisisDetail ?? i18n.t("myTeamTab.history.defaults.noCrisisDetail"),
      healthyYearsDetail: management.healthy_years_detail ?? management.healthyYearsDetail ?? i18n.t("myTeamTab.history.defaults.noHealthDetail"),
      efficiencyDetail: management.efficiency_detail ?? management.efficiencyDetail ?? i18n.t("myTeamTab.history.defaults.noEfficiencyDetail"),
      investmentDetail: management.investment_detail ?? management.investmentDetail ?? i18n.t("myTeamTab.history.defaults.noInvestmentDetail"),
    },
  };
}

// Ano de fundação REAL (payload do backend `founded_year`). Sem valor confiável → null,
// e o front mostra estado neutro em vez de inventar um ano.
function resolveTeamFoundedYear(team) {
  const explicitYear = Number(team?.founded_year ?? team?.ano_fundacao);
  if (Number.isFinite(explicitYear) && explicitYear > 1800) {
    return explicitYear;
  }
  return null;
}

function teamHeritageLabel(founded) {
  if (!founded) return i18n.t("myTeamTab.history.heritage.team");
  if (founded <= 1970) return i18n.t("myTeamTab.history.heritage.historic");
  return i18n.t("myTeamTab.history.heritage.consolidated");
}

function emptyRealSport() {
  return {
    seasons: i18n.t("myTeamTab.history.defaults.loadingReal"),
    currentStreak: i18n.t("myTeamTab.history.defaults.loadingReal"),
    bestStreak: i18n.t("myTeamTab.history.defaults.loadingReal"),
    podiumRate: "0%",
    winRate: "0%",
    races: 0,
    wins: 0,
    podiums: 0,
  };
}

// Fallback NEUTRO de gestão: usa só o estado financeiro atual (real); o resto fica "—"
// até o histórico real (get_team_history_dossier) chegar. Sem estimativas fabricadas.
function emptyRealManagement(team) {
  const debt = team?.debt_balance ?? 0;
  return {
    operationHealth: financialState(team?.financial_state),
    peakCash: i18n.t("myTeamTab.history.defaults.dash"),
    worstCrisis: debt > 0 ? i18n.t("myTeamTab.history.defaults.currentDebt", { value: formatMoney(debt) }) : i18n.t("myTeamTab.history.defaults.noRelevantDebt"),
    healthyYears: i18n.t("myTeamTab.history.defaults.dash"),
    efficiency: i18n.t("myTeamTab.history.defaults.dash"),
    biggestInvestment: i18n.t("myTeamTab.history.defaults.dash"),
    summary: i18n.t("myTeamTab.history.defaults.managementUnconsolidated"),
    peakCashDetail: i18n.t("myTeamTab.history.defaults.noBalanceHistoryDetail"),
    worstCrisisDetail: debt > 0 ? i18n.t("myTeamTab.history.defaults.currentLiability") : i18n.t("myTeamTab.history.defaults.noCrisisRegistered"),
    healthyYearsDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
    efficiencyDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
    investmentDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
  };
}

function findHistoricRival(team, teams) {
  return [...(teams ?? [])]
    .filter((entry) => entry.id !== team?.id)
    .sort((a, b) => Math.abs((a.posicao ?? 99) - (team?.posicao ?? 99)) - Math.abs((b.posicao ?? 99) - (team?.posicao ?? 99)))[0];
}

function categoryGroupLabel(category) {
  if (category?.includes("mazda")) return i18n.t("myTeamTab.history.groups.mazda");
  if (category?.includes("toyota")) return i18n.t("myTeamTab.history.groups.toyota");
  if (category === "bmw_m2") return i18n.t("myTeamTab.history.groups.bmw");
  if (category === "gt4") return i18n.t("myTeamTab.history.groups.gt4");
  if (category === "gt3") return i18n.t("myTeamTab.history.groups.gt3");
  if (category === "lmp2") return i18n.t("myTeamTab.history.groups.lmp2");
  if (category === "endurance") return i18n.t("myTeamTab.history.groups.endurance");
  return i18n.t("myTeamTab.history.groups.default");
}

// Exportada pelo mesmo motivo do dossiê: o v2 pinta a saúde da operação com a
// MESMA regra, e duas cópias divergiriam na primeira palavra nova.
export function operationHealthTone(label) {
  const normalized = String(label ?? "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();

  if (normalized.includes("pressionada") || normalized.includes("critica") || normalized.includes("crise") || normalized.includes("colapso")) {
    return {
      card: "border-status-red/30 bg-[#241014]/95 bg-[radial-gradient(circle_at_12%_10%,rgba(255,103,103,0.14),transparent_12rem),linear-gradient(145deg,rgba(45,16,21,0.96),rgba(7,16,29,0.99))]",
      text: "text-status-red",
    };
  }

  if (normalized.includes("estavel") || normalized.includes("monitorada")) {
    return {
      card: "border-status-yellow/30 bg-[#201a0b]/95 bg-[radial-gradient(circle_at_12%_10%,rgba(242,196,109,0.14),transparent_12rem),linear-gradient(145deg,rgba(35,29,12,0.96),rgba(7,16,29,0.99))]",
      text: "text-status-yellow",
    };
  }

  return {
    card: "border-status-green/30 bg-[#0b1d19] bg-[radial-gradient(circle_at_12%_10%,rgba(94,231,168,0.14),transparent_12rem),linear-gradient(145deg,rgba(12,35,30,0.96),rgba(7,16,29,0.99))]",
    text: "text-status-green",
  };
}
