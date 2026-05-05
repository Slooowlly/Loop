import { recentResults } from "./nextRaceBriefing";

function combineVariants(openers, closers) {
  const variants = [];

  for (const opener of openers) {
    for (const closer of closers) {
      variants.push((context) => `${opener(context)} ${closer(context)}`.trim());
    }
  }

  return variants;
}

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

const headlinePools = {
  leader_hot: combineVariants(
    [
      ({ trackName }) => `Você chega a ${trackName} defendendo a liderança em um fim de semana que promete tensão alta.`,
      ({ trackName }) => `Você desembarca em ${trackName} com a ponta do campeonato nas maos e um clima de pressão imediata.`,
      ({ trackName }) => `A defesa da liderança passa por ${trackName}, uma etapa que já nasce carregada.`,
      ({ trackName }) => `Chegar lider a ${trackName} muda o tom da rodada e aumenta o peso de cada detalhe.`,
      ({ trackName }) => `A liderança chega com você a ${trackName}, mas a etapa não promete conforto.`,
    ],
    [
      () => "A largada tende a abrir um fim de semana de controle fino e margem curta.",
      () => "Desde o inicio, a rodada aponta para uma defesa de liderança sob calor competitivo.",
    ],
  ),
  leader: combineVariants(
    [
      ({ trackName }) => `Você chega a ${trackName} defendendo a liderança do campeonato.`,
      ({ trackName }) => `A etapa de ${trackName} abre com você no topo da tabela.`,
      ({ trackName }) => `Você desembarca em ${trackName} com a responsabilidade de sustentar a ponta.`,
      ({ trackName }) => `Chegar lider a ${trackName} coloca você no centro da narrativa da rodada.`,
      ({ trackName }) => `A rodada comeca em ${trackName} com você como referência do campeonato.`,
    ],
    [
      () => "O desafio agora e seguir ditando o ritmo sem oferecer brecha ao pelotao de perseguição.",
      () => "A pauta esportiva da etapa passa por manter a ponta e administrar a pressão com autoridade.",
    ],
  ),
  chase: combineVariants(
    [
      ({ trackName, leaderName }) => `Você chega a ${trackName} tentando encurtar a distância para ${leaderName ?? "a ponta"}.`,
      ({ trackName, leaderName }) => `A rodada de ${trackName} abre com uma chance real de pressionar ${leaderName ?? "o lider"}.`,
      ({ trackName, leaderName }) => `Você vai a ${trackName} com a missao clara de apróximar a tabela de ${leaderName ?? "quem lidera"}.`,
      ({ trackName, leaderName }) => `Em ${trackName}, a ordem do fim de semana e pressionar ${leaderName ?? "a liderança"}.`,
      ({ trackName, leaderName }) => `A visita a ${trackName} coloca você diante de uma oportunidade concreta de encurtar o campeonato para ${leaderName ?? "o lider"}.`,
    ],
    [
      ({ remainingRounds }) => `${remainingRounds} etapas seguem abertas depois desta corrida.`,
      () => "O recado do campeonato e simples: esta e uma rodada para mexer na frente.",
    ],
  ),
  pressure: combineVariants(
    [
      ({ trackName }) => `Você chega a ${trackName} precisando proteger terreno na tabela.`,
      ({ trackName }) => `A etapa de ${trackName} coloca sua posição sob vigilancia direta.`,
      ({ trackName }) => `Você desembarca em ${trackName} sabendo que a rodada pode redesenhar o bloco da frente.`,
      ({ trackName }) => `Em ${trackName}, a prioridade competitiva passa por segurar a sua faixa do campeonato.`,
      ({ trackName }) => `A visita a ${trackName} não abre espaço para um fim de semana neutro na classificação geral.`,
    ],
    [
      () => "Cada ponto da rodada pode alterar a ordem imediata da disputa.",
      () => "A margem de conforto e curta, e a etapa pede resposta imediata.",
    ],
  ),
  outsider: combineVariants(
    [
      ({ trackName }) => `Você chega a ${trackName} buscando fechar a temporada com dignidade competitiva.`,
      ({ trackName }) => `A etapa de ${trackName} vira oportunidade para salvar lastro esportivo na reta final.`,
      ({ trackName }) => `Você vai a ${trackName} tentando reagir com maturidade em um campeonato que ficou distante.`,
      ({ trackName }) => `Em ${trackName}, a meta já não e sonhar alto, e sim recolocar a campanha em pe firme.`,
      ({ trackName }) => `A rodada de ${trackName} abre uma chance de resposta honesta antes do fim da temporada.`,
    ],
    [
      () => "O objetivo agora e competir com peso, mesmo sem a conta do título ao alcance imediato.",
      () => "A pauta esportiva da corrida passa mais por recuperar respeito competitivo do que por fantasia de tabela.",
    ],
  ),
  survival: combineVariants(
    [
      ({ trackName }) => `Você chega a ${trackName} precisando reagir para recolocar a campanha em movimento.`,
      ({ trackName }) => `A etapa de ${trackName} pede uma resposta direta da equipe no campeonato.`,
      ({ trackName }) => `Você desembarca em ${trackName} com a necessidade clara de interromper a inércia da campanha.`,
      ({ trackName }) => `A rodada de ${trackName} se apresenta como uma prova importante de recuperacao esportiva.`,
      ({ trackName }) => `Em ${trackName}, o campeonato cobra uma corrida que devolva tração ao projeto.`,
    ],
    [
      () => "Pontuar com autoridade aqui pode mudar o humor da temporada.",
      () => "Mais do que um resultado isolado, a corrida pede um sinal de retomada.",
    ],
  ),
};

