import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

// Explicador de debug: "por que essa rivalidade?". Roda a percepção de rivalidade
// (iracing_perceive_rivalries) sobre uma corrida — ao vivo ou salva — para um
// CARRO-SONDA à escolha (o jogador OU uma IA), e mostra o livro-razão por oponente.
// Serve pra calibrar os limiares antes de qualquer efeito no jogo: grava-se uma
// corrida de IA de ~20 min, salva, e confere-se aqui se o sistema percebeu a
// corrida "como você sentiu". Nenhum delta é aplicado no motor de rivalidade.

const KIND_META = {
  contato: { tone: "text-status-red border-status-red/30 bg-status-red/10" },
  duelo: { tone: "text-accent-primary border-accent-primary/30 bg-accent-primary/10" },
  pendulo: { tone: "text-status-yellow border-status-yellow/30 bg-status-yellow/10" },
  ultrapassagem: { tone: "text-status-green border-status-green/30 bg-status-green/10" },
};

function fmtGap(v) {
  return v == null ? "—" : `${v.toFixed(2)}s`;
}

function RivalryPerceptionPanel() {
  const { t } = useTranslation();
  const [savedList, setSavedList] = useState([]);
  const [source, setSource] = useState("live"); // "live" | nome do arquivo
  const [history, setHistory] = useState(null); // RaceHistory carregado (p/ lista de carros)
  const [probe, setProbe] = useState(null); // car_idx da sonda
  const [result, setResult] = useState(null); // RivalryPerception
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [saveMsg, setSaveMsg] = useState("");

  const refreshSaved = useCallback(
    () => invoke("iracing_list_saved_races").then(setSavedList).catch(() => {}),
    [],
  );

  useEffect(() => {
    refreshSaved();
  }, [refreshSaved]);

  // Persiste a corrida ao vivo em disco (o trace de campo fica gravado para análise
  // depois — sem isso, a análise "ao vivo" some quando o app fecha).
  const saveLive = useCallback(async () => {
    setSaveMsg("");
    try {
      const name = await invoke("iracing_save_race_history");
      await refreshSaved();
      setSource(name);
      setSaveMsg(`Salva: ${name}`);
    } catch (e) {
      setSaveMsg(String(e));
    }
  }, [refreshSaved]);

  // Carrega o RaceHistory da fonte escolhida só para montar a lista de carros
  // (a análise em si vai no comando de percepção, que recarrega por conta própria).
  const loadHistory = useCallback(async (src) => {
    setError("");
    setResult(null);
    setHistory(null);
    setProbe(null);
    try {
      const h =
        src === "live"
          ? await invoke("iracing_get_race_history")
          : await invoke("iracing_load_saved_race", { name: src });
      setHistory(h);
      setProbe(h?.player_car_idx ?? null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    loadHistory(source);
  }, [source, loadHistory]);

  const analyze = useCallback(async () => {
    if (probe == null) return;
    setLoading(true);
    setError("");
    try {
      const r = await invoke("iracing_perceive_rivalries", {
        savedName: source === "live" ? null : source,
        probeCarIdx: probe,
      });
      setResult(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [source, probe]);

  // Carros disponíveis como sonda (fora o pace car), ordenados por número.
  const cars = (history?.cars_meta ?? [])
    .filter((c) => !c.is_pace)
    .slice()
    .sort((a, b) => a.car_number - b.car_number);

  const hasHistory = history && (history.laps?.length ?? 0) > 0;

  return (
    <div className="border-t border-white/10 px-5 py-3.5">
      <p className="text-[13px] font-medium text-text-primary">{t("rivalryPerception.title")}</p>
      <p className="pb-3 text-[11px] text-text-secondary">
        {t("rivalryPerception.descPre")}<em>{t("rivalryPerception.descEm")}</em>{t("rivalryPerception.descPost")}
      </p>

      {/* Fonte + sonda + botão */}
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-[9px] font-bold uppercase tracking-[0.14em] text-text-muted">{t("rivalryPerception.raceLabel")}</span>
          <select
            value={source}
            onChange={(e) => setSource(e.target.value)}
            className="min-w-[180px] rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-[12px] text-text-primary outline-none transition-glass focus:border-white/25"
          >
            <option value="live">{t("rivalryPerception.liveOption")}</option>
            {savedList.map((s) => (
              <option key={s.name} value={s.name}>
                {s.modified ? `${s.modified} · ` : ""}
                {s.outcome || s.name} {t("rivalryPerception.savedLaps", { laps: s.laps })}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1">
          <span className="text-[9px] font-bold uppercase tracking-[0.14em] text-text-muted">{t("rivalryPerception.probeLabel")}</span>
          <select
            value={probe ?? ""}
            onChange={(e) => setProbe(Number(e.target.value))}
            disabled={!hasHistory}
            className="min-w-[160px] rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-[12px] text-text-primary outline-none transition-glass focus:border-white/25 disabled:opacity-50"
          >
            {cars.map((c) => (
              <option key={c.idx} value={c.idx}>
                #{c.car_number}
                {c.idx === history.player_car_idx
                  ? ` (${t("rivalryPerception.you")})`
                  : c.is_ai
                  ? ` · ${t("rivalryPerception.ai")}`
                  : ""}
              </option>
            ))}
          </select>
        </label>

        <button
          type="button"
          onClick={analyze}
          disabled={loading || probe == null || !hasHistory}
          className={`shrink-0 rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
            loading || probe == null || !hasHistory
              ? "cursor-default bg-white/5 text-text-muted"
              : "cursor-pointer bg-accent-primary/20 text-text-primary hover:bg-accent-primary/30"
          }`}
        >
          {loading ? t("rivalryPerception.analyzing") : t("rivalryPerception.analyze")}
        </button>
      </div>

      {!hasHistory && !error && (
        <p className="mt-3 rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-[11px] text-text-secondary">
          {source === "live"
            ? t("rivalryPerception.emptyLive")
            : t("rivalryPerception.emptySaved")}
        </p>
      )}

      {error && (
        <p className="mt-3 rounded-lg border border-status-red/30 bg-status-red/10 px-3 py-2 font-mono text-[11px] text-text-primary">
          {error}
        </p>
      )}

      {result && <PerceptionResults result={result} />}
    </div>
  );
}

function PerceptionResults({ result }) {
  const { t } = useTranslation();
  return (
    <div className="mt-3 space-y-2">
      <div className="flex items-center gap-2 text-[11px] text-text-secondary">
        <span className="rounded-full border border-white/10 bg-white/5 px-2 py-0.5 font-mono">
          {t("rivalryPerception.probeIdx", { idx: result.probe_car_idx })}
        </span>
        <span>
          {result.is_player_probe ? t("rivalryPerception.you") : t("rivalryPerception.ai")} ·{" "}
          {t("rivalryPerception.lapsCount", { laps: result.total_laps })} ·{" "}
          {t("rivalryPerception.rivalsCount", { n: result.opponents.length })}
        </span>
      </div>

      {result.opponents.length === 0 ? (
        <p className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-[11px] text-text-secondary">
          {t("rivalryPerception.noRivalries")}
        </p>
      ) : (
        result.opponents.map((o) => <OpponentCard key={o.car_idx} o={o} />)
      )}
    </div>
  );
}

function OpponentCard({ o }) {
  const { t } = useTranslation();
  return (
    <div className="glass rounded-xl p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-baseline gap-2">
          <span className="text-[14px] font-bold text-text-primary">#{o.car_number || "?"}</span>
          <span className="font-mono text-[9px] text-text-muted">idx {o.car_idx}</span>
        </div>
        <div className="flex items-center gap-2">
          {o.capped && (
            <span className="rounded-full border border-status-yellow/30 bg-status-yellow/10 px-2 py-0.5 text-[9px] font-bold uppercase tracking-[0.1em] text-status-yellow">
              cap
            </span>
          )}
          <span className="text-[11px] text-text-secondary">
            {t("rivalryPerception.intensity")}{" "}
            <span className="text-[13px] font-bold text-text-primary">
              {Math.round(o.projected_perceived)}
            </span>
          </span>
        </div>
      </div>

      {/* Barra da intensidade projetada */}
      <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-white/10">
        <div
          className="h-full rounded-full bg-accent-primary/70"
          style={{ width: `${Math.min(100, o.projected_perceived)}%` }}
        />
      </div>

      {/* Métricas cruas */}
      <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[10px] text-text-secondary">
        <span>{t("rivalryPerception.duel")} <b className="text-text-primary">{t("rivalryPerception.lapsShort", { laps: o.duel_laps })}</b></span>
        <span>{t("rivalryPerception.smallestGap")} <b className="text-text-primary">{fmtGap(o.closest_gap_secs)}</b></span>
        <span>{t("rivalryPerception.swaps")} <b className="text-text-primary">{o.swaps}</b></span>
        <span>{t("rivalryPerception.passed")} <b className="text-text-primary">{o.overtakes_for}</b>/<b className="text-text-primary">{o.overtakes_against}</b></span>
        <span>{t("rivalryPerception.late")} <b className="text-text-primary">{o.late_overtakes}</b></span>
        <span>{t("rivalryPerception.fight")} <b className="text-text-primary">P{o.best_fight_position || "?"}</b></span>
        <span>{t("rivalryPerception.relev")} <b className="text-text-primary">×{o.relevance_mult.toFixed(1)}</b></span>
      </div>

      {/* Sinais (o "por quê") */}
      <div className="mt-2 space-y-1">
        {o.hits.map((h, i) => {
          const meta = KIND_META[h.kind];
          const tone = meta?.tone ?? "text-text-secondary border-white/10 bg-white/5";
          const label = meta ? t(`rivalryPerception.kinds.${h.kind}`) : h.kind;
          return (
            <div key={i} className="flex items-center gap-2 text-[11px]">
              <span className={`shrink-0 rounded border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.08em] ${tone}`}>
                {label}
              </span>
              <span className="min-w-0 flex-1 truncate text-text-secondary">{h.detail}</span>
              <span className="shrink-0 font-mono text-[10px] text-text-muted">
                +{h.recent_delta.toFixed(1)}r / +{h.historical_delta.toFixed(1)}h
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default RivalryPerceptionPanel;
