#!/usr/bin/env node
// Gera o PACOTE DE VOZ do engenheiro: as peças pré-gravadas do rádio da equipe.
//
// A CONTAGEM não é escrita aqui de propósito. Ela sai de `engenheiro::catalogo()` no Rust e
// é publicada em `docs/engenheiro-catalogo.md`, que é gerado — número colado em comentário
// envelhece na primeira fala nova, e este arquivo já teve quatro contagens divergindo entre
// si. Quando precisar do total, leia o catálogo.
//
// Irmão mais velho do `spotter-pack.mjs`, e as decisões da POC valem iguais aqui — Cloud TTS
// com OAuth2 do ADC, Chirp 3 HD Algenib a 24 kHz, silêncio das bordas aparado, cadeia de
// rádio gravada dentro do arquivo. O que muda é a ESCALA e a origem da lista.
//
// ## As duas defesas contra a peça muda
//
// A Chirp 3 devolve 200 OK com silêncio dentro, e um `.wav` mudo no disco é PIOR que arquivo
// nenhum: a rodada seguinte o pula pelo "já existe", o `audio-para-opus` converte, e o
// engenheiro fica calado naquela resposta sem erro em lugar nenhum. As duas travas nasceram no
// pacote do spotter e desceram para cá em 12/08/2026:
//
//   • Nada vira arquivo antes de ser MEDIDO. `gerarUmaTomada` devolve amostras, quem grava é a
//     rodada, e só depois do veredito. Antes o `escreverWav` vinha primeiro e o apagamento
//     depois, então uma interrupção no meio (Ctrl+C, queda de rede, 429 numa das seis chamadas
//     simultâneas) deixava o inválido gravado e definitivo.
//   • O "já existe" RELÊ o arquivo (`jaEstaBoa`) em vez de olhar só o nome. Peça inutilizável
//     de rodada antiga volta para a fila sozinha.
//
// O piso que separa fala de silêncio é o mesmo dos dois pacotes, importado de
// `tts-poc/filtro-radio.mjs`, e o guard `scripts/tests/peca-de-voz-utilizavel.test.mjs` cobra
// as duas travas nos dois geradores.
//
// ## A lista não mora aqui
//
// As falas vêm de `docs/engenheiro-catalogo.json`, que sai do Rust — do mesmo
// `engenheiro::fala::catalogo()` que o app usa para saber o que TOCAR. Uma lista só, dois
// consumidores: o renderizador emite `frente_gap_1_2`, este script grava
// `frente_gap_1_2.wav`, e o teste de cobertura do crate prova que os dois conjuntos batem.
//
// Escrever as falas neste arquivo, como faz o pacote do spotter, funcionaria — e divergiria
// na primeira frase nova. Com mil peças a divergência não seria notada: o app pediria um
// arquivo inexistente e o engenheiro ficaria mudo naquela resposta específica, sem erro
// nenhum em lugar nenhum.
//
// Gere o JSON antes:
//   cargo test --lib despeja_catalogo_para_revisao
//
// ## O que este script NÃO gera
//
// As três peças que o engenheiro empresta do spotter (`ultima_volta`, `amarela`, `reparo`).
// Spotter e engenheiro são a mesma pessoa, e gravar a mesma frase duas vezes daria duas
// tomadas dela — a deriva de timbre que o pacote pré-gravado existe para evitar. Elas não
// estão no catálogo de propósito; o guard `scripts/tests/engenheiro-pecas-do-spotter.test.mjs`
// cuida desse acoplamento.
//
// ## Este script grava WAV, e o app não carrega WAV
//
// Os WAV daqui são os MASTERS: ficam no disco, fora do git, e servem ao auditor
// (`engenheiro-auditar.mjs`, que mede forma de onda). O que o app importa — e o que vai para o
// commit — é o Opus. São milhares de peças, e em PCM elas somam centenas de MB de repositório
// que não delta-comprime (a conta com os números do dia está em `audio-para-opus.mjs`).
// Depois de gerar, converta:
//
//   node scripts/audio-para-opus.mjs
//
// Esquecer esse passo tem o modo de falha da família: a peça nova existe em WAV, o guard de
// disco reclama do `.opus` ausente, e se o guard for ignorado o engenheiro fica MUDO naquela
// resposta específica, sem erro em lugar nenhum.
//
// Uso:
//   node scripts/engenheiro-pack.mjs                # só o que falta
//   node scripts/engenheiro-pack.mjs --refazer      # regera tudo
//   node scripts/engenheiro-pack.mjs --so frente_gap  # só as chaves com esse prefixo
//   node scripts/engenheiro-pack.mjs --sem-radio    # voz limpa, para comparar

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import {
  aparar,
  aplicarRadio,
  avaliarPeca,
  buracoInterno,
  escreverWav,
  lerWav,
} from "./tts-poc/filtro-radio.mjs";

