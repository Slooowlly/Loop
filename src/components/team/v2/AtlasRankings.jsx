import { useState } from "react";
import { useTranslation } from "react-i18next";

import Tooltip from "../../ui/Tooltip";
import TeamLogoMark from "../TeamLogoMark";
import { teamHighlight } from "../worldTeamChartGeometry";
import { bandAccent } from "./atlasV2Geometry";
import { atlasCategoryLogo } from "./atlasCategoryLogos";
import { LOGO_FRAME_HEIGHT, LOGO_FRAME_WIDTH, normalizedLogoLayout } from "./atlasLogoNormalization";
import { useNormalizedLogo } from "./useNormalizedLogo";
import { dismissPlayerGlow, isPlayerGlowDismissed } from "./atlasPlayerGlow";
import goldTrophy from "../../../assets/utilities/trophies/ouro.png";

// Vão entre o card do gráfico e a coluna lateral. É a distância que o rabicho
// colorido percorre, ligando o fim da linha à linha da equipe no card.
const CHART_CARD_GAP = 22;

// Coluna lateral: um card independente por campeonato, participando do grid
// principal. Nada aqui é posicionado sobre o gráfico — é o ponto central do
// redesenho em relação ao v1, onde estas tabelas flutuavam por cima da plotagem.
//
// Toda posição vertical vem da régua compartilhada (`vertical`): a altura do card,
// a altura do cabeçalho e o centro de cada linha são os MESMOS números que o
// gráfico usa. `topOffset` compensa a barra dos anos, que existe só do lado do
// gráfico — sem ele as duas réguas começariam em alturas diferentes.
export function AtlasRankings({
  laneRef,
  cards,
  vertical,
  lastAxisYear,
  focusedTeamId,
  pinnedTeamId,
  onFocus,
  onTeamClick,
  onTeamDoubleClick,
  playerCategory = null,
  onOpenChampions,
}) {
  // A luz respira até o primeiro clique dentro do card do jogador — ver
  // `atlasPlayerGlow.js`.
  const [glowActive, setGlowActive] = useState(() => !isPlayerGlowDismissed());

  function extinguishGlow() {
    setGlowActive(false);
    dismissPlayerGlow();
  }

  return (
    <div
      ref={laneRef}
      data-testid="atlas-v2-rankings"
      className="relative min-h-0 min-w-0"
      // Item da linha 2 do grid, igual à área de plotagem: mesma origem vertical,
      // sem recuo algum. Um `padding-top` aqui seria inútil de qualquer forma —
      // os cards são absolutos e ignorariam o padding, que foi exatamente o bug.
      style={{ gridColumn: 2, gridRow: 2 }}
      onMouseLeave={() => onFocus(null)}
    >
      {cards.map((card) => {
        const division = vertical?.divisions?.[card.key];
        if (!division) return null;
        return (
          <RankingCard
            key={card.key}
            card={card}
            division={division}
            vertical={vertical}
            // A luz só acende no card da categoria em que o jogador corre AGORA, e
            // só enquanto ele não tiver aberto nada ali dentro.
            isGlowing={glowActive && !!playerCategory && card.band.category === playerCategory}
            onGlowConsumed={extinguishGlow}
            // O rabicho só faz sentido quando a linha daquele campeonato chega de
            // fato à borda direita do gráfico.
            showConnector={card.referenceYear === lastAxisYear}
            focusedTeamId={focusedTeamId}
            pinnedTeamId={pinnedTeamId}
            onFocus={onFocus}
            onTeamClick={onTeamClick}
            onTeamDoubleClick={onTeamDoubleClick}
            onOpenChampions={onOpenChampions}
          />
        );
      })}
    </div>
  );
}

