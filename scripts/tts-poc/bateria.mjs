#!/usr/bin/env node
// Bateria headless da POC de TTS (Etapas 6 e 7) — o mesmo cano do app, sem a janela.
//
// POR QUE EXISTE, se o painel do app já dispara: trinta gerações são trinta cliques e
// vinte minutos de alguém olhando para a tela. Aqui a bateria roda sozinha, inclusive
// a parte chata — a pausa de vários minutos para provocar a chamada fria.
//
// O QUE ELE NÃO MEDE, e é honesto dizer: o "primeiro som". Fora do navegador não há
// relógio de áudio confiável, então este script para no PRIMEIRO BLOCO DE ÁUDIO
// RECEBIDO. Esse é o termo dominante e o que varia; o que falta até o alto-falante é
// uma constante do dispositivo (pré-buffer + latência de saída) que o painel do app
// mede e imprime. Some os dois para ter o número que o jogador sente.
//
// Uso:
//   node scripts/tts-poc/bateria.mjs                      # bateria padrão (10x3)
//   node scripts/tts-poc/bateria.mjs --repeticoes 3       # ensaio rápido
//   node scripts/tts-poc/bateria.mjs --pausa-min 5        # inclui a chamada fria
//   node scripts/tts-poc/bateria.mjs --sem-streaming      # modo controle
//   node scripts/tts-poc/bateria.mjs --relatorio arquivo.jsonl   # só recalcula

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { CATEGORIAS, varianteDaVez } from "../../src/dev/tts/ttsScripts.js";
import { resumir, classificar, formatarMs, formatarPercentual } from "../../src/dev/tts/ttsMetrics.js";

const ENDPOINT = "https://generativelanguage.googleapis.com/v1beta/interactions";
const MODELO_PADRAO = "gemini-3.1-flash-tts-preview";
const VOZ_PADRAO = "Charon";
const TAXA = 24000;
const BYTES_POR_AMOSTRA = 2;
const INTERVALO_MS = 1200; // respiro entre chamadas: medir serviço, não rate limit
// Camada gratuita do 3.1 Flash TTS: 3 requisições por minuto (o próprio erro de cota
// informa `limit: 3`). Na camada paga o teto é outro — passe `--rpm`.
const RPM_PADRAO = 3;
const JANELA_MS = 60000;

/**
 * Limitador de janela deslizante. Existe para PRESERVAR a medição, não só para evitar
 * o 429: estourar a cota devolve 200 com um evento de erro no meio do SSE, e essas
 * chamadas entrariam na amostra como se fossem falha do serviço.
 *
 * Importante que ele deixe a rajada acontecer em vez de espaçar tudo igualmente: se
 * cada chamada saísse a 21 s da anterior, TODAS seriam "mornas" e a comparação entre
 * chamada quente e fria — o ponto da Etapa 6 — desapareceria.
 */
function criarLimitador(rpm) {
  const disparos = [];
  return async function aguardarVez() {
    if (rpm <= 0) return;
    for (;;) {
      const agora = Date.now();
      while (disparos.length && agora - disparos[0] >= JANELA_MS) disparos.shift();
      if (disparos.length < rpm) {
        disparos.push(agora);
        return;
      }
      const esperar = JANELA_MS - (agora - disparos[0]) + 250;
      process.stdout.write(`  (cota: aguardando ${(esperar / 1000).toFixed(0)} s)\r`);
      await new Promise((r) => setTimeout(r, esperar));
    }
  };
}

function lerArgumentos(argv) {
  const opcoes = {
    repeticoes: 10,
    modelo: MODELO_PADRAO,
    voz: VOZ_PADRAO,
    streaming: true,
    usarDirecao: true,
    pausaMin: 0,
    intervaloMs: INTERVALO_MS,
    rpm: RPM_PADRAO,
    saida: path.join("docs", "tts-poc", "bateria.jsonl"),
    salvarAudio: true,
    relatorio: null,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    const proximo = () => argv[(i += 1)];
    if (a === "--repeticoes") opcoes.repeticoes = Number(proximo());
    else if (a === "--modelo") opcoes.modelo = proximo();
    else if (a === "--voz") opcoes.voz = proximo();
    else if (a === "--sem-streaming") opcoes.streaming = false;
    else if (a === "--sem-direcao") opcoes.usarDirecao = false;
    else if (a === "--pausa-min") opcoes.pausaMin = Number(proximo());
    else if (a === "--intervalo") opcoes.intervaloMs = Number(proximo());
    else if (a === "--rpm") opcoes.rpm = Number(proximo());
    else if (a === "--sem-audio") opcoes.salvarAudio = false;
    else if (a === "--saida") opcoes.saida = proximo();
    else if (a === "--relatorio") opcoes.relatorio = proximo();
    else if (a === "--ajuda" || a === "-h") opcoes.ajuda = true;
  }
  return opcoes;
}

