// i18n-ignore-file — bancada da POC de TTS, alcançada só pelo painel de laboratório
// (TtsPocPage, que já carrega a mesma marca). As falas daqui são material de medição de
// latência, e não copy de jogo.
// Etapa 3 — as falas do teste, em três categorias de comprimento crescente.
//
// Cada categoria tem DEZ variantes de tamanho parecido, e não uma frase repetida dez
// vezes. Isso não é enfeite: repetir o mesmo texto arrisca bater em cache do lado do
// servidor e produzir uma latência artificialmente boa, o que invalidaria a bateria
// inteira. Variantes de comprimento equivalente mantêm a comparação honesta.
//
// A `direcao` é o jeito do Gemini TTS receber atuação: o modelo não tem parâmetro de
// emoção, ele é dirigido por prompt. Ela entra ANTES do texto e conta como token de
// entrada — por isso o painel deixa desligar, para medir o custo dela.

export const CATEGORIAS = [
  {
    id: "curta",
    rotulo: "Curta e urgente",
    // No produto final este tipo continua PRÉ-GERADO e local. Está aqui só para
    // estabelecer o piso da latência possível.
    nota: "Piso de latência. No produto final continua pré-gerada e local.",
    direcao: "Fale como um spotter de automobilismo: urgente, seco, alto e muito rápido.",
    variantes: [
      "Carro à esquerda. Mantenha sua linha.",
      "Carro à direita. Segure o volante firme.",
      "Bandeira amarela no setor dois. Levante.",
      "Óleo na saída da curva quatro. Cuidado.",
      "Carro parado na reta oposta. Atenção.",
      "Dois carros ao lado. Não feche a porta.",
      "Detritos na entrada dos boxes. Desvie.",
      "Chuva forte chegando na curva um.",
      "Carro lento adiante. Prepare o desvio.",
      "Bandeira azul. Deixe o líder passar.",
    ],
  },
  {
    id: "media",
    rotulo: "Informativa média",
    nota: "Resposta contextual simples — o caso comum do engenheiro em corrida.",
    direcao: "Fale como um engenheiro de pista pelo rádio: calmo, objetivo, ritmo firme.",
    variantes: [
      "Daniel, o carro da frente está perdendo tempo no segundo setor. Continue pressionando, estamos nos aproximando.",
      "Daniel, o pneu traseiro esquerdo está aquecendo demais nas curvas longas. Suavize a entrada e recupere no final.",
      "Daniel, o segundo colocado parou nos boxes agora. Mantendo esse ritmo, você sai na frente dele na sua parada.",
      "Daniel, o consumo está três décimos acima do plano. Levante um pouco na reta e fechamos a corrida tranquilos.",
      "Daniel, o safety car deve entrar na próxima volta. Segure a posição e não force nada até a relargada.",
      "Daniel, seu ritmo está meio segundo melhor que o do líder. Mantenha a cabeça fria que a diferença cai sozinha.",
      "Daniel, a pista está secando na linha de corrida. Mais duas voltas e trocamos para os pneus de pista seca.",
      "Daniel, o carro de trás está com pneus novos e vem forte. Defenda o interior na freada da curva onze.",
      "Daniel, os comissários estão avaliando o incidente da largada. Nada decidido ainda, siga concentrado no ritmo.",
      "Daniel, faltam doze voltas e a janela de parada abre agora. Avise se o equilíbrio do carro mudar na frenagem.",
    ],
  },
  {
    id: "narrativa",
    rotulo: "Narrativa de sabor",
    nota: "O tipo de fala que só existe se a geração dinâmica for viável.",
    direcao:
      "Fale como um engenheiro de pista veterano falando com um piloto que ele conhece há anos: caloroso, pausado, íntimo.",
    variantes: [
      "Ei, Daniel… primeira classificação com a equipe nova. A garagem mudou, mas eu continuo aqui com você. Vamos colocar uma volta limpa no quadro.",
      "Daniel, faz três temporadas que não vínhamos para cá com um carro assim. Aproveite a manhã, sinta o traçado, e depois conversamos sobre o acerto.",
      "Sabe, Daniel, quando você chegou nesta equipe ninguém apostava um centavo. Hoje o box inteiro para para ver os seus tempos. Vamos honrar isso.",
      "Daniel, eu sei que esta pista te deve uma. No ano passado o motor apagou na última volta. Hoje o carro está inteiro e a decisão é sua.",
      "Bom dia, Daniel. Casa cheia, câmeras em cima e um contrato para renovar. Nada disso muda o que fazemos: uma volta de cada vez, do jeito de sempre.",
      "Daniel, o seu antigo engenheiro está do outro lado do box hoje. Ele conhece você bem, então vamos mudar um pouco a estratégia e surpreender o pessoal.",
      "Daniel, é a última corrida da temporada e o campeonato já era. Mas tem gente na arquibancada que veio só por você. Vamos dar um espetáculo de despedida.",
      "Daniel, dois anos atrás você me disse que um dia largaria na primeira fila aqui. Pois é, largamos. Agora falta a parte difícil, e ela começa agora.",
      "Ei, Daniel… a equipe passou a noite inteira reconstruindo este carro depois do acidente de ontem. Leve ele até o fim, é tudo o que o pessoal está pedindo.",
      "Daniel, olhe o tamanho desta arquibancada. Faz tempo que eu não via isso numa etapa de abertura. Respire fundo, aproveite, e vamos trabalhar.",
    ],
  },
];

// Etapa 11 — a fala pedida ANTES da linha de chegada, quando a vitória ainda é só
// provável. É a aposta de que uma geração de 1 a 3 segundos serve, desde que pedida
// com antecedência.
export const CATEGORIA_ANTECIPADA = {
  id: "antecipada",
  rotulo: "Comemoração antecipada",
  nota: "Pedida no último setor, tocada na linha — ou descartada se a vitória evaporar.",
  direcao: "Fale como um engenheiro de pista emocionado, segurando o choro, mas ainda profissional.",
  variantes: [
    "Última curva, Daniel… traz o carro para casa. Essa primeira vitória é nossa!",
    "Última curva, Daniel… é agora. Traz ele inteiro que a vitória é nossa!",
    "Fecha essa curva, Daniel… segura firme. Essa primeira é nossa, cara!",
    "Última curva, Daniel… não olhe para trás. Traz o carro para casa!",
    "É isso, Daniel… última curva. Traz ele para casa que a vitória é nossa!",
  ],
};

// Etapa 12 — o plano B, sempre local. Nunca depende de rede, nunca depende do Google.
export const FALA_LOCAL_DE_RESERVA = "Excelente trabalho. Leva o carro até a linha.";

export function categoriaPorId(id) {
  return [...CATEGORIAS, CATEGORIA_ANTECIPADA].find((c) => c.id === id) ?? CATEGORIAS[0];
}

/** Percorre as variantes em ciclo, para que dez disparos usem dez textos distintos. */
export function varianteDaVez(categoria, indice) {
  const lista = categoria.variantes;
  return lista[indice % lista.length];
}
