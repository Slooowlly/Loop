// AS REDAÇÕES DO SPOTTER: chave de evento → o que a voz diz.
//
// Módulo separado do gerador (`spotter-pack.mjs`) por um motivo só: ele é a FONTE ÚNICA
// destas frases e precisa ser importável sem efeito colateral. O gerador fala com a Cloud
// TTS e escreve no disco ao ser importado, então quem só quer o texto — o leitor do registro
// do rádio, `radio-timeline.mjs` — não pode importar de lá. Uma segunda cópia das redações
// seria a garantia de que as duas divergiriam na primeira regravação.
//
// Quem gera o áudio continua sendo o `spotter-pack.mjs`, que importa daqui.

// Chave → o que a voz diz.
//
// Curto por princípio: o aviso disputa atenção com uma freada, e cada sílaba a mais
// é atraso. "Tem um carro à sua esquerda" chega depois de a curva ter acabado.
//
// ── PONTUAÇÃO VIRA FÔLEGO PARTIDO ──
//
// Regra medida em quatro falas desta lista: **toda pausa escrita é um lugar onde o modelo
// pode respirar**, e a respiração aparece como silêncio DENTRO do arquivo. Custou:
//
//   "Atenção, carro fora da pista."   → 2,39 s com 0,21 s de buraco   → 1,32 s sem o "Atenção,"
//   "Dano no carro, box obrigatório." → 2,53 s com 0,48 s de buraco   → 1,85 s como "Reparo obrigatório nos boxes."
//   "Segura, segura. Vem carro."      → 2,30 s com 0,43 s de buraco   → 1,28 s como "Segura que vem carro."
//   "Pode ir, pista limpa."           → 1,68 s com 0,31 s de buraco   → 0,81 s como "Pode voltar."
//
// Não é determinístico — "Não entra ainda, vem carro." tem vírgula e saiu inteiriça —, mas
// a chance é real e o preço é alto: num aviso que vale por 2 a 5 segundos, meio segundo de
// silêncio no meio é meio aviso perdido. **Prefira uma oração só.** A conjunção ("Segura
// QUE vem carro") faz o trabalho da vírgula sem oferecer o ponto de respiro. O gerador
// avisa quando acontece — o `buracoInterno` existe para isto —, mas o aviso só chega depois
// de gastar a geração.
//
// REFINAMENTO, medido nas falas de relargada: **o que quebra com força é o TERMINADOR DE
// FRASE no meio da string** — ponto ou exclamação. Vírgula quebra às vezes; repetição
// separada por vírgula pode não quebrar nenhuma vez. "Verde, verde, verde!" saiu inteiriça
// a 1,53 s, enquanto "Verde! Vai, vai!" partiu em 0,28 s de buraco, e "Verde, vai, vai!"
// ainda partiu em 0,21 s. A saída que sempre funciona continua sendo a oração única:
// "Vai que é verde!" deu 0,90 s, a fala mais curta da família.
//
// Corolário útil: **dobrar a palavra é seguro e é a forma de dar urgência** (é o que o
// `ainda_ai_2` já fazia). Terminar uma oração no meio, não.
//
// ── E O QUE O MEDIDOR NÃO PEGA ──
//
// "Verde, verde, verde!" passou em TODAS as verificações — 1,53 s, zero buraco interno,
// pico saudável — e foi reprovada de ouvido: a voz lê a tripla em tom plano, sem escalada,
// e o que devia soar urgente soa mecânico. O `buracoInterno` e a duração pegam defeito
// MECÂNICO; tom não tem medidor. Nenhuma fala entra no pacote sem alguém ouvir, e este é
// o caso que prova por quê — dobrar funciona (`"Segura, segura."`), triplicar não.
// Chaves terminadas em `_2` / `_3` são VARIAÇÕES da mesma fala: o detector emite só a
// chave base e o reprodutor faz o rodízio. Vale sobretudo para o lembrete, que sai a
// cada 3 s enquanto a disputa dura — a mesma frase quatro vezes seguidas deixaria de
// ser informação e viraria alarme de carro.
export const FALAS = {
  esquerda: "Esquerda.",
  direita: "Direita.",
  // "Três largos" foi recusado de ouvido: quem corre no iRacing conhece o aviso como
  // "three wide". Só que escrever em inglês também não resolveu — a voz pt-BR tropeça
  // no `th` e no ditongo, e as três tentativas literais saíram ruins. O que funcionou
  // foi escrever o SOM em português: a voz lê "uáide" sem hesitar, e o resultado é o
  // termo que o piloto reconhece.
  //
  // A terceira variação é em português puro de propósito. Alternar é como se fala de
  // verdade, e ela é a rede de segurança: se um dia a voz mudar e o truque fonético
  // parar de funcionar, ainda sobra um aviso inteligível no rodízio.
  tres_largos: "Tri uáide, cuidado.",
  tres_largos_2: "Thríuaid, cuidado.",
  tres_largos_3: "Carro dos dois lados.",
  duas_esquerda: "Dois à esquerda.",
  duas_direita: "Dois à direita.",
  // LIBERAÇÃO. O `livre` puro só sai quando NÃO HÁ MAIS NINGUÉM dos dois lados; sair
  // de três largos para um carro só diz qual porta abriu. Um "Livre." sozinho depois
  // de três largos seria ambíguo justamente no instante em que o piloto decide o
  // movimento — e ambiguidade aqui custa contato.
  //
  // Sem variação de propósito: esta é a fala mais perigosa do pacote (se for entendida
  // errado, o piloto fecha a porta em cima de alguém), e três redações são três
  // chances de confundir. Uma frase, sempre a mesma, sempre inequívoca.
  livre: "Livre.",
  livre_esquerda: "Livre à esquerda.",
  livre_direita: "Livre à direita.",
  // O LEMBRETE, enquanto o carro do lado não sai — nomeando o lado, porque numa
  // disputa longa "ainda aí" não diz para onde não ir. As reticências que a ideia
  // original pedia ("Segura…") viraram repetição: pontuação de suspensão produz pausa
  // imprevisível, e dobrar a palavra dá a mesma insistência sem depender disso.
  ainda_esquerda: "Ainda à esquerda.",
  ainda_esquerda_2: "Continua à esquerda.",
  ainda_esquerda_3: "Ele segue à esquerda.",
  ainda_direita: "Ainda à direita.",
  ainda_direita_2: "Continua à direita.",
  ainda_direita_3: "Ele segue à direita.",
  // Os dois lados ocupados: não há um lado a nomear, e o que resta a dizer é sobre
  // segurar a posição.
  ainda_ai: "Ainda aí.",
  ainda_ai_2: "Segura, segura.",
  ainda_ai_3: "Mantém a posição.",
  // OBSTÁCULO À FRENTE. Sai uma vez por episódio, entre 2 e 5 segundos antes de o
  // piloto chegar ao ponto — a janela medida em `docs/spotter-obstaculo.md`.
  //
  // As três descrevem a situação AGORA, não o acontecimento. "Um carro saiu" narra o
  // passado; quando a fala chega ao ouvido o carro ainda está lá, e é isso que o piloto
  // precisa saber. A terceira é a curta, para quando o rádio já está cheio.
  //
  // Duas lições da audição, ambas pagas nesta terceira variação:
  //
  // NUNCA ELIDA O REFERENTE DE UM ESTADO. Ela já foi "Atenção, carro fora." e foi
  // reprovada: sem "da pista", o "fora" fica solto e o ouvido o preenche com o único
  // sujeito que uma fala de spotter sempre tem — o piloto que está ouvindo. Soava como
  // "você está fora da corrida". Encurtar é cortar palavra supérflua, nunca o que diz DE
  // QUEM se fala. As laterais podem elidir ("Esquerda.", "Livre.") porque ali o referente
  // É a volta do piloto e o idioma está consagrado; fala sobre OUTRO carro não tem essa
  // licença.
  //
  // "ATENÇÃO," CUSTA CARO E NÃO INFORMA. A correção seguinte foi "Atenção, carro fora da
  // pista.", e a geração mediu o preço: 2,39 s contra 1,32 s sem a interjeição — a
  // variação CURTA saiu a mais longa da família —, e o modelo ainda partiu a frase em
  // dois fôlegos, deixando 0,21 s de silêncio no meio de um aviso que vale por 2 a 5
  // segundos. Toda fala de perigo já é atenção; anunciá-la gasta o tempo da informação.
  carro_fora_frente: "Carro fora da pista logo à frente.",
  carro_fora_frente_2: "Um carro saiu da pista logo à frente.",
  carro_fora_frente_3: "Carro fora da pista.",
  // CARRO PARADO À FRENTE. A família de maior confiança do sistema — 15% de avisos
  // inúteis contra 35% da de cima, porque um carro parado tende a continuar parado.
  //
  // "Na pista" não é enfeite: é o que separa esta família da de cima. Um carro parado na
  // grama é obstáculo de beira; um carro parado na trajetória é o que se acerta em cheio,
  // e o piloto precisa saber qual dos dois vai encontrar.
  carro_parado_frente: "Carro parado na pista logo à frente.",
  carro_parado_frente_2: "Tem carro parado logo à frente.",
  carro_parado_frente_3: "Carro parado na pista.",
  // CARRO SAINDO DOS BOXES. Medido: 0,46 e 0,56 por piloto por corrida, nenhum piloto
  // ouvindo mais que duas vezes — e, ao contrário da família `lento` que ficou engavetada,
  // esta dispara em corrida VERDE (Lime Rock, 19 vezes, sem uma bandeira na prova).
  //
  // As três dizem o mesmo de três jeitos, todas em oração única: a IA vai aos boxes em
  // bloco, e quando isso acontece as três se alternam em vez de repetir a mesma frase.
  carro_saindo_box: "Carro saindo do box à frente.",
  carro_saindo_box_2: "Carro entrando na pista à frente.",
  carro_saindo_box_3: "Carro lento saindo do box.",
  // TRÁFEGO POR TRÁS. É um estado sustentado, não um aviso por carro: no acidente gravado
  // de Okayama, 18 carros distintos passaram pela janela em 22 s e o spotter disse três
  // coisas. Entrada, lembrete enquanto dura, liberação quando o trem passa.
  //
  // A entrada precisa dizer MAIS RÁPIDO. "Carro por trás" é sempre verdade numa corrida e
  // não é informação; o que é notícia é que este chega muito mais rápido que o piloto.
  carro_atras: "Carro mais rápido por trás.",
  carro_atras_2: "Vem carro por trás, deixa passar.",
  carro_atras_3: "Tráfego chegando por trás.",
  ainda_atras: "Ainda vem gente por trás.",
  ainda_atras_2: "Mais um chegando por trás.",
  ainda_atras_3: "Segue chegando gente.",
  // A liberação vai SEM variação, como o `livre`. Entendida errado, ela é a fala que faz
  // o piloto voltar a atacar com alguém ainda em cima dele — e a redação mais direta é a
  // que menos se presta a isso. Variar aqui seria trocar clareza por variedade no único
  // lugar onde a troca não compensa.
  livre_atras: "Livre atrás.",
  // RETORNO À PISTA. A chamada arquetípica do ofício: o piloto rodou, está parado ou de ré,
  // sem espelho e sem visão, e a única pergunta que ele tem é "posso entrar?".
  //
  // Medido: 12 retornos reais em duas corridas, 0,15 por piloto. E o número que desenhou a
  // família — em 11 de 16 episódios a pista estava limpa no retorno, ou seja, um "pode ir"
  // automático erraria cinco vezes. Errar para esse lado é o erro do `livre`: o piloto
  // entra na frente de alguém porque nós dissemos que dava.
  //
  // "Segura" é a insistência de quem está vendo o que o piloto não vê; a repetição dobrada
  // faz o trabalho que um ponto de exclamação faria num texto.
  segura_volta: "Segura que vem carro.",
  segura_volta_2: "Não entra ainda, vem carro.",
  segura_volta_3: "Espera, tem gente chegando.",
  // A liberação vai SEM variação, como `livre` e `livre_atras`: é a fala que faz o piloto
  // ENTRAR na pista, e três redações são três chances de confundir no pior instante.
  pode_voltar: "Pode voltar.",
  // BANDEIRAS. O `session_flags` já era lido desde sempre e o rótulo já aparecia na tela;
  // o que faltava era a boca. Medido: 8 transições em 36 min de corrida real, na faixa das
  // famílias raras. Uma fala por episódio e nenhum lembrete — a amarela de Okayama durou
  // 350 s, e repetir ali viraria alarme de carro.
  //
  // A amarela vem primeiro porque é a que fecha um buraco nosso: o `spotter_tras` emudece
  // sob amarela de propósito ("deixa passar" é instrução errada quando ninguém pode
  // passar), e hoje o piloto perde o aviso sem saber por quê.
  amarela: "Bandeira amarela.",
  amarela_2: "Amarela, cuidado à frente.",
  amarela_3: "Amarela na pista.",
  // A branca vira "Última volta" porque é isso que um spotter diz. Ninguém narra a cor de
  // uma bandeira que o piloto já vai ver — narra o que ela significa para ele.
  // A RELARGADA — a amarela apagando. Medido: `pace_mode` não serve de sinal (fica
  // constante a corrida inteira em Lime Rock), e o que marca o instante é a borda de
  // descida da amarela. Em Okayama, uma vez em 17 minutos.
  //
  // É a fala mais perecível do pacote: ela diz "agora vale correr", e chegar tarde é
  // chegar depois de o campo já ter acelerado. Daí as três serem as mais curtas que a
  // informação permite.
  verde: "Bandeira verde, acelera!",
  verde_2: "Vai que é verde!",
  verde_3: "Relargada, acelera!",
  ultima_volta: "Última volta.",
  // O meatball é do SEU carro. É ordem, não aviso, e a redação diz o que fazer.
  //
  // A PRETA foi retirada: no Loop ela é o NOSSO mecanismo de dano — o `car::breakdown`
  // dispara `!black #N <segundos>` para simular quebra de peça, com o tempo de parada
  // próprio de cada componente. Anunciá-la diria "você foi punido por pilotagem" quando o
  // que houve foi o motor quebrando. A informação certa ali é sobre a PEÇA, e quem a tem
  // é o engenheiro. Não recrie a chave sem resolver essa ambiguidade.
  reparo: "Reparo obrigatório nos boxes.",
  // CLIMA. O mais raro do sistema: as duas corridas de formato 3 são inteiramente secas, e
  // a única progressão do acervo é a de Ledenon — `track_wetness` 1→2 aos 927 s e 2→3 aos
  // 1018 s. Uma fala por travessia de FAIXA (seca / úmida / molhada), não por degrau do
  // canal: as duas transições de Ledenon caem na mesma faixa e dão uma fala só.
  //
  // Sem variação. Cada uma sai no máximo uma vez por travessia, e uma corrida raramente
  // produz duas — não há repetição a quebrar, e três redações seriam três chances de
  // confundir uma informação que decide o pneu que o piloto vai pedir.
  pista_molhando: "Pista começando a molhar.",
  pista_molhada: "Pista molhada agora.",
  pista_secando: "Pista secando.",
  // O teste antes da largada. Aqui a pressa não existe, e a frase precisa dizer
  // QUEM está falando — é o que transforma "ouvi um som" em "o spotter está de pé".
  teste: "Spotter na escuta.",
};