function RankingCard({ card, division, vertical, showConnector, focusedTeamId, pinnedTeamId, isGlowing = false, onGlowConsumed, onFocus, onTeamClick, onTeamDoubleClick, onOpenChampions }) {
  const { t } = useTranslation();
  // O ano de referência entra no próprio título, como na referência — o selo
  // separado competia com o nome do campeonato por atenção.
  const title = Number.isFinite(card.referenceYear) ? `${card.label} ${card.referenceYear}` : card.label;
  // A mesma cor que o título dentro do gráfico usa: um campeonato tem uma cor só,
  // dos dois lados do vão.
  const accent = bandAccent(card.band);

  return (
    <section
      data-testid={`atlas-v2-ranking-${card.key}`}
      className="absolute inset-x-0 rounded-xl border border-[#2b4266] bg-[linear-gradient(180deg,#0c1828_0%,#091220_100%)] shadow-[0_10px_28px_rgba(0,0,0,0.28),inset_0_1px_0_rgba(148,197,255,0.05)]"
      style={{ top: division.top, height: division.height }}
      // Qualquer clique dentro do card conta como "abriu": o cabeçalho leva aos
      // campeões, a linha leva ao dossiê, e os dois provam que a porta foi achada.
      // Em captura, para apagar a luz mesmo quando o clique abre um painel por cima.
      onClickCapture={isGlowing ? onGlowConsumed : undefined}
    >
      {isGlowing ? (
        <span
          aria-hidden="true"
          data-testid={`atlas-v2-player-glow-${card.key}`}
          className="discovery-glow-layer"
          style={{ "--discovery-glow": accent }}
        />
      ) : null}

      {/* Cabeçalho de painel, não de accordion. A altura é a mesma que o gráfico
          reserva acima da primeira posição — é o que mantém as linhas alinhadas. */}
      {/* O cabeçalho INTEIRO é a porta do salão dos campeões.
          Antes o único alvo era o troféu de 22px, que ninguém lia como botão — a
          tela dos campeões existia e não tinha como ser descoberta. Agora a faixa
          toda responde ao clique, e o chevron na ponta direita é o que anuncia
          isso em repouso. É um destino diferente do que as linhas abaixo abrem,
          por isso o troféu continua ao lado do chevron: ele diz PARA ONDE vai. */}
      <Tooltip texto={t("globalTeams.championsOpen", { band: card.label })}>
        <button
          type="button"
          data-testid={`atlas-v2-champions-open-${card.key}`}
          onClick={() => onOpenChampions?.(card.band)}
          aria-label={t("globalTeams.championsOpen", { band: card.label })}
          className="group flex w-full cursor-pointer items-center gap-3 border-b border-[#2b4266] px-3.5 text-left transition-colors hover:bg-white/[0.05]"
          style={{
            height: division.headerHeight,
            background: `linear-gradient(180deg, color-mix(in srgb, ${accent} 11%, transparent) 0%, color-mix(in srgb, ${accent} 2%, transparent) 100%)`,
          }}
        >
          <CategoryLogo category={card.band.category} accent={accent} />
          <span
            data-testid={`atlas-v2-ranking-title-${card.key}`}
            className="min-w-0 flex-1 truncate text-[14.5px] font-bold uppercase tracking-[0.08em]"
            style={{ color: accent }}
          >
            {title}
          </span>
          <span
            aria-hidden="true"
            data-testid={`atlas-v2-champions-chevron-${card.key}`}
            className="shrink-0 text-[15px] leading-none opacity-45 transition-opacity group-hover:opacity-100"
            style={{ color: accent }}
          >
            ›
          </span>
        </button>
      </Tooltip>

      {card.rows.length === 0 ? (
        <p className="px-3 pt-3 text-center text-[11px] text-slate-500">
          {t("globalTeams.bandNotStarted", { band: card.label })}
        </p>
      ) : null}

      {card.rows.map((entry) => {
        const { isFocused, isDimmed } = teamHighlight(entry.team_id, focusedTeamId, pinnedTeamId);
        // Centro da linha = rankY da régua compartilhada, o mesmo Y do ponto de
        // 2025 no gráfico. Posicionamento absoluto para não depender da ordem nem
        // de posições contíguas.
        const center = vertical.rankY(card.key, entry.position) - division.top;
        return (
          <div
            key={entry.team_id}
            className="absolute inset-x-1.5"
            style={{ top: center - division.rowHeight / 2, height: division.rowHeight }}
          >
            {showConnector ? (
              <span
                aria-hidden="true"
                data-testid={`atlas-v2-row-connector-${entry.team_id}`}
                className="pointer-events-none absolute top-1/2 -translate-y-1/2"
                style={{
                  right: "100%",
                  width: CHART_CARD_GAP,
                  height: 2,
                  background: `linear-gradient(to right, ${entry.cor}, transparent)`,
                }}
              />
            ) : null}
            <button
              type="button"
              data-testid={`atlas-v2-ranking-row-${entry.team_id}`}
              onMouseEnter={() => onFocus(entry.team_id)}
              onFocus={() => onFocus(entry.team_id)}
              onClick={() => onTeamClick({ ...entry.row, band_key: card.key, band_category: card.band.category })}
              onDoubleClick={() => onTeamDoubleClick({ ...entry.row, band_key: card.key, band_category: card.band.category })}
              // Ao vivo a linha ganha duas colunas: a variação, logo depois do
              // número da posição (é dele que ela fala), e o placar parcial antes
              // dos troféus.
              className={`grid h-full w-full cursor-pointer ${
                card.isLive
                  ? "grid-cols-[20px_14px_24px_minmax(0,1fr)_auto_34px]"
                  : "grid-cols-[20px_24px_minmax(0,1fr)_40px]"
              } items-center gap-x-2 rounded-md px-2 text-left transition-colors ${
                isFocused ? "bg-white/[0.07]" : ""
              } ${isDimmed ? "opacity-40" : ""}`}
            >
              <span className="text-right font-mono text-[12px] font-bold text-slate-500">{entry.position}</span>
              {card.isLive ? (
                <DeltaGlyph entry={entry} baselineYear={card.baselineYear} referenceYear={card.referenceYear} />
              ) : null}
              <span className="grid place-items-center">
                <TeamLogoMark teamName={entry.nome} color={entry.cor} size="xs" testId="atlas-v2-ranking-logo" />
              </span>
              {/* pl-1 soma ao gap para dar os ~8px de respiro entre o brasão e o
                  nome; o TeamLogoMark ocupa a célula inteira. */}
              <span className="truncate pl-1 text-[13.5px] font-semibold" style={{ color: entry.cor }}>
                {entry.nome}
              </span>
              {card.isLive ? <LiveScore entry={entry} /> : null}
              <BandTitles titles={entry.titles} band={card.label} />
            </button>
          </div>
        );
      })}
    </section>
  );
}

