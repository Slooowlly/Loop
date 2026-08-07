import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import TeamLogoMark from "../../team/TeamLogoMark";
import Tooltip from "../../ui/Tooltip";
import { formatLastChampionshipResult } from "../preSeasonFormatters.js";
import useDisplacedContext from "./useDisplacedContext.js";

// Modal: Pilotos sem vaga (fim da pré-temporada).
//
// É uma tela de CONSEQUÊNCIA, não de dados: o jogador já não pode fazer nada por
// esses pilotos, ele só precisa saber o que aconteceu antes de clicar em iniciar.
// Por isso cada piloto cabe numa linha — a versão anterior gastava uma ficha com
// borda, sombra e três linhas por nome, mais o rótulo "Ex-equipe" repetido em
// todas elas para uma relação que a logo e a cor da equipe já contam, mais um
// selo de licença que vinha igual em quase todo mundo.
//
// O que a tela não dizia e agora diz: eles não correm a temporada. `vacancies.rs`
// zera a `categoria_atual` de quem sobra, então esses pilotos saem da escada e só
// voltam na próxima janela.

// Mesmo vocabulário do RivalMarker, que decora os nomes no resto do jogo.
const RIVAL_ICON = { nemesis: "\u{1F4A5}", rival: "\u{1F525}" };

