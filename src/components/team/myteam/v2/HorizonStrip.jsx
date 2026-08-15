import { useTranslation } from "react-i18next";
import { ArrowDownLeft, ArrowUpRight, CalendarRange, Flag, Target, Trophy, Wallet } from "lucide-react";

import GarageSheet, { SheetHeader } from "./GarageSheet";
import GarageRow, { GarageRule } from "./GarageRow";
import { formatMoney, formatSignedMoney } from "../../../../utils/formatters";

// Os dois horizontes que sobraram depois que o caixa subiu para o cabeçalho.
//
// Eram TRÊS cards com a mesma forma — três linhas de rótulo e valor cada — e só o
// título os separava, então batiam o olho como se dissessem a mesma coisa. Pior: o
// card "Agora" repetia o caixa do topo e, numa temporada de uma rodada, "Resultado" e
// "Acumulado" mostravam literalmente o mesmo número em cards vizinhos.
//
// O card "Agora" não virou outro card: ele foi absorvido pelo cabeçalho, onde o caixa
// já mora e onde dívida e fôlego são o contexto dele, não linhas independentes.
//
// Os dois que ficaram têm naturezas diferentes e agora têm formas diferentes: a
// rodada é um FLUXO (entrou, saiu, sobrou — desenhado como duas barras opostas), a
// temporada é uma CONTA (o acumulado mais o prêmio dá a projeção — escrita como
// soma). Nenhum dos dois se lê como uma lista.
//
// Os dois verdictos subiram para o cabeçalho da folha, na mesma borda direita em que
// mora o caixa da faixa de comando. Eles ficavam soltos no corpo do card, cada um
// numa altura diferente do outro.
function HorizonStrip({ roundIncome, roundExpenses, roundNet, seasonNetToDate, expectedPrize, projectedAnnual, hasProjection }) {
  const { t } = useTranslation();
  const scale = Math.max(1, roundIncome, roundExpenses);

  return (
    <div className="grid gap-3 lg:grid-cols-2" data-testid="my-team-v2-horizons">
      <GarageSheet>
        <SheetHeader
          aside={t("myTeamTabV2.horizons.result")}
          value={formatSignedMoney(roundNet)}
          valueTone={roundNet >= 0 ? "text-status-green" : "text-status-red"}
        >
          <Flag size={15} strokeWidth={1.8} aria-hidden="true" className="text-accent-primary" />
          {t("myTeamTabV2.horizons.round")}
        </SheetHeader>

        <div className="px-4 py-2">
          <FlowBar
            label={t("myTeamTabV2.horizons.income")}
            value={`+${formatMoney(roundIncome)}`}
            percent={(roundIncome / scale) * 100}
            bar="bg-status-green/80"
            tone="text-status-green"
            Icon={ArrowDownLeft}
          />
          <FlowBar
            label={t("myTeamTabV2.horizons.expenses")}
            value={`-${formatMoney(roundExpenses)}`}
            percent={(roundExpenses / scale) * 100}
            bar="bg-status-red/80"
            tone="text-status-red"
            Icon={ArrowUpRight}
            last
          />
        </div>
      </GarageSheet>

      <GarageSheet>
        <SheetHeader
          aside={hasProjection ? t("myTeamTabV2.horizons.projection") : t("myTeamTabV2.horizons.toDate")}
          value={formatSignedMoney(hasProjection ? projectedAnnual : seasonNetToDate)}
          valueTone={(hasProjection ? projectedAnnual : seasonNetToDate) >= 0 ? "text-status-green" : "text-status-red"}
        >
          <CalendarRange size={15} strokeWidth={1.8} aria-hidden="true" className="text-accent-primary" />
          {t("myTeamTabV2.horizons.season")}
        </SheetHeader>

        {!hasProjection ? (
          <p className="px-4 py-4 text-[11px] leading-5 text-text-muted">{t("myTeamTabV2.horizons.noProjection")}</p>
        ) : (
          /* A conta escrita como conta: o jogador vê de onde a projeção sai, em vez
             de três números soltos que ele teria de somar de cabeça. */
          <div className="px-4 py-2" data-testid="season-equation">
            <EquationRow
              operator=""
              Icon={Wallet}
              label={t("myTeamTabV2.horizons.toDate")}
              value={formatSignedMoney(seasonNetToDate)}
              tone={seasonNetToDate >= 0 ? "text-status-green" : "text-status-red"}
            />
            <EquationRow
              operator="+"
              Icon={Trophy}
              iconTone="text-podium-gold"
              label={t("myTeamTabV2.horizons.prize")}
              value={formatMoney(expectedPrize)}
              tone="text-status-green"
            />
            <EquationRow
              operator="="
              Icon={Target}
              label={t("myTeamTabV2.horizons.projection")}
              value={formatSignedMoney(projectedAnnual)}
              tone={projectedAnnual >= 0 ? "text-status-green" : "text-status-red"}
              total
            />
          </div>
        )}
      </GarageSheet>
    </div>
  );
}

// A seta é a direção do dinheiro, não um enfeite: entra pela esquerda, sai pela
// direita — a mesma gramática do Sankey logo abaixo. Ela perdeu a placa de 44px pelo
// mesmo motivo dos medidores: a placa era o objeto mais pesado de uma linha cujo
// assunto é o número na ponta.
function FlowBar({ label, value, percent, bar, tone, Icon, last = false }) {
  return (
    <GarageRow
      divided={!last}
      label={
        <span className="inline-flex items-center gap-1.5">
          {Icon ? <Icon size={13} strokeWidth={1.8} aria-hidden="true" className="shrink-0" /> : null}
          {label}
        </span>
      }
      value={value}
      valueTone={tone}
    >
      <GarageRule percent={percent} barClass={bar} />
    </GarageRow>
  );
}

// A linha da conta NÃO usa a grade de três colunas das outras: o rótulo aqui é frase
// ("Acumulado até aqui", "Prêmio de construtores") e, espremido nos 116px da coluna
// de rótulo, quebraria em três linhas. Ela não perde o alinhamento por isso — a
// coluna de números da folha é dada pela margem direita do bloco, e é nela que o
// valor encosta, igual ao das linhas com régua.
function EquationRow({ operator, label, value, tone, Icon = null, iconTone = "text-text-muted", total = false }) {
  return (
    <div className={`flex items-center gap-2.5 py-1.5 ${total ? "border-t border-white/[0.08]" : ""}`}>
      <span className="w-2 shrink-0 text-center font-garage text-[11px] text-text-muted">{operator}</span>
      {Icon ? <Icon size={13} strokeWidth={1.7} aria-hidden="true" className={`shrink-0 ${iconTone}`} /> : null}
      <span className="text-[11px] text-text-secondary">{label}</span>
      <span className={`ml-auto whitespace-nowrap font-garage text-[13px] tabular-nums ${tone}`}>{value}</span>
    </div>
  );
}

export default HorizonStrip;
