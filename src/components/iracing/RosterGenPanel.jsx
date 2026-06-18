import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import GlassCard from "../ui/GlassCard";
import GlassButton from "../ui/GlassButton";

const CARS = [
  { key: "mx5", label: "Mazda MX-5" },
  { key: "gr86", label: "Toyota GR86" },
  { key: "bmwm2", label: "BMW M2 (G87)" },
];

function RosterGenPanel() {
  const [saves, setSaves] = useState([]);
  const [careerId, setCareerId] = useState("");
  const [categoria, setCategoria] = useState("");
  const [carKey, setCarKey] = useState("mx5");
  const [rosterName, setRosterName] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState(null);
  const [error, setError] = useState("");
  const [paint, setPaint] = useState(null);
  const [paintError, setPaintError] = useState("");
  const [custid, setCustid] = useState(null);
  const [custidMsg, setCustidMsg] = useState("");
  const [connected, setConnected] = useState(false);
  const [applyBusy, setApplyBusy] = useState(false);
  const [applyResult, setApplyResult] = useState(null);
  const [applyError, setApplyError] = useState("");

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
    setCustidMsg("Lendo a sessão do iRacing…");
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
    setRosterName((prev) => prev || `Carreira ${selected.player_name || ""}`.trim());
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

  const canGenerate = careerId && categoria.trim() && rosterName.trim() && !busy;

  return (
    <GlassCard hover={false} className="space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <span className="text-2xl">🧩</span>
          <div>
            <h2 className="text-lg font-semibold text-text-primary">
              Gerar AI Roster (carreira → iRacing)
            </h2>
            <p className="text-xs text-text-secondary">
              Escreve o grid da carreira como roster de IA do iRacing — os pilotos
              entram com nossos nomes, atributos e cor do time.
            </p>
          </div>
        </div>
        <span
          className={[
            "flex shrink-0 items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[11px] font-semibold",
            connected
              ? "bg-status-green/15 text-status-green"
              : "bg-white/5 text-text-muted",
          ].join(" ")}
          title="Detecção automática — liga sozinho quando o iRacing abre"
        >
          <span
            className={[
              "inline-block h-1.5 w-1.5 rounded-full",
              connected ? "animate-pulse bg-status-green" : "bg-text-muted",
            ].join(" ")}
          />
          {connected ? "iRacing conectado" : "iRacing fechado"}
        </span>
      </div>

      {saves.length === 0 ? (
        <p className="rounded-xl border border-white/10 bg-white/5 p-4 text-center text-xs text-text-secondary">
          Nenhum save encontrado. Crie uma carreira primeiro.
        </p>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Carreira">
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

          <Field label="Categoria (id)">
            <input
              value={categoria}
              onChange={(e) => setCategoria(e.target.value)}
              placeholder="ex.: mazda_rookie, gt3…"
              className="w-full rounded-lg border border-white/10 bg-white/5 px-2 py-1.5 text-xs text-text-primary outline-none"
            />
          </Field>

          <Field label="Carro">
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

          <Field label="Nome do roster (pasta)">
            <input
              value={rosterName}
              onChange={(e) => setRosterName(e.target.value)}
              placeholder="ex.: Carreira João"
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
          {busy ? "Gerando…" : "Gerar roster.json"}
        </GlassButton>
        {selected && (
          <span className="text-[10px] text-text-muted">
            Categoria padrão do save: <span className="font-mono">{selected.category}</span>
          </span>
        )}
      </div>

      {error && (
        <p className="rounded-xl border border-status-red/30 bg-status-red/10 px-3 py-2 text-[11px] font-semibold text-status-red">
          {error}
        </p>
      )}
      {result && (
        <div className="rounded-xl border border-status-green/30 bg-status-green/10 px-3 py-2 text-[11px] text-status-green">
          <p className="font-semibold">
            Roster gerado com {result.drivers} piloto(s) de IA.
          </p>
          <p className="mt-0.5 break-all font-mono text-[10px] text-text-secondary">
            {result.path}
          </p>
        </div>
      )}

      {/* Cor do carro do jogador (aplicar na garagem do iRacing) */}
      {paint && (
        <div className="space-y-2 rounded-xl border border-white/10 bg-white/5 p-3">
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold text-text-primary">
              🎨 Cor do seu carro
            </span>
            <span className="text-[11px] text-text-secondary">
              — time {paint.team_name}
            </span>
          </div>
          <div className="flex items-center gap-3">
            <span
              className="h-9 w-9 shrink-0 rounded-md border border-white/20"
              style={{ background: paint.color1 }}
            />
            <p className="text-[11px] leading-snug text-text-secondary">
              No iRacing → <span className="text-text-primary">Garagem → Pintura</span>:
              padrão <b className="text-text-primary">{paint.pattern}</b>, cores{" "}
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
              copiar
            </button>
          </div>
          <p className="text-[10px] text-text-muted">
            Aplique uma vez — seu carro fica igual ao do seu companheiro de equipe.
          </p>
        </div>
      )}
      {paintError && (
        <p className="text-[10px] text-text-muted">Cor do seu carro: {paintError}</p>
      )}

      {/* Captura do custid (base da pintura automática — Opção B) */}
      <div className="space-y-1.5 rounded-xl border border-white/10 bg-white/5 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <GlassButton
            variant="secondary"
            onClick={detectCustid}
            className="!min-h-0 !rounded-lg !px-3 !py-1.5 text-[11px]"
          >
            Detectar meu custid (iRacing)
          </GlassButton>
          {custid != null && (
            <span className="text-[11px] text-status-green">
              ✓ custid: <span className="font-mono">{custid}</span>
            </span>
          )}
        </div>
        {custidMsg && (
          <p className="text-[10px] text-text-muted">{custidMsg}</p>
        )}
        <p className="text-[10px] leading-snug text-text-muted">
          Capturado <b className="text-text-secondary">automaticamente</b> enquanto
          você corre (o monitor lê a sessão e guarda). Este botão só confirma o que
          já foi pego. É a base da pintura automática do seu carro.
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
              {applyBusy ? "Pintando…" : "🎨 Pintar meu carro na cor do time"}
            </GlassButton>
            <span className="text-[10px] text-text-muted">
              gera <span className="font-mono">car_{custid ?? "?"}.tga</span> na cor do time
            </span>
          </div>
          {applyError && (
            <p className="mt-1 text-[10px] text-status-red">{applyError}</p>
          )}
          {applyResult && (
            <div className="mt-1 text-[10px] text-status-green">
              <p>
                ✓ Pintado na cor{" "}
                <span className="font-mono">{applyResult.color}</span>.
              </p>
              <p className="break-all font-mono text-text-secondary">
                {applyResult.path}
              </p>
            </div>
          )}
          <p className="mt-1 text-[10px] italic leading-snug text-text-muted">
            ⚠️ Feche o iRacing antes (ele lê as pinturas ao abrir / ao entrar na
            garagem). Precisa de "Mostrar pinturas personalizadas" ligado.
          </p>
        </div>
      </div>

      <p className="text-[10px] italic leading-snug text-text-muted">
        ⚠️ Abra o iRacing depois de gerar (ele lê os rosters ao iniciar). O jogador
        é excluído do roster — ele dirige, não vira IA.
      </p>
    </GlassCard>
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