const championshipParagraphPools = {
  leader: combineVariants(
    [
      ({ trackName }) => `A equipe desembarca em ${trackName} com você na ponta da tabela.`,
      ({ trackName }) => `Chegar líder a ${trackName} muda o peso de toda a preparação da rodada.`,
      ({ trackName }) => `O box chega a ${trackName} defendendo a primeira colocação.`,
      ({ trackName }) => `A liderança transforma ${trackName} em um teste de gestão de pressão.`,
      ({ trackName }) => `Com você no topo, ${trackName} deixa de ser apenas mais uma etapa do calendário.`,
    ],
    [
      () => "O trabalho da rodada é controlar a pressão imediata sem ceder o comando da narrativa esportiva.",
      () => "A missão do fim de semana é sair daqui ainda ditando o ritmo do campeonato.",
    ],
  ),
  chase: combineVariants(
    [
      ({ playerStanding, gapToLeader }) => `Você ocupa a ${playerStanding.posicao_campeonato}ª colocação e entra nesta etapa ${gapToLeader} pontos atrás da liderança.`,
      ({ playerStanding, gapToLeader }) => `A ${playerStanding.posicao_campeonato}ª posição ainda mantém a disputa aberta, mas os ${gapToLeader} pontos de diferença cobram agressividade limpa.`,
      ({ playerStanding, gapToLeader }) => `Você abre a rodada na ${playerStanding.posicao_campeonato}ª colocação, com ${gapToLeader} pontos separando a campanha da ponta.`,
      ({ playerStanding, gapToLeader }) => `A leitura do campeonato coloca você na ${playerStanding.posicao_campeonato}ª colocação e a ${gapToLeader} pontos do líder.`,
      ({ playerStanding, gapToLeader }) => `Entrar nesta etapa na ${playerStanding.posicao_campeonato}ª posição e com ${gapToLeader} pontos de desvantagem mantém a conta do título viva, mas apertada.`,
    ],
    [
      () => "Cada resultado limpo agora pesa diretamente na luta pelo título.",
      () => "A rodada ganhou valor de confronto direto na parte alta da tabela.",
    ],
  ),
  pressure: combineVariants(
    [
      () => "Você entra nesta etapa precisando proteger a sua faixa da tabela.",
      () => "A sua posição no campeonato passa por uma rodada de defesa esportiva.",
      () => "O campeonato empurrou você para um fim de semana de contenção e resposta.",
      () => "A corrida chega com uma tarefa clara: impedir que a tabela aperte ainda mais.",
      () => "O momento da temporada não abre espaço para administração passiva da pontuação.",
    ],
    [
      () => "A rodada carrega pontos demais para permitir um fim de semana burocratico.",
      () => "Qualquer perda aqui tende a ser sentida de imediato no bloco da frente.",
    ],
  ),
  outsider: combineVariants(
    [
      ({ playerStanding, gapToLeader }) => `Você ocupa a ${playerStanding.posicao_campeonato}ª colocação e entra nesta etapa ${gapToLeader} pontos atrás da liderança.`,
      ({ playerStanding, gapToLeader }) => `A ${playerStanding.posicao_campeonato}ª colocação e a distância de ${gapToLeader} pontos mudam a escala da ambição para esta rodada.`,
      ({ playerStanding, gapToLeader }) => `Você abre a etapa na ${playerStanding.posicao_campeonato}ª posição, com ${gapToLeader} pontos de desvantagem para a ponta.`,
      ({ playerStanding, gapToLeader }) => `O recorte do campeonato coloca você longe da liderança: ${playerStanding.posicao_campeonato}ª colocação e ${gapToLeader} pontos de margem.`,
      ({ playerStanding, gapToLeader }) => `Entrar nesta rodada a ${gapToLeader} pontos do topo e na ${playerStanding.posicao_campeonato}ª colocação exige uma leitura mais realista da campanha.`,
    ],
    [
      () => "O foco realista agora é somar forte, recuperar confiança e evitar que a reta final escape de vez do controle.",
      () => "A tarefa esportiva virou recuperar consistência e voltar a produzir rodadas com peso competitivo.",
    ],
  ),
  survival: combineVariants(
    [
      ({ trackName }) => `A equipe chega a ${trackName} precisando reagir no campeonato.`,
      ({ trackName }) => `A rodada de ${trackName} aparece como chance de estancar a perda de ritmo na temporada.`,
      ({ trackName }) => `Você desembarca em ${trackName} com necessidade clara de resposta esportiva.`,
      ({ trackName }) => `O campeonato empurra a campanha para uma etapa de reacerto em ${trackName}.`,
      ({ trackName }) => `A visita a ${trackName} abre um ponto de inflexão importante para a equipe.`,
    ],
    [
      () => "O objetivo esportivo da rodada é recuperar consistência antes que a temporada aperte ainda mais.",
      () => "Mais do que o resultado final, o campeonato pede uma corrida que devolva direção ao projeto.",
    ],
  ),
};

