import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import TeamLogoMark from "../team/TeamLogoMark";
import { getReadableTeamColor } from "../../utils/teamColors";

// Alarde de contrato expirando. Era uma tarja pequena no topo da coluna de metas,
// que o jogador passava batido justamente na etapa em que a informação decide a
// temporada seguinte. Agora ele para a tela: entra uma vez por etapa (a chave de
// "já vi" mora no useAttentionStore, com o id da corrida), e sai no botão, no
// clique fora ou no Esc.
//
// `yearEnd` é o ANO do calendário, não o número da temporada: o jogador lê a data
// em toda a interface do jogo, e "temporada 33" não é ancoragem nenhuma.
function ContractExpiringModal({ teamName, teamColor, yearEnd, onClose }) {
  const { t } = useTranslation();
  const corDoNome = getReadableTeamColor(teamColor, { fallback: "#e6edf3" });

  useEffect(() => {
    function aoTeclar(evento) {
      if (evento.key === "Escape") onClose?.();
    }

    window.addEventListener("keydown", aoTeclar);
    return () => window.removeEventListener("keydown", aoTeclar);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-[200] flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm animate-fade-in"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="animate-scale-in relative w-full max-w-xl overflow-hidden rounded-3xl border border-status-yellow/40 bg-[#12100a] shadow-[0_30px_90px_rgba(0,0,0,0.75),0_0_60px_rgba(210,153,34,0.18)]"
        onClick={(e) => e.stopPropagation()}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="contrato-expirando-titulo"
      >
        {/* Faixa de alerta no topo */}
        <div className="h-1 w-full bg-gradient-to-r from-transparent via-status-yellow to-transparent" />

        <div className="flex flex-col items-center px-8 py-8 text-center">
          <div className="relative mb-5 flex h-16 w-16 items-center justify-center">
            <span className="absolute inset-0 animate-pulse rounded-full bg-status-yellow/20" />
            <span className="relative flex h-16 w-16 items-center justify-center rounded-full border border-status-yellow/50 bg-status-yellow/10 text-3xl text-status-yellow">
              ⚠
            </span>
          </div>

          <p className="text-[11px] font-bold uppercase tracking-[0.28em] text-status-yellow">
            {t("nextRaceTab.contractAlert.eyebrow")}
          </p>

          <h3
            id="contrato-expirando-titulo"
            className="mt-2 text-3xl font-black uppercase tracking-tight text-text-primary"
          >
            {t("nextRaceTab.labels.contractExpiring")}
          </h3>

          {/* A equipe é o assunto do aviso, então ela aparece na própria cor, com o
              mesmo logo que o jogador vê no resto do app. */}
          <div className="mt-5 flex items-center gap-3">
            <TeamLogoMark
              teamName={teamName}
              color={teamColor}
              size="sm"
              testId="contract-alert-team-logo"
            />
            <span
              className="text-2xl font-black uppercase tracking-tight"
              style={{ color: corDoNome }}
            >
              {teamName}
            </span>
          </div>

          <p className="mt-4 max-w-md text-base leading-relaxed text-text-secondary">
            {t("nextRaceTab.contractAlert.headline")}
          </p>

          <p className="mt-3 max-w-md text-sm leading-relaxed text-text-muted">
            {t("nextRaceTab.contractAlert.body")}
          </p>

          {yearEnd != null && (
            <p className="mt-5 rounded-full border border-white/10 bg-white/5 px-4 py-1.5 text-[11px] font-bold uppercase tracking-[0.18em] text-text-muted">
              {t("nextRaceTab.contractAlert.seasonNote", { year: yearEnd })}
            </p>
          )}

          <button
            type="button"
            onClick={onClose}
            className="mt-7 rounded-xl border border-status-yellow/50 bg-status-yellow/15 px-8 py-3 text-sm font-bold uppercase tracking-[0.14em] text-status-yellow transition hover:bg-status-yellow/25"
          >
            {t("nextRaceTab.contractAlert.action")}
          </button>
        </div>
      </div>
    </div>
  );
}

export default ContractExpiringModal;
