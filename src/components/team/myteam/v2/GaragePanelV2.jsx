import { useTranslation } from "react-i18next";
import { Flame, Megaphone, TriangleAlert, Wallet, Wrench } from "lucide-react";

import GlassCard from "../../../ui/GlassCard";
import MeterBar from "./MeterBar";
import { formatMoney } from "../../../../utils/formatters";
import { formatPercent } from "../teamMetrics";

// O que decorre de ter contratado esta dupla: quanto ela pesa no teto, quanta
// audiência ela traz e como está a política interna.
//
// Folha e presença são medidores SEM régua de propósito: `TeamStanding` não traz
// salários nem presença das outras equipes, e desenhar uma média de grid ali seria
// inventar número. As duas coisas que TÊM referência no payload — habilidade e mídia
// dos pilotos — ganham régua no cartão de cada um.
//
// A presença parava no número: "53.2" não conta o que ela trouxe. O que ela rendeu na
// rodada entra na legenda dela — o resto do contexto da dupla (quanto cada um pesa na
// folha, há quanto tempo está na casa) mora no cartão de cada piloto, ao lado do
// salário e do nome a que pertence.
function GaragePanelV2({ drivers, salaryCeiling, presence, climate, sponsorshipIncome = 0, gateIncome = 0 }) {
  const { t } = useTranslation();
  const payroll = drivers.reduce((sum, driver) => sum + driver.salary, 0);
  const payrollPressure = salaryCeiling > 0 ? (payroll / salaryCeiling) * 100 : 0;
  // O que a presença RENDEU na última rodada. As duas linhas são as que a fama
  // escala no backend (patrocínio e bilheteria/portão); bilheteria vem 0 em linha
  // legada, e aí a frase simplesmente não a cita em vez de exibir "$0".
  const presenceReturn = sponsorshipIncome > 0
    ? gateIncome > 0
      ? t("myTeamTabV2.garage.presenceReturnWithGate", {
          sponsorship: formatMoney(sponsorshipIncome),
          gate: formatMoney(gateIncome),
        })
      : t("myTeamTabV2.garage.presenceReturn", { sponsorship: formatMoney(sponsorshipIncome) })
    : null;

  return (
    <GlassCard hover={false} className="flex h-full flex-col rounded-[24px] p-6" data-testid="my-team-v2-garage">
      {/* O título fecha com uma linha em vez de só respiro: espaço vazio sozinho some
          num card deste tamanho — foram 24px de folga que passaram despercebidos. A
          régua separa o cabeçalho do primeiro medidor de um jeito que se vê. */}
      <p className="flex items-center gap-2 border-b border-white/[0.08] pb-4 text-[10px] uppercase tracking-[0.22em] text-text-muted">
        <Wrench size={19} strokeWidth={1.8} aria-hidden="true" className="text-accent-primary" />
        {t("myTeamTabV2.garage.title")}
      </p>

      {/* Os dois cartões desta linha nascem com contagens diferentes de medidor — três
          aqui, quatro no carro. Sem distribuir a folga, um terminava alto e o outro
          baixo, e a linha inteira parecia desalinhada.
          `evenly`, não `between`: o `between` empilha TODA a folga entre os medidores
          e cola o primeiro no topo — com 100px sobrando davam 104px entre as barras e
          zero acima da primeira. Uniforme, o mesmo respiro vai antes, entre e depois. */}
      <div className="mt-6 flex flex-1 flex-col justify-evenly gap-5">
        <MeterBar
          testId="garage-payroll-meter"
          Icon={Wallet}
          label={t("myTeamTabV2.lineup.payroll")}
          value={t("myTeamTabV2.lineup.payrollValue", { percent: formatPercent(payrollPressure), ceiling: formatMoney(salaryCeiling) })}
          percent={payrollPressure}
          tone={payrollPressure >= 95 ? "bad" : payrollPressure >= 80 ? "warn" : "neutral"}
          caption={t("myTeamTabV2.lineup.payrollCaption")}
        />

        {presence > 0 ? (
          <MeterBar
            testId="garage-presence-meter"
            Icon={Megaphone}
            label={t("myTeamTabV2.lineup.presence")}
            value={presence.toFixed(1)}
            percent={presence}
            caption={[t("myTeamTabV2.lineup.presenceCaption"), presenceReturn].filter(Boolean).join(" ")}
          />
        ) : null}

        <div className="flex items-start gap-3.5 border-t border-white/[0.08] pt-4">
          <span className={`grid h-11 w-11 shrink-0 place-items-center rounded-2xl border ${climate.hurtsMorale ? "border-status-red/25 bg-status-red/10 text-status-red" : climate.tension >= 20 ? "border-status-yellow/25 bg-status-yellow/10 text-status-yellow" : "border-status-green/25 bg-status-green/10 text-status-green"}`}>
            <Flame size={24} strokeWidth={1.6} aria-hidden="true" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm text-text-secondary">{t("myTeamTabV2.lineup.climate")}</span>
              <span className={`text-sm ${climate.tone}`} data-testid="garage-climate">
                {t("myTeamTabV2.lineup.climateValue", { label: climate.label, tension: Math.round(climate.tension) })}
              </span>
            </div>
            <div className="mt-2 h-2 rounded-full bg-white/10">
              <div
                className={`h-full rounded-full ${climate.barTone}`}
                style={{ width: `${Math.max(2, Math.min(100, climate.tension))}%` }}
              />
            </div>
            {climate.hurtsMorale ? (
              <p className="mt-1.5 flex items-start gap-1.5 text-xs leading-5 text-status-red" data-testid="garage-morale-warning">
                <TriangleAlert size={15} strokeWidth={1.8} aria-hidden="true" className="mt-0.5 shrink-0" />
                {t("myTeamTab.garage.moraleWarning")}
              </p>
            ) : (
              <p className="mt-1.5 text-xs leading-5 text-text-muted">{t("myTeamTabV2.garage.tensionCaption")}</p>
            )}
            {/* A inversão é o único evento da política interna que já ACONTECEU — a
                tensão é estado, isto é histórico. Só aparece quando houve alguma:
                "0 inversões" é ruído numa garagem que nunca trocou a ordem. */}
            {climate.inversions > 0 ? (
              <p className="mt-1.5 text-xs leading-5 text-status-yellow" data-testid="garage-inversions">
                {t("myTeamTabV2.garage.inversions", { count: climate.inversions })}
              </p>
            ) : null}
          </div>
        </div>
      </div>
    </GlassCard>
  );
}

export default GaragePanelV2;