/** Mesma varredura do lado Rust: acha o base64 do áudio onde quer que ele esteja. */
const CHAVES_AUDIO = ["data", "audio", "audioData", "audio_data", "inlineData", "inline_data", "b64"];

function pareceBase64(s) {
  return typeof s === "string" && s.length >= 64 && /^[A-Za-z0-9+/=]+$/.test(s);
}

function coletarAudio(valor, saida = []) {
  if (Array.isArray(valor)) {
    for (const item of valor) coletarAudio(item, saida);
    return saida;
  }
  if (valor && typeof valor === "object") {
    const ehTexto = valor.type === "text";
    for (const [chave, v] of Object.entries(valor)) {
      if (!ehTexto && CHAVES_AUDIO.includes(chave) && pareceBase64(v)) {
        saida.push(v);
        continue;
      }
      coletarAudio(v, saida);
    }
  }
  return saida;
}

function resolverChave() {
  const chave = (process.env.GEMINI_API_KEY || process.env.GOOGLE_API_KEY || "").trim();
  if (!chave) {
    console.error(
      "Defina GEMINI_API_KEY antes de rodar.\n" +
        '  PowerShell:  $env:GEMINI_API_KEY = "sua-chave"\n' +
        '  bash:        export GEMINI_API_KEY="sua-chave"',
    );
    process.exit(1);
  }
  return chave;
}

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * Envelopa o PCM cru num WAV tocável. A API devolve `audio/l16` sem cabeçalho — sem
 * estes 44 bytes nenhum player abre o arquivo.
 *
 * Salvar é padrão, e não opcional, por experiência própria: a primeira rodada desta
 * POC mediu tudo e não guardou uma única fala, então não deu para ouvir se a voz
 * prestava nem para checar se o modelo leu a direção de atuação em voz alta. Latência
 * sem áudio é meia medição.
 */
function embrulharWav(pcm, taxa = TAXA, canais = 1) {
  const cabecalho = Buffer.alloc(44);
  const bytesPorSegundo = taxa * canais * BYTES_POR_AMOSTRA;
  cabecalho.write("RIFF", 0);
  cabecalho.writeUInt32LE(36 + pcm.length, 4);
  cabecalho.write("WAVE", 8);
  cabecalho.write("fmt ", 12);
  cabecalho.writeUInt32LE(16, 16); // tamanho do bloco fmt
  cabecalho.writeUInt16LE(1, 20); // 1 = PCM sem compressão
  cabecalho.writeUInt16LE(canais, 22);
  cabecalho.writeUInt32LE(taxa, 24);
  cabecalho.writeUInt32LE(bytesPorSegundo, 28);
  cabecalho.writeUInt16LE(canais * BYTES_POR_AMOSTRA, 32); // alinhamento do bloco
  cabecalho.writeUInt16LE(BYTES_POR_AMOSTRA * 8, 34); // bits por amostra
  cabecalho.write("data", 36);
  cabecalho.writeUInt32LE(pcm.length, 40);
  return Buffer.concat([cabecalho, pcm]);
}

/** Nome previsível e ordenável: a pasta vira a própria lista de reprodução. */
function nomeDoArquivo(registro, indice) {
  const carimbo = registro.quando.replace(/[-:]/g, "").replace(/\..+$/, "");
  const n = String(indice + 1).padStart(2, "0");
  return `${carimbo}-${registro.categoria}-${n}-${registro.fase}.wav`;
}

const estado = { ultimaEm: null, jaRodou: false };

function faseAtual() {
  if (!estado.jaRodou) return "primeira";
  const delta = Date.now() - estado.ultimaEm;
  if (delta >= 120000) return "fria";
  if (delta >= 20000) return "morna";
  return "sequencia";
}

