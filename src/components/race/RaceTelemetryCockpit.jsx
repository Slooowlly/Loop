import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { bestEffort } from "../../utils/bestEffort";
import useCareerStore from "../../stores/useCareerStore";
import LapTimeLineChart from "./LapTimeLineChart";
import RaceTraceChart from "./RaceTraceChart";
import PaceDeltaChart from "./PaceDeltaChart";
import WeatherTimelineChart from "./WeatherTimelineChart";
import TeamLogoMark from "../team/TeamLogoMark";
import Tooltip from "../ui/Tooltip";
import IracingSemDadosAviso from "../iracing/IracingSemDadosAviso";
import { capitalizar, formatLapSeconds } from "../../utils/formatters";
import { AXIS_TICK, GRID, PALETTE, PLAYER_COLOR } from "../../utils/chartTheme";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  // O balão do gráfico é outro mecanismo: quem o desenha é o recharts, a partir
  // do ponto sob o cursor. `Tooltip` sozinho é o do app, aqui e no resto.
  Tooltip as ChartTooltip,
  ReferenceLine,
  ResponsiveContainer,
} from "recharts";

// Cockpit de telemetria da pós-corrida (aba Telemetria do RaceResultViewV2). Reusa
// os MESMOS dados (`telemetry.charts` + `telemetry.tire_strategies`) e componentes de
// gráfico compartilhados, mas num layout empilhado (não-abas): RACE TRACE full →
// VARIAÇÃO full → RITMO + GAP pequenos lado a lado → ESTRATÉGIA DE PIT.

const MONO =
  '"Cascadia Code", "Cascadia Mono", "JetBrains Mono", "SF Mono", Consolas, "Roboto Mono", ui-monospace, monospace';

// Ícones (PNG) em public/utilities/textures — espaços viram %20 na URL.
const TX = "/utilities/textures/";
const FUEL_ICON = `${TX}Fuel.webp`;
const TIRE_DRY_ICON = `${TX}Pneu%20Seco.webp`;
const TIRE_WET_ICON = `${TX}Pneu%20Molhado.webp`;

// Cores de composto: seco = âmbar, molhado = azul, indefinido = cinza.
const COMPOUND = {
  Dry: { color: "#f0a83c", label: "Seco", icon: TIRE_DRY_ICON },
  Wet: { color: "#4aa3ff", label: "Molhado", icon: TIRE_WET_ICON },
  Unknown: { color: "#6e7681", label: "Indefinido", icon: null },
};
function compoundOf(c) {
  return COMPOUND[c] || COMPOUND.Unknown;
}

function SectionTitle({ children, right }) {
  return (
    <div className="flex items-center justify-between mb-2.5">
      <h3 style={{ color: "#8b949e" }} className="text-[11px] font-semibold uppercase tracking-[0.16em]">{children}</h3>
      {right}
    </div>
  );
}

function Panel({ children, className = "" }) {
  return (
    <div
      style={{ background: "rgba(255,255,255,0.025)", border: "1px solid rgba(255,255,255,0.06)" }}
      className={`rounded-2xl p-4 ${className}`}
    >
      {children}
    </div>
  );
}

function Chip({ active, onClick, children }) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={active ? { background: "rgba(255,255,255,0.14)", color: "#fff" } : { color: "#8b949e" }}
      className="rounded-md px-2.5 py-1 text-[11px] font-semibold transition"
    >
      {children}
    </button>
  );
}

// Estimativa do bloco de troca de pneus (s) — não exato, ~20–22s.
const TIRE_EST_SECS = 21;

