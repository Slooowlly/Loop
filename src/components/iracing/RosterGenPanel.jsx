import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import GlassCard from "../ui/GlassCard";
import GlassButton from "../ui/GlassButton";
import Tooltip from "../ui/Tooltip";

const CARS = [
  { key: "mx5", label: "Mazda MX-5" },
  { key: "gr86", label: "Toyota GR86" },
  { key: "bmwm2", label: "BMW M2 (G87)" },
];

// Tempo de volta (ms) → "m:ss.mmm". "—" se não houver volta válida.
function fmtLap(ms) {
  if (!ms || ms <= 0) return "—";
  const total = ms / 1000;
  const min = Math.floor(total / 60);
  const sec = (total - min * 60).toFixed(3).padStart(6, "0");
  return `${min}:${sec}`;
}

// Gap ao líder (ms) → "+s.sss". "—" se zero/ausente.
function fmtGap(ms) {
  if (!ms || ms <= 0) return "—";
  return `+${(ms / 1000).toFixed(3)}`;
}

function RosterGenPanel() {
  const { t } = useTranslation();
  const [saves, setSaves] = useState([]);
  const [careerId, setCareerId] = useState("");
  const [categoria, setCategoria] = useState("");
  const [carKey, setCarKey] = useState("mx5");
  const [rosterName, setRosterName] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState(null);
  const [error, setError] = useState("");
  const [seasonBusy, setSeasonBusy] = useState(false);
  const [seasonResult, setSeasonResult] = useState(null);
  const [targetTrackId, setTargetTrackId] = useState("");
  const [paint, setPaint] = useState(null);
  const [paintError, setPaintError] = useState("");
  const [custid, setCustid] = useState(null);
  const [custidMsg, setCustidMsg] = useState("");
  const [connected, setConnected] = useState(false);
  const [applyBusy, setApplyBusy] = useState(false);
  const [applyResult, setApplyResult] = useState(null);
  const [applyError, setApplyError] = useState("");
  const [rainBusy, setRainBusy] = useState(false);
  const [rainResult, setRainResult] = useState(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [preview, setPreview] = useState(null);
  const [dumpMsg, setDumpMsg] = useState("");

  // Dumpa o YAML da sessão do iRacing num arquivo (para inspeção do ResultsPositions).
  async function dumpSessionYaml() {
    setDumpMsg(t("rosterGen.readingSession"));
    try {
      const path = await invoke("iracing_dump_session_yaml");
      setDumpMsg(t("rosterGen.yamlWritten", { path }));
    } catch (e) {
      setDumpMsg(t("rosterGen.genericError", { error: String(e) }));
    }
  }

  // PREVIEW (read-only) da ponte sessão→RaceResult: reconstrói o resultado da
  // última corrida do iRacing como a carreira o veria — sem gravar nada.
  async function previewResult() {
    setPreviewBusy(true);
    setError("");
    setPreview(null);
    try {
      const res = await invoke("iracing_preview_race_result", { careerId });
      setPreview(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setPreviewBusy(false);
    }
  }

  // TESTE DE CHUVA: 3 seasons (Seco/Molhado/Tempestade) com 16 pilotos controlados,
  // driver_skill já re-rankeado pela penalidade de chuva por piloto.
  async function exportRainTest() {
    setRainBusy(true);
    setError("");
    setRainResult(null);
    try {
      const res = await invoke("iracing_export_rain_test");
      setRainResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setRainBusy(false);
    }
  }

  async function applyPaint() {
    setApplyBusy(true);
    setApplyError("");
    setApplyResult(null);
    try {
      const res = await invoke("iracing_apply_player_paint", {
        careerId,
        carKey,
      });
      setApplyResult(res);
    } catch (e) {
      setApplyError(String(e));
    } finally {
      setApplyBusy(false);
    }
  }

  async function detectCustid() {
    setCustidMsg(t("rosterGen.readingIracingSession"));
    try {
      const id = await invoke("iracing_player_custid");
      setCustid(id);
      setCustidMsg("");
    } catch (e) {
      setCustidMsg(String(e));
    }
  }

  // Vigia a conexão com o iRacing sozinho e, ao conectar, busca o custid.
  useEffect(() => {
    let alive = true;
    async function tick() {
      try {
        const on = await invoke("iracing_connected");
        if (!alive) return;
        setConnected(on);
        if (on) {
          invoke("iracing_player_custid")
            .then((id) => alive && setCustid(id))
            .catch(() => {});
        }
      } catch {
        /* ignore */
      }
    }
    tick();
    const handle = setInterval(tick, 4000);
    return () => {
      alive = false;
      clearInterval(handle);
    };
  }, []);

  useEffect(() => {
    invoke("list_saves")
      .then((list) => {
        setSaves(list || []);
        if (list && list.length > 0) setCareerId(list[0].career_id);
      })
      .catch((e) => setError(String(e)));
  }, []);

  const selected = useMemo(
    () => saves.find((s) => s.career_id === careerId) || null,
    [saves, careerId],
  );

  // Pré-preenche categoria e nome do roster ao trocar de save.
  useEffect(() => {
    if (!selected) return;
    setCategoria(selected.category || "");
    setRosterName(
      (prev) =>
        prev ||
        t("rosterGen.defaultRosterName", { name: selected.player_name || "" }).trim(),
    );
  }, [selected]);

  // Busca a pintura (cor do time) do jogador ao trocar de save.
  useEffect(() => {
    if (!careerId) {
      setPaint(null);
      return;
    }
    setPaint(null);
    setPaintError("");
    invoke("iracing_player_paint", { careerId })
      .then(setPaint)
      .catch((e) => setPaintError(String(e)));
  }, [careerId]);

  async function generate() {
    setBusy(true);
    setError("");
    setResult(null);
    try {
      const res = await invoke("iracing_generate_roster", {
        careerId,
        categoria,
        rosterName,
        carKey,
      });
      setResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function generateSeason() {
    setSeasonBusy(true);
    setError("");
    setSeasonResult(null);
    try {
      const res = await invoke("iracing_generate_season", {
        careerId,
        categoria,
        rosterName,
        carKey,
        targetTrackId: targetTrackId.trim() ? Number(targetTrackId.trim()) : null,
      });
      setSeasonResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setSeasonBusy(false);
    }
  }

  // AI Season de TESTE: zerada (sem resultados), com a corrida 1 usando o clima
  // roteirizado da 1ª corrida — pra visualizar o roteiro no menu do iRacing.
  async function generateTestSeason() {
    setSeasonBusy(true);
    setError("");
    setSeasonResult(null);
    try {
      const res = await invoke("iracing_generate_season", {
        careerId,
        categoria,
        rosterName,
        carKey,
        targetTrackId: null,
        testBlank: true,
      });
      setSeasonResult(res);
    } catch (e) {
      setError(String(e));
    } finally {
      setSeasonBusy(false);
    }
  }

  const canGenerate = careerId && categoria.trim() && rosterName.trim() && !busy;

  return (
    <>
      <GlassCard hover={false} className="space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <span className="text-2xl">🧩</span>
          <div>
            <h2 className="text-lg font-semibold text-text-primary">
              {t("rosterGen.title")}
            </h2>
            <p className="text-xs text-text-secondary">
              {t("rosterGen.subtitle")}
            </p>
          </div>
        </div>
        <Tooltip texto={t("rosterGen.connectionTitle")}>
          <span
            className={[
              "flex shrink-0 items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[11px] font-semibold",
              connected
                ? "bg-status-green/15 text-status-green"
                : "bg-white/5 text-text-muted",
            ].join(" ")}
          >
            <span
              className={[
                "inline-block h-1.5 w-1.5 rounded-full",
                connected ? "animate-pulse bg-status-green" : "bg-text-muted",
              ].join(" ")}
            />
            {connected ? t("rosterGen.connected") : t("rosterGen.disconnected")}
          </span>
        </Tooltip>
      </div>

      {saves.length === 0 ? (
        <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
          {t("rosterGen.noSaves")}
        </p>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label={t("rosterGen.fieldCareer")}>
            <select
              value={careerId}
              onChange={(e) => setCareerId(e.target.value)}
              className="w-full rounded-lg border border-white/10 bg-white/5 px-2 py-1.5 text-xs text-text-primary outline-none"
            >
              {saves.map((s) => (
                <option key={s.career_id} value={s.career_id}>
                  {s.player_name} · {s.category_name} · T{s.season}
                </option>
              ))}
            </select>
          </Field>

          <Field label={t("rosterGen.fieldCategory")}>
            <input
              value={categoria}
              onChange={(e) => setCategoria(e.target.value)}
              placeholder={t("rosterGen.categoryPlaceholder")}
              className="w-full rounded-lg border border-white/10 bg-white/5 px-2 py-1.5 text-xs text-text-primary outline-none"
            />
          </Field>

          <Field label={t("rosterGen.fieldCar")}>
            <select
              value={carKey}
              onChange={(e) => setCarKey(e.target.value)}
              className="w-full rounded-lg border border-white/10 bg-white/5 px-2 py-1.5 text-xs text-text-primary outline-none"
            >
              {CARS.map((c) => (
                <option key={c.key} value={c.key}>
                  {c.label}
                </option>
              ))}
            </select>
          </Field>

          <Field label={t("rosterGen.fieldRosterName")}>
            <input
              value={rosterName}
              onChange={(e) => setRosterName(e.target.value)}
              placeholder={t("rosterGen.rosterNamePlaceholder")}
              className="w-full rounded-lg border border-white/10 bg-white/5 px-2 py-1.5 text-xs text-text-primary outline-none"
            />
          </Field>
        </div>
      )}

      <div className="flex items-center gap-3">
        <GlassButton
          variant="primary"
          onClick={generate}
          disabled={!canGenerate}
          className="!min-h-0 !rounded-lg !px-4 !py-2 text-xs"
        >
          {busy ? t("rosterGen.generating") : t("rosterGen.generateRoster")}
        </GlassButton>
        <GlassButton
          variant="secondary"
          onClick={generateSeason}
          disabled={!canGenerate || seasonBusy}
          className="!min-h-0 !rounded-lg !px-4 !py-2 text-xs"
        >
          {seasonBusy ? t("rosterGen.generating") : t("rosterGen.generateSeason")}
        </GlassButton>
        <GlassButton
          variant="secondary"
          onClick={generateTestSeason}
          disabled={!canGenerate || seasonBusy}
          title={t("rosterGen.testSeasonTitle")}
          className="!min-h-0 !rounded-lg !px-4 !py-2 text-xs"
        >
          {t("rosterGen.testSeasonBtn")}
        </GlassButton>
        <GlassButton
          variant="secondary"
          onClick={exportRainTest}
          disabled={rainBusy}
          title={t("rosterGen.rainTestTitle")}
          className="!min-h-0 !rounded-lg !px-4 !py-2 text-xs"
        >
          {rainBusy ? t("rosterGen.generating") : t("rosterGen.rainTestBtn")}
        </GlassButton>
        <GlassButton
          variant="secondary"
          onClick={previewResult}
          disabled={!careerId || previewBusy}
          title={t("rosterGen.previewTitleAttr")}
          className="!min-h-0 !rounded-lg !px-4 !py-2 text-xs"
        >
          {previewBusy ? t("rosterGen.reading") : t("rosterGen.previewBtn")}
        </GlassButton>
        <GlassButton
          variant="secondary"
          onClick={dumpSessionYaml}
          title={t("rosterGen.dumpYamlTitle")}
          className="!min-h-0 !rounded-lg !px-4 !py-2 text-xs"
        >
          {t("rosterGen.dumpYamlBtn")}
        </GlassButton>
        <Tooltip texto={t("rosterGen.trackOverrideTitle")}>
          <input
            value={targetTrackId}
            onChange={(e) => setTargetTrackId(e.target.value.replace(/[^0-9]/g, ""))}
            placeholder={t("rosterGen.trackOverridePlaceholder")}
            className="w-40 rounded-lg border border-white/10 bg-white/5 px-2 py-1.5 text-xs text-text-primary outline-none"
          />
        </Tooltip>
        {selected && (
          <span className="text-[10px] text-text-muted">
            {t("rosterGen.defaultCategory")} <span className="font-mono">{selected.category}</span>
          </span>
        )}
      </div>

      {dumpMsg && (
        <p className="break-all rounded-xl border border-white/10 bg-white/5 px-3 py-2 text-[10px] font-mono text-text-secondary">
          {dumpMsg}
        </p>
      )}
      {error && (
        <p className="rounded-xl border border-status-red/30 bg-status-red/10 px-3 py-2 text-[11px] font-semibold text-status-red">
          {error}
        </p>
      )}
      {result && (
        <div className="rounded-xl border border-status-green/30 bg-status-green/10 px-3 py-2 text-[11px] text-status-green">
          <p className="font-semibold">
            {t("rosterGen.rosterGenerated", { drivers: result.drivers })}
          </p>
          <p className="mt-0.5 break-all font-mono text-[10px] text-text-secondary">
            {result.path}
          </p>
        </div>
      )}
      {seasonResult && (
        <div className="rounded-xl border border-status-green/30 bg-status-green/10 px-3 py-2 text-[11px] text-status-green">
          <p className="font-semibold">
            {t("rosterGen.seasonGenerated", {
              name: seasonResult.name,
              events: seasonResult.events,
            })}
          </p>
          {seasonResult.targeted_track && (
            <p className="mt-0.5">
              {seasonResult.auto_targeted ? t("rosterGen.nextRace") : t("rosterGen.override")}:{" "}
              <span className="font-semibold">{seasonResult.targeted_track}</span>
            </p>
          )}
          <p className="mt-0.5 break-all font-mono text-[10px] text-text-secondary">
            {seasonResult.path}
          </p>
        </div>
      )}
      {rainResult && (
        <div className="space-y-2 rounded-xl border border-status-green/30 bg-status-green/10 p-3 text-[11px]">
          <p className="font-semibold text-status-green">
            {t("rosterGen.rainTestCreated")}{" "}
            {rainResult.seasons.map((s) => `"${s}"`).join(" · ")}
          </p>
          <p className="text-text-secondary">
            {t("rosterGen.rainTestIntro")}
          </p>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-[10px]">
              <thead className="text-text-muted">
                <tr>
                  <th className="py-1 pr-3">{t("rosterGen.colDriver")}</th>
                  <th className="px-2">{t("rosterGen.colFactor")}</th>
                  <th className="px-2">{t("rosterGen.colDry")}</th>
                  <th className="px-2">{t("rosterGen.colWet")}</th>
                  <th className="px-2">{t("rosterGen.colStorm")}</th>
                </tr>
              </thead>
              <tbody className="font-mono text-text-secondary">
                {rainResult.rows.map((r) => {
                  const key = r.name === "Bom-Mestre" || r.name === "Alien-Pessimo";
                  return (
                    <tr
                      key={r.name}
                      className={key ? "text-status-green font-semibold" : ""}
                    >
                      <td className="py-0.5 pr-3">{r.name}</td>
                      <td className="px-2">{r.fator_chuva}</td>
                      <td className="px-2">{r.skill_seco}</td>
                      <td className="px-2">{r.skill_molhado}</td>
                      <td className="px-2">{r.skill_tempestade}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <p className="text-[10px] leading-snug text-text-muted">
            {t("rosterGen.rainValidationLead")} <b>{t("rosterGen.dry")}</b>{" "}
            {t("rosterGen.rainValidationMid")}{" "}
            <b>{t("rosterGen.storm")}</b>
            {t("rosterGen.rainValidationComma")}{" "}
            <span className="text-status-green">Bom-Mestre</span>{" "}
            {t("rosterGen.rainValidationPass")}{" "}
            <span className="text-status-green">Alien-Pessimo</span>{" "}
            {t("rosterGen.rainValidationTail")}
          </p>
        </div>
      )}

      {preview && (
        <div className="space-y-2 rounded-xl border border-white/10 bg-white/5 p-3 text-[11px]">
          <div className="flex flex-wrap items-baseline gap-2">
            <span className="text-sm font-semibold text-text-primary">
              {t("rosterGen.previewTitle")} {preview.track_name}
            </span>
            <span className="text-text-secondary">
              {t("rosterGen.previewLapsDnfs", {
                laps: preview.total_laps,
                dnfs: preview.total_dnfs,
              })}
            </span>
            <span className="text-[10px] text-text-muted">
              {t("rosterGen.previewReadOnly")}
            </span>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-[10px]">
              <thead className="text-text-muted">
                <tr>
                  <th className="py-1 pr-2">{t("rosterGen.colPos")}</th>
                  <th className="px-2">{t("rosterGen.colDriver")}</th>
                  <th className="px-2">{t("rosterGen.colTeam")}</th>
                  <th className="px-2 text-center">{t("rosterGen.colGrid")}</th>
                  <th className="px-2 text-center">{t("rosterGen.colDelta")}</th>
                  <th className="px-2 text-right">{t("rosterGen.colBestLap")}</th>
                  <th className="px-2 text-right">{t("rosterGen.colGap")}</th>
                  <th className="px-2 text-center">{t("rosterGen.colFastest")}</th>
                  <th className="px-2">{t("rosterGen.colStatus")}</th>
                </tr>
              </thead>
              <tbody className="text-text-secondary">
                {[...preview.race_results]
                  .sort((a, b) => {
                    if (a.is_dnf !== b.is_dnf) return a.is_dnf ? 1 : -1;
                    return a.finish_position - b.finish_position;
                  })
                  .map((r, i) => (
                    <tr
                      key={r.pilot_id || i}
                      className={r.is_jogador ? "font-semibold text-status-green" : ""}
                    >
                      <td className="py-0.5 pr-2 font-mono">
                        {r.is_dnf ? "—" : i + 1}
                      </td>
                      <td className="px-2">
                        {r.is_jogador ? "🧑 " : ""}
                        {r.pilot_name}
                      </td>
                      <td className="px-2 text-text-muted">{r.team_name}</td>
                      <td className="px-2 text-center font-mono">
                        {r.grid_position > 0 ? r.grid_position : "—"}
                      </td>
                      <td
                        className={[
                          "px-2 text-center font-mono",
                          r.positions_gained > 0
                            ? "text-status-green"
                            : r.positions_gained < 0
                              ? "text-status-red"
                              : "",
                        ].join(" ")}
                      >
                        {r.positions_gained > 0
                          ? `+${r.positions_gained}`
                          : r.positions_gained < 0
                            ? r.positions_gained
                            : "—"}
                      </td>
                      <td className="px-2 text-right font-mono">
                        {fmtLap(r.best_lap_time_ms)}
                      </td>
                      <td className="px-2 text-right font-mono text-text-muted">
                        {i === 0 || r.is_dnf ? "—" : fmtGap(r.gap_to_winner_ms)}
                      </td>
                      <td className="px-2 text-center">
                        {r.has_fastest_lap ? "🟣" : ""}
                      </td>
                      <td className="px-2 text-[10px]">
                        {r.is_dnf ? (
                          <span className="text-status-red">
                            DNF{r.dnf_reason ? ` · ${r.dnf_reason}` : ""}
                          </span>
                        ) : r.notable_incident ? (
                          <span className="text-text-muted">⚠️ {r.notable_incident}</span>
                        ) : (
                          "—"
                        )}
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
          <p className="text-[10px] leading-snug text-text-muted">
            {t("rosterGen.previewValidation")}
          </p>
        </div>
      )}

      {/* Cor do carro do jogador (aplicar na garagem do iRacing) */}
      {paint && (
        <div className="space-y-2 rounded-xl border border-white/10 bg-white/5 p-3">
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold text-text-primary">
              {t("rosterGen.paintTitle")}
            </span>
            <span className="text-[11px] text-text-secondary">
              {t("rosterGen.paintTeam")} {paint.team_name}
            </span>
          </div>
          <div className="flex items-center gap-3">
            <span
              className="h-9 w-9 shrink-0 rounded-md border border-white/20"
              style={{ background: paint.color1 }}
            />
            <p className="text-[11px] leading-snug text-text-secondary">
              {t("rosterGen.paintInstrLead")}{" "}
              <span className="text-text-primary">{t("rosterGen.paintGarageMenu")}</span>
              {t("rosterGen.paintPatternLabel")}{" "}
              <b className="text-text-primary">{paint.pattern}</b>
              {t("rosterGen.paintColorsLabel")}{" "}
              <span className="font-mono text-text-primary">{paint.color1}</span> /{" "}
              <span className="font-mono">{paint.color2}</span> /{" "}
              <span className="font-mono">{paint.color3}</span>.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <code className="rounded bg-black/30 px-2 py-1 font-mono text-[11px] text-text-primary">
              {paint.spec}
            </code>
            <button
              type="button"
              onClick={() => navigator.clipboard?.writeText(paint.spec)}
              className="rounded-md bg-white/10 px-2 py-1 text-[11px] text-text-secondary transition-glass hover:text-text-primary"
            >
              {t("rosterGen.copy")}
            </button>
          </div>
          <p className="text-[10px] text-text-muted">
            {t("rosterGen.paintApplyOnce")}
          </p>
        </div>
      )}
      {paintError && (
        <p className="text-[10px] text-text-muted">{t("rosterGen.paintError", { error: paintError })}</p>
      )}

      {/* Captura do custid (base da pintura automática — Opção B) */}
      <div className="space-y-1.5 rounded-xl border border-white/10 bg-white/5 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <GlassButton
            variant="secondary"
            onClick={detectCustid}
            className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
          >
            {t("rosterGen.detectCustid")}
          </GlassButton>
          {custid != null && (
            <span className="text-[11px] text-status-green">
              {t("rosterGen.custidLabel")} <span className="font-mono">{custid}</span>
            </span>
          )}
        </div>
        {custidMsg && (
          <p className="text-[10px] text-text-muted">{custidMsg}</p>
        )}
        <p className="text-[10px] leading-snug text-text-muted">
          {t("rosterGen.custidCapturedLead")}{" "}
          <b className="text-text-secondary">{t("rosterGen.custidCapturedBold")}</b>{" "}
          {t("rosterGen.custidCapturedTail")}
        </p>

        {/* Pintura automática do carro do jogador (Opção B) */}
        <div className="mt-1 border-t border-white/10 pt-2">
          <div className="flex flex-wrap items-center gap-2">
            <GlassButton
              variant="primary"
              onClick={applyPaint}
              disabled={applyBusy || custid == null || !careerId}
              className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
            >
              {applyBusy ? t("rosterGen.painting") : t("rosterGen.paintCarBtn")}
            </GlassButton>
            <span className="text-[10px] text-text-muted">
              {t("rosterGen.paintGenLead")} <span className="font-mono">car_{custid ?? "?"}.tga</span> {t("rosterGen.paintGenTail")}
            </span>
          </div>
          {applyError && (
            <p className="mt-1 text-[10px] text-status-red">{applyError}</p>
          )}
          {applyResult && (
            <div className="mt-1 text-[10px] text-status-green">
              <p>
                {t("rosterGen.paintedColor")}{" "}
                <span className="font-mono">{applyResult.color}</span>.
              </p>
              <p className="break-all font-mono text-text-secondary">
                {applyResult.path}
              </p>
            </div>
          )}
          <p className="mt-1 text-[10px] italic leading-snug text-text-muted">
            {t("rosterGen.paintCloseWarning")}
          </p>
        </div>
      </div>

      <p className="text-[10px] italic leading-snug text-text-muted">
        {t("rosterGen.finalWarning")}
      </p>
      </GlassCard>
    </>
  );
}

function Field({ label, children }) {
  return (
    <label className="block space-y-1">
      <span className="text-[10px] uppercase tracking-wide text-text-muted">{label}</span>
      {children}
    </label>
  );
}

export default RosterGenPanel;
