import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Anchor,
  Ban,
  ChevronsUp,
  CircleCheck,
  CloudRain,
  Crosshair,
  Crown,
  Dices,
  Flame,
  Medal,
  Rocket,
  Shuffle,
  Sparkles,
  Star,
  Swords,
  Target,
  Timer,
  TrendingDown,
  TrendingUp,
  Trophy,
  Users,
  Zap,
} from "lucide-react";
import i18n from "../../i18n/index.js";
import useCareerStore from "../../stores/useCareerStore";
import FlagIcon from "../ui/FlagIcon";
import TeamLogoMark from "../team/TeamLogoMark";
import { renderTextWithDriverMentions } from "../../utils/driverMentions";
import { extractNationalityLabel, formatCategoryName } from "../../utils/formatters";
import "./SeasonChampionOverlay.css";

// Pop-up "Campeão da Temporada": celebra o campeão da categoria do jogador ao fim
// da temporada, sobre a home. Montado como host global no App, aparece sempre que o
// store tiver `championOverlay` — payload real vindo de `get_season_champion_payload`
// (ver commands/career/champion.rs). Nada aqui é dado de exemplo: prêmios e recordes
// que a temporada não produziu simplesmente não vêm na lista.

// Proporção larga de propósito: o SVG escala pela largura (`height:auto`), então um
// viewBox quadrado demais viraria um gráfico de meia tela de altura no modal cheio.
const CHART = { W: 1120, H: 300, padL: 42, padR: 150, padT: 14, padB: 30 };

// Nome de equipe é bem mais longo que sobrenome de piloto ("Grid Start Racing
// School" contra "Fortin"), então a curva dos construtores cede mais margem direita
// para o rótulo do fim da linha não sair pela borda do SVG.
const TEAM_CHART_PAD_R = 250;

// Quantas equipes entram no gráfico de construtores. Mais que isso vira novelo — a
// classificação embaixo continua mostrando o grid inteiro.
const CHART_TEAMS = 4;

// Ícone de cada recorde/prêmio. Mora AQUI, não no payload: o backend manda o fato,
// a escolha do desenho é da tela. Um id sem entrada cai no genérico.
const RECORD_ICONS = {
  vitorias: Trophy,
  poles: Timer,
  podios: Medal,
  voltas_rapidas: Zap,
  posicoes_ganhas: TrendingUp,
  posicoes_perdidas: TrendingDown,
  rei_da_chuva: CloudRain,
  abandonos: Ban,
  mais_chegadas: CircleCheck,
  vitorias_da_pole: Rocket,
  sequencia_vitorias: Flame,
  etapas_na_lideranca: Crown,
  maior_recuperacao: ChevronsUp,
  melhor_etapa: Sparkles,
  melhor_media_chegada: Target,
  melhor_media_grid: Crosshair,
};

const AWARD_ICONS = {
  grand_chelem: Trophy,
  zebra: Dices,
  duelo: Swords,
  revelacao: Star,
  regularidade: Anchor,
  virada: TrendingUp,
  etapa_do_ano: Shuffle,
  equipe_do_ano: Users,
};

// Quantos prêmios ganham cartão grande. O resto desce para a faixa miúda embaixo —
// mesmo arranjo do pódio com a classificação: o destaque em cima, a lista completa
// e discreta em baixo. Quatro é o que fecha duas linhas cheias de dois.
const HERO_AWARDS = 4;

// Cores do realce de menção de piloto, na paleta do pop-up (o padrão do util é o azul
// do resto do app). Ver utils/driverMentions — mesmo sistema da revista de notícias.
const MENTION_CLASSES = {
  activeClass: "text-[#12E0E0]",
  idleClass: "hover:text-[#12E0E0]",
};

// Cores das linhas: ouro para o campeão, ciano para o jogador, cinza/bronze para os
// demais, na ordem da classificação.
const NEUTRAL_LINE_COLORS = ["#9aa6b4", "#c8895a", "#5b6b7d", "#4a5666"];

