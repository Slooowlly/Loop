import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { AXIS_TICK, GRID, PLAYER_COLOR } from "../../utils/chartTheme";
import { formatLapSeconds } from "../../utils/formatters";

// Gráfico ÚNICO de "Ritmo" (linha de tempo de volta), compartilhado entre o overlay
// do iRacing Conectado (ao vivo) e o pós-corrida (RaceCharts). Fonte única → os dois
// ficam sempre idênticos. Recebe `rows` = [{ lap, time }]; o `time` é em segundos.
// O contêiner de altura fica por conta de quem usa (overlay 180px, pós-corrida 300px).

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
          tickFormatter={(v) => formatLapSeconds(v)}
        />
        <Tooltip
          contentStyle={{
            background: "#0a0f16",
            border: "1px solid rgba(255,255,255,0.15)",
            borderRadius: 8,
            fontSize: 11,
          }}
          formatter={(v) => [formatLapSeconds(v), "volta"]}
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
