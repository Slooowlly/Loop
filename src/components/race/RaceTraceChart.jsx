import { useMemo } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceArea,
  ReferenceDot,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { AXIS_TICK, BAD, GOOD, GRID, YELLOW } from "../../utils/chartTheme";
import i18n from "../../i18n/index.js";

// Race trace COMPARTILHADO (posição/gap por volta, uma linha por carro), usado pelo
// pós-corrida da carreira, pelo pós real do iRacing e pelo overlay ao vivo. O MIOLO é
// o mesmo nos três; as anotações que diferem por contexto entram como props OPCIONAIS:
//   - linhas verticais de melhor/erro (bestLap/mistakeLap) — usadas no pós-corrida
//   - pins de incidente do jogador (playerPins) + aro da melhor volta (bestLapPin) — ao vivo
// Cada tela mantém o seu "chrome" (chips de modo, legenda, altura) por fora.
//
// Props:
//   rows         [{ lap, c{idx}: number }]  (já pivotado e, no modo gap, com teto aplicado)
//   cars         [{ idx, isPlayer }]        (ordem de desenho; jogador por último/por cima)
//   colorForCar  (idx) => cor
//   nameByIdx    { idx: nome }              (tooltip)
//   mode         "gap" | "position"
//   lapDomain    [min, max]                 (eixo X)
//   gapCap       número | null              (teto do eixo no modo gap)
//   yellowLaps   number[]                   (faixas de bandeira amarela)
//   bestLap/mistakeLap  number              (linhas verticais; 0 = não desenha)
//   playerPins   [{ key, x, y, color, kind }]
//   bestLapPin   { x, y } | null
//   tickFontSize número                     (10 no overlay menor, 11 nos painéis)

// Rótulo das linhas verticais (★ melhor momento / ✕ erro mais caro) COM tooltip nativo:
// um <title> + uma área de hover transparente larga pra ficar fácil de mirar.
function MarkerLabel({ viewBox, glyph, color, tip, fontSize = 13 }) {
  if (!viewBox) return null;
  const { x, y } = viewBox;
  return (
    <g style={{ cursor: "help" }}>
      <title>{tip}</title>
      <rect x={x - 10} y={y - 4} width={20} height={22} fill="transparent" />
      <text x={x} y={y + 11} textAnchor="middle" fill={color} fontSize={fontSize} fontWeight={700}>
        {glyph}
      </text>
    </g>
  );
}

