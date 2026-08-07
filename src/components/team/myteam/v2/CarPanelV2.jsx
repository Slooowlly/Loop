import { useTranslation } from "react-i18next";
import { Cog, Gauge, ShieldCheck, Timer, Users } from "lucide-react";

import GlassCard from "../../../ui/GlassCard";
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
    <GlassCard hover={false} className="flex h-full flex-col rounded-[24px] p-6" data-testid="my-team-v2-car">
      {/* Ver o comentário gêmeo em GaragePanelV2: a régua sob o título é o que separa
          o cabeçalho do primeiro medidor — respiro sozinho não se via. */}
      <div className="flex flex-wrap items-baseline justify-between gap-3 border-b border-white/8 pb-4">
        <p className="flex items-center gap-2 text-[10px] uppercase tracking-[0.22em] text-text-muted">
          <Timer size={19} strokeWidth={1.8} aria-hidden="true" className="text-accent-primary" />
          {t("myTeamTabV2.car.title")}
        </p>
        {meters[0]?.average === null ? null : (
          <p className="text-[10px] uppercase tracking-[0.16em] text-text-muted">{t("myTeamTabV2.car.rulerLegend")}</p>
        )}
      </div>

      {/* O cartão ocupa a linha inteira desde que a garagem foi desmontada, então os
          três medidores vão LADO A LADO no desktop: empilhados numa largura dupla,
          cada barra virava um traço de 1000px com um número na ponta, e o card
          terminava com meia tela de vazio embaixo. Empilha de novo abaixo de `lg`. */}
      <div className="mt-6 flex flex-1 flex-col justify-evenly gap-5 lg:grid lg:grid-cols-3 lg:gap-6">
        {meters.map((meter) => (
          <MeterBar
            key={meter.key}
            testId={`car-meter-${meter.key}`}
            Icon={METER_ICONS[meter.key]}
            label={t(`myTeamTabV2.car.meters.${meter.key}`)}
            value={meterValue(t, meter)}
            percent={meter.percent}
            averagePercent={meter.averagePercent}
            tone={meter.tone}
            caption={meterCaption(t, meter)}
          />
        ))}
      </div>

      <div className="mt-5 flex items-center justify-between gap-3 border-t border-white/8 pt-4">
        <span className="flex items-center gap-3.5 text-sm text-text-secondary">
          <span className="grid h-11 w-11 shrink-0 place-items-center rounded-2xl border border-white/10 bg-white/[0.06] text-text-secondary">
            <Gauge size={24} strokeWidth={1.6} aria-hidden="true" />
          </span>
          {t("myTeamTabV2.car.strategyRisk")}
        </span>
        <span className="text-sm text-text-primary">{pitRisk(strategyRisk)}</span>
      </div>
    </GlassCard>
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
