/**
 * Tela "o que mudou" entre versões — e o passo onde a atualização é CONFIRMADA.
 *
 * O jogador clica em atualizar no UpdateGate, cai aqui, lê o que muda e só então
 * confirma. É deliberado: as notas costumam trazer coisas que mudam números na
 * tela (salário anual, correção de estatística) e, escondidas atrás de um link
 * opcional, quase ninguém as lia — a dúvida chegava depois, como suporte.
 *
 * Fora do fluxo de atualização (status ≠ "available") vira só leitura, com Fechar.
 *
 * O conteúdo vem do campo `notes` do manifesto (update.body) — é o que você
 * escreve ao publicar. Renderiza como linhas simples (1 item por linha).
 */
import { useTranslation } from "react-i18next";
import { useUpdater } from "./UpdaterProvider";

export default function UpdateChangelogModal() {
  const { t } = useTranslation();
  const { update, status, showChangelog, closeChangelog, install, dismiss } =
    useUpdater();
  if (!showChangelog) return null;

  const confirming = status === "available";

  function confirm() {
    // Fecha ANTES de instalar: a barra de progresso vive no UpdateGate, atrás
    // desta tela. Sem isso o download roda escondido.
    closeChangelog();
    install();
  }

  function postpone() {
    closeChangelog();
    dismiss();
  }

  const notes = (update?.body ?? "").trim();
  // Cada linha do `notes` vira um item; tira marcadores comuns (-, *, •).
  const lines = notes
    .split(/\r?\n/)
    .map((l) => l.replace(/^\s*[-*•]\s*/, "").trim())
    .filter(Boolean);

  return (
    <div className="fixed inset-0 z-[140] flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={closeChangelog}
      />
      <div className="glass-strong relative max-h-[70vh] w-[420px] overflow-y-auto rounded-2xl border border-white/12 p-6 shadow-2xl">
        <h3 className="mb-1 text-[15px] font-semibold text-text-primary">
          {t("updater.changelogTitle")}
        </h3>
        <p className="mb-4 text-[12px] uppercase tracking-wide text-text-secondary">
          {t("updater.versionLabel", { version: update?.version })}
        </p>

        {lines.length ? (
          <ul className="mb-5 flex flex-col gap-2">
            {lines.map((line, i) => (
              <li
                key={i}
                className="flex gap-2 text-[13px] leading-relaxed text-text-secondary"
              >
                <span className="mt-1.5 h-1 w-1 shrink-0 rounded-full bg-accent-primary" />
                <span>{line}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="mb-5 text-[13px] text-text-secondary">
            {t("updater.noNotes")}
          </p>
        )}

        {confirming ? (
          <div className="flex flex-col gap-2">
            <button
              type="button"
              onClick={confirm}
              className="flex h-9 w-full items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90"
            >
              {t("updater.confirmInstall")}
            </button>
            <button
              type="button"
              onClick={postpone}
              className="flex h-9 w-full items-center justify-center rounded-xl text-[13px] text-text-secondary transition-colors hover:text-text-primary"
            >
              {t("updater.later")}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={closeChangelog}
            className="flex h-9 w-full items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90"
          >
            {t("updater.close")}
          </button>
        )}
      </div>
    </div>
  );
}