function lineColor(driver, index) {
  if (driver.is_champion) return "#F4C752";
  if (driver.is_player) return "#12E0E0";
  return NEUTRAL_LINE_COLORS[Math.max(0, index - 1)] ?? "#5b6b7d";
}

// Equipe tem cor própria (a mesma do escudo e do resto do app), então a curva usa a
// identidade dela. O ouro/ciano só entram quando a equipe não tem cor cadastrada,
// para o líder e a equipe do jogador continuarem legíveis no meio do novelo.
function teamLineColor(team, index) {
  if (team.cor) return team.cor;
  if (team.posicao === 1) return "#F4C752";
  if (team.is_player_team) return "#12E0E0";
  return NEUTRAL_LINE_COLORS[Math.max(0, index - 1)] ?? "#5b6b7d";
}

function medal(index) {
  return index === 0 ? "co-b1" : index === 1 ? "co-b2" : "co-b3";
}

function initials(name) {
  return (name ?? "")
    .replace(/\./g, "")
    .split(" ")
    .filter(Boolean)
    .map((word) => word[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
}

function lastName(name) {
  const parts = (name ?? "").trim().split(/\s+/).filter(Boolean);
  return parts[parts.length - 1] ?? name ?? "";
}

// Rótulo de equipe no fim da curva. Nome inteiro só cabe até certo ponto — depois
// disso corta, porque um "Grid Start Racing School" empurra o número para fora.
function shortTeam(name) {
  const clean = (name ?? "").trim();
  return clean.length > 20 ? `${clean.slice(0, 19)}…` : clean;
}

// O backend manda os valores de interpolação como texto; o i18next só pluraliza
// com `count` NUMÉRICO, então os que são número voltam a ser número aqui.
function interpolationArgs(args) {
  const parsed = {};
  for (const [key, value] of Object.entries(args ?? {})) {
    parsed[key] = /^-?\d+(\.\d+)?$/.test(value) ? Number(value) : value;
  }
  return parsed;
}

function formatPoints(value) {
  const number = Number(value) || 0;
  return Number.isInteger(number) ? String(number) : number.toFixed(1);
}

// O gráfico é o mesmo dos dois campeonatos: recebe SÉRIES já resolvidas (cor, rótulo,
// curva) e não sabe se a linha é de um piloto ou de uma equipe.
function PointsChart({ series, rounds, padR = CHART.padR, ariaLabel }) {
  const { t } = useTranslation();
  const { W, H, padL, padT, padB } = CHART;

  const totals = series.map((entry) => Number(entry.total) || 0);
  const ymax = Math.max(50, Math.ceil(Math.max(...totals, 1) / 50) * 50);
  const lastIndex = Math.max(rounds - 1, 1);
  const X = (index) => padL + (index / lastIndex) * (W - padL - padR);
  const Y = (value) => padT + (1 - value / ymax) * (H - padT - padB);

  // Líder e jogador desenhados por último, para ficarem por cima das outras linhas.
  const ordered = series
    .map((entry, index) => ({ ...entry, index }))
    .sort((a, b) => (a.highlight ? 1 : 0) - (b.highlight ? 1 : 0));

  const gridValues = [0, 1, 2, 3, 4].map((step) => (ymax * step) / 4);
  // No máximo 5 marcas no eixo X, sempre incluindo a primeira e a última etapa.
  const tickStep = Math.max(1, Math.ceil(rounds / 5));
  const xTicks = [];
  for (let round = 1; round <= rounds; round += tickStep) {
    xTicks.push(round);
  }
  if (xTicks[xTicks.length - 1] !== rounds) {
    xTicks.push(rounds);
  }

  return (
    <svg
      className="co-chart"
      viewBox={`0 0 ${W} ${H}`}
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label={ariaLabel ?? t("seasonChampion.chart.ariaLabel")}
    >
      {gridValues.map((value) => {
        const y = Y(value);
        return (
          <g key={`g${value}`}>
            <line x1={padL} y1={y} x2={W - padR} y2={y} stroke="rgba(255,255,255,0.07)" strokeWidth="1" />
            <text x={padL - 8} y={y + 3.5} fill="#6f7c8b" fontSize="11" textAnchor="end" fontFamily="inherit">
              {Math.round(value)}
            </text>
          </g>
        );
      })}
      {xTicks.map((round) => (
        <text
          key={`x${round}`}
          x={X(round - 1)}
          y={H - 10}
          fill="#6f7c8b"
          fontSize="11"
          textAnchor="middle"
          fontFamily="inherit"
        >
          {t("seasonChampion.chart.stageTick", { n: round })}
        </text>
      ))}
      {ordered.map((entry) => {
        const { color, highlight } = entry;
        const curve = entry.cumulative ?? [];
        const points = curve.map((value, index) => `${X(index).toFixed(1)},${Y(value).toFixed(1)}`).join(" ");
        const endX = X(curve.length - 1);
        const endY = Y(curve[curve.length - 1] ?? 0);
        return (
          <g key={entry.id}>
            <polyline
              points={points}
              fill="none"
              stroke={color}
              strokeWidth={highlight ? 3.4 : 2}
              strokeLinejoin="round"
              strokeLinecap="round"
              opacity={highlight ? 1 : 0.8}
            />
            <circle cx={endX} cy={endY} r={highlight ? 4.5 : 3.3} fill={color} />
            <text
              x={endX + 8}
              y={endY + 4}
              fill={color}
              fontSize={highlight ? 13 : 12}
              fontWeight={highlight ? 700 : 500}
              fontFamily="inherit"
            >
              {`${entry.label} ${formatPoints(entry.total)}`}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

function SeasonChampionOverlay() {
  const { t } = useTranslation();
  const championOverlay = useCareerStore((s) => s.championOverlay);
  const hideChampionOverlay = useCareerStore((s) => s.hideChampionOverlay);
  // Piloto sob o mouse numa menção (prêmio, recorde). Acende a posição dele no pódio
  // ou na lista miúda de classificação — o mesmo sistema da revista de notícias.
  const [hoveredDriverId, setHoveredDriverId] = useState(null);
  // Qual campeonato está na tela. Dois quadros muito diferentes disputando a mesma
  // rolagem viravam uma tela só longa demais; separados, cada um pode respirar.
  const [view, setView] = useState("drivers");
  const closeOverlay = useCallback(() => {
    hideChampionOverlay();
  }, [hideChampionOverlay]);

  // Cada temporada abre no quadro de pilotos: é o campeonato do jogador.
  useEffect(() => {
    setView("drivers");
  }, [championOverlay]);

  useEffect(() => {
    if (!championOverlay) return undefined;
    const onKey = (e) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopImmediatePropagation();
      closeOverlay();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [championOverlay, closeOverlay]);

  const drivers = useMemo(() => championOverlay?.drivers ?? [], [championOverlay]);
  const standings = useMemo(() => championOverlay?.standings ?? [], [championOverlay]);
  // Só id + nome: é o que o matcher de menções precisa para reconhecer os nomes
  // citados nos prêmios e recordes.
  const mentionDrivers = useMemo(
    () => standings.map((row) => ({ id: row.id, nome: row.nome })),
    [standings],
  );
  const mention = useCallback(
    (text) =>
      renderTextWithDriverMentions(
        text,
        mentionDrivers,
        hoveredDriverId,
        setHoveredDriverId,
        MENTION_CLASSES,
      ),
    [mentionDrivers, hoveredDriverId],
  );

  if (!championOverlay || drivers.length === 0) return null;

  const champion = drivers.find((driver) => driver.is_champion) ?? drivers[0];
  const categoryLabel = formatCategoryName(championOverlay.category_id);
  // A pílula traz o nome completo do campeonato; o subtítulo usa só a sigla.
  const seriesLabel = categoryLabel.split(" ")[0];
  const top3 = drivers.slice(0, 3).map((driver, index) => ({ ...driver, index }));
  // Pódio na disposição visual clássica: 2º à esquerda, 1º no meio, 3º à direita.
  const podium = [top3[1], top3[0], top3[2]].filter(Boolean);
  const discBg = ["var(--gold)", "#d7dee8", "#c8895a"];
  const margin = Math.round(Number(championOverlay.margin) || 0);
  // Prêmio do jogador vem primeiro: a tela é da carreira dele, e o momento dele não
  // pode cair na faixa miúda porque a temporada rendeu muita menção. Fora isso, a
  // ordem é a que o backend autorou (do mais raro para o mais circunstancial).
  const awards = [...(championOverlay.awards ?? [])].sort(
    (a, b) => (b.is_player ? 1 : 0) - (a.is_player ? 1 : 0),
  );
  const heroAwards = awards.slice(0, HERO_AWARDS);
  const restAwards = awards.slice(HERO_AWARDS);
  // Equipe e posição de quem levou o prêmio — só nos que falam de UM piloto (os de
  // dupla, pista e equipe vêm sem `who_id`).
  const awardContext = (award) => {
    const row = award.who_id ? standings.find((entry) => entry.id === award.who_id) : null;
    if (!row) return null;
    return row.equipe
      ? t("seasonChampion.awards.context", { team: row.equipe, position: row.posicao })
      : t("seasonChampion.awards.contextNoTeam", { position: row.posicao });
  };
  // Gentílico sem o emoji ("Canadense"), quando a nacionalidade traz os dois.
  const nationality = extractNationalityLabel(champion.nacionalidade);
  // A campanha do campeão. `rounds` fecha o conjunto dando a escala: 4 vitórias em 5
  // etapas e 4 em 22 são campeonatos diferentes.
  const championStats = [
    { key: "wins", value: champion.vitorias },
    { key: "podiums", value: champion.podios },
    { key: "poles", value: champion.poles },
    { key: "fastestLaps", value: champion.voltas_rapidas },
    { key: "rounds", value: championOverlay.rounds },
  ].filter((stat) => Number.isFinite(Number(stat.value)));
  const constructors = championOverlay.constructors ?? [];
  const records = championOverlay.records ?? [];
  // Do 4º para baixo: o pódio já mostra os três primeiros.
  const restOfField = standings.slice(3);

  // Séries do gráfico: a curva de pilotos e a de construtores entram no MESMO
  // desenho, já resolvidas em cor e rótulo.
  const driverSeries = drivers.map((driver, index) => ({
    id: driver.id,
    label: lastName(driver.nome),
    total: driver.pontos,
    cumulative: driver.cumulative,
    color: lineColor(driver, index),
    highlight: driver.is_champion || driver.is_player,
  }));

  const championTeam = constructors[0];
  const teamSeries = constructors.slice(0, CHART_TEAMS).map((team, index) => ({
    id: team.nome,
    label: shortTeam(team.nome),
    total: team.pontos,
    cumulative: team.cumulative ?? [],
    color: teamLineColor(team, index),
    highlight: team.posicao === 1 || team.is_player_team,
  }));
  // Vantagem da campeã para a vice — o mesmo fecho de conta do quadro de pilotos.
  const teamMargin = Math.round(
    Math.max(0, (championTeam?.pontos ?? 0) - (constructors[1]?.pontos ?? 0)),
  );
  // Escala da barra de cada linha: a campeã enche, o resto é fração dela.
  const teamPointsMax = Math.max(...constructors.map((team) => Number(team.pontos) || 0), 1);
  const championTeamStats = championTeam
    ? [
        { key: "wins", value: championTeam.vitorias },
        { key: "podiums", value: championTeam.podios },
        { key: "poles", value: championTeam.poles },
        { key: "fastestLaps", value: championTeam.voltas_rapidas },
        { key: "rounds", value: championOverlay.rounds },
      ].filter((stat) => Number.isFinite(Number(stat.value)))
    : [];
  // Sem construtores no payload (temporada antiga, categoria sem equipe nomeada) o
  // alternador some: não há segundo quadro para ir.
  const hasConstructors = constructors.length > 0;
  const showingTeams = hasConstructors && view === "teams";

  return (
    <div className="champ-ov" onClick={closeOverlay}>
      <div
        className="co-modal"
        role="dialog"
        aria-label={t("seasonChampion.dialogLabel")}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="co-head">
          <div className="co-lhs">
            <span className="co-pulse"><span className="a" /><span className="b" /></span>
            <div>
              <h3>{t("seasonChampion.head.title", { year: championOverlay.year })}</h3>
              <div className="sub">
                {championOverlay.player_is_champion
                  ? t("seasonChampion.head.subtitleYou", { series: seriesLabel })
                  : t("seasonChampion.head.subtitleOther", { series: seriesLabel })}
              </div>
            </div>
          </div>
          <div className="co-headright">
            <span className="co-catpill">{categoryLabel}</span>
            <button className="co-x" type="button" onClick={closeOverlay}>{t("seasonChampion.close")}</button>
          </div>
        </div>

        <div className="co-body">
          {/* Alternador dos dois campeonatos. Fica no topo do corpo, e não no
              cabeçalho, porque ele troca o CONTEÚDO da rolagem — o cabeçalho fala da
              temporada, que é a mesma nos dois lados. */}
          {hasConstructors && (
            <div className="co-switch" role="tablist" aria-label={t("seasonChampion.tabs.label")}>
              <button
                type="button"
                role="tab"
                aria-selected={!showingTeams}
                className={showingTeams ? "" : "on"}
                onClick={() => setView("drivers")}
              >
                <Trophy size={14} strokeWidth={2.2} />
                {t("seasonChampion.tabs.drivers")}
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={showingTeams}
                className={showingTeams ? "on" : ""}
                onClick={() => setView("teams")}
              >
                <Users size={14} strokeWidth={2.2} />
                {t("seasonChampion.tabs.constructors")}
              </button>
            </div>
          )}

          {!showingTeams && (
            <>
          <div className="co-hero">
            <div className="co-champ">
              <div className="co-champid">
                {/* O troféu entra na linha do rótulo: sozinho numa linha própria ele
                    só empurrava o nome para baixo e sobrava vazio dos dois lados. */}
                <div className="co-kicker">
                  <Trophy size={17} strokeWidth={2} />
                  {t("seasonChampion.hero.kicker")}
                </div>
                <div className="co-cname">{champion.nome}</div>
                <div className="co-meta">
                  {/* Bandeira de verdade pelo componente da casa (PNG por código ISO),
                      e não o emoji: a fonte do Windows não desenha os indicadores
                      regionais e o campeão aparecia com "CA" escrito ao lado. */}
                  <FlagIcon nacionalidade={champion.nacionalidade} className="co-flag" />
                  {nationality && <span className="co-nat">{nationality}</span>}
                  {champion.equipe && (
                    <span className="co-team">
                      {t("seasonChampion.hero.forTeam")} <b>{champion.equipe}</b>
                    </span>
                  )}
                  {champion.is_player && <span className="co-youbadge">{t("seasonChampion.hero.youExclaim")}</span>}
                </div>
                <div className="co-clinch">
                  ✦ {t("seasonChampion.hero.clinchPre")}{" "}
                  <b>{t("seasonChampion.hero.clinchPoints", { count: margin })}</b>{" "}
                  {t("seasonChampion.hero.clinchPost")}
                </div>
              </div>
              {/* O escudo da equipe campeã, no vão entre o nome e os números. */}
              {champion.equipe && (
                <div className="co-crest">
                  <TeamLogoMark
                    teamName={champion.equipe}
                    color={champion.equipe_cor}
                    size="lg"
                    halo
                    testId="season-champion-team-logo"
                  />
                </div>
              )}
              {/* A campanha do campeão em números, no vazio que sobrava à direita do
                  nome: conta COMO o título foi ganho, não só que foi. */}
              <div className="co-champrun">
                <div className="co-bigpts">
                  <div className="v">{formatPoints(champion.pontos)}</div>
                  <div className="l">{t("seasonChampion.hero.pointsLabel")}</div>
                </div>
                <div className="co-runstats">
                  {championStats.map((stat) => (
                    <div className="co-runstat" key={stat.key}>
                      <span className="n">{stat.value}</span>
                      <span className="k">{t(`seasonChampion.hero.stats.${stat.key}`)}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>

          <div className="co-card">
            <h4>{t("seasonChampion.chart.title")}</h4>
            <div className="co-chartwrap">
              <PointsChart series={driverSeries} rounds={championOverlay.rounds} />
            </div>
            <div className="co-legend">
              {drivers.map((driver, index) => (
                <span key={driver.id}>
                  <i style={{ background: lineColor(driver, index) }} />
                  {driver.nome}
                  {driver.is_champion ? (
                    <Trophy size={13} strokeWidth={2.2} className="co-legend-crown" />
                  ) : null}
                  {" · "}
                  {t("seasonChampion.points", { count: Math.round(Number(driver.pontos) || 0) })}
                </span>
              ))}
            </div>
          </div>

          <div className="co-lower">
            <div className="co-card">
              <h4>{t("seasonChampion.podium.title")}</h4>
              <div className="co-podium">
                {podium.map((driver) => {
                  const lit = hoveredDriverId === driver.id;
                  return (
                    <div
                      className={`co-step${driver.is_player ? " you" : ""}${lit ? " lit" : ""}`}
                      key={driver.id}
                      onMouseEnter={() => setHoveredDriverId(driver.id)}
                      onMouseLeave={() => setHoveredDriverId(null)}
                    >
                      <div className="co-disc" style={{ background: discBg[driver.index] }}>
                        {initials(driver.nome)}
                      </div>
                      <div className="co-who">
                        {driver.nome}
                        <small>
                          {driver.equipe ? `${driver.equipe} · ` : ""}
                          {t("seasonChampion.points", {
                            count: Math.round(Number(driver.pontos) || 0),
                          })}
                        </small>
                      </div>
                      <div className={`co-bar ${medal(driver.index)}`}>
                        <span className="co-rankn">{driver.index + 1}</span>
                      </div>
                    </div>
                  );
                })}
              </div>
              {/* Classificação do 4º para baixo, miúda: sem ela, passar o mouse num
                  piloto citado num prêmio não teria posição nenhuma para acender. Em
                  colunas (não em linhas que embrulham) para ler de cima para baixo
                  como classificação, e não como um bloco de texto corrido. */}
              {restOfField.length > 0 && (
                <div className="co-rest">
                  {restOfField.map((row) => {
                    const lit = hoveredDriverId === row.id;
                    return (
                      <span
                        className={`co-restrow${row.is_player ? " you" : ""}${lit ? " lit" : ""}`}
                        key={row.id}
                        onMouseEnter={() => setHoveredDriverId(row.id)}
                        onMouseLeave={() => setHoveredDriverId(null)}
                      >
                        <b>{row.posicao}</b>
                        <i className="cor" style={{ background: row.equipe_cor || "#3a4553" }} />
                        <span className="nm">{row.nome}</span>
                        <i className="pts">{Math.round(Number(row.pontos) || 0)}</i>
                      </span>
                    );
                  })}
                </div>
              )}
            </div>

            {awards.length > 0 && (
              <div className="co-card">
                <h4>{t("seasonChampion.awards.title")}</h4>
                <div className="co-awards">
                  {heroAwards.map((award) => {
                    const Icon = AWARD_ICONS[award.id] ?? Sparkles;
                    const context = awardContext(award);
                    return (
                      <div
                        className={`co-award${award.is_player ? " you" : ""}`}
                        key={award.id}
                      >
                        <span className="ic"><Icon size={25} strokeWidth={1.9} /></span>
                        <div className="txt">
                          <div className="t">{t(`seasonChampion.awards.items.${award.id}.title`)}</div>
                          {/* `who` pode ser um piloto, uma dupla ("A × B") ou uma pista;
                              o matcher só realça o que reconhece como nome de piloto. */}
                          <div className="who">{mention(award.who)}</div>
                          {context ? <div className="ctx">{context}</div> : null}
                          <div className="d">
                            {mention(
                              t(
                                `seasonChampion.awards.items.${award.id}.detail`,
                                interpolationArgs(award.args),
                              ),
                            )}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
                {/* Os prêmios que não couberam no destaque continuam na tela, em uma
                    linha só cada — nenhum fato da temporada é escondido. */}
                {restAwards.length > 0 && (
                  <div className="co-awardrest">
                    {restAwards.map((award) => {
                      const Icon = AWARD_ICONS[award.id] ?? Sparkles;
                      return (
                        <span
                          className={`co-awardmini${award.is_player ? " you" : ""}`}
                          key={award.id}
                        >
                          <Icon size={13} strokeWidth={2} />
                          <b>{t(`seasonChampion.awards.items.${award.id}.title`)}</b>
                          <span className="w">{mention(award.who)}</span>
                        </span>
                      );
                    })}
                  </div>
                )}
              </div>
            )}
          </div>

          {records.length > 0 && (
            <div className="co-card">
              <h4>{t("seasonChampion.records.title")}</h4>
              <div className="co-supers">
                {records.map((record) => {
                  // Unidade fixa e curta ("vit.", "DNF") — só alguns recordes têm, e
                  // `i18n.exists` é a checagem confiável: `t()` com defaultValue vazio
                  // devolveria o NOME DA CHAVE, porque "" é falsy para o i18next.
                  const unitKey = `seasonChampion.records.items.${record.id}.unit`;
                  const unit = i18n.exists(unitKey) ? t(unitKey) : "";
                  const Icon = RECORD_ICONS[record.id] ?? Sparkles;
                  return (
                    <div className={`co-super${record.is_player ? " you" : ""}`} key={record.id}>
                      <span className="ic"><Icon size={16} strokeWidth={2} /></span>
                      <div className="txt">
                        <div className="lab">{t(`seasonChampion.records.items.${record.id}.label`)}</div>
                        <div className="nm">{mention(record.who)}</div>
                        {/* Contexto do recorde (a pista) em linha própria: nome de pista
                            ao lado do número estourava o cartão e comia o do piloto. */}
                        {record.sufixo ? <div className="ctx">{record.sufixo}</div> : null}
                      </div>
                      <div className="val">
                        {record.valor}
                        {unit ? <small>{unit}</small> : null}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
            </>
          )}

          {/* Campeonato de construtores: a mesma temporada contada pelo outro lado.
              Sai da soma dos mesmos resultados do quadro de pilotos, então os dois
              não têm como discordar. */}
          {showingTeams && championTeam && (
            <>
              <div className="co-hero">
                <div className="co-champ">
                  <div className="co-champid">
                    <div className="co-kicker">
                      <Users size={17} strokeWidth={2} />
                      {t("seasonChampion.constructors.kicker")}
                    </div>
                    <div className="co-cname">{championTeam.nome}</div>
                    <div className="co-meta">
                      {/* A dupla (ou o rodízio) que fez os pontos. É o que uma equipe
                          tem no lugar da nacionalidade de um piloto. */}
                      <span className="co-lineuplabel">{t("seasonChampion.constructors.lineup")}</span>
                      {(championTeam.pilotos ?? []).map((piloto) => (
                        <span
                          className={`co-lineup${piloto.is_player ? " you" : ""}`}
                          key={piloto.id}
                        >
                          {piloto.nome}
                        </span>
                      ))}
                      {championTeam.is_player_team && (
                        <span className="co-youbadge">{t("seasonChampion.constructors.yourTeam")}</span>
                      )}
                    </div>
                    <div className="co-clinch">
                      ✦ {t("seasonChampion.constructors.clinchPre")}{" "}
                      <b>{t("seasonChampion.hero.clinchPoints", { count: teamMargin })}</b>{" "}
                      {t("seasonChampion.constructors.clinchPost")}
                    </div>
                  </div>
                  <div className="co-crest">
                    <TeamLogoMark
                      teamName={championTeam.nome}
                      color={championTeam.cor}
                      size="lg"
                      halo
                      testId="season-constructor-champion-logo"
                    />
                  </div>
                  <div className="co-champrun">
                    <div className="co-bigpts">
                      <div className="v">{formatPoints(championTeam.pontos)}</div>
                      <div className="l">{t("seasonChampion.hero.pointsLabel")}</div>
                    </div>
                    <div className="co-runstats">
                      {championTeamStats.map((stat) => (
                        <div className="co-runstat" key={stat.key}>
                          <span className="n">{stat.value}</span>
                          <span className="k">{t(`seasonChampion.hero.stats.${stat.key}`)}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              </div>

              <div className="co-card">
                <h4>{t("seasonChampion.constructors.chartTitle")}</h4>
                <div className="co-chartwrap">
                  <PointsChart
                    series={teamSeries}
                    rounds={championOverlay.rounds}
                    padR={TEAM_CHART_PAD_R}
                    ariaLabel={t("seasonChampion.constructors.chartAria")}
                  />
                </div>
                <div className="co-legend">
                  {constructors.slice(0, CHART_TEAMS).map((team, index) => (
                    <span key={team.nome}>
                      <i style={{ background: teamLineColor(team, index) }} />
                      {team.nome}
                      {team.posicao === 1 ? (
                        <Trophy size={13} strokeWidth={2.2} className="co-legend-crown" />
                      ) : null}
                      {" · "}
                      {t("seasonChampion.points", { count: Math.round(Number(team.pontos) || 0) })}
                    </span>
                  ))}
                </div>
              </div>

              {/* A classificação inteira, uma equipe por linha larga: escudo, quem
                  pontuou por ela e a campanha em números. Antes eram cartões de duas
                  linhas em grade, e o campeonato de construtores virava rodapé. */}
              <div className="co-card">
                <h4>{t("seasonChampion.constructors.title")}</h4>
                <div className="co-ctable">
                  {constructors.map((team) => {
                    const share = Math.max(
                      2,
                      Math.round(((Number(team.pontos) || 0) / teamPointsMax) * 100),
                    );
                    const color = teamLineColor(team, team.posicao - 1);
                    const teamStats = [
                      { key: "wins", value: team.vitorias },
                      { key: "podiums", value: team.podios },
                      { key: "poles", value: team.poles },
                      { key: "fastestLaps", value: team.voltas_rapidas },
                    ];
                    return (
                      <div
                        className={`co-crow${team.posicao === 1 ? " lead" : ""}${team.is_player_team ? " you" : ""}`}
                        key={team.nome}
                      >
                        <span className="pos">{team.posicao}</span>
                        <TeamLogoMark teamName={team.nome} color={team.cor} size="sm" halo />
                        <div className="who">
                          <div className="nm">
                            {team.nome}
                            {/* Zero vem por outra chave: em pt-BR o plural do i18next
                                põe 0 no singular, e "0 vitória" está errado. */}
                            <small>
                              {team.vitorias > 0
                                ? t("seasonChampion.constructors.wins", { count: team.vitorias })
                                : t("seasonChampion.constructors.noWins")}
                            </small>
                          </div>
                          <div className="co-cbar">
                            <i style={{ width: `${share}%`, background: color }} />
                          </div>
                          <div className="co-cdrv">
                            {(team.pilotos ?? []).map((piloto) => (
                              <span
                                className={piloto.is_player ? "you" : ""}
                                key={piloto.id}
                              >
                                {piloto.nome}
                                <i>{formatPoints(piloto.pontos)}</i>
                              </span>
                            ))}
                          </div>
                        </div>
                        <div className="co-cstats">
                          {teamStats.map((stat) => (
                            <span className="co-cstat" key={stat.key}>
                              <b>{stat.value}</b>
                              <small>{t(`seasonChampion.constructors.stats.${stat.key}`)}</small>
                            </span>
                          ))}
                        </div>
                        <div className="co-cpts">
                          <span className="v">{formatPoints(team.pontos)}</span>
                          <small>{t("seasonChampion.hero.pointsLabel")}</small>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            </>
          )}
        </div>

        <div className="co-foot">
          <span className="note">
            {showingTeams ? t("seasonChampion.footer.noteTeams") : t("seasonChampion.footer.note")}
          </span>
          <div className="co-btns">
            <button className="co-btn primary" type="button" onClick={closeOverlay}>
              {t("seasonChampion.footer.continue")} ▶
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default SeasonChampionOverlay;
