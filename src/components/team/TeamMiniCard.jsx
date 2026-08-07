import { useTranslation } from "react-i18next";

import AnchoredCard, { NumeroDaFicha, SecaoDaFicha } from "../ui/AnchoredCard";
import TeamLogoMark from "./TeamLogoMark";
import { formatMoney } from "../../utils/formatters";
import {
  RANKING_TIER_COLORS,
  carTierIndex,
  qualityTierIndex,
} from "./myteam/teamMetrics";

// A ficha rápida da equipe — a irmã da ficha do piloto, no mesmo gesto.
//
// No mercado o jogador não escolhe um piloto no vácuo: escolhe um ASSENTO, e o
// assento é metade piloto, metade equipe. Clicar no nome respondia a primeira
// metade e deixava a segunda numa gaveta atrás de um duplo clique.
//
// Não busca nada. Tudo o que ela desenha já veio no `TeamStanding` que pintou o
// card do grid — abrir a ficha é instantâneo, e a tela não ganha vinte consultas
// por desenhar vinte logos.
//
// Os tiers em vez dos números crus são a mesma decisão do ranking da categoria:
// "88 de confiabilidade" não responde se isso é bom, "Robusto" responde. E os
// dois falam a MESMA régua, então o jogador não precisa aprender duas.

const LARGURA = 300;

function LinhaDeTier({ rotulo, tier, label }) {
  const cor = RANKING_TIER_COLORS[tier] ?? RANKING_TIER_COLORS[2];
  return (
    <div className="flex items-center gap-2 py-[3px]">
      <span className="min-w-0 flex-1 truncate text-[10.5px] text-[color:var(--text-muted)]">
        {rotulo}
      </span>
      <span className="flex shrink-0 items-center gap-[3px]" aria-hidden="true">
        {Array.from({ length: 5 }).map((_, i) => (
          <span
            key={i}
            className="h-1.5 w-1.5 rounded-full"
            style={{ background: i <= tier ? cor : "rgba(255,255,255,0.12)" }}
          />
        ))}
      </span>
      <span
        className="w-[74px] shrink-0 text-right text-[10.5px] font-bold"
        style={{ color: cor }}
      >
        {label}
      </span>
    </div>
  );
}

function Corpo({ team }) {
  const { t } = useTranslation();
  const tierCarro = carTierIndex(team.car_level);
  const tierConfiabilidade = qualityTierIndex(team.confiabilidade);
  const tierPitCrew = qualityTierIndex(team.pit_crew_quality);
  const titulos = team.categoria_titulos ?? 0;
  const vitorias = team.categoria_vitorias ?? 0;
  const caixa = team.cash_balance ?? 0;

  return (
    <>
      <div className="flex items-center gap-2.5 px-3 pb-2.5 pt-3">
        <TeamLogoMark
          teamName={team.nome}
          color={team.cor_primaria}
          size="sm"
          testId="team-mini-card-logo"
        />
        <div className="min-w-0 flex-1">
          <p className="truncate pr-5 text-[14px] font-bold leading-[1.1] text-[color:var(--text-primary)]">
            {team.nome}
          </p>
          <p className="mt-0.5 truncate text-[10px] text-[color:var(--text-muted)]">
            {team.founded_year > 0
              ? t("teamMiniCard.founded", { year: team.founded_year })
              : t("teamMiniCard.noFoundedYear")}
          </p>
        </div>
      </div>

      <SecaoDaFicha titulo={t("teamMiniCard.season")}>
        <div className="flex items-start gap-1">
          <NumeroDaFicha
            rotulo={t("teamMiniCard.position")}
            valor={team.temp_posicao > 0 ? team.temp_posicao : 0}
          />
          <NumeroDaFicha rotulo={t("teamMiniCard.points")} valor={team.pontos} />
          <NumeroDaFicha rotulo={t("teamMiniCard.wins")} valor={team.vitorias} destaque />
        </div>
      </SecaoDaFicha>

      <SecaoDaFicha titulo={t("teamMiniCard.structure")}>
        <LinhaDeTier
          rotulo={t("myTeamTab.ranking.columns.carLevel")}
          tier={tierCarro}
          label={t(`myTeamTab.ranking.tiers.car.${tierCarro}`)}
        />
        <LinhaDeTier
          rotulo={t("myTeamTab.ranking.columns.reliability")}
          tier={tierConfiabilidade}
          label={t(`myTeamTab.ranking.tiers.reliability.${tierConfiabilidade}`)}
        />
        <LinhaDeTier
          rotulo={t("myTeamTab.ranking.columns.pitCrew")}
          tier={tierPitCrew}
          label={t(`myTeamTab.ranking.tiers.pitCrew.${tierPitCrew}`)}
        />
        <div className="mt-1 flex items-center gap-2 border-t border-white/8 pt-1.5">
          <span className="min-w-0 flex-1 truncate text-[10.5px] text-[color:var(--text-muted)]">
            {t("teamMiniCard.cash")}
          </span>
          <span
            className="shrink-0 font-mono text-[11px] font-bold"
            style={{ color: caixa < 0 ? "var(--status-red)" : "var(--text-primary)" }}
          >
            {formatMoney(caixa)}
          </span>
        </div>
      </SecaoDaFicha>

      <SecaoDaFicha titulo={t("teamMiniCard.inCategory")}>
        <p className="text-[11px] leading-snug text-[color:var(--text-secondary)]">
          {titulos > 0 || vitorias > 0 ? (
            <>
              {titulos > 0 && (
                <span className="font-bold" style={{ color: "#ffd700" }}>
                  {t("teamMiniCard.titles", { count: titulos })}
                </span>
              )}
              {titulos > 0 && vitorias > 0 && <span aria-hidden="true"> · </span>}
              {vitorias > 0 && <span>{t("teamMiniCard.categoryWins", { count: vitorias })}</span>}
            </>
          ) : (
            <span className="italic text-[color:var(--text-muted)]">
              {t("teamMiniCard.noRecord")}
            </span>
          )}
        </p>
      </SecaoDaFicha>
    </>
  );
}

/// Envolve o logo da equipe e abre a ficha rápida no clique.
export default function TeamMiniCard({ team, children }) {
  const { t } = useTranslation();

  return (
    <AnchoredCard
      largura={LARGURA}
      testId="team-mini-card"
      rotuloFechar={t("teamMiniCard.close")}
      desabilitado={!team}
      // O logo é imagem: sublinhar não diz nada nele. O brilho é o mesmo realce
      // que os cards do grid já usam para dizer "isto responde ao mouse".
      realce="transition-all hover:brightness-125 focus-visible:brightness-125"
      conteudo={<Corpo team={team} />}
    >
      {children}
    </AnchoredCard>
  );
}