const ENDPOINT = "https://texttospeech.googleapis.com/v1/text:synthesize";
const IDIOMA = "pt-BR";
const VOZ = "pt-BR-Chirp3-HD-Algenib";
const TAXA = 24000;
const CATALOGO = path.join("docs", "engenheiro-catalogo.json");
const DESTINO = path.join("src", "assets", "engenheiro");

/// Uma resposta do engenheiro é mais longa que um aviso de spotter — ela tem sujeito e
/// contexto ("O carro da frente está a um e dois.") em vez de duas sílabas. Acima disto o
/// modelo provavelmente interpretou em vez de ler, e vale conferir de ouvido.
const DURACAO_SUSPEITA_S = 4.5;

/// Quantas chamadas manter no ar ao mesmo tempo.
///
/// A cota da Cloud TTS é de 200 requisições por minuto, e a latência medida é de ~1,1 s por
/// chamada. Em série, mil peças levariam uns vinte minutos; com seis simultâneas, uns quatro — e
/// seis ficam bem abaixo do teto mesmo se todas voltarem instantaneamente (6 × 60 = 360/min
/// no pior caso teórico, mas a latência real segura em ~330). Subir mais só troca minutos por
/// risco de 429, e o 429 custa a espera exponencial do retry.
const SIMULTANEAS = 6;

/// Quantas vezes insistir num pedido recusado, e quanto esperar entre as tentativas.
///
/// A moderação do Google recusa frase inofensiva de forma NÃO determinística: medido na POC,
/// `Livre.` apanhou em 5 de 9 tentativas com o texto idêntico. Como é aleatório, insistir com
/// a MESMA frase funciona — não há o que reescrever, e reescrever seria deixar o moderador
/// decidir a redação do produto.
///
/// A espera cresce porque o outro motivo de recusa é cota (429), e aí insistir de imediato só
/// gasta tentativa.
const TENTATIVAS = 5;
const ESPERA_BASE_MS = 500;
/// Espera inicial do 429, que é outro problema: a cota é por MINUTO, e insistir dentro da
/// mesma janela em que ela estourou é insistir contra uma porta que só abre no minuto
/// seguinte. Oito segundos dobrando chegam a ~2 minutos no total das cinco tentativas.
const ESPERA_429_MS = 8000;

function lerArgumentos(argv) {
  const o = { refazer: false, radio: true, projeto: process.env.GCP_PROJECT || "", so: null };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--refazer") o.refazer = true;
    else if (a === "--sem-radio") o.radio = false;
    else if (a === "--projeto") o.projeto = argv[(i += 1)];
    else if (a === "--so") o.so = argv[(i += 1)];
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

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

const opcoes = lerArgumentos(process.argv);

if (!fs.existsSync(CATALOGO)) {
  console.error(
    `Sem ${CATALOGO}. Gere o catálogo a partir do Rust primeiro:\n` +
      "  cargo test --lib despeja_catalogo_para_revisao",
  );
  process.exit(1);
}
const catalogo = JSON.parse(fs.readFileSync(CATALOGO, "utf8"));

const acesso =
  process.env.GCP_TOKEN?.trim() ||
  gcloud(
    ["auth", "application-default", "print-access-token"],
    "Sem credencial. Rode uma vez:\n" +
      "  gcloud auth application-default login\n" +
      "  gcloud services enable texttospeech.googleapis.com",
  );
const projeto =
  opcoes.projeto ||
  gcloud(["config", "get-value", "project"], "Projeto de cota não resolvido. Passe --projeto <id>.");

fs.mkdirSync(DESTINO, { recursive: true });

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
    // Insistir por padrão, desistir só nos três status em que insistir é provadamente
    // inútil: 401 (chave de API recusada — precisa de OAuth2), 403 (falta o
    // `x-goog-user-project`) e 404 (endpoint ou voz inexistente). O importante é o que NÃO
    // está nessa lista: a recusa da moderação, cujo status é desconhecido — listá-la seria
    // uma aposta, e uma aposta errada derruba exatamente o caso que o retry existe para
    // cobrir.
    if ([401, 403, 404].includes(resposta.status)) throw new Error(ultima);
    if (tentativa < TENTATIVAS) {
      // O 429 pede uma espera de outra ordem. A cota da Cloud TTS é POR MINUTO, então os
      // 7,5 s que o backoff normal soma no total nem chegam à virada da janela — a rodada
      // completa perdeu 12 peças exatamente assim, todas em 429, todas depois de esgotar as
      // cinco tentativas dentro do mesmo minuto em que a cota já estava estourada.
      //
      // O jitter existe porque as seis chamadas simultâneas batem no teto JUNTAS e, com
      // espera fixa, voltariam juntas — reproduzindo o pico que causou o 429.
      const base = resposta.status === 429 ? ESPERA_429_MS : ESPERA_BASE_MS;
      const jitter = resposta.status === 429 ? Math.random() * 4000 : 0;
      await dormir(base * 2 ** (tentativa - 1) + jitter);
    }
  }
  throw new Error(`${TENTATIVAS} tentativas recusadas. Última: ${ultima}`);
}

