import { useState, useMemo, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import RaceTraceChart from "../race/RaceTraceChart";
import PaceDeltaChart from "../race/PaceDeltaChart";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  // Apelidado porque o `Tooltip` do app também entra neste arquivo, logo abaixo. Os dois
  // importados com o mesmo nome é erro de módulo ES; o bundler engolia, o segundo import
  // vencia, e o `<Tooltip content={<BattleTooltip />} />` do gráfico de batalha virava o
  // tooltip do app recebendo uma prop que ele não conhece — o balão do gráfico não abria e
  // nada acusava. Corrigido em 11/08/2026, quando o teste do painel não conseguiu nem
  // compilar o arquivo.
  Tooltip as ChartTooltip,
  ReferenceLine,
  ResponsiveContainer,
} from "recharts";
import GlassCard from "../ui/GlassCard";
import GlassButton from "../ui/GlassButton";
import Tooltip from "../ui/Tooltip";
import { AXIS_TICK, GRID, PALETTE, PLAYER_COLOR, YELLOW } from "../../utils/chartTheme";
import { formatLapSeconds } from "../../utils/formatters";

/** Segundos decorridos → "m:ss" (relógio da corrida). */
function formatClock(seconds) {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

const BATTLE_CAP = 30; // teto (s) do gráfico de batalha — briga é coisa de perto.

function PostRacePanel() {
  const { t } = useTranslation();
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
        if (!silent) setSaveMsg(t("postRacePanel.saved", { name }));
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
          if (name) setSaveMsg(t("postRacePanel.autoSaved", { name }));
        }
        // Dificuldade adaptativa: NÃO é daqui. O ajuste roda no import automático
        // (iracing_auto_import_if_ready), amarrado à corrida ENTRAR na carreira —
        // este painel é ferramenta de análise e processar aqui duplicaria o ajuste.
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
      // X FRACIONÁRIO: `lap` são as voltas COMPLETAS do líder e `progress` o quanto ele
      // já andou da volta em curso. Dentro da mesma volta o backend grava um ponto por
      // troca de posição — sem a parte fracionária eles empilham no mesmo X, e o trace
      // perde a volta inteira de dados e desenha um degrau vertical na virada.
      const row = { lap: snap.lap + (Number.isFinite(snap.progress) ? snap.progress : 0) };
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
    if (!traceRows.length) return [1, 1];
    return [traceRows[0].lap, traceRows[traceRows.length - 1].lap];
  }, [traceRows]);

  // A amarela é gravada com as voltas COMPLETAS do líder no instante — a volta que
  // estava em curso ocupa [L, L+1] no eixo fracionário. O gráfico pinta a faixa
  // centrada (±0,5 volta), então o centro é L + 0,5.
  const yellowBands = useMemo(
    () => (history?.yellow_laps ?? []).map((l) => l + 0.5),
    [history],
  );

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
              {t("postRacePanel.title")}
            </h2>
            <p className="text-xs text-text-secondary">
              {t("postRacePanel.subtitle")}
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
              {t("postRacePanel.backToLive")}
            </GlassButton>
          ) : (
            <>
              <Tooltip texto={auto ? t("postRacePanel.autoOnTitle") : t("postRacePanel.autoOffTitle")}>
                <button
                  type="button"
                  onClick={() => setAuto((a) => !a)}
                  className={[
                    "flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[11px] font-semibold transition-glass",
                    auto
                      ? "bg-status-green/15 text-status-green"
                      : "bg-white/5 text-text-secondary hover:text-text-primary",
                  ].join(" ")}
                >
                  <span
                    className={[
                      "inline-block h-1.5 w-1.5 rounded-full",
                      auto ? "animate-pulse bg-status-green" : "bg-text-muted",
                    ].join(" ")}
                  />
                  {auto ? t("postRacePanel.live") : t("postRacePanel.paused")}
                </button>
              </Tooltip>
              {hasData && (
                <GlassButton
                  variant="secondary"
                  onClick={() => saveRace(false)}
                  className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
                >
                  {t("postRacePanel.save")}
                </GlassButton>
              )}
              <GlassButton
                variant="secondary"
                onClick={load}
                className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
              >
                {loading ? "…" : hasData ? t("postRacePanel.refresh") : t("postRacePanel.loadBtn")}
              </GlassButton>
            </>
          )}
        </div>
      </div>

      {/* Barra de corridas salvas */}
      {(savedList.length > 0 || viewingSaved) && (
        <div className="flex flex-wrap items-center gap-2 text-[11px]">
          <span className="text-text-muted">{t("postRacePanel.savedLabel")}</span>
          <select
            value={viewingSaved || ""}
            onChange={(e) => openSaved(e.target.value)}
            className="rounded-lg border border-white/10 bg-white/5 px-2 py-1 text-[11px] text-text-primary outline-none"
          >
            <option value="">{t("postRacePanel.chooseRace")}</option>
            {savedList.map((r) => (
              <option key={r.name} value={r.name}>
                {r.modified} · {r.laps}{t("postRacePanel.lapsSuffix")}{r.outcome ? ` · ${r.outcome}` : ""}
              </option>
            ))}
          </select>
          {viewingSaved && (
            <span className="rounded-md bg-accent-primary/15 px-2 py-1 font-semibold text-accent-primary">
              {t("postRacePanel.reviewingSaved")}
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
            ? t("postRacePanel.noLapsYet")
            : t("postRacePanel.connecting")}
        </p>
      )}

      {hasData && (
        <>
          {/* Cartões de resumo */}
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <SummaryCard
              label={t("postRacePanel.cardStartFinish")}
              value={
                playerPositions.length
                  ? `P${playerPositions[0]} → P${playerPositions[playerPositions.length - 1]}`
                  : "--"
              }
            />
            <SummaryCard
              label={t("postRacePanel.cardBestLap")}
              value={pace ? formatLapSeconds(pace.best) : "--"}
            />
            <SummaryCard
              label={t("postRacePanel.cardYourLaps")}
              value={history.player_laps.length || "--"}
            />
            <SummaryCard
              label={t("postRacePanel.cardYellowLaps")}
              value={history.yellow_laps.length}
            />
          </div>

          {/* Abas */}
          <div className="flex gap-2">
            <TabButton active={tab === "trace"} onClick={() => setTab("trace")}>
              {t("postRacePanel.tabTrace")}
            </TabButton>
            <TabButton active={tab === "pace"} onClick={() => setTab("pace")}>
              {t("postRacePanel.tabConsistency")}
            </TabButton>
            <TabButton active={tab === "battle"} onClick={() => setTab("battle")}>
              {t("postRacePanel.tabBattle")}
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
                    {t("postRacePanel.modeGap")}
                  </ModeChip>
                  <ModeChip
                    active={traceMode === "position"}
                    onClick={() => setTraceMode("position")}
                  >
                    {t("postRacePanel.modePosition")}
                  </ModeChip>
                </div>
                <span className="flex items-center gap-1.5 text-[11px] text-text-secondary">
                  <span
                    className="inline-block h-[3px] w-4 rounded-full"
                    style={{ background: PLAYER_COLOR }}
                  />
                  {t("postRacePanel.you")}
                  {history.yellow_laps.length > 0 && (
                    <>
                      <span
                        className="ml-2 inline-block h-3 w-3 rounded-sm"
                        style={{ background: `${YELLOW}55` }}
                      />
                      {t("postRacePanel.yellow")}
                    </>
                  )}
                </span>
              </div>

              {traceRows.length < 2 ? (
                <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
                  {t("postRacePanel.fewLaps")}
                </p>
              ) : (
                <div className="h-[320px] w-full">
                  <RaceTraceChart
                    rows={displayRows}
                    cars={carIdxs.map((idx) => ({ idx, isPlayer: idx === history.player_car_idx }))}
                    colorForCar={colorFor}
                    nameByIdx={{ [history.player_car_idx]: t("postRacePanel.youName") }}
                    mode={traceMode}
                    lapDomain={lapDomain}
                    gapCap={gapCap}
                    yellowLaps={yellowBands}
                  />
                </div>
              )}
              {clipped && (
                <p className="text-[10px] italic text-text-muted">
                  {t("postRacePanel.clippedNote", { cap: gapCap })}
                </p>
              )}
            </div>
          )}

          {tab === "pace" && (
            <div className="space-y-3">
              {!pace ? (
                <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
                  {t("postRacePanel.noYourLaps")}
                </p>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-x-6 gap-y-1 text-xs text-text-secondary">
                    <span>
                      {t("postRacePanel.average")}{" "}
                      <span className="font-semibold text-text-primary">
                        {formatLapSeconds(pace.avg)}
                      </span>
                    </span>
                    <span>
                      {t("postRacePanel.best")}{" "}
                      <span className="font-semibold text-status-green">
                        {formatLapSeconds(pace.best)}
                      </span>
                    </span>
                    <span className="flex items-center gap-3">
                      <span className="flex items-center gap-1">
                        <span className="inline-block h-2.5 w-2.5 rounded-sm bg-status-green" />
                        {t("postRacePanel.faster")}
                      </span>
                      <span className="flex items-center gap-1">
                        <span className="inline-block h-2.5 w-2.5 rounded-sm bg-status-red" />
                        {t("postRacePanel.slower")}
                      </span>
                    </span>
                  </div>
                  <div className="h-[280px] w-full">
                    <PaceDeltaChart rows={pace.rows} />
                  </div>
                </>
              )}
            </div>
          )}

          {tab === "battle" && (
            <div className="space-y-3">
              {!battle ? (
                <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
                  {t("postRacePanel.noBattleData")}
                </p>
              ) : (
                <>
                  <div className="flex flex-wrap items-center gap-x-5 gap-y-1 text-[11px] text-text-secondary">
                    <span className="flex items-center gap-1.5">
                      <span className="inline-block h-[3px] w-4 rounded-full bg-[#f59e0b]" />
                      {t("postRacePanel.carAhead")}
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="inline-block h-[3px] w-4 rounded-full bg-[#22d3ee]" />
                      {t("postRacePanel.carBehind")}
                    </span>
                    <span>{t("postRacePanel.battleLegend")}</span>
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
                            value: t("postRacePanel.raceTime"),
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
                        <ChartTooltip content={<BattleTooltip />} />
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
                    {t("postRacePanel.battleNote", { cap: BATTLE_CAP })}
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

function BattleTooltip({ active, payload }) {
  const { t } = useTranslation();
  if (!active || !payload || payload.length === 0) return null;
  const r = payload[0].payload;
  return (
    <div className="rounded-lg border border-white/15 bg-app-card/95 px-3 py-2 text-[11px] shadow-lg backdrop-blur">
      <div className="font-semibold text-text-primary">
        {t("postRacePanel.tooltipLine", {
          clock: formatClock(r.t),
          lap: r.lap,
          pos: r.pos,
        })}
      </div>
      <div className="text-[#f59e0b]">
        {r.aheadIdx >= 0
          ? t("postRacePanel.tooltipAhead", {
              gap: r.ahead.toFixed(2),
              car: r.aheadIdx,
            })
          : t("postRacePanel.leading")}
      </div>
      <div className="text-[#22d3ee]">
        {r.behindIdx >= 0
          ? t("postRacePanel.tooltipBehind", {
              gap: Math.abs(r.behind).toFixed(2),
              car: r.behindIdx,
            })
          : t("postRacePanel.noOneBehind")}
      </div>
    </div>
  );
}

export default PostRacePanel;
