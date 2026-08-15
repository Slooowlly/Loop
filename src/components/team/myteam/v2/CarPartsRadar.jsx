import { useTranslation } from "react-i18next";

import TeamLogoMark, { getTeamLogoSrc } from "../../TeamLogoMark";
import { CAR_PART_GROUPS, PART_LEVEL_MAX, carPartsRadar, radarView } from "./gridMetrics";

// O radar NÃO é medido: não tem eixo em pixel para calibrar, só proporção. O viewBox
// fixa a relação entre o raio e a reserva dos rótulos, e o SVG escala uniformemente
// até o teto do container — assim o desenho nunca depende de o ResizeObserver ter
// medido a coluna certa.
const VIEW = radarView(520, { max: 520, min: 520 });

// A grade vai em rgba literal: `stroke="currentColor"` com `text-white/8` dependia de
// o Tailwind ter gerado aquela opacidade, e quando não gera o `currentColor` cai na
// cor de texto herdada — a teia discreta vira branca. Medido: `text-white/8`
// resolvia para #e6edf3.
const GRADE = "rgba(255,255,255,0.09)";

// GRÁFICO 2 — o carro por ÁREA, de TODAS as equipes no mesmo desenho.
//
// O radar anterior tinha cinco eixos de grandezas diferentes (carro, confiabilidade,
// pit crew, caixa, pontos) e três polígonos. Ele respondia "estou torto?" mas não
// "torto ONDE" — porque o eixo "carro" era o `car_level`, que é a MÉDIA das onze
// peças. Duas equipes em Nível 4 podem ter chassis 9 e motor 1, ou tudo em 4: são
// investimentos opostos com a mesma etiqueta, e a média apagava a diferença.
//
// As onze peças, porém, não cabem num eixo cada: onze direções com quatro anéis
// desenhavam uma teia em que a grade tinha mais presença que o dado. Elas entram
// agrupadas em cinco áreas funcionais (ver `CAR_PART_GROUPS`) — o que o olho separa
// de relance — e o nome das peças de cada área fica no tooltip do rótulo.
//
// Todas as equipes entram, fracas; a que estiver sob o mouse fica opaca e grossa. É
// o único jeito de ler 6 a 10 polígonos sobrepostos sem virar novelo — e é a leitura
// que o comparativo pede: contra QUEM eu perco, e em QUE peça.
//
// O radar NÃO tem controle próprio: quem acende cada equipe é a TABELA ao lado
// (`hoveredTeamId` vem de fora). A fita de chips que morava aqui embaixo era uma
// segunda lista das mesmas equipes que a tabela já enumera — e obrigava a olhar para
// baixo do gráfico para mexer no gráfico. O alvo natural é o nome da equipe onde ele
// já está.
function CarPartsRadar({ cars, playerTeamId, hoveredTeamId = null }) {
  const { t } = useTranslation();
  const radar = carPartsRadar({ cars, playerTeamId, view: VIEW });

  if (!radar.hasData) {
    return (
      <p className="border-y border-white/[0.08] px-4 py-8 text-center text-[11px] leading-5 text-text-secondary">
        {t("myTeamTabV2.parts.empty")}
      </p>
    );
  }

  // Um id de fora que não está no grid (equipe de outra categoria, linha em cache)
  // não pode apagar o destaque: cai de volta na sua equipe.
  const hasHovered = radar.teams.some((team) => team.id === hoveredTeamId);
  const active = hasHovered ? hoveredTeamId : (radar.player?.id ?? null);
  const activeTeam = radar.teams.find((team) => team.id === active) ?? null;
  // Desenha na ordem: as apagadas primeiro, a ativa por último. Sem isso a linha em
  // destaque some por baixo de uma equipe qualquer desenhada depois.
  const ordered = [...radar.teams].sort((a, b) => Number(a.id === active) - Number(b.id === active));

  return (
    <div data-testid="my-team-v2-parts-radar">
      <svg
        viewBox={`0 0 ${VIEW.width} ${VIEW.height}`}
        className="mx-auto block h-auto w-full max-w-[520px]"
        role="img"
        aria-label={t("myTeamTabV2.parts.title")}
      >
        {radar.rings.map((ring, index) => (
          <polygon key={index} points={ring} fill="none" stroke={GRADE} />
        ))}
        {radar.axes.map((axis) => (
          <line
            key={axis.key}
            x1={VIEW.cx}
            y1={VIEW.cy}
            x2={axis.spoke.x}
            y2={axis.spoke.y}
            stroke={GRADE}
          />
        ))}

        {ordered.map((team) => {
          const isActive = team.id === active;
          return (
            <polygon
              key={team.id}
              points={team.polygon}
              fill={isActive ? team.color : "none"}
              fillOpacity={isActive ? 0.16 : 0}
              stroke={team.color}
              strokeWidth={isActive ? 2.5 : 1}
              strokeOpacity={isActive ? 1 : 0.28}
              className="transition-all duration-200"
              data-testid={team.isPlayer ? "parts-radar-player-polygon" : undefined}
            />
          );
        })}

        {radar.axes.map((axis) => (
          <g key={axis.key}>
            {/* O `<title>` abre a área: passar o mouse no rótulo diz quais peças
                entraram nela, sem poluir o desenho com onze nomes. */}
            <title>
              {t("myTeamTabV2.parts.groupTooltip", {
                group: t(`myTeamTabV2.parts.groups.${axis.key}`),
                parts: axis.parts.map((key) => t(`myTeamTabV2.parts.names.${key}`)).join(", "),
              })}
            </title>
            <text
              x={axis.label.x}
              y={axis.label.y + (axis.above ? -3 : 0)}
              textAnchor={axis.anchor}
              className="fill-text-muted text-[11px] uppercase tracking-[0.06em]"
            >
              {t(`myTeamTabV2.parts.groups.${axis.key}`)}
            </text>
            <text
              x={axis.label.x}
              y={axis.label.y + (axis.above ? 11 : 14)}
              textAnchor={axis.anchor}
              className="fill-text-primary font-garage text-[12px]"
            >
              {axisReading(t, axis, active, radar)}
            </text>
          </g>
        ))}
      </svg>

      {/* Sem a fita de chips, nada dizia de QUEM é o polígono aceso. Esta linha é a
          legenda do desenho: a cor e o nome de quem está em destaque agora. */}
      {activeTeam ? (
        <p className="mt-4 flex items-center justify-center gap-2.5 text-xs" data-testid="parts-radar-active">
          {/* Nome COMPLETO e a mesma logo da tabela: é o mesmo par que a linha sob o
              mouse mostra do outro lado do card, e é o que fecha o laço entre apontar
              lá e o desenho mudar aqui. */}
          {getTeamLogoSrc(activeTeam.fullName) ? (
            <TeamLogoMark teamName={activeTeam.fullName} color={activeTeam.color} size="xs" testId="parts-radar-active-logo" />
          ) : (
            <span aria-hidden="true" className="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: activeTeam.color }} />
          )}
          <span className="text-text-primary">{activeTeam.fullName || activeTeam.name}</span>
          {activeTeam.isPlayer ? <span className="text-accent-primary">{t("myTeamTabV2.parts.youMark")}</span> : null}
        </p>
      ) : null}

      {radar.weakest ? (
        <p className="mt-3 text-xs leading-5 text-text-secondary" data-testid="parts-radar-reading">
          {radar.weakest.gapToBest >= 0.5
            ? t("myTeamTabV2.parts.readingWeak", {
                part: t(`myTeamTabV2.parts.groups.${radar.weakest.key}`),
                count: Math.round(radar.weakest.gapToBest),
              })
            : t("myTeamTabV2.parts.readingBest")}
        </p>
      ) : null}
    </div>
  );
}

// O número ao lado do rótulo segue quem está em destaque: com a sua equipe acesa ele
// mostra o seu nível contra o melhor do grid; com uma rival acesa, o dela. Ler o
// polígono aceso e um número de outra equipe seria pior que não ter número.
function axisReading(t, axis, activeId, radar) {
  const team = radar.teams.find((row) => row.id === activeId);
  const index = CAR_PART_GROUPS.findIndex((group) => group.key === axis.key);
  const level = team ? team.levels[index] : axis.averageLevel;
  // Uma casa decimal: a área é a MÉDIA de duas ou três peças, então arredondar para
  // inteiro empataria áreas que estão a meio nível de distância.
  const rounded = Math.round(level * 10) / 10;
  return t("myTeamTabV2.parts.axisReading", { level: rounded, max: PART_LEVEL_MAX });
}

export default CarPartsRadar;