/// Os pisos de pico e duração que separam "peça" de "silêncio com status 200" moram em
/// `tts-poc/filtro-radio.mjs` (`PICO_MINIMO`, `DURACAO_MINIMA_S`, `avaliarPeca`), junto com a
/// medição que os produziu, porque o pacote do spotter aplica exatamente os mesmos.

/// Silêncio DENTRO da tomada acima do qual ela é refeita.
///
/// Medido no acervo: fala de oração corrida sai com 0,01 s de buraco interno; a que o modelo
/// parte em dois fôlegos sai com 0,35 s — trinta e cinco vezes mais. Meio caminho entre os dois
/// separa os casos com folga dos dois lados.
///
/// E o fôlego é da TOMADA, não do texto: medida a mesma frase duas vezes em audição, ela saiu
/// limpa numa e com 0,28 s na outra. É por isso que refazer resolve — e é por isso que refazer
/// PRECISA existir, porque um aviso no console sobre uma peça que já está no disco não conserta
/// nada: a execução seguinte a pula pelo "já existe".
const BURACO_MAXIMO_S = 0.15;

/// Texto que PEDE respiro no meio: mais de uma frase numa peça só.
///
/// São as três do conselho de poupar ("A pista está comendo os carros hoje. Pega mais leve."),
/// onde a pausa é o certo e não o defeito. Sem esta isenção, o gerador as refaria cinco vezes
/// cada e ficaria com a última assim mesmo.
function respiroEsperado(texto) {
  return /[.;:—?!]\s+\S/.test(texto.trim());
}

async function gerarUma(chave, texto) {
  // Insiste enquanto a tomada voltar ruim. É o MESMO recurso do retry de HTTP e pelo mesmo
  // motivo: o defeito é não determinístico, então repetir o pedido idêntico funciona.
  //
  // Texto de uma letra é o caso patológico: `"e,"` devolveu silêncio duas vezes e meia vogal
  // na terceira. O modelo não erra — devolve 200 OK com lixo dentro.
  //
  // Vence a ÚLTIMA tentativa, e não a melhor: guardar a melhor pediria manter uma cópia por
  // tentativa em memória. A troca é boa de propósito — um fôlego que sobrevive a cinco tomadas
  // não é azar, é o texto, e o aviso do fim passa a ser diagnóstico em vez de ruído.
  //
  // Nada é gravado aqui. Quem grava é o chamador, e só depois de ler o veredito.
  const respiro = respiroEsperado(texto);
  for (let tentativa = 1; ; tentativa += 1) {
    const r = await gerarUmaTomada(chave, texto);
    const existe = r.utilizavel;
    const inteira = respiro || r.buraco <= BURACO_MAXIMO_S;
    if ((existe && inteira) || tentativa >= TENTATIVAS) {
      // MUDA é a peça que não existe — essa vai para falhas e NÃO vira arquivo. Fôlego teimoso
      // não: a peça é utilizável, e descartá-la deixaria um buraco no pacote por um defeito de
      // prosódia.
      if (!existe) r.mudo = true;
      else if (!inteira) r.folego = true;
      return r;
    }
    await new Promise((r2) => setTimeout(r2, ESPERA_BASE_MS * tentativa));
  }
}

