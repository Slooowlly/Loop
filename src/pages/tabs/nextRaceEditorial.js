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
    return "A equipe quer transformar o ritmo de treino em um resultado limpo.";
  }
  const readable = recentResults(playerStanding)
    .map((result) => {
      if (!result) return "resultado indefinido";
      if (result.is_dnf) return "DNF";
      return `${result.position ?? "--"}º lugar`;
    })
    .join(", ");
  return readable
    ? `Nas três leituras mais recentes você veio de ${readable}.`
    : "O momento recente ainda não criou uma tendência clara.";
}

// Copy voltado ao LEITOR (2ª pessoa, voz do jogo), uma entrada por tese. Cada campo
// tem 2 variantes, escolhidas por seed determinístico (mesma etapa ⇒ mesmo texto).
// `lead` = parágrafo do eixo; `outlook` = o que isso significa para a corrida;
// `quote` = voz da equipe à imprensa.
export const THESIS_EDITORIAL = {
  redemption: {
    headline: [(c) => `${c.trackName}: hora de virar a chave.`, (c) => `A reação começa em ${c.trackName}.`],
    lead: [
      (c) => `A última corrida deixou marca, e ${c.trackName} chega como a etapa da resposta. O fim de semana inteiro se organiza em torno de uma ideia simples: recolocar a campanha nos trilhos.`,
      (c) => `Depois do tombo recente, a equipe trata ${c.trackName} como ponto de virada. Não é sobre esquecer o que passou — é sobre transformar frustração em execução limpa.`,
    ],
    outlook: [
      (c) => `O plano não é heroísmo: é uma corrida sólida, sem novos erros, que devolva confiança ao box. ${c.formSentence}`,
      (c) => `A prioridade é estancar a sangria e sair daqui com pontos de verdade. ${c.formSentence}`,
    ],
    quote: [
      () => `A gente sabe o que aconteceu. Agora é cabeça fria, corrida limpa e recomeçar a somar.`,
      (c) => `Página virada. ${c.trackName} é onde a nossa resposta começa.`,
    ],
  },
  title_defense: {
    headline: [(c) => `${c.trackName}: defender a ponta.`, (c) => `Liderança em jogo em ${c.trackName}.`],
    lead: [
      (c) => `Você chega a ${c.trackName} na frente do campeonato, e isso muda o peso de cada detalhe. A etapa é menos sobre atacar e mais sobre não entregar o que já é seu.`,
      (c) => `Estar no topo transforma ${c.trackName} num teste de sangue frio. O trabalho do fim de semana é sair daqui ainda ditando o ritmo.`,
    ],
    outlook: [
      (c) => `Controlar a pressão, administrar riscos e proteger a diferença já basta — não é preciso vencer para vencer. ${c.formSentence}`,
      (c) => `Um resultado alto e limpo mantém a arquitetura da temporada intacta. ${c.formSentence}`,
    ],
    quote: [
      () => `Entramos para defender o que é nosso. Cabeça no lugar e a ponta continua conosco.`,
      () => `Liderar cobra frieza. É isso que a gente vai entregar aqui.`,
    ],
  },
  title_chase: {
    headline: [(c) => `${c.trackName}: encurtar a conta.`, (c) => `A caça ao título passa por ${c.trackName}.`],
    lead: [
      (c) => `A ${c.gapToLeader} ponto(s) de ${c.leaderName} e com ${c.remainingRounds} etapa(s) pela frente, ${c.trackName} vira uma chance real de mexer na tabela. A conta do título segue viva.`,
      (c) => `O campeonato ainda está ao alcance, e ${c.trackName} é o tipo de etapa que decide caças a título. Cada ponto sobre ${c.leaderName} pesa daqui pra frente.`,
    ],
    outlook: [
      (c) => `O recado é agressividade controlada: buscar a frente sem rasgar a corrida. ${c.formSentence}`,
      (c) => `Encostar de vez exige um domingo forte e limpo. ${c.formSentence}`,
    ],
    quote: [
      () => `Estamos perto. Não é hora de ansiedade — é hora de encostar.`,
      () => `A distância existe, mas dá pra reduzir aqui. Vamos pra cima com juízo.`,
    ],
  },
  nemesis: {
    headline: [(c) => `${c.trackName}: acerto de contas.`, (c) => `O duelo com ${c.nemesisName} esquenta.`],
    lead: [
      (c) => `${c.nemesisName} está neste grid, e é impossível separar a sua etapa desse confronto. ${c.trackName} vira palco de um duelo que já tem história.`,
      (c) => `Toda temporada tem uma rivalidade que define corridas, e a sua está aqui em ${c.trackName}. ${c.nemesisName} é a régua que mede o seu fim de semana.`,
    ],
    outlook: [
      (c) => `Bater o rival direto vale mais que a posição no papel — mas sem deixar o duelo custar a corrida. ${c.formSentence}`,
      (c) => `O foco é ganhar a comparação direta e sair por cima. ${c.formSentence}`,
    ],
    quote: [
      () => `Sabemos exatamente com quem estamos brigando. Essa é pessoal, e a gente quer ganhar.`,
      () => `Tem confronto marcado aqui. É disso que corrida boa é feita.`,
    ],
  },
  pressure: {
    headline: [(c) => `${c.trackName}: segurar o terreno.`, (c) => `Sob pressão em ${c.trackName}.`],
    lead: [
      (c) => `A tabela apertou atrás de você, e ${c.trackName} não abre espaço para um fim de semana passivo. A etapa é sobre defender a sua faixa no campeonato.`,
      (c) => `Com a diferença cada vez mais curta, ${c.trackName} pede resposta imediata. Não dá para administrar de forma passiva o que a tabela está cobrando.`,
    ],
    outlook: [
      (c) => `Correr com clareza, sem desperdício, e impedir que a classificação aperte ainda mais. ${c.formSentence}`,
      (c) => `Pontuar forte aqui virou obrigação, não luxo. ${c.formSentence}`,
    ],
    quote: [
      () => `A margem ficou curta. A resposta precisa aparecer agora, nesta corrida.`,
      () => `Não dá pra entregar um fim de semana burocrático neste momento.`,
    ],
  },
  weather: {
    headline: [(c) => `${c.trackName} molhada embaralha tudo.`, (c) => `Loteria do clima em ${c.trackName}.`],
    lead: [
      (c) => `A previsão de ${c.climaLabel} transforma ${c.trackName} numa corrida de leitura, não de bravata. O clima pode reescrever o grid do começo ao fim.`,
      (c) => `Quando a pista molha, ${c.trackName} deixa de ser previsível. A meteorologia entra como fator real de distorção da etapa.`,
    ],
    outlook: [
      (c) => `A pista vai premiar quem erra menos, não quem ataca mais. Sobreviver bem ao caos já altera a corrida. ${c.formSentence}`,
      (c) => `Frieza acima de tudo: um resultado limpo aqui pode valer mais do que parece. ${c.formSentence}`,
    ],
    quote: [
      () => `Com esse clima, a corrida é de cabeça. Menos erro, mais paciência.`,
      () => `Chuva iguala o grid. A gente lê a corrida e aproveita a abertura.`,
    ],
  },
  track_trauma: {
    headline: [(c) => `${c.trackName} tem contas a acertar.`, (c) => `Respeito a ${c.trackName}.`],
    lead: [
      (c) => `${c.trackName} não é uma pista qualquer para você — o histórico aqui pede cautela antes de qualquer promessa ousada. Execução limpa vale mais que bravata.`,
      (c) => `A memória recente de ${c.trackName} recomenda prudência. A etapa chega com um alerta claro: aqui, errar pouco é o caminho.`,
    ],
    outlook: [
      (c) => `Disciplina do começo ao fim, sem forçar o que a pista costuma cobrar caro. ${c.formSentence}`,
      (c) => `A meta é quebrar o retrospecto com uma corrida madura. ${c.formSentence}`,
    ],
    quote: [
      () => `Essa pista já nos cobrou caro. Desta vez, é foco e disciplina.`,
      () => `Sabemos o histórico aqui. Nada de facilitar.`,
    ],
  },
  track_fortress: {
    headline: [(c) => `${c.trackName}: terreno amigo.`, (c) => `Boas lembranças em ${c.trackName}.`],
    lead: [
      (c) => `${c.trackName} guarda boas memórias suas, e isso autoriza uma leitura mais ousada da etapa. A pista joga a favor desde o primeiro treino.`,
      (c) => `Poucas pistas caem tão bem para você quanto ${c.trackName}. O retrospecto aqui dá base para atacar com ambição legítima.`,
    ],
    outlook: [
      (c) => `A questão é transformar referência passada em resultado presente. ${c.formSentence}`,
      (c) => `Com a pista a favor, dá para pensar grande sem ilusão. ${c.formSentence}`,
    ],
    quote: [
      () => `Gostamos daqui. É hora de transformar essa memória boa em pontos.`,
      () => `Essa pista combina com a gente. Vamos aproveitar.`,
    ],
  },
  breakdown: {
    headline: [(c) => `${c.trackName}: administrar o carro.`, (c) => `Confiabilidade em xeque em ${c.trackName}.`],
    lead: [
      (c) => `O carro chega a ${c.trackName} com risco real de quebra, e isso muda o cálculo do fim de semana. É risco, não certeza — mas pesa em cada decisão.`,
      (c) => `Fragilidade mecânica é o assunto de ${c.trackName}. A etapa vira um equilíbrio entre ataque e preservar o carro até a bandeirada.`,
    ],
    outlook: [
      (c) => `Talvez valha poupar em alguns momentos para garantir a chegada. ${c.formSentence}`,
      (c) => `Chegar inteiro pode valer mais do que arriscar tudo. ${c.formSentence}`,
    ],
    quote: [
      () => `O carro pede atenção. Vamos pesar cada ataque contra o risco de não terminar.`,
      () => `Melhor um ponto garantido que um zero heroico. Cabeça fria aqui.`,
    ],
  },
  grand_stage: {
    headline: [(c) => `${c.trackName}: o grande palco.`, (c) => `Holofotes em ${c.trackName}.`],
    lead: [
      (c) => `${c.eventOccasion} coloca ${c.trackName} sob os holofotes. A vitrine e a pressão pesam além dos pontos em jogo.`,
      (c) => `Nem toda corrida carrega o peso de ${c.trackName} nesta ocasião. É o tipo de etapa que separa quem se agiganta de quem encolhe.`,
    ],
    outlook: [
      (c) => `O palco cobra presença: entregar sob pressão é metade da história aqui. ${c.formSentence}`,
      (c) => `Grandes ocasiões se lembram de quem apareceu. ${c.formSentence}`,
    ],
    quote: [
      () => `Etapa assim a gente marca na memória. Viemos para aparecer.`,
      () => `O palco é grande. É exatamente onde queremos estar.`,
    ],
  },
  debut: {
    headline: [(c) => `${c.trackName} abre a temporada.`, (c) => `Recomeço em ${c.trackName}.`],
    lead: [
      (c) => `A temporada começa em ${c.trackName} com o grid inteiro partindo do zero. Não há tabela para defender — há uma campanha inteira para construir.`,
      (c) => `${c.trackName} recomeça tudo: tabela zerada e a primeira leitura de forças do ano. A etapa é sobre plantar a base da temporada.`,
    ],
    outlook: [
      (c) => `Um começo sólido evita ter que correr atrás do prejuízo logo de cara. ${c.formSentence}`,
      (c) => `Largar bem na estreia dá lastro de confiança para o resto do calendário. ${c.formSentence}`,
    ],
    quote: [
      () => `Temporada nova, conta zerada. Queremos começar com o pé direito.`,
      () => `É a hora de transformar a pré-temporada em pontos de verdade.`,
    ],
  },
  baseline: {
    headline: [(c) => `${c.trackName}: somar e crescer.`, (c) => `Construção em ${c.trackName}.`],
    lead: [
      (c) => `Sem um enredo dominante, ${c.trackName} é uma rodada de construção: pontuar forte, ganhar posição e evitar perdas bobas. Consistência é o nome do jogo.`,
      (c) => `${c.trackName} não traz um grande drama de tabela, e tudo bem. A etapa é sobre fazer o simples bem feito e crescer no campeonato.`,
    ],
    outlook: [
      (c) => `Uma corrida limpa e eficiente aqui melhora a respiração da temporada. ${c.formSentence}`,
      (c) => `Cada ponto somado sem sustos reorganiza a campanha ao seu redor. ${c.formSentence}`,
    ],
    quote: [
      () => `Nada de invenção. Corrida limpa, pontos no bolso e seguimos crescendo.`,
      () => `Foco no básico bem feito. É assim que a temporada se constrói.`,
    ],
  },
};

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
    trackName: nextRace?.track_name ?? "esta etapa",
    teamName: playerTeam?.nome ?? "a equipe",
    leaderName: leader?.nome ?? "a ponta",
    rivalName: briefingRival?.driver_name ?? "o rival direto",
    nemesisName: nemesisName ?? briefingRival?.driver_name ?? "seu rival",
    gapToLeader,
    remainingRounds,
    bestFinish: trackHistory?.best_finish ?? null,
    climaLabel: climaLabel ?? "pista molhada",
    eventOccasion: eventOccasion ?? "Uma etapa de destaque",
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
