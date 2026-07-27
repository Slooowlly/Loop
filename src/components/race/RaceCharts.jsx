import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../../stores/useCareerStore";
import LapTimeLineChart from "./LapTimeLineChart";
import RaceTraceChart from "./RaceTraceChart";
import PaceDeltaChart from "./PaceDeltaChart";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
} from "recharts";
import { AXIS_TICK, BAD, GOOD, GRID, PALETTE, PLAYER_COLOR, YELLOW } from "../../utils/chartTheme";
import { formatLapSeconds } from "../../utils/formatters";

// Gráficos do pós-corrida (race trace, posição, tempos de volta, gap pro rival),
// com marcações do melhor momento (verde) e do erro mais caro (vermelho). Os
// dados vêm empacotados em `telemetry.charts` (capturados no import).

function TabButton({ active, onClick, children }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "rounded-lg px-3 py-1.5 text-xs font-semibold transition",
        active ? "bg-[#58a6ff]/20 text-[#58a6ff]" : "text-gray-400 hover:text-white",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

function ModeChip({ active, onClick, children }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "rounded-md px-2.5 py-1 text-[11px] font-semibold transition",
        active ? "bg-white/15 text-white" : "bg-white/5 text-gray-400 hover:text-white",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

// Faixas amarelas + linhas-marcação (melhor momento / erro mais caro) reusadas
// nos gráficos que têm "volta" no eixo X.
function LapMarkers({ yellowLaps, bestLap, mistakeLap }) {
  return (
    <>
      {yellowLaps?.map((lap) => (
        <ReferenceArea key={`y${lap}`} x1={lap - 0.5} x2={lap + 0.5} fill={YELLOW} fillOpacity={0.14} stroke="none" />
      ))}
      {bestLap > 0 && (
        <ReferenceLine x={bestLap} stroke={GOOD} strokeDasharray="4 3" strokeOpacity={0.9}
          label={{ value: "★", position: "top", fill: GOOD, fontSize: 13 }} />
      )}
      {mistakeLap > 0 && (
        <ReferenceLine x={mistakeLap} stroke={BAD} strokeDasharray="4 3" strokeOpacity={0.9}
          label={{ value: "✕", position: "top", fill: BAD, fontSize: 12 }} />
      )}
    </>
  );
}

function RaceCharts({ charts, mistakeLap = 0, bestMomentLap = 0 }) {
  const { t } = useTranslation();
  const [tab, setTab] = useState("trace");
  const [traceMode, setTraceMode] = useState("position");
  const [carColors, setCarColors] = useState({ by_name: {}, player_color: null });
  const [paceIdx, setPaceIdx] = useState(null); // idx do piloto no pace; null = jogador
  const [paceMenuOpen, setPaceMenuOpen] = useState(false);
  const careerId = useCareerStore((s) => s.careerId);

  const cars = charts?.cars ?? [];
  const lapTimes = charts?.lap_times ?? [];
  const carLapTimes = charts?.car_lap_times ?? [];
  const rivalGap = charts?.rival_gap ?? [];
  const yellowLaps = charts?.yellow_laps ?? [];

  // Cores dos times (por nome do piloto) para colorir as linhas.
  useEffect(() => {
    if (!careerId) return;
    invoke("iracing_car_colors", { careerId })
      .then((c) => setCarColors(c || { by_name: {}, player_color: null }))
      .catch(() => {});
  }, [careerId]);

  // Carros ordenados com o jogador por ÚLTIMO (linha dele por cima).
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

  // Pivot do trace: uma linha por volta com c{idx} = gap OU posição.
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

  // Teto do eixo de gap (apara sentinelas de carros parados/DQ).
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

  // Pace Consistency: piloto selecionável (jogador no topo).
  const playerIdx = useMemo(() => cars.find((c) => c.is_player)?.idx ?? -1, [cars]);
  const paceDrivers = useMemo(() => {
    const out = [];
    const player = cars.find((c) => c.is_player);
    if (player) out.push({ idx: player.idx, name: player.name, isPlayer: true });
    const others = cars
      .filter((c) => !c.is_player)
      .map((c) => ({ idx: c.idx, name: c.name, isPlayer: false }));
    others.sort((a, b) => a.name.localeCompare(b.name));
    return [...out, ...others];
  }, [cars]);
  const activePaceIdx = paceIdx ?? playerIdx;
  const activePaceName =
    paceDrivers.find((d) => d.idx === activePaceIdx)?.name || t("raceCharts.youLabel");
  const hasPace = lapTimes.length > 0 || carLapTimes.length > 0;

  // Tempos de volta do piloto selecionado: delta à média, melhor volta destacada.
  const pace = useMemo(() => {
    const isPlayer = activePaceIdx === playerIdx;
    const src =
      isPlayer && lapTimes.length
        ? lapTimes.map((p) => ({ lap: p.lap, time_s: p.time_s }))
        : carLapTimes.filter((l) => l.idx === activePaceIdx).map((l) => ({ lap: l.lap, time_s: l.time_s }));
    if (!src.length) return null;
    const times = src.map((p) => p.time_s);
    const avg = times.reduce((a, b) => a + b, 0) / times.length;
    const best = Math.min(...times);
    return {
      avg,
      best,
      rows: src.map((p) => ({ lap: p.lap, delta: p.time_s - avg, time: p.time_s, isBest: p.time_s === best })),
    };
  }, [activePaceIdx, playerIdx, lapTimes, carLapTimes]);

  const nameByIdx = useMemo(() => {
    const m = {};
    for (const c of cars) m[c.idx] = c.name;
    return m;
  }, [cars]);

  const hasTrace = traceRows.length >= 2 && cars.length > 0;

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        {hasTrace && (
          <TabButton active={tab === "trace"} onClick={() => setTab("trace")}>{t("raceCharts.raceTrace")}</TabButton>
        )}
        {hasPace && <TabButton active={tab === "pace"} onClick={() => setTab("pace")}>{t("raceCharts.paceConsistency")}</TabButton>}
        {hasPace && <TabButton active={tab === "ritmo"} onClick={() => setTab("ritmo")}>{t("raceCharts.pace")}</TabButton>}
        {rivalGap.length >= 2 && (
          <TabButton active={tab === "rival"} onClick={() => setTab("rival")}>{t("raceCharts.rivalGap")}</TabButton>
        )}
        <span className="ml-auto flex items-center gap-3 text-[11px] text-gray-400">
          <span className="flex items-center gap-1"><span className="inline-block h-[3px] w-4 rounded bg-[#58a6ff]" />{t("raceCharts.you")}</span>
          {bestMomentLap > 0 && <span className="flex items-center gap-1 text-green-400">★ {t("raceCharts.bestLegend")}</span>}
          {mistakeLap > 0 && <span className="flex items-center gap-1 text-red-400">✕ {t("raceCharts.mistakeLegend")}</span>}
        </span>
      </div>

      {/* RACE TRACE / POSIÇÃO */}
      {tab === "trace" && hasTrace && (
        <div className="space-y-2">
          <div className="flex gap-1.5">
            <ModeChip active={traceMode === "position"} onClick={() => setTraceMode("position")}>{t("raceCharts.position")}</ModeChip>
            <ModeChip active={traceMode === "gap"} onClick={() => setTraceMode("gap")}>{t("raceCharts.gapToLeader")}</ModeChip>
          </div>
          <div className="h-[340px] w-full">
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

          {/* Legenda: nome do piloto na cor do time/carro */}
          <div className="flex flex-wrap gap-x-3 gap-y-1">
            {orderedCars.map((car) => (
              <span key={car.idx} className="flex items-center gap-1 text-[10px] text-gray-400">
                <span className="inline-block h-[3px] w-3.5 rounded" style={{ background: colorFor[car.idx] }} />
                {car.name}
                {car.is_player ? t("raceCharts.youParen") : ""}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* PACE CONSISTENCY (tempos de volta) — piloto selecionável */}
      {(tab === "pace" || tab === "ritmo") && hasPace && (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
            <div className="relative">
              <button
                type="button"
                onClick={() => setPaceMenuOpen((o) => !o)}
                className="flex items-center gap-1.5 rounded-lg bg-white/5 px-3 py-1.5 text-xs font-semibold text-white hover:bg-white/10"
              >
                {activePaceName}
                {activePaceIdx === playerIdx ? t("raceCharts.youParen") : ""}
                <span className="text-gray-400">▾</span>
              </button>
              {paceMenuOpen && (
                <div className="absolute left-0 top-full z-20 mt-1 max-h-60 w-56 overflow-y-auto rounded-xl border border-white/10 bg-[#0d131c] p-1 shadow-xl">
                  {paceDrivers.map((d) => (
                    <button
                      key={d.idx}
                      type="button"
                      onClick={() => {
                        setPaceIdx(d.idx);
                        setPaceMenuOpen(false);
                      }}
                      className="flex w-full items-center justify-between rounded-lg px-3 py-1.5 text-left text-xs text-gray-300 hover:bg-white/10 hover:text-white"
                    >
                      <span>
                        {d.name}
                        {d.isPlayer ? t("raceCharts.youParen") : ""}
                      </span>
                      {d.idx === activePaceIdx && <span className="text-[#58a6ff]">✓</span>}
                    </button>
                  ))}
                </div>
              )}
            </div>
            {pace && (
              <div className="flex flex-wrap items-center gap-x-6 gap-y-1 text-xs text-gray-400">
                <span>{t("raceCharts.avgLabel")} <span className="font-semibold text-white">{formatLapSeconds(pace.avg)}</span></span>
                <span>{t("raceCharts.bestLabel")} <span className="font-semibold text-green-400">{formatLapSeconds(pace.best)}</span></span>
              </div>
            )}
          </div>
          {!pace ? (
            <div className="flex h-[300px] items-center justify-center text-xs text-gray-500">
              {t("raceCharts.noLaps")}
            </div>
          ) : tab === "ritmo" ? (
            // Linha de tempo de volta — MESMO gráfico do iRacing Conectado (componente
            // compartilhado), aqui com o piloto selecionado no seletor acima.
            <div className="h-[300px] w-full">
              <LapTimeLineChart rows={pace.rows} color={colorFor[activePaceIdx] || PLAYER_COLOR} />
            </div>
          ) : (
          <div className="h-[300px] w-full">
            <PaceDeltaChart rows={pace.rows} bestLap={bestMomentLap} mistakeLap={mistakeLap} />
          </div>
          )}
        </div>
      )}

      {/* GAP PRO RIVAL */}
      {tab === "rival" && rivalGap.length >= 2 && (
        <div className="space-y-2">
          <p className="text-[11px] text-gray-400">
            {t("raceCharts.rivalIntroPre")}<span className="font-semibold text-white">{charts.rival_name || t("raceCharts.rivalFallback")}</span>{t("raceCharts.rivalIntroPost")}
          </p>
          <div className="h-[280px] w-full">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={rivalGap} margin={{ top: 12, right: 12, bottom: 18, left: 4 }}>
                <CartesianGrid stroke={GRID} />
                <LapMarkers yellowLaps={yellowLaps} bestLap={bestMomentLap} mistakeLap={mistakeLap} />
                <XAxis type="number" dataKey="lap" allowDecimals={false}
                  tick={{ fill: AXIS_TICK, fontSize: 11 }} stroke={GRID}
                  label={{ value: t("raceCharts.lap"), position: "insideBottom", offset: -8, fill: AXIS_TICK, fontSize: 11 }} />
                <YAxis tick={{ fill: AXIS_TICK, fontSize: 11 }} stroke={GRID} width={44}
                  tickFormatter={(v) => `${Math.abs(v).toFixed(1)}s`} />
                <Tooltip content={<RivalTooltip rivalName={charts.rival_name} />} />
                <ReferenceLine y={0} stroke={PLAYER_COLOR} strokeOpacity={0.7} />
                <Line type="monotone" dataKey="gap_s" stroke="#f59e0b" strokeWidth={2} dot={{ r: 2 }}
                  connectNulls isAnimationActive={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </div>
      )}
    </div>
  );
}

function RivalTooltip({ active, payload, rivalName }) {
  const { t } = useTranslation();
  if (!active || !payload?.length) return null;
  const r = payload[0].payload;
  const ahead = r.gap_s >= 0;
  return (
    <div className="rounded-lg border border-white/15 bg-[#0a0f16]/95 px-3 py-2 text-[11px] shadow-lg backdrop-blur">
      <div className="font-semibold text-white">{t("raceCharts.lap")} {r.lap}</div>
      <div className="text-[#f59e0b]">
        {rivalName || t("raceCharts.rival")} {ahead ? t("raceCharts.ahead") : t("raceCharts.behind")}: {Math.abs(r.gap_s).toFixed(2)}s
      </div>
    </div>
  );
}

export default RaceCharts;