function TraceTooltip({ active, payload, label, mode, nameByIdx }) {
  if (!active || !payload?.length) return null;
  const rows = payload
    .filter((p) => p.value != null)
    .sort((a, b) => a.value - b.value)
    .slice(0, 12);
  return (
    <div className="rounded-lg border border-white/15 bg-[#0a0f16]/95 px-3 py-2 text-[11px] shadow-lg backdrop-blur">
      <div className="mb-1 font-semibold text-white">
        Volta {typeof label === "number" ? label.toFixed(1) : label}
      </div>
      {rows.map((p) => {
        const idx = Number(p.dataKey.slice(1));
        return (
          <div key={p.dataKey} className="flex items-center justify-between gap-3">
            <span className="flex items-center gap-1.5" style={{ color: p.color }}>
              <span className="inline-block h-2 w-2 rounded-full" style={{ background: p.color }} />
              {nameByIdx?.[idx] ?? `Carro ${idx}`}
            </span>
            <span className="tabular-nums text-gray-400">
              {mode === "gap" ? `+${p.value.toFixed(2)}s` : `P${p.value}`}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function RaceTraceChart({
  rows,
  cars,
  colorForCar,
  nameByIdx,
  mode = "gap",
  lapDomain = ["dataMin", "dataMax"],
  gapCap = null,
  yellowLaps = [],
  bestLap = 0,
  mistakeLap = 0,
  // Rótulos das duas linhas verticais. Vêm do i18n em vez de literal PT: nenhuma das quatro
  // telas que montam este gráfico passa a prop, então o default é o que o jogador SEMPRE lê.
  bestLabel = i18n.t("raceCharts.bestMoment"),
  mistakeLabel = i18n.t("raceCharts.costliestMistake"),
  playerPins = [],
  bestLapPin = null,
  tickFontSize = 11,
}) {
  // Eixo X: as voltas do trace são FRACIONÁRIAS (volta do líder + progresso dele),
  // então um domínio numérico exato faz o recharts interpolar os ticks a partir das
  // pontas e imprimir coisas como "4.000019819304725". Arredondamos o domínio para
  // inteiros e ditamos os ticks — o dado continua fracionário, só o rótulo é inteiro.
  const lapAxis = useMemo(() => {
    const nums = (rows ?? []).map((r) => r.lap).filter((v) => Number.isFinite(v));
    const rawMin = typeof lapDomain?.[0] === "number" ? lapDomain[0] : Math.min(...nums);
    const rawMax = typeof lapDomain?.[1] === "number" ? lapDomain[1] : Math.max(...nums);
    if (!Number.isFinite(rawMin) || !Number.isFinite(rawMax) || rawMax < rawMin) {
      return { domain: lapDomain, ticks: undefined };
    }
    const min = Math.max(0, Math.floor(rawMin));
    const max = Math.max(min + 1, Math.ceil(rawMax));
    const step = Math.max(1, Math.ceil((max - min) / 8));
    const ticks = [];
    for (let v = min; v <= max; v += step) ticks.push(v);
    if (ticks[ticks.length - 1] !== max) ticks.push(max);
    return { domain: [min, max], ticks };
  }, [rows, lapDomain]);

  return (
    <ResponsiveContainer width="100%" height="100%">
      <LineChart data={rows} margin={{ top: 12, right: 12, bottom: 18, left: 4 }}>
        <CartesianGrid stroke={GRID} />
        {yellowLaps?.map((lap) => (
          <ReferenceArea key={`y${lap}`} x1={lap - 0.5} x2={lap + 0.5} fill={YELLOW} fillOpacity={0.14} stroke="none" />
        ))}
        {bestLap > 0 && (
          <ReferenceLine
            x={bestLap}
            stroke={GOOD}
            strokeDasharray="4 3"
            strokeOpacity={0.9}
            label={(props) => (
              <MarkerLabel
                {...props}
                glyph="★"
                color={GOOD}
                tip={i18n.t("raceCharts.markerTip", { glyph: "★", label: bestLabel, lap: bestLap })}
              />
            )}
          />
        )}
        {mistakeLap > 0 && (
          <ReferenceLine
            x={mistakeLap}
            stroke={BAD}
            strokeDasharray="4 3"
            strokeOpacity={0.9}
            label={(props) => (
              <MarkerLabel
                {...props}
                glyph="✕"
                color={BAD}
                tip={i18n.t("raceCharts.markerTip", { glyph: "✕", label: mistakeLabel, lap: mistakeLap })}
                fontSize={12}
              />
            )}
          />
        )}
        <XAxis
          type="number"
          dataKey="lap"
          domain={lapAxis.domain}
          ticks={lapAxis.ticks}
          allowDecimals={false}
          tick={{ fill: AXIS_TICK, fontSize: tickFontSize }}
          stroke={GRID}
          label={{ value: "Volta", position: "insideBottom", offset: -8, fill: AXIS_TICK, fontSize: tickFontSize }}
        />
        <YAxis
          reversed
          allowDecimals={false}
          domain={mode === "gap" ? [0, gapCap ?? "dataMax"] : [1, "dataMax"]}
          tick={{ fill: AXIS_TICK, fontSize: tickFontSize }}
          stroke={GRID}
          width={44}
          tickFormatter={(v) => (mode === "gap" ? `${v}s` : `P${v}`)}
        />
        <Tooltip content={<TraceTooltip mode={mode} nameByIdx={nameByIdx} />} />
        {cars.map((car) => (
          <Line
            key={car.idx}
            type="monotone"
            dataKey={`c${car.idx}`}
            stroke={colorForCar(car.idx)}
            strokeWidth={car.isPlayer ? 3 : 1.3}
            strokeOpacity={car.isPlayer ? 1 : 0.6}
            dot={false}
            activeDot={car.isPlayer ? { r: 4 } : false}
            connectNulls
            isAnimationActive={false}
          />
        ))}
        {playerPins?.map((p) => (
          <ReferenceDot
            key={p.key}
            x={p.x}
            y={p.y}
            r={3.5}
            fill={p.color}
            stroke="#0b0f16"
            strokeWidth={1}
            isFront
            label={{ value: p.kind, position: "top", fill: p.color, fontSize: 9 }}
          />
        ))}
        {bestLapPin && (
          <ReferenceDot
            x={bestLapPin.x}
            y={bestLapPin.y}
            r={6}
            fill="none"
            stroke="#a855f7"
            strokeWidth={2.5}
            isFront
            label={{ value: "★", position: "bottom", fill: "#a855f7", fontSize: 11 }}
          />
        )}
      </LineChart>
    </ResponsiveContainer>
  );
}

export default RaceTraceChart;
