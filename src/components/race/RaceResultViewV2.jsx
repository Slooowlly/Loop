import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../../stores/useCareerStore";
import FlagIcon from "../ui/FlagIcon";
import TeamLogoMark from "../team/TeamLogoMark";
import RivalMarker from "../driver/RivalMarker";
import RaceTelemetryCockpit from "./RaceTelemetryCockpit";
import { MOCK_TELEMETRY } from "./__mockTelemetry";
import {
  buildDriverMentionMatcher,
  driverMentionClass,
  renderTextWithDriverMentions,
} from "../../utils/driverMentions";
import { getTeamGlow } from "../../utils/teamColors";
import { isPortuguese, localizedAiError } from "../../utils/aiFallback";

// Tela pós-corrida REDESENHADA (v2), atrás de flag de dev. NÃO substitui a atual —
// renderizada em paralelo no Dashboard só quando a flag liga, para comparar lado a
// lado. Duas abas: DEBRIEF (tabela rica + debrief do engenheiro) e TELEMETRIA
// (cockpit). Recebe os MESMOS props da tela atual: result, evaluation, telemetry.

const ASSESSMENT = {
  MuitoAcima: { label: "Muito acima do esperado", color: "#4ade80", emoji: "🔥" },
  Acima: { label: "Acima do esperado", color: "#4ade80", emoji: "✅" },
  Dentro: { label: "Dentro do esperado", color: "#58a6ff", emoji: "🎯" },
  Abaixo: { label: "Abaixo do esperado", color: "#f59e0b", emoji: "⚠️" },
  MuitoAbaixo: { label: "Muito abaixo do esperado", color: "#ef4444", emoji: "🔻" },
};

const PODIUM = { 1: "#f5c76d", 2: "#c9d1d9", 3: "#cd8a55" };

// O `font-mono` do app aponta pro mesmo Space Grotesk (não é mono). Para os números
// da corrida usamos uma mono de verdade do sistema — dígitos crus e alinhados.
const MONO =
  '"Cascadia Code", "Cascadia Mono", "JetBrains Mono", "SF Mono", Consolas, "Roboto Mono", ui-monospace, monospace';

function weatherLabel(w) {
  if (w === "HeavyRain") return "Chuva forte";
  if (w === "Wet") return "Chuva";
  if (w === "Damp") return "Úmido";
  return "Seco";
}

function weatherIcon(w) {
  if (w === "HeavyRain") return "⛈";
  if (w === "Wet") return "🌧";
  if (w === "Damp") return "🌦";
  return "☀";
}

function formatLapMs(ms) {
  if (!Number.isFinite(ms) || ms <= 0) return "—";
  const s = ms / 1000;
  const m = Math.floor(s / 60);
  const rest = s - m * 60;
  return `${m}:${rest.toFixed(3).padStart(6, "0")}`;
}

function formatUSD(v) {
  const n = Math.round(v || 0);
  return `$${n.toLocaleString("en-US")}`;
}

function formatGap(entry) {
  if (!entry) return "—";
  if (entry.finish_position === 1 && !entry.is_dnf) return "—";
  if (entry.is_dnf) return "—";
  const s = (entry.gap_to_winner_ms ?? 0) / 1000;
  if (s <= 0) return "—";
  return `+${s.toFixed(3)}`;
}

