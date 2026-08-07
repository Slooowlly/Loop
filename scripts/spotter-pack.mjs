#!/usr/bin/env node
// Gera o PACOTE DE VOZ do spotter: uma fala por chave de evento.
//
// É o primeiro pacote de verdade a sair da POC (ver docs/tts-poc-latencia.md), e
// segue as decisões que ela custou a estabelecer:
//
//   • Cloud TTS com OAuth2 do ADC (chave de API é recusada) e `x-goog-user-project`.
//   • Chirp 3 HD Algenib, 24 kHz — a voz escolhida na audição.
//   • Silêncio das bordas APARADO. Aqui isso não é estética, é latência: meio
//     segundo de silêncio de cabeça é meio segundo de atraso num aviso que só vale
//     enquanto o carro ainda está do lado.
//   • Cadeia de rádio BAKED IN. Estas falas nunca são coladas umas nas outras — cada
//     uma sai sozinha —, então não há a razão que obrigava a filtrar depois de
//     juntar, e filtrar na geração deixa o app com um `<audio>` e nada mais.
//
// O nome do arquivo é a CHAVE do evento (`iracing_sdk::spotter`). Sem carimbo de
// tempo: o app importa por nome, e um pacote que muda de nome a cada geração não
// serviria de pacote.
//
// Grava WAV, que é o MASTER e fica fora do git. O app carrega o Opus — depois de gerar, rode
// `node scripts/audio-para-opus.mjs`. Ver o cabeçalho de `engenheiro-pack.mjs` para o porquê.
//
// Uso:
//   node scripts/spotter-pack.mjs             # só o que falta
//   node scripts/spotter-pack.mjs --refazer   # regera tudo
//   node scripts/spotter-pack.mjs --sem-radio # grava a voz limpa (para comparar)

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { aparar, aplicarRadio, buracoInterno, escreverWav, lerWav, pico } from "./tts-poc/filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const IDIOMA = "pt-BR";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const DESTINO = path.join("src", "assets", "spotter");

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
const FALAS = {
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

// Uma fala do spotter é curta. Bem mais longa que isto significa que o modelo
// resolveu interpretar em vez de anunciar, e vale conferir de ouvido.
const DURACAO_SUSPEITA_S = 2.0;

function lerArgumentos(argv) {
  const o = { refazer: false, radio: true, projeto: process.env.GCP_PROJECT || "" };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--refazer") o.refazer = true;
    else if (a === "--sem-radio") o.radio = false;
    else if (a === "--projeto") o.projeto = argv[(i += 1)];
  }
  return o;
}

function gcloud(args, erro) {
  try {
    return execFileSync("gcloud", args, { encoding: "utf8", shell: true }).trim();
  } catch {
    console.error(erro);
    process.exit(1);
  }
}

const opcoes = lerArgumentos(process.argv);
const acesso =
  process.env.GCP_TOKEN?.trim() ||
  gcloud(
    ["auth", "application-default", "print-access-token"],
    "Sem credencial. Rode uma vez:\n" +
      "  gcloud auth application-default login\n" +
      "  gcloud services enable texttospeech.googleapis.com",
  );
const projeto =
  opcoes.projeto || gcloud(["config", "get-value", "project"], "Projeto de cota não resolvido. Passe --projeto <id>.");

fs.mkdirSync(DESTINO, { recursive: true });

/// Quantas vezes insistir num pedido recusado, e quanto esperar entre as tentativas.
///
/// A moderação de conteúdo do Google recusa frase inofensiva de forma NÃO determinística:
/// medido na POC, `Livre.` apanhou em 5 de 9 tentativas com o texto idêntico. Como é
/// aleatório, insistir com a mesma frase funciona — não há o que reescrever, e reescrever
/// seria deixar o moderador decidir a redação do produto.
///
/// A espera cresce porque o outro motivo de recusa é cota (429), e aí insistir de imediato
/// só gasta tentativa. Cinco tentativas com 0,5 s, 1 s, 2 s, 4 s cobrem os dois casos e
/// somam menos de 8 s no pior caso de um arquivo.
const TENTATIVAS = 5;
const ESPERA_BASE_MS = 500;

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

