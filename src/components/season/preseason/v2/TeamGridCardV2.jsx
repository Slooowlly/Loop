import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../../team/TeamLogoMark";
import Tooltip from "../../../ui/Tooltip";
import DriverMiniCard from "../../../driver/DriverMiniCard";
import TeamMiniCard from "../../../team/TeamMiniCard";
import {
  getRankStyle,
  getTeamMovementBadge,
  formatTenureCounter,
} from "../../preSeasonFormatters.js";

// Uma linha de assento.
//
// Sobre o peso de cada estado: na semana da fotografia QUASE TODO contrato vence.
// Marcar isso com uma etiqueta em cada linha marcava o normal — vinte chips âmbar
// numa tela só, que o olho aprende a ignorar em dois segundos. O contrato vencendo
// virou o tom do próprio contador de tempo de casa: mesma informação, sem um objeto
// novo na tela. Assento livre e aposentadoria continuam altos, porque esses sim são
// a exceção — são eles que abrem lugar.
//
// Os pips de tempo de casa ficam GRUDADOS no número de anos. Do outro lado da linha
// eles eram cinco bolinhas sem legenda: ninguém liga um ao outro atravessando um nome.
function SeatRow({
  driverId,
  driverName,
  tenureSeasons,
  contratoVence,
  aposentado,
  accent,
  isPrimarySlot,
}) {
  const { t } = useTranslation();

  if (!driverName) {
    return (
      <div
        data-testid="preseason-seat-open"
        className="mt-1 flex items-center gap-2 rounded-lg border border-dashed px-3 py-2"
        style={{
          borderColor: "rgba(63,185,80,0.45)",
          background: "rgba(63,185,80,0.10)",
        }}
      >
        <span className="text-[14px] font-bold leading-none text-[color:var(--status-green)] opacity-80">+</span>
        <span className="text-[12px] font-bold uppercase tracking-[0.08em] text-[color:var(--status-green)]">
          {t("preSeason.roster.openSlot")}
        </span>
      </div>
    );
  }

  const tenureCounter = formatTenureCounter(tenureSeasons);
  const pipCount = Math.min(Math.max(tenureSeasons ?? 0, 0), 5);
  // Aposentadoria decide o assento sozinha — vem antes do contrato vencendo.
  const tenureColor = aposentado
    ? "var(--status-red)"
    : contratoVence
      ? "var(--status-yellow)"
      : "var(--text-muted)";

  const row = (
    <div className="flex items-center gap-2 border-t border-white/[0.08] py-2.5">
      <DriverMiniCard driverId={driverId}>
        <p
          className={`min-w-0 flex-1 truncate leading-[1.1] ${
            isPrimarySlot ? "text-[15px] font-bold" : "text-[14px] font-semibold"
          }`}
        >
          {driverName}
        </p>
      </DriverMiniCard>
      {aposentado && (
        <span className="shrink-0 text-[12px] leading-none" aria-label={t("preSeason.roster.retiring")}>
          {"\u{1F6AA}"}
        </span>
      )}
      {tenureCounter && (
        <span className="flex shrink-0 items-center gap-2">
          <span className="flex items-center gap-[3px]" aria-hidden="true">
            {Array.from({ length: 5 }).map((_, i) => (
              <span
                key={i}
                className="h-1.5 w-1.5 rounded-full"
                style={{ background: i < pipCount ? accent : "rgba(255,255,255,0.14)" }}
              />
            ))}
          </span>
          <span
            className="text-[11px] font-semibold tabular-nums"
            style={{ color: tenureColor }}
          >
            {tenureCounter.label}
          </span>
        </span>
      )}
    </div>
  );

  if (!aposentado && !contratoVence) return row;

  return (
    <Tooltip
      texto={aposentado ? t("preSeason.roster.retiring") : t("preSeason.roster.contractExpiring")}
    >
      {row}
    </Tooltip>
  );
}