function RaceResultViewV2({ result, evaluation, telemetry, maintenance, onDismiss }) {
  const careerId = useCareerStore((state) => state.careerId);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const season = useCareerStore((state) => state.season);
  const lastRaceId = useCareerStore((state) => state.lastRaceId);
  const language = useCareerStore((state) => state.language);
  const [tab, setTab] = useState("debrief");
  // Debrief por IA (engenheiro → você). Lazy: gera ao abrir, cacheado por corrida.
  // Fallback pro texto determinístico do cérebro enquanto carrega / se falhar.
  const [aiDebrief, setAiDebrief] = useState(null);
  // Começa "carregando" (true) pra NÃO piscar o template determinístico antes do
  // effect rodar — o engenheiro fica "no rádio" e o texto só aparece quando chega.
  const [aiLoading, setAiLoading] = useState(true);
  // DEV: injeta telemetria fake pra conferir o cockpit sem correr no iRacing.
  const [mockTelem, setMockTelem] = useState(false);
  const telemetryForCockpit = mockTelem ? MOCK_TELEMETRY : telemetry;
  const [drivers, setDrivers] = useState([]);
  // Clicar em "melhor volta" reordena pela volta mais rápida; volta ao normal em 5s.
  const [sortByLap, setSortByLap] = useState(false);
  const lapSortTimer = useRef(null);
  // Hover: destaca a linha sob o mouse + companheiro(s) de equipe, na cor da equipe.
  const [hoverTeam, setHoverTeam] = useState(null);
  // Piloto realçado ao passar o mouse no nome dele no texto do engenheiro → acende a
  // linha correspondente na tabela de resultados (na cor da equipe).
  const [hoveredDriverId, setHoveredDriverId] = useState(null);

  function handleSortByLap() {
    setSortByLap(true);
    if (lapSortTimer.current) clearTimeout(lapSortTimer.current);
    lapSortTimer.current = setTimeout(() => setSortByLap(false), 5000);
  }
  useEffect(() => () => {
    if (lapSortTimer.current) clearTimeout(lapSortTimer.current);
  }, []);

  // Busca o debrief do engenheiro por IA (lazy + cache no backend por race_id).
  useEffect(() => {
    let active = true;
    if (!careerId || !lastRaceId) {
      setAiLoading(false);
      return undefined;
    }
    setAiLoading(true);
    invoke("post_race_debrief_ai", { careerId, raceId: lastRaceId })
      .then((r) => {
        if (active && r && (r.headline || r.body)) {
          setAiDebrief({ headline: r.headline || null, body: r.body || null });
        }
      })
      .catch(() => {})
      .finally(() => {
        if (active) setAiLoading(false);
      });
    return () => {
      active = false;
    };
  }, [careerId, lastRaceId]);

  // Nacionalidade e cor de equipe não vêm no resultado da corrida — puxamos do
  // elenco da categoria (mesmo caminho da tela atual) e cruzamos por id/nome.
  useEffect(() => {
    let active = true;
    if (!careerId || !playerTeam?.categoria) return undefined;
    invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria })
      .then((data) => {
        if (active) setDrivers(Array.isArray(data) ? data : []);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [careerId, playerTeam?.categoria]);

  // Quebras de peça da corrida (Peça 3): quem teve problema, qual peça, e o desfecho.
  // Resumo no Debrief + detalhe por piloto na Telemetria. Cruzado por pilot_id/nome.
  const [breakdowns, setBreakdowns] = useState([]);
  useEffect(() => {
    let active = true;
    if (!careerId || !lastRaceId) return undefined;
    invoke("get_race_breakdowns", { careerId, raceId: lastRaceId })
      .then((data) => {
        if (active) setBreakdowns(Array.isArray(data) ? data : []);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [careerId, lastRaceId]);

  // Agrupa as quebras por piloto (um piloto pode ter mais de uma peça largando).
  const breakdownsByDriver = useMemo(() => {
    const m = {};
    for (const b of breakdowns) {
      (m[b.driver_id] ||= []).push(b);
    }
    return m;
  }, [breakdowns]);

  const natById = useMemo(() => {
    const m = {};
    for (const d of drivers) if (d?.id) m[d.id] = d.nacionalidade;
    return m;
  }, [drivers]);

  const teamColorByName = useMemo(() => {
    const m = {};
    for (const d of drivers) if (d?.equipe_nome && d?.equipe_cor) m[d.equipe_nome] = d.equipe_cor;
    return m;
  }, [drivers]);

  // Paradas (aprox.) por nome de piloto, da telemetria — refinado no passo do painel
  // de pneus (inclui fuel-only). Aqui conta trocas de pneu como nº de paradas.
  const pitByName = useMemo(() => {
    const m = {};
    for (const t of telemetry?.tire_strategies ?? []) {
      if (t?.pilot_name) m[t.pilot_name] = t.tire_changes ?? 0;
    }
    return m;
  }, [telemetry]);

  const sortedResults = useMemo(() => {
    const rows = [...(result?.race_results ?? [])];
    if (sortByLap) {
      const lapVal = (e) =>
        Number.isFinite(e.best_lap_time_ms) && e.best_lap_time_ms > 0 ? e.best_lap_time_ms : Infinity;
      rows.sort((a, b) => lapVal(a) - lapVal(b));
      return rows;
    }
    rows.sort((a, b) => {
      if (!!a.is_dnf !== !!b.is_dnf) return a.is_dnf ? 1 : -1;
      return (a.finish_position ?? 999) - (b.finish_position ?? 999);
    });
    return rows;
  }, [result, sortByLap]);

  const playerEntry = useMemo(
    () => sortedResults.find((e) => e.is_jogador) ?? null,
    [sortedResults],
  );

  // Pilotos que o texto do engenheiro pode mencionar: os que correram esta etapa
  // (id/nome vêm dos próprios resultados — auto-consistente com a tabela).
  const mentionDrivers = useMemo(
    () => (result?.race_results ?? []).map((e) => ({ id: e.pilot_id, nome: e.pilot_name })),
    [result],
  );

  // Companheiro de equipe (mesmo team_id, não-jogador) — padrão do Gap na telemetria.
  const teammateName = useMemo(() => {
    if (!playerEntry) return null;
    const mate = (result?.race_results ?? []).find(
      (r) => r.team_id === playerEntry.team_id && !r.is_jogador,
    );
    return mate?.pilot_name ?? null;
  }, [result, playerEntry]);

  const assess = evaluation ? ASSESSMENT[evaluation.assessment] ?? ASSESSMENT.Dentro : null;
  const gained = playerEntry?.positions_gained ?? 0;
  const accent = assess?.color || "#58a6ff";

  return (
    <div style={{ color: "#e6edf3" }} className="font-sans min-h-[calc(100vh-3.5rem)] flex flex-col justify-center">
      <div
        style={{
          background: "linear-gradient(160deg, rgba(10,15,22,0.62) 0%, rgba(8,13,20,0.74) 100%)",
          border: "1px solid rgba(255,255,255,0.08)",
          backdropFilter: "blur(40px)",
          WebkitBackdropFilter: "blur(40px)",
          boxShadow: "0 26px 70px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.05)",
        }}
        className="rounded-[20px] overflow-hidden"
      >
        {/* Cabeçalho + abas */}
        <div
          style={{ background: "rgba(255,255,255,0.025)", borderBottom: "1px solid rgba(255,255,255,0.07)" }}
          className="flex items-center justify-between px-5 py-3.5"
        >
          <div className="glass-light flex items-stretch h-[76px] rounded-2xl overflow-hidden">
            {/* Bloco do clima */}
            <div
              className="flex items-center justify-center"
              style={{ width: "78px", background: "rgba(255,255,255,0.045)", borderRight: "1px solid rgba(255,255,255,0.08)" }}
              title={weatherLabel(result?.weather)}
            >
              <span className="text-[36px] leading-none">{weatherIcon(result?.weather)}</span>
            </div>
            {/* Pista + condição */}
            <div className="flex flex-col justify-center px-6 min-w-0" style={{ borderRight: "1px solid rgba(255,255,255,0.08)" }}>
              <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.18em] leading-none mb-1.5">classificação final</div>
              <div className="text-[25px] font-semibold text-white leading-none truncate">{result?.track_name || "Corrida"}</div>
              <div style={{ color: "#8b949e" }} className="text-[13px] mt-2 leading-none">{weatherLabel(result?.weather)}</div>
            </div>
            {/* Segmentos de stat */}
            <div className="flex flex-col justify-center px-7 text-center" style={{ borderRight: "1px solid rgba(255,255,255,0.08)" }}>
              <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.16em] leading-none mb-2.5">voltas</div>
              <div style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[24px] font-medium leading-none">{result?.total_laps ?? 0}</div>
            </div>
            {season?.ano && (
              <div className="flex flex-col justify-center px-7 text-center">
                <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.16em] leading-none mb-2.5">temporada</div>
                <div style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[24px] font-medium leading-none">{season.ano}</div>
              </div>
            )}
          </div>
          <div className="flex items-center gap-2">
            {import.meta.env.DEV && tab === "telemetry" && (
              <button
                type="button"
                onClick={() => setMockTelem((v) => !v)}
                style={mockTelem ? { background: "#a855f733", color: "#d6bcfa", border: "0.5px solid #a855f755" } : { border: "0.5px solid rgba(255,255,255,0.12)", color: "#8b949e" }}
                className="text-[11px] px-3 py-[7px] rounded-lg"
                title="DEV: injeta telemetria fake para conferir o cockpit"
              >
                {mockTelem ? "🧪 fake ON" : "🧪 dados fake"}
              </button>
            )}
            <span
              style={{ background: "#0b0f16", border: "0.5px solid rgba(255,255,255,0.08)" }}
              className="flex gap-1.5 rounded-[10px] p-[3px]"
            >
              <button
                type="button"
                onClick={() => setTab("debrief")}
                style={tab === "debrief" ? { background: "#58a6ff22", color: "#58a6ff" } : { color: "#8b949e" }}
                className="text-[12px] px-[15px] py-[6px] rounded-lg uppercase tracking-wide"
              >
                Debrief
              </button>
              <button
                type="button"
                onClick={() => setTab("telemetry")}
                style={tab === "telemetry" ? { background: "#58a6ff22", color: "#58a6ff" } : { color: "#8b949e" }}
                className="text-[12px] px-[15px] py-[6px] rounded-lg uppercase tracking-wide"
              >
                Telemetria
              </button>
            </span>
            {onDismiss && (
              <button
                type="button"
                onClick={onDismiss}
                style={{ border: "0.5px solid rgba(255,255,255,0.12)", color: "#c9d1d9" }}
                className="text-[12px] px-3 py-[7px] rounded-lg hover:bg-white/5"
              >
                Continuar
              </button>
            )}
          </div>
        </div>

        <div className="p-3.5 flex flex-col gap-3">
          {tab === "debrief" ? (
            <>
              {/* Tabela rica de resultados */}
              <div style={{ background: "rgba(0,0,0,0.20)", border: "1px solid rgba(255,255,255,0.06)" }} className="rounded-2xl overflow-hidden">
                <table className="w-full border-collapse" style={{ tableLayout: "fixed" }}>
                  <colgroup>
                    <col style={{ width: "64px" }} />
                    <col style={{ width: "54px" }} />
                    <col style={{ width: "40px" }} />
                    <col />
                    <col />
                    <col style={{ width: "58px" }} />
                    <col style={{ width: "50px" }} />
                    <col style={{ width: "112px" }} />
                    <col style={{ width: "86px" }} />
                    <col style={{ width: "64px" }} />
                  </colgroup>
                  <thead>
                    <tr style={{ background: "rgba(255,255,255,0.025)" }}>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">pos</th>
                      <th className="py-2.5"></th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">nac</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-2 text-left uppercase tracking-wider">piloto</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-2 text-left uppercase tracking-wider">equipe</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">voltas</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-center uppercase tracking-wider">pits</th>
                      <th
                        onClick={handleSortByLap}
                        title="Clique para ordenar pela volta mais rápida (volta ao normal em 5s)"
                        style={{ color: sortByLap ? "#d6bcfa" : "#8b949e" }}
                        className="text-[11px] font-normal py-2.5 px-1.5 text-right uppercase tracking-wider cursor-pointer select-none hover:text-white"
                      >
                        melhor volta {sortByLap ? "↓" : ""}
                      </th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 px-1 text-right uppercase tracking-wider">gap</th>
                      <th style={{ color: "#8b949e" }} className="text-[11px] font-normal py-2.5 pr-5 text-right uppercase tracking-wider">pts</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sortedResults.map((e) => {
                      const isPlayer = e.is_jogador;
                      const teamColor = teamColorByName[e.team_name] || "#5f5e5a";
                      const pits = pitByName[e.pilot_name] ?? 0;
                      const isHoverTeam = hoverTeam && e.team_name === hoverTeam;
                      const isMentionHovered = hoveredDriverId && e.pilot_id === hoveredDriverId;
                      const baseRow = { transition: "background 120ms", height: "46px" };
                      const mentionTone = isMentionHovered ? getTeamGlow(teamColor) : null;
                      const rowStyle = mentionTone
                        ? {
                            ...baseRow,
                            background: mentionTone.soft,
                            boxShadow: `inset 0 0 0 1.5px ${mentionTone.solid}`,
                          }
                        : isHoverTeam
                          ? { ...baseRow, background: withAlpha(teamColor, 0.2) || "rgba(255,255,255,0.06)" }
                          : e.is_dnf
                            ? { ...baseRow, background: "rgba(239,68,68,0.09)" }
                            : isPlayer
                              ? { ...baseRow, background: "rgba(45,212,191,0.08)" }
                              : baseRow;
                      const txt = e.is_dnf ? "#f0a3a3" : isPlayer ? "#ffffff" : "#e6edf3";
                      const isPodium = !e.is_dnf && PODIUM[e.finish_position];
                      return (
                        <tr
                          key={e.pilot_id}
                          style={rowStyle}
                          onMouseEnter={() => setHoverTeam(e.team_name)}
                          onMouseLeave={() => setHoverTeam(null)}
                        >
                          <td
                            className="text-center py-2"
                            style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)", borderLeft: `3px solid ${teamColor}`, paddingLeft: "18px", paddingRight: "8px" }}
                          >
                            {e.is_dnf ? (
                              <span style={{ background: "rgba(239,68,68,0.18)", color: "#fca5a5", border: "1px solid rgba(239,68,68,0.3)", fontFamily: MONO }} className="text-[10px] px-1.5 py-0.5 rounded">DNF</span>
                            ) : isPodium ? (
                              <span
                                style={{
                                  background: PODIUM[e.finish_position],
                                  color: "#1a1407",
                                  boxShadow: `0 2px 8px ${PODIUM[e.finish_position]}55`,
                                  fontFamily: MONO,
                                  fontVariantNumeric: "tabular-nums",
                                }}
                                className="inline-flex items-center justify-center w-7 h-7 rounded-md text-[13.5px] font-semibold"
                              >
                                {e.finish_position}
                              </span>
                            ) : (
                              <span className="text-[15px]" style={{ color: isPlayer ? "#fff" : "#c9d1d9", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}>
                                {e.finish_position}
                              </span>
                            )}
                          </td>
                          <td className="text-center text-[13px] py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.is_dnf ? (
                              <span style={{ color: "#8b949e" }}>—</span>
                            ) : gainedCell(e.positions_gained)}
                          </td>
                          <td className="text-center py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            <FlagIcon nacionalidade={natById[e.pilot_id]} className="inline-block" />
                          </td>
                          <td className="text-left px-2 py-2 text-[13px] truncate" style={{ color: txt, borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.pilot_name}
                            {e.has_fastest_lap && (
                              <span title="Volta mais rápida" style={{ color: "#d6bcfa" }} className="ml-1">⚡</span>
                            )}
                            <RivalMarker driverId={e.pilot_id} className="ml-1 inline-block" />
                            {isPlayer && <span style={{ color: "#8b949e" }} className="text-[11px]"> você</span>}
                          </td>
                          <td className="text-left px-2 py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            <span className="flex items-center gap-1.5">
                              <TeamLogoMark teamName={e.team_name} color={teamColor} size="xs" testId="v2-team-logo" />
                              <span className="text-[12.5px] truncate" style={{ color: "#c9d1d9" }}>{e.team_name}</span>
                            </span>
                          </td>
                          <td className="text-center py-2 text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: e.is_dnf ? "#e08a8a" : "#c9d1d9", borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.laps_completed ?? 0}
                          </td>
                          <td className="text-center py-2 text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: "#c9d1d9", borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {pits > 0 ? pits : <span style={{ color: "#444" }}>—</span>}
                          </td>
                          <td className="text-right pr-1.5 py-2" style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.has_fastest_lap ? (
                              <span style={{ background: "rgba(168,85,247,0.2)", color: "#d6bcfa", fontFamily: MONO }} className="text-[12.5px] px-1.5 py-0.5 rounded tabular-nums">{formatLapMs(e.best_lap_time_ms)}</span>
                            ) : (
                              <span className="text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: e.is_dnf ? "#e08a8a" : "#c9d1d9" }}>{formatLapMs(e.best_lap_time_ms)}</span>
                            )}
                          </td>
                          <td className="text-right pr-1 py-2 text-[12.5px] tabular-nums" style={{ fontFamily: MONO, color: e.is_dnf ? "#f0a3a3" : "#8b949e", borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {formatGap(e)}
                          </td>
                          <td className="text-right pr-5 py-2 text-[14px] tabular-nums font-semibold" style={{ fontFamily: MONO, color: txt, borderTop: "0.5px solid rgba(255,255,255,0.05)" }}>
                            {e.points_earned > 0 ? e.points_earned : <span style={{ color: "#444" }}>—</span>}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>

              {/* Resumo de quebras (Peça 3): quem teve problema de peça e o desfecho. O
                  detalhe volta a volta fica na aba Telemetria. */}
              {breakdowns.length > 0 && (
                <div
                  style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)" }}
                  className="rounded-2xl px-5 py-3"
                >
                  <div className="text-[11px] uppercase tracking-[0.14em] mb-2" style={{ color: "#8b949e" }}>
                    🔧 Problemas de peça
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {Object.entries(breakdownsByDriver).map(([id, list]) => {
                      const first = list[0];
                      const hasDnf = list.some((b) => b.severity === "dnf");
                      const hasHeavy = list.some((b) => b.severity === "heavy");
                      const color = hasDnf ? "#f0a3a3" : hasHeavy ? "#f0b37a" : "#e6d27a";
                      const parts = list
                        .map((b) => `${b.part_name} (${b.penalty_secs != null ? `${b.penalty_secs}s` : "DNF"})`)
                        .join(", ");
                      const tip = list.map((b) => `V${b.lap}: ${b.label}`).join(" · ");
                      return (
                        <div
                          key={id}
                          title={tip}
                          style={{
                            background: first.is_player ? "rgba(45,212,191,0.10)" : "rgba(255,255,255,0.04)",
                            border: `1px solid ${first.is_player ? "rgba(45,212,191,0.4)" : "rgba(255,255,255,0.08)"}`,
                          }}
                          className="rounded-lg px-2.5 py-1 text-[12px]"
                        >
                          <span
                            style={{ color: first.is_player ? "#5eead4" : "#e6edf3" }}
                            className="font-medium"
                          >
                            {first.driver_name}
                          </span>
                          <span style={{ color }} className="ml-1.5">
                            {parts}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Debrief do engenheiro — painel completo (placeholder até a IA pós-corrida) */}
              {evaluation && (
                <div
                  style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)", borderLeft: `3px solid ${accent}` }}
                  className="rounded-2xl"
                >
                  {/* Cabeçalho: engenheiro · assessment · nota */}
                  <div className="flex items-center justify-between px-6 pt-4">
                    <div style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-[0.14em] flex items-center gap-2">
                      🎧 Debrief do engenheiro
                    </div>
                    {assess && (
                      <div className="flex items-center gap-3 whitespace-nowrap">
                        <span style={{ background: `${accent}1f`, color: accent }} className="inline-flex items-center gap-1.5 text-[12.5px] rounded-full px-3 py-[5px]">
                          {assess.emoji} {assess.label}
                        </span>
                        {Number.isFinite(evaluation.grade) && (
                          <span className="flex items-baseline gap-1.5">
                            <span style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-wider">nota</span>
                            <span style={{ color: accent, fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[27px] font-bold leading-none">
                              {evaluation.grade.toFixed(1)}
                            </span>
                          </span>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Manchete + leitura da equipe */}
                  <div className="px-6 pt-3.5 pb-5">
                    <EngineerSpeech
                      loading={aiLoading && !aiDebrief}
                      ai={aiDebrief}
                      fallbackHeadline={evaluation.headline}
                      fallbackBody={evaluation.team_read}
                      language={language}
                      accent={accent}
                      mentionDrivers={mentionDrivers}
                      hoveredDriverId={hoveredDriverId}
                      onMentionHover={setHoveredDriverId}
                    />
                  </div>

                  {/* Régua de métricas */}
                  <div className="flex" style={{ borderTop: "1px solid rgba(255,255,255,0.07)" }}>
                    <DebriefMetric label="largou → chegou" divider>
                      P{playerEntry?.grid_position ?? "—"}{" → "}
                      {playerEntry?.is_dnf ? "DNF" : `P${playerEntry?.finish_position ?? "—"}`}
                      {gained !== 0 && (
                        <span style={{ color: gained > 0 ? "#4ade80" : "#f87171" }} className="text-[14px] ml-1.5">
                          {gained > 0 ? "▲" : "▼"}{Math.abs(gained)}
                        </span>
                      )}
                    </DebriefMetric>
                    <DebriefMetric label="meta da corrida" divider>
                      P{evaluation.target_low}{evaluation.target_high !== evaluation.target_low ? `–P${evaluation.target_high}` : ""}
                    </DebriefMetric>
                    <DebriefMetric label="melhor volta" divider>
                      {formatLapMs(playerEntry?.best_lap_time_ms)}
                    </DebriefMetric>
                    <DebriefMetric label="incidentes" divider>
                      {playerEntry?.incidents_count ?? 0}
                    </DebriefMetric>
                    <MaintenanceMetric maintenance={maintenance} />
                  </div>
                </div>
              )}
            </>
          ) : (
            <RaceTelemetryCockpit
              telemetry={telemetryForCockpit}
              teammateName={teammateName}
              breakdowns={breakdowns}
            />
          )}
        </div>
      </div>
    </div>
  );
}

function withAlpha(hex, a) {
  if (typeof hex === "string" && /^#[0-9a-fA-F]{6}$/.test(hex)) {
    return `${hex}${Math.round(a * 255).toString(16).padStart(2, "0")}`;
  }
  return undefined;
}

// Revela um texto palavra por palavra, como fala chegando. Espaços preservados;
// cada palavra entra com um pequeno atraso escalonado (CSS .speech-word). Se `mentions`
// for passado, nomes de piloto viram um único "word" animado + interativo (hover
// acende o piloto na tabela de resultados).
function SpeechWords({ text, delayStep = 30, startDelay = 0, mentions = null }) {
  const matcher = mentions?.drivers ? buildDriverMentionMatcher(mentions.drivers) : null;
  const segments = matcher ? String(text).split(matcher.regex) : [String(text)];
  const nodes = [];
  let wi = 0;
  segments.forEach((seg, si) => {
    if (seg === "") return;
    const driverId = matcher?.byName.get(seg);
    if (driverId) {
      const delay = startDelay + wi * delayStep;
      wi += 1;
      const isActive = mentions.hoveredDriverId === driverId;
      nodes.push(
        <span key={`s${si}`} className="speech-word" style={{ animationDelay: `${delay}ms` }}>
          <span
            onMouseEnter={() => mentions.onHover(driverId)}
            onMouseLeave={() => mentions.onHover(null)}
            className={driverMentionClass(isActive, "text-[#58a6ff]", "text-white hover:text-[#58a6ff]")}
          >
            {seg}
          </span>
        </span>,
      );
      return;
    }
    seg.split(/(\s+)/).forEach((tok, ti) => {
      if (tok === "") return;
      if (/^\s+$/.test(tok)) {
        nodes.push(tok);
        return;
      }
      const delay = startDelay + wi * delayStep;
      wi += 1;
      nodes.push(
        <span key={`s${si}w${ti}`} className="speech-word" style={{ animationDelay: `${delay}ms` }}>
          {tok}
        </span>,
      );
    });
  });
  return nodes;
}

// Bloco da fala do engenheiro: enquanto a IA gera, mostra o equalizer de rádio
// (texto "tapado"); quando chega, revela manchete+corpo com animação de fala. Se a
// IA falhar, cai no texto determinístico do cérebro (sem animação, nunca vazio).
function EngineerSpeech({
  loading,
  ai,
  fallbackHeadline,
  fallbackBody,
  language,
  accent,
  mentionDrivers,
  hoveredDriverId,
  onMentionHover,
}) {
  if (loading) {
    return (
      <div className="flex items-center gap-3.5 py-2.5">
        <span className="eq" style={{ "--eq-color": accent }} aria-hidden="true">
          <i /><i /><i /><i /><i />
        </span>
        <span style={{ color: "#9aa5b1" }} className="text-[14.5px] italic">O engenheiro está no rádio…</span>
      </div>
    );
  }
  const animated = !!ai;
  // Sem IA: em português cai no texto determinístico do cérebro; em outro idioma
  // mostra "erro na geração de texto" localizado (não despeja PT para não-PT).
  const noAi = !ai?.headline && !ai?.body;
  if (noAi && !isPortuguese(language)) {
    return (
      <p style={{ color: "#6b7280" }} className="text-[14.5px] italic leading-relaxed">
        {localizedAiError(language)}
      </p>
    );
  }
  const headline = ai?.headline || fallbackHeadline;
  const body = ai?.body || fallbackBody;
  const mentions = { drivers: mentionDrivers, hoveredDriverId, onHover: onMentionHover };
  // `key` por conteúdo: ao trocar de loading→texto, remonta e dispara a animação.
  return (
    <div key={animated ? "ai" : "fallback"}>
      {headline && (
        <div style={{ color: "#fff" }} className="text-[24px] font-semibold leading-snug tracking-tight max-w-[82%]">
          {animated ? (
            <SpeechWords text={headline} delayStep={45} startDelay={0} mentions={mentions} />
          ) : (
            renderTextWithDriverMentions(headline, mentionDrivers, hoveredDriverId, onMentionHover)
          )}
        </div>
      )}
      {body && (
        <p style={{ color: "#9aa5b1" }} className="text-[14.5px] leading-relaxed mt-2.5 mb-0">
          {animated ? (
            <SpeechWords text={body} delayStep={26} startDelay={420} mentions={mentions} />
          ) : (
            renderTextWithDriverMentions(body, mentionDrivers, hoveredDriverId, onMentionHover)
          )}
        </p>
      )}
    </div>
  );
}

function DebriefMetric({ label, divider, children }) {
  return (
    <div
      className="flex-1 px-5 py-3.5"
      style={divider ? { borderRight: "1px solid rgba(255,255,255,0.06)" } : undefined}
    >
      <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.12em] leading-none">{label}</div>
      <div style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[19px] mt-2 leading-none">
        {children}
      </div>
    </div>
  );
}

// Célula "Manutenção" da régua: total sempre visível (âmbar, pra puxar atenção ao
// cuidado com o carro) + tooltip de breakdown por item ao passar o mouse.
function MaintenanceMetric({ maintenance }) {
  const total = maintenance?.total ?? 0;
  const items = Array.isArray(maintenance?.items) ? maintenance.items : [];
  return (
    <div className="flex-1 px-5 py-3.5 relative group">
      <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.12em] leading-none">manutenção</div>
      <div
        style={{ color: "#e0a458", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}
        className="text-[19px] mt-2 leading-none inline-flex items-center gap-1.5 cursor-help"
      >
        {formatUSD(total)}
        {items.length > 0 && <span style={{ color: "#6e7681" }} className="text-[11px]">ⓘ</span>}
      </div>
      {items.length > 0 && (
        <div
          style={{
            background: "rgba(13,19,30,0.97)",
            border: "1px solid rgba(255,255,255,0.1)",
            boxShadow: "0 16px 40px rgba(0,0,0,0.55)",
            backdropFilter: "blur(8px)",
          }}
          className="absolute bottom-full right-4 mb-2 z-30 rounded-xl px-4 py-3 min-w-[200px] opacity-0 translate-y-1 pointer-events-none group-hover:opacity-100 group-hover:translate-y-0 transition-all duration-150"
        >
          <div style={{ color: "#8b949e" }} className="text-[10px] uppercase tracking-[0.14em] mb-2.5">Fatura do carro</div>
          <div className="flex flex-col gap-1.5">
            {items.map((it) => (
              <div key={it.key} className="flex items-center justify-between gap-6">
                <span style={{ color: "#c9d1d9" }} className="text-[12.5px]">{it.label}</span>
                <span style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[12.5px]">{formatUSD(it.cost)}</span>
              </div>
            ))}
          </div>
          <div style={{ borderTop: "1px solid rgba(255,255,255,0.08)" }} className="flex items-center justify-between gap-6 mt-2.5 pt-2.5">
            <span style={{ color: "#8b949e" }} className="text-[11px] uppercase tracking-wide">Total</span>
            <span style={{ color: "#e0a458", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[13px]">{formatUSD(total)}</span>
          </div>
        </div>
      )}
    </div>
  );
}

function gainedCell(g) {
  if (!g) return <span style={{ color: "#8b949e" }}>—</span>;
  const up = g > 0;
  return (
    <span style={{ color: up ? "#4ade80" : "#f87171", fontWeight: 500 }}>
      {up ? "▲" : "▼"}
      {Math.abs(g)}
    </span>
  );
}

export default RaceResultViewV2;
