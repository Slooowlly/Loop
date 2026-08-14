import { useState } from "react";
import { useTranslation } from "react-i18next";

import GlassCard from "../../../ui/GlassCard";
import Tooltip from "../../../ui/Tooltip";
import TeamLogoMark, { getTeamLogoSrc } from "../../TeamLogoMark";
import CarPartsRadar from "./CarPartsRadar";
import EfficiencyScatter from "./EfficiencyScatter";
import { formatMoney, formatMoneyCompact } from "../../../../utils/formatters";
import { RANKING_TIER_COLORS, carTierIndex, qualityTierIndex } from "../teamMetrics";
import { readableOn } from "./gridMetrics";

// O comparativo, agora com três resoluções da mesma pergunta: o radar dá a FORMA
// ("onde estou torto"), a tabela dá os NÚMEROS, e a dispersão dá o JULGAMENTO
// ("converto dinheiro em pontos?").
//
// Nível do carro, confiabilidade e pit crew são a faixa do v1, com cor e nome: a
// palavra é a leitura, e um número ao lado dela diria a mesma coisa em algarismo.
// A escala fina de cada eixo vive no radar ao lado, contra a do líder.
//
// A ordem das colunas segue a pergunta, não o alfabeto: nome, CAIXA (o que a aba de
// gestão existe para comparar), as três técnicas e, no fim, os pontos — que são a
// consequência de tudo o que vem antes.
//
// A coluna de caixa mostra o SALDO ABSOLUTO de cada equipe. Ela já foi a diferença
// contra o jogador — e essa versão mentia: um grid saudável aparecia como "-$342k",
// "-$918k", "-$457k" em vermelho, que qualquer um lê como equipe quebrada. O sinal
// de menos e o vermelho são vocabulário de dívida; usá-los para dizer "tem menos que
// você" é gastar o alarme mais forte da tela com uma informação que não é alarme. Em
// valor absoluto, vermelho volta a significar uma só coisa: caixa negativo de fato.
function GridComparative({ teams, cars, playerTeam, historyTeamId, onTeamHistoryOpen }) {
  const { t } = useTranslation();
  const rows = (Array.isArray(teams) ? teams : []).slice(0, 10);
  const playerCash = playerTeam?.cash_balance ?? 0;
  // O destaque do radar mora AQUI, e não dentro dele: quem aponta a equipe é a linha
  // da tabela, do outro lado do card. O radar só recebe o id e desenha.
  const [hoveredTeamId, setHoveredTeamId] = useState(null);

  return (
    <GlassCard hover={false} className="rounded-[28px]" data-testid="my-team-v2-comparative">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">{t("myTeamTabV2.comparative.eyebrow")}</p>
      <h3 className="mt-2 text-xl font-semibold text-text-primary">{t("myTeamTabV2.comparative.title")}</h3>

      <div className="mt-5 grid gap-6 lg:grid-cols-[0.6fr_1.4fr]">
        <div>
          <p className="mb-3 text-[10px] uppercase tracking-[0.22em] text-text-muted">{t("myTeamTabV2.parts.title")}</p>
          <CarPartsRadar cars={cars} playerTeamId={playerTeam?.id} hoveredTeamId={hoveredTeamId} />
        </div>

        <div className="overflow-x-auto">
          <table className="min-w-full text-left text-sm" aria-label={t("myTeamTabV2.comparative.title")}>
            <thead>
              <tr className="border-b border-white/[0.08] text-[10px] uppercase tracking-[0.18em] text-text-muted">
                <th className="py-2.5 pr-3 font-normal">#</th>
                {/* A coluna do nome não pode ficar com a folga toda: era ela que
                    empurrava as outras seis para o canto direito e deixava meia
                    tabela vazia. `w-px` + `whitespace-nowrap` faz a célula pedir só o
                    que o conteúdo ocupa, e a folga se distribui entre as técnicas —
                    que agora carregam a palavra da faixa e sabem o que fazer com ela. */}
                <th className="w-px whitespace-nowrap py-2.5 pr-8 font-normal">{t("myTeamTabV2.comparative.columns.team")}</th>
                {/* Ordem: caixa logo depois do nome, pontos no fim. Caixa é a coluna
                    que a aba de gestão existe para comparar; largada no canto direito
                    ela ficava a uma tela de distância do nome da equipe. */}
                <th className="w-px whitespace-nowrap py-2.5 pr-8 text-right font-normal">
                  {t("myTeamTabV2.comparative.columns.cash")}
                </th>
                {/* As três técnicas são a faixa do v1, sem número: a palavra JÁ é a
                    leitura, e o valor bruto ao lado dela só repetiria em algarismo o
                    que ela diz. Quem quiser a escala tem o radar, que mostra cada
                    eixo contra o do líder. */}
                <th className="py-2.5 pr-6 font-normal">{t("myTeamTabV2.comparative.columns.car")}</th>
                <th className="py-2.5 pr-6 font-normal">{t("myTeamTabV2.comparative.columns.reliability")}</th>
                <th className="py-2.5 pr-6 font-normal">{t("myTeamTabV2.comparative.columns.pitCrew")}</th>
                <th className="py-2.5 text-right font-normal">{t("myTeamTabV2.comparative.columns.points")}</th>
              </tr>
            </thead>
            {/* O `onMouseLeave` fica no corpo inteiro, e não linha a linha: sair de
                uma linha para a vizinha dispararia o "apagar" DEPOIS do "acender" da
                próxima em alguns navegadores, e o radar piscaria a cada troca. */}
            <tbody onMouseLeave={() => setHoveredTeamId(null)}>
              {rows.map((team, index) => {
                const isPlayer = team.id === playerTeam?.id;
                const cash = isPlayer ? playerCash : Number(team.cash_balance) || 0;
                return (
                  <tr
                    key={team.id}
                    className={[
                      "border-b border-white/[0.06] last:border-0 transition-all duration-200",
                      team.id === historyTeamId
                        ? "bg-status-yellow/10 text-text-primary ring-1 ring-status-yellow/45"
                        : isPlayer
                          ? "bg-accent-primary/10 text-text-primary"
                          : "text-text-secondary",
                    ].join(" ")}
                    onMouseEnter={() => setHoveredTeamId(team.id)}
                    // O contorno vai em `style` porque a linha já usa `ring` para o
                    // realce do histórico e `bg` para o jogador: uma terceira classe
                    // disputando as mesmas propriedades vira loteria de ordem de CSS.
                    style={
                      team.id === hoveredTeamId
                        ? { outline: "1px solid rgba(255,255,255,0.18)", outlineOffset: "-1px" }
                        : undefined
                    }
                    data-testid={isPlayer ? "comparative-player-row" : undefined}
                  >
                    <td className="py-2.5 pr-3 font-mono text-xs text-text-muted">
                      {String(team.posicao ?? index + 1).padStart(2, "0")}
                    </td>
                    <td className="w-px whitespace-nowrap py-2.5 pr-8">
                      <div className="flex items-center gap-2.5">
                        <TeamMark team={team} />
                        <Tooltip texto={t("myTeamTab.ranking.doubleClickHint")}>
                          <button
                            type="button"
                            onDoubleClick={() => onTeamHistoryOpen?.(team)}
                            // Teclado também acende o radar: o botão do nome é o
                            // único nó focável da linha.
                            onFocus={() => setHoveredTeamId(team.id)}
                            className="rounded-lg text-left transition-glass hover:brightness-125 focus:outline-none focus:ring-2 focus:ring-accent-primary/45"
                            style={{ color: team.cor_primaria ?? "#f0f6fc" }}
                          >
                            {/* Nome COMPLETO: a marca à esquerda já é a sigla quando
                                a equipe não tem logo, e "APE" ao lado de "APE" não
                                informa nada. A coluna tem largura de sobra. */}
                            {team.nome || team.nome_curto}
                          </button>
                        </Tooltip>
                      </div>
                    </td>
                    <td className="w-px whitespace-nowrap py-2.5 pr-8 text-right font-mono text-xs">
                      {/* A sua linha em valor cheio, as outras compactas: é a sua que
                          se lê ao centavo, e seis valores longos em coluna viram
                          parede de dígitos. Vermelho SÓ no saldo negativo. */}
                      <span
                        className={cash < 0 ? "text-status-red" : isPlayer ? "text-text-primary" : "text-text-secondary"}
                      >
                        {isPlayer ? formatMoney(cash) : formatMoneyCompact(cash)}
                      </span>
                    </td>
                    <TechCell
                      tier={carTierIndex(team.car_level)}
                      label={t(`myTeamTab.ranking.tiers.car.${carTierIndex(team.car_level)}`)}
                    />
                    <TechCell
                      tier={qualityTierIndex(team.confiabilidade)}
                      label={t(`myTeamTab.ranking.tiers.reliability.${qualityTierIndex(team.confiabilidade)}`)}
                    />
                    <TechCell
                      tier={qualityTierIndex(team.pit_crew_quality)}
                      label={t(`myTeamTab.ranking.tiers.pitCrew.${qualityTierIndex(team.pit_crew_quality)}`)}
                    />
                    <td className="py-2.5 text-right font-mono text-sm text-text-primary">{team.pontos ?? 0}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      <div className="mt-6 border-t border-white/[0.08] pt-5">
        <EfficiencyScatter teams={teams} playerTeamId={playerTeam?.id} />
      </div>
    </GlassCard>
  );
}

// A faixa, e só a faixa — como no v1. O ponto com brilho e a palavra na cor dele.
// O número bruto seria redundante: "Dominante" já é a leitura, e quem quiser a
// escala tem o radar ao lado, que mostra o valor de cada eixo contra o do líder.
function TechCell({ tier, label }) {
  const color = RANKING_TIER_COLORS[tier] ?? RANKING_TIER_COLORS[0];
  return (
    <td className="whitespace-nowrap py-2.5 pr-6">
      <span className="inline-flex items-center gap-2 text-xs font-semibold" style={{ color }}>
        <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full" style={{ backgroundColor: color, boxShadow: `0 0 8px ${color}80` }} />
        {label}
      </span>
    </td>
  );
}

// A marca da equipe. `TeamLogoMark` cai num retângulo de cor pura quando o nome não
// está no catálogo de logos — e seis retângulos vazios em coluna lêem como imagem
// quebrada, não como identidade. Aqui o fallback carrega a sigla, que é o mesmo que
// a tabela usa para nomear a equipe.
function TeamMark({ team }) {
  const logo = getTeamLogoSrc(team?.nome);
  if (logo) {
    return <TeamLogoMark teamName={team?.nome} color={team?.cor_primaria} size="sm" testId="comparative-team-logo" />;
  }

  const color = team?.cor_primaria ?? "#30363d";
  const initials = (team?.nome_curto || team?.nome || "?").slice(0, 3).toUpperCase();
  return (
    <span
      data-testid="comparative-team-logo"
      className="grid h-7 w-[42px] shrink-0 place-items-center rounded-lg border border-white/10 font-mono text-[11px] font-semibold shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]"
      style={{ backgroundColor: color, color: readableOn(color) }}
    >
      {initials}
    </span>
  );
}

export default GridComparative;
