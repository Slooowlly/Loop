import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../team/TeamLogoMark";
import TeamDriverRow from "./TeamDriverRow";
import { getRankStyle, getTeamMovementBadge } from "../preSeasonFormatters.js";

// Card de time do grid central (reutilizado no fluxo normal e nas sub-classes
// de Production/Endurance).
export default function TeamGridCard({ team, accent, hoveredFreeAgentCat, onOpenHistory }) {
  const { t } = useTranslation();
  const rankStyle = getRankStyle(team.temp_posicao);
  const movement = getTeamMovementBadge(team.categoria_anterior, team._categoria || team.classe);
  const teamLogoFallback = movement?.color ?? team.cor_primaria ?? accent;
  // Conexão entre colunas: passar o mouse sobre um piloto livre acende as equipes
  // da mesma categoria (por championship OU por classe) e esmaece as demais.
  const matchesHover = hoveredFreeAgentCat != null
    && (team._categoria === hoveredFreeAgentCat || team.classe === hoveredFreeAgentCat);
  const isDimmed = hoveredFreeAgentCat != null && !matchesHover;
  return (
    <article
      onDoubleClick={() => onOpenHistory?.(team)}
      title={t("preSeason.teamHistoryDblClick")}
      className="glass transition-glass relative cursor-pointer select-none overflow-hidden rounded-xl border p-3 hover:-translate-y-0.5 hover:scale-[1.01]"
      style={{
        borderColor: matchesHover
          ? accent
          : movement
            ? movement.border
            : rankStyle?.border
              ? `${rankStyle.border}88`
              : "rgba(255,255,255,0.11)",
        opacity: isDimmed ? 0.32 : 1,
        boxShadow: matchesHover ? `0 0 0 1px ${accent}, 0 10px 34px -14px ${accent}` : undefined,
        transition: "opacity .16s ease, border-color .16s ease, box-shadow .16s ease, transform .16s ease",
      }}
    >
      {rankStyle && !movement && (
        <div
          className="pointer-events-none absolute right-0 top-0 h-full w-28"
          style={{ background: `radial-gradient(circle at 94% 14%, ${rankStyle.glow} 0%, transparent 68%)` }}
        />
      )}
      {movement && (
        <div
          className="pointer-events-none absolute right-0 top-0 h-full w-32"
          style={{ background: `radial-gradient(circle at 94% 14%, ${movement.bg.replace("0.12", "0.18")} 0%, transparent 68%)` }}
        />
      )}

      <div className="relative mb-3 flex items-start gap-3">
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <TeamLogoMark
            teamName={team.nome}
            color={teamLogoFallback}
            size="md"
            testId="preseason-team-logo"
          />
          <div className="min-w-0 flex-1">
            <p className="truncate text-[19px] font-bold leading-[1.05]">{team.nome}</p>
          </div>
          {movement && (
            <span
              className="ml-auto shrink-0 rounded-md border px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em]"
              style={{ color: movement.color, backgroundColor: movement.bg, borderColor: movement.border }}
            >
              {movement.kind ? t(`preSeason.teamMovement.${movement.kind}`) : movement.label}
            </span>
          )}
        </div>
      </div>

      <div className="relative divide-y divide-white/8">
        <TeamDriverRow
          driverName={team.piloto_1_nome}
          tenureSeasons={team.piloto_1_tenure_seasons}
          accent={accent}
          isPrimarySlot
        />
        <TeamDriverRow
          driverName={team.piloto_2_nome}
          tenureSeasons={team.piloto_2_tenure_seasons}
          accent={accent}
        />
      </div>
    </article>
  );
}