/// Uma tomada: sintetiza, apara e aplica o rádio. NÃO grava nada.
///
/// Separar o processamento da gravação é o ponto todo, e é a mesma defesa que o pacote do
/// spotter já tinha. Aqui o `escreverWav` acontecia ANTES da medição: a peça muda virava
/// arquivo no disco e só depois era apagada, então bastava o processo morrer no meio — Ctrl+C,
/// queda de rede, 429 numa das seis chamadas simultâneas — para o `.wav` inválido ficar. E
/// ficar era permanente: a execução seguinte o pulava pelo "já existe", o `audio-para-opus`
/// convertia, e o pacote saía com um buraco que ninguém vê. O sintoma na pista é SILÊNCIO, que
/// é indistinguível de "o engenheiro não tinha o que dizer".
async function gerarUmaTomada(chave, texto) {
  const t0 = performance.now();
  const bruto = await sintetizar(texto);
  const ms = Math.round(performance.now() - t0);

  // O leitor de WAV da POC trabalha em cima de arquivo, então o bruto passa por um temporário
  // com nome iniciado por ponto — o `import.meta.glob` do Vite ignora, e um pacote
  // interrompido no meio não deixa lixo que o app tente carregar.
  const temporario = path.join(DESTINO, `.${chave}.cru.wav`);
  fs.writeFileSync(temporario, bruto);
  const { amostras, taxa } = lerWav(temporario);
  fs.unlinkSync(temporario);

  const antes = amostras.length / taxa;
  const cortado = aparar(amostras, taxa);
  const final = opcoes.radio ? aplicarRadio(cortado, taxa) : cortado;

  const peca = avaliarPeca(final, taxa);
  return {
    ms,
    final,
    taxa,
    antes,
    depois: peca.duracao,
    pico: peca.pico,
    utilizavel: peca.utilizavel,
    buraco: buracoInterno(cortado, taxa),
  };
}

/// A peça que já está no disco ainda é uma peça?
///
/// O "já existe" pula pelo NOME, e um `.wav` mudo — gravado por uma rodada antiga, de quando
/// este script gravava antes de medir — tem nome tão bom quanto o de uma fala boa. Sem esta
/// releitura, o único jeito de sair do buraco seria alguém desconfiar e apagar o arquivo à mão.
///
/// Custa uma leitura do acervo inteiro por rodada: medido em 12/08/2026, 300 peças em 180 ms,
/// ou seja ~0,6 ms por peça — poucos segundos para o acervo inteiro. É barato ao lado das
/// milhares de chamadas HTTP que
/// a rodada faz, e é o preço de o "já existe" voltar a significar "já está boa".
function jaEstaBoa(destino) {
  try {
    const { amostras, taxa } = lerWav(destino);
    return avaliarPeca(amostras, taxa);
  } catch {
    return { utilizavel: false, motivo: "não abre como WAV" };
  }
}

// ─── A rodada ────────────────────────────────────────────────────────────────

// A fila revalida o que já está no disco em vez de confiar no nome. Peça inutilizável de
// rodada antiga volta para a fila sozinha, e é assim que o buraco deixa de ser permanente.
const fila = [];
const ressuscitadas = [];
for (const entrada of catalogo) {
  const { chave } = entrada;
  if (opcoes.so && !chave.startsWith(opcoes.so)) continue;
  const destino = path.join(DESTINO, `${chave}.wav`);
  if (opcoes.refazer || !fs.existsSync(destino)) {
    fila.push(entrada);
    continue;
  }
  const disco = jaEstaBoa(destino);
  if (!disco.utilizavel) {
    ressuscitadas.push(`${chave} (${disco.motivo})`);
    fila.push(entrada);
  }
}

console.log(
  `\n${catalogo.length} falas no catálogo · ${fila.length} a gerar` +
    `${opcoes.so ? ` (filtro "${opcoes.so}")` : ""}` +
    `${opcoes.radio ? "" : " · SEM rádio"}\n`,
);
if (ressuscitadas.length) {
  console.log(`  ${ressuscitadas.length} peça(s) no disco estavam inutilizáveis e voltaram para a fila:`);
  for (const r of ressuscitadas.slice(0, 20)) console.log(`     ↻  ${r}`);
  if (ressuscitadas.length > 20) console.log(`     … e mais ${ressuscitadas.length - 20}`);
  console.log("");
}
if (!fila.length) {
  console.log("Nada a fazer. Use --refazer para regerar.\n");
  process.exit(0);
}

const avisos = [];
const falhas = [];
let concluidas = 0;

