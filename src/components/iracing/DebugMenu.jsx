import { useState } from "react";
import { useTranslation } from "react-i18next";

import RivalryPerceptionPanel from "./RivalryPerceptionPanel";
import useFerramentasDeDebug from "../../hooks/useFerramentasDeDebug";

// O Menu Debug das Configurações: o interruptor e tudo que ele revela.
//
// Sai do `Settings.jsx` inteiro, sem mudar uma linha do desenho — o componente devolve
// um fragmento e continua vivendo dentro do mesmo painel `glass-strong`, então as
// divisórias `border-t` seguem encadeando como antes.
//
// O interruptor MORA aqui, e é por isso que ele nasce fechado a cada montagem: é estado
// de sessão da tela, não preferência salva. As ferramentas de bancada
// (`useFerramentasDeDebug`) também são chamadas aqui, e de propósito fora do
// `debugMenuOpen` — a hidratação precisa acontecer na montagem para que abrir o menu
// mostre o gravador e a demo do rádio já no estado de verdade, em vez de piscar
// desligados e corrigir depois.
//
// O que continua vindo de fora é só o que a tela COMPARTILHA com o bloco da bandeira
// amarela: o status do `app.ini` e o campo de chat livre, que saem do mesmo
// `useRaceControl`. Chamar o hook de novo aqui dobraria os `invoke` de leitura.
function DebugMenu({ yellowStatus, chatText, setChatText, chatMsg, chatBusy, sendChatTest }) {
  const { t } = useTranslation();
  const [debugMenuOpen, setDebugMenuOpen] = useState(false);
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

  return (
    <>
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
    </>
  );
}

export default DebugMenu;