const weekendParagraphPools = {
  weekend_hot: combineVariants(
    [
      ({ rivalName }) => `${rivalName} chega como referência direta em um fim de semana tratado pelo paddock como ponto de virada.`,
      ({ rivalName }) => `O paddock lê a rodada como um confronto aberto, com ${rivalName} no centro da tensão competitiva.`,
      ({ rivalName }) => `${rivalName} entra na etapa como figura dominante de um fim de semana que ganhou temperatura cedo.`,
      ({ rivalName }) => `A leitura externa aponta para uma rodada quente, e ${rivalName} aparece como termômetro imediato da disputa.`,
      ({ rivalName }) => `Há clima de duelo grande no paddock, com ${rivalName} puxando a referência da frente.`,
    ],
    [
      ({ formSentence, remainingRounds }) => `${formSentence} ${remainingRounds > 0 ? `Depois desta corrida ainda restam ${remainingRounds} etapas para mexer na tabela.` : "Esta corrida fecha a conta da temporada."}`,
      ({ formSentence }) => `${formSentence} O fim de semana ganhou peso de rodada-charneira para a parte alta do campeonato.`,
    ],
  ),
  history_positive: combineVariants(
    [
      () => "O histórico recente nesta pista ajuda a sustentar uma leitura mais agressiva para a etapa.",
      () => "A memória esportiva do circuito joga a favor de um fim de semana mais afirmativo.",
      () => "Os antecedentes nesta pista permitem olhar a rodada com ambição legitima.",
      () => "O retrospecto aqui oferece base para pensar em uma etapa de ataque controlado.",
      () => "A pista devolve lembranças boas o bastante para autorizar uma leitura mais ousada da rodada.",
    ],
    [
      ({ formSentence }) => formSentence,
      ({ formSentence }) => `${formSentence} A questao agora e transformar referência passada em resultado presente.`,
    ],
  ),
  history_negative: combineVariants(
    [
      () => "A pista cobra respeito: o histórico aqui pede execução limpa antes de qualquer promessa mais ousada.",
      () => "O retrospecto neste circuito recomenda prudencia antes de discurso agressivo.",
      () => "A etapa chega em uma pista que ainda guarda contas em aberto com a campanha.",
      () => "A memória recente do circuito não permite tratar esta corrida como rodada comum.",
      () => "O histórico da pista funciona como alerta para um fim de semana de disciplina.",
    ],
    [
      ({ formSentence }) => formSentence,
      ({ formSentence }) => `${formSentence} Aqui, errar pouco pode valer mais do que atacar demais.`,
    ],
  ),
  weather_unstable: combineVariants(
    [
      () => "O clima deve embaralhar a etapa e ampliar o valor de uma corrida sem erros.",
      () => "A previsao deixa a rodada mais movediça e aumenta o peso de cada decisão.",
      () => "Com tempo instavel no radar, a etapa tende a punir exageros e premiar leitura fria.",
      () => "O clima retira previsibilidade da corrida e coloca execução acima de bravata.",
      () => "A meteorologia entra na pauta da etapa como fator real de distorcao competitiva.",
    ],
    [
      ({ formSentence }) => formSentence,
      ({ formSentence }) => `${formSentence} Numa rodada assim, a pista costuma recompensar quem erra menos.`,
    ],
  ),
  outsider: combineVariants(
    [
      ({ rivalName }) => `${rivalName} e o restante da frente chegam com margem clara nesta rodada.`,
      ({ rivalName }) => `A dianteira chega respaldada, com ${rivalName} puxando um bloco que corre com folga sobre a sua campanha.`,
      ({ rivalName }) => `${rivalName} comanda a referência externa de uma etapa em que a frente chega mais leve do que você.`,
      ({ rivalName }) => `O pelotao principal abre a rodada com margem, e ${rivalName} simboliza essa vantagem imediata.`,
      ({ rivalName }) => `A etapa comeca com ${rivalName} e a frente operando de uma posição de conforto relativa.`,
    ],
    [
      ({ formSentence }) => `${formSentence} O título depende de uma combinacao muito improvável de resultados, entao o box ajusta a expectativa para uma corrida limpa e oportunista.`,
      ({ formSentence }) => `${formSentence} A leitura do fim de semana sai do romantismo da tabela e entra no territorio do oportunismo competitivo.`,
    ],
  ),
  neutral: combineVariants(
    [
      ({ rivalName }) => `${rivalName} segue como referência direta desta rodada.`,
      ({ rivalName }) => `${rivalName} continua servindo como comparacao imediata para medir o fim de semana.`,
      ({ rivalName }) => `A leitura competitiva da etapa passa por como você responde ao ritmo de ${rivalName}.`,
      ({ rivalName }) => `${rivalName} permanece como espelho mais visivel do bloco em que a rodada será decidida.`,
      ({ rivalName }) => `No recorte esportivo desta etapa, ${rivalName} ainda e o nome que melhor mede a sua margem de crescimento.`,
    ],
    [
      ({ formSentence }) => `Enquanto isso, ${formSentence}`,
      ({ formSentence }) => `${formSentence} A rodada ainda oferece espaço para reorganizar a narrativa do campeonato.`,
    ],
  ),
};