async function gerar({
  chave,
  categoria,
  texto,
  modelo,
  voz,
  streaming,
  usarDirecao,
  aguardarVez,
  dirAudio,
  indice = 0,
}) {
  if (aguardarVez) await aguardarVez();
  const fase = faseAtual();
  const msDesdeUltima = estado.ultimaEm == null ? null : Date.now() - estado.ultimaEm;
  const entrada = usarDirecao ? `${categoria.direcao}\n\n${texto}` : texto;

  const registro = {
    quando: new Date().toISOString(),
    origem: "bateria-headless",
    modelo,
    voz,
    idioma: "pt-BR",
    categoria: categoria.id,
    texto,
    caracteres: texto.length,
    palavras: texto.trim().split(/\s+/).length,
    streaming,
    direcao: usarDirecao ? categoria.direcao : null,
    arquivoAudio: null,
    fase,
    msDesdeUltima,
    msAteRespostaHttp: null,
    msPrimeiroBloco: null,
    // No headless, "primeiro som" é o primeiro bloco: não há saída de áudio aqui.
    msPrimeiroSom: null,
    msTotal: null,
    duracaoAudioMs: null,
    blocos: 0,
    bytesAudio: 0,
    interrupcoes: 0,
    sucesso: false,
    erro: null,
    // Quando o servidor responde 200 mas não manda áudio, o motivo está nos eventos de
    // controle do SSE. Sem guardá-los a falha vira "não veio áudio" e não se investiga.
    eventos: null,
    detalheFalha: null,
  };

  const pedacos = [];
  const t0 = performance.now();
  estado.jaRodou = true;
  estado.ultimaEm = Date.now();

  try {
    const resposta = await fetch(ENDPOINT, {
      method: "POST",
      headers: {
        "x-goog-api-key": chave,
        "content-type": "application/json",
        accept: streaming ? "text/event-stream" : "application/json",
      },
      body: JSON.stringify({
        model: modelo,
        input: entrada,
        response_format: { type: "audio" },
        generation_config: { speech_config: [{ voice: voz }] },
        stream: streaming,
      }),
    });

    registro.msAteRespostaHttp = performance.now() - t0;

    if (!resposta.ok) {
      const corpo = (await resposta.text()).slice(0, 400);
      registro.erro = `HTTP ${resposta.status}: ${corpo}`;
      registro.msTotal = performance.now() - t0;
      return registro;
    }

    const anotar = (b64) => {
      if (registro.msPrimeiroBloco == null) {
        registro.msPrimeiroBloco = performance.now() - t0;
        registro.msPrimeiroSom = registro.msPrimeiroBloco; // ver cabeçalho
      }
      registro.blocos += 1;
      const bytes = Buffer.from(b64, "base64");
      registro.bytesAudio += bytes.length;
      pedacos.push(bytes);
    };

    // Diário dos eventos de controle (tudo menos os deltas de áudio, que são milhares).
    const diario = [];
    const anotarEvento = (valor) => {
      if (valor?.delta?.type === "audio") return;
      if (diario.length < 40) diario.push(JSON.stringify(valor).slice(0, 500));
    };

    if (streaming) {
      const decodificador = new TextDecoder();
      let sobra = "";
      for await (const pedaco of resposta.body) {
        sobra += decodificador.decode(pedaco, { stream: true });
        const linhas = sobra.split("\n");
        sobra = linhas.pop() ?? "";
        for (const linha of linhas) {
          const bruto = linha.trim();
          if (!bruto.startsWith("data:")) continue;
          const json = bruto.slice(5).trim();
          if (!json || json === "[DONE]") continue;
          let valor;
          try {
            valor = JSON.parse(json);
          } catch {
            continue;
          }
          anotarEvento(valor);
          for (const b64 of coletarAudio(valor)) anotar(b64);
        }
      }
    } else {
      const valor = await resposta.json();
      anotarEvento(valor);
      for (const b64 of coletarAudio(valor)) anotar(b64);
    }

    registro.msTotal = performance.now() - t0;
    registro.duracaoAudioMs = (registro.bytesAudio / (TAXA * BYTES_POR_AMOSTRA)) * 1000;
    registro.sucesso = registro.blocos > 0;
    if (!registro.sucesso) {
      registro.eventos = diario;
      registro.detalheFalha = diario.join(" | ").slice(0, 1200);
      // Cota estourada não é falha do serviço — é cadência errada do teste. Misturar as
      // duas coisas inflaria o percentual de falhas e reprovaria o Gemini por engano.
      registro.quotaExcedida = /quota_exceeded|exceeded your current quota/i.test(
        registro.detalheFalha,
      );
      registro.erro = registro.quotaExcedida
        ? "Cota da camada gratuita estourada (não é falha do serviço)."
        : `Sem áudio (200). Eventos: ${registro.detalheFalha || "nenhum"}`;
    }
  } catch (e) {
    registro.erro = String(e?.message ?? e);
    registro.msTotal = performance.now() - t0;
  }

  if (registro.sucesso && dirAudio) {
    // O primeiro bloco pode trazer cabeçalho RIFF; nesse caso o PCM já vem embrulhado
    // e embrulhar de novo geraria um arquivo com dois cabeçalhos.
    const pcm = Buffer.concat(pedacos);
    const jaTemCabecalho = pcm.length > 12 && pcm.toString("ascii", 0, 4) === "RIFF";
    const arquivo = path.join(dirAudio, nomeDoArquivo(registro, indice));
    fs.writeFileSync(arquivo, jaTemCabecalho ? pcm : embrulharWav(pcm));
    registro.arquivoAudio = path.relative(process.cwd(), arquivo);
  }

  return registro;
}

