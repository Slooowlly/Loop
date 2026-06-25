import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../../stores/useCareerStore";
import LapTimeLineChart from "../race/LapTimeLineChart";
import PaceDeltaChart from "../race/PaceDeltaChart";
import RaceTraceChart from "../race/RaceTraceChart";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ReferenceLine,
  ResponsiveContainer,
} from "recharts";

// Overlay de "iRacing Conectado": aparece quando o jogador volta o foco pro nosso
// app E o iRacing está rodando. Mostra um blur por cima do app, a mensagem de
// conexão e gráficos AO VIVO da sessão (variação de voltas, gap pro carro da
// frente, ritmo do jogador e o race pace do grid) — feedback de "tudo certo".

const AXIS_TICK = "#94a3b8";
const GRID = "rgba(255,255,255,0.07)";
const PLAYER_COLOR = "#58a6ff";
const PALETTE = [
  "#f59e0b", "#10b981", "#ec4899", "#8b5cf6", "#06b6d4", "#ef4444",
  "#a3e635", "#f97316", "#14b8a6", "#e879f9", "#facc15", "#34d399",
  "#fb7185", "#c084fc", "#22d3ee", "#fbbf24", "#4ade80", "#f472b6",
];

// Inatividade (sem mouse/teclado) que reabre a tela de feedback sozinha.
const IDLE_MS = 10000;

function formatLap(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "--";
  const m = Math.floor(seconds / 60);
  const s = seconds - m * 60;
  return `${m}:${s.toFixed(3).padStart(6, "0")}`;
}

function ChartCard({ title, hint, hasData, children }) {
  return (
    <div className="flex flex-col rounded-2xl border border-white/10 bg-[#0d131c]/80 p-3">
      <div className="mb-1 flex items-baseline justify-between">
        <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-text-secondary">{title}</p>
        {hint && <span className="text-[10px] text-text-muted">{hint}</span>}
      </div>
      {hasData ? (
        children
      ) : (
        <div className="flex flex-1 items-center justify-center py-8">
          <p className="text-center text-[11px] text-text-muted">Aguardando as primeiras voltas…</p>
        </div>
      )}
    </div>
  );
}