const quotePools = {
  leader: combineVariants(
    [
      () => "\"Entramos para defender o que e nosso.",
      () => "\"Chegamos lideres e a responsabilidade agora e corresponder a isso.",
      () => "\"A ponta do campeonato não permite dispersao.",
      () => "\"Estar na frente muda o peso de cada detalhe deste fim de semana.",
      () => "\"A liderança exige frieza antes de qualquer impulso.",
    ],
    [
      () => "O importante e sair daqui ainda ditando o ritmo do campeonato.\"",
      () => "Nosso trabalho e transformar controle em resultado.\"",
    ],
  ),
  chase: combineVariants(
    [
      () => "\"Estamos perto.",
      () => "\"A distância existe, mas a rodada oferece espaço para reduzir isso.",
      () => "\"O campeonato ainda esta acessivel se a execução vier limpa.",
      () => "\"Não e hora de ansiedade; e hora de encostar na frente.",
      () => "\"Estamos numa faixa em que um fim de semana forte muda a conversa.",
    ],
    [
      () => "A equipe quer agressividade controlada desde a classificação para encostar de vez na ponta.\"",
      () => "Precisamos sair daqui tendo pressionado a frente de verdade.\"",
    ],
  ),
  pressure: combineVariants(
    [
      () => "\"A tabela apertou e isso muda a forma de encarar a rodada.",
      () => "\"Não da para entregar um fim de semana passivo neste momento.",
      () => "\"A margem ficou curta e a resposta precisa aparecer agora.",
      () => "\"Esta e uma etapa para correr com clareza e sem desperdicio.",
      () => "\"O campeonato esta cobrando firmeza competitiva nesta rodada.",
    ],
    [
      () => "A prioridade e sair daqui com a posição protegida e a campanha respirando.\"",
      () => "Pontuar forte virou obrigação esportiva, não luxo.\"",
    ],
  ),
  outsider: combineVariants(
    [
      () => `"Nossa equipe quer uma rodada limpa, madura`,
      () => `"Nossa equipe sabe que a tabela já não permite fantasia`,
      () => `"Nossa equipe olha para esta etapa como chance de reconstrução esportiva`,
      () => `"Nossa equipe precisa voltar a produzir um fim de semana respeitavel`,
      () => `"Nossa equipe entra na rodada pensando mais em consistência do que em bravata`,
    ],
    [
      () => "e forte o bastante para recolocar a campanha em trilho competitivo.\"",
      () => "e devolver peso competitivo a uma campanha que perdeu altitude.\"",
    ],
  ),
  survival: combineVariants(
    [
      () => `"Nossa equipe espera um fim de semana forte`,
      () => `"Nossa equipe entra na etapa precisando de uma resposta clara`,
      () => `"Nossa equipe quer recuperar tração competitiva nesta rodada`,
      () => `"Nossa equipe sabe que esta e uma corrida importante para reorganizar a campanha`,
      () => `"Nossa equipe trata a etapa como ponto de retomada`,
    ],
    [
      () => "para recolocar a campanha no rumo certo.\"",
      () => "para dar direcao esportiva ao restante da temporada.\"",
    ],
  ),
};