function imprimirTabela(titulo, linhas) {
  console.log(`\n${titulo}`);
  const cabecalho = [
    "Conjunto",
    "n",
    "falhas",
    "melhor",
    "mediana",
    "média",
    "P90",
    "P95",
    "pior",
    "<1s",
    "<1,5s",
    "<2s",
    ">3s",
  ];
  const corpo = linhas
    .filter((l) => l.resumo.total > 0)
    .map((l) => [
      l.rotulo,
      String(l.resumo.sucessos),
      formatarPercentual(l.resumo.percentualFalhas),
      formatarMs(l.resumo.melhor),
      formatarMs(l.resumo.mediana),
      formatarMs(l.resumo.media),
      formatarMs(l.resumo.p90),
      formatarMs(l.resumo.p95),
      formatarMs(l.resumo.pior),
      formatarPercentual(l.resumo.abaixo1000),
      formatarPercentual(l.resumo.abaixo1500),
      formatarPercentual(l.resumo.abaixo2000),
      formatarPercentual(l.resumo.acima3000),
    ]);

  const larguras = cabecalho.map((c, i) =>
    Math.max(c.length, ...corpo.map((linha) => linha[i].length)),
  );
  const formatar = (celulas) =>
    celulas.map((c, i) => (i === 0 ? c.padEnd(larguras[i]) : c.padStart(larguras[i]))).join("  ");

  console.log(formatar(cabecalho));
  console.log(larguras.map((l) => "-".repeat(l)).join("  "));
  for (const linha of corpo) console.log(formatar(linha));
}

/**
 * Cota é reconhecida pela EVIDÊNCIA no registro, não só pela marca gravada: registros
 * de rodadas anteriores guardaram os eventos do SSE antes de a marca existir.
 */
function barradoPorCota(r) {
  if (r.quotaExcedida === true) return true;
  return /quota_exceeded|exceeded your current quota/i.test(
    `${r.detalheFalha ?? ""} ${r.erro ?? ""}`,
  );
}

function relatar(todos) {
  // Chamadas barradas por cota saem da amostra: elas não mediram o serviço.
  const barradas = todos.filter(barradoPorCota);
  // Registros anteriores à captura de diagnóstico não têm a chave `eventos`. Sem os
  // eventos do SSE não dá para saber se foi cota ou falha de verdade — e chutar em
  // qualquer direção falsearia o percentual de falhas. Ficam fora, contados à parte.
  const semDiagnostico = todos.filter(
    (r) => !r.sucesso && !barradoPorCota(r) && !("eventos" in r),
  );
  const registros = todos.filter((r) => !barradoPorCota(r) && (r.sucesso || "eventos" in r));

  const porCategoria = CATEGORIAS.map((c) => ({
    rotulo: c.rotulo,
    resumo: resumir(registros.filter((r) => r.categoria === c.id)),
  }));
  const porFase = ["primeira", "fria", "morna", "sequencia"].map((f) => ({
    rotulo: `fase: ${f}`,
    resumo: resumir(registros.filter((r) => r.fase === f)),
  }));
  const geral = resumir(registros);

  if (barradas.length) {
    console.log(
      `\n${barradas.length} chamada(s) barrada(s) por cota — fora da amostra ` +
        "(não são falha do serviço; ajuste --rpm).",
    );
  }
  if (semDiagnostico.length) {
    console.log(
      `${semDiagnostico.length} registro(s) antigo(s) sem diagnóstico de SSE — fora da amostra ` +
        "(não dá para saber se foi cota ou falha).",
    );
  }

  imprimirTabela("Tempo até o primeiro bloco de áudio (ms)", [
    ...porCategoria,
    ...porFase,
    { rotulo: "TOTAL", resumo: geral },
  ]);

  const veredito = classificar(geral);
  console.log(`\nVeredito preliminar (sem o custo de reprodução): ${veredito.rotulo}`);
  console.log(`  ${veredito.detalhe}`);
  console.log(
    "\nLembrete: some o pré-buffer e a latência de saída medidos no painel do app\n" +
      "para chegar ao tempo até o PRIMEIRO SOM, que é o critério real da Etapa 8.",
  );

  const falhas = registros.filter((r) => !r.sucesso);
  if (falhas.length) {
    console.log(`\nFalhas (${falhas.length}):`);
    const contagem = new Map();
    for (const f of falhas) {
      const chave = (f.erro ?? "desconhecido").slice(0, 120);
      contagem.set(chave, (contagem.get(chave) ?? 0) + 1);
    }
    for (const [erro, n] of contagem) console.log(`  ${n}x  ${erro}`);
  }
}

