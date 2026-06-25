import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

// "Variação de voltas" / Pace Consistency COMPARTILHADO: barras do delta de cada volta
// em relação à média. Usado pelo pós-corrida da carreira, pelo pós real do iRacing e
// pelo overlay ao vivo. O miolo é o mesmo; o que diverge entra como prop opcional:
//   - linhas verticais de melhor/erro (bestLap/mistakeLap) e faixas amarelas — pós-carreira
//   - destaque da volta mais rápida: automático quando a linha tem `isBest` (ao vivo não tem)
//   - rótulo "Volta" no eixo X (showXLabel) — o overlay é compacto e não usa
//
// rows: [{ lap, delta, isBest?, time? }]  (delta em segundos vs média; time opcional p/ tooltip)

const AXIS_TICK = "#94a3b8";
const GRID = "rgba(255,255,255,0.07)";
const YELLOW = "#facc15";
const GOOD = "#22c55e";
const BAD = "#ef4444";

function formatLap(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "--";
  const m = Math.floor(seconds / 60);
  const s = seconds - m * 60;
  return `${m}:${s.toFixed(3).padStart(6, "0")}`;
}

function PaceTooltip({ active, payload }) {
  if (!active || !payload?.length) return null;
  const r = payload[0].payload;
  return (
    <div className="rounded-lg border border-white/15 bg-[#0a0f16]/95 px-3 py-2 text-[11px] shadow-lg backdrop-blur">
      <div className="font-semibold text-white">Volta {r.lap}</div>
      {Number.isFinite(r.time) && <div className="text-gray-400">{formatLap(r.time)}</div>}
      <div className={r.delta > 0 ? "text-red-400" : "text-green-400"}>
        {r.delta > 0 ? "+" : ""}
        {r.delta.toFixed(2)}s à média
      </div>
    </div>
  );
}

function PaceDeltaChart({
  rows,
  yellowLaps = [],
  bestLap = 0,
  mistakeLap = 0,
  showXLabel = true,
  tickFontSize = 11,
  yAxisWidth = 44,
}) {
  return (
    <ResponsiveContainer width="100%" height="100%">
      <BarChart data={rows} margin={{ top: 12, right: 12, bottom: showXLabel ? 18 : 4, left: 4 }}>
        <CartesianGrid stroke={GRID} vertical={false} />
        {yellowLaps?.map((lap) => (
          <ReferenceArea key={`y${lap}`} x1={lap - 0.5} x2={lap + 0.5} fill={YELLOW} fillOpacity={0.14} stroke="none" />
        ))}
        {bestLap > 0 && (
          <ReferenceLine
            x={bestLap}
            stroke={GOOD}
            strokeDasharray="4 3"
            strokeOpacity={0.9}
            label={{ value: "★", position: "top", fill: GOOD, fontSize: 13 }}
          />
        )}
        {mistakeLap > 0 && (
          <ReferenceLine
            x={mistakeLap}
            stroke={BAD}
            strokeDasharray="4 3"
            strokeOpacity={0.9}
            label={{ value: "✕", position: "top", fill: BAD, fontSize: 12 }}
          />
        )}
        <XAxis
          dataKey="lap"
          tick={{ fill: AXIS_TICK, fontSize: tickFontSize }}
          stroke={GRID}
          label={
            showXLabel
              ? { value: "Volta", position: "insideBottom", offset: -8, fill: AXIS_TICK, fontSize: tickFontSize }
              : undefined
          }
        />
        <YAxis
          tick={{ fill: AXIS_TICK, fontSize: tickFontSize }}
          stroke={GRID}
          width={yAxisWidth}
          tickFormatter={(v) => `${v > 0 ? "+" : ""}${v.toFixed(1)}s`}
        />
        <Tooltip content={<PaceTooltip />} />
        <ReferenceLine y={0} stroke={AXIS_TICK} strokeOpacity={0.5} />
        <Bar dataKey="delta" radius={[3, 3, 0, 0]} isAnimationActive={false}>
          {rows.map((r) => (
            <Cell key={r.lap} fill={r.isBest ? GOOD : r.delta > 0 ? BAD : "#16a34a"} />
          ))}
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  );
}

export default PaceDeltaChart;
