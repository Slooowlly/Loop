import { useTranslation } from "react-i18next";
import { formatSalaryAnnual } from "../../../utils/formatters";
import TeamLogoMark from "../../team/TeamLogoMark";

export default function ProposalCard({ proposal: p, isAdvancingWeek, onRespond }) {
  const { t } = useTranslation();
  return (
    <article className="glass animate-scale-in rounded-xl px-4 py-3.5">
      <div className="flex min-w-0 items-center gap-3">
        <TeamLogoMark
          teamName={p.equipe_nome}
          color={p.equipe_cor_primaria}
          size="md"
          testId="player-proposal-team-logo"
        />
        <div className="min-w-0 flex-1">
          <p
            className="text-body-sm font-bold uppercase tracking-[0.16em]"
            style={{ color: p.equipe_cor_primaria }}
          >
            {p.papel} | {p.categoria_nome}
          </p>
          <p className="mt-1 truncate text-title-md">{p.equipe_nome}</p>
        </div>
        {p.semanas_restantes != null && (
          <span className="shrink-0 rounded-full border border-amber-400/30 bg-amber-400/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.14em] text-amber-300">
            {p.semanas_restantes <= 0 ? t("preSeason.proposals.lastWeek") : t("preSeason.proposals.expiresIn", { count: p.semanas_restantes })}
          </span>
        )}
      </div>

      <div className="my-3 grid grid-cols-2 gap-2">
        <div className="glass-light rounded-lg p-2.5">
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            {t("preSeason.proposals.salary")}
          </p>
          <p className="num-medium mt-0.5 font-bold text-[color:var(--status-green)]">
            {formatSalaryAnnual(p.salario_oferecido)}
          </p>
        </div>
        <div className="glass-light rounded-lg p-2.5">
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            {t("preSeason.proposals.duration")}
          </p>
          <p className="num-medium mt-0.5 font-bold text-[color:var(--text-primary)]">
            {t("preSeason.proposals.years", { count: p.duracao_anos })}
          </p>
        </div>
        {p.companheiro_nome && (
          <div className="glass-light rounded-lg p-2.5">
            <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
              {t("preSeason.proposals.teammate")}
            </p>
            <p className="text-body mt-0.5 font-semibold text-[color:var(--text-primary)] truncate">
              {p.companheiro_nome}
              {p.companheiro_skill != null ? ` (${p.companheiro_skill})` : ""}
            </p>
          </div>
        )}
        <div className={`glass-light rounded-lg p-2.5 ${p.companheiro_nome ? "" : "col-span-2"}`}>
          <p className="text-[8px] uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
            {t("preSeason.proposals.car")}
          </p>
          <div className="mt-1.5 flex items-center gap-2">
            <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[#21262d]">
              <div
                className="h-full rounded-full"
                style={{
                  width: `${p.car_performance_rating ?? 0}%`,
                  backgroundColor: p.equipe_cor_primaria,
                }}
              />
            </div>
            <span className="text-body font-bold">{p.car_performance_rating}</span>
          </div>
        </div>
      </div>

      <div className="flex gap-2">
        <button
          onClick={() =>
            onRespond(
              p.proposal_id,
              true,
              p.equipe_cor_primaria,
              p.categoria,
              p.equipe_nome,
            )
          }
          disabled={isAdvancingWeek}
          className="transition-glass glow-blue flex-1 rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-3 py-2 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-50"
        >
          {t("preSeason.proposals.accept")}
        </button>
        <button
          onClick={() => onRespond(p.proposal_id, false)}
          disabled={isAdvancingWeek}
          className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-body font-semibold text-[color:var(--text-secondary)] hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {t("preSeason.proposals.decline")}
        </button>
      </div>
    </article>
  );
}