async function sintetizar(texto) {
  let ultima;
  for (let tentativa = 1; tentativa <= TENTATIVAS; tentativa += 1) {
    const resposta = await fetch(ENDPOINT, {
      method: "POST",
      headers: {
        authorization: `Bearer ${acesso}`,
        "x-goog-user-project": projeto,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        input: { text: texto },
        voice: { languageCode: IDIOMA, name: VOZ },
        audioConfig: { audioEncoding: "LINEAR16", sampleRateHertz: TAXA },
      }),
    });
    if (resposta.ok) {
      const { audioContent } = await resposta.json();
      return Buffer.from(audioContent, "base64"); // LINEAR16 já vem com cabeçalho RIFF
    }
    const corpo = (await resposta.text()).slice(0, 500);
    ultima = `HTTP ${resposta.status}: ${corpo}`;
    // A regra é INSISTIR POR PADRÃO, e desistir só nos três status em que insistir é
    // provadamente inútil: 401 (a API recusa chave de API — precisa de OAuth2), 403
    // (falta o `x-goog-user-project`) e 404 (endpoint ou voz que não existe). Todos os
    // três estão medidos em `docs/tts-poc-latencia.md` e nenhum melhora com repetição.
    //
    // O importante é o que NÃO está nessa lista: a recusa da moderação. O doc registra a
    // mensagem ("sensitive words that violate the Prohibited Use policy") e não o status,
    // e foi medida no Gemini TTS enquanto isto aqui fala com a Cloud TTS. Como o status é
    // desconhecido, listar quem se repete seria apostar — e uma aposta errada aqui derruba
    // exatamente o caso para o qual esta função existe.
    if (resposta.status === 401 || resposta.status === 403 || resposta.status === 404) {
      throw new Error(ultima);
    }
    if (tentativa < TENTATIVAS) {
      const moderacao = /sensitive words|Prohibited Use/i.test(corpo);
      process.stdout.write(
        `   ↻  ${moderacao ? "moderação recusou" : `recusa ${resposta.status}`}` +
          ` na tentativa ${tentativa}, insistindo com o MESMO texto\n`,
      );
      await dormir(ESPERA_BASE_MS * 2 ** (tentativa - 1));
    }
  }
  throw new Error(`${TENTATIVAS} tentativas recusadas. Última: ${ultima}`);
}

const avisos = [];
const falhas = [];
let gerados = 0;

for (const [chave, texto] of Object.entries(FALAS)) {
  const destino = path.join(DESTINO, `${chave}.wav`);
  if (!opcoes.refazer && fs.existsSync(destino)) {
    console.log(`   ·  ${chave.padEnd(14)} já existe`);
    continue;
  }

  const t0 = performance.now();
  // Uma fala que não sai não pode derrubar as outras. Antes, a exceção subia e a rodada
  // morria no meio — com o agravante de que as JÁ GERADAS ficavam no disco, então a
  // execução seguinte as pulava pelo `já existe` e o buraco virava invisível. O resumo do
  // fim é que fecha a conta.
  let bruto;
  try {
    bruto = await sintetizar(texto);
  } catch (e) {
    falhas.push({ chave, texto, motivo: String(e.message || e).split("\n")[0] });
    console.log(`   ✘  ${chave.padEnd(14)} NÃO SAIU — ${String(e.message || e).slice(0, 120)}`);
    continue;
  }
  const ms = Math.round(performance.now() - t0);

  // Grava, lê de volta pelo mesmo leitor que o resto da POC usa e processa.
  const temporario = path.join(DESTINO, `.${chave}.cru.wav`);
  fs.writeFileSync(temporario, bruto);
  const { amostras, taxa } = lerWav(temporario);
  fs.unlinkSync(temporario);

  const antes = amostras.length / taxa;
  const cortado = aparar(amostras, taxa);
  const final = opcoes.radio ? aplicarRadio(cortado, taxa) : cortado;
  escreverWav(destino, final, taxa);

  const depois = final.length / taxa;
  const buraco = buracoInterno(cortado, taxa);
  if (buraco > 0) {
    avisos.push(`${chave}: ${buraco.toFixed(2)}s de silêncio DENTRO da fala — o modelo partiu em dois fôlegos`);
  }
  if (depois > DURACAO_SUSPEITA_S) {
    avisos.push(`${chave}: ${depois.toFixed(2)}s é longo demais para um aviso de spotter`);
  }

  gerados += 1;
  console.log(
    `  ${String(gerados).padStart(2)}  ${chave.padEnd(14)} ${String(ms).padStart(5)} ms  ` +
      `${antes.toFixed(2)}s → ${depois.toFixed(2)}s  pico ${pico(final).toFixed(2)}  ${destino}`,
  );
}

const bytes = Object.keys(FALAS)
  .map((c) => path.join(DESTINO, `${c}.wav`))
  .filter((p) => fs.existsSync(p))
  .reduce((soma, p) => soma + fs.statSync(p).size, 0);
console.log(`\n${Object.keys(FALAS).length} falas, ${(bytes / 1024).toFixed(0)} KB no total.`);

for (const aviso of avisos) console.warn(`  ⚠  ${aviso}`);

// O que não saiu, no fim e em voz alta. Um pacote incompleto não avisa sozinho: o detector
// emite a chave, a camada de voz não acha o arquivo, e o sintoma na pista é silêncio — que
// é indistinguível de "não havia o que falar". Saída diferente de zero para que o release
// não empacote um pacote com buraco achando que deu certo.
if (falhas.length) {
  console.error(`\n  ${falhas.length} fala(s) NÃO gerada(s):`);
  for (const f of falhas) console.error(`    ✘  ${f.chave.padEnd(14)} "${f.texto}"  —  ${f.motivo}`);
  console.error(
    `\n  Rode de novo: as que saíram são puladas pelo "já existe" e só estas serão tentadas.\n` +
      `  A moderação é aleatória, então repetir com o MESMO texto costuma resolver.`,
  );
  process.exitCode = 1;
}
