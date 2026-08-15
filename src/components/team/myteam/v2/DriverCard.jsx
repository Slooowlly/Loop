import { useTranslation } from "react-i18next";
import { Gauge, HeartPulse, Medal, Megaphone, Sparkles, Star, Trophy } from "lucide-react";

import GarageSheet from "./GarageSheet";
import GarageRow from "./GarageRow";
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
//
// O salário desceu do canto superior direito para dentro da tabela de linhas. Lá em
// cima ele era um número solto numa coluna própria, e a fatia da folha morava a três
// linhas de distância do único número que a explica, o custo por ponto. Agora os
// quatro — habilidade, mídia, salário e custo por ponto — caem na mesma coluna, e a
// pergunta "ele devolve o que custa?" se lê de cima para baixo.
// `payroll` é a folha dos DOIS contratos: sem ela, os dois salários lado a lado só
// viram "a dupla é equilibrada" depois de uma divisão feita de cabeça.
function DriverCard({ driver, averages, hasGrid, poolSize, payroll = 0, teammateMedia = null }) {
  const { t } = useTranslation();
  const payrollShare = payroll > 0 && driver.salary > 0 ? (driver.salary / payroll) * 100 : null;
  const tenure = tenureLabel(t, driver.tenureSeasons);

  return (
    <GarageSheet testId={`driver-card-${driver.role}`}>
      <div className="flex items-start justify-between gap-3 border-b border-white/[0.08] px-4 py-3">
        <div className="min-w-0">
          <p className="font-garage text-[10px] uppercase tracking-[0.22em] text-text-muted">{driver.role}</p>
          <h4 className={`mt-1 truncate text-lg font-semibold ${driver.highlight ? "text-accent-primary" : "text-text-primary"}`}>
            {driver.name}
          </h4>
          {/* Os selos ficam NESTA linha, não numa própria: como só um dos dois pilotos
              costuma ser estreante ou estar lesionado, uma linha condicional
              desalinhava os dois cartões inteiros — medidores, estatísticas e rodapé
              desciam num lado e não no outro. A altura mínima trava o resto. */}
          <div className="mt-2 flex min-h-[22px] flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-text-secondary">
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
        </div>
        {/* A posição no campeonato é a única leitura desta faixa que responde "ele
            está indo bem?" — vale um número grande, não uma linha de metadados. Ela
            fica no alto à direita, na mesma coluna do caixa da faixa de comando: é o
            veredito de cada bloco, sempre no mesmo canto. A altura mínima segura o
            alinhamento quando ainda não há posição. */}
        <div className="flex min-h-[40px] shrink-0 items-baseline gap-2 text-right" data-testid={`driver-championship-${driver.role}`}>
          {driver.championshipPosition > 0 ? (
            <>
              <span className="font-garage text-[32px] font-semibold leading-none tabular-nums text-text-primary">
                {formatOrdinal(driver.championshipPosition)}
              </span>
              <span className="max-w-[74px] text-[10px] uppercase leading-3 tracking-[0.16em] text-text-muted">
                {t("myTeamTabV2.lineup.championshipLabel")}
              </span>
            </>
          ) : (
            <span className="text-[11px] text-text-muted">{t("myTeamTabV2.lineup.championshipPending")}</span>
          )}
        </div>
      </div>

      {!driver.hasDetail ? (
        <p className="px-4 py-6 text-center text-[11px] leading-5 text-text-secondary">
          {t("myTeamTabV2.lineup.noDetail")}
        </p>
      ) : (
        <>
          <div className="px-4 py-2">
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
            {/* O peso dele na folha, colado no número que ele contextualiza. */}
            <GarageRow label={t("myTeamTabV2.lineup.salary")} value={formatMoney(driver.salary)}>
              {payrollShare === null ? null : (
                <span data-testid={`driver-payroll-share-${driver.role}`}>
                  {t("myTeamTabV2.lineup.payrollShare", { percent: formatPercent(payrollShare) })}
                </span>
              )}
            </GarageRow>
            {/* A leitura de gestão: quanto custa cada ponto que ele traz. Sem pontos
                ainda, a conta não existe — e mostrar um número aqui seria inventar. */}
            {/* Sem ícone, ao contrário dos dois medidores acima: lá a marca separa
                barras de forma idêntica, aqui não há barra nenhuma para confundir, e
                o ícone só empurraria o rótulo para uma segunda linha. */}
            <GarageRow
              label={t("myTeamTabV2.lineup.costPerPoint")}
              value={driver.costPerPoint === null ? t("myTeamTabV2.lineup.noPoints") : formatMoney(driver.costPerPoint)}
              divided={false}
            />
          </div>

          <div className="grid grid-cols-3 divide-x divide-white/[0.08] border-t border-white/[0.08]">
            <Stat label={t("myTeamTabV2.lineup.points")} value={driver.pontos} Icon={Star} tone="text-accent-primary" />
            <Stat label={t("myTeamTabV2.lineup.wins")} value={driver.vitorias} Icon={Trophy} tone="text-podium-gold" />
            <Stat label={t("myTeamTabV2.lineup.podiums")} value={driver.podios} Icon={Medal} tone="text-podium-silver" />
          </div>
        </>
      )}
    </GarageSheet>
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

// O rodapé de campanha: rótulo em cima, número embaixo, alinhados à esquerda de cada
// terço. Eram três caixas centralizadas de canto redondo com um ícone grande no topo,
// e o ícone era o objeto mais visível de um bloco cujo assunto é o número.
function Stat({ label, value, Icon, tone = "text-text-muted" }) {
  return (
    <div className="px-4 py-2.5">
      <p className="flex items-center gap-1.5 text-[9px] uppercase tracking-[0.18em] text-text-muted">
        {Icon ? <Icon size={12} strokeWidth={1.8} aria-hidden="true" className={tone} /> : null}
        {label}
      </p>
      <p className="mt-1 font-garage text-[17px] font-semibold tabular-nums text-text-primary">{value}</p>
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