function IracingConnectedOverlay() {
  const [visible, setVisible] = useState(false);
  const [history, setHistory] = useState(null);
  const [selectedClass, setSelectedClass] = useState(null);
  const [carColors, setCarColors] = useState({ by_name: {}, player_color: null });
  const [paceDriver, setPaceDriver] = useState(null); // idx; null = jogador
  const [paceMenuOpen, setPaceMenuOpen] = useState(false);
  const lastInteractionRef = useRef(Date.now());
  const careerId = useCareerStore((s) => s.careerId);

  // Busca as cores dos times (por nome do piloto) quando o overlay abre.
  useEffect(() => {
    if (!visible || !careerId) return;
    invoke("iracing_car_colors", { careerId })
      .then((c) => setCarColors(c || { by_name: {}, player_color: null }))
      .catch(() => {});
  }, [visible, careerId]);

  // Marca a última interação do usuário (para o backtrack por inatividade).
  useEffect(() => {
    const mark = () => {
      lastInteractionRef.current = Date.now();
    };
    const events = ["mousemove", "mousedown", "keydown", "wheel", "touchstart"];
    events.forEach((e) => window.addEventListener(e, mark, { passive: true }));
    return () => events.forEach((e) => window.removeEventListener(e, mark));
  }, []);

  // Mostra quando o app ganha foco (alt-tab de volta) e o iRacing está conectado.
  useEffect(() => {
    async function showIfConnected() {
      try {
        const connected = await invoke("iracing_connected");
        if (connected) setVisible(true);
      } catch {
        /* ignore */
      }
    }
    function onVisibility() {
      if (document.visibilityState === "visible") showIfConnected();
    }
    window.addEventListener("focus", showIfConnected);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", showIfConnected);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);

  // Backtrack: 10s sem interação + iRacing conectado → reabre o feedback sozinho.
  useEffect(() => {
    if (visible) return undefined;
    const id = setInterval(async () => {
      if (Date.now() - lastInteractionRef.current < IDLE_MS) return;
      try {
        const connected = await invoke("iracing_connected");
        if (connected) setVisible(true);
      } catch {
        /* ignore */
      }
    }, 1000);
    return () => clearInterval(id);
  }, [visible]);

  // Enquanto aberto: puxa o histórico ao vivo e fecha se o iRacing desconectar.
  useEffect(() => {
    if (!visible) return undefined;
    let alive = true;
    async function tick() {
      try {
        const [h, connected] = await Promise.all([
          invoke("iracing_get_race_feedback"),
          invoke("iracing_connected"),
        ]);
        if (!alive) return;
        setHistory(h);
        if (!connected) setVisible(false);
      } catch {
        /* ignore */
      }
    }
    tick();
    const id = setInterval(tick, 1000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [visible]);

  // Esc fecha.
  useEffect(() => {
    if (!visible) return undefined;
    const onKey = (e) => {
      if (e.key === "Escape") setVisible(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible]);

  const playerLaps = useMemo(() => history?.player_laps ?? [], [history]);
  const track = useMemo(() => history?.player_track ?? [], [history]);
  const laps = useMemo(() => history?.laps ?? [], [history]);
  const yellowLaps = useMemo(() => history?.yellow_laps ?? [], [history]);
  const carsMeta = useMemo(() => history?.cars_meta ?? [], [history]);
  const driverNames = useMemo(() => history?.driver_names ?? {}, [history]);
  const pitSet = useMemo(() => new Set(history?.player_pit_laps ?? []), [history]);
  const playerIdx = history?.player_car_idx ?? -1;

  // Voltas "limpas": tempo válido, ignora a volta 1 (largada, naturalmente lenta)
  // e as voltas de pit (in/out lap). Base do ritmo E da variação — sem outliers.
  const cleanLaps = useMemo(
    () => playerLaps.filter((p) => p.time > 0 && p.lap > 1 && !pitSet.has(p.lap)),
    [playerLaps, pitSet],
  );

  // Ritmo do jogador (tempos absolutos das voltas limpas).
  const paceRows = useMemo(
    () => cleanLaps.map((p) => ({ lap: p.lap, time: p.time })),
    [cleanLaps],
  );

  // Seletor do "Variação de voltas": lista de pilotos (jogador no topo).
  const carLaps = useMemo(() => history?.car_laps ?? [], [history]);
  const paceDrivers = useMemo(() => {
    const out = [{ idx: playerIdx, name: driverNames[playerIdx] || "Você", isPlayer: true }];
    const others = [];
    for (const meta of carsMeta) {
      if (meta.is_pace || meta.idx === playerIdx) continue;
      others.push({ idx: meta.idx, name: driverNames[meta.idx] || `c${meta.idx}`, isPlayer: false });
    }
    others.sort((a, b) => a.name.localeCompare(b.name));
    return [...out, ...others];
  }, [carsMeta, driverNames, playerIdx]);

  const activePaceIdx = paceDriver ?? playerIdx;
  const isPlayerPace = activePaceIdx === playerIdx;
  const activePaceName =
    paceDrivers.find((d) => d.idx === activePaceIdx)?.name || driverNames[activePaceIdx] || "Você";

  // Variação de voltas do piloto selecionado (delta à média; ignora volta 1 e,
  // para o jogador, voltas de pit). Para a IA só dá pra ignorar a volta 1.
  const { deltaRows, avgLap } = useMemo(() => {
    let lapsArr;
    if (isPlayerPace) {
      lapsArr = cleanLaps.map((p) => ({ lap: p.lap, time: p.time }));
    } else {
      lapsArr = carLaps
        .filter((l) => l.car_idx === activePaceIdx && l.time > 0 && l.lap > 1)
        .map((l) => ({ lap: l.lap, time: l.time }));
    }
    if (!lapsArr.length) return { deltaRows: [], avgLap: 0 };
    const avg = lapsArr.reduce((a, r) => a + r.time, 0) / lapsArr.length;
    return { avgLap: avg, deltaRows: lapsArr.map((r) => ({ lap: r.lap, delta: r.time - avg })) };
  }, [isPlayerPace, cleanLaps, carLaps, activePaceIdx]);

  // Gap pro carro da frente (amostras ~2Hz onde há alguém à frente), com teto de
  // 20s para não estourar a escala em saídas de pit/incidentes.
  const gapRows = useMemo(
    () =>
      track
        .filter((t) => t.ahead_idx >= 0 && t.gap_ahead > 0)
        .map((t, i) => ({ i, gap: Math.min(t.gap_ahead, 20) })),
    [track],
  );

  // Classe (class_id) por carro + nomes das classes (para as abas).
  const classNames = useMemo(() => history?.class_names ?? {}, [history]);
  const classByIdx = useMemo(() => {
    const m = {};
    for (const meta of carsMeta) m[meta.idx] = meta.class_id;
    return m;
  }, [carsMeta]);

  // Categorias presentes (exclui pace car), com rótulo do nome curto.
  const classes = useMemo(() => {
    const counts = new Map();
    for (const meta of carsMeta) {
      if (meta.is_pace) continue;
      counts.set(meta.class_id, (counts.get(meta.class_id) ?? 0) + 1);
    }
    return [...counts.keys()].map((id) => ({
      id,
      label: classNames[id] || (id ? `Classe ${id}` : "Geral"),
    }));
  }, [carsMeta, classNames]);
  const multiClass = classes.length > 1;

  // Default/saneamento da classe selecionada = a do jogador (ou a primeira).
  useEffect(() => {
    if (!multiClass) {
      if (selectedClass !== null) setSelectedClass(null);
      return;
    }
    const valid = classes.some((c) => c.id === selectedClass);
    if (!valid) {
      const playerClass = classByIdx[playerIdx];
      const fallback = classes.some((c) => c.id === playerClass) ? playerClass : classes[0].id;
      setSelectedClass(fallback);
    }
  }, [multiClass, classes, classByIdx, playerIdx, selectedClass]);

  // Race trace: gap ao líder (da CLASSE filtrada) por volta — distância entre os
  // carros, igual ao Monitor de Corrida. Uma linha por carro + faixas de bandeira.
  const traceCars = useMemo(() => {
    const seen = new Map();
    for (const snap of laps) {
      for (const c of snap.cars) {
        if (!seen.has(c.idx)) seen.set(c.idx, c.idx === playerIdx);
      }
    }
    const inClass = (idx) => !multiClass || selectedClass == null || classByIdx[idx] === selectedClass;
    const others = [];
    const player = [];
    for (const [idx, isPlayer] of seen) {
      if (!inClass(idx)) continue;
      (isPlayer ? player : others).push({ idx, isPlayer });
    }
    return [...others, ...player]; // jogador por último → linha por cima
  }, [laps, playerIdx, multiClass, selectedClass, classByIdx]);

  const activeIdxSet = useMemo(() => new Set(traceCars.map((c) => c.idx)), [traceCars]);

  // Cor da linha = cor do time/carro (por nome do piloto); fallback paleta.
  const colorFor = useMemo(() => {
    const byName = carColors?.by_name ?? {};
    const map = {};
    let i = 0;
    for (const c of traceCars) {
      if (c.isPlayer) {
        map[c.idx] = carColors?.player_color || PLAYER_COLOR;
      } else {
        const nm = driverNames[c.idx];
        map[c.idx] = (nm && byName[nm]) || PALETTE[i++ % PALETTE.length];
      }
    }
    return map;
  }, [traceCars, carColors, driverNames]);

  // Pivot: uma linha por volta com c{idx} = gap ao líder DA CLASSE (o menor gap
  // entre os carros ativos vira o 0s da classe).
  const traceRows = useMemo(() => {
    const rows = [];
    for (const snap of laps) {
      let minGap = Infinity;
      for (const c of snap.cars) {
        if (activeIdxSet.has(c.idx) && Number.isFinite(c.gap)) minGap = Math.min(minGap, c.gap);
      }
      if (!Number.isFinite(minGap)) minGap = 0;
      const row = { lap: snap.lap };
      for (const c of snap.cars) {
        if (activeIdxSet.has(c.idx)) row[`c${c.idx}`] = c.gap - minGap;
      }
      rows.push(row);
    }
    return rows.sort((a, b) => a.lap - b.lap);
  }, [laps, activeIdxSet]);

  // Teto fixo de 20s: mantém o gráfico legível (não estica por causa de carros
  // lapeados/parados a 30s+).
  const gapCap = 20;

  const displayRows = useMemo(() => {
    return traceRows.map((row) => {
      const out = { lap: row.lap };
      for (const k in row) if (k !== "lap") out[k] = Math.min(row[k], gapCap);
      return out;
    });
  }, [traceRows]);

  // Nome do piloto por carro (preferido); cai pro número/idx se faltar.
  const nameByIdx = useMemo(() => {
    const m = {};
    for (const meta of carsMeta) {
      m[meta.idx] =
        driverNames[meta.idx] || (meta.car_number ? `#${meta.car_number}` : `c${meta.idx}`);
    }
    for (const idx in driverNames) {
      if (m[idx] == null) m[idx] = driverNames[idx];
    }
    return m;
  }, [carsMeta, driverNames]);

  // ── Pins do jogador no race trace (só do jogador) ──────────────────────────
  const playerIncidents = useMemo(() => history?.player_incidents ?? [], [history]);
  const playerInTrace = activeIdxSet.has(playerIdx);

  // Melhor volta do jogador (voltas limpas) → aro roxo.
  const bestLapNum = useMemo(() => {
    if (!cleanLaps.length) return null;
    let best = cleanLaps[0];
    for (const p of cleanLaps) if (p.time < best.time) best = p;
    return best.lap;
  }, [cleanLaps]);

  // Gap do jogador por volta (já relativo à classe e clampado), para posicionar
  // os pins na linha dele (interpolando em voltas fracionárias).
  const playerGapByLap = useMemo(() => {
    const m = new Map();
    for (const row of displayRows) {
      const v = row[`c${playerIdx}`];
      if (v != null) m.set(row.lap, v);
    }
    return m;
  }, [displayRows, playerIdx]);

  const playerPins = useMemo(() => {
    if (!playerInTrace) return [];
    const gapAt = (lapF) => {
      const lo = Math.floor(lapF);
      const hi = Math.ceil(lapF);
      const a = playerGapByLap.get(lo);
      const b = playerGapByLap.get(hi);
      if (a == null && b == null) return null;
      if (a == null) return b;
      if (b == null || hi === lo) return a;
      return a + (b - a) * (lapF - lo);
    };
    return playerIncidents
      .map((m, i) => {
        const y = gapAt(m.lap_f);
        if (y == null) return null;
        const kind = m.off_track ? "off" : `${m.points}x`;
        const color = m.off_track
          ? "#f59e0b"
          : m.points >= 4
          ? "#ef4444"
          : m.points >= 2
          ? "#fb923c"
          : "#eab308";
        return { key: `pin${i}`, x: m.lap_f, y, kind, color };
      })
      .filter(Boolean);
  }, [playerIncidents, playerInTrace, playerGapByLap]);

  const bestLapPin = useMemo(() => {
    if (!playerInTrace || bestLapNum == null) return null;
    const y = playerGapByLap.get(bestLapNum);
    return y == null ? null : { x: bestLapNum, y };
  }, [playerInTrace, bestLapNum, playerGapByLap]);

  if (!visible) return null;

  return (
    <div
      className="fixed inset-0 z-[150] flex items-center justify-center bg-[#06090e]/70 p-4 backdrop-blur-md animate-scale-in"
      onClick={() => setVisible(false)}
    >
      <div
        className="flex max-h-[92vh] w-full max-w-5xl flex-col overflow-hidden rounded-3xl border border-white/10 bg-[#0b0f16]/95 shadow-[0_30px_80px_rgba(0,0,0,0.7)]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Cabeçalho */}
        <div className="flex items-center justify-between border-b border-white/8 px-6 py-4">
          <div className="flex items-center gap-3">
            <span className="relative flex h-3 w-3">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-status-green/70" />
              <span className="relative inline-flex h-3 w-3 rounded-full bg-status-green" />
            </span>
            <div>
              <h3 className="text-lg font-bold tracking-tight text-text-primary">iRacing Conectado</h3>
              <p className="text-[11px] text-text-secondary">Tudo pronto — é só correr.</p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => setVisible(false)}
            className="rounded-lg px-3 py-1.5 text-[11px] font-semibold text-text-secondary transition-glass hover:bg-white/10 hover:text-text-primary"
          >
            Fechar (Esc)
          </button>
        </div>

        {/* Corpo: gráficos ao vivo */}
        <div className="grid gap-3 overflow-y-auto px-6 py-5">
          <div className="grid gap-3 md:grid-cols-3">
            {/* Variação de voltas (Pace Consistency) — escolhe o piloto */}
            <ChartCard
              title="Variação de voltas"
              hint={avgLap ? `méd ${formatLap(avgLap)}` : null}
              hasData={deltaRows.length >= 1}
            >
              <div className="relative mb-2">
                <button
                  type="button"
                  onClick={() => setPaceMenuOpen((o) => !o)}
                  className="flex items-center gap-1.5 rounded-lg bg-white/5 px-2.5 py-1 text-[11px] font-semibold text-text-primary hover:bg-white/10"
                >
                  {activePaceName}
                  {isPlayerPace ? " (você)" : ""}
                  <span className="text-text-muted">▾</span>
                </button>
                {paceMenuOpen && (
                  <div className="absolute left-0 top-full z-20 mt-1 max-h-52 w-52 overflow-y-auto rounded-xl border border-white/10 bg-[#0d131c] p-1 shadow-xl">
                    {paceDrivers.map((d) => (
                      <button
                        key={d.idx}
                        type="button"
                        onClick={() => {
                          setPaceDriver(d.idx);
                          setPaceMenuOpen(false);
                        }}
                        className="flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-[11px] text-text-secondary hover:bg-white/10 hover:text-text-primary"
                      >
                        <span>
                          {d.name}
                          {d.isPlayer ? " (você)" : ""}
                        </span>
                        {d.idx === activePaceIdx && <span className="text-[#58a6ff]">✓</span>}
                      </button>
                    ))}
                  </div>
                )}
              </div>
              <div className="h-[180px] w-full">
                <PaceDeltaChart rows={deltaRows} showXLabel={false} tickFontSize={10} yAxisWidth={38} />
              </div>
            </ChartCard>

            {/* Gap pro carro da frente */}
            <ChartCard title="Gap pro carro da frente" hint="ao vivo · teto 20s" hasData={gapRows.length >= 2}>
              <div className="h-[180px] w-full">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={gapRows} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
                    <CartesianGrid stroke={GRID} />
                    <XAxis dataKey="i" tick={false} stroke={GRID} height={6} />
                    <YAxis
                      domain={[0, 20]}
                      tick={{ fill: AXIS_TICK, fontSize: 10 }}
                      stroke={GRID}
                      width={38}
                      tickFormatter={(v) => `${v.toFixed(0)}s`}
                    />
                    <Tooltip
                      contentStyle={{ background: "#0a0f16", border: "1px solid rgba(255,255,255,0.15)", borderRadius: 8, fontSize: 11 }}
                      formatter={(v) => [`${v.toFixed(2)}s`, "gap à frente"]}
                      labelFormatter={() => ""}
                    />
                    <Line type="monotone" dataKey="gap" stroke="#f59e0b" strokeWidth={2} dot={false} isAnimationActive={false} />
                  </LineChart>
                </ResponsiveContainer>
              </div>
            </ChartCard>

            {/* Ritmo do usuário */}
            <ChartCard title="Ritmo do usuário" hint="tempo de volta" hasData={paceRows.length >= 1}>
              <div className="h-[180px] w-full">
                <LapTimeLineChart rows={paceRows} />
              </div>
            </ChartCard>
          </div>

          {/* Race trace — grid inteiro: distância entre os carros (gap ao líder),
              com as voltas de bandeira amarela desenhadas. */}
          <ChartCard
            title="Distância entre os carros"
            hint="gap ao líder · 🟡 bandeira"
            hasData={displayRows.length >= 2}
          >
            {multiClass && (
              <div className="mb-2 flex flex-wrap gap-1.5">
                {classes.map((c) => (
                  <button
                    key={c.id}
                    type="button"
                    onClick={() => setSelectedClass(c.id)}
                    className={[
                      "rounded-full px-3 py-1 text-[11px] font-semibold transition",
                      selectedClass === c.id
                        ? "bg-[#58a6ff]/20 text-[#58a6ff] ring-1 ring-[#58a6ff]/40"
                        : "bg-white/5 text-text-muted hover:text-text-primary",
                    ].join(" ")}
                  >
                    {c.label}
                  </button>
                ))}
              </div>
            )}
            <div className="h-[240px] w-full">
              <RaceTraceChart
                rows={displayRows}
                cars={traceCars.map((c) => ({ idx: c.idx, isPlayer: c.isPlayer }))}
                colorForCar={(idx) => colorFor[idx]}
                nameByIdx={nameByIdx}
                mode="gap"
                lapDomain={["dataMin", "dataMax"]}
                gapCap={gapCap}
                yellowLaps={yellowLaps}
                playerPins={playerPins}
                bestLapPin={bestLapPin}
                tickFontSize={10}
              />
            </div>

            {/* Legenda: nome do piloto na cor do time/carro */}
            {traceCars.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1">
                {traceCars.map((c) => (
                  <span key={c.idx} className="flex items-center gap-1 text-[10px] text-text-secondary">
                    <span className="inline-block h-[3px] w-3.5 rounded" style={{ background: colorFor[c.idx] }} />
                    {nameByIdx[c.idx]}
                    {c.isPlayer ? " (você)" : ""}
                  </span>
                ))}
              </div>
            )}
          </ChartCard>
        </div>
      </div>
    </div>
  );
}

export default IracingConnectedOverlay;