// Um worker por slot, todos comendo da mesma fila. Mais simples que fatiar em lotes e
// naturalmente balanceado: quem termina primeiro pega a próxima, então uma peça lenta não
// segura as outras cinco.
let proxima = 0;
async function trabalhador() {
  for (;;) {
    const i = proxima;
    proxima += 1;
    if (i >= fila.length) return;
    const { chave, texto } = fila[i];
    const destino = path.join(DESTINO, `${chave}.wav`);
    try {
      const r = await gerarUma(chave, texto);
      concluidas += 1;
      if (r.mudo) {
        // NADA é gravado, e o que houver de rodada antiga sai do disco. Um `.wav` mudo é pior
        // que arquivo nenhum: com ele, a execução seguinte pula a chave pelo "já existe" e o
        // buraco some de vista; sem ele, o gerador tenta de novo até alguém resolver.
        if (fs.existsSync(destino)) fs.unlinkSync(destino);
        falhas.push({
          chave,
          texto,
          motivo: `inutilizável em ${TENTATIVAS} tentativas (pico ${r.pico.toFixed(3)}, ${r.depois.toFixed(2)}s)`,
        });
        console.log(`  ✘  ${chave.padEnd(24)} INUTILIZÁVEL — pico ${r.pico.toFixed(3)}, ${r.depois.toFixed(2)}s, nada gravado`);
        continue;
      }

      escreverWav(destino, r.final, r.taxa);

      if (r.folego) {
        avisos.push(
          `${chave}: ${r.buraco.toFixed(2)}s de silêncio DENTRO da fala em ${TENTATIVAS} tentativas — ` +
            `sobreviveu ao refazer, então é o TEXTO que parte em dois fôlegos`,
        );
      } else if (r.buraco > 0) {
        avisos.push(`${chave}: ${r.buraco.toFixed(2)}s de silêncio dentro da fala`);
      }
      if (r.depois > DURACAO_SUSPEITA_S) {
        avisos.push(`${chave}: ${r.depois.toFixed(2)}s é longo demais para uma resposta de rádio`);
      }
      console.log(
        `  ${String(concluidas).padStart(3)}/${fila.length}  ${chave.padEnd(24)} ` +
          `${String(r.ms).padStart(5)} ms  ${r.antes.toFixed(2)}s → ${r.depois.toFixed(2)}s  ` +
          `pico ${r.pico.toFixed(2)}`,
      );
    } catch (e) {
      // Uma fala que não sai não pode derrubar as outras. E o resumo do fim é o que fecha a
      // conta: as JÁ geradas ficam no disco e a execução seguinte as pula pelo "já existe",
      // então sem o resumo o buraco fica invisível.
      const motivo = String(e.message || e).split("\n")[0];
      falhas.push({ chave, texto, motivo });
      console.log(`  ✘  ${chave.padEnd(24)} NÃO SAIU — ${motivo.slice(0, 100)}`);
    }
  }
}

await Promise.all(Array.from({ length: SIMULTANEAS }, trabalhador));

// ─── O resumo ────────────────────────────────────────────────────────────────

const noDisco = catalogo
  .map(({ chave }) => path.join(DESTINO, `${chave}.wav`))
  .filter((p) => fs.existsSync(p));
const bytes = noDisco.reduce((soma, p) => soma + fs.statSync(p).size, 0);
console.log(
  `\n${noDisco.length}/${catalogo.length} falas no disco · ${(bytes / 1024 / 1024).toFixed(1)} MB em WAV.`,
);

for (const aviso of avisos) console.warn(`  ⚠  ${aviso}`);

// Saída diferente de zero quando falta peça, para o release não empacotar um pacote com
// buraco achando que deu certo. O sintoma na pista de uma peça ausente é SILÊNCIO — que é
// indistinguível de "o engenheiro não tinha o que dizer".
if (falhas.length) {
  console.error(`\n  ${falhas.length} fala(s) NÃO gerada(s):`);
  for (const f of falhas) {
    console.error(`    ✘  ${f.chave.padEnd(24)} "${f.texto}"  —  ${f.motivo}`);
  }
  console.error(
    "\n  Rode de novo: as que saíram são puladas pelo \"já existe\" e só estas serão tentadas.\n" +
      "  A moderação é aleatória, então repetir com o MESMO texto costuma resolver.",
  );
  process.exitCode = 1;
} else if (noDisco.length < catalogo.length && !opcoes.so) {
  // Só alarma numa rodada COMPLETA. Com `--so` a rodada é parcial de propósito, e falhar ali
  // ensinaria a ignorar a saída de erro deste script — que é justamente o sinal que protege
  // o release de empacotar um pacote com buraco.
  console.error(
    `\n  Faltam ${catalogo.length - noDisco.length} falas no disco que não foram tentadas nesta rodada.`,
  );
  process.exitCode = 1;
}