const rivalSummaryPools = {
  ahead: combineVariants(
    [
      ({ briefingRival }) => `${briefingRival.driver_name} chega como referência imediata do campeonato, ocupando a P${briefingRival.championship_position}`,
      ({ briefingRival }) => `${briefingRival.driver_name} abre a rodada como o nome mais próximo a ser alcancado, na P${briefingRival.championship_position}`,
      ({ briefingRival }) => `${briefingRival.driver_name} aparece na frente da sua campanha, segurando a P${briefingRival.championship_position}`,
      ({ briefingRival }) => `A referência direta da tabela atende por ${briefingRival.driver_name}, atual P${briefingRival.championship_position}`,
      ({ briefingRival }) => `${briefingRival.driver_name} entra nesta etapa como espelho mais imediato da sua luta na classificação, na P${briefingRival.championship_position}`,
    ],
    [
      ({ briefingRival }) => `com ${briefingRival.gap_points} ponto${briefingRival.gap_points === 1 ? "" : "s"} de margem.`,
      ({ briefingRival }) => `e leva ${briefingRival.gap_points} ponto${briefingRival.gap_points === 1 ? "" : "s"} de vantagem para a largada.`,
    ],
  ),
  aheadOutsider: combineVariants(
    [
      ({ briefingRival }) => `${briefingRival.driver_name} lidera a referência esportiva da rodada, ocupando a P${briefingRival.championship_position}`,
      ({ briefingRival }) => `No seu recorte de campanha, ${briefingRival.driver_name} aparece como principal parametro da etapa na P${briefingRival.championship_position}`,
      ({ briefingRival }) => `${briefingRival.driver_name} simboliza a frente da disputa que ainda da para morder, abrindo a etapa em P${briefingRival.championship_position}`,
      ({ briefingRival }) => `A referência mais concreta desta rodada passa por ${briefingRival.driver_name}, atual P${briefingRival.championship_position}`,
      ({ briefingRival }) => `${briefingRival.driver_name} funciona como alvo esportivo mais util da etapa, largando da P${briefingRival.championship_position}`,
    ],
    [
      ({ briefingRival }) => `com ${briefingRival.gap_points} ponto${briefingRival.gap_points === 1 ? "" : "s"} de margem sobre você.`,
      ({ briefingRival }) => `e carrega ${briefingRival.gap_points} ponto${briefingRival.gap_points === 1 ? "" : "s"} de vantagem neste recorte da tabela.`,
    ],
  ),
  behind: combineVariants(
    [
      ({ briefingRival }) => `${briefingRival.driver_name} aparece como alvo direto na tabela, ocupando a P${briefingRival.championship_position}`,
      ({ briefingRival }) => `A perseguição mais imediata atende por ${briefingRival.driver_name}, hoje na P${briefingRival.championship_position}`,
      ({ briefingRival }) => `${briefingRival.driver_name} e o piloto que a rodada permite mirar de forma mais concreta, na P${briefingRival.championship_position}`,
      ({ briefingRival }) => `${briefingRival.driver_name} abre a etapa como presa esportiva mais clara do seu bloco, ocupando a P${briefingRival.championship_position}`,
      ({ briefingRival }) => `No recorte da classificação, ${briefingRival.driver_name} e o alvo direto da rodada a partir da P${briefingRival.championship_position}`,
    ],
    [
      ({ briefingRival }) => `e andando ${briefingRival.gap_points} ponto${briefingRival.gap_points === 1 ? "" : "s"} atras.`,
      ({ briefingRival }) => `com uma diferenca de ${briefingRival.gap_points} ponto${briefingRival.gap_points === 1 ? "" : "s"} para o seu lado.`,
    ],
  ),
  neutral: combineVariants(
    [
      ({ rivalName }) => `${rivalName} segue como a comparacao mais imediata desta etapa`,
      ({ rivalName }) => `${rivalName} permanece como o nome mais util para medir a rodada`,
      ({ rivalName }) => `A referência direta do fim de semana continua sendo ${rivalName}`,
      ({ rivalName }) => `${rivalName} ainda funciona como espelho esportivo mais claro para a etapa`,
      ({ rivalName }) => `No bloco em que a corrida deve se decidir, ${rivalName} segue como parametro central`,
    ],
    [
      () => "para medir o fim de semana.",
      () => "na leitura esportiva desta rodada.",
    ],
  ),
};

const scenarioPools = {
  leader: combineVariants(
    [
      () => "Um top 5 mantem a pressão sob controle",
      () => "Sair desta rodada entre os cinco primeiros segura o campeonato",
      () => "Controlar danos com um resultado alto preserva a arquitetura da temporada",
      () => "Pontuar forte sem dramatizar a etapa já cumpre boa parte da missao do lider",
      () => "A referência de resultado para quem lidera passa por não entregar terreno desnecessario",
    ],
    [
      () => "e protege a liderança para a próxima rodada.",
      () => "e sustenta a dianteira antes do próximo compromisso.",
    ],
  ),
  outsider: combineVariants(
    [
      () => "O título depende de uma combinacao muito improvável de resultados.",
      () => "A conta do campeonato ficou improvável demais para ser tratada como meta central.",
      () => "A luta pelo título entrou em um territorio pouco realista para esta campanha.",
      () => "O campeonato já não responde apenas a um bom domingo; a conta virou improvável.",
      () => "A temporada deixou o título em uma faixa muito improvável de alcance imediato.",
    ],
    [
      () => "O foco realista e somar pontos, ganhar ritmo e aproveitar qualquer abertura que apareca no caos da prova.",
      () => "A pauta esportiva agora e pontuar, recuperar forma e capitalizar qualquer corrida quebrada a frente.",
    ],
  ),
  chaseRivalSpotlight: combineVariants(
    [
      ({ rivalName }) => `Se você vencer e ${rivalName} perder terreno fora do top 5`,
      ({ rivalName }) => `Uma vitoria combinada com uma rodada ruim de ${rivalName}`,
      ({ rivalName }) => `Caso você transforme a etapa em triunfo e ${rivalName} escape da zona alta`,
      ({ rivalName }) => `A combinacao de um domingo forte seu com perda de ritmo de ${rivalName}`,
      ({ rivalName }) => `Se a corrida sorrir para você e ${rivalName} não sustentar a frente`,
    ],
    [
      () => "pode recolocar a liderança em distância de ataque imediato.",
      () => "devolve tração direta para a luta pela ponta do campeonato.",
    ],
  ),
  weatherUnstable: combineVariants(
    [
      () => "A margem entre ataque e prejuizo fica menor quando o clima embaralha a etapa.",
      () => "Com tempo instavel, o limite entre boa leitura e desastre esportivo encurta rapidamente.",
      () => "Corridas assim apertam a diferenca entre ousadia boa e dano grande.",
      () => "Quando o clima interfere, a etapa costuma premiar menos o brilho e mais a disciplina.",
      () => "A meteorologia torna a rodada menos linear e aumenta o custo de um erro simples.",
    ],
    [
      () => "Um resultado limpo aqui pode valer mais do que parece.",
      () => "Nessas condições, sobreviver bem ao caos já altera o campeonato.",
    ],
  ),
  pressure: combineVariants(
    [
      () => "Pontuar forte aqui e o que separa uma campanha viva de uma rodada que aperta ainda mais a tabela.",
      () => "Esta e uma etapa em que um domingo solido evita que o campeonato deslize para uma faixa desconfortavel.",
      () => "O peso esportivo da corrida passa por impedir que a classificação aperte ainda mais.",
      () => "A rodada tem cara de divisoria entre estabilizar a campanha e perder terreno sensivel.",
      () => "O campeonato transformou esta corrida em um teste direto de sustentacao da campanha.",
    ],
    [
      () => "Perder pontos demais aqui tende a ser sentido imediatamente.",
      () => "A tabela vai cobrar cada detalhe da execução deste fim de semana.",
    ],
  ),
  fallback: combineVariants(
    [
      () => "Um podio aqui reduz a pressão do campeonato",
      () => "Levar a campanha ao bloco da frente nesta etapa alivia a tabela",
      () => "Um resultado alto nesta corrida melhora a respiracao esportiva da temporada",
      () => "Voltar ao top da rodada já muda a temperatura da campanha",
      () => "A etapa oferece espaço para um resultado que reorganize o campeonato ao seu redor",
    ],
    [
      () => "e abre espaço para atacar nas próximas etapas.",
      () => "e devolve margem de manobra para o trecho seguinte da temporada.",
    ],
  ),
};

