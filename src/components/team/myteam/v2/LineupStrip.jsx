import { useTranslation } from "react-i18next";
import { Flame, Megaphone, TriangleAlert } from "lucide-react";

import GlassCard from "../../../ui/GlassCard";
import MeterBar from "./MeterBar";
import { formatMoney } from "../../../../utils/formatters";

// O que a DUPLA produz junta — a faixa que fica entre os dois cartões de piloto, que
// é onde ela pertence: nenhuma das duas leituras é de um piloto ou do outro.
//
// A presença pública é uma média ponderada das duas mídias (70% do mais midiático,
// 30% do segundo): não existe "a presença do N1". Cada cartão diz a fatia que aquele
// piloto puxa; o número resultante, e o que ele rendeu de patrocínio e bilheteria na
// rodada, é desta faixa.
//
// O clima é literalmente a relação entre os dois. Ele morava num cartão de garagem que
// dividia espaço com folha salarial — três assuntos empilhados sem parentesco. Aqui ele
// fica ao lado da única coisa que compartilha o sujeito com ele.
//
// Sem régua de propósito: `TeamStanding` não traz presença nem tensão das outras
// equipes, e desenhar uma média de grid aqui seria inventar número.
function LineupStrip({ presence, climate, sponsorshipIncome = 0, gateIncome = 0 }) {
  const { t } = useTranslation();
  // O que a presença RENDEU na última rodada. As duas linhas são as que a fama escala
  // no backend (patrocínio e bilheteria/portão); bilheteria vem 0 em linha legada, e aí
  // a frase simplesmente não a cita em vez de exibir "$0".
  const presenceReturn = sponsorshipIncome > 0
    ? gateIncome > 0
      ? t("myTeamTabV2.bond.presenceReturnWithGate", {
          sponsorship: formatMoney(sponsorshipIncome),
          gate: formatMoney(gateIncome),
        })
      : t("myTeamTabV2.bond.presenceReturn", { sponsorship: formatMoney(sponsorshipIncome) })
    : null;

  return (
    <GlassCard hover={false} className="rounded-[24px] p-6" data-testid="my-team-v2-bond">
      <div className="grid gap-6 lg:grid-cols-2">
        {presence > 0 ? (
          <MeterBar
            testId="bond-presence-meter"
            Icon={Megaphone}
            label={t("myTeamTabV2.lineup.presence")}
            value={presence.toFixed(1)}
            percent={presence}
            caption={[t("myTeamTabV2.lineup.presenceCaption"), presenceReturn].filter(Boolean).join(" ")}
          />
        ) : null}

        <div className="flex items-start gap-3.5">
          <span className={`grid h-11 w-11 shrink-0 place-items-center rounded-2xl border ${climate.hurtsMorale ? "border-status-red/25 bg-status-red/10 text-status-red" : climate.tension >= 20 ? "border-status-yellow/25 bg-status-yellow/10 text-status-yellow" : "border-status-green/25 bg-status-green/10 text-status-green"}`}>
            <Flame size={24} strokeWidth={1.6} aria-hidden="true" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm text-text-secondary">{t("myTeamTabV2.lineup.climate")}</span>
              <span className={`text-sm ${climate.tone}`} data-testid="bond-climate">
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
              <p className="mt-1.5 flex items-start gap-1.5 text-xs leading-5 text-status-red" data-testid="bond-morale-warning">
                <TriangleAlert size={15} strokeWidth={1.8} aria-hidden="true" className="mt-0.5 shrink-0" />
                {t("myTeamTab.garage.moraleWarning")}
              </p>
            ) : (
              <p className="mt-1.5 text-xs leading-5 text-text-muted">{t("myTeamTabV2.bond.tensionCaption")}</p>
            )}
            {/* A inversão é o único evento da política interna que já ACONTECEU — a
                tensão é estado, isto é histórico. Só aparece quando houve alguma:
                "0 inversões" é ruído numa dupla que nunca trocou de ordem. */}
            {climate.inversions > 0 ? (
              <p className="mt-1.5 text-xs leading-5 text-status-yellow" data-testid="bond-inversions">
                {t("myTeamTabV2.bond.inversions", { count: climate.inversions })}
              </p>
            ) : null}
          </div>
        </div>
      </div>
    </GlassCard>
  );
}

export default LineupStrip;
