import { useTranslation } from "react-i18next";
import { Flame, Megaphone, TriangleAlert } from "lucide-react";

import GarageSheet from "./GarageSheet";
import GarageRow, { GarageRule } from "./GarageRow";
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
    <GarageSheet testId="my-team-v2-bond">
      {/* Filete vertical no lugar do respiro de 24px: as duas leituras são irmãs e
          precisam ler como duas colunas da mesma folha, não como dois blocos que
          por acaso caíram lado a lado. */}
      <div className="grid gap-x-6 px-4 py-2 lg:grid-cols-2 lg:divide-x lg:divide-white/[0.08]">
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

        <div className="lg:pl-6">
          <GarageRow
            divided={false}
            label={
              <span className="inline-flex items-center gap-1.5">
                <Flame size={13} strokeWidth={1.8} aria-hidden="true" className="shrink-0" />
                {t("myTeamTabV2.lineup.climate")}
              </span>
            }
            value={
              <span data-testid="bond-climate">
                {t("myTeamTabV2.lineup.climateValue", { label: climate.label, tension: Math.round(climate.tension) })}
              </span>
            }
            valueTone={climate.tone}
            caption={
              <>
                {climate.hurtsMorale ? (
                  <p className="flex items-start gap-1.5 text-status-red" data-testid="bond-morale-warning">
                    <TriangleAlert size={13} strokeWidth={1.8} aria-hidden="true" className="mt-0.5 shrink-0" />
                    {t("myTeamTab.garage.moraleWarning")}
                  </p>
                ) : (
                  <p>{t("myTeamTabV2.bond.tensionCaption")}</p>
                )}
                {/* A inversão é o único evento da política interna que já ACONTECEU — a
                    tensão é estado, isto é histórico. Só aparece quando houve alguma:
                    "0 inversões" é ruído numa dupla que nunca trocou de ordem. */}
                {climate.inversions > 0 ? (
                  <p className="text-status-yellow" data-testid="bond-inversions">
                    {t("myTeamTabV2.bond.inversions", { count: climate.inversions })}
                  </p>
                ) : null}
              </>
            }
          >
            <GarageRule percent={climate.tension} barClass={climate.barTone} />
          </GarageRow>
        </div>
      </div>
    </GarageSheet>
  );
}

export default LineupStrip;
