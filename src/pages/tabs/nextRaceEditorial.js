// Template DETERMINÍSTICO da prévia pré-corrida (o fallback da Sala de Estratégia
// quando a IA não está disponível: 1ª corrida da carreira/categoria, cooldown do
// servidor, offline, ou gate de engajamento).
//
// Reescrito para ser dirigido pela MESMA tese dominante que monta os `facts` da IA
// (ver `nextRaceThesis.js`). Uma fonte só de verdade: o eixo escolhido decide o
// headline, os dois parágrafos e a voz da equipe. Antes, este arquivo tinha ~930
// linhas de pools combinatórios — e ~2/3 alimentavam campos que a tela NUNCA
// renderiza (scenario, rivalSummary, rivalSupport, paddockSupport, history*,
// weekendStories*). Tudo isso foi removido. A superfície real é: headline, os dois
// parágrafos do corpo, e a citação.

import i18n from "../../i18n/index.js";
import { ordinal } from "../../i18n/format.js";
import { recentResults } from "./nextRaceBriefing";

function hashString(value) {
  const text = String(value ?? "");
  let hash = 0;
  for (let index = 0; index < text.length; index += 1) {
    hash = (hash * 31 + text.charCodeAt(index)) | 0;
  }
  return Math.abs(hash);
}

function pickVariant(variants, seed, context) {
  if (!Array.isArray(variants) || variants.length === 0) {
    return "";
  }
  const selected = variants[hashString(seed) % variants.length];
  return typeof selected === "function" ? selected(context) : selected;
}

function buildSeed(...parts) {
  return parts.filter(Boolean).join("|");
}

// Frase de forma recente (reusada em vários parágrafos como "cauda" factual).
function buildFormSentence(playerStanding) {
  if (!playerStanding) {
    return i18n.t("editorial.form.noStanding");
  }
  const readable = recentResults(playerStanding)
    .map((result) => {
      if (!result) return i18n.t("editorial.form.resultUndefined");
      if (result.is_dnf) return i18n.t("editorial.form.dnf");
      const pos = result.position != null ? ordinal(result.position) : "--";
      return i18n.t("editorial.form.place", { pos });
    })
    .join(", ");
  return readable
    ? i18n.t("editorial.form.recent", { list: readable })
    : i18n.t("editorial.form.noTrend");
}

// Copy voltado ao LEITOR (2ª pessoa, voz do jogo), uma entrada por tese. Cada campo
// tem 2 variantes (chaves i18n `..._0`/`..._1`), escolhidas por seed determinístico
// (mesma etapa ⇒ mesmo texto). `lead` = parágrafo do eixo; `outlook` = o que isso
// significa; `quote` = voz da equipe. As funções chamam i18n.t no render (não
// congelam no idioma do boot); o ctx `c` traz os valores de interpolação.
const V = (baseKey) => [(c) => i18n.t(`${baseKey}_0`, c), (c) => i18n.t(`${baseKey}_1`, c)];

const EDITORIAL_KEYS = [
  "redemption",
  "title_defense",
  "title_chase",
  "nemesis",
  "pressure",
  "weather",
  "track_trauma",
  "track_fortress",
  "breakdown",
  "grand_stage",
  "debut",
  "baseline",
];

export const THESIS_EDITORIAL = Object.fromEntries(
  EDITORIAL_KEYS.map((key) => [
    key,
    {
      headline: V(`editorial.${key}.headline`),
      lead: V(`editorial.${key}.lead`),
      outlook: V(`editorial.${key}.outlook`),
      quote: V(`editorial.${key}.quote`),
    },
  ]),
);

// Monta o copy da Sala de Estratégia a partir da tese dominante já eleita. Devolve
// só o que a tela usa: headline, os dois parágrafos e a citação. `actionHint` é
// mantido como fallback do 2º parágrafo (o render usa `paragraphs[1] || actionHint`).
export function buildEditorialCopy({
  thesis,
  playerStanding,
  leader,
  briefingRival,
  playerTeam,
  nextRace,
  trackHistory,
  gapToLeader = 0,
  remainingRounds = 0,
  nemesisName = null,
  climaLabel = null,
  eventOccasion = null,
}) {
  const key = thesis?.key ?? "baseline";
  const pool = THESIS_EDITORIAL[key] ?? THESIS_EDITORIAL.baseline;
  const ctx = {
    trackName: nextRace?.track_name ?? i18n.t("editorial.ctx.track"),
    teamName: playerTeam?.nome ?? i18n.t("editorial.ctx.team"),
    leaderName: leader?.nome ?? i18n.t("editorial.ctx.leader"),
    rivalName: briefingRival?.driver_name ?? i18n.t("editorial.ctx.rival"),
    nemesisName: nemesisName ?? briefingRival?.driver_name ?? i18n.t("editorial.ctx.nemesis"),
    gapToLeader,
    remainingRounds,
    bestFinish: trackHistory?.best_finish ?? null,
    climaLabel: climaLabel ?? i18n.t("editorial.ctx.clima"),
    eventOccasion: eventOccasion ?? i18n.t("editorial.ctx.occasion"),
    formSentence: buildFormSentence(playerStanding),
  };
  const seed = buildSeed(key, ctx.trackName, ctx.rivalName, playerStanding?.id, nextRace?.rodada);
  const lead = pickVariant(pool.lead, `${seed}|lead`, ctx);
  const outlook = pickVariant(pool.outlook, `${seed}|outlook`, ctx);
  return {
    headline: pickVariant(pool.headline, `${seed}|headline`, ctx),
    paragraphs: [lead, outlook].filter(Boolean),
    quote: pickVariant(pool.quote, `${seed}|quote`, ctx),
    actionHint: outlook,
  };
}

// Estado do campeonato — ainda usado fora do editorial (rótulo de estado nos `facts`
// e sinais da tese). Mantido intacto.
export function classifyChampionshipState({
  playerStanding,
  leader,
  remainingRounds = 0,
  outlook,
  gapBehind,
  championshipUnderway = true,
}) {
  // Abertura de temporada: tabela ainda zerada, sem líder/gaps reais.
  if (!championshipUnderway) {
    return "opener";
  }

  if (!playerStanding || !leader) {
    return "survival";
  }

  if (playerStanding.posicao_campeonato === 1 || outlook?.titleFight === "leader") {
    return "leader";
  }

  if (outlook?.titleFight === "longshot") {
    return "outsider";
  }

  const gapToLeader = Math.max(0, (leader.pontos ?? 0) - (playerStanding.pontos ?? 0));
  if (gapToLeader <= 12 && remainingRounds >= 2) {
    return "chase";
  }

  if (gapBehind != null && gapBehind <= 4) {
    return "pressure";
  }

  return "survival";
}
