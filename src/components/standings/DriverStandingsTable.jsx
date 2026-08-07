import { Fragment } from "react";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import RivalMarker from "../driver/RivalMarker";
import TeamLogoMarkShared from "../team/TeamLogoMark";
import FlagIcon from "../ui/FlagIcon";
import Tooltip from "../ui/Tooltip";
import ResultBadge from "./ResultBadge";
import { SpecialClassHeader } from "./SpecialStandingNotices";
import { getReadableTeamColor, podiumClass } from "./standingsFormatting";

const SEVERE_INJURY_TYPES = new Set(["Grave", "Critica"]);

// Piso da coluna de rodada: o selo do ResultBadge tem 40px (`w-10`) mais os 2px de
// respiro de cada lado da célula.
const RODADA_MIN_PX = 44;

// Colunas de identidade (44+28+42+180+84) e a de pontos, que fecha a tabela.
const IDENTIDADE_PX = 378;
const PONTOS_PX = 66;
const COLUNAS_FIXAS_PX = IDENTIDADE_PX + PONTOS_PX;

// Quantas rodadas ficam na tela antes de valer a pena rolar. Até 12 o calendário
// encolhe para caber inteiro, pontos inclusive. Passando disso — só a GT3, com 14 —
// as colunas param de espremer e as últimas ficam para o arrasto: 14 selos lado a
// lado deixariam a tabela uma parede de badges.
const RODADAS_VISIVEIS_MAX = 12;

// No regime do arrasto a largura sai da conta inversa: a coluna de rodada é o que
// faz exatamente RODADAS_VISIVEIS_MAX delas caberem ao lado da identidade, e a
// tabela é essa coluna vezes o calendário inteiro. Os <col> das rodadas ficam sem
// `width` de propósito — o `table-fixed` divide entre elas o que sobra, o que
// devolve exatamente essa mesma largura.
function larguraDaTabela(totalRodadas) {
  if (totalRodadas <= RODADAS_VISIVEIS_MAX) {
    return "100%";
  }
  return `calc(${COLUNAS_FIXAS_PX}px + ${totalRodadas} * (100% - ${IDENTIDADE_PX}px) / ${RODADAS_VISIVEIS_MAX})`;
}

function injuryStatusMarker(injuryType) {
  if (!injuryType) {
    return null;
  }

  if (SEVERE_INJURY_TYPES.has(injuryType)) {
    return { emoji: "🚑", title: i18n.t("standings.badge.injurySevere") };
  }

  return { emoji: "🩸", title: i18n.t("standings.badge.injuryActive") };
}

export function DriverStatusMarkers({ driver }) {
  const injuryMarker = injuryStatusMarker(driver.lesao_ativa_tipo);

  return (
    <>
      <RivalMarker driverId={driver.id} />
      {driver.is_estreante_da_vida ? (
        <Tooltip texto={i18n.t("standings.badge.lifeRookie")}>
          <span className="shrink-0 text-xs" aria-label={i18n.t("standings.badge.lifeRookie")}>
            {"\u{1F331}"}
          </span>
        </Tooltip>
      ) : null}
      {driver.is_estreante && !driver.is_estreante_da_vida ? (
        <Tooltip texto={i18n.t("standings.badge.rookie")}>
          <span className="shrink-0 text-xs" aria-label={i18n.t("standings.badge.rookie")}>
            {"\u2B50"}
          </span>
        </Tooltip>
      ) : null}
      {injuryMarker ? (
        <Tooltip texto={injuryMarker.title}>
          <span className="shrink-0 text-xs" aria-label={injuryMarker.title}>
            {injuryMarker.emoji}
          </span>
        </Tooltip>
      ) : null}
      {driver.is_aposentado ? (
        <Tooltip texto={i18n.t("standings.badge.retiredTitle")}>
          <span
            className="shrink-0 rounded bg-white/10 px-1 text-[10px] font-semibold uppercase tracking-wide text-white/55"
            aria-label={i18n.t("standings.badge.retired")}
          >
            {i18n.t("standings.badge.retired")}
          </span>
        </Tooltip>
      ) : null}
    </>
  );
}

