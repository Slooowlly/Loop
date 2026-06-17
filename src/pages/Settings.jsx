import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import GlassCard from "../components/ui/GlassCard";
import GlassSelect from "../components/ui/GlassSelect";
import GlassButton from "../components/ui/GlassButton";
import LoadingOverlay from "../components/ui/LoadingOverlay";

function Settings() {
  const navigate = useNavigate();
  const [config, setConfig] = useState(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [navigating, setNavigating] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  // Teste isolado do SDK do iRacing (comando `iracing_read_session`).
  const [sdkTesting, setSdkTesting] = useState(false);
  const [sdkResult, setSdkResult] = useState(null);
  const [sdkError, setSdkError] = useState("");

  async function testIracingSdk() {
    setSdkTesting(true);
    setSdkResult(null);
    setSdkError("");
    try {
      const session = await invoke("iracing_read_session");
      setSdkResult(session);
    } catch (err) {
      setSdkError(typeof err === "string" ? err : String(err));
    } finally {
      setSdkTesting(false);
    }
  }

  // Telemetria ao vivo: polling do comando `iracing_read_telemetry`.
  const [livePolling, setLivePolling] = useState(false);
  const [liveData, setLiveData] = useState(null);
  const [liveError, setLiveError] = useState("");
  const pollRef = useRef(null);
  const pollingRef = useRef(false); // evita chamadas sobrepostas se uma demorar

  function stopLiveTelemetry() {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
    setLivePolling(false);
  }

  function startLiveTelemetry() {
    if (pollRef.current) return;
    setLiveError("");
    setLivePolling(true);
    // ~15 Hz: suave para a UI sem martelar o backend.
    pollRef.current = setInterval(async () => {
      if (pollingRef.current) return;
      pollingRef.current = true;
      try {
        const data = await invoke("iracing_read_telemetry");
        setLiveData(data);
        setLiveError("");
      } catch (err) {
        setLiveError(typeof err === "string" ? err : String(err));
        setLiveData(null);
      } finally {
        pollingRef.current = false;
      }
    }, 66);
  }

  // Monitor de Corrida unificado (tentativas + batidas + DNF).
  const [racePolling, setRacePolling] = useState(false);
  const [raceData, setRaceData] = useState(null);
  const [raceFlash, setRaceFlash] = useState(null);
  const racePollRef = useRef(null);
  const raceBusyRef = useRef(false);
  const flashTimeoutRef = useRef(null);

  function stopRaceWatch() {
    if (racePollRef.current) {
      clearInterval(racePollRef.current);
      racePollRef.current = null;
    }
    setRacePolling(false);
  }

  function startRaceWatch() {
    if (racePollRef.current) return;
    setRacePolling(true);
    // O backend amostra a 60 Hz; aqui só lemos o snapshot a ~7 Hz.
    racePollRef.current = setInterval(async () => {
      if (raceBusyRef.current) return;
      raceBusyRef.current = true;
      try {
        const status = await invoke("iracing_poll_race");
        setRaceData(status);
        if (status.event) {
          setRaceFlash(status.event);
          if (flashTimeoutRef.current) clearTimeout(flashTimeoutRef.current);
          flashTimeoutRef.current = setTimeout(() => setRaceFlash(null), 5000);
        }
      } catch (err) {
        console.error("Falha no poll do monitor:", err);
      } finally {
        raceBusyRef.current = false;
      }
    }, 150);
  }

  async function resetRaceTest() {
    await invoke("iracing_reset_race").catch(() => {});
    setRaceData(null);
    setRaceFlash(null);
  }

  // Garante que os pollings parem ao sair da tela.
  useEffect(() => {
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
      if (racePollRef.current) clearInterval(racePollRef.current);
      if (flashTimeoutRef.current) clearTimeout(flashTimeoutRef.current);
    };
  }, []);

  useEffect(() => {
    loadConfig();
  }, []);

  async function loadConfig() {
    try {
      const cfg = await invoke("get_config");
      setConfig(cfg);
    } catch (err) {
      console.error("Falha ao carregar config:", err);
    } finally {
      setLoading(false);
    }
  }

  async function saveConfig(newCfg) {
    setSaving(true);
    setErrorMessage("");
    try {
      await invoke("update_config", { newConfig: newCfg });
    } catch (err) {
      console.error("Falha ao salvar config:", err);
      setErrorMessage(err.toString());
      loadConfig();
    } finally {
      setSaving(false);
    }
  }

  async function selectDirectory(field) {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Selecionar Pasta do iRacing",
      });
      if (selected) {
        const newCfg = { ...config, [field]: selected };
        setConfig(newCfg);
        saveConfig(newCfg);
      }
    } catch (err) {
      console.error("Erro ao abrir seletor:", err);
    }
  }

  const handleToggle = (field) => {
    const newCfg = { ...config, [field]: !config[field] };
    setConfig(newCfg);
    saveConfig(newCfg);
  };

  const handleChange = (field, value) => {
    const newCfg = { ...config, [field]: value };
    setConfig(newCfg);
    saveConfig(newCfg);
  };

  if (loading || !config) {
    return (
      <div className="entry-shell flex items-center justify-center">
        <p className="animate-pulse text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
          Carregando Configurações...
        </p>
      </div>
    );
  }

  return (
    <div className="entry-shell !block !h-full !min-h-0 !overflow-y-auto px-4 py-12">
      <div className="entry-backdrop" />
      <div className="entry-glow left-[5%] top-[10%] h-80 w-80 bg-blue-500/10" />
      <div className="entry-glow bottom-[5%] right-[5%] h-96 w-96 bg-cyan-500/10" />

      <div className="relative z-10 mx-auto max-w-2xl space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between pb-4">
          <button
            onClick={() => navigate("/menu")}
            className="group flex items-center gap-2 text-text-secondary transition-glass hover:text-text-primary"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="18"
              height="18"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              className="transition-transform group-hover:-translate-x-1"
            >
              <path d="m15 18-6-6 6-6" />
            </svg>
            <span className="text-[11px] font-bold uppercase tracking-[0.22em]">Voltar</span>
          </button>

          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-semibold tracking-tight text-text-primary">
              Configurações
            </h1>
            {saving && (
              <span className="animate-pulse rounded-full border border-accent-primary/30 bg-accent-primary/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.12em] text-accent-primary">
                Salvando...
              </span>
            )}
          </div>
        </div>

        {/* Error Alert */}
        {errorMessage && (
          <div className="flex animate-scale-in items-center gap-4 rounded-2xl border border-status-red/30 bg-status-red/10 p-4">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              className="shrink-0 text-status-red"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
            <p className="text-xs font-semibold text-status-red">{errorMessage}</p>
            <button
              onClick={() => setErrorMessage("")}
              className="ml-auto text-status-red/60 transition-glass hover:text-status-red"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M18 6 6 18" />
                <path d="m6 6 12 12" />
              </svg>
            </button>
          </div>
        )}

        {/* Section: Preferências Gerais */}
        <GlassCard hover={false} className="glass-strong rounded-[30px] !p-8 gap-6 flex flex-col">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <div className="h-4 w-1.5 rounded-full bg-accent-primary" />
              <h2 className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
                Preferências Gerais
              </h2>
            </div>
            <p className="text-[11px] text-text-secondary">Personalize sua experiência de jogo e idioma.</p>
          </div>

          <div className="grid gap-6 pt-2">
            {/* Idioma */}
            <div className="flex items-center justify-between gap-4">
              <div className="space-y-0.5">
                <label className="text-sm font-semibold text-text-primary">Idioma da Interface</label>
                <p className="text-[11px] text-text-secondary">Escolha o idioma dos menus e ferramentas.</p>
              </div>
              <GlassSelect
                value={config.language}
                onChange={(e) => handleChange("language", e.target.value)}
                className="w-auto min-w-[160px]"
              >
                <option value="pt-BR">Português (BR)</option>
                <option value="en-US">English (US)</option>
              </GlassSelect>
            </div>

            <hr className="border-white/8" />

            {/* Autosave */}
            <div
              className="flex cursor-pointer items-center justify-between"
              onClick={() => handleToggle("autosave_enabled")}
            >
              <div className="space-y-0.5">
                <label className="text-sm font-semibold text-text-primary cursor-pointer">
                  Salvamento Automático
                </label>
                <p className="text-[11px] text-text-secondary">
                  Salva o progresso ao final de cada semana/corrida.
                </p>
              </div>
              <div
                className={`h-6 w-12 rounded-full p-1 transition-all duration-300 ${
                  config.autosave_enabled ? "bg-accent-primary" : "bg-white/10"
                }`}
              >
                <div
                  className={`h-4 w-4 rounded-full bg-white transition-all duration-300 ${
                    config.autosave_enabled ? "translate-x-6" : "translate-x-0"
                  }`}
                />
              </div>
            </div>
          </div>
        </GlassCard>

        {/* Section: Integração iRacing */}
        <GlassCard hover={false} className="glass-strong rounded-[30px] !p-8 gap-6 flex flex-col">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <div className="h-4 w-1.5 rounded-full bg-accent-primary" />
              <h2 className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
                Integração iRacing
              </h2>
            </div>
            <p className="text-[11px] text-text-secondary">
              Configure os locais das pastas para exportação de dados.
            </p>
          </div>

          <div className="grid gap-8 pt-2">
            {[
              { label: "Pasta AI Rosters", field: "airosters_path", desc: "Local das pastas de oponentes (.json)" },
              { label: "Pasta AI Seasons", field: "aiseasons_path", desc: "Local dos arquivos de temporadas (.json)" },
            ].map((pathItem) => (
              <div key={pathItem.field} className="space-y-3">
                <div className="flex items-center justify-between gap-4">
                  <label className="text-sm font-semibold text-text-primary">{pathItem.label}</label>
                  <GlassButton
                    variant="secondary"
                    onClick={() => selectDirectory(pathItem.field)}
                    className="shrink-0 text-[10px]"
                  >
                    Alterar Pasta
                  </GlassButton>
                </div>
                <div className="glass-light flex items-center gap-3 rounded-2xl border border-white/10 px-4 py-3">
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    className="shrink-0 text-text-muted"
                  >
                    <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
                  </svg>
                  <p className="flex-1 truncate font-mono text-[11px] text-text-secondary">
                    {config[pathItem.field] || "(Não configurado)"}
                  </p>
                </div>
                <p className="text-[10px] italic text-text-muted">{pathItem.desc}</p>
              </div>
            ))}
          </div>
        </GlassCard>

        {/* Section: Teste SDK iRacing (telemetria ao vivo) */}
        <GlassCard hover={false} className="glass-strong rounded-[30px] !p-8 gap-6 flex flex-col">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <div className="h-4 w-1.5 rounded-full bg-accent-primary" />
              <h2 className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
                Teste SDK iRacing
              </h2>
            </div>
            <p className="text-[11px] text-text-secondary">
              Lê a sessão do iRacing em execução (shared memory). Requer o sim aberto numa sessão.
            </p>
          </div>

          <div className="grid gap-4 pt-2">
            <GlassButton
              variant="secondary"
              onClick={testIracingSdk}
              disabled={sdkTesting}
              className="shrink-0 text-[11px]"
            >
              {sdkTesting ? "Lendo..." : "Testar Conexão"}
            </GlassButton>

            {sdkError && (
              <div className="rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3">
                <p className="text-[11px] font-semibold text-status-red">{sdkError}</p>
              </div>
            )}

            {sdkResult && (
              <div className="space-y-3">
                <div className="grid grid-cols-2 gap-3">
                  {[
                    ["Pista", sdkResult.track_name || "—"],
                    ["Versão API", sdkResult.api_version],
                    ["Tick Rate", `${sdkResult.tick_rate}/s`],
                    ["Sessão (update)", sdkResult.session_info_update],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      className="glass-light rounded-2xl border border-white/10 px-4 py-3"
                    >
                      <p className="text-[10px] uppercase tracking-[0.12em] text-text-muted">{label}</p>
                      <p className="truncate text-sm font-semibold text-text-primary">{value}</p>
                    </div>
                  ))}
                </div>
                <div className="space-y-2">
                  <p className="text-[10px] uppercase tracking-[0.12em] text-text-muted">
                    YAML de Sessão ({sdkResult.session_info_len} bytes)
                  </p>
                  <pre className="glass-light max-h-64 overflow-auto rounded-2xl border border-white/10 px-4 py-3 font-mono text-[10px] leading-relaxed text-text-secondary whitespace-pre-wrap">
                    {sdkResult.session_yaml}
                  </pre>
                </div>
              </div>
            )}
          </div>
        </GlassCard>

        {/* Section: Telemetria ao vivo */}
        <GlassCard hover={false} className="glass-strong rounded-[30px] !p-8 gap-6 flex flex-col">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <div className={`h-4 w-1.5 rounded-full ${livePolling ? "animate-pulse bg-status-green" : "bg-accent-primary"}`} />
              <h2 className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
                Telemetria ao Vivo
              </h2>
            </div>
            <p className="text-[11px] text-text-secondary">
              Lê os dados por tick (~15 Hz) do iRacing em execução: velocidade, RPM, marcha, pedais e mais.
            </p>
          </div>

          <div className="grid gap-4 pt-2">
            <GlassButton
              variant={livePolling ? "primary" : "secondary"}
              onClick={livePolling ? stopLiveTelemetry : startLiveTelemetry}
              className="shrink-0 text-[11px]"
            >
              {livePolling ? "Parar" : "Iniciar Telemetria"}
            </GlassButton>

            {liveError && (
              <div className="rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3">
                <p className="text-[11px] font-semibold text-status-red">{liveError}</p>
              </div>
            )}

            {liveData && (
              <div className="space-y-4">
                {/* Marcha + velocidade em destaque */}
                <div className="flex items-end justify-center gap-8 py-2">
                  <div className="text-center">
                    <p className="text-[10px] uppercase tracking-[0.12em] text-text-muted">Marcha</p>
                    <p className="font-mono text-5xl font-bold text-text-primary tabular-nums">
                      {liveData.gear < 0 ? "R" : liveData.gear === 0 ? "N" : liveData.gear}
                    </p>
                  </div>
                  <div className="text-center">
                    <p className="text-[10px] uppercase tracking-[0.12em] text-text-muted">Velocidade</p>
                    <p className="font-mono text-5xl font-bold text-text-primary tabular-nums">
                      {Math.round(liveData.speed_kmh)}
                      <span className="ml-1 text-sm text-text-muted">km/h</span>
                    </p>
                  </div>
                </div>

                {/* Barras de RPM, acelerador e freio */}
                <div className="space-y-3">
                  {[
                    { label: "RPM", value: liveData.rpm, max: 12000, fmt: (v) => Math.round(v), color: "bg-accent-primary" },
                    { label: "Acelerador", value: liveData.throttle * 100, max: 100, fmt: (v) => `${Math.round(v)}%`, color: "bg-status-green" },
                    { label: "Freio", value: liveData.brake * 100, max: 100, fmt: (v) => `${Math.round(v)}%`, color: "bg-status-red" },
                  ].map((bar) => (
                    <div key={bar.label} className="space-y-1">
                      <div className="flex items-center justify-between text-[10px] uppercase tracking-[0.12em]">
                        <span className="text-text-muted">{bar.label}</span>
                        <span className="font-mono text-text-secondary tabular-nums">{bar.fmt(bar.value)}</span>
                      </div>
                      <div className="h-2 overflow-hidden rounded-full bg-white/10">
                        <div
                          className={`h-full rounded-full ${bar.color} transition-[width] duration-75`}
                          style={{ width: `${Math.min(100, Math.max(0, (bar.value / bar.max) * 100))}%` }}
                        />
                      </div>
                    </div>
                  ))}
                </div>

                {/* Demais canais */}
                <div className="grid grid-cols-3 gap-3">
                  {[
                    ["Volta", liveData.lap],
                    ["Posição", liveData.position],
                    ["Progresso", `${(liveData.lap_dist_pct * 100).toFixed(1)}%`],
                    ["Combustível", `${liveData.fuel_level.toFixed(1)} L`],
                    ["Tempo volta", `${liveData.lap_current_time.toFixed(1)}s`],
                    ["Na pista", liveData.on_track ? "Sim" : "Não"],
                  ].map(([label, value]) => (
                    <div key={label} className="glass-light rounded-2xl border border-white/10 px-3 py-2 text-center">
                      <p className="text-[9px] uppercase tracking-[0.1em] text-text-muted">{label}</p>
                      <p className="truncate font-mono text-sm font-semibold text-text-primary tabular-nums">{value}</p>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </GlassCard>

        {/* Section: Monitor de Corrida (unificado) */}
        <GlassCard hover={false} className="glass-strong rounded-[30px] !p-8 gap-6 flex flex-col">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <div className={`h-4 w-1.5 rounded-full ${racePolling ? "animate-pulse bg-status-green" : "bg-accent-primary"}`} />
              <h2 className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
                Monitor de Corrida
              </h2>
            </div>
            <p className="text-[11px] text-text-secondary">
              Tentativas, batidas e desfecho num só lugar. Cada tentativa lista suas batidas; reiniciar/fechar sem cruzar a bandeirada vira DNF citando a pior. Completar a corrida rebaixa a gravidade (o carro sobreviveu).
            </p>
          </div>

          <div className="grid gap-4 pt-2">
            <div className="flex gap-3">
              <GlassButton variant={racePolling ? "primary" : "secondary"} onClick={racePolling ? stopRaceWatch : startRaceWatch} className="shrink-0 text-[11px]">
                {racePolling ? "Parar" : "Iniciar Monitor"}
              </GlassButton>
              <GlassButton variant="secondary" onClick={resetRaceTest} className="shrink-0 text-[11px]">
                Zerar Teste
              </GlassButton>
            </div>

            {raceFlash && (
              <div className="animate-scale-in rounded-2xl border border-status-yellow/40 bg-status-yellow/10 px-4 py-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.14em] text-status-yellow">Evento</p>
                <p className="text-[11px] font-semibold text-text-primary">{raceFlash}</p>
              </div>
            )}

            {raceData && (
              <div className="space-y-4">
                <div className="flex items-center gap-4">
                  <div className="glass-light rounded-2xl border border-white/10 px-4 py-3 text-center">
                    <p className="text-[10px] uppercase tracking-[0.12em] text-text-muted">Tentativa</p>
                    <p className="font-mono text-3xl font-bold text-text-primary tabular-nums">
                      {raceData.attempt_number > 0 ? `#${raceData.attempt_number}` : "—"}
                    </p>
                  </div>
                  <div className="flex-1 space-y-1.5">
                    {[
                      ["Estado", raceData.session_state_label],
                      ["Superfície", raceData.track_surface_label],
                      ["Voltas", raceData.lap_completed],
                      ["Incidentes", raceData.incident_count],
                    ].map(([label, value]) => (
                      <div key={label} className="flex items-center justify-between text-[11px]">
                        <span className="font-mono uppercase tracking-[0.1em] text-text-muted">{label}</span>
                        <span className="font-mono font-semibold text-text-primary tabular-nums">{value}</span>
                      </div>
                    ))}
                  </div>
                </div>

                {(() => {
                  const sevColor = {
                    leve: "text-status-yellow", moderado: "text-status-orange", grave: "text-status-red",
                    destruído: "text-status-purple", catastrófico: "text-[#ff2d55]",
                  }[raceData.crash_severity_now] || "text-text-secondary";
                  const barColor = {
                    leve: "bg-status-yellow", moderado: "bg-status-orange", grave: "bg-status-red",
                    destruído: "bg-status-purple", catastrófico: "bg-[#ff2d55]",
                  }[raceData.crash_severity_now] || "bg-accent-primary";
                  return (
                    <div className="glass-light rounded-2xl border border-white/10 px-4 py-2.5">
                      <div className="flex items-center justify-between">
                        <span className="text-[10px] uppercase tracking-[0.12em] text-text-muted">
                          Batida ao vivo · {(Math.round(raceData.g_force * 10) / 10).toFixed(1)}g · {Math.round(raceData.speed_kmh)} km/h
                        </span>
                        <span className={`text-sm font-bold tabular-nums ${sevColor}`}>
                          {Math.round(raceData.crash_score)} {raceData.crash_severity_now}
                        </span>
                      </div>
                      <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-white/10">
                        <div className={`h-full rounded-full ${barColor} transition-[width] duration-75`} style={{ width: `${Math.min(100, (raceData.crash_score / 230) * 100)}%` }} />
                      </div>
                    </div>
                  );
                })()}

                {!raceData.connected && (
                  <p className="text-[11px] italic text-text-muted">
                    iRacing fechado ou sessão não pronta. Se você fechou no meio de uma corrida, a tentativa ativa vira DNF.
                  </p>
                )}

                {raceData.attempts.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-[10px] uppercase tracking-[0.12em] text-text-muted">Livro de tentativas</p>
                    <div className="space-y-2">
                      {raceData.attempts.slice().reverse().map((a) => {
                        const meta = {
                          active: { label: "Ativa", chip: "bg-status-green/20 text-status-green", border: "border-status-green/30 bg-status-green/10" },
                          finished: { label: "Finalizada", chip: "bg-accent-primary/20 text-accent-primary", border: "border-accent-primary/30 bg-accent-primary/10" },
                          dnf: { label: "DNF", chip: "bg-status-red/20 text-status-red", border: "border-status-red/30 bg-status-red/10" },
                          not_started: { label: "Não largou", chip: "bg-white/10 text-text-muted", border: "border-white/10 bg-white/5" },
                        }[a.status] || { label: a.status, chip: "bg-white/10 text-text-muted", border: "border-white/10 bg-white/5" };
                        const sevChip = {
                          leve: "bg-status-yellow/20 text-status-yellow", moderado: "bg-status-orange/20 text-status-orange",
                          grave: "bg-status-red/20 text-status-red", destruído: "bg-status-purple/20 text-status-purple",
                          catastrófico: "bg-[#ff2d55]/25 text-[#ff2d55]",
                        };
                        return (
                          <div key={a.number} className={`rounded-2xl border px-4 py-3 ${meta.border}`}>
                            <div className="flex items-center gap-3">
                              <span className="font-mono text-sm font-bold text-text-primary">#{a.number}</span>
                              <span className={`rounded-full px-2 py-0.5 text-[9px] font-bold uppercase tracking-[0.1em] ${meta.chip}`}>{meta.label}</span>
                              <span className="ml-auto font-mono text-[10px] text-text-muted">{a.laps_completed} voltas</span>
                            </div>
                            {a.reason && <p className="mt-1.5 text-[10px] text-text-secondary">{a.reason}</p>}
                            {a.crashes.length > 0 && (
                              <div className="mt-2 space-y-1">
                                {a.crashes.map((c, i) => (
                                  <div key={i} className="flex items-center gap-2 text-[10px]">
                                    <span className={`shrink-0 rounded-full px-2 py-0.5 text-[9px] font-bold uppercase ${sevChip[c.severity] || "bg-white/10 text-text-muted"}`}>{c.severity}</span>
                                    <span className="shrink-0 text-text-muted">volta {c.lap}</span>
                                    <span className="flex-1 truncate text-text-muted">{c.factors.join(" · ")}</span>
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </GlassCard>

        <div className="flex flex-col items-center gap-4 pt-8">
          <GlassButton
            variant="primary"
            onClick={async () => {
              const saves = await invoke("list_saves").catch(() => []);
              if (saves.length > 0) {
                setNavigating(true);
                setTimeout(() => navigate("/menu"), 700);
              } else {
                const confirmed = window.confirm(
                  "Deseja criar sua primeira carreira agora?",
                );
                if (confirmed) {
                  setNavigating(true);
                  setTimeout(() => navigate("/new-career"), 700);
                }
              }
            }}
          >
            Salvar
          </GlassButton>
          <p className="text-[10px] font-bold uppercase tracking-[0.4em] text-text-muted">
            Loop — v{config.version}
          </p>
        </div>
      </div>
      <LoadingOverlay open={navigating} title="Salvando" message="Aplicando configurações..." />
    </div>
  );
}

export default Settings;
