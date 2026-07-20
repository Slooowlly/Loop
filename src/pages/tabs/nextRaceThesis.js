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

// Rótulo curto por tese — usado em debug/UI, nunca no texto final.
export const THESIS_TITLES = {
  redemption: "Redenção",
  title_defense: "Defesa da liderança",
  title_chase: "Caça ao título",
  nemesis: "Duelo pessoal",
  pressure: "Terreno sob ameaça",
  track_trauma: "Contas com a pista",
  track_fortress: "Pista das suas",
  weather: "Loteria do clima",
  breakdown: "Fragilidade mecânica",
  grand_stage: "O palco",
  debut: "Recomeço",
  baseline: "Somar e crescer",
};

function trackLabel(signals) {
  return signals.trackName || "esta etapa";
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
    const detalhe = last.is_dnf
      ? `um DNF na última corrida${last.trackName ? ` (${last.trackName})` : ""}`
      : `um resultado muito abaixo do seu normal (P${last.position})`;
    return {
      statement: `Gancho central — REAÇÃO. O piloto vem de ${detalhe} e esta etapa é sobre responder, virar a chave e recolocar a campanha nos trilhos. Toda a leitura do fim de semana parte desse tombo recente.`,
      support: ["recent_form", "championship_situation", "objective", "track_history"],
    };
  },

  // 2) Líder do campeonato. Peso de quem tem algo a perder. Se vier logo depois de
  //    um DNF (mas segurou a ponta), a defesa fica ainda mais delicada.
  title_defense(signals) {
    if (!signals.championshipUnderway || !signals.playerIsLeader) return null;
    const perseguidor =
      signals.gapBehind != null
        ? ` O perseguidor direto está a ${signals.gapBehind} ponto(s) atrás.`
        : "";
    const susto =
      signals.lastResult?.is_dnf
        ? " — e você chega logo depois de um DNF, o que deixa a defesa ainda mais delicada"
        : "";
    return {
      statement: `Gancho central — DEFESA DA LIDERANÇA. Você chega a ${trackLabel(signals)} na ponta do campeonato${susto}.${perseguidor} A etapa é sobre administrar a pressão e sair daqui ainda ditando o ritmo, sem oferecer brecha.`,
      support: ["chaser", "rival_direct", "objective", "constructors"],
    };
  },

  // 3) Perto do líder, com etapas para virar. A conta do título ainda vive.
  title_chase(signals) {
    if (signals.championshipState !== "chase") return null;
    return {
      statement: `Gancho central — CAÇA AO TÍTULO. Você está a ${signals.gapToLeader} ponto(s) do líder${signals.leaderName ? ` (${signals.leaderName})` : ""}, com ${signals.remainingRounds} etapa(s) pela frente. Esta é uma chance real de encurtar a tabela — a etapa vale confronto direto na parte alta.`,
      support: ["leader", "rival_direct", "objective", "recent_form"],
    };
  },

  // 4) O nemesis está no grid. Duelo pessoal vivido, com retrospecto.
  nemesis(signals) {
    const n = signals.nemesis;
    if (!n || !n.in_grid) return null;
    const h2h = n.chapters > 0 ? ` Retrospecto direto ${n.h2h_player_wins}-${n.h2h_rival_wins}.` : "";
    const label = n.label ? ` ("${n.label}")` : "";
    return {
      statement: `Gancho central — DUELO PESSOAL. Seu nemesis ${n.driver_name}${label} está neste grid.${h2h} O fim de semana gira em torno desse confronto: é ele a régua que mede a sua etapa.`,
      support: ["rival_direct", "recent_form", "track_history", "objective"],
    };
  },

  // 5) A tabela aperta por trás. Jogo de contenção.
  pressure(signals) {
    if (signals.championshipState !== "pressure") return null;
    const atras = signals.gapBehind != null ? ` (${signals.gapBehind} ponto(s))` : "";
    return {
      statement: `Gancho central — TERRENO SOB AMEAÇA. A tabela apertou atrás de você${atras}. Esta etapa é sobre proteger a sua faixa do campeonato: não dá para entregar um fim de semana passivo.`,
      support: ["chaser", "rival_direct", "objective", "recent_form"],
    };
  },

  // 6) Pista com contas em aberto (abandonos ou pódio distante).
  track_trauma(signals) {
    const t = signals.trackHistory;
    if (!t || !t.has_data) return null;
    const dnfs = t.dnfs ?? 0;
    const best = t.best_finish ?? 99;
    if (dnfs < 1 && best < 10) return null;
    const conta =
      dnfs >= 1
        ? `${dnfs} abandono(s) aqui`
        : `melhor resultado só P${best}`;
    return {
      statement: `Gancho central — CONTAS COM A PISTA. ${trackLabel(signals)} tem histórico em aberto com você (${conta}). O eixo é respeito e execução limpa: aqui, errar pouco vale mais do que atacar demais.`,
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
      statement: `Gancho central — PISTA DAS SUAS. ${trackLabel(signals)} guarda boas lembranças (melhor resultado P${best}). O fim de semana começa com a pista a favor — a leitura pode ser mais ousada.`,
      support: ["track_history", "track_last", "recent_form", "objective"],
    };
  },

  // 8) Clima instável. A corrida vira loteria.
  weather(signals) {
    if (!signals.climaWet) return null;
    return {
      statement: `Gancho central — LOTERIA DO CLIMA. A previsão é de pista ${signals.climaLabel || "molhada"}, e isso embaralha o grid e decide a corrida. O eixo é leitura fria: a pista vai premiar quem erra menos, não quem ataca mais.`,
      support: ["weather", "recent_form", "objective", "favorite"],
    };
  },

  // 9) Carro frágil. Risco mecânico é o fator a administrar.
  breakdown(signals) {
    if (!signals.breakdownNotable) return null;
    const alvo = signals.breakdownParts ? ` — atenção a ${signals.breakdownParts}` : "";
    return {
      statement: `Gancho central — FRAGILIDADE MECÂNICA. O carro chega com risco de quebra ${signals.breakdownLevel}${alvo}. É risco, não certeza: o eixo do fim de semana é pesar ataque contra confiabilidade, e talvez poupar o carro.`,
      support: ["breakdown", "objective", "recent_form"],
    };
  },

  // 10) Grande palco (final ou casa cheia). Maior que os pontos.
  grand_stage(signals) {
    if (!signals.grandStage) return null;
    const publico = signals.audienceLabel ? ` — cerca de ${signals.audienceLabel} pessoas` : "";
    return {
      statement: `Gancho central — O PALCO. ${signals.eventOccasion}${publico}. A vitrine e a pressão pesam além da tabela: o eixo é a ocasião, e o que ela cobra de quem está no centro dela.`,
      support: ["importance", "championship_situation", "objective", "favorite"],
    };
  },

  // 11) Estreia da temporada. Todo mundo do zero.
  debut(signals) {
    if (signals.championshipUnderway) return null;
    return {
      statement: `Gancho central — RECOMEÇO. Abertura da temporada em ${trackLabel(signals)}: tabela zerada, todo o grid larga do zero. A etapa é sobre construir a base da campanha e transformar a expectativa da pré-temporada em pontos de verdade.`,
      support: ["objective", "teammate", "favorite"],
    };
  },

  // 12) Nada se destaca. Rodada de construção.
  baseline(signals) {
    return {
      statement: `Gancho central — SOMAR E CRESCER. Sem um enredo dominante, ${trackLabel(signals)} é uma rodada para pontuar forte, ganhar posição na tabela e evitar perdas bobas. O eixo é consistência competitiva.`,
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
  const pub = signals.audienceLabel ? `, com cerca de ${signals.audienceLabel} pessoas` : "";
  return ` E não é um palco qualquer: ${occ}${pub} — a ocasião amplifica tudo o que está em jogo.`;
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
    title: THESIS_TITLES[key],
    statement: built.statement + stageAppendix(signals, key),
    support: new Set(built.support ?? []),
  };
}
