import { useTranslation } from "react-i18next";
import { Cog, Gauge, ShieldCheck, Timer, Users } from "lucide-react";

import GarageSheet, { SheetHeader } from "./GarageSheet";
import GarageRow from "./GarageRow";
import MeterBar from "./MeterBar";
import { pitRisk } from "../teamMetrics";
import { carMeterReadings } from "./gridMetrics";

// Um ícone por eixo. Não é enfeite: são três medidores com a mesma forma e a mesma
// régua, e sem uma marca própria eles viram três barras indistinguíveis quando o
// olho passa rápido.
const METER_ICONS = {
  car_level: Cog,
  confiabilidade: ShieldCheck,
  pit_crew_quality: Users,
};

// GRÁFICO 3 — os eixos técnicos com régua.
//
// O v1 mostrava `Nível 1/10`, `56` e `0k` em barras de 0 a 100 sem referência: não
// dava para saber se 56 de confiabilidade era bom. Aqui cada barra ganha a marca da
// média da categoria e uma legenda que diz a posição no grid — o mesmo dado, agora
// com o contexto que o torna acionável.
//
// Os três botões de eixo do v1 sumiram de propósito: eram um seletor de visualização
// (nada no jogo mudava ao clicar) e escondiam duas das três leituras por vez. As
// três cabem juntas.
function CarPanelV2({ team, teams }) {
  const { t } = useTranslation();
  const meters = carMeterReadings(team, teams);
  const strategyRisk = team?.pit_strategy_risk ?? 0;

  return (
    <GarageSheet className="flex h-full flex-col" testId="my-team-v2-car">
      {/* A régua sob o título é o que separa o cabeçalho do primeiro medidor —
          respiro sozinho não se via. O comentário gêmeo morava no GaragePanelV2,
          apagado em 14/08/2026 por não ter consumidor desde que a garagem foi
          desmontada em LineupStrip e DriverCard. */}
      <SheetHeader aside={meters[0]?.average === null ? null : t("myTeamTabV2.car.rulerLegend")}>
        <Timer size={15} strokeWidth={1.8} aria-hidden="true" className="text-accent-primary" />
        {t("myTeamTabV2.car.title")}
      </SheetHeader>

      {/* O bloco ocupa a linha inteira desde que a garagem foi desmontada, então os
          três medidores vão LADO A LADO no desktop: empilhados numa largura dupla,
          cada barra virava um traço de 1000px com um número na ponta, e o card
          terminava com meia tela de vazio embaixo. Empilha de novo abaixo de `lg`. */}
      <div className="flex flex-1 flex-col justify-evenly px-4 py-2 lg:grid lg:grid-cols-3 lg:gap-x-6 lg:divide-x lg:divide-white/[0.08]">
        {meters.map((meter, index) => (
          <div key={meter.key} className={index === 0 ? "" : "lg:pl-6"}>
            <MeterBar
              testId={`car-meter-${meter.key}`}
              Icon={METER_ICONS[meter.key]}
              label={t(`myTeamTabV2.car.meters.${meter.key}`)}
              value={meterValue(t, meter)}
              percent={meter.percent}
              averagePercent={meter.averagePercent}
              tone={meter.tone}
              caption={meterCaption(t, meter)}
              divided={false}
            />
          </div>
        ))}
      </div>

      <div className="border-t border-white/[0.08] px-4 py-1.5">
        <GarageRow
          divided={false}
          label={
            <span className="inline-flex items-center gap-1.5">
              <Gauge size={13} strokeWidth={1.8} aria-hidden="true" className="shrink-0" />
              {t("myTeamTabV2.car.strategyRisk")}
            </span>
          }
          value={pitRisk(strategyRisk)}
        />
      </div>
    </GarageSheet>
  );
}

function meterValue(t, meter) {
  if (meter.key === "car_level") return t("myTeamTabV2.car.levelValue", { level: Math.round(meter.value) });
  return String(Math.round(meter.value));
}

// A legenda diz a POSIÇÃO, não repete o número. "Último do grid" é acionável;
// "56 de 100" não é. Quando o grid inteiro está zerado num eixo — ninguém investiu
// em pit crew ainda —, é isso que ela informa: a vantagem ali está de graça.
function meterCaption(t, meter) {
  if (meter.average === null || meter.gridSize < 2) return null;
  if (meter.average === 0 && meter.value === 0) return t("myTeamTabV2.car.captions.untouched");
  if (meter.rank === 1) return t("myTeamTabV2.car.captions.best");
  if (meter.rank > 0 && meter.rank === meter.gridSize) return t("myTeamTabV2.car.captions.worst");
  if (meter.rank > 0) return t("myTeamTabV2.car.captions.rank", { rank: meter.rank, grid: meter.gridSize });
  return null;
}

export default CarPanelV2;
