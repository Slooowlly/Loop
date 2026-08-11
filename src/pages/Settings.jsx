import { useState, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import GlassSelect from "../components/ui/GlassSelect";
import GlassButton from "../components/ui/GlassButton";
import LoadingOverlay from "../components/ui/LoadingOverlay";
import ParticleBackdrop from "../components/ui/ParticleBackdrop";
import RivalryPerceptionPanel from "../components/iracing/RivalryPerceptionPanel";
import IracingDiagnosticoPanel from "../components/iracing/IracingDiagnosticoPanel";
import PttEngenheiroSettings from "../components/iracing/PttEngenheiroSettings";
import { useOverlayFlags } from "../overlay/useOverlayFlags";
import { estaLigada as vozSpotterLigada, falar as falarSpotter, ligar as ligarVozSpotter } from "../lib/spotterVoice";
import { definirVolume, volumeRadio } from "../lib/volumeRadio";
import useConfiguracaoDoApp from "../hooks/useConfiguracaoDoApp";
import useFerramentasDeDebug from "../hooks/useFerramentasDeDebug";
import useRaceControl from "../hooks/useRaceControl";
import useSaves from "../hooks/useSaves";
import useSpotterNativo from "../hooks/useSpotterNativo";
import { useTranslation } from "react-i18next";

// Fundo da tela: "particles" (campo de partículas, igual ao menu) ou "glass"
// (gradiente azul original). Para voltar ao fundo anterior, troque para "glass".
const SETTINGS_BG = "particles";

function Settings() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  // Os três blocos de dados da tela vivem em hooks próprios: config do app, Race Control
  // e as ferramentas de bancada. O componente ficou com o desenho e com o que é só dele.
  const {
    config,
    setConfig,
    loading,
    saving,
    errorMessage,
    setErrorMessage,
    handleToggle,
    handleChange,
  } = useConfiguracaoDoApp();
  const {
    yellowStatus,
    yellowMsg,
    autoYellow,
    yellowBusy,
    raceControlOn,
    toggleRaceControl,
    chatText,
    setChatText,
    chatMsg,
    chatBusy,
    sendChatTest,
  } = useRaceControl(t);
  const {
    capture,
    captureMsg,
    toggleCapture,
    radioDemo,
    toggleRadioDemo,
    armBusy,
    armMsg,
    armBreakdown,
    armBreakdownGrid,
  } = useFerramentasDeDebug(t);

  const [navigating, setNavigating] = useState(false);
  const [debugMenuOpen, setDebugMenuOpen] = useState(false);

  // Estado AO VIVO do pipeline de overlay — o que diz se o iRacing está em VR agora.
  const overlayFlags = useOverlayFlags();

  // Spotter: o Loop cala o nativo do iRacing enquanto está aberto e o devolve ao fechar.
  // A ponte (status + escrita no app.ini) mora em `hooks/useSpotterNativo`; aqui fica só
  // o eco otimista no config da tela, que é o que o interruptor desenha.
  const {
    ocupado: spotterBusy,
    disponivel: spotterDisponivel,
    alternar: alternarSpotter,
  } = useSpotterNativo();

  async function toggleSpotter() {
    if (spotterBusy || !config) return;
    const next = !config.spotter_takeover;
    setConfig({ ...config, spotter_takeover: next });
    const falha = await alternarSpotter(next);
    if (falha) setErrorMessage(falha);
  }

  // Só para decidir o destino do botão de baixo (menu vs. primeira carreira). Sem
  // carregar ao montar: a lista é lida no clique, que é quando a resposta importa.
  const { recarregar: recarregarSaves } = useSaves({ carregarAoMontar: false });

  // Voz do spotter. Mora no localStorage, não no config do backend: é preferência
  // de saída de áudio desta máquina, e o Rust não tem nada a decidir sobre ela.
  const [spotterVoz, setSpotterVoz] = useState(vozSpotterLigada);
  // Mesma história: preferência de saída de áudio DESTA máquina, e vale para as duas
  // bocas do rádio — o spotter e o engenheiro são a mesma pessoa.
  const [volumeRad, setVolumeRad] = useState(volumeRadio);

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
              {t("settings.title")}
            </h1>
            {saving && (
              <span className="animate-pulse rounded-full border border-accent-primary/30 bg-accent-primary/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.12em] text-accent-primary">
                {t("settings.savingBadge")}
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

          {/* Telemetria de produto — reversível a qualquer momento. `telemetry_enabled`
              é tri-estado no disco (null = nunca perguntado), mas aqui só existe
              ligado/desligado: qualquer clique grava um booleano explícito. */}
          <div
            className="flex cursor-pointer items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5"
            onClick={() => handleChange("telemetry_enabled", !config.telemetry_enabled)}
          >
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.telemetry.label")}</p>
              <p className="text-[11px] text-text-secondary">
                {config.telemetry_enabled ? t("settings.telemetry.on") : t("settings.telemetry.off")}
              </p>
            </div>
            <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${config.telemetry_enabled ? "bg-accent-primary" : "bg-white/10"}`}>
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${config.telemetry_enabled ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {/* ── Grupo: Corrida ── */}
          <div id="racecontrol" style={{ scrollMarginTop: "1rem" }} className="border-t border-white/10 px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-text-secondary">
            {t("settings.raceSection")}
          </div>

          {/* Diagnóstico da conexão com o iRacing — FORA do menu de debug de
              propósito: é o que o jogador precisa achar quando a telemetria vem
              zerada, e é de lá que sai o log para anexar num relato. */}
          <IracingDiagnosticoPanel />

          {/* Overlay em VR — quando desenhar os painéis (torre + rádio nos quads da OpenXR
              layer). "Automático" segue a DETECÇÃO: a layer só é carregada quando o
              iRacing abre em VR, então ela mesma é a resposta. Importa porque o pipeline é
              caro — cada quadro copia 8 MB de pixels, 10 a 30 vezes por segundo — e no
              monitor isso é só pressão de memória no WebView2, que já derrubou a janela
              com "Out of Memory" numa corrida longa. */}
          <div className="flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.vrOverlay.label")}</p>
              <p className="text-[11px] text-text-secondary">
                {overlayFlags.simInVr ? t("settings.vrOverlay.detected") : t("settings.vrOverlay.notDetected")}
              </p>
            </div>
            <div className="w-[160px] shrink-0">
              <GlassSelect
                value={config.vr_overlay_mode || "auto"}
                onChange={(e) => handleChange("vr_overlay_mode", e.target.value)}
                className="!min-h-0 !rounded-lg !px-3 !py-2 !text-[13px]"
              >
                <option value="auto">{t("settings.vrOverlay.modeAuto")}</option>
                <option value="on">{t("settings.vrOverlay.modeOn")}</option>
                <option value="off">{t("settings.vrOverlay.modeOff")}</option>
              </GlassSelect>
            </div>
          </div>

          {/* Override de live/gravação: só faz sentido se o VR puder estar ativo, então
              desaparece no modo "off" — aí o overlay de monitor abre sempre. */}
          {(config.vr_overlay_mode || "auto") !== "off" && (
            <div
              className="flex cursor-pointer items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5"
              onClick={() => handleToggle("monitor_overlay_in_vr")}
            >
              <div className="min-w-0 pl-4">
                <p className="text-[13px] font-medium text-text-primary">{t("settings.monitorOverlayInVr.label")}</p>
                <p className="text-[11px] text-text-secondary">
                  {config.monitor_overlay_in_vr
                    ? t("settings.monitorOverlayInVr.on")
                    : t("settings.monitorOverlayInVr.off")}
                </p>
              </div>
              <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${config.monitor_overlay_in_vr ? "bg-accent-primary" : "bg-white/10"}`}>
                <div className={`h-4 w-4 rounded-full bg-white transition-all ${config.monitor_overlay_in_vr ? "translate-x-5" : "translate-x-0"}`} />
              </div>
            </div>
          )}

          {/* Spotter do Loop — cala o nativo do iRacing (voice/text da seção [SPCC] do
              app.ini) e o devolve ao fechar o Loop. Desabilitado quando não há app.ini. */}
          <div
            className={`flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5 ${
              spotterDisponivel && !spotterBusy ? "cursor-pointer" : "cursor-default opacity-55"
            }`}
            onClick={spotterDisponivel && !spotterBusy ? toggleSpotter : undefined}
          >
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.spotter.label")}</p>
              <p className="text-[11px] text-text-secondary">
                {!spotterDisponivel
                  ? t("settings.spotter.semAppIni")
                  : config.spotter_takeover
                    ? t("settings.spotter.on")
                    : t("settings.spotter.off")}
              </p>
            </div>
            <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${config.spotter_takeover ? "bg-accent-primary" : "bg-white/10"}`}>
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${config.spotter_takeover ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {/* Pintura automática do carro do jogador. Grava car_<custid>.tga na pasta de
              pintura do iRacing junto com a exportação da etapa, e de novo quando ele troca
              de equipe no mercado. Sem popup de propósito: o arquivo é local, ninguém mais
              na sessão vê essa cor, e a pintura que já estava lá é preservada uma vez em
              .tga.loop-bak antes da primeira escrita. */}
          <div
            className="flex cursor-pointer items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5"
            onClick={() => handleToggle("auto_paint_car")}
          >
            <div className="min-w-0">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.autoPaint.label")}</p>
              <p className="text-[11px] text-text-secondary">
                {config.auto_paint_car ? t("settings.autoPaint.on") : t("settings.autoPaint.off")}
              </p>
            </div>
            <div className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${config.auto_paint_car ? "bg-accent-primary" : "bg-white/10"}`}>
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${config.auto_paint_car ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {/* Voz do spotter + botão de ouvir agora. O teste automático sai ao sentar no
              carro, mas quem quer conferir a saída de áudio ANTES de abrir o simulador
              não deveria ter que entrar numa sessão pra isso. */}
          <div className="flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5">
            <div
              className="min-w-0 flex-1 cursor-pointer"
              onClick={() => {
                const next = !spotterVoz;
                setSpotterVoz(next);
                ligarVozSpotter(next);
              }}
            >
              <p className="text-[13px] font-medium text-text-primary">
                {t("settings.spotterVoz.label")}
              </p>
              <p className="text-[11px] text-text-secondary">
                {spotterVoz ? t("settings.spotterVoz.on") : t("settings.spotterVoz.off")}
              </p>
            </div>
            <button
              type="button"
              onClick={() => falarSpotter("teste", { forcar: true })}
              className="shrink-0 cursor-pointer rounded-lg bg-white/10 px-4 py-2 text-[12px] font-semibold text-text-primary transition-glass hover:bg-white/20"
            >
              {t("settings.spotterVoz.testar")}
            </button>
            <div
              className={`h-6 w-11 shrink-0 cursor-pointer rounded-full p-1 transition-all ${spotterVoz ? "bg-accent-primary" : "bg-white/10"}`}
              onClick={() => {
                const next = !spotterVoz;
                setSpotterVoz(next);
                ligarVozSpotter(next);
              }}
            >
              <div className={`h-4 w-4 rounded-full bg-white transition-all ${spotterVoz ? "translate-x-5" : "translate-x-0"}`} />
            </div>
          </div>

          {/* Volume do rádio — UM controle para as duas bocas. O acervo sai da cadeia em
              quase escala cheia (RMS 0,175, pico 0,97), que é o nível certo dentro do
              arquivo e ensurdecedor tocado a ganho 1 por cima do jogo. Quanto atenuar
              depende do fone e do volume do simulador, então é do jogador. */}
          <div className="flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5">
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-medium text-text-primary">
                {t("settings.volumeRadio.label")}
              </p>
              <p className="text-[11px] text-text-secondary">
                {t("settings.volumeRadio.hint")}
              </p>
            </div>
            <span className="w-10 shrink-0 text-right text-[12px] font-semibold tabular-nums text-text-secondary">
              {Math.round(volumeRad * 100)}%
            </span>
            <input
              type="range"
              min="0"
              max="100"
              step="5"
              value={Math.round(volumeRad * 100)}
              aria-label={t("settings.volumeRadio.label")}
              onChange={(e) => {
                const v = Number(e.target.value) / 100;
                setVolumeRad(v);
                definirVolume(v);
              }}
              // Solta o botão e ouve: escolher volume sem referência é chute, e a peça de
              // teste é a mesma que o jogador vai ouvir na pista.
              onMouseUp={() => falarSpotter("teste", { forcar: true })}
              onKeyUp={() => falarSpotter("teste", { forcar: true })}
              className="h-1.5 w-32 shrink-0 cursor-pointer appearance-none rounded-full bg-white/10 accent-accent-primary"
            />
          </div>

          {/* Engenheiro por rádio (push-to-talk): voz, botão e microfone. Fica logo abaixo
              da voz do spotter porque são a mesma pessoa — o que os separa é quem começa
              a conversa. */}
          <PttEngenheiroSettings />

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

          <div className="flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5">
            <div className="min-w-0 pr-4">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.debug.menuLabel")}</p>
              <p className="text-[11px] text-text-secondary">
                {t("settings.debug.menuDesc")}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-label={t("settings.debug.menuLabel")}
              aria-checked={debugMenuOpen}
              onClick={() => setDebugMenuOpen((open) => !open)}
              className={`relative h-6 w-11 shrink-0 rounded-full transition-glass ${
                debugMenuOpen ? "bg-status-green/70" : "bg-white/15"
              }`}
            >
              <span
                className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${
                  debugMenuOpen ? "left-[22px]" : "left-0.5"
                }`}
              />
            </button>
          </div>

          {debugMenuOpen && (
            <>
          {/* Detalhes técnicos — escondidos por padrão */}
          <details className="group border-t border-white/10">
            <summary className="flex cursor-pointer list-none items-center gap-1 px-5 py-2.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-text-secondary transition-glass hover:text-text-primary [&::-webkit-details-marker]:hidden">
              {t("settings.debug.techDetails")}
              <span className="transition-transform group-open:rotate-90">›</span>
            </summary>
            <div className="space-y-1 px-5 pb-4">
              <div className="flex items-center justify-between text-[11px]">
                <span className="uppercase tracking-[0.1em] text-text-muted">app.ini</span>
                <span className={`font-semibold ${yellowStatus?.app_ini_found ? "text-status-green" : "text-status-red"}`}>
                  {yellowStatus?.app_ini_found ? t("settings.debug.appIniFound") : t("settings.debug.appIniNotFound")}
                </span>
              </div>
              {yellowStatus?.app_ini_path && (
                <p className="truncate font-mono text-[9px] text-text-muted">{yellowStatus.app_ini_path}</p>
              )}
              {yellowStatus?.slot != null && (
                <div className="flex items-center justify-between text-[10px] text-text-muted">
                  <span>{t("settings.debug.slot", { slot: yellowStatus.slot })}</span>
                  <span className="font-mono">
                    {t("settings.debug.slotValues", { original: yellowStatus.original, current: yellowStatus.current_value })}
                  </span>
                </div>
              )}
              <p className="pt-1 text-[9px] leading-snug text-text-muted">
                {t("settings.debug.macroNote1")}<span className="font-mono">!y$</span>{t("settings.debug.macroNote2")}<span className="font-mono">app.ini.iracerapp.bak</span>{t("settings.debug.macroNote3")}
              </p>
            </div>
          </details>

          {/* Teste de comando de chat livre (ex.: !black #1 20) — caminho parametrizado, sem macro */}
          <div className="border-t border-white/10 px-5 py-3.5">
            <p className="text-[13px] font-medium text-text-primary">{t("settings.debug.chatTitle")}</p>
            <p className="pb-2.5 text-[11px] text-text-secondary">
              {t("settings.debug.chatDesc")}
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
                {chatBusy ? t("settings.debug.sending") : t("settings.debug.send")}
              </button>
            </div>
            {chatMsg && (
              <p className="mt-2.5 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">{chatMsg}</p>
            )}
          </div>

          {/* Teste do disparo AO VIVO da quebra (arma o carro do jogador pra próxima volta) */}
          <div className="border-t border-white/10 px-5 py-3.5">
            <p className="text-[13px] font-medium text-text-primary">{t("settings.debug.breakdownTitle")}</p>
            <p className="pb-2.5 text-[11px] text-text-secondary">
              {t("settings.debug.breakdownDesc1")}<span className="font-mono">!black</span>/<span className="font-mono">!dq</span>{t("settings.debug.breakdownDesc2")}
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
                {armBusy ? t("settings.debug.arming") : t("settings.debug.armMyCar")}
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
                {armBusy ? t("settings.debug.arming") : t("settings.debug.armGrid")}
              </button>
            </div>
            {armMsg && (
              <p className="mt-2.5 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">{armMsg}</p>
            )}
          </div>

          {/* Demo do overlay de rádio: card de exemplo ciclando, pra achar/posicionar o overlay */}
          <div className="flex items-center justify-between border-t border-white/10 px-5 py-3.5">
            <div className="min-w-0 pr-4">
              <p className="text-[13px] font-medium text-text-primary">{t("settings.debug.radioTitle")}</p>
              <p className="text-[11px] text-text-secondary">
                {t("settings.debug.radioDesc")}
              </p>
            </div>
            <button
              type="button"
              role="switch"
              aria-label={t("settings.debug.radioTitle")}
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
                <p className="text-[13px] font-medium text-text-primary">{t("settings.debug.captureTitle")}</p>
                <p className="text-[11px] text-text-secondary">
                  {t("settings.debug.captureDesc")}
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
                {capture.active ? t("settings.debug.captureStop", { frames: capture.frames }) : t("settings.debug.captureStart")}
              </button>
            </div>
            {captureMsg && (
              <p className="mt-2.5 break-all rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">{captureMsg}</p>
            )}
            {capture.dir && (
              <p className="mt-1 break-all text-[10px] text-text-muted">{t("settings.debug.captureFolder", { dir: capture.dir })}</p>
            )}
          </div>

          {/* Explicador de rivalidades percebidas (debug/calibração) */}
          <RivalryPerceptionPanel />
            </>
          )}
        </div>

        <div className="flex flex-col items-center gap-4 pt-8">
          <GlassButton
            variant="primary"
            onClick={async () => {
              // Falha de leitura vira `null` e cai no ramo "tem carreira" — levar ao menu
              // é o destino seguro: lá o jogador vê a lista de verdade em vez de ser
              // empurrado para criar uma carreira que talvez já exista.
              const saves = await recarregarSaves();
              if (saves == null || saves.length > 0) {
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