function RaceTelemetryCockpit({ telemetry, teammateName = null, breakdowns = [] }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((s) => s.careerId);
  const lastRaceId = useCareerStore((s) => s.lastRaceId);
  const [traceMode, setTraceMode] = useState("gap");
  const [carColors, setCarColors] = useState({ by_name: {}, player_color: null });
  // Gap: piloto analisado (idx). null = usa o padrão (companheiro → rival → 1º outro).
  const [gapTargetIdx, setGapTargetIdx] = useState(null);
  const [gapMenuOpen, setGapMenuOpen] = useState(false);

  const charts = telemetry?.charts ?? null;
  const cars = charts?.cars ?? [];
  const lapTimes = charts?.lap_times ?? [];
  const yellowLaps = charts?.yellow_laps ?? [];
  const tireStrategies = telemetry?.tire_strategies ?? [];
  const mistakeLap = telemetry?.mistake?.lap ?? 0;
  const bestMomentLap = telemetry?.best_moment?.lap ?? 0;
  const raceLaps = telemetry?.race_laps ?? 0;

  useEffect(() => {
    if (!careerId) return;
    // Mesma leitura do RaceCharts: cor é decoração, a paleta padrão cobre a falha, e o
    // rastro separa "falhou" de "não há cor".
    bestEffort(invoke("iracing_car_colors", { careerId }), "iracing_car_colors").then((c) =>
      setCarColors(c || { by_name: {}, player_color: null }),
    );
  }, [careerId]);

  const orderedCars = useMemo(() => {
    const others = cars.filter((c) => !c.is_player);
    const player = cars.filter((c) => c.is_player);
    return [...others, ...player];
  }, [cars]);

  const colorFor = useMemo(() => {
    const byName = carColors?.by_name ?? {};
    const map = {};
    let i = 0;
    for (const c of orderedCars) {
      if (c.is_player) map[c.idx] = carColors?.player_color || PLAYER_COLOR;
      else map[c.idx] = byName[c.name] || PALETTE[i++ % PALETTE.length];
    }
    return map;
  }, [orderedCars, carColors]);

  const nameByIdx = useMemo(() => {
    const m = {};
    for (const c of cars) m[c.idx] = c.name;
    return m;
  }, [cars]);

  // Pivot do trace: uma linha por volta, c{idx} = gap OU posição.
  const traceRows = useMemo(() => {
    const byLap = new Map();
    for (const car of cars) {
      for (const p of car.points) {
        if (!byLap.has(p.lap)) byLap.set(p.lap, { lap: p.lap });
        byLap.get(p.lap)[`c${car.idx}`] = traceMode === "gap" ? p.gap : p.position;
      }
    }
    return [...byLap.values()].sort((a, b) => a.lap - b.lap);
  }, [cars, traceMode]);

  const gapCap = useMemo(() => {
    if (traceMode !== "gap") return null;
    const vals = [];
    for (const row of traceRows)
      for (const k in row) if (k !== "lap" && Number.isFinite(row[k]) && row[k] >= 0) vals.push(row[k]);
    if (!vals.length) return null;
    vals.sort((a, b) => a - b);
    const p90 = vals[Math.min(vals.length - 1, Math.floor(vals.length * 0.9))];
    return Math.max(Math.ceil((p90 * 1.2) / 5) * 5, 20);
  }, [traceRows, traceMode]);

  const displayRows = useMemo(() => {
    if (traceMode !== "gap" || gapCap == null) return traceRows;
    return traceRows.map((row) => {
      const out = { lap: row.lap };
      for (const k in row) if (k !== "lap") out[k] = Math.min(row[k], gapCap);
      return out;
    });
  }, [traceRows, traceMode, gapCap]);

  const lapDomain = useMemo(() => {
    if (!traceRows.length) return [1, 1];
    return [traceRows[0].lap, traceRows[traceRows.length - 1].lap];
  }, [traceRows]);

  // Tempos de volta do jogador → delta à média (Variação + Ritmo).
  const pace = useMemo(() => {
    if (!lapTimes.length) return null;
    const times = lapTimes.map((p) => p.time_s);
    const avg = times.reduce((a, b) => a + b, 0) / times.length;
    const best = Math.min(...times);
    const variance = times.reduce((a, b) => a + (b - avg) ** 2, 0) / times.length;
    return {
      avg,
      best,
      std: Math.sqrt(variance),
      rows: lapTimes.map((p) => ({ lap: p.lap, delta: p.time_s - avg, time: p.time_s, isBest: p.time_s === best })),
    };
  }, [lapTimes]);

  // ── Gap a um piloto selecionável (padrão = companheiro de equipe) ──────────────
  const playerCar = useMemo(() => cars.find((c) => c.is_player) ?? null, [cars]);
  const otherCars = useMemo(
    () => cars.filter((c) => !c.is_player).slice().sort((a, b) => a.name.localeCompare(b.name)),
    [cars],
  );
  const defaultGapIdx = useMemo(() => {
    const mate = telemetry?.__mockTeammateName || teammateName;
    if (mate) {
      const c = cars.find((x) => x.name === mate && !x.is_player);
      if (c) return c.idx;
    }
    if (charts?.rival_name) {
      const r = cars.find((x) => x.name === charts.rival_name && !x.is_player);
      if (r) return r.idx;
    }
    return otherCars[0]?.idx ?? null;
  }, [cars, otherCars, charts, teammateName, telemetry]);
  const activeGapIdx = gapTargetIdx ?? defaultGapIdx;
  const activeGapName = cars.find((c) => c.idx === activeGapIdx)?.name ?? "—";

  // Gap (s) volta a volta entre você e o piloto escolhido: gap-ao-líder seu menos o
  // dele. >0 = ele à frente (você caçando); <0 = você à frente.
  const gapRows = useMemo(() => {
    if (activeGapIdx == null || !playerCar) return [];
    const target = cars.find((c) => c.idx === activeGapIdx);
    if (!target) return [];
    const pByLap = new Map(playerCar.points.map((p) => [p.lap, p.gap]));
    const out = [];
    for (const tp of target.points) {
      const pg = pByLap.get(tp.lap);
      if (pg == null) continue;
      out.push({ lap: tp.lap, gap_s: pg - tp.gap });
    }
    return out.sort((a, b) => a.lap - b.lap);
  }, [activeGapIdx, cars, playerCar]);

  // Marcações na faixa de clima: as paradas do jogador (🛞 troca / ⛽ combustível).
  const weatherMarkers = useMemo(() => {
    const stops = telemetry?.player_tire?.stops ?? [];
    if (!stops.length || raceLaps < 2) return [];
    return stops.map((p) => ({
      frac: Math.max(0, Math.min(1, (p.lap - 1) / Math.max(1, raceLaps - 1))),
      icon: p.tire_change ? (p.track_wet ? TIRE_WET_ICON : TIRE_DRY_ICON) : FUEL_ICON,
      label: t("telemetryCockpit.stopMarkerLabel", { lap: p.lap, secs: p.box_secs.toFixed(1) }),
      isPlayer: true,
    }));
  }, [telemetry, raceLaps]);

  const playerColor = carColors?.player_color || PLAYER_COLOR;
  const hasTrace = traceRows.length >= 2 && cars.length > 0;
  const anyPits = tireStrategies.some((s) => (s.stops?.length ?? 0) > 0);
  const totalStops = telemetry?.player_tire?.stops?.length ?? 0;

  if (!telemetry || (!charts && tireStrategies.length === 0)) {
    return (
      <div
        style={{ background: "rgba(0,0,0,0.18)", border: "0.5px solid rgba(255,255,255,0.07)", color: "#8b949e" }}
        className="rounded-2xl p-10 text-center text-[13px]"
      >
        {t("telemetryCockpit.emptyState")}
        <IracingSemDadosAviso />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {/* Régua de métricas */}
      <div className="flex gap-3">
        <MetricTile label={t("telemetryCockpit.metricBestLap")} value={pace ? formatLapSeconds(pace.best) : "--"} />
        <MetricTile label={t("telemetryCockpit.metricLapsSeen")} value={telemetry?.laps_seen ?? lapTimes.length} />
        <MetricTile label={t("telemetryCockpit.metricConsistency")} value={pace ? `±${pace.std.toFixed(3)}s` : "--"} />
        <MetricTile label={t("telemetryCockpit.metricStops")} value={totalStops} />
      </div>

      {/* RACE TRACE — largura total */}
      {hasTrace && (
        <Panel>
          <SectionTitle
            right={
              <div className="flex gap-1.5">
                <Chip active={traceMode === "position"} onClick={() => setTraceMode("position")}>{t("telemetryCockpit.tracePosition")}</Chip>
                <Chip active={traceMode === "gap"} onClick={() => setTraceMode("gap")}>{t("telemetryCockpit.traceGapToLeader")}</Chip>
              </div>
            }
          >
            {t("telemetryCockpit.raceTrace")}
          </SectionTitle>
          <div className="h-[320px] w-full">
            <RaceTraceChart
              rows={displayRows}
              cars={orderedCars.map((c) => ({ idx: c.idx, isPlayer: c.is_player }))}
              colorForCar={(idx) => colorFor[idx]}
              nameByIdx={nameByIdx}
              mode={traceMode}
              lapDomain={lapDomain}
              gapCap={gapCap}
              yellowLaps={yellowLaps}
              bestLap={bestMomentLap}
              mistakeLap={mistakeLap}
            />
          </div>
          <div className="flex flex-wrap gap-x-3 gap-y-1 mt-2">
            {orderedCars.map((car) => (
              <span key={car.idx} style={{ color: "#8b949e" }} className="flex items-center gap-1 text-[10px]">
                <span className="inline-block h-[3px] w-3.5 rounded" style={{ background: colorFor[car.idx] }} />
                {car.name}{car.is_player ? t("telemetryCockpit.you") : ""}
              </span>
            ))}
          </div>
        </Panel>
      )}

      {/* VARIAÇÃO DE VOLTAS — largura total, embaixo do trace */}
      {pace && (
        <Panel>
          <SectionTitle
            right={
              <span style={{ color: "#8b949e" }} className="text-[11px]">
                {t("telemetryCockpit.avg")} <span style={{ color: "#fff", fontFamily: MONO }}>{formatLapSeconds(pace.avg)}</span>
                <span className="mx-2">·</span>
                {t("telemetryCockpit.best")} <span style={{ color: "#4ade80", fontFamily: MONO }}>{formatLapSeconds(pace.best)}</span>
              </span>
            }
          >
            {t("telemetryCockpit.lapVariation")}
          </SectionTitle>
          <div className="h-[260px] w-full">
            <PaceDeltaChart rows={pace.rows} yellowLaps={yellowLaps} bestLap={bestMomentLap} mistakeLap={mistakeLap} />
          </div>
        </Panel>
      )}

      {/* RITMO + GAP PRO RIVAL — menores, lado a lado */}
      <div className="grid grid-cols-2 gap-3">
        <Panel>
          <SectionTitle>{t("telemetryCockpit.pace")}</SectionTitle>
          {pace ? (
            <div className="h-[220px] w-full">
              <LapTimeLineChart rows={pace.rows} color={playerColor} />
            </div>
          ) : (
            <EmptyMini>{t("telemetryCockpit.noPlayerLaps")}</EmptyMini>
          )}
        </Panel>
        <Panel>
          <SectionTitle
            right={
              otherCars.length > 0 && (
                <div className="relative">
                  <button
                    type="button"
                    onClick={() => setGapMenuOpen((o) => !o)}
                    style={{ background: "rgba(255,255,255,0.06)", color: "#fff" }}
                    className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[11px] font-semibold"
                  >
                    {activeGapName}
                    <span style={{ color: "#8b949e" }}>▾</span>
                  </button>
                  {gapMenuOpen && (
                    <div
                      style={{ background: "#0d131c", border: "1px solid rgba(255,255,255,0.1)" }}
                      className="absolute right-0 top-full z-30 mt-1 max-h-56 w-48 overflow-y-auto rounded-xl p-1 shadow-xl"
                    >
                      {otherCars.map((c) => (
                        <button
                          key={c.idx}
                          type="button"
                          onClick={() => {
                            setGapTargetIdx(c.idx);
                            setGapMenuOpen(false);
                          }}
                          style={{ color: c.idx === activeGapIdx ? "#58a6ff" : "#c9d1d9" }}
                          className="flex w-full items-center justify-between rounded-lg px-3 py-1.5 text-left text-[12px] hover:bg-white/10"
                        >
                          <span className="truncate">{c.name}</span>
                          {c.idx === activeGapIdx && <span className="text-[#58a6ff]">✓</span>}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              )
            }
          >
            {t("telemetryCockpit.gap")}
          </SectionTitle>
          {gapRows.length >= 2 ? (
            <div className="h-[220px] w-full">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={gapRows} margin={{ top: 8, right: 10, bottom: 16, left: 2 }}>
                  <CartesianGrid stroke={GRID} />
                  <XAxis type="number" dataKey="lap" allowDecimals={false}
                    tick={{ fill: AXIS_TICK, fontSize: 10 }} stroke={GRID}
                    label={{ value: t("telemetryCockpit.lap"), position: "insideBottom", offset: -6, fill: AXIS_TICK, fontSize: 10 }} />
                  <YAxis tick={{ fill: AXIS_TICK, fontSize: 10 }} stroke={GRID} width={40}
                    tickFormatter={(v) => `${Math.abs(v).toFixed(1)}s`} />
                  <ChartTooltip content={<RivalTooltip rivalName={activeGapName} />} />
                  <ReferenceLine y={0} stroke={PLAYER_COLOR} strokeOpacity={0.7} />
                  <Line type="monotone" dataKey="gap_s" stroke="#f59e0b" strokeWidth={2} dot={{ r: 2 }}
                    connectNulls isAnimationActive={false} />
                </LineChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyMini>{t("telemetryCockpit.noGapData")}</EmptyMini>
          )}
        </Panel>
      </div>

      {/* ESTRATÉGIA DE PIT — painel grande embaixo */}
      <Panel>
        <SectionTitle
          right={
            <span className="flex items-center gap-3 text-[10px]" style={{ color: "#8b949e" }}>
              <span className="flex items-center gap-1"><img src={TIRE_DRY_ICON} alt="" className="h-3.5 w-3.5 object-contain" />{t("telemetryCockpit.legendDry")}</span>
              <span className="flex items-center gap-1"><img src={TIRE_WET_ICON} alt="" className="h-3.5 w-3.5 object-contain" />{t("telemetryCockpit.legendWet")}</span>
              <span className="flex items-center gap-1"><img src={FUEL_ICON} alt="" className="h-3.5 w-3.5 object-contain" />{t("telemetryCockpit.legendFuelOnly")}</span>
            </span>
          }
        >
          {t("telemetryCockpit.pitStrategy")}
        </SectionTitle>
        {/* Gate pela EXISTÊNCIA de estratégia, não pela existência de parada: uma
            corrida em que ninguém parou ainda tem estratégia (a de zero paradas). */}
        {tireStrategies.length > 0 ? (
          <PitStrategy strategies={tireStrategies} raceLaps={raceLaps} colorFor={colorFor} nameByIdx={nameByIdx} />
        ) : (
          <EmptyMini>{t("telemetryCockpit.noStrategyData")}</EmptyMini>
        )}
      </Panel>

      {/* TEMPOS DE PIT — tabela dedicada (a parada mais rápida de cada carro) */}
      {anyPits && (
        <Panel>
          <SectionTitle
            right={<span style={{ color: "#6e7681" }} className="text-[10px]">{t("telemetryCockpit.pitTimesHint")}</span>}
          >
            {t("telemetryCockpit.pitTimes")}
          </SectionTitle>
          <PitTimesTable
            strategies={tireStrategies}
            playerCarIdx={telemetry?.player_tire?.car_idx ?? -999}
            colorFor={colorFor}
            nameByIdx={nameByIdx}
          />
        </Panel>
      )}

      {/* QUEBRAS DE PEÇA — detalhe por piloto (Peça 3): peça, problema, volta e desfecho */}
      {breakdowns && breakdowns.length > 0 && (
        <Panel>
          <SectionTitle
            right={<span style={{ color: "#6e7681" }} className="text-[10px]">{t("telemetryCockpit.breakdownsHint")}</span>}
          >
            {t("telemetryCockpit.breakdowns")}
          </SectionTitle>
          <div className="overflow-x-auto">
            <table className="w-full border-collapse text-[12px]">
              <thead>
                <tr style={{ background: "rgba(255,255,255,0.025)" }}>
                  <th className="text-left font-normal py-2 px-2 uppercase tracking-wider" style={{ color: "#8b949e" }}>{t("telemetryCockpit.colDriver")}</th>
                  <th className="text-left font-normal py-2 px-2 uppercase tracking-wider" style={{ color: "#8b949e" }}>{t("telemetryCockpit.colPart")}</th>
                  <th className="text-left font-normal py-2 px-2 uppercase tracking-wider" style={{ color: "#8b949e" }}>{t("telemetryCockpit.colProblem")}</th>
                  <th className="text-center font-normal py-2 px-2 uppercase tracking-wider" style={{ color: "#8b949e" }}>{t("telemetryCockpit.colLap")}</th>
                  <th className="text-right font-normal py-2 px-2 uppercase tracking-wider" style={{ color: "#8b949e" }}>{t("telemetryCockpit.colOutcome")}</th>
                </tr>
              </thead>
              <tbody>
                {breakdowns.map((b, i) => {
                  const isDnf = b.severity === "dnf";
                  const color = isDnf ? "#f0a3a3" : b.severity === "heavy" ? "#f0b37a" : "#e6d27a";
                  return (
                    <tr
                      key={`${b.driver_id}-${b.part}-${b.lap}-${i}`}
                      style={{
                        background: b.is_player ? "rgba(45,212,191,0.08)" : "transparent",
                        borderTop: "1px solid rgba(255,255,255,0.05)",
                      }}
                    >
                      <td className="py-1.5 px-2" style={{ color: b.is_player ? "#5eead4" : "#e6edf3" }}>{b.driver_name}</td>
                      <td className="py-1.5 px-2" style={{ color: "#c9d1d9" }}>{b.part_name}</td>
                      <td className="py-1.5 px-2" style={{ color: "#9da7b0" }}>{capitalizar(b.label)}</td>
                      <td className="py-1.5 px-2 text-center" style={{ color: "#8b949e" }}>{b.lap}</td>
                      <td className="py-1.5 px-2 text-right font-medium" style={{ color }}>
                        {isDnf ? t("telemetryCockpit.dnf") : `+${b.penalty_secs}s`}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </Panel>
      )}

      {/* CLIMA DA CORRIDA — timeline determinístico (mesmo da sala de estratégia) */}
      {(telemetry?.__mockWeather || (careerId && lastRaceId)) && (
        <Panel>
          <WeatherTimelineChart
            careerId={careerId}
            raceId={lastRaceId}
            markers={weatherMarkers}
            forecast={false}
            mockData={telemetry?.__mockWeather || null}
          />
        </Panel>
      )}
    </div>
  );
}

function MetricTile({ label, value }) {
  return (
    <div
      style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.06)" }}
      className="flex-1 rounded-xl px-4 py-3"
    >
      <div style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.12em] leading-none">{label}</div>
      <div style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }} className="text-[19px] mt-2 leading-none">{value}</div>
    </div>
  );
}

function EmptyMini({ children }) {
  return (
    <div style={{ color: "#6e7681" }} className="flex items-center justify-center h-[180px] text-center text-[12px] px-4">
      {children}
    </div>
  );
}

// Linha do tempo de estratégia (estilo F1 Manager): por carro, uma barra de stints
// colorida por composto. Troca de pneu = mudança de cor (fronteira do stint). Parada
// SÓ de combustível = ícone ⛽ no ponto da volta (sem trocar a cor).
function PitStrategy({ strategies, raceLaps, colorFor, nameByIdx }) {
  const { t } = useTranslation();
  const compLabel = (c) => t(`telemetryCockpit.compound.${COMPOUND[c] ? c : "Unknown"}`);
  // TODOS os competidores entram, inclusive os de zero paradas — não parar também é
  // estratégia. Ordena os que pararam primeiro, os sem parada depois, cada bloco por
  // nome (ordem estável, o jogador se identifica pela cor).
  const rows = useMemo(() => {
    const nomeDe = (s) => s.pilot_name || nameByIdx[s.car_idx] || "";
    const porNome = (a, b) => nomeDe(a).localeCompare(nomeDe(b));
    const paradas = (s) => s.stops?.length ?? 0;
    const comParada = strategies.filter((s) => paradas(s) > 0).sort(porNome);
    const semParada = strategies.filter((s) => paradas(s) === 0).sort(porNome);
    return [...comParada, ...semParada];
  }, [strategies, nameByIdx]);

  const maxLap = useMemo(() => {
    let m = raceLaps || 0;
    for (const s of strategies) {
      for (const st of s.stints || []) m = Math.max(m, st.from_lap);
      for (const p of s.stops || []) m = Math.max(m, p.lap);
    }
    return Math.max(m, 1);
  }, [strategies, raceLaps]);

  // Posição (%) de uma volta na barra — MESMA fórmula da fronteira dos stints, pra o
  // separador da parada cair exatamente onde a cor troca.
  const sepLeft = (lap) => ((lap - 1) / Math.max(1, maxLap)) * 100;

  return (
    <div className="flex flex-col gap-2.5">
      {rows.map((s) => {
        const stints = [...(s.stints || [])].sort((a, b) => a.from_lap - b.from_lap);
        const stops = [...(s.stops || [])].sort((a, b) => a.lap - b.lap);
        // Zero paradas: barra única, do começo ao fim, no composto da largada.
        const semParadas = stops.length === 0;
        // Composto vigente numa volta (do último stint começado até ela).
        const compoundAt = (lap) => {
          let c = stints[0]?.compound ?? s.start_compound ?? "Unknown";
          for (const st of stints) {
            if (st.from_lap <= lap) c = st.compound;
            else break;
          }
          return c;
        };
        // Segmentos numerados divididos por TODAS as paradas (troca E combustível):
        // abastecer também cria um trecho novo, mesmo sem mudar a cor.
        const cuts = [
          1,
          maxLap + 1,
          ...stops.map((p) => p.lap),
          ...stints.filter((st) => st.changed).map((st) => st.from_lap),
        ];
        const bounds = [...new Set(cuts)].filter((l) => l >= 1 && l <= maxLap + 1).sort((a, b) => a - b);
        const segments = [];
        for (let i = 0; i < bounds.length - 1; i++) {
          if (bounds[i + 1] > bounds[i]) {
            segments.push({ start: bounds[i], end: bounds[i + 1], compound: compoundAt(bounds[i]) });
          }
        }
        return (
          <div key={s.car_idx} className="flex items-center gap-3">
            {/* Nome (alinhado à barra via espaçador da faixa de marcadores) */}
            <div className="w-36 shrink-0 flex flex-col">
              <div className="h-[28px]" />
              <div className="h-7 flex items-center gap-2 min-w-0">
                <span className="inline-block h-2.5 w-2.5 rounded-full shrink-0" style={{ background: colorFor[s.car_idx] || "#5f5e5a" }} />
                <span style={{ color: "#c9d1d9" }} className="text-[12px] truncate">{s.pilot_name || nameByIdx[s.car_idx] || t("telemetryCockpit.carN", { n: s.car_idx })}</span>
              </div>
            </div>
            {/* Timeline: faixa de marcadores (⛽) acima + barra de trechos */}
            <div className="flex-1 flex flex-col">
              <div className="relative h-[28px]">
                {(s.stops || []).map((p, i) => {
                  const isFuel = !p.tire_change;
                  const comp = compoundOf(compoundAt(p.lap));
                  const icon = isFuel ? FUEL_ICON : comp.icon || TIRE_DRY_ICON;
                  const label = isFuel
                    ? t("telemetryCockpit.refuel", { lap: p.lap, secs: p.box_secs.toFixed(1) })
                    : t("telemetryCockpit.tireChange", { compound: compLabel(compoundAt(p.lap)), lap: p.lap, secs: p.box_secs.toFixed(1) });
                  return (
                    <Tooltip key={`m${i}`} texto={label}>
                      <span
                        className="absolute bottom-0 -translate-x-1/2 flex flex-col items-center leading-none"
                        style={{ left: `${sepLeft(p.lap)}%` }}
                      >
                        <span
                          style={{ background: "rgba(13,19,30,0.95)", border: "1px solid rgba(255,255,255,0.18)" }}
                          className="rounded-md p-[3px] flex items-center"
                        >
                          <img src={icon} alt="" className="h-[18px] w-[18px] object-contain block" />
                        </span>
                        <span
                          style={{ width: 0, height: 0, borderLeft: "3px solid transparent", borderRight: "3px solid transparent", borderTop: "4px solid rgba(255,255,255,0.45)" }}
                        />
                      </span>
                    </Tooltip>
                  );
                })}
              </div>
              <div className="relative h-7 rounded-md overflow-hidden" style={{ background: "rgba(255,255,255,0.04)" }}>
                {segments.map((seg, i) => {
                  const left = ((seg.start - 1) / Math.max(1, maxLap)) * 100;
                  const width = ((seg.end - seg.start) / Math.max(1, maxLap)) * 100;
                  const comp = compoundOf(seg.compound);
                  return (
                    <Tooltip
                      key={i}
                      texto={semParadas
                        ? t("telemetryCockpit.noStopSegment", { compound: compLabel(seg.compound) })
                        : t("telemetryCockpit.segment", { n: i + 1, compound: compLabel(seg.compound), range: `${seg.start}${seg.end <= maxLap ? `–${seg.end - 1}` : "+"}` })}
                    >
                      <div
                        className="absolute top-0 bottom-0 flex items-center justify-center"
                        style={{ left: `${left}%`, width: `${width}%`, background: comp.color, opacity: 0.85 }}
                      >
                        {/* Sem parada: a barra inteira diz o que aconteceu, em vez do nº do trecho. */}
                        {semParadas ? (
                          <span style={{ color: "#0c1117", fontFamily: MONO }} className="text-[11px] font-bold truncate px-2">
                            {t("telemetryCockpit.noStopBar", { compound: compLabel(seg.compound) })}
                          </span>
                        ) : (
                          width > 4 && (
                            <span style={{ color: "#0c1117", fontFamily: MONO }} className="text-[12px] font-bold">{i + 1}</span>
                          )
                        )}
                      </div>
                    </Tooltip>
                  );
                })}
                {/* Separador PRETO em TODA parada (troca ou só combustível) — bem visível */}
                {(s.stops || []).map((p, i) => (
                  <Tooltip
                    key={`sep${i}`}
                    texto={p.tire_change
                      ? t("telemetryCockpit.pitSeparatorTireChange", { lap: p.lap, secs: p.box_secs.toFixed(1) })
                      : t("telemetryCockpit.pitSeparatorFuelOnly", { lap: p.lap, secs: p.box_secs.toFixed(1) })}
                  >
                    <span
                      className="absolute top-0 bottom-0 z-10"
                      style={{
                        left: `${sepLeft(p.lap)}%`,
                        width: "4px",
                        transform: "translateX(-2px)",
                        background: "#000",
                        boxShadow: "inset 1px 0 0 rgba(255,255,255,0.3), inset -1px 0 0 rgba(255,255,255,0.3)",
                      }}
                    />
                  </Tooltip>
                ))}
              </div>
            </div>
            {/* Nº de paradas (alinhado à barra) */}
            <div className="w-16 shrink-0 flex flex-col">
              <div className="h-[28px]" />
              <div className="h-7 flex items-center justify-end">
                <span style={{ color: semParadas ? "#6e7681" : "#8b949e", fontFamily: MONO }} className="text-[11px]">
                  {semParadas
                    ? t("telemetryCockpit.noStopCount")
                    : t("telemetryCockpit.pits", { count: stops.length })}
                </span>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

// Tabela dedicada de tempos de pit: uma linha por carro = a parada MAIS RÁPIDA dele
// (menor tempo na caixa = estafe dos boxes). Pneu/combustível estimado (troca ~21s).
function PitTimesTable({ strategies, playerCarIdx, colorFor, nameByIdx }) {
  const { t } = useTranslation();
  const rows = useMemo(() => {
    const best = {};
    (strategies || []).forEach((s) => {
      (s.stops || []).forEach((d) => {
        const cur = best[s.car_idx];
        if (!cur || d.box_secs < cur.box_secs) {
          best[s.car_idx] = {
            ...d,
            pilot_name: s.pilot_name || nameByIdx[s.car_idx] || t("telemetryCockpit.carN", { n: s.car_idx }),
            team: s.team_name || "—",
            car_idx: s.car_idx,
            isPlayer: s.car_idx === playerCarIdx,
          };
        }
      });
    });
    return Object.values(best).sort((a, b) => a.box_secs - b.box_secs);
  }, [strategies, playerCarIdx, nameByIdx]);

  if (!rows.length) return null;

  return (
    <table className="w-full" style={{ tableLayout: "fixed" }}>
      <colgroup>
        <col style={{ width: "40px" }} />
        <col />
        <col />
        <col style={{ width: "104px" }} />
        <col style={{ width: "62px" }} />
        <col style={{ width: "78px" }} />
        <col style={{ width: "78px" }} />
      </colgroup>
      <thead>
        <tr style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-wider">
          <th className="py-2 text-center">#</th>
          <th className="py-2 px-2 text-left">{t("telemetryCockpit.colTeam")}</th>
          <th className="py-2 px-2 text-left">{t("telemetryCockpit.colPilot")}</th>
          <th className="py-2 px-2 text-right">{t("telemetryCockpit.colPitTime")}</th>
          <th className="py-2 text-center">{t("telemetryCockpit.colLapCap")}</th>
          <th className="py-2 text-center">
            <span className="inline-flex items-center justify-center gap-1">
              <img src={TIRE_DRY_ICON} alt="" className="h-3.5 w-3.5 object-contain" />{t("telemetryCockpit.colTires")}
            </span>
          </th>
          <th className="py-2 text-center">
            <span className="inline-flex items-center justify-center gap-1">
              <img src={FUEL_ICON} alt="" className="h-3.5 w-3.5 object-contain" />{t("telemetryCockpit.colFuel")}
            </span>
          </th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => {
          const fuel = r.tire_change ? Math.max(0, r.box_secs - TIRE_EST_SECS) : r.box_secs;
          const tire = r.tire_change ? r.box_secs - fuel : 0;
          return (
            <tr
              key={r.car_idx}
              style={{ borderTop: "0.5px solid rgba(255,255,255,0.05)", background: r.isPlayer ? "rgba(88,166,255,0.08)" : undefined }}
            >
              <td className="py-2.5 text-center text-[12px]" style={{ color: "#6e7681", fontFamily: MONO }}>{i + 1}</td>
              <td className="py-2.5 px-2">
                <span className="flex items-center gap-2 min-w-0">
                  <TeamLogoMark teamName={r.team} color={colorFor[r.car_idx] ?? null} size="xs" />
                  <span className="text-[12.5px] truncate" style={{ color: r.isPlayer ? "#58a6ff" : "#c9d1d9" }}>{r.team}</span>
                </span>
              </td>
              <td className="py-2.5 px-2 text-[12.5px] truncate" style={{ color: r.isPlayer ? "#58a6ff" : "#9aa5b1" }}>
                <span className="flex items-center gap-1.5">
                  {r.pilot_name}
                  {r.track_wet && (
                    <Tooltip texto={t("telemetryCockpit.trackWet")}>
                      <img src={TIRE_WET_ICON} alt="" className="h-3.5 w-3.5 object-contain" />
                    </Tooltip>
                  )}
                </span>
              </td>
              <td className="py-2.5 px-2 text-right text-[13px] font-semibold" style={{ color: "#fff", fontFamily: MONO, fontVariantNumeric: "tabular-nums" }}>
                {r.box_secs.toFixed(2)}s
              </td>
              <td className="py-2.5 text-center text-[12px]" style={{ color: "#8b949e", fontFamily: MONO }}>{r.lap}</td>
              <td className="py-2.5 text-center text-[12px]" style={{ color: "#9aa5b1", fontFamily: MONO }}>
                {r.tire_change ? `~${Math.round(tire)}s` : "—"}
              </td>
              <td className="py-2.5 text-center text-[12px]" style={{ color: "#9aa5b1", fontFamily: MONO }}>
                {fuel >= 2 ? `~${Math.round(fuel)}s` : "—"}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function RivalTooltip({ active, payload, rivalName }) {
  const { t } = useTranslation();
  if (!active || !payload?.length) return null;
  const r = payload[0].payload;
  const ahead = r.gap_s >= 0;
  return (
    <div className="rounded-lg border border-white/15 px-3 py-2 text-[11px] shadow-lg" style={{ background: "rgba(10,15,22,0.96)" }}>
      <div style={{ color: "#fff" }} className="font-semibold">{t("telemetryCockpit.lap")} {r.lap}</div>
      <div style={{ color: "#f59e0b" }}>
        {rivalName || t("telemetryCockpit.rival")} {ahead ? t("telemetryCockpit.ahead") : t("telemetryCockpit.behind")}: {Math.abs(r.gap_s).toFixed(2)}s
      </div>
    </div>
  );
}

export default RaceTelemetryCockpit;
