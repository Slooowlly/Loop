import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

// Painel de diagnóstico da conexão com o iRacing.
//
// Fica FORA do menu de debug de propósito: quem mais precisa dele é o jogador
// que abriu o app, viu tudo zerado e não sabe o que reportar. Um botão, um
// veredito em português e o caminho do arquivo de log para anexar.

// Cor e tom por veredito. `neutro` é para os estados que NÃO são erro (sim
// fechado, só a UI aberta) — pintá-los de vermelho treinaria o jogador a
// ignorar o painel justamente quando ele estiver certo.
const TOM = {
  ok: "text-status-green",
  sim_fechado: "text-text-secondary",
  so_launcher: "text-text-secondary",
  sessao_nao_pronta: "text-status-yellow",
  acesso_negado: "text-status-red",
  sim_sem_sdk: "text-status-red",
  nao_suportado: "text-text-muted",
};

function IracingDiagnosticoPanel() {
  const { t } = useTranslation();
  const [diag, setDiag] = useState(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  // Envio ao desenvolvedor: nota opcional do jogador e o ticket devolvido.
  const [nota, setNota] = useState("");
  const [enviando, setEnviando] = useState(false);
  const [ticket, setTicket] = useState("");

  async function verificar() {
    setBusy(true);
    setMsg("");
    try {
      setDiag(await invoke("iracing_diagnostico"));
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function revelarLog() {
    try {
      await invoke("iracing_log_revelar");
    } catch (e) {
      setMsg(String(e));
    }
  }

  // Envio SEMPRE por clique — nada sai da máquina sozinho.
  async function enviarLog() {
    setEnviando(true);
    setMsg("");
    setTicket("");
    try {
      setTicket(await invoke("iracing_log_enviar", { nota: nota.trim() || null }));
    } catch (e) {
      setMsg(String(e));
    } finally {
      setEnviando(false);
    }
  }

  async function copiarLog() {
    try {
      const texto = await invoke("iracing_log_ler");
      await navigator.clipboard.writeText(texto);
      setMsg(t("settings.iracingDiag.logCopied"));
    } catch (e) {
      setMsg(String(e));
    }
  }

  const veredito = diag?.veredito;

  return (
    <div className="border-t border-white/10 px-5 py-3.5">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 pr-4">
          <p className="text-[13px] font-medium text-text-primary">
            {t("settings.iracingDiag.title")}
          </p>
          <p className="text-[11px] text-text-secondary">{t("settings.iracingDiag.desc")}</p>
        </div>
        <button
          type="button"
          onClick={verificar}
          disabled={busy}
          className={`shrink-0 rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
            busy
              ? "cursor-default bg-white/5 text-text-muted"
              : "cursor-pointer bg-white/10 text-text-primary hover:bg-white/20"
          }`}
        >
          {busy ? t("settings.iracingDiag.checking") : t("settings.iracingDiag.check")}
        </button>
      </div>

      {veredito && (
        <div className="mt-3 rounded-lg border border-white/10 bg-white/5 px-3 py-2.5">
          <p className={`text-[12px] font-semibold ${TOM[veredito] || "text-text-primary"}`}>
            {t(`settings.iracingDiag.veredito.${veredito}`)}
          </p>
          <p className="mt-1 text-[11px] leading-snug text-text-secondary">
            {t(`settings.iracingDiag.acao.${veredito}`)}
          </p>

          {/* Detalhes crus — o que se pede num relato de bug. */}
          <details className="group mt-2">
            <summary className="flex cursor-pointer list-none items-center gap-1 text-[10px] font-semibold uppercase tracking-[0.12em] text-text-muted transition-glass hover:text-text-secondary [&::-webkit-details-marker]:hidden">
              {t("settings.iracingDiag.details")}
              <span className="transition-transform group-open:rotate-90">›</span>
            </summary>
            <div className="mt-1.5 space-y-0.5 font-mono text-[10px] text-text-muted">
              <p>
                {t("settings.iracingDiag.fieldMemory")}: {String(diag.memoria_ok)}
                {diag.memoria_erro ? ` (${t("settings.iracingDiag.fieldError")} ${diag.memoria_erro})` : ""}
                {diag.memoria_nome ? ` — ${diag.memoria_nome}` : ""}
              </p>
              <p>
                {t("settings.iracingDiag.fieldWindow")}: {String(diag.janela_encontrada)} /{" "}
                {t("settings.iracingDiag.fieldSimulator")}: {String(diag.janela_simulador)}
              </p>
              <p>
                status: {diag.status ?? "--"} / vars: {diag.num_vars ?? "--"} / yaml:{" "}
                {diag.session_info_len ?? "--"}
              </p>
              <p>
                {t("settings.iracingDiag.fieldTicks")}: {diag.ticks_observados} /{" "}
                {t("settings.iracingDiag.fieldElevated")}: {String(diag.elevado)}
              </p>
              {diag.log_caminho && <p className="truncate">{diag.log_caminho}</p>}
            </div>
          </details>
        </div>
      )}

      {/* Envio ao desenvolvedor — o caminho curto entre "está estranho aqui" e
          um relato com log. A nota é opcional mas vale ouro: descrever o que
          estava acontecendo é o que o log não conta. */}
      <div className="mt-3 rounded-lg border border-white/10 bg-white/5 px-3 py-2.5">
        <p className="text-[12px] font-medium text-text-primary">
          {t("settings.iracingDiag.sendTitle")}
        </p>
        <p className="mt-0.5 text-[10px] leading-snug text-text-muted">
          {t("settings.iracingDiag.sendPrivacy")}
        </p>
        <div className="mt-2 flex items-center gap-2">
          <input
            type="text"
            value={nota}
            onChange={(e) => setNota(e.target.value)}
            maxLength={300}
            placeholder={t("settings.iracingDiag.sendNotePlaceholder")}
            className="min-w-0 flex-1 rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-[12px] text-text-primary outline-none transition-glass focus:border-white/25"
          />
          <button
            type="button"
            onClick={enviarLog}
            disabled={enviando}
            className={`shrink-0 rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
              enviando
                ? "cursor-default bg-white/5 text-text-muted"
                : "cursor-pointer bg-status-green/20 text-text-primary hover:bg-status-green/30"
            }`}
          >
            {enviando ? t("settings.iracingDiag.sending") : t("settings.iracingDiag.send")}
          </button>
        </div>
        {ticket && (
          <p className="mt-2 text-[11px] text-status-green">
            {t("settings.iracingDiag.sentTicket")}{" "}
            <span className="font-mono font-bold tracking-widest">{ticket}</span>
          </p>
        )}
      </div>

      <div className="mt-2.5 flex items-center gap-2">
        <button
          type="button"
          onClick={revelarLog}
          className="cursor-pointer rounded-lg bg-white/5 px-3 py-1.5 text-[11px] font-semibold text-text-secondary transition-glass hover:bg-white/10 hover:text-text-primary"
        >
          {t("settings.iracingDiag.openLog")}
        </button>
        <button
          type="button"
          onClick={copiarLog}
          className="cursor-pointer rounded-lg bg-white/5 px-3 py-1.5 text-[11px] font-semibold text-text-secondary transition-glass hover:bg-white/10 hover:text-text-primary"
        >
          {t("settings.iracingDiag.copyLog")}
        </button>
      </div>

      {msg && (
        <p className="mt-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">
          {msg}
        </p>
      )}
    </div>
  );
}

export default IracingDiagnosticoPanel;
