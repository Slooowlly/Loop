import { useState, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import GlassSelect from "../components/ui/GlassSelect";
import GlassButton from "../components/ui/GlassButton";
import LoadingOverlay from "../components/ui/LoadingOverlay";
import ParticleBackdrop from "../components/ui/ParticleBackdrop";

// Fundo da tela: "particles" (campo de partículas, igual ao menu) ou "glass"
// (gradiente azul original). Para voltar ao fundo anterior, troque para "glass".
const SETTINGS_BG = "particles";

function Settings() {
  const navigate = useNavigate();
  const location = useLocation();
  const [config, setConfig] = useState(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [navigating, setNavigating] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");

  // Race Control: macro de bandeira amarela (edita o app.ini do iRacing).
  const [yellowStatus, setYellowStatus] = useState(null);
  const [yellowMsg, setYellowMsg] = useState("");

  // Estado do "automático" (flag do RaceControl) e trava anti-duplo-clique.
  const [autoYellow, setAutoYellow] = useState(false);
  const [yellowBusy, setYellowBusy] = useState(false);

  // A macro já é instalada sozinha ao abrir as Configurações (useEffect abaixo);
  // aqui o toggle só liga/desliga o disparo automático da bandeira.
  const raceControlOn = Boolean(yellowStatus?.installed && autoYellow);
  async function toggleRaceControl() {
    if (yellowBusy || !yellowStatus?.installed) return;
    const next = !autoYellow;
    setYellowBusy(true);
    setAutoYellow(next);
    try {
      await invoke("iracing_set_auto_yellow", { enabled: next });
    } catch (err) {
      setAutoYellow(!next);
      setYellowMsg(String(err));
    } finally {
      setYellowBusy(false);
    }
  }

  // Ao abrir as Configurações, garante a macro de bandeira instalada no app.ini.
  // Assim, quando o jogador for correr, ela já está pronta — sem ele precisar
  // ativar nada nem saber o que é "macro". (O iRacing reescreve o app.ini ao
  // fechar, então o ideal é o sim estar fechado neste momento.)
  useEffect(() => {
    (async () => {
      try {
        let status = await invoke("iracing_yellow_macro_status");
        if (status?.app_ini_found && !status.installed) {
          status = await invoke("iracing_install_yellow_macro");
        }
        setYellowStatus(status);
      } catch (err) {
        console.error("Falha ao preparar a macro de bandeira:", err);
      }
    })();
    invoke("iracing_auto_yellow_enabled").then((v) => setAutoYellow(Boolean(v))).catch(() => {});
  }, []);

  useEffect(() => {
    loadConfig();
  }, []);

  // Rola ate a secao pedida pelos atalhos do menu (navigate state.section).
  useEffect(() => {
    if (loading) return undefined;
    const sec = location.state?.section;
    if (!sec) return undefined;
    const id = setTimeout(() => {
      document.getElementById(sec)?.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 60);
    return () => clearTimeout(id);
  }, [loading, location.state]);

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
    // Só o fundo (tela cheia, sem texto piscando nem colapsar) até o config carregar.
    return (
      <div className="entry-shell !block !h-full px-4 py-12">
        <div className="entry-backdrop" />
        <div className="entry-glow left-[5%] top-[10%] h-80 w-80 bg-blue-500/10" />
        <div className="entry-glow bottom-[5%] right-[5%] h-96 w-96 bg-cyan-500/10" />
      </div>
    );
  }

  return (
    <div className="entry-shell !block !h-full !min-h-0 !overflow-y-auto px-4 py-12">
      {SETTINGS_BG === "particles" ? (
        <ParticleBackdrop />
      ) : (
        <>
          <div className="entry-backdrop" />
          <div className="entry-glow left-[5%] top-[10%] h-80 w-80 bg-blue-500/10" />
          <div className="entry-glow bottom-[5%] right-[5%] h-96 w-96 bg-cyan-500/10" />
        </>
      )}

      <div className="page-in relative z-10 mx-auto max-w-xl space-y-5">
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

        {/* Painel único de configurações em lista */}
        <div className="overflow-hidden rounded-2xl border border-white/10 bg-white/[0.04] shadow-[0_16px_50px_rgba(0,0,0,0.25)] backdrop-blur-2xl">
          {/* ── Grupo: Geral ── */}
          <div id="geral" style={{ scrollMarginTop: "1rem" }} className="px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted">
            Geral
          </div>

          {/* Idioma */}
          <div className="flex items-center justify-between gap-4 border-t border-white/[0.05] px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-medium text-text-primary">Idioma</p>
              <p className="text-[11px] text-text-muted">Menus e ferramentas do jogo.</p>
            </div>
            <div className="w-[160px] shrink-0">
              <GlassSelect
                value={config.language}
                onChange={(e) => handleChange("language", e.target.value)}
                className="!min-h-0 !rounded-lg !px-3 !py-2 !text-[13px]"
              >
                <option value="pt-BR">Português (BR)</option>
                <option value="en-US">English (US)</option>
              </GlassSelect>
            </div>
          </div>

          {/* Salvamento automático */}
          <div
            className="flex cursor-pointer items-center justify-between gap-4 border-t border-white/[0.05] px-5 py-3.5"
            onClick={() => handleToggle("autosave_enabled")}
          >
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-text-primary">Salvamento automático</p>
              <p className="text-[11px] text-text-muted">Salva o progresso ao final de cada semana/corrida.</p>
            </div>
            <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${config.autosave_enabled ? "bg-accent-primary" : "bg-white/10"}`}>
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${config.autosave_enabled ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {/* ── Grupo: Corrida ── */}
          <div id="racecontrol" style={{ scrollMarginTop: "1rem" }} className="border-t border-white/[0.07] px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted">
            Corrida
          </div>

          {/* Bandeira amarela automática — liga/desliga o disparo (a macro já foi instalada ao abrir a tela) */}
          <div
            className={`flex items-center justify-between gap-4 border-t border-white/[0.05] px-5 py-3.5 ${
              yellowStatus?.installed && !yellowBusy ? "cursor-pointer" : "cursor-default opacity-55"
            }`}
            onClick={yellowStatus?.installed && !yellowBusy ? toggleRaceControl : undefined}
          >
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-text-primary">Bandeira amarela automática</p>
              <p className="text-[11px] text-text-muted">
                {!yellowStatus?.app_ini_found
                  ? "iRacing não encontrado neste PC."
                  : raceControlOn
                    ? "Ligada — joga amarela sozinho em acidentes contra a IA."
                    : "Joga bandeira amarela sozinho em acidentes contra a IA."}
              </p>
            </div>
            <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${raceControlOn ? "bg-status-green" : "bg-white/10"}`}>
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${raceControlOn ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {yellowMsg && (
            <div className="border-t border-white/[0.05] px-5 py-3">
              <p className="rounded-lg border border-status-yellow/30 bg-status-yellow/10 px-3 py-2 text-[11px] font-medium text-text-primary">{yellowMsg}</p>
            </div>
          )}

          {/* Detalhes técnicos — escondidos por padrão */}
          <details className="group border-t border-white/[0.05]">
            <summary className="flex cursor-pointer list-none items-center gap-1 px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-text-muted transition-glass hover:text-text-secondary [&::-webkit-details-marker]:hidden">
              Detalhes técnicos
              <span className="transition-transform group-open:rotate-90">›</span>
            </summary>
            <div className="space-y-1 px-5 pb-4">
              <div className="flex items-center justify-between text-[11px]">
                <span className="uppercase tracking-[0.1em] text-text-muted">app.ini</span>
                <span className={`font-semibold ${yellowStatus?.app_ini_found ? "text-status-green" : "text-status-red"}`}>
                  {yellowStatus?.app_ini_found ? "Encontrado" : "Não encontrado"}
                </span>
              </div>
              {yellowStatus?.app_ini_path && (
                <p className="truncate font-mono text-[9px] text-text-muted">{yellowStatus.app_ini_path}</p>
              )}
              {yellowStatus?.slot != null && (
                <div className="flex items-center justify-between text-[10px] text-text-muted">
                  <span>Slot AutoChatStr{yellowStatus.slot}</span>
                  <span className="font-mono">
                    original: "{yellowStatus.original}" · atual: "{yellowStatus.current_value}"
                  </span>
                </div>
              )}
              <p className="pt-1 text-[9px] leading-snug text-text-muted">
                Substitui a macro "You're welcome" por <span className="font-mono">!y$</span>. O iRacing reescreve o app.ini ao
                fechar — por isso, edite com ele fechado. Backup em <span className="font-mono">app.ini.iracerapp.bak</span>.
              </p>
            </div>
          </details>
        </div>

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