// O selo "em andamento" morava aqui, no canto do cabeçalho. Saiu: repetido em
// todo card ele virava mais um adorno, e ainda comia a largura do título — nome
// de campeonato longo chegava truncado por causa dele.
//
// O estado "acontecendo agora" continua dito, e por sinais que falam do dado em
// vez de rotulá-lo: o ano corrente no título, a coluna de variação, o placar
// parcial e, no gráfico, a faixa verde da temporada em curso com o traço
// interrompido. A legenda explica esse traço.

// Placar parcial: pontos em mono, do lado do nome. Só existe na tabela ao vivo —
// num ano fechado a posição final já contou a história toda.
function LiveScore({ entry }) {
  const { t } = useTranslation();
  if (!Number.isFinite(entry.points)) return <span aria-hidden="true" />;
  return (
    <Tooltip texto={t("globalTeams.liveScoreTitle", { points: entry.points, wins: entry.wins ?? 0 })}>
      <span className="text-right font-mono text-[12px] font-bold tabular-nums text-slate-300">
        {entry.points}
      </span>
    </Tooltip>
  );
}

// Variação contra a última temporada DECIDIDA — não contra o ano anterior no
// calendário. Equipe que não estava nesta divisão ano passado não tem variação:
// leva o mesmo círculo vazado que a legenda chama de estreia.
function DeltaGlyph({ entry, baselineYear, referenceYear }) {
  const { t } = useTranslation();
  if (entry.isNewInBand) {
    return (
      <Tooltip texto={t("globalTeams.liveNewInBandTitle", { year: referenceYear })}>
        <span className="text-center text-[10px] leading-none text-slate-400">○</span>
      </Tooltip>
    );
  }
  if (!Number.isFinite(entry.delta)) return <span aria-hidden="true" />;
  if (entry.delta === 0) {
    return (
      <Tooltip texto={t("globalTeams.liveDeltaSameTitle", { year: baselineYear })}>
        <span className="text-center text-[10px] leading-none text-slate-600">–</span>
      </Tooltip>
    );
  }
  const up = entry.delta > 0;
  return (
    <Tooltip
      texto={t(up ? "globalTeams.liveDeltaUpTitle" : "globalTeams.liveDeltaDownTitle", {
        count: Math.abs(entry.delta),
        year: baselineYear,
      })}
    >
      <span
        data-testid={`atlas-v2-delta-${entry.team_id}`}
        className={`text-center text-[9px] leading-none ${up ? "text-status-green" : "text-status-red"}`}
      >
        {up ? "▲" : "▼"}
      </span>
    </Tooltip>
  );
}