function PositionDelta({ delta }) {
  if (!delta) {
    return <span className="text-[10px] font-semibold text-text-muted">•</span>;
  }

  if (delta > 0) {
    return (
      <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-green">
        ▲{delta}
      </span>
    );
  }

  return (
    <span className="inline-flex items-center justify-center gap-0.5 text-[10px] font-semibold leading-none text-status-red">
      ▼{Math.abs(delta)}
    </span>
  );
}

function rowStyle({ isInSelectedTeam, teamColor }) {
  if (isInSelectedTeam && teamColor) {
    return {
      backgroundColor: `${teamColor}22`,
      boxShadow: `inset 3px 0 0 0 ${teamColor}`,
    };
  }

  return undefined;
}

// A tabela de pilotos: uma coluna por rodada (duplo clique numa rodada concluída
// reabre a corrida), realce da dupla da equipe em foco e as faixas de classe quando a
// categoria é multiclasse.
function DriverStandingsTable({
  sections,
  specialClassGroups,
  leadClassId,
  totalRodadas,
  completedRounds,
  currentRound,
  positionDeltaMap,
  previousChampionId,
  selectedTeamId,
  selectedTeamColor,
  onDriverHover,
  onDriverClick,
  onDriverDoubleClick,
  onDriverActivate,
  onReviewRace,
}) {
  const { t } = useTranslation();

  return (
    <div className="mt-6 overflow-x-auto">
      {/* As colunas de identidade são medidas pelo conteúdo, não por gosto: a de
          posição cabe "12" + o selo "▲5" e nada mais, e a da bandeira é a caixa de
          24px do FlagIcon com um respiro de 2px de cada lado. Largura sobrando aqui
          vira rio de espaço morto entre o número e a bandeira.

          As colunas de rodada, ao contrário, NÃO têm largura fixa: sem `width` o
          `table-fixed` divide entre elas o que sobra, então o calendário encolhe
          para caber e a coluna de pontos continua na tela sem precisar ser fixada
          (foi a coluna fixa que pintava a faixa escura sobre o card de vidro). O
          `minWidth` é só o piso — abaixo de RODADA_MIN_PX o selo "P10" (40px) não
          cabe mais. */}
      <table
        className="table-fixed"
        style={{
          width: larguraDaTabela(totalRodadas),
          minWidth: COLUNAS_FIXAS_PX + totalRodadas * RODADA_MIN_PX,
        }}
      >
        <colgroup>
          <col style={{ width: "44px" }} />
          <col style={{ width: "28px" }} />
          <col style={{ width: "42px" }} />
          <col style={{ width: "180px" }} />
          <col style={{ width: "84px" }} />
          {Array.from({ length: totalRodadas }, (_, index) => (
            <col key={`rodada-col-${index + 1}`} />
          ))}
          <col style={{ width: "66px" }} />
        </colgroup>
        <thead>
          <tr className="border-b border-white/10 text-left text-[11px] uppercase tracking-[0.18em] text-text-muted">
            <th className="py-3 pr-0.5">{t("standings.col.pos")}</th>
            <th className="py-3 text-center" />
            <th className="py-3 pr-1 text-center">Id</th>
            <th className="py-3 pr-1">{t("standings.col.driver")}</th>
            <th className="py-3 pr-1">{t("standings.col.team")}</th>
            {Array.from({ length: totalRodadas }, (_, index) => {
              const rodada = index + 1;
              const isCompleted = rodada <= completedRounds;
              return (
                // Sem texto o balão sai da frente sozinho — a rodada que ainda
                // não correu não tem o que explicar.
                <Tooltip key={rodada} texto={isCompleted ? t("standings.badge.reviewRace") : undefined}>
                  <th
                    className={[
                      "px-0.5 py-3 text-center",
                      rodada > completedRounds ? "opacity-30" : "",
                      rodada === currentRound ? "text-accent-primary" : "",
                      isCompleted ? "cursor-pointer hover:text-accent-primary transition-colors" : "",
                    ].join(" ")}
                    onDoubleClick={isCompleted ? () => onReviewRace?.(rodada) : undefined}
                  >
                    R{rodada}
                  </th>
                </Tooltip>
              );
            })}
            <th className="py-3 pl-3 pr-1 text-right">Pts</th>
          </tr>
        </thead>
        <tbody>
          {sections.map((section, sectionIndex) => (
            <Fragment key={`drivers-${section.id}`}>
              {/* A faixa da classe da própria linha, aberta no topo, seria eco: o
                  cabeçalho do painel já diz "MAZDA · Production" e a ordenação
                  garante que ela vem primeiro. As classes visitantes seguem
                  separadas, que aí a faixa é o que avisa da troca. */}
              {specialClassGroups && !(sectionIndex === 0 && section.id === leadClassId) ? (
                <tr>
                  <td colSpan={totalRodadas + 6} className="px-0 pt-4 pb-2">
                    <SpecialClassHeader section={section} sticky />
                  </td>
                </tr>
              ) : null}
              {section.items.map((driver, index) => {
                const isInSelectedTeam = selectedTeamId != null && driver.equipe_id === selectedTeamId;
                const teamColor = selectedTeamColor;
                const displayPosition = specialClassGroups ? index + 1 : driver.posicao_campeonato;

                return (
                  <tr
                    key={driver.id}
                    role="button"
                    tabIndex={0}
                    onMouseEnter={() => onDriverHover(driver.id)}
                    onMouseLeave={() => onDriverHover(null)}
                    onClick={() => onDriverClick(driver.id)}
                    onDoubleClick={() => onDriverDoubleClick(driver.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        onDriverActivate(driver.id);
                      }
                    }}
                    className={[
                      "cursor-pointer border-b border-white/5 transition-glass",
                      !isInSelectedTeam && driver.is_jogador
                        ? "bg-accent-primary/8 hover:bg-accent-primary/15"
                        : !isInSelectedTeam
                          ? "hover:bg-white/5 focus-visible:bg-white/5"
                          : "",
                    ].join(" ")}
                    style={rowStyle({ isInSelectedTeam, teamColor })}
                  >
                    <td className="py-3 pr-0.5 text-sm font-semibold">
                      <div className="flex items-center gap-1 whitespace-nowrap">
                        <span className={podiumClass(index)}>{displayPosition}</span>
                        <PositionDelta delta={positionDeltaMap.get(driver.id)} />
                      </div>
                    </td>
                    <td className="py-3 text-center">
                      <FlagIcon nacionalidade={driver.nacionalidade} className="mx-auto" />
                    </td>
                    <td className="py-3 pr-1 text-center text-sm font-medium text-text-secondary">
                      {driver.idade ?? "—"}
                    </td>
                    <td className="py-3 pr-1">
                      <div className="flex items-center gap-2">
                        <Tooltip texto={driver.nome} soSeCortado>
                          <span
                            className={[
                              "block truncate text-sm whitespace-nowrap",
                              driver.is_jogador ? "font-semibold text-accent-primary" : "text-text-primary",
                            ].join(" ")}
                          >
                            {driver.is_jogador ? "▸ " : ""}
                            {driver.nome}
                            {driver.is_jogador ? " ◂" : ""}
                          </span>
                        </Tooltip>
                        <DriverStatusMarkers driver={driver} />
                        {driver.id === previousChampionId ? (
                          <Tooltip texto={t("standings.badge.prevChampion")}>
                            <span className="shrink-0 text-sm">🏆</span>
                          </Tooltip>
                        ) : null}
                      </div>
                    </td>
                    <td className="py-3 pr-1 pl-0 text-sm font-semibold uppercase tracking-[0.02em]">
                      <div className="flex items-center gap-1.5">
                        <TeamLogoMarkShared
                          teamName={driver.equipe_nome}
                          color={driver.equipe_cor}
                          size="sm"
                        />
                        <Tooltip texto={driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"} soSeCortado>
                          <span
                            className="block truncate"
                            style={{ color: getReadableTeamColor(driver.equipe_cor) }}
                          >
                            {driver.equipe_nome_curto ?? driver.equipe_nome ?? "—"}
                          </span>
                        </Tooltip>
                      </div>
                    </td>
                    {(driver.results ?? []).map((result, rodadaIndex) => (
                      <td key={`${driver.id}-r${rodadaIndex + 1}`} className="px-0.5 py-3 text-center">
                        <ResultBadge result={result} />
                      </td>
                    ))}
                    <td className="kfx py-3 pl-3 pr-1 text-right font-mono text-sm font-semibold text-text-primary">
                      {driver.pontos}
                    </td>
                  </tr>
                );
              })}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default DriverStandingsTable;
