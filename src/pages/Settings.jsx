import { useState, useEffect } from "react";
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
    <div className="entry-shell overflow-y-auto px-4 py-12">
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