const actionHintPools = {
  leader: combineVariants(
    [
      () => "O box esta pronto para controlar a rodada.",
      () => "A equipe entra nesta etapa com plano claro de defesa da ponta.",
      () => "A largada pede execução fria de quem tem a liderança nas maos.",
      () => "A estrutura do fim de semana foi desenhada para proteger a dianteira.",
      () => "Tudo converge para uma corrida de lider que sabe a hora de atacar e a hora de administrar.",
    ],
    [
      () => "Simule agora para transformar a liderança em resultado.",
      () => "Levar a etapa para a simulação agora é o passo natural desta defesa de campeonato.",
    ],
  ),
  outsider: combineVariants(
    [
      () => "Buscar um top 8 limpo",
      () => "A meta prática da rodada e sair com pontos fortes e poucos danos",
      () => "O alvo mais realista do fim de semana passa por uma corrida madura e eficiente",
      () => "Esta etapa pede oportunismo e controle de perdas antes de qualquer heroismo",
      () => "O caminho esportivo mais sensato e construir uma prova limpa e firme",
    ],
    [
      () => "e capitalizar qualquer erro a frente e o caminho mais realista para esta etapa.",
      () => "para depois aproveitar o caos eventual a frente e converter isso em pontos pesados.",
    ],
  ),
  chase: combineVariants(
    [
      ({ rivalName }) => `O duelo direto com ${rivalName} esta armado.`,
      ({ rivalName }) => `${rivalName} aparece como medida imediata desta corrida.`,
      ({ rivalName }) => `A rodada coloca ${rivalName} no centro do seu recorte esportivo.`,
      ({ rivalName }) => `${rivalName} e a referência direta de uma etapa feita para mexer na ponta.`,
      ({ rivalName }) => `O caminho para encurtar a tabela passa por responder ao ritmo de ${rivalName}.`,
    ],
    [
      () => "Simular agora leva o fim de semana direto para a tela de resultados.",
      () => "Levar a etapa para a simulação agora coloca esse confronto em campo imediatamente.",
    ],
  ),
  pressure: combineVariants(
    [
      ({ rivalName }) => `A etapa pede resposta direta no duelo com ${rivalName}.`,
      ({ rivalName }) => `${rivalName} aparece como a barreira mais imediata entre você e uma rodada segura.`,
      ({ rivalName }) => `O recorte da corrida passa por não deixar ${rivalName} controlar esse bloco sozinho.`,
      ({ rivalName }) => `A pressão da tabela transforma o duelo com ${rivalName} em prioridade prática.`,
      ({ rivalName }) => `Segurar a rodada passa, inevitavelmente, por como você enfrenta ${rivalName}.`,
    ],
    [
      () => "Simular agora coloca a briga decisiva da etapa em movimento.",
      () => "Levar a corrida para a simulação agora é o passo seguinte dessa defesa de terreno.",
    ],
  ),
  fallback: combineVariants(
    [
      () => "Quando estiver pronto,",
      () => "A etapa já esta desenhada no painel.",
      () => "O briefing já aponta os riscos e oportunidades do fim de semana.",
      () => "A leitura competitiva da rodada esta fechada.",
      () => "O próximo passo agora e simples.",
    ],
    [
      () => "simule a corrida para fechar o fim de semana e atualizar o campeonato.",
      () => "leve a corrida para a simulação e transforme a prévia em classificação real.",
    ],
  ),
};

