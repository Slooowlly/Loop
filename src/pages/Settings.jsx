import { useState, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import GlassSelect from "../components/ui/GlassSelect";
import GlassButton from "../components/ui/GlassButton";
import LoadingOverlay from "../components/ui/LoadingOverlay";
import ParticleBackdrop from "../components/ui/ParticleBackdrop";
import RivalryPerceptionPanel from "../components/iracing/RivalryPerceptionPanel";
import useCareerStore from "../stores/useCareerStore";
import { useTranslation } from "react-i18next";

// Fundo da tela: "particles" (campo de partículas, igual ao menu) ou "glass"
// (gradiente azul original). Para voltar ao fundo anterior, troque para "glass".
const SETTINGS_BG = "particles";

function Settings() {
  const { t } = useTranslation();
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

  // Teste de comando de chat de TEXTO LIVRE (ex.: !black #1 20) — caminho
  // parametrizado que NÃO depende de macro no app.ini. Só pra validar.
  const [chatText, setChatText] = useState("!black #1 20");
  const [chatMsg, setChatMsg] = useState("");
  const [chatBusy, setChatBusy] = useState(false);
  async function sendChatTest() {
    const text = chatText.trim();
    if (chatBusy || !text) return;
    setChatBusy(true);
    setChatMsg("");
    try {
      await invoke("iracing_send_chat_text", { text });
      setChatMsg(`Enviado: "${text}"`);
    } catch (err) {
      setChatMsg(String(err));
    } finally {
      setChatBusy(false);
    }
  }

  // Teste do DISPARO AO VIVO da quebra: arma uma falha garantida no carro do jogador
  // (motor na parede) pra próxima volta cruzada. Ao passar pela linha, o monitor manda o
  // !black/!dq sozinho — valida o pipeline ponta a ponta (detecção de volta + disparo).
  const [armBusy, setArmBusy] = useState(false);
  const [armMsg, setArmMsg] = useState("");

  // Demo do overlay de RÁDIO: liga um card de exemplo ciclando na tela (pra achar/posicionar
  // o overlay sem esperar uma quebra real). Estado mora no backend (vale pra janela do rádio).
  const [radioDemo, setRadioDemo] = useState(false);
  async function toggleRadioDemo() {
    const next = !radioDemo;
    setRadioDemo(next);
    try {
      await invoke("overlay_set_demo", { on: next });
    } catch {
      /* ignora */
    }
  }

  // Gravador de corrida (DEBUG): salva a telemetria crua + YAML + histórico num arquivo, pra
  // calibrar o app com dados reais de pista. Só pra dev — não vai pro fluxo comercial.
  const [capture, setCapture] = useState({ active: false, frames: 0, dir: "" });
  const [captureMsg, setCaptureMsg] = useState("");
  async function refreshCapture() {
    try {
      setCapture(await invoke("race_capture_status"));
    } catch {
      /* ignora */
    }
  }
  async function toggleCapture() {
    try {
      if (capture.active) {
        const path = await invoke("race_capture_stop");
        setCaptureMsg(path ? `Salvo: ${path}` : "Gravação parada.");
      } else {
        const path = await invoke("race_capture_start");
        setCaptureMsg(`Gravando… entre numa sessão do iRacing. Arquivo: ${path}`);
      }
      await refreshCapture();
    } catch (err) {
      setCaptureMsg(String(err));
    }
  }
  async function armBreakdown() {
    if (armBusy) return;
    setArmBusy(true);
    setArmMsg("");
    try {
      const ok = await invoke("iracing_arm_test_breakdown");
      setArmMsg(
        ok
          ? "Armado! Cruze a linha de chegada — a quebra dispara sozinha na próxima volta."
          : "Não deu pra armar: entre numa sessão do iRacing (o número do seu carro precisa estar visível).",
      );
    } catch (err) {
      setArmMsg(String(err));
    } finally {
      setArmBusy(false);
    }
  }
  async function armBreakdownGrid() {
    if (armBusy) return;
    setArmBusy(true);
    setArmMsg("");
    try {
      await invoke("iracing_arm_test_breakdown_grid");
      setArmMsg(
        "Grade armada! Com a corrida rolando, os carros vão largar peças ao longo das próximas voltas (1 a cada ~1,5s pra não spammar).",
      );
    } catch (err) {
      setArmMsg(String(err));
    } finally {
      setArmBusy(false);
    }
  }

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
    invoke("overlay_demo_enabled").then((v) => setRadioDemo(Boolean(v))).catch(() => {});
  }, []);

  // Poll do status da gravação de debug (contagem de frames enquanto grava).
  useEffect(() => {
    refreshCapture();
    const t = setInterval(refreshCapture, 2000);
    return () => clearInterval(t);
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
    // Reflete a troca de idioma no store na hora (decide fallback PT vs "erro"
    // localizado nas telas de narrativa), sem exigir reload da carreira.
    if (field === "language") {
      useCareerStore.getState().setLanguage(value);
    }
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
            onClick={() => navigate(location.state?.from ?? "/menu")}
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
            <span className="text-[11px] font-bold uppercase tracking-[0.22em]">{t("settings.back")}</span>
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
        <div className="glass-strong overflow-hidden rounded-2xl">
          {/* ── Grupo: Geral ── */}
          <div id="geral" style={{ scrollMarginTop: "1rem" }} className="px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-text-secondary">
            {t("settings.general")}
          </div>

          {/* Idioma */}
          <div className="flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.language.label")}</p>
              <p className="text-[11px] text-text-secondary">{t("settings.language.desc")}</p>
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
            className="flex cursor-pointer items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5"
            onClick={() => handleToggle("autosave_enabled")}
          >
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.autosave.label")}</p>
              <p className="text-[11px] text-text-secondary">{t("settings.autosave.desc")}</p>
            </div>
            <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${config.autosave_enabled ? "bg-accent-primary" : "bg-white/10"}`}>
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${config.autosave_enabled ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {/* ── Grupo: Corrida ── */}
          <div id="racecontrol" style={{ scrollMarginTop: "1rem" }} className="border-t border-white/10 px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-text-secondary">
            {t("settings.raceSection")}
          </div>

          {/* Bandeira amarela automática — liga/desliga o disparo (a macro já foi instalada ao abrir a tela) */}
          <div
            className={`flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5 ${
              yellowStatus?.installed && !yellowBusy ? "cursor-pointer" : "cursor-default opacity-55"
            }`}
            onClick={yellowStatus?.installed && !yellowBusy ? toggleRaceControl : undefined}
          >
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.autoYellow.label")}</p>
              <p className="text-[11px] text-text-secondary">
                {!yellowStatus?.app_ini_found
                  ? t("settings.autoYellow.notFound")
                  : raceControlOn
                    ? t("settings.autoYellow.on")
                    : t("settings.autoYellow.off")}
              </p>
            </div>
            <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${raceControlOn ? "bg-status-green" : "bg-white/10"}`}>
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${raceControlOn ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {yellowMsg && (
            <div className="border-t border-white/10 px-5 py-3">
              <p className="rounded-lg border border-status-yellow/30 bg-status-yellow/10 px-3 py-2 text-[11px] font-medium text-text-primary">{yellowMsg}</p>
            </div>
          )}

          {/* Detalhes técnicos — escondidos por padrão */}
          <details className="group border-t border-white/10">
            <summary className="flex cursor-pointer list-none items-center gap-1 px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-text-secondary transition-glass hover:text-text-primary [&::-webkit-details-marker]:hidden">
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

          {/* Teste de comando de chat livre (ex.: !black #1 20) — caminho parametrizado, sem macro */}
          <div className="border-t border-white/10 px-5 py-3.5">
            <p className="text-[13px] font-medium text-text-primary">Comando de chat (teste)</p>
            <p className="pb-2.5 text-[11px] text-text-secondary">
              Envia texto livre ao chat do iRacing (foca a janela, abre o chat e digita). Com o sim aberto numa sessão com IA.
            </p>
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={chatText}
                onChange={(e) => setChatText(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && sendChatTest()}
                spellCheck={false}
                placeholder="!black #1 20"
                className="min-w-0 flex-1 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[12px] text-text-primary outline-none transition-glass focus:border-white/25"
              />
              <button
                type="button"
                onClick={sendChatTest}
                disabled={chatBusy || !chatText.trim()}
                className={`shrink-0 rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
                  chatBusy || !chatText.trim()
                    ? "cursor-default bg-white/5 text-text-muted"
                    : "cursor-pointer bg-status-yellow/20 text-text-primary hover:bg-status-yellow/30"
                }`}
              >
                {chatBusy ? "Enviando…" : "Enviar"}
              </button>
            </div>
            {chatMsg && (
              <p className="mt-2.5 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">{chatMsg}</p>
            )}
          </div>

          {/* Teste do disparo AO VIVO da quebra (arma o carro do jogador pra próxima volta) */}
          <div className="border-t border-white/10 px-5 py-3.5">
            <p className="text-[13px] font-medium text-text-primary">Quebra ao vivo (teste)</p>
            <p className="pb-2.5 text-[11px] text-text-secondary">
              Arma uma falha garantida no SEU carro. Com o sim aberto numa sessão correndo, clique e cruze a linha — o monitor dispara o <span className="font-mono">!black</span>/<span className="font-mono">!dq</span> sozinho na próxima volta.
            </p>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={armBreakdown}
                disabled={armBusy}
                className={`rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
                  armBusy
                    ? "cursor-default bg-white/5 text-text-muted"
                    : "cursor-pointer bg-status-red/20 text-text-primary hover:bg-status-red/30"
                }`}
              >
                {armBusy ? "Armando…" : "Armar (meu carro)"}
              </button>
              <button
                type="button"
                onClick={armBreakdownGrid}
                disabled={armBusy}
                className={`rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
                  armBusy
                    ? "cursor-default bg-white/5 text-text-muted"
                    : "cursor-pointer bg-status-red/20 text-text-primary hover:bg-status-red/30"
                }`}
              >
                {armBusy ? "Armando…" : "Armar (grade toda)"}
              </button>
            </div>
            {armMsg && (
              <p className="mt-2.5 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">{armMsg}</p>
            )}
          </div>

          {/* Demo do overlay de rádio: card de exemplo ciclando, pra achar/posicionar o overlay */}
          <div className="flex items-center justify-between border-t border-white/10 px-5 py-3.5">
            <div className="min-w-0 pr-4">
              <p className="text-[13px] font-medium text-text-primary">Testar overlay de rádio</p>
              <p className="text-[11px] text-text-secondary">
                Mostra um card de exemplo ciclando no centro da tela (sobre o jogo), pra você achar e posicionar o overlay do rádio sem esperar uma quebra real. Lembre de desligar depois.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={radioDemo}
              onClick={toggleRadioDemo}
              className={`relative h-6 w-11 shrink-0 rounded-full transition-glass ${
                radioDemo ? "bg-status-green/70" : "bg-white/15"
              }`}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${
                  radioDemo ? "left-[22px]" : "left-0.5"
                }`}
              />
            </button>
          </div>

          {/* Gravador de corrida (DEBUG): salva a telemetria real pra calibração */}
          <div className="border-t border-white/10 px-5 py-3.5">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="text-[13px] font-medium text-text-primary">Gravar corrida (debug)</p>
                <p className="text-[11px] text-text-secondary">
                  Salva TODA a telemetria da corrida ao vivo (+ YAML da sessão + histórico) num arquivo, pra calibrar o app com dados reais de pista. Só pra dev — não entra no jogo comercial.
                </p>
              </div>
              <button
                type="button"
                onClick={toggleCapture}
                className={`shrink-0 whitespace-nowrap rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
                  capture.active
                    ? "cursor-pointer bg-status-red/25 text-text-primary hover:bg-status-red/35"
                    : "cursor-pointer bg-white/10 text-text-primary hover:bg-white/15"
                }`}
              >
                {capture.active ? `Parar (${capture.frames} frames)` : "Iniciar gravação"}
              </button>
            </div>
            {captureMsg && (
              <p className="mt-2.5 break-all rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">{captureMsg}</p>
            )}
            {capture.dir && (
              <p className="mt-1 break-all text-[10px] text-text-muted">Pasta: {capture.dir}</p>
            )}
          </div>

          {/* Explicador de rivalidades percebidas (debug/calibração) */}
          <RivalryPerceptionPanel />
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
                  t("settings.firstCareerConfirm"),
                );
                if (confirmed) {
                  setNavigating(true);
                  setTimeout(() => navigate("/new-career"), 700);
                }
              }
            }}
          >
            {t("settings.save")}
          </GlassButton>
          <p className="text-[10px] font-bold uppercase tracking-[0.4em] text-text-muted">
            Loop — v{(config.version ?? "1.0.0").split(".").slice(0, 2).join(".")}
          </p>
        </div>
      </div>
      <LoadingOverlay open={navigating} title={t("settings.saving")} message={t("settings.applying")} />
    </div>
  );
}

export default Settings;