async function principal() {
  const opcoes = lerArgumentos(process.argv);

  if (opcoes.ajuda) {
    console.log(fs.readFileSync(new URL(import.meta.url), "utf8").split("\n").slice(1, 26).join("\n"));
    return;
  }

  if (opcoes.relatorio) {
    const linhas = fs
      .readFileSync(opcoes.relatorio, "utf8")
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean)
      .map((l) => JSON.parse(l));
    relatar(linhas);
    return;
  }

  const chave = resolverChave();
  fs.mkdirSync(path.dirname(opcoes.saida), { recursive: true });
  const fluxo = fs.createWriteStream(opcoes.saida, { flags: "a" });
  opcoes.aguardarVez = criarLimitador(opcoes.rpm);

  if (opcoes.salvarAudio) {
    opcoes.dirAudio = path.join(path.dirname(opcoes.saida), "audio");
    fs.mkdirSync(opcoes.dirAudio, { recursive: true });
  }

  console.log(
    `Modelo ${opcoes.modelo} | voz ${opcoes.voz} | streaming ${opcoes.streaming ? "on" : "off"} | ` +
      `${opcoes.repeticoes}x por categoria | teto ${opcoes.rpm} req/min`,
  );
  console.log(opcoes.dirAudio ? `Áudio em ${opcoes.dirAudio}` : "Sem gravação de áudio.");

  const registros = [];
  for (const categoria of CATEGORIAS) {
    for (let i = 0; i < opcoes.repeticoes; i += 1) {
      const texto = varianteDaVez(categoria, i);
      const r = await gerar({ chave, categoria, texto, indice: i, ...opcoes });
      registros.push(r);
      fluxo.write(`${JSON.stringify(r)}\n`);
      const marca = r.sucesso ? formatarMs(r.msPrimeiroBloco) : `FALHA — ${r.erro?.slice(0, 80)}`;
      console.log(
        `  ${categoria.id.padEnd(10)} ${String(i + 1).padStart(2)}/${opcoes.repeticoes}  ` +
          `${String(marca).padStart(10)}  (${r.fase})`,
      );
      await dormir(opcoes.intervaloMs);
    }
  }

  // A chamada fria é o caso que mais assusta em produção: o jogador abre o jogo e a
  // primeira fala é a pior de todas. Sem esperar de verdade, não dá para medir.
  if (opcoes.pausaMin > 0) {
    console.log(`\nPausa de ${opcoes.pausaMin} min para provocar a chamada fria...`);
    await dormir(opcoes.pausaMin * 60000);
    for (const categoria of CATEGORIAS) {
      const r = await gerar({
        chave,
        categoria,
        texto: varianteDaVez(categoria, 7),
        indice: 7,
        ...opcoes,
      });
      registros.push(r);
      fluxo.write(`${JSON.stringify(r)}\n`);
      console.log(`  fria  ${categoria.id.padEnd(10)} ${formatarMs(r.msPrimeiroBloco)}`);
      await dormir(opcoes.intervaloMs);
    }
  }

  fluxo.end();
  console.log(`\nRegistros anexados em ${opcoes.saida}`);
  relatar(registros);
}

principal().catch((e) => {
  console.error(e);
  process.exit(1);
});