export const EDITORIAL_COPY_POOLS = {
  headline: headlinePools,
  championshipParagraph: championshipParagraphPools,
  weekendParagraph: weekendParagraphPools,
  quote: quotePools,
  rivalSummaryAhead: rivalSummaryPools,
  scenario: scenarioPools,
  actionHint: actionHintPools,
};

export function classifyChampionshipState({
  playerStanding,
  leader,
  remainingRounds = 0,
  outlook,
  gapBehind,
}) {
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

export function classifyWeekendState({
  trackHistory,
  briefingRival,
  nextRace,
  weekendStories = [],
}) {
  const unstableWeather =
    nextRace?.clima === "Wet" || nextRace?.clima === "HeavyRain" || nextRace?.clima === "Damp";
  const strongStories = weekendStories.filter((story) => {
    const importance = String(story?.importanceLabel ?? story?.importance ?? "").toLowerCase();
    return importance.includes("alta") || importance.includes("destaque");
  }).length;

  if (strongStories > 0 && briefingRival?.driver_name) {
    return "weekend_hot";
  }

  if (trackHistory?.has_data) {
    if ((trackHistory.best_finish ?? 99) <= 3 && (trackHistory.dnfs ?? 0) === 0) {
      return "history_positive";
    }

    if ((trackHistory.dnfs ?? 0) >= 1 || (trackHistory.best_finish ?? 99) >= 8) {
      return "history_negative";
    }
  }

  if (unstableWeather) {
    return "weather_unstable";
  }

  if (briefingRival?.driver_name) {
    return "rival_spotlight";
  }

  return "weekend_neutral";
}

export function buildEditorialCopy({
  championshipState,
  weekendState,
  playerStanding,
  leader,
  rival,
  briefingRival,
  playerTeam,
  nextRace,
  trackHistory,
  weekendStories = [],
  gapToLeader = 0,
  remainingRounds = 0,
  audienceEstimate = 0,
}) {
  const rivalName = briefingRival?.driver_name ?? rival?.nome ?? "o rival direto";
  const trackName = nextRace?.track_name ?? "esta etapa";
  const teamName = playerTeam?.nome ?? "a equipe";
  const formSentence = buildFormSentence(playerStanding);
  const storyLead = weekendStories[0]?.summary ?? null;
  const context = {
    playerStanding,
    leader,
    briefingRival,
    playerTeam,
    nextRace,
    trackHistory,
    weekendStories,
    gapToLeader,
    remainingRounds,
    audienceEstimate,
    rivalName,
    trackName,
    teamName,
    leaderName: leader?.nome ?? "a ponta",
    formSentence,
  };
  const headlineKey =
    championshipState === "leader" &&
    (weekendState === "weather_unstable" || weekendState === "weekend_hot")
      ? "leader_hot"
      : championshipState;
  const weekendParagraphKey =
    championshipState === "outsider" && weekendState !== "weekend_hot"
      ? "outsider"
      : weekendState === "rival_spotlight" || weekendState === "weekend_neutral"
        ? "neutral"
        : weekendState;
  const quoteKey =
    championshipState === "leader" ||
    championshipState === "chase" ||
    championshipState === "pressure" ||
    championshipState === "outsider"
      ? championshipState
      : "survival";
  const rivalSummaryKey = !briefingRival
    ? "neutral"
    : briefingRival.is_ahead
      ? championshipState === "outsider"
        ? "aheadOutsider"
        : "ahead"
      : "behind";
  const scenarioKey =
    championshipState === "leader"
      ? "leader"
      : championshipState === "outsider"
        ? "outsider"
        : championshipState === "pressure"
          ? "pressure"
          : championshipState === "chase" && weekendState === "rival_spotlight"
            ? "chaseRivalSpotlight"
            : weekendState === "weather_unstable"
              ? "weatherUnstable"
              : "fallback";
  const actionHintKey =
    championshipState === "leader"
      ? "leader"
      : championshipState === "outsider"
        ? "outsider"
        : championshipState === "pressure"
          ? "pressure"
          : championshipState === "chase"
            ? "chase"
            : "fallback";
  const seedBase = buildSeed(
    championshipState,
    weekendState,
    trackName,
    rivalName,
    playerStanding?.id,
    nextRace?.rodada,
  );

  return {
    headline: pickVariant(EDITORIAL_COPY_POOLS.headline[headlineKey], `${seedBase}|headline`, context),
    paragraphs: [
      pickVariant(
        EDITORIAL_COPY_POOLS.championshipParagraph[championshipState],
        `${seedBase}|championship-paragraph`,
        context,
      ),
      pickVariant(
        EDITORIAL_COPY_POOLS.weekendParagraph[weekendParagraphKey],
        `${seedBase}|weekend-paragraph`,
        context,
      ),
    ].filter(Boolean),
    quote: pickVariant(EDITORIAL_COPY_POOLS.quote[quoteKey], `${seedBase}|quote`, context),
    rivalSummary: pickVariant(
      EDITORIAL_COPY_POOLS.rivalSummaryAhead[rivalSummaryKey],
      `${seedBase}|rival-summary`,
      context,
    ),
    rivalSupport: buildRivalSupport({ championshipState, briefingRival, rivalName, gapToLeader }),
    scenario: pickVariant(EDITORIAL_COPY_POOLS.scenario[scenarioKey], `${seedBase}|scenario`, context),
    actionHint: pickVariant(EDITORIAL_COPY_POOLS.actionHint[actionHintKey], `${seedBase}|action`, context),
    historyValue: buildHistoryValue(trackHistory, playerStanding),
    historyMeta: buildHistoryMeta(trackHistory, playerStanding),
    paddockSupport: storyLead ?? buildPaddockSupport({ weekendState, audienceEstimate, trackName }),
    weekendStoriesEmpty:
      "O paddock ainda não produziu manchetes fortes para esta etapa, entao a leitura segue focada na pista.",
    weekendStoriesMeta: buildWeekendStoriesMeta(weekendStories),
  };
}

function buildRivalSupport({ championshipState, briefingRival, rivalName, gapToLeader }) {
  if (briefingRival?.driver_name) {
    if (briefingRival.is_ahead) {
      if (championshipState === "outsider") {
        return `O duelo com ${briefingRival.driver_name} vale mais pela reaceleraçao da campanha do que pela conta do título neste momento.`;
      }

      return `O duelo com ${briefingRival.driver_name} ajuda a medir se a etapa serve para encurtar os ${gapToLeader} ponto${gapToLeader === 1 ? "" : "s"} para a ponta.`;
    }

    return `Passar ${briefingRival.driver_name} nesta rodada muda a leitura imediata do campeonato e limpa a pressão no bloco da frente.`;
  }

  return `A referência direta segue sendo ${rivalName}, especialmente no recorte esportivo desta rodada.`;
}


function buildPaddockSupport({ weekendState, audienceEstimate, trackName }) {
  if (weekendState === "weekend_hot") {
    return `O paddock trata ${trackName} como uma das rodadas mais tensas deste trecho da temporada.`;
  }

  if (weekendState === "history_negative") {
    return "A leitura do fim de semana passa menos por bravata e mais por disciplina de execução.";
  }

  if (audienceEstimate > 0) {
    return `A expectativa do paddock aponta para ${formatAudience(audienceEstimate)} de publico estimado ao longo do fim de semana.`;
  }

  return "O paddock espera bom movimento de publico nesta etapa.";
}

function buildHistoryValue(trackHistory, playerStanding) {
  const starts = trackHistory?.has_data ? (trackHistory.starts ?? 0) : recentHistoryStarts(playerStanding);
  return `${starts} ${starts === 1 ? "Largada" : "Largadas"}`;
}

function buildHistoryMeta(trackHistory, playerStanding) {
  if (trackHistory?.has_data) {
    if (trackHistory.best_finish == null) {
      return trackHistory.dnfs > 0
        ? `${trackHistory.dnfs} abandono${trackHistory.dnfs === 1 ? "" : "s"} nesta pista`
        : "Histórico discreto nesta pista até aqui.";
    }

    if ((trackHistory.best_finish ?? 99) <= 3 && (trackHistory.dnfs ?? 0) === 0) {
      return `Pista de boas lembranças: melhor resultado P${trackHistory.best_finish} na temporada ${trackHistory.last_visit_season ?? "atual"}.`;
    }

    if ((trackHistory.dnfs ?? 0) >= 1) {
      return `Ha velocidade para reagir aqui, mas o retrospecto inclui ${trackHistory.dnfs} abandono${trackHistory.dnfs === 1 ? "" : "s"}.`;
    }

    return `Melhor resultado: P${trackHistory.best_finish} (Temporada ${trackHistory.last_visit_season ?? "atual"})`;
  }

  const bestFinish = recentBestFinish(playerStanding);
  if (bestFinish == null) {
    return "Sem referência historica forte nesta pista.";
  }

  return `Melhor resultado recente: P${bestFinish}.`;
}

function buildWeekendStoriesMeta(stories) {
  if (!stories.length) {
    return "Sem chamadas fortes";
  }

  return `${stories.length} manchete${stories.length === 1 ? "" : "s"} filtrada${stories.length === 1 ? "" : "s"}`;
}

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
    ? `Nas tres leituras mais recentes você veio de ${readable}.`
    : "O momento recente ainda não criou uma tendência clara.";
}

function recentHistoryStarts(playerStanding) {
  if (!playerStanding?.results) return 0;
  return playerStanding.results.filter(Boolean).length;
}

function recentBestFinish(playerStanding) {
  if (!playerStanding?.results) return null;

  const finishes = playerStanding.results
    .filter((result) => result && !result.is_dnf && Number.isFinite(result.position))
    .map((result) => result.position);

  if (finishes.length === 0) {
    return null;
  }

  return Math.min(...finishes);
}

function formatAudience(value) {
  return value ? value.toLocaleString("pt-BR") : "-";
}
