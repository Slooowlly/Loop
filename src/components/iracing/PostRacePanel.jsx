import { useState, useMemo, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  LineChart,
  Line,
  BarChart,
  Bar,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
} from "recharts";
import GlassCard from "../ui/GlassCard";
import GlassButton from "../ui/GlassButton";

// Paleta distinta e legível no tema escuro para as linhas dos carros.
const PALETTE = [
  "#f59e0b", "#10b981", "#ec4899", "#8b5cf6", "#06b6d4", "#ef4444",
  "#a3e635", "#f97316", "#14b8a6", "#e879f9", "#facc15", "#34d399",
  "#fb7185", "#c084fc", "#22d3ee", "#fbbf24", "#4ade80", "#f472b6",
];
const PLAYER_COLOR = "#3b82f6";
const AXIS_TICK = "#94a3b8";
const GRID = "rgba(255,255,255,0.07)";
const YELLOW = "#facc15";

/** Segundos → "m:ss.mmm" (tempo de volta). */
function formatLap(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "--";
  const m = Math.floor(seconds / 60);
  const s = seconds - m * 60;
  return `${m}:${s.toFixed(3).padStart(6, "0")}`;
}

/** Segundos → "+s.mmm" (delta com sinal). */
function formatDelta(seconds) {
  const sign = seconds > 0 ? "+" : seconds < 0 ? "−" : "";
  return `${sign}${Math.abs(seconds).toFixed(2)}s`;
}