function DisplacedRow({ driver, context, isPlayerTeam }) {
  const { t } = useTranslation();
  const result = formatLastChampionshipResult(driver);
  const teamColor = driver.previous_team_color ?? "var(--text-secondary)";

  const sharedRaces = context?.shared_races ?? 0;
  const playerAhead = context?.player_ahead ?? 0;
  const driverAhead = context?.driver_ahead ?? 0;
  // Verde/vermelho só quando o placar tem lado. Empate fica neutro — pintar um
  // 4–4 de alguma cor seria inventar um vencedor.
  const headToHeadColor =
    playerAhead > driverAhead
      ? "var(--status-green)"
      : driverAhead > playerAhead
        ? "var(--status-red)"
        : "var(--text-muted)";
  const rivalRole = context?.rival_role ?? null;

  return (
    <div
      data-testid="displaced-driver-row"
      className={`grid grid-cols-[auto_1fr_auto] items-center gap-3 rounded-lg py-2 pr-2.5 ${
        isPlayerTeam ? "bg-[#58a6ff14] pl-2.5 shadow-[inset_2px_0_0_var(--accent-primary)]" : "pl-1"
      }`}
    >
      {driver.previous_team_name ? (
        <TeamLogoMark
          teamName={driver.previous_team_name}
          color={driver.previous_team_color}
          size="xs"
          testId="displaced-driver-previous-team-logo"
        />
      ) : (
        <span className="h-6 w-[36px]" />
      )}

      <div className="min-w-0">
        <p className="flex items-center gap-1.5 truncate text-[14.5px] font-bold leading-[1.15] text-[color:var(--text-primary)]">
          {driver.driver_name}
          {rivalRole && (
            <Tooltip texto={t(`preSeason.displaced.role.${rivalRole}`)}>
              <span
                data-testid="displaced-driver-rival-role"
                aria-label={t(`preSeason.displaced.role.${rivalRole}`)}
                className="shrink-0 text-[12px] leading-none"
              >
                {RIVAL_ICON[rivalRole]}
              </span>
            </Tooltip>
          )}
        </p>
        {driver.previous_team_name && (
          <p className="mt-0.5 truncate text-[11px] text-[color:var(--text-muted)]">
            <span className="font-semibold" style={{ color: teamColor }}>
              {driver.previous_team_name}
            </span>
            {driver.seasons_at_last_team > 0 && (
              <>
                <span className="mx-1.5 opacity-40">·</span>
                {t("preSeason.displaced.seasonsAtTeam", { count: driver.seasons_at_last_team })}
              </>
            )}
          </p>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-3">
        {isPlayerTeam && (
          <span className="rounded-md bg-[#58a6ff2e] px-1.5 py-0.5 text-[9px] font-black uppercase tracking-[0.1em] text-[color:var(--accent-primary)]">
            {t("preSeason.displaced.yourTeamBadge")}
          </span>
        )}
        {/* Resultado do campeonato e, embaixo, o retrospecto contra o jogador.
            Só aparece para quem realmente dividiu grid com ele — para os outros a
            linha continua exatamente como era. */}
        <div className="flex flex-col items-end leading-[1.1]">
          {result && <span className="text-[13px] font-extrabold tabular-nums">{result}</span>}
          {sharedRaces > 0 && (
            <Tooltip
              texto={t("preSeason.displaced.headToHeadTooltip", {
                count: sharedRaces,
                player: playerAhead,
                rival: driverAhead,
              })}
            >
              <span
                data-testid="displaced-driver-head-to-head"
                className="mt-0.5 text-[9.5px] font-bold tabular-nums"
                style={{ color: headToHeadColor }}
              >
                {t("preSeason.displaced.headToHead", { player: playerAhead, rival: driverAhead })}
              </span>
            </Tooltip>
          )}
        </div>
      </div>
    </div>
  );
}

export default function DisplacedDriversModal({
  groups,
  totalCount,
  playerTeamName,
  careerId,
  onClose,
  onConfirm,
}) {
  const { t } = useTranslation();

  const driverIds = useMemo(
    () => groups.flatMap((group) => group.drivers.map((driver) => driver.driver_id)),
    [groups],
  );
  const contextByDriver = useDisplacedContext(careerId, driverIds);

  // Dos N pilotos que sobraram, os que saíram da equipe do jogador são os únicos
  // que mudam alguma coisa para ele. No meio da lista eles não têm destaque nenhum.
  const fromPlayerTeam = useMemo(() => {
    if (!playerTeamName) return [];
    return groups
      .flatMap((group) => group.drivers)
      .filter((driver) => driver.previous_team_name === playerTeamName);
  }, [groups, playerTeamName]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="glass-strong animate-fade-in mx-4 w-full max-w-4xl rounded-2xl p-6 md:p-7">
        <div className="text-[9px] font-black uppercase tracking-[0.22em] text-[#f85149]">
          {t("preSeason.displaced.eyebrow")}
        </div>
        <h2 className="mt-1.5 text-[23px] font-bold leading-[1.12] tracking-[-0.015em] text-[color:var(--text-primary)]">
          {t("preSeason.displaced.title", { count: totalCount })}
        </h2>
        <p className="mt-1.5 max-w-[560px] text-body leading-[1.5] text-[color:var(--text-secondary)]">
          {t("preSeason.displaced.lead", { count: totalCount })}
        </p>

        {fromPlayerTeam.length > 0 && (
          <div
            data-testid="displaced-player-team-flag"
            className="mt-3.5 flex items-center gap-3 rounded-xl border border-[#58a6ff4d] bg-gradient-to-br from-[#58a6ff24] to-[#58a6ff0a] px-3.5 py-2.5"
          >
            <span className="text-[15px] leading-none text-[color:var(--accent-primary)]">◆</span>
            <span className="text-body leading-[1.4] text-[color:var(--text-secondary)]">
              {t("preSeason.displaced.yourTeam", {
                count: fromPlayerTeam.length,
                drivers: fromPlayerTeam.map((d) => d.driver_name).join(", "),
                team: playerTeamName,
              })}
            </span>
          </div>
        )}

        <div className="my-4 max-h-[64vh] space-y-4 overflow-y-auto pr-1">
          {groups.map((group) => (
            <section
              key={group.category}
              className="rounded-sm border-l-[3px] pl-3"
              style={{ borderColor: group.color }}
            >
              {/* Espinha colorida no lugar da barra com caixa, gradiente e régua:
                  a cor já é o rótulo, o resto era moldura. */}
              <div className="mb-1.5 flex items-baseline gap-2.5">
                <span
                  className="text-[11px] font-black uppercase tracking-[0.16em]"
                  style={{ color: group.color }}
                >
                  {group.label}
                </span>
                <span className="text-[10.5px] font-semibold tabular-nums text-[color:var(--text-muted)]">
                  {t("preSeason.displaced.groupCount", { count: group.drivers.length })}
                </span>
              </div>

              <div className="divide-y divide-white/[0.055]">
                {group.drivers.map((driver) => (
                  <DisplacedRow
                    key={driver.driver_id}
                    driver={driver}
                    context={contextByDriver[driver.driver_id]}
                    isPlayerTeam={
                      Boolean(playerTeamName) && driver.previous_team_name === playerTeamName
                    }
                  />
                ))}
              </div>
            </section>
          ))}
        </div>

        {/* Ação e saída, não duas opções de mesmo peso. */}
        <div className="flex gap-3">
          <button
            onClick={onClose}
            className="transition-glass flex-1 rounded-xl border border-white/15 bg-white/5 px-4 py-2.5 text-body font-semibold text-[color:var(--text-secondary)] hover:bg-white/10"
          >
            {t("preSeason.actions.back")}
          </button>
          <button
            onClick={onConfirm}
            className="transition-glass glow-blue flex-[1.8] rounded-xl border border-[#3fb95099] bg-[#3fb950] px-4 py-2.5 text-body font-bold text-[#06101f] hover:bg-[#52d16a]"
          >
            {t("preSeason.actions.startSeason")}
          </button>
        </div>
      </div>
    </div>
  );
}
