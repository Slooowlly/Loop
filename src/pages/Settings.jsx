import { useState, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import GlassSelect from "../components/ui/GlassSelect";
import GlassButton from "../components/ui/GlassButton";
import LoadingOverlay from "../components/ui/LoadingOverlay";
import ParticleBackdrop from "../components/ui/ParticleBackdrop";
import DebugMenu from "../components/iracing/DebugMenu";
import IracingDesfazerPanel from "../components/iracing/IracingDesfazerPanel";
import IracingDiagnosticoPanel from "../components/iracing/IracingDiagnosticoPanel";
import PttEngenheiroSettings from "../components/iracing/PttEngenheiroSettings";
import { useOverlayFlags } from "../overlay/useOverlayFlags";
import { estaLigada as vozSpotterLigada, falar as falarSpotter, ligar as ligarVozSpotter } from "../lib/spotterVoice";
import { definirVolume, volumeRadio } from "../lib/volumeRadio";
import useConfiguracaoDoApp from "../hooks/useConfiguracaoDoApp";
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
  // Os dois blocos de dados da tela vivem em hooks próprios: config do app e Race Control.
  // As ferramentas de bancada foram junto com o Menu Debug para `DebugMenu`, que é dono
  // do interruptor e do próprio estado. O componente ficou com o desenho e com o que é só
  // dele.
  const {
    config,
    setConfig,
    loading,
    saving,
    errorMessage,
    setErrorMessage,
    falhaAoCarregar,
    loadConfig,
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

  const [navigating, setNavigating] = useState(false);

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

  // O config não veio e a leitura já terminou. Sem isto a tela ficava no fundo vazio para
  // sempre, com o mesmo desenho de "ainda carregando" — e o caminho de volta ao menu, que é
  // um botão do cabeçalho, morria junto. Aqui a falha é dita, o retorno continua de pé e o
  // jogador pode tentar de novo sem fechar o app.
  if (!config && falhaAoCarregar) {
    return (
      <div className="entry-shell !block !h-full px-4 py-12">
        <div className="entry-backdrop" />
        <div className="entry-glow left-[5%] top-[10%] h-80 w-80 bg-blue-500/10" />
        <div className="entry-glow bottom-[5%] right-[5%] h-96 w-96 bg-cyan-500/10" />

        <div className="page-in relative z-10 mx-auto flex max-w-xl flex-col items-center gap-4 pt-24 text-center">
          <p className="text-sm text-status-red" role="alert">
            {t("settings.loadError")}
          </p>
          <div className="flex items-center gap-3">
            <GlassButton onClick={loadConfig} disabled={loading}>
              {t("settings.retry")}
            </GlassButton>
            <button
              onClick={() => navigate(location.state?.from ?? "/menu")}
              className="rounded-xl border border-white/15 px-4 py-2 text-[11px] font-bold uppercase tracking-[0.22em] text-text-secondary transition-glass hover:text-text-primary"
            >
              {t("settings.back")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (!config) {
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

          {/* O caminho de volta dos dois arquivos que o Loop escreve na pasta do iRacing
              sem perguntar. Fica logo abaixo do interruptor da pintura porque os dois são
              a mesma conversa vista de lados opostos: o interruptor impede as próximas,
              este devolve as que já foram escritas. */}
          <IracingDesfazerPanel />

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

          <DebugMenu
            yellowStatus={yellowStatus}
            chatText={chatText}
            setChatText={setChatText}
            chatMsg={chatMsg}
            chatBusy={chatBusy}
            sendChatTest={sendChatTest}
          />
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
