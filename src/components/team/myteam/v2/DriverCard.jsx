import { useTranslation } from "react-i18next";
import { Banknote, Gauge, HeartPulse, Medal, Megaphone, Sparkles, Star, Trophy } from "lucide-react";

import GlassCard from "../../../ui/GlassCard";
import FlagIcon from "../../../ui/FlagIcon";
import MeterBar from "./MeterBar";
import { formatMoney } from "../../../../utils/formatters";
import { formatOrdinal, formatPercent } from "../teamMetrics";

// O dossiê de um piloto da dupla.
//
// A aba de gestão mostrava nome, bandeira e salário — e com isso não respondia a
// pergunta que ela existe para responder: *esse piloto é bom para esta categoria, e
// ele devolve o que custa?* Habilidade e mídia agora vêm com a régua da média da
// categoria, do mesmo jeito que o painel do carro faz contra o grid de equipes.
//
// Mídia fica ao lado de habilidade de propósito: é ela que alimenta a presença
// pública da equipe, que multiplica o patrocínio de cada rodada. Contratar um rosto
// conhecido é receita, não vitrine — e essa cadeia só fica visível se os dois números
// estiverem no mesmo cartão.
// `payroll` é a folha dos DOIS contratos: sem ela, os dois salários lado a lado só
// viram "a dupla é equilibrada" depois de uma divisão feita de cabeça.
function DriverCard({ driver, averages, hasGrid, poolSize, payroll = 0, teammateMedia = null }) {
  const { t } = useTranslation();
  const payrollShare = payroll > 0 && driver.salary > 0 ? (driver.salary / payroll) * 100 : null;
  const tenure = tenureLabel(t, driver.tenureSeasons);

  return (
    <GlassCard hover={false} className="rounded-[24px] p-5" data-testid={`driver-card-${driver.role}`}>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[10px] uppercase tracking-[0.22em] text-text-muted">{driver.role}</p>
          <h4 className={`mt-1 truncate text-lg font-semibold ${driver.highlight ? "text-accent-primary" : "text-text-primary"}`}>
            {driver.name}
          </h4>
          {/* Os selos ficam NESTA linha, não numa própria: como só um dos dois pilotos
              costuma ser estreante ou estar lesionado, uma linha condicional
              desalinhava os dois cartões inteiros — medidores, estatísticas e rodapé
              desciam num lado e não no outro. A altura mínima trava o resto. */}
          <div className="mt-2 flex min-h-[24px] flex-wrap items-center gap-x-3 gap-y-1 text-xs text-text-secondary">
            <span className="inline-flex items-center gap-1.5">
              <FlagIcon nacionalidade={driver.nationality} className="shrink-0" />
              {driver.nationalityLabel}
            </span>
            {driver.age > 0 ? <span>{t("myTeamTabV2.lineup.age", { count: driver.age })}</span> : null}
            {/* Tempo de casa: quantas temporadas SEGUIDAS ele ocupa este assento. Fica
                na faixa de metadados porque é do piloto — nacionalidade, idade e
                quanto tempo ele está aqui respondem juntas "quem é esse". */}
            {tenure ? <span data-testid={`driver-tenure-${driver.role}`}>{tenure}</span> : null}
            {driver.isRookie ? (
              <Badge tone="border-accent-primary/30 text-accent-primary" Icon={Sparkles}>
                {t("myTeamTabV2.lineup.rookie")}
              </Badge>
            ) : null}
            {driver.injury ? (
              <Badge tone="border-status-red/30 text-status-red" Icon={HeartPulse}>
                {t("myTeamTabV2.lineup.injured")}
              </Badge>
            ) : null}
          </div>
          {/* A posição no campeonato é a única leitura desta faixa que responde "ele
              está indo bem?" — vale um número grande, não uma linha de metadados. A
              altura mínima segura o alinhamento quando ainda não há posição. */}
          <div className="mt-3 flex min-h-[38px] items-baseline gap-2.5" data-testid={`driver-championship-${driver.role}`}>
            {driver.championshipPosition > 0 ? (
              <>
                <span className="font-mono text-[34px] font-semibold leading-none text-text-primary">
                  {formatOrdinal(driver.championshipPosition)}
                </span>
                <span className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  {t("myTeamTabV2.lineup.championshipLabel")}
                </span>
              </>
            ) : (
              <span className="text-xs text-text-muted">{t("myTeamTabV2.lineup.championshipPending")}</span>
            )}
          </div>
        </div>
        <div className="shrink-0 text-right">
          <p className="font-mono text-sm text-text-primary">{formatMoney(driver.salary)}</p>
          <p className="mt-1 text-[10px] uppercase tracking-[0.16em] text-text-muted">{t("myTeamTabV2.lineup.salary")}</p>
          {/* O peso dele na folha, colado no número que ele contextualiza. */}
          {payrollShare === null ? null : (
            <p className="mt-2 text-xs text-text-secondary" data-testid={`driver-payroll-share-${driver.role}`}>
              {t("myTeamTabV2.lineup.payrollShare", { percent: formatPercent(payrollShare) })}
            </p>
          )}
        </div>
      </div>

      {!driver.hasDetail ? (
        <p className="mt-5 rounded-2xl border border-white/8 bg-black/10 px-4 py-5 text-center text-xs leading-5 text-text-secondary">
          {t("myTeamTabV2.lineup.noDetail")}
        </p>
      ) : (
        <>
          <div className="mt-5 space-y-4">
            <MeterBar
              testId={`driver-skill-${driver.role}`}
              Icon={Gauge}
              label={t("myTeamTabV2.lineup.skill")}
              value={String(Math.round(driver.skill))}
              percent={driver.skill}
              averagePercent={hasGrid ? averages?.skill : null}
              tone={toneAgainst(driver.skill, hasGrid ? averages?.skill : null)}
              caption={rankCaption(t, driver.skillRank, poolSize)}
            />
            <MeterBar
              testId={`driver-media-${driver.role}`}
              Icon={Megaphone}
              label={t("myTeamTabV2.lineup.media")}
              value={String(Math.round(driver.midia))}
              percent={driver.midia}
              averagePercent={hasGrid ? averages?.midia : null}
              tone={toneAgainst(driver.midia, hasGrid ? averages?.midia : null)}
              caption={mediaCaption(t, driver, teammateMedia)}
            />
          </div>

          <div className="mt-5 grid grid-cols-3 gap-2 border-t border-white/8 pt-4">
            <Stat label={t("myTeamTabV2.lineup.points")} value={driver.pontos} Icon={Star} tone="text-accent-primary" />
            <Stat label={t("myTeamTabV2.lineup.wins")} value={driver.vitorias} Icon={Trophy} tone="text-podium-gold" />
            <Stat label={t("myTeamTabV2.lineup.podiums")} value={driver.podios} Icon={Medal} tone="text-podium-silver" />
          </div>

          {/* A leitura de gestão: quanto custa cada ponto que ele traz. Sem pontos
              ainda, a conta não existe — e mostrar um número aqui seria inventar. */}
          <div className="mt-4 flex items-center justify-between gap-3 border-t border-white/8 pt-3">
            <span className="flex items-center gap-3.5 text-sm text-text-secondary">
              <span className="grid h-11 w-11 shrink-0 place-items-center rounded-2xl border border-white/10 bg-white/[0.06] text-text-secondary">
                <Banknote size={24} strokeWidth={1.6} aria-hidden="true" />
              </span>
              {t("myTeamTabV2.lineup.costPerPoint")}
            </span>
            <span className="font-mono text-sm text-text-primary">
              {driver.costPerPoint === null ? t("myTeamTabV2.lineup.noPoints") : formatMoney(driver.costPerPoint)}
            </span>
          </div>
        </>
      )}
    </GlassCard>
  );
}