/** Segundos decorridos → "m:ss" (relógio da corrida). */
function formatClock(seconds) {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

const BATTLE_CAP = 30; // teto (s) do gráfico de batalha — briga é coisa de perto.

function PostRacePanel() {
  const [history, setHistory] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [tab, setTab] = useState("trace"); // trace | pace
  const [traceMode, setTraceMode] = useState("gap"); // gap | position
  const [auto, setAuto] = useState(true);
  const [saveMsg, setSaveMsg] = useState("");
  const [savedList, setSavedList] = useState([]);
  const [viewingSaved, setViewingSaved] = useState(null); // nome do arquivo, ou null = ao vivo
  // Assinatura do último dado aplicado — evita re-render (e recálculo dos
  // gráficos) quando nada mudou. É aqui que mora a performance: a corrida
  // terminou → assinatura constante → nenhuma re-renderização.
  const sigRef = useRef("");
  // Tentativa já auto-salva (evita salvar a mesma corrida toda hora).
  const autoSavedRef = useRef(-1);

  const refreshSavedList = useCallback(async () => {
    try {
      setSavedList(await invoke("iracing_list_saved_races"));
    } catch {
      /* sem pasta ainda — ok */
    }
  }, []);

  const saveRace = useCallback(
    async (silent) => {
      try {
        const name = await invoke("iracing_save_race_history");
        if (!silent) setSaveMsg(`Salva: ${name}`);
        refreshSavedList();
        return name;
      } catch (e) {
        if (!silent) setSaveMsg(String(e));
        return null;
      }
    },
    [refreshSavedList],
  );

  const fetchHistory = useCallback(
    async (silent) => {
      if (!silent) setLoading(true);
      if (!silent) setError("");
      try {
        const data = await invoke("iracing_get_race_history");
        const sig = `${data.attempt_number}:${data.laps.length}:${data.player_laps.length}:${data.yellow_laps.length}:${data.finished}`;
        if (sig !== sigRef.current || !silent) {
          sigRef.current = sig;
          setHistory(data);
        }
        // Auto-salva uma vez quando a corrida encerra (e gerou dados).
        const hasData =
          data.laps.length > 0 || data.player_track.length > 0;
        if (
          data.finished &&
          hasData &&
          autoSavedRef.current !== data.attempt_number
        ) {
          autoSavedRef.current = data.attempt_number;
          const name = await saveRace(true);
          if (name) setSaveMsg(`Corrida salva automaticamente: ${name}`);
        }
      } catch (e) {
        if (!silent) setError(String(e));
      } finally {
        if (!silent) setLoading(false);
      }
    },
    [saveRace],
  );

  const load = useCallback(() => fetchHistory(false), [fetchHistory]);

  const openSaved = useCallback(async (name) => {
    if (!name) return;
    try {
      const data = await invoke("iracing_load_saved_race", { name });
      sigRef.current = `saved:${name}`;
      setHistory(data);
      setViewingSaved(name);
      setAuto(false); // congela o ao vivo enquanto revê
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const backToLive = useCallback(() => {
    setViewingSaved(null);
    sigRef.current = "";
    setAuto(true);
  }, []);

  // Auto-atualização leve: puxa o histórico a cada 3s, mas só aplica (re-render)
  // se mudou de verdade. Pausável pelo usuário e desligada ao rever uma salva.
  useEffect(() => {
    if (!auto) return undefined;
    fetchHistory(true);
    const id = setInterval(() => fetchHistory(true), 3000);
    return () => clearInterval(id);
  }, [auto, fetchHistory]);

  // Carrega a lista de salvas ao montar.
  useEffect(() => {
    refreshSavedList();
  }, [refreshSavedList]);

  // Limpa a mensagem de "salva" depois de alguns segundos.
  useEffect(() => {
    if (!saveMsg) return undefined;
    const id = setTimeout(() => setSaveMsg(""), 5000);
    return () => clearTimeout(id);
  }, [saveMsg]);

  // Índices de carros presentes no histórico (ordenados; jogador por último p/
  // desenhar a linha dele POR CIMA das demais).
  const carIdxs = useMemo(() => {
    if (!history) return [];
    const set = new Set();
    for (const snap of history.laps) for (const c of snap.cars) set.add(c.idx);
    const arr = [...set].sort((a, b) => a - b);
    const pIdx = history.player_car_idx;
    return arr.filter((i) => i !== pIdx).concat(arr.includes(pIdx) ? [pIdx] : []);
  }, [history]);

  const colorFor = useCallback(
    (idx) => {
      if (history && idx === history.player_car_idx) return PLAYER_COLOR;
      const pos = carIdxs.indexOf(idx);
      return PALETTE[(pos < 0 ? idx : pos) % PALETTE.length];
    },
    [carIdxs, history],
  );

  // Linhas do race trace: uma linha por volta, com o valor (gap OU posição) de
  // cada carro sob a chave `c{idx}`.
  const traceRows = useMemo(() => {
    if (!history) return [];
    return history.laps.map((snap) => {
      const row = { lap: snap.lap };
      for (const c of snap.cars) {
        row[`c${c.idx}`] = traceMode === "gap" ? c.gap : c.position;
      }
      return row;
    });
  }, [history, traceMode]);

  // Teto do eixo de gap. Carros desqualificados / parados muito tempo no pit têm
  // `CarIdxF2Time` absurdo (sentinela), que estoura a escala e torna o pelotão
  // ilegível. Limitamos o eixo ao ~percentil 90 dos gaps (com margem e um piso),
  // e aparamos as linhas acima disso para o topo.
  const gapCap = useMemo(() => {
    if (traceMode !== "gap") return null;
    const vals = [];
    for (const row of traceRows) {
      for (const k in row) {
        if (k === "lap") continue;
        const v = row[k];
        if (Number.isFinite(v) && v >= 0) vals.push(v);
      }
    }
    if (vals.length === 0) return null;
    vals.sort((a, b) => a - b);
    const p90 = vals[Math.min(vals.length - 1, Math.floor(vals.length * 0.9))];
    return Math.max(Math.ceil((p90 * 1.2) / 5) * 5, 20);
  }, [traceRows, traceMode]);

  // Linhas exibidas: em modo gap, apara os valores acima do teto (ficam rentes
  // ao topo). `_clipped` marca quando isso aconteceu, para avisar na legenda.
  const displayRows = useMemo(() => {
    if (traceMode !== "gap" || gapCap == null) return traceRows;
    return traceRows.map((row) => {
      const out = { lap: row.lap };
      for (const k in row) {
        if (k === "lap") continue;
        out[k] = Math.min(row[k], gapCap);
      }
      return out;
    });
  }, [traceRows, traceMode, gapCap]);

  const clipped = useMemo(() => {
    if (traceMode !== "gap" || gapCap == null) return false;
    return traceRows.some((row) =>
      Object.entries(row).some(([k, v]) => k !== "lap" && v > gapCap),
    );
  }, [traceRows, traceMode, gapCap]);

  const lapDomain = useMemo(() => {
    if (!history || history.laps.length === 0) return [1, 1];
    return [history.laps[0].lap, history.laps[history.laps.length - 1].lap];
  }, [history]);

  // Posição do jogador volta a volta (para os cartões de resumo).
  const playerPositions = useMemo(() => {
    if (!history) return [];
    const out = [];
    for (const snap of history.laps) {
      const me = snap.cars.find((c) => c.idx === history.player_car_idx);
      if (me) out.push(me.position);
    }
    return out;
  }, [history]);

  // Consistência de ritmo: delta à média de cada volta do jogador.
  const pace = useMemo(() => {
    if (!history || history.player_laps.length === 0) return null;
    const times = history.player_laps.map((p) => p.time);
    const avg = times.reduce((a, b) => a + b, 0) / times.length;
    const best = Math.min(...times);
    const rows = history.player_laps.map((p) => ({
      lap: p.lap,
      delta: p.time - avg,
      time: p.time,
      isBest: p.time === best,
    }));
    return { avg, best, rows };
  }, [history]);

  // Batalha do jogador: gap pro carro à frente (acima de 0) e atrás (abaixo),
  // com o jogador na linha 0. Linha encostando em 0 = briga colada.
  const battle = useMemo(() => {
    if (!history || history.player_track.length < 2) return null;
    const t0 = history.player_track[0].session_time;
    const rows = history.player_track.map((p) => ({
      t: Math.max(0, p.session_time - t0),
      lap: p.lap,
      pos: p.position,
      ahead: p.ahead_idx >= 0 ? Math.min(p.gap_ahead, BATTLE_CAP) : null,
      behind: p.behind_idx >= 0 ? -Math.min(p.gap_behind, BATTLE_CAP) : null,
      aheadIdx: p.ahead_idx,
      behindIdx: p.behind_idx,
    }));
    return { rows };
  }, [history]);

  const hasData =
    history &&
    (history.laps.length > 0 ||
      history.player_laps.length > 0 ||
      history.player_track.length > 0);

  return (
    <GlassCard hover={false} className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <span className="text-2xl">🏁</span>
          <div>
            <h2 className="text-lg font-semibold text-text-primary">
              Pós-Corrida — Análise
            </h2>
            <p className="text-xs text-text-secondary">
              Race trace, gap ao líder e consistência de ritmo volta a volta.
            </p>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {viewingSaved ? (
            <GlassButton
              variant="secondary"
              onClick={backToLive}
              className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
            >
              ← Ao vivo
            </GlassButton>
          ) : (
            <>
              <button
                type="button"
                onClick={() => setAuto((a) => !a)}
                className={[
                  "flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[11px] font-semibold transition-glass",
                  auto
                    ? "bg-status-green/15 text-status-green"
                    : "bg-white/5 text-text-secondary hover:text-text-primary",
                ].join(" ")}
                title={auto ? "Atualizando sozinho a cada 3s" : "Atualização automática pausada"}
              >
                <span
                  className={[
                    "inline-block h-1.5 w-1.5 rounded-full",
                    auto ? "animate-pulse bg-status-green" : "bg-text-muted",
                  ].join(" ")}
                />
                {auto ? "Ao vivo" : "Pausado"}
              </button>
              {hasData && (
                <GlassButton
                  variant="secondary"
                  onClick={() => saveRace(false)}
                  className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
                >
                  💾 Salvar
                </GlassButton>
              )}
              <GlassButton
                variant="secondary"
                onClick={load}
                className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
              >
                {loading ? "…" : hasData ? "Atualizar" : "Carregar"}
              </GlassButton>
            </>
          )}
        </div>
      </div>

      {/* Barra de corridas salvas */}
      {(savedList.length > 0 || viewingSaved) && (
        <div className="flex flex-wrap items-center gap-2 text-[11px]">
          <span className="text-text-muted">Salvas:</span>
          <select
            value={viewingSaved || ""}
            onChange={(e) => openSaved(e.target.value)}
            className="rounded-lg border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-text-primary outline-none"
          >
            <option value="">— escolher corrida —</option>
            {savedList.map((r) => (
              <option key={r.name} value={r.name}>
                {r.modified} · {r.laps}v{r.outcome ? ` · ${r.outcome}` : ""}
              </option>
            ))}
          </select>
          {viewingSaved && (
            <span className="rounded-md bg-accent-primary/15 px-2 py-1 font-semibold text-accent-primary">
              revendo salva
            </span>
          )}
        </div>
      )}

      {saveMsg && (
        <p className="rounded-xl border border-status-green/30 bg-status-green/10 px-3 py-2 text-[11px] font-semibold text-status-green">
          {saveMsg}
        </p>
      )}

      {error && (
        <p className="rounded-xl border border-status-red/30 bg-status-red/10 p-3 text-xs font-semibold text-status-red">
          {error}
        </p>
      )}

      {!hasData && !error && (
        <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
          {history
            ? "Sem voltas registradas ainda. O histórico é montado ao vivo durante a corrida — ele aparece aqui sozinho conforme as voltas passam."
            : "Conectando ao histórico da corrida…"}
        </p>
      )}

      {hasData && (
        <>
          {/* Cartões de resumo */}
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <SummaryCard
              label="Largada → chegada"
              value={
                playerPositions.length
                  ? `P${playerPositions[0]} → P${playerPositions[playerPositions.length - 1]}`
                  : "--"
              }
            />
            <SummaryCard
              label="Melhor volta"
              value={pace ? formatLap(pace.best) : "--"}
            />
            <SummaryCard
              label="Voltas (você)"
              value={history.player_laps.length || "--"}
            />
            <SummaryCard
              label="Voltas de amarela"
              value={history.yellow_laps.length}
            />
          </div>

          {/* Abas */}
          <div className="flex gap-2">
            <TabButton active={tab === "trace"} onClick={() => setTab("trace")}>
              Race Trace
            </TabButton>
            <TabButton active={tab === "pace"} onClick={() => setTab("pace")}>
              Consistência
            </TabButton>
            <TabButton active={tab === "battle"} onClick={() => setTab("battle")}>
              Batalha
            </TabButton>
          </div>

          {tab === "trace" && (
            <div className="space-y-3">
              <div className="flex items-center justify-between gap-3">
                <div className="flex gap-1.5">
                  <ModeChip
                    active={traceMode === "gap"}
                    onClick={() => setTraceMode("gap")}
                  >
                    Gap ao líder
                  </ModeChip>
                  <ModeChip
                    active={traceMode === "position"}
                    onClick={() => setTraceMode("position")}
                  >
                    Posição
                  </ModeChip>
                </div>
                <span className="flex items-center gap-1.5 text-[11px] text-text-secondary">
                  <span
                    className="inline-block h-[3px] w-4 rounded-full"
                    style={{ background: PLAYER_COLOR }}
                  />
                  você
                  {history.yellow_laps.length > 0 && (
                    <>
                      <span
                        className="ml-2 inline-block h-3 w-3 rounded-sm"
                        style={{ background: `${YELLOW}55` }}
                      />
                      amarela
                    </>
                  )}
                </span>
              </div>

              {traceRows.length < 2 ? (
                <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
                  Poucas voltas registradas para traçar. Volta a volta aparece
                  conforme o líder completa cada volta.
                </p>
              ) : (
                <div className="h-[320px] w-full">
                  <ResponsiveContainer width="100%" height="100%">
                    <LineChart
                      data={displayRows}
                      margin={{ top: 8, right: 12, bottom: 18, left: 4 }}
                    >
                      <CartesianGrid stroke={GRID} />
                      {/* Faixa amarela nas voltas com bandeira ativa */}
                      {history.yellow_laps.map((lap) => (
                        <ReferenceArea
                          key={`y${lap}`}
                          x1={lap - 0.5}
                          x2={lap + 0.5}
                          fill={YELLOW}
                          fillOpacity={0.16}
                          stroke="none"
                        />
                      ))}
                      <XAxis
                        type="number"
                        dataKey="lap"
                        domain={lapDomain}
                        allowDecimals={false}
                        tick={{ fill: AXIS_TICK, fontSize: 11 }}
                        stroke={GRID}
                        label={{
                          value: "Volta",
                          position: "insideBottom",
                          offset: -8,
                          fill: AXIS_TICK,
                          fontSize: 11,
                        }}
                      />
                      <YAxis
                        reversed
                        allowDecimals={false}
                        domain={
                          traceMode === "gap"
                            ? [0, gapCap ?? "dataMax"]
                            : [1, "dataMax"]
                        }
                        tick={{ fill: AXIS_TICK, fontSize: 11 }}
                        stroke={GRID}
                        width={44}
                        tickFormatter={(v) =>
                          traceMode === "gap" ? `${v}s` : `P${v}`
                        }
                      />
                      <Tooltip content={<TraceTooltip mode={traceMode} playerIdx={history.player_car_idx} />} />
                      {carIdxs.map((idx) => {
                        const isPlayer = idx === history.player_car_idx;
                        return (
                          <Line
                            key={idx}
                            type="monotone"
                            dataKey={`c${idx}`}
                            stroke={colorFor(idx)}
                            strokeWidth={isPlayer ? 3 : 1.3}
                            strokeOpacity={isPlayer ? 1 : 0.65}
                            dot={false}
                            activeDot={isPlayer ? { r: 4 } : false}
                            connectNulls
                            isAnimationActive={false}
                          />
                        );
                      })}
                    </LineChart>
                  </ResponsiveContainer>
                </div>
              )}
              {clipped && (
                <p className="text-[10px] italic text-text-muted">
                  Gaps acima de {gapCap}s foram aparados no topo (carros muito
                  atrás, parados no pit ou desqualificados) para manter o pelotão
                  legível.
                </p>
              )}
            </div>
          )}

          {tab === "pace" && (
            <div className="space-y-3">
              {!pace ? (
                <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
                  Nenhuma volta sua registrada ainda.
                </p>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-x-6 gap-y-1 text-xs text-text-secondary">
                    <span>
                      Média:{" "}
                      <span className="font-semibold text-text-primary">
                        {formatLap(pace.avg)}
                      </span>
                    </span>
                    <span>
                      Melhor:{" "}
                      <span className="font-semibold text-status-green">
                        {formatLap(pace.best)}
                      </span>
                    </span>
                    <span className="flex items-center gap-3">
                      <span className="flex items-center gap-1">
                        <span className="inline-block h-2.5 w-2.5 rounded-sm bg-status-green" />
                        mais rápida
                      </span>
                      <span className="flex items-center gap-1">
                        <span className="inline-block h-2.5 w-2.5 rounded-sm bg-status-red" />
                        mais lenta
                      </span>
                    </span>
                  </div>
                  <div className="h-[280px] w-full">
                    <ResponsiveContainer width="100%" height="100%">
                      <BarChart
                        data={pace.rows}
                        margin={{ top: 8, right: 12, bottom: 18, left: 4 }}
                      >
                        <CartesianGrid stroke={GRID} vertical={false} />
                        <XAxis
                          dataKey="lap"
                          tick={{ fill: AXIS_TICK, fontSize: 11 }}
                          stroke={GRID}
                          label={{
                            value: "Volta",
                            position: "insideBottom",
                            offset: -8,
                            fill: AXIS_TICK,
                            fontSize: 11,
                          }}
                        />
                        <YAxis
                          tick={{ fill: AXIS_TICK, fontSize: 11 }}
                          stroke={GRID}
                          width={44}
                          tickFormatter={(v) => `${v > 0 ? "+" : ""}${v.toFixed(1)}s`}
                        />
                        <Tooltip content={<PaceTooltip />} />
                        <ReferenceLine y={0} stroke={AXIS_TICK} strokeOpacity={0.5} />
                        <Bar dataKey="delta" radius={[3, 3, 0, 0]} isAnimationActive={false}>
                          {pace.rows.map((r) => (
                            <Cell
                              key={r.lap}
                              fill={
                                r.isBest
                                  ? "#22c55e"
                                  : r.delta > 0
                                    ? "#ef4444"
                                    : "#16a34a"
                              }
                            />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  </div>
                </>
              )}
            </div>
          )}

          {tab === "battle" && (
            <div className="space-y-3">
              {!battle ? (
                <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
                  Sem dados de batalha ainda. Eles são amostrados ~1×/segundo
                  enquanto você corre.
                </p>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-x-5 gap-y-1 text-[11px] text-text-secondary">
                    <span className="flex items-center gap-1.5">
                      <span className="inline-block h-[3px] w-4 rounded-full bg-[#f59e0b]" />
                      carro à frente
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="inline-block h-[3px] w-4 rounded-full bg-[#22d3ee]" />
                      carro atrás
                    </span>
                    <span>linha 0 = você · perto de 0 = briga colada</span>
                  </div>
                  <div className="h-[300px] w-full">
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart
                        data={battle.rows}
                        margin={{ top: 8, right: 12, bottom: 18, left: 4 }}
                      >
                        <CartesianGrid stroke={GRID} />
                        <XAxis
                          type="number"
                          dataKey="t"
                          tick={{ fill: AXIS_TICK, fontSize: 11 }}
                          stroke={GRID}
                          tickFormatter={formatClock}
                          label={{
                            value: "Tempo de corrida",
                            position: "insideBottom",
                            offset: -8,
                            fill: AXIS_TICK,
                            fontSize: 11,
                          }}
                        />
                        <YAxis
                          domain={[-BATTLE_CAP, BATTLE_CAP]}
                          tick={{ fill: AXIS_TICK, fontSize: 11 }}
                          stroke={GRID}
                          width={44}
                          tickFormatter={(v) => `${Math.abs(v)}s`}
                        />
                        <Tooltip content={<BattleTooltip />} />
                        <ReferenceLine y={0} stroke={PLAYER_COLOR} strokeOpacity={0.7} />
                        <Line
                          type="monotone"
                          dataKey="ahead"
                          stroke="#f59e0b"
                          strokeWidth={2}
                          dot={false}
                          connectNulls={false}
                          isAnimationActive={false}
                        />
                        <Line
                          type="monotone"
                          dataKey="behind"
                          stroke="#22d3ee"
                          strokeWidth={2}
                          dot={false}
                          connectNulls={false}
                          isAnimationActive={false}
                        />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                  <p className="text-[10px] italic text-text-muted">
                    Gaps acima de {BATTLE_CAP}s ficam achatados no topo/fundo.
                    Trechos sem linha = você liderando (sem ninguém à frente) ou
                    em último.
                  </p>
                </>
              )}
            </div>
          )}
        </>
      )}
    </GlassCard>
  );
}

function SummaryCard({ label, value }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/5 p-3">
      <div className="text-[10px] uppercase tracking-wide text-text-muted">
        {label}
      </div>
      <div className="mt-0.5 text-lg font-semibold text-text-primary">
        {value}
      </div>
    </div>
  );
}

function TabButton({ active, onClick, children }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "rounded-lg px-3 py-1.5 text-xs font-semibold transition-glass",
        active
          ? "bg-accent-primary/20 text-accent-primary"
          : "text-text-secondary hover:text-text-primary",
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
        "rounded-md px-2.5 py-1 text-[11px] font-semibold transition-glass",
        active
          ? "bg-white/15 text-text-primary"
          : "bg-white/5 text-text-secondary hover:text-text-primary",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

function TraceTooltip({ active, payload, label, mode, playerIdx }) {
  if (!active || !payload || payload.length === 0) return null;
  const rows = payload
    .filter((p) => p.value != null)
    .sort((a, b) => a.value - b.value)
    .slice(0, 12);
  return (
    <div className="rounded-lg border border-white/15 bg-app-card/95 px-3 py-2 text-[11px] shadow-lg backdrop-blur">
      <div className="mb-1 font-semibold text-text-primary">Volta {label}</div>
      {rows.map((p) => {
        const idx = Number(p.dataKey.slice(1));
        const isPlayer = idx === playerIdx;
        return (
          <div key={p.dataKey} className="flex items-center justify-between gap-3">
            <span className="flex items-center gap-1.5" style={{ color: p.color }}>
              <span
                className="inline-block h-2 w-2 rounded-full"
                style={{ background: p.color }}
              />
              {isPlayer ? "Você" : `Carro ${idx}`}
            </span>
            <span className="tabular-nums text-text-secondary">
              {mode === "gap" ? `+${p.value.toFixed(2)}s` : `P${p.value}`}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function PaceTooltip({ active, payload }) {
  if (!active || !payload || payload.length === 0) return null;
  const r = payload[0].payload;
  return (
    <div className="rounded-lg border border-white/15 bg-app-card/95 px-3 py-2 text-[11px] shadow-lg backdrop-blur">
      <div className="font-semibold text-text-primary">Volta {r.lap}</div>
      <div className="text-text-secondary">{formatLap(r.time)}</div>
      <div className={r.delta > 0 ? "text-status-red" : "text-status-green"}>
        {formatDelta(r.delta)} à média
      </div>
    </div>
  );
}

function BattleTooltip({ active, payload }) {
  if (!active || !payload || payload.length === 0) return null;
  const r = payload[0].payload;
  return (
    <div className="rounded-lg border border-white/15 bg-app-card/95 px-3 py-2 text-[11px] shadow-lg backdrop-blur">
      <div className="font-semibold text-text-primary">
        {formatClock(r.t)} · volta {r.lap} · P{r.pos}
      </div>
      <div className="text-[#f59e0b]">
        {r.aheadIdx >= 0 ? `à frente: +${r.ahead.toFixed(2)}s (Carro ${r.aheadIdx})` : "liderando"}
      </div>
      <div className="text-[#22d3ee]">
        {r.behindIdx >= 0
          ? `atrás: ${Math.abs(r.behind).toFixed(2)}s (Carro ${r.behindIdx})`
          : "sem ninguém atrás"}
      </div>
    </div>
  );
}

export default PostRacePanel;
