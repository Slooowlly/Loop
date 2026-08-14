import { useTranslation } from "react-i18next";

import GlassCard from "../ui/GlassCard";
import TeamLogoMark from "../team/TeamLogoMark";
import { getTeamGlow } from "../../utils/teamColors";

function TeamCard({ team, selected, onSelect }) {
  const { t } = useTranslation();
  const selectionValue = team.id ?? team.index;
  const name = team.name ?? team.nome;
  const primaryColor = team.primaryColor ?? team.cor_primaria ?? "#ffffff";
  // Nome na cor da equipe; getTeamGlow clareia cores escuras pra ficarem legíveis
  // no card escuro sem perder o matiz.
  const nameColor = getTeamGlow(primaryColor).solid;
  const badge = team.country ?? null;

  return (
    <GlassCard
      selected={selected}
      darkBg
      onClick={() => onSelect(selectionValue)}
      className="flex min-h-[150px] flex-col"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-3">
          <TeamLogoMark
            teamName={name}
            color={primaryColor}
            size="md"
            testId="team-select-logo"
          />
          <h3 className="text-lg font-semibold" style={{ color: nameColor }}>
            {name}
          </h3>
        </div>
        {badge ? (
          <span className="shrink-0 rounded-full bg-white/[0.08] px-3 py-1 text-xs text-text-secondary">
            {badge}
          </span>
        ) : null}
      </div>

      {/* mt-auto: fixa o bloco de performance no rodapé do card. Como o grid estica os
          cards da linha pra mesma altura, os "PERFORMANCE" ficam alinhados embaixo,
          mesmo com nomes de 1 ou 2 linhas. */}
      <div className="mt-auto space-y-3 pt-8">
        <div className="flex items-center justify-between text-xs uppercase tracking-[0.16em] text-text-secondary">
          <span>{t("newCareer.teamCard.performance")}</span>
          <span className="font-semibold tracking-normal text-text-primary">???</span>
        </div>
        {/* Barra "desconhecida": hachura em vez de preenchimento — o desempenho do
            carro é um mistério que só se revela correndo. */}
        <div className="h-2.5 overflow-hidden rounded-full bg-white/[0.08]">
          <div
            className="h-full w-full opacity-40"
            style={{
              backgroundImage:
                "repeating-linear-gradient(45deg, rgba(255,255,255,0.2) 0 6px, transparent 6px 12px)",
            }}
          />
        </div>
      </div>
    </GlassCard>
  );
}

export default TeamCard;
