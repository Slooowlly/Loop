import { useTranslation } from "react-i18next";
import { ArrowDownLeft, ArrowUpRight, CalendarRange, Flag, Target, Trophy, Wallet } from "lucide-react";

import GlassCard from "../../../ui/GlassCard";
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
function HorizonStrip({ roundIncome, roundExpenses, roundNet, seasonNetToDate, expectedPrize, projectedAnnual, hasProjection }) {
  const { t } = useTranslation();
  const scale = Math.max(1, roundIncome, roundExpenses);

  return (
    <div className="grid gap-4 lg:grid-cols-2" data-testid="my-team-v2-horizons">
      <GlassCard hover={false} className="rounded-[24px] p-5">
        <p className="flex items-center gap-2 text-[10px] uppercase tracking-[0.22em] text-text-muted">
          <Flag size={19} strokeWidth={1.8} aria-hidden="true" className="text-accent-primary" />
          {t("myTeamTabV2.horizons.round")}
        </p>

        <div className="mt-3 flex items-baseline gap-3">
          <span className={`font-mono text-3xl font-semibold ${roundNet >= 0 ? "text-status-green" : "text-status-red"}`}>
            {formatSignedMoney(roundNet)}
          </span>
          <span className="text-xs text-text-secondary">{t("myTeamTabV2.horizons.result")}</span>
        </div>

        <div className="mt-5 space-y-4">
          <FlowBar
            label={t("myTeamTabV2.horizons.income")}
            value={`+${formatMoney(roundIncome)}`}
            percent={(roundIncome / scale) * 100}
            bar="bg-status-green"
            tone="text-status-green"
            tile="border-status-green/25 bg-status-green/10 text-status-green"
            Icon={ArrowDownLeft}
          />
          <FlowBar
            label={t("myTeamTabV2.horizons.expenses")}
            value={`-${formatMoney(roundExpenses)}`}
            percent={(roundExpenses / scale) * 100}
            bar="bg-status-red"
            tone="text-status-red"
            tile="border-status-red/25 bg-status-red/10 text-status-red"
            Icon={ArrowUpRight}
          />
        </div>
      </GlassCard>

      <GlassCard hover={false} className="rounded-[24px] p-5">
        <p className="flex items-center gap-2 text-[10px] uppercase tracking-[0.22em] text-text-muted">
          <CalendarRange size={19} strokeWidth={1.8} aria-hidden="true" className="text-accent-primary" />
          {t("myTeamTabV2.horizons.season")}
        </p>

        {!hasProjection ? (
          <>
            <div className="mt-3 flex items-baseline gap-3">
              <span className={`font-mono text-3xl font-semibold ${seasonNetToDate >= 0 ? "text-status-green" : "text-status-red"}`}>
                {formatSignedMoney(seasonNetToDate)}
              </span>
              <span className="text-xs text-text-secondary">{t("myTeamTabV2.horizons.toDate")}</span>
            </div>
            <p className="mt-4 text-xs leading-5 text-text-muted">{t("myTeamTabV2.horizons.noProjection")}</p>
          </>
        ) : (
          <>
            <div className="mt-3 flex items-baseline gap-3">
              <span className={`font-mono text-3xl font-semibold ${projectedAnnual >= 0 ? "text-status-green" : "text-status-red"}`}>
                {formatSignedMoney(projectedAnnual)}
              </span>
              <span className="text-xs text-text-secondary">{t("myTeamTabV2.horizons.projection")}</span>
            </div>

            {/* A conta escrita como conta: o jogador vê de onde a projeção sai, em vez
                de três números soltos que ele teria de somar de cabeça. */}
            <div className="mt-5 space-y-2" data-testid="season-equation">
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
          </>
        )}
      </GlassCard>
    </div>
  );
}

// A seta é a direção do dinheiro, não um enfeite: entra pela esquerda, sai pela
// direita — a mesma gramática do Sankey logo abaixo. E, como nos medidores da aba
// Equipe, a placa é o começo da linha: a barra nasce da borda direita dela.
function FlowBar({ label, value, percent, bar, tone, tile, Icon }) {
  return (
    <div className="flex items-start gap-3.5">
      {Icon ? (
        <span className={`grid h-11 w-11 shrink-0 place-items-center rounded-2xl border ${tile}`}>
          <Icon size={24} strokeWidth={1.6} aria-hidden="true" />
        </span>
      ) : null}
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-3">
          <span className="text-sm text-text-secondary">{label}</span>
          <span className={`font-mono text-sm ${tone}`}>{value}</span>
        </div>
        <div className="mt-2 h-2 rounded-full bg-white/10">
          <div className={`h-full rounded-full ${bar}`} style={{ width: `${Math.max(2, Math.min(100, percent))}%` }} />
        </div>
      </div>
    </div>
  );
}

function EquationRow({ operator, label, value, tone, Icon = null, iconTone = "text-text-muted", total = false }) {
  return (
    <div className={`flex items-center gap-2.5 ${total ? "border-t border-white/[0.08] pt-2.5" : ""}`}>
      <span className="w-3 shrink-0 text-center font-mono text-sm text-text-muted">{operator}</span>
      {Icon ? <Icon size={17} strokeWidth={1.7} aria-hidden="true" className={`shrink-0 ${iconTone}`} /> : null}
      <span className="text-sm text-text-secondary">{label}</span>
      <span className={`ml-auto font-mono text-sm ${tone}`}>{value}</span>
    </div>
  );
}

export default HorizonStrip;