function Badge({ children, tone, Icon }) {
  return (
    <span className={`inline-flex items-center gap-1 rounded-full border bg-black/20 px-2 py-0.5 text-[11px] ${tone}`}>
      {Icon ? <Icon size={14} strokeWidth={1.8} aria-hidden="true" /> : null}
      {children}
    </span>
  );
}

function Stat({ label, value, Icon, tone = "text-text-muted" }) {
  return (
    <div className="rounded-2xl border border-white/8 bg-black/10 p-3 text-center">
      {Icon ? <Icon size={20} strokeWidth={1.6} aria-hidden="true" className={`mx-auto ${tone}`} /> : null}
      <p className="mt-2 font-mono text-lg font-semibold text-text-primary">{value}</p>
      <p className="mt-1 text-[10px] uppercase tracking-[0.16em] text-text-muted">
        {label}
      </p>
    </div>
  );
}

function toneAgainst(value, average) {
  if (average === null || average === undefined) return "neutral";
  if (value >= average * 1.05) return "good";
  if (value >= average * 0.9) return "warn";
  return "bad";
}

// A presença pública da equipe é `mais midiático × 0.7 + segundo × 0.3` — não existe
// "a presença dele". O que existe é a FATIA que ele puxa, e é isso que a legenda diz:
// quem tem a mídia maior é o rosto da equipe, o outro entra subordinado. Sem a mídia do
// companheiro (assento vazio, save sem detalhe) a legenda volta à frase genérica.
function mediaCaption(t, driver, teammateMedia) {
  if (teammateMedia === null || teammateMedia === undefined) return t("myTeamTabV2.lineup.mediaCaption");
  return driver.midia >= teammateMedia
    ? t("myTeamTabV2.lineup.presenceShareTop")
    : t("myTeamTabV2.lineup.presenceShareSecond");
}

// `tenure_seasons` conta temporadas CONSECUTIVAS neste assento, incluindo a corrente —
// 1 é quem chegou nesta virada, não um veterano de uma temporada.
function tenureLabel(t, seasons) {
  const value = Number(seasons);
  if (!Number.isFinite(value) || value <= 0) return null;
  if (value === 1) return t("myTeamTabV2.lineup.tenureFirst");
  return t("myTeamTabV2.lineup.tenureSeasons", { count: value });
}

function rankCaption(t, rank, poolSize) {
  if (!(rank > 0) || !(poolSize > 1)) return null;
  if (rank === 1) return t("myTeamTabV2.lineup.bestOfCategory");
  return t("myTeamTabV2.lineup.rankInCategory", { rank: formatOrdinal(rank), pool: poolSize });
}

export default DriverCard;
