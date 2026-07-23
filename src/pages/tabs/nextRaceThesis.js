// A TESE DOMINANTE da prévia pré-corrida.
//
// Antes, o briefing era uma lista plana de ~23 fatos de importância radicalmente
// diferente, e a IA (ou o template) se agarrava no único bloco com carga narrativa
// — normalmente o DNF — e jogava o resto fora. A cura não é somar nem tirar fatos:
// é dar HIERARQUIA. Cada corrida escolhe UM eixo (a "tese"), e todo o resto vira
// apoio ou pano de fundo em torno dele.
//
// `selectThesis` é pura e testável: recebe os sinais já computados no
// `buildBriefingContext` e devolve `{ key, title, statement, support }`. A mesma
// tese alimenta tanto os `facts` enviados ao servidor de IA quanto o headline/lead
// do template determinístico — uma fonte só, sem divergência.
//
// i18n: os `statement` são o EIXO (display fallback E fatos p/ a IA) → saem no idioma
// ativo. Os `{{...}}` são valores computados aqui; o "(s)" é atalho literal (o eixo é
// reescrito pela IA, não precisa de plural gramatical).

import i18n from "../../i18n/index.js";

// Ordem de prioridade: a primeira tese que casar vence. A ordem reflete "carga
// narrativa" — o beat mais quente/pessoal ganha do mais frio/estrutural. É uma
// lista deliberadamente explícita para ser trivial de reordenar.
//
// Calibração: clima ranqueia ACIMA do histórico de pista (quando chove de verdade,
// é o fator dominante da corrida). A `redemption` sobe ao topo, mas com um gatilho
// condicional (ver builder): ela cede à briga de título quando o tropeço não te
// derrubou da ponta.
export const THESIS_PRIORITY = [
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

// Quantas posições abaixo da média recente uma chegada limpa precisa ser para contar
// como "resultado muito abaixo do normal" (gatilho de redenção sem DNF). Relativo ao
// piloto, não um corte absoluto — um P15 só assusta quem costuma andar na frente.
const DISMAL_MARGIN = 8;

// Rótulo curto por tese (i18n `thesis.titles.<key>`) — usado em debug/UI, resolvido
// em selectThesis (não congela no idioma do boot).

function trackLabel(signals) {
  return signals.trackName || i18n.t("thesis.trackFallback");
}

// Cada tese é uma função (signals) → { ok, statement, support } | null.
// `statement` é o EIXO: factual e apontado (o que a história É), não prosa pronta —
// o servidor desenvolve. `support` são os ids de fato que DEVEM subir para a camada
// de APOIO; o resto cai em PANO DE FUNDO.
const THESIS_BUILDERS = {
  // 1) Vem de um tombo recente. O beat mais quente que existe — mas condicional:
  //    "depende do estrago". Se o tropeço NÃO te derrubou da ponta (você segue
  //    líder), ele cede para a defesa do título ("defender após o susto"). Se te
  //    derrubou (segue numa caça/pressão) ou você já está fora da briga, a redenção
  //    é o eixo. Um chase/pressure LIMPO (sem tropeço recente) nem chega aqui.
  redemption(signals) {
    const last = signals.lastResult;
    if (!last) return null;
    const dismal =
      !last.is_dnf &&
      Number.isFinite(last.position) &&
      signals.averageFinish != null &&
      last.position >= signals.averageFinish + DISMAL_MARGIN;
    const hadSetback = last.is_dnf || dismal;
    if (!hadSetback) return null;
    // Ainda na ponta ⇒ o tropeço não custou a liderança ⇒ a história é a DEFESA,
    // não a reação. Cede.
    if (signals.playerIsLeader) return null;
    const detail = last.is_dnf
      ? i18n.t("thesis.redemption.detailDnf", { track: last.trackName ? ` (${last.trackName})` : "" })
      : i18n.t("thesis.redemption.detailDismal", { pos: last.position });
    return {
      statement: i18n.t("thesis.redemption.statement", { detail }),
      support: ["recent_form", "injury", "championship_situation", "objective", "track_history"],
    };
  },

  // 2) Líder do campeonato. Peso de quem tem algo a perder. Se vier logo depois de
  //    um DNF (mas segurou a ponta), a defesa fica ainda mais delicada.
  title_defense(signals) {
    if (!signals.championshipUnderway || !signals.playerIsLeader) return null;
    const chaser =
      signals.gapBehind != null ? i18n.t("thesis.title_defense.chaser", { gap: signals.gapBehind }) : "";
    const setback = signals.lastResult?.is_dnf ? i18n.t("thesis.title_defense.setback") : "";
    return {
      statement: i18n.t("thesis.title_defense.statement", { track: trackLabel(signals), setback, chaser }),
      support: ["chaser", "pressure", "rival_direct", "objective", "constructors"],
    };
  },

  // 3) Perto do líder, com etapas para virar. A conta do título ainda vive.
  title_chase(signals) {
    if (signals.championshipState !== "chase") return null;
    const leader = signals.leaderName ? i18n.t("thesis.title_chase.leaderName", { name: signals.leaderName }) : "";
    return {
      statement: i18n.t("thesis.title_chase.statement", {
        gap: signals.gapToLeader,
        leader,
        rounds: signals.remainingRounds,
      }),
      support: ["leader", "rival_direct", "objective", "recent_form"],
    };
  },

  // 4) O nemesis está no grid. Duelo pessoal vivido, com retrospecto.
  nemesis(signals) {
    const n = signals.nemesis;
    if (!n || !n.in_grid) return null;
    const h2h =
      n.chapters > 0 ? i18n.t("thesis.nemesis.h2h", { wins: n.h2h_player_wins, losses: n.h2h_rival_wins }) : "";
    const label = n.label ? i18n.t("thesis.nemesis.label", { label: n.label }) : "";
    return {
      statement: i18n.t("thesis.nemesis.statement", { name: n.driver_name, label, h2h }),
      support: ["rival_direct", "recent_form", "track_history", "objective"],
    };
  },

  // 5) A tabela aperta por trás. Jogo de contenção.
  pressure(signals) {
    if (signals.championshipState !== "pressure") return null;
    const gap = signals.gapBehind != null ? i18n.t("thesis.pressure.gap", { gap: signals.gapBehind }) : "";
    return {
      statement: i18n.t("thesis.pressure.statement", { gap }),
      support: ["chaser", "pressure", "rival_direct", "objective", "recent_form"],
    };
  },

  // 6) Pista com contas em aberto (abandonos ou pódio distante).
  track_trauma(signals) {
    const t = signals.trackHistory;
    if (!t || !t.has_data) return null;
    const dnfs = t.dnfs ?? 0;
    const best = t.best_finish ?? 99;
    if (dnfs < 1 && best < 10) return null;
    const detail =
      dnfs >= 1 ? i18n.t("thesis.track_trauma.countDnf", { n: dnfs }) : i18n.t("thesis.track_trauma.countBest", { best });
    return {
      statement: i18n.t("thesis.track_trauma.statement", { track: trackLabel(signals), detail }),
      support: ["track_history", "track_last", "recent_form", "objective"],
    };
  },

  // 7) Pista boa de casa. A memória joga a favor.
  track_fortress(signals) {
    const t = signals.trackHistory;
    if (!t || !t.has_data) return null;
    const dnfs = t.dnfs ?? 0;
    const best = t.best_finish ?? 99;
    if (!(best <= 3 && dnfs === 0)) return null;
    return {
      statement: i18n.t("thesis.track_fortress.statement", { track: trackLabel(signals), best }),
      support: ["track_history", "track_last", "recent_form", "objective"],
    };
  },

  // 8) Clima instável. A corrida vira loteria.
  weather(signals) {
    if (!signals.climaWet) return null;
    return {
      statement: i18n.t("thesis.weather.statement", { clima: signals.climaLabel || i18n.t("thesis.weatherWetFallback") }),
      support: ["weather", "recent_form", "objective", "favorite"],
    };
  },

  // 9) Carro frágil. Risco mecânico é o fator a administrar.
  breakdown(signals) {
    if (!signals.breakdownNotable) return null;
    const parts = signals.breakdownParts ? i18n.t("thesis.breakdown.parts", { parts: signals.breakdownParts }) : "";
    return {
      statement: i18n.t("thesis.breakdown.statement", { level: signals.breakdownLevel, parts }),
      support: ["breakdown", "objective", "recent_form"],
    };
  },

  // 10) Grande palco (final ou casa cheia). Maior que os pontos.
  grand_stage(signals) {
    if (!signals.grandStage) return null;
    const audience = signals.audienceLabel ? i18n.t("thesis.grand_stage.audience", { audience: signals.audienceLabel }) : "";
    return {
      statement: i18n.t("thesis.grand_stage.statement", { occasion: signals.eventOccasion, audience }),
      support: ["importance", "fame", "championship_situation", "objective", "favorite"],
    };
  },

  // 11) Estreia da temporada. Todo mundo do zero.
  debut(signals) {
    if (signals.championshipUnderway) return null;
    return {
      statement: i18n.t("thesis.debut.statement", { track: trackLabel(signals) }),
      support: ["objective", "teammate", "favorite"],
    };
  },

  // 12) Nada se destaca. Rodada de construção.
  baseline(signals) {
    return {
      statement: i18n.t("thesis.baseline.statement", { track: trackLabel(signals) }),
      support: ["championship_situation", "rival_direct", "recent_form", "objective"],
    };
  },
};

// "Palco combina": a grande final / casa cheia AMPLIFICA qualquer eixo, em vez de
// competir por um slot próprio. Não se aplica quando o palco já é o eixo
// (`grand_stage`) nem na estreia (`debut`, cujo tom de recomeço é outro).
function stageAppendix(signals, key) {
  if (!signals.grandStage || key === "grand_stage" || key === "debut") return "";
  const occ = (signals.eventOccasion || "").toLowerCase();
  if (!occ) return "";
  const audience = signals.audienceLabel ? i18n.t("thesis.stageAppendix.audience", { audience: signals.audienceLabel }) : "";
  return i18n.t("thesis.stageAppendix.text", { occasion: occ, audience });
}

// Elege a tese dominante. Sempre devolve algo (baseline é o piso). `support` é um
// Set de ids de fato promovidos ao APOIO.
export function selectThesis(signals = {}) {
  let key = "baseline";
  let built = null;
  for (const candidate of THESIS_PRIORITY) {
    const result = THESIS_BUILDERS[candidate]?.(signals);
    if (result) {
      key = candidate;
      built = result;
      break;
    }
  }
  if (!built) {
    built = THESIS_BUILDERS.baseline(signals);
    key = "baseline";
  }
  return {
    key,
    title: i18n.t(`thesis.titles.${key}`),
    statement: built.statement + stageAppendix(signals, key),
    support: new Set(built.support ?? []),
  };
}