// Troféus DA CATEGORIA em que a equipe está agora — a conta vem de `bandTitles`.
// Sem título nesta divisão, nada é desenhado: a coluna vazia diz "ainda não ganhou
// aqui" melhor do que um traço, que lia como variação de posição.
function BandTitles({ titles, band }) {
  const { t } = useTranslation();
  if (!titles) return <span aria-hidden="true" />;
  return (
    <Tooltip texto={t("globalTeams.bandTitlesCount", { count: titles, band })}>
      <span className="flex items-center justify-end gap-1 text-[11.5px] font-bold text-[#f2c46d]">
        <img src={goldTrophy} alt="" draggable={false} className="h-3.5 w-3.5 object-contain drop-shadow-[0_0_8px_rgba(242,196,109,0.35)]" />
        {titles}
      </span>
    </Tooltip>
  );
}

// Brasão da categoria, na ponta esquerda do cabeçalho. O troféu genérico que
// ficava aqui dizia só "campeonato" — coisa que o título já diz. O brasão é
// reconhecimento imediato: o jogador identifica a divisão pela marca antes de ler
// o nome dela.
//
// Não é o alvo de clique: o alvo é a faixa inteira. Ele cresce junto com o hover
// do cabeçalho, então continua respondendo ao mesmo gesto.
//
// A moldura tem tamanho fixo — é ela que mantém os títulos dos cards alinhados —,
// mas o tamanho DENTRO dela é medido, não arbitrado: cada brasão é escalado para
// que o conteúdo visível ocupe a mesma área dos outros, seja escudo ou letreiro.
// Ver `atlasLogoNormalization.js`, que explica por que área e não altura.
//
// Enquanto a medida não chega — e ela nunca chega fora de um navegador de verdade —
// vale o encaixe simples por `object-contain`. Não é o ideal, é o que não quebra.
//
// Só que o encaixe simples é MAIOR que o normalizado, então mostrá-lo enquanto se
// espera dá um salto de tamanho assim que a medida chega. Por isso o brasão fica
// invisível até haver resposta: ou a medida, ou a confirmação de que não dá para
// medir. Falha de carga também conta como resposta, senão o brasão sumiria de vez.
function CategoryLogo({ category, accent }) {
  const src = atlasCategoryLogo(category);
  const { box, measured } = useNormalizedLogo(src);
  const layout = normalizedLogoLayout(box);

  // Categoria sem brasão mapeado: um ponto na cor do campeonato mantém o ritmo do
  // cabeçalho sem inventar um símbolo que não existe.
  if (!src) {
    return (
      <span
        aria-hidden="true"
        data-testid="atlas-v2-category-logo-fallback"
        className="h-2 w-2 shrink-0 rounded-full"
        style={{ background: accent }}
      />
    );
  }
  return (
    <span
      aria-hidden="true"
      data-testid={`atlas-v2-category-logo-frame-${category}`}
      // `overflow-hidden` só corta padding transparente: a caixa opaca é limitada
      // à moldura pelo próprio cálculo, então nada de visível chega na borda.
      className="relative grid shrink-0 place-items-center overflow-hidden transition-transform group-hover:scale-110"
      style={{ width: LOGO_FRAME_WIDTH, height: LOGO_FRAME_HEIGHT }}
    >
      <img
        src={src}
        alt=""
        draggable={false}
        data-testid={`atlas-v2-category-logo-${category}`}
        className={[
          layout ? "absolute max-w-none" : "max-h-full max-w-full object-contain",
          measured ? "" : "invisible",
        ].join(" ").trim()}
        style={layout ?? undefined}
      />
    </span>
  );
}

export default AtlasRankings;
