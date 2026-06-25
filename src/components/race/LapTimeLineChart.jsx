import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

// Gráfico ÚNICO de "Ritmo" (linha de tempo de volta), compartilhado entre o overlay
// do iRacing Conectado (ao vivo) e o pós-corrida (RaceCharts). Fonte única → os dois
// ficam sempre idênticos. Recebe `rows` = [{ lap, time }]; o `time` é em segundos.
// O contêiner de altura fica por conta de quem usa (overlay 180px, pós-corrida 300px).

const AXIS_TICK = "#94a3b8";
const GRID = "rgba(255,255,255,0.07)";
const PLAYER_COLOR = "#58a6ff";

function formatLap(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "--";
  const m = Math.floor(seconds / 60);
  const s = seconds - m * 60;
  return `${m}:${s.toFixed(3).padStart(6, "0")}`;
}

function LapTimeLineChart({ rows, color = PLAYER_COLOR }) {
  return (
    <ResponsiveContainer width="100%" height="100%">
      <LineChart data={rows} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
        <CartesianGrid stroke={GRID} />
        <XAxis dataKey="lap" tick={{ fill: AXIS_TICK, fontSize: 10 }} stroke={GRID} />
        <YAxis
          tick={{ fill: AXIS_TICK, fontSize: 10 }}
          stroke={GRID}
          width={48}
          domain={["dataMin - 0.5", "dataMax + 0.5"]}
          tickFormatter={(v) => formatLap(v)}
        />
        <Tooltip
          contentStyle={{
            background: "#0a0f16",
            border: "1px solid rgba(255,255,255,0.15)",
            borderRadius: 8,
            fontSize: 11,
          }}
          formatter={(v) => [formatLap(v), "volta"]}
          labelFormatter={(l) => `Volta ${l}`}
        />
        <Line
          type="monotone"
          dataKey="time"
          stroke={color}
          strokeWidth={2.5}
          dot={{ r: 2 }}
          isAnimationActive={false}
        />
      </LineChart>
    </ResponsiveContainer>
  );
}

export default LapTimeLineChart;