export default function TeamGridCardV2({
  team,
  accent,
  hoveredFreeAgentCat,
  onOpenHistory,
}) {
  const { t } = useTranslation();
  const rankStyle = getRankStyle(team.temp_posicao);
  const movement = getTeamMovementBadge(team.categoria_anterior, team._categoria || team.classe);
  const teamLogoFallback = movement?.color ?? team.cor_primaria ?? accent;
  const matchesHover = hoveredFreeAgentCat != null
    && (team._categoria === hoveredFreeAgentCat || team.classe === hoveredFreeAgentCat);
  const isDimmed = hoveredFreeAgentCat != null && !matchesHover;
  // Títulos e vitórias NA CATEGORIA em que a equipe corre agora. O acumulado de
  // carreira misturava escalões: cinco títulos de Mazda Rookie apareciam no grid
  // do GT3 como se dissessem algo sobre o assento que o jogador está olhando.
  const titles = team.categoria_titulos ?? 0;
  const wins = team.categoria_vitorias ?? 0;
  // Só assento VAZIO acende a moldura. Incluir aposentadoria aqui pintava metade
  // do grid de verde na semana da fotografia, e um destaque que vale para metade
  // dos cards não destaca nada.
  const hasOpenSeat = !team.piloto_1_nome || !team.piloto_2_nome;

  return (
    <Tooltip texto={t("preSeason.teamHistoryDblClick")}>
      <article
        data-testid="preseason-team-card-v2"
        onDoubleClick={() => onOpenHistory?.(team)}
        className="glass transition-glass relative cursor-pointer select-none overflow-hidden rounded-xl border px-4 pb-3 pt-3.5 hover:-translate-y-0.5 hover:scale-[1.01]"
        style={{
          borderColor: matchesHover
            ? accent
            : hasOpenSeat
              ? "rgba(63,185,80,0.4)"
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

        <div className="relative flex items-center gap-3">
          {team.temp_posicao > 0 && (
            <span
              className="grid h-7 w-7 shrink-0 place-items-center rounded-lg text-[12px] font-black tabular-nums"
              style={{
                color: rankStyle?.border ?? "var(--text-secondary)",
                background: rankStyle ? `${rankStyle.border}22` : "rgba(255,255,255,0.07)",
                border: rankStyle ? `1px solid ${rankStyle.border}55` : "1px solid transparent",
              }}
            >
              {team.temp_posicao}
            </span>
          )}
          <TeamMiniCard team={team}>
            <TeamLogoMark
              teamName={team.nome}
              color={teamLogoFallback}
              size="md"
              testId="preseason-team-logo"
            />
          </TeamMiniCard>
          <div className="min-w-0 flex-1">
            <p className="truncate text-[18px] font-bold leading-[1.05]">{team.nome}</p>
            {/* Linha de identidade: quando a equipe existe e o que ela fez AQUI. */}
            <p className="mt-1 flex items-center gap-2 truncate text-[9.5px] text-[color:var(--text-muted)]">
              {team.founded_year > 0 && (
                <span>{t("preSeason.v2.grid.founded", { year: team.founded_year })}</span>
              )}
              {titles > 0 && (
                <span className="font-bold" style={{ color: "#ffd700" }}>
                  {t("preSeason.v2.grid.titles", { count: titles })}
                </span>
              )}
              {wins > 0 && (
                <span className="font-semibold">{t("preSeason.v2.grid.wins", { count: wins })}</span>
              )}
              {titles === 0 && wins === 0 && (
                <span className="italic">{t("preSeason.v2.grid.noRecord")}</span>
              )}
            </p>
          </div>
          {movement && (
            <span
              className="shrink-0 rounded-md border px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em]"
              style={{ color: movement.color, backgroundColor: movement.bg, borderColor: movement.border }}
            >
              {movement.kind ? t(`preSeason.teamMovement.${movement.kind}`) : movement.label}
            </span>
          )}
        </div>

        <div className="relative mt-2.5">
          <SeatRow
            driverId={team.piloto_1_id}
            driverName={team.piloto_1_nome}
            tenureSeasons={team.piloto_1_tenure_seasons}
            contratoVence={team.piloto_1_contrato_vence}
            aposentado={team.piloto_1_aposentado}
            accent={accent}
            isPrimarySlot
          />
          <SeatRow
            driverId={team.piloto_2_id}
            driverName={team.piloto_2_nome}
            tenureSeasons={team.piloto_2_tenure_seasons}
            contratoVence={team.piloto_2_contrato_vence}
            aposentado={team.piloto_2_aposentado}
            accent={accent}
          />
        </div>
      </article>
    </Tooltip>
  );
}
