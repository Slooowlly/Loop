#!/usr/bin/env node
// Arreio de validação do spotter, sobre CORRIDA JÁ GRAVADA.
//
// Todo número que sustenta o spotter hoje — a janela de 2 a 5 s, o limiar de 40% de perda
// de ritmo, a taxa de 0,38 avisos por piloto, os 40 falsos positivos da largada parada —
// saiu de script descartável, rodado uma vez e perdido. A consequência prática é que não
// dá para revalidar nada: quando um detector muda, ou quando chega a próxima captura, a
// única saída é reescrever tudo. E o custo disso já apareceu uma vez, quando uma métrica
// mal escolhida (contar fechamento de episódio em vez de utilidade real) produziu uma
// recomendação errada que só foi pega na corrida seguinte.
//
//   node scripts/analise-spotter.mjs <captura.jsonl.gz | pasta>... [--avisos] [--sem-jogador-real]
//
// Aceita N caminhos, arquivos e pastas misturados. O caminho curto — o que se roda quando
// uma frente nova entrega — é passar as duas capturas de referência direto:
//
//   node scripts/analise-spotter.mjs race_1785885657.jsonl.gz race_1785889561.jsonl.gz
//
// A verificação somada precisa das duas na MESMA rodada, e nada do que entra na linha de
// comando é ignorado em silêncio: o que não for lido aparece no bloco ENTRADA com o
// motivo. Um argumento engolido calado é como um teste que não roda — parece verde.
//
// `--avisos` lista aviso por aviso (piloto, obstáculo, distância, tempo até chegar,
// veredito). É por onde se descobre POR QUE um número não bate, em vez de aceitar o
// agregado — foi um agregado que escondeu a métrica errada da primeira medição.
//
// O que sai:
//   1. os episódios que o detector encontra, com desfecho;
//   2. os avisos, com cada carro da IA simulado como jogador — nas duas capturas o jogador
//      real fica no box, e sem essa simulação a via do rádio nunca é exercitada;
//   3. quantos avisos foram ÚTEIS, medido na chegada e não no registro;
//   4. a varredura de piso de permanência × teto de velocidade;
//   5. a taxa-base de obstáculo à frente.
//
// NÃO é um teste de `npm run test:structure`, de propósito: as capturas moram em
// `%APPDATA%` e não estão no repositório. Um guard que dependesse delas quebraria em
// qualquer máquina limpa e no CI. É ferramenta de análise, rodada à mão.

import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

// ═══════════════════════ As constantes vêm do fonte Rust ═══════════════════════
//
// Este arreio é um ESPELHO em JS de detector escrito em Rust, e espelho derivado é pior
// que espelho nenhum: valida algo que não é o que roda. Copiar os números para cá seria
// exatamente isso. Então eles são lidos do fonte como texto — a mesma técnica que a suíte
// `scripts/tests/*.test.mjs` usa para pegar regressão visual sem screenshot.
//
// Se uma constante sumir ou mudar de nome, isto FALHA ALTO. Um padrão silencioso aqui
// produziria uma tabela inteira de números plausíveis e errados.

function lerConstantesRust(relativo, nomes, expressoes = {}) {
  const abs = path.join(RAIZ, relativo);
  const fonte = fs.readFileSync(abs, "utf8");
  const valores = {};
  const faltando = [];
  // Constantes que não são um literal: uma OU de bits, por exemplo. O padrão vem da tabela
  // DETECTORES, mas o VALOR continua saindo do fonte — só a forma de lê-lo é que muda.
  for (const [nome, re] of Object.entries(expressoes)) {
    const m = re.exec(fonte);
    if (!m) {
      faltando.push(nome);
      continue;
    }
    const partes = m[1].split("|").map((x) => x.trim()).filter(Boolean);
    if (!partes.length || partes.some((x) => !/^(0x[0-9a-f]+|\d+)$/i.test(x))) {
      faltando.push(nome);
      continue;
    }
    valores[nome] = partes.reduce((a, x) => a | Number(x), 0);
  }
  for (const nome of nomes) {
    const re = new RegExp(
      `^\\s*(?:pub\\s+)?const\\s+${nome}\\s*:\\s*\\w+\\s*=\\s*(0x[0-9a-fA-F_]+|-?[\\d_]+(?:\\.\\d+)?)\\s*;`,
      "m",
    );
    const m = re.exec(fonte);
    if (!m) {
      faltando.push(nome);
      continue;
    }
    valores[nome] = Number(m[1].replace(/_/g, ""));
  }
  if (faltando.length) {
    throw new Error(
      `${relativo}: constante ausente ou fora do formato esperado — ${faltando.join(", ")}.\n` +
        `  O arreio lê os limiares do fonte justamente para não derivar dele. Se a constante ` +
        `mudou de nome, atualize a tabela DETECTORES; não use um padrão inventado, porque ` +
        `o relatório inteiro passaria a validar outro detector.`,
    );
  }
  return valores;
}

/// Os detectores que este arreio sabe reproduzir.
///
/// Acrescentar o detector de uma frente nova é acrescentar UMA ENTRADA aqui — a lista de
/// constantes que ele lê do próprio fonte, as famílias que ele emite e a fábrica do
/// espelho. O laço de análise não muda.
const DETECTORES = [
  {
    nome: "frente",
    fonte: "src-tauri/src/iracing_sdk/spotter_frente.rs",
    obrigatorio: true,
    constantes: [
      "SUP_FORA_DO_MUNDO",
      "SUP_FORA_DA_PISTA",
      "SUP_NA_CAIXA",
      "SUP_ENTRANDO_BOX",
      "SUP_NA_PISTA",
      "ESTADO_CORRIDA",
      "JANELA_VEL_S",
      "PICO_JANELA_S",
      "PICO_BALDES",
      "PICO_MIN_KMH",
      "FRACAO_RITMO",
      "TTA_MIN_S",
      "TTA_MAX_S",
      "DIST_MAX_M",
      "PARADO_KMH",
      "SALTO_MAX_S",
      "AUSENCIA_MAX_S",
      "MAX_CARROS",
    ],
    forma: "pontual",
    familias: [
      { chave: "fora", rotulo: "fora da pista" },
      { chave: "parado", rotulo: "parado" },
    ],
    // Cada pool disputa um tique entre si. `fora` e `parado` são medidas em separado
    // porque só a primeira tem áudio hoje — a segunda é medida COMO SE tivesse.
    pools: [["fora"], ["parado"]],
    preparar: prepararFrente,
    criar: (K) => new EspelhoFrente(K),
  },
  // Frente B — carro lento à frente, relativo ao campo.
  {
    nome: "lento",
    fonte: "src-tauri/src/iracing_sdk/spotter_lento.rs",
    obrigatorio: false,
    constantes: [
      "SUP_FORA_DO_MUNDO", "SUP_FORA_DA_PISTA", "SUP_NA_CAIXA", "SUP_ENTRANDO_BOX",
      "SUP_NA_PISTA", "ESTADO_CORRIDA", "JANELA_VEL_S", "PICO_JANELA_S", "PICO_BALDES",
      "PICO_MIN_KMH", "PARADO_KMH", "TRECHOS", "AMOSTRAS_TRECHO", "MIN_AMOSTRAS_TRECHO",
      "MIN_CAMPO", "CORTE", "CORTE_SAIDA", "CORTE_GRAVE", "PERMANENCIA_S", "TTA_MIN_S",
      "TTA_MAX_S", "DIST_MAX_M", "DIST_MIN_M", "SALTO_MAX_S", "AUSENCIA_MAX_S", "MAX_CARROS",
    ],
    forma: "pontual",
    familias: [
      { chave: "lento", rotulo: "lento" },
      { chave: "muito_lento", rotulo: "muito lento" },
    ],
    // Um pool só: o grau sai da razão no instante do aviso, e os dois graus disputam o
    // mesmo tique. Separá-los na arbitragem daria dois avisos onde o detector dá um.
    pools: [["lento", "muito_lento"]],
    preparar: prepararLento,
    criar: (K) => new EspelhoLento(K),
    // A métrica do CONTRATO pergunta se o carro ainda estava abaixo de 70% do PRÓPRIO pico
    // recente. Ela foi escrita para obstáculo — carro que saiu da pista ou parou — e num
    // trecho lento o pico de um carro normal é o da reta anterior, o que faz ritmo normal
    // parecer problema. Pior: um carro PERSISTENTEMENTE lento tem o próprio pico igualmente
    // baixo, a razão contra si mesmo fica perto de 1, e a métrica o absolve.
    //
    // As duas alternativas abaixo trocam a referência de "ele mesmo" para "o campo naquele
    // instante" — a mesma pergunta que o detector faz para abrir. Uma usa o corte de saída
    // (com histerese), a outra o corte de entrada. As três aparecem juntas porque é aí que
    // mora a divergência contra o número publicado pela frente B, e escolher uma delas
    // caladamente seria escolher o número.
    utilidades: [
      {
        rotulo: "razão < CORTE_SAIDA do campo",
        ainda: (m, d, K, q, idx) => razaoDeCampo(m, d, K, q, idx) < K.CORTE_SAIDA,
      },
      {
        rotulo: "razão < CORTE do campo",
        ainda: (m, d, K, q, idx) => razaoDeCampo(m, d, K, q, idx) < K.CORTE,
      },
    ],
  },
  // Frente A — carro chegando por trás / bandeira azul.
  {
    nome: "tras",
    fonte: "src-tauri/src/iracing_sdk/spotter_tras.rs",
    obrigatorio: false,
    constantes: [
      "SUP_FORA_DA_PISTA", "SUP_NA_CAIXA", "SUP_ENTRANDO_BOX", "SUP_NA_PISTA",
      "ESTADO_CORRIDA", "BANDEIRA_AZUL", "JANELA_RITMO_S", "FRACAO_LENTO", "FRACAO_RETOMOU",
      "CONFIRMA_S", "CONFIRMA_AZUL_S", "SAIDA_BOX_S", "DIST_MAX_M", "TTA_MAX_S",
      "FECHAMENTO_MIN_MS", "LIBERA_S", "LEMBRETE_S", "LEMBRETE_PASSO_S", "LEMBRETE_MAX_S",
      "CAMPO_MIN_MS", "CAMPO_MIN_CARROS", "AUSENCIA_MAX_S", "SALTO_MAX_S", "MAX_CARROS",
    ],
    // `BANDEIRAS_AMARELA` é uma OU de quatro bits (`0x8 | 0x100 | ...`), que a extração de
    // constante simples não lê. Vem da mesma linha do fonte, por uma expressão própria —
    // ainda do fonte, nunca copiada.
    expressoes: { BANDEIRAS_AMARELA: /const\s+BANDEIRAS_AMARELA\s*:\s*i32\s*=\s*([0-9a-fx|\s]+);/ },
    forma: "sustentado",
    familias: [{ chave: "tras", rotulo: "tráfego atrás" }],
    pools: [],
    // O jogador REAL entra como piloto simulado aqui, e só aqui: em Okayama é ELE o
    // acidente que a frente A existe para prever. Fora dele o caso não está no acervo.
    incluiJogadorReal: true,
    preparar: prepararTras,
    criar: (K) => new EspelhoTras(K),
  },
];

/// Fração do pico recente abaixo da qual um carro DE VOLTA à pista ainda é problema.
///
/// Não sai do fonte Rust porque não é do detector: é da MÉTRICA. O detector decide o que
/// vira aviso; isto decide se o aviso serviu. São 70% e não os 60% da abertura de
/// propósito — o critério de utilidade é mais frouxo que o de detecção, senão um carro
/// que já recuperou parte do ritmo mas ainda atravanca contaria como não-problema.
const FRACAO_UTIL = 0.7;

/// A varredura do item 4. Piso de permanência (s) × teto de velocidade do obstáculo no
/// instante do aviso (km/h). `Infinity` é a configuração que roda hoje: sem teto.
const VARREDURA_PISO_S = [0, 0.5, 1.0, 1.5];
const VARREDURA_TETO_KMH = [Infinity, 100, 70, 50, 30];

/// As janelas da taxa-base (m).
const JANELAS_BASE_M = [100, 150, 200, 300];

/// Capturas maiores que isto são puladas quando se passa a PASTA inteira — a de julho tem
/// 240 MB de Open Qualify e arrasta a rodada toda. Passar o arquivo direto na linha de
/// comando ignora este limite.
const LIMITE_PASTA_MB = 64;

/// `--avisos` na linha de comando: lista aviso por aviso, com o veredito de cada um.
const DETALHAR_AVISOS = process.argv.includes("--avisos");

/// `--linha-do-tempo=<arquivo.json>`: despeja o instante de cada fala de spotter, por piloto,
/// para `scripts/analise-radio.mjs` juntar com as famílias do engenheiro. Sem a bandeira, nada
/// muda — o relatório de sempre sai igual.
const LINHA_DO_TEMPO =
  process.argv.find((a) => a.startsWith("--linha-do-tempo="))?.slice("--linha-do-tempo=".length) ??
  null;

/// `--sem-jogador-real`: o carro do jogador da gravação deixa de contar como obstáculo.
///
/// Por padrão ele conta, porque é o que o detector faria — o Rust só ignora o próprio
/// jogador, e para um piloto da IA o carro parado do humano é um obstáculo como qualquer
/// outro. Mas nas duas capturas o humano passa a prova no box: quando ele aparece imóvel
/// na pista é o guincho, não incidente de corrida. Os dois números são diferentes e os
/// dois interessam, então o relatório mostra sempre os dois e a bandeira escolhe qual vale.
const IGNORAR_JOGADOR_REAL = process.argv.includes("--sem-jogador-real");

/// `--azul-para-todos`: dá a bandeira azul do humano a todo piloto simulado.
///
/// Diagnóstico, não configuração. A azul vive no `session_flags` GLOBAL, que é o canal do
/// jogador de verdade: dá-la a um carro da IA é inventar sinal que aquele carro não teve.
/// Existe para responder UMA pergunta — se a diferença entre o que este arreio mede e o
/// que a frente A publicou vem daí — e a resposta entra no relatório em vez de virar
/// suposição.
const AZUL_PARA_TODOS = process.argv.includes("--azul-para-todos");

/// `--trilha=tras:5` (ou `--trilha=tras:5:920-935`): por que o estado NÃO abre para aquele
/// carro.
///
/// Um espelho que reporta "zero" não diz nada: pode ser fiel ao detector ou pode ter um
/// defeito calado. Isto abre a caixa — a cada tique, a fração de ritmo, o campo, quem vinha
/// atrás, e QUAL condição faltou. Sem janela, sai só a tabulação por motivo sobre a corrida
/// inteira; com janela, sai tique a tique.
const TRILHA = (() => {
  const arg = process.argv.find((a) => a.startsWith("--trilha="));
  if (!arg) return null;
  const [detector, carro, janela] = arg.slice("--trilha=".length).split(":");
  const idx = Number(carro);
  if (!detector || !Number.isInteger(idx)) {
    console.error(`--trilha malformado: use --trilha=<detector>:<carro>[:<t0>-<t1>]`);
    process.exit(1);
  }
  let de = -Infinity;
  let ate = Infinity;
  if (janela) {
    const [a, b] = janela.split("-").map(Number);
    if (!Number.isFinite(a) || !Number.isFinite(b)) {
      console.error(`--trilha: janela \`${janela}\` não é <t0>-<t1> em segundos`);
      process.exit(1);
    }
    de = a;
    ate = b;
  }
  return { detector, carro: idx, de, ate };
})();

// irsdk_TrkSurf — o QUE o carro pisa. Separa a roçada na área asfaltada do carro enterrado
// na brita, que é a diferença entre não-evento e obstáculo.
const MATERIAL = {
  "-1": "ausente", 0: "fora do mundo", 1: "asfalto1", 2: "asfalto2", 3: "asfalto3",
  4: "asfalto4", 5: "concreto", 6: "pedra", 7: "terra", 8: "grama1", 9: "grama2",
  10: "grama3", 11: "grama4", 12: "areia", 13: "brita1", 14: "brita2", 15: "grama5",
  16: "brita3", 17: "brita4", 18: "brita5", 19: "brita6", 20: "brita7", 21: "brita8",
};

const DESFECHO = {
  retomou: "retomou",
  box: "foi para o box",
  sumiu: "sumiu do array",
  saiu: "saiu da pista",
  parou: "parou",
  ultrapassado: "ultrapassado",
  sessao: "sessão acabou",
  aberto: "ainda aberto no fim da captura",
};

// ═══════════════════════════ Leitura da captura ═══════════════════════════

/// As capturas são truncadas quando o app fecha, e um `createGunzip()` em pipe estoura com
/// `Z_BUF_ERROR`. `Z_SYNC_FLUSH` entrega o que deu para descomprimir, que é o que se quer
/// de um log.
function linhasDaCaptura(arquivo) {
  const bruto = arquivo.endsWith(".gz")
    ? zlib.gunzipSync(fs.readFileSync(arquivo), { finishFlush: zlib.constants.Z_SYNC_FLUSH })
    : fs.readFileSync(arquivo);
  return bruto.toString("utf8").split(/\r?\n/);
}

// ═══════════════════════════ Passe 1: o mundo ═══════════════════════════
//
// A leitura é CRUA e comum aos três detectores: o que o gravador viu, sem derivação. Cada
// detector deriva o que é dele em `preparar` (velocidade, mapa de trecho, acumuladores de
// ritmo), porque as três derivações são diferentes e cada uma tem as suas constantes.
//
// O que é derivado NÃO depende de quem é o jogador, e é por isso que sai daqui e não de
// dentro de cada jogador simulado: fazer as mesmas contas quarenta vezes daria os mesmos
// números por quarenta vezes o custo.

/// Teto de índice na leitura. Os três detectores usam `MAX_CARROS = 64`; se algum subir,
/// o carregamento falha alto em vez de ler o mundo pela metade.
const MAX_LEITURA = 64;

/// O que o YAML diz de cada sessão do fim de semana.
///
/// `SessionInfo.Sessions` é uma LISTA — treino, classificatório, corrida —, e o
/// `SessionType` de cada uma vem depois do `SessionNum` dela. Pegar o primeiro
/// `SessionType` do arquivo, que era o que o arreio fazia, rotula um fim de semana inteiro
/// pelo nome da primeira sessão. Numa captura de qualify + corrida isso imprimia "Open
/// Qualify" para uma captura que contém uma corrida.
function tiposDeSessao(yaml) {
  const tipos = new Map();
  const bloco = /SessionInfo:\s*\n([\s\S]*?)(?=\n\w)/.exec(yaml);
  const texto = bloco ? bloco[1] : yaml;
  const re = /-\s*SessionNum:\s*(\d+)([\s\S]*?)(?=\n\s*-\s*SessionNum:|$)/g;
  let mm;
  while ((mm = re.exec(texto)) !== null) {
    const tipo = /SessionType:\s*(.+)/.exec(mm[2]);
    tipos.set(Number(mm[1]), tipo ? tipo[1].trim() : null);
  }
  return tipos;
}

/// Uma sessão é de corrida? Só ela entra nas somas e na verificação.
///
/// Conservador de propósito: tipo desconhecido NÃO é corrida. `session_state == 4` vale em
/// classificatório também, então sem esta separação 476 s de treino entram na amostra como
/// se fossem prova — e é exatamente o que aconteceu na primeira gravação do acervo.
function ehCorrida(tipo) {
  return !!tipo && /race/i.test(tipo) && !/qualify/i.test(tipo);
}

/// Lê a captura inteira e devolve UM mundo por `session_num`.
///
/// Cada sessão é um mundo separado, com seu relógio, seus quadros e seu tipo. Nas capturas
/// de formato 3 há uma sessão só e isto não muda nada; num fim de semana gravado inteiro é
/// a diferença entre dois blocos e um blocão.
function lerMundos(arquivo) {
  const MAX = MAX_LEITURA;
  const mundos = new Map();
  let comprimentoM = 0;
  let pista = null;
  let versao = null;
  let tipos = new Map();
  /// `car_left_right` não existe no formato 1 — o campo veio depois. Ausência do CAMPO e
  /// canal PARADO são diagnósticos diferentes: o segundo some na próxima corrida, o
  /// primeiro é definitivo para aquela gravação.
  let temCanalLateral = false;

  const novoMundo = (num) => ({
    arquivo: path.basename(arquivo),
    sessaoNum: num,
    pista: null,
    comprimentoM: 0,
    tipoSessao: null,
    versao: null,
    temCanalLateral: false,
    n: 0,
    quadrosTotais: 0,
    duracaoS: 0,
    maxIdx: -1,
    primeiro: null,
    ultimo: null,
    nBruto: 0, lrBruto: [], estadoBruto: [], lrNaPista: [],
    t: [], estado: [], comprimento: [], bandeiras: [], replay: [],
    jogIdx: [], jogPct: [], jogVelMs: [], jogNaPista: [], jogNoCarro: [], jogPos: [],
    carros: Array.from({ length: MAX }, () => ({
      presente: [], pct: [], sup: [], mat: [], pit: [], pos: [], est: [],
    })),
  });

  for (const linha of linhasDaCaptura(arquivo)) {
    if (!linha) continue;
    let o;
    try {
      o = JSON.parse(linha);
    } catch {
      continue; // última linha de um `.gz` truncado
    }
    if (o.kind === "header") {
      versao = o.version;
      continue;
    }
    if (o.kind === "session") {
      const c = /TrackLength:\s*([\d.]+)\s*km/.exec(o.yaml);
      if (c) comprimentoM = parseFloat(c[1]) * 1000;
      const p = /TrackDisplayName:\s*(.+)/.exec(o.yaml);
      if (p) pista = p[1].trim();
      const novos = tiposDeSessao(o.yaml);
      if (novos.size) tipos = novos;
      continue;
    }
    if (o.kind !== "frame") continue;

    const tele = o.tele;
    // Sem `session_num` (gravador antigo) tudo cai na sessão 0 — uma sessão só, que é o
    // que essas capturas de fato têm.
    const num = tele.session_num ?? 0;
    if (!mundos.has(num)) mundos.set(num, novoMundo(num));
    const m = mundos.get(num);
    if (tele.car_left_right !== undefined) temCanalLateral = true;

    m.quadrosTotais += 1;
    if (m.primeiro === null) m.primeiro = tele.session_time;
    m.ultimo = tele.session_time;

    // O canal lateral é de topo e vem em TODO quadro, não só nos que têm `cars[]`. Medi-lo
    // no fluxo de 20 Hz jogaria fora dois terços das transições de um canal que o próprio
    // fonte descreve como nervoso — e a pergunta aqui é justamente quantas ele tem.
    m.nBruto += 1;
    m.lrBruto.push(tele.car_left_right ?? 0);
    m.estadoBruto.push(tele.session_state);
    m.lrNaPista.push(tele.on_track && !tele.is_replay_playing && !tele.player_on_pit_road ? 1 : 0);

    // Quadro sem `cars[]` não é falha: o gravador só grava o array a 20 Hz, e só em
    // corrida. É o fluxo de 20 Hz que o detector enxerga aqui.
    if (!Array.isArray(tele.cars) || tele.cars.length === 0) continue;

    m.comprimentoM = comprimentoM;
    const q = m.n;
    m.n += 1;
    m.t.push(tele.session_time);
    m.estado.push(tele.session_state);
    m.comprimento.push(m.comprimentoM);
    m.bandeiras.push(tele.session_flags ?? 0);
    m.replay.push(tele.is_replay_playing ? 1 : 0);

    const presentes = new Set();
    for (const c of tele.cars) {
      const i = c.idx;
      if (i < 0 || i >= MAX) continue;
      presentes.add(i);
      if (i > m.maxIdx) m.maxIdx = i;
      const alvo = m.carros[i];
      alvo.presente[q] = 1;
      alvo.pct[q] = c.lap_dist_pct;
      alvo.sup[q] = c.track_surface;
      alvo.mat[q] = c.track_surface_material ?? -1;
      alvo.pit[q] = c.on_pit_road ? 1 : 0;
      alvo.pos[q] = c.position;
      alvo.est[q] = c.est_time ?? 0;
    }
    for (let i = 0; i < MAX; i += 1) {
      if (presentes.has(i)) continue;
      const alvo = m.carros[i];
      alvo.presente[q] = 0;
      alvo.pct[q] = NaN;
      alvo.sup[q] = -1;
      alvo.mat[q] = -1;
      alvo.pit[q] = 0;
      alvo.pos[q] = 0;
      alvo.est[q] = 0;
    }

    // O jogador de verdade da gravação. `lap_dist_pct` da telemetria é o mesmo recurso
    // que o Rust usa quando o jogador não está em `cars[]` — e ele some de lá.
    const ij = tele.player_car_idx ?? 0;
    m.jogIdx.push(ij);
    m.jogPct.push(presentes.has(ij) ? m.carros[ij].pct[q] : tele.lap_dist_pct);
    m.jogVelMs.push(tele.speed_ms ?? 0);
    m.jogPos.push(tele.position ?? 0);
    m.jogNoCarro.push(tele.on_track && !tele.is_replay_playing ? 1 : 0);
    m.jogNaPista.push(tele.on_track && !tele.is_replay_playing && !tele.player_on_pit_road ? 1 : 0);
  }

  const lista = [...mundos.values()].sort((a, b) => a.sessaoNum - b.sessaoNum);
  for (const m of lista) {
    m.duracaoS = m.primeiro === null ? 0 : m.ultimo - m.primeiro;
    m.pista = pista;
    m.versao = versao;
    m.temCanalLateral = temCanalLateral;
    m.tipoSessao = tipos.get(m.sessaoNum) ?? null;
    m.ehCorrida = ehCorrida(m.tipoSessao);
  }
  return lista;
}

/// Velocidade derivada sobre a janela. `NaN` até haver histórico bastante — e `NaN` NÃO é
/// zero: um carro sem histórico não pode ser confundido com um carro parado, que é o erro
/// que transformaria toda entrada de carro no mundo em obstáculo.
function atualizarVelocidade(e, tempo, pct, comprimentoM, K) {
  e.hist.push([tempo, pct]);
  while (e.hist.length && tempo - e.hist[0][0] > K.JANELA_VEL_S * 2) e.hist.shift();
  const base = e.hist.find(([t]) => tempo - t >= K.JANELA_VEL_S);
  // A posição de 0,25 s atrás. Só o detector `lento` usa (é ela que dá o trecho da volta),
  // mas sai daqui porque é a MESMA amostra que produz a velocidade — casar as duas em
  // pontos diferentes é o viés que o espelho não pode introduzir sozinho.
  e.basePct = base ? base[1] : NaN;
  if (!base) {
    e.vel = NaN;
    return;
  }
  const dt = tempo - base[0];
  if (dt <= 0) {
    e.vel = NaN;
    return;
  }
  let d = pct - base[1];
  if (d < -0.5) d += 1; // cruzou a linha dentro da janela
  if (d > 0.5) d -= 1;
  e.vel = ((d * comprimentoM) / dt) * 3.6;
}

/// Baldes rotativos de 1 s: o pico dos últimos 10 s sai de um `max` sobre 10 números.
function atualizarPico(e, tempo, K) {
  const v = Math.max(Number.isNaN(e.vel) ? 0 : e.vel, 0);
  const passo = K.PICO_JANELA_S / K.PICO_BALDES;
  if (e.baldeAte === -Infinity) e.baldeAte = tempo + passo;
  while (tempo >= e.baldeAte) {
    e.balde = (e.balde + 1) % K.PICO_BALDES;
    e.baldes[e.balde] = 0;
    e.baldeAte += passo;
  }
  if (v > e.baldes[e.balde]) e.baldes[e.balde] = v;
}

function picoDe(e) {
  let p = 0;
  for (let i = 0; i < e.baldes.length; i += 1) if (e.baldes[i] > p) p = e.baldes[i];
  return p;
}

/// Os quadros em que o tempo saltou. Cada detector calcula o seu, com a SUA constante:
/// hoje os três usam 5 s, e o dia em que um deles mudar é justamente o dia em que copiar
/// o número do vizinho daria um espelho errado sem avisar.
function saltosDe(m, K) {
  const salto = new Uint8Array(m.n);
  let ultimo = 0;
  for (let q = 0; q < m.n; q += 1) {
    const t = m.t[q];
    salto[q] = t < ultimo || t - ultimo > K.SALTO_MAX_S ? 1 : 0;
    ultimo = t;
  }
  return salto;
}

/// Derivação do detector `frente`: velocidade por Δpct, pico em baldes, última vez visto.
function prepararFrente(m, K) {
  const salto = saltosDe(m, K);
  const carros = Array.from({ length: K.MAX_CARROS }, () => ({ vel: [], pico: [], visto: [] }));
  const est = Array.from({ length: K.MAX_CARROS }, () => ({
    hist: [], baldes: new Float64Array(K.PICO_BALDES), balde: 0, baldeAte: -Infinity,
    vel: NaN, visto: NaN,
  }));
  const zerar = () => {
    for (const e of est) {
      e.hist.length = 0; e.baldes.fill(0); e.balde = 0; e.baldeAte = -Infinity;
      e.vel = NaN; e.visto = NaN;
    }
  };
  for (let q = 0; q < m.n; q += 1) {
    if (salto[q]) zerar();
    const t = m.t[q];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const c = m.carros[i];
      const e = est[i];
      if (!salto[q] && c.presente[q]) {
        e.visto = t;
        if (c.sup[q] === K.SUP_FORA_DO_MUNDO || c.pct[q] < 0) {
          e.hist.length = 0;
          e.vel = NaN;
        } else {
          atualizarVelocidade(e, t, c.pct[q], m.comprimento[q], K);
          atualizarPico(e, t, K);
        }
      }
      carros[i].vel[q] = e.vel;
      carros[i].pico[q] = picoDe(e);
      carros[i].visto[q] = e.visto;
    }
  }
  return { salto, carros };
}

/// Derivação do detector `lento`: o mesmo da velocidade, mais o mapa de ritmo por trecho
/// da volta e a razão bruta de cada carro contra esse mapa.
///
/// O fator do campo NÃO entra aqui: ele exclui o jogador, e o jogador muda a cada
/// simulação. O que entra é o material para calculá-lo em O(1) por jogador — a lista
/// ordenada das razões do quadro e o posto de cada carro nela.
function prepararLento(m, K) {
  const salto = saltosDe(m, K);
  const carros = Array.from({ length: K.MAX_CARROS }, () => ({
    vel: [], pico: [], visto: [], razao: [], trecho: [],
  }));
  const ordenados = new Array(m.n);
  const est = Array.from({ length: K.MAX_CARROS }, () => ({
    hist: [], baldes: new Float64Array(K.PICO_BALDES), balde: 0, baldeAte: -Infinity,
    vel: NaN, visto: NaN, basePct: NaN, trecho: -1, razao: NaN,
  }));
  const novoTrecho = () => ({ amostras: new Float64Array(K.AMOSTRAS_TRECHO), n: 0, cursor: 0 });
  let mapa = Array.from({ length: K.TRECHOS }, novoTrecho);
  const zerar = () => {
    for (const e of est) {
      e.hist.length = 0; e.baldes.fill(0); e.balde = 0; e.baldeAte = -Infinity;
      e.vel = NaN; e.visto = NaN; e.basePct = NaN; e.trecho = -1; e.razao = NaN;
    }
    // O mapa vai junto: um salto é replay, rebobinada ou pista nova, e mapa de ritmo de
    // outra pista é pior que nenhum.
    mapa = Array.from({ length: K.TRECHOS }, novoTrecho);
  };
  const referencia = (t) => {
    if (t.n < K.MIN_AMOSTRAS_TRECHO) return NaN;
    const v = Array.from(t.amostras.subarray(0, t.n)).sort((a, b) => a - b);
    const med = v.length % 2 ? v[(v.length - 1) / 2] : (v[v.length / 2 - 1] + v[v.length / 2]) / 2;
    return med > 0 ? med : NaN;
  };

  for (let q = 0; q < m.n; q += 1) {
    if (salto[q]) zerar();
    const t = m.t[q];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const c = m.carros[i];
      const e = est[i];
      if (!salto[q] && c.presente[q]) {
        e.visto = t;
        if (c.sup[q] === K.SUP_FORA_DO_MUNDO || c.pct[q] < 0) {
          e.hist.length = 0;
          e.vel = NaN;
          e.razao = NaN;
        } else {
          atualizarVelocidade(e, t, c.pct[q], m.comprimento[q], K);
          atualizarPico(e, t, K);
          // O trecho vem da BASE da janela, não do `pct` de agora: a velocidade derivada
          // descreve o pedaço de pista que o carro acabou de percorrer.
          const trecho = Number.isNaN(e.basePct)
            ? -1
            : Math.floor(((e.basePct % 1) + 1) % 1 * K.TRECHOS) % K.TRECHOS;
          const valido = c.sup[q] === K.SUP_NA_PISTA && !c.pit[q];
          if (valido && trecho >= 0 && !Number.isNaN(e.vel)) {
            if (e.trecho !== trecho) {
              e.trecho = trecho;
              // Uma amostra por TRAVESSIA, nunca por quadro: um carro parado dentro de um
              // trecho entregaria 20 amostras por segundo de velocidade zero e afogaria a
              // própria referência que o denunciaria.
              const tr = mapa[trecho];
              tr.amostras[tr.cursor] = e.vel;
              tr.cursor = (tr.cursor + 1) % K.AMOSTRAS_TRECHO;
              tr.n = Math.min(tr.n + 1, K.AMOSTRAS_TRECHO);
            }
            const r = referencia(mapa[trecho]);
            e.razao = Number.isNaN(r) ? NaN : e.vel / r;
          } else {
            e.trecho = trecho;
            e.razao = NaN;
          }
        }
      }
      carros[i].vel[q] = e.vel;
      carros[i].pico[q] = picoDe(e);
      carros[i].visto[q] = e.visto;
      carros[i].razao[q] = e.razao ?? NaN;
      carros[i].trecho[q] = e.trecho;
    }
    // A lista ordenada do quadro. `posto` é onde cada carro caiu nela — é o que permite
    // tirar o jogador da mediana sem reordenar nada.
    const pares = [];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (!m.carros[i].presente[q]) continue;
      const r = carros[i].razao[q];
      if (!Number.isNaN(r)) pares.push([r, i]);
    }
    pares.sort((a, b) => a[0] - b[0]);
    const valores = Float64Array.from(pares.map((p) => p[0]));
    const posto = new Int8Array(K.MAX_CARROS).fill(-1);
    pares.forEach(([, i], k) => {
      posto[i] = k;
    });
    ordenados[q] = { valores, posto };
  }
  return { salto, carros, ordenados };
}

/// A razão do carro contra a mediana do campo no quadro `q` — o mesmo quociente que o
/// detector `lento` usa para abrir. `Infinity` quando não há o que comparar: o carro sumiu,
/// entrou no box, ou o campo não tem amostra. Infinito nunca é "ainda lento", que é a
/// direção conservadora — na dúvida o aviso conta como inútil, não como acerto.
function razaoDeCampo(m, d, K, q, idx) {
  const c = m.carros[idx];
  if (!c.presente[q] || c.pit[q]) return Infinity;
  const bruta = d.carros[idx].razao[q];
  if (Number.isNaN(bruta)) return Infinity;
  // Sem excluir ninguém: aqui não há jogador, há uma pergunta sobre o mundo.
  const fator = medianaSem(d.ordenados[q], -1, K.MIN_CAMPO);
  if (Number.isNaN(fator) || fator <= 0) return Infinity;
  return bruta / fator;
}

/// A mediana da lista ordenada, tirando o carro `fora` (posto `-1` = não está nela).
/// `NaN` quando sobra menos que `minimo` — que é o "sem campo, sem comparação" do Rust.
function medianaSem(ord, fora, minimo) {
  const v = ord.valores;
  const k = fora >= 0 ? ord.posto[fora] : -1;
  const n = v.length - (k >= 0 ? 1 : 0);
  if (n < minimo) return NaN;
  const em = (j) => (k >= 0 && j >= k ? v[j + 1] : v[j]);
  return n % 2 ? em((n - 1) / 2) : (em(n / 2 - 1) + em(n / 2)) / 2;
}

/// Derivação do detector `tras`: os dois acumuladores por carro (metros andados e tempo de
/// referência consumido) e a mediana do campo.
///
/// A mediana do campo é do MUNDO, não do jogador: o Rust não exclui ninguém dela, então
/// ela é a mesma para os 41 jogadores simulados e sai calculada uma vez só.
function prepararTras(m, K) {
  const salto = saltosDe(m, K);
  const carros = Array.from({ length: K.MAX_CARROS }, () => ({ ms: [], ritmo: [], visto: [] }));
  const campoRitmo = new Float64Array(m.n);
  const campoMs = new Float64Array(m.n);
  const est = Array.from({ length: K.MAX_CARROS }, () => ({
    hist: [], pctAnt: NaN, estAnt: NaN, metros: 0, referencia: 0, visto: NaN,
  }));
  const zerar = () => {
    for (const e of est) {
      e.hist.length = 0; e.pctAnt = NaN; e.estAnt = NaN;
      e.metros = 0; e.referencia = 0; e.visto = NaN;
    }
  };
  for (let q = 0; q < m.n; q += 1) {
    if (salto[q]) zerar();
    const t = m.t[q];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const c = m.carros[i];
      const e = est[i];
      if (!salto[q] && c.presente[q]) {
        e.visto = t;
        if (!(c.pct[q] < 0)) {
          if (!Number.isNaN(e.pctAnt)) {
            let d = c.pct[q] - e.pctAnt;
            if (d < -0.5) d += 1;
            if (d > 0.5) d -= 1;
            e.metros += d * m.comprimento[q];
          }
          e.pctAnt = c.pct[q];
          if (c.est[q] > 0) {
            if (!Number.isNaN(e.estAnt)) {
              const d = c.est[q] - e.estAnt;
              // O `est_time` volta a zero na linha; a amostra da virada é descartada em
              // vez de adivinhada.
              if (d >= 0 && d <= 5) e.referencia += d;
            }
            e.estAnt = c.est[q];
          }
          e.hist.push([t, e.metros, e.referencia]);
          while (e.hist.length && t - e.hist[0][0] > K.JANELA_RITMO_S * 1.5) e.hist.shift();
        }
      }
      // A amostra MAIS NOVA que já cobre a janela, do fim para o começo — pela outra ponta
      // a janela efetiva seria a do buffer inteiro, e a folga viraria atraso de reação.
      let base = null;
      for (let k = e.hist.length - 1; k >= 0; k -= 1) {
        if (t - e.hist[k][0] >= K.JANELA_RITMO_S) {
          base = e.hist[k];
          break;
        }
      }
      const fim = e.hist.length ? e.hist[e.hist.length - 1] : null;
      let ms = NaN;
      let ritmo = NaN;
      if (base && fim) {
        const dt = fim[0] - base[0];
        if (dt > 0) {
          ms = (fim[1] - base[1]) / dt;
          ritmo = (fim[2] - base[2]) / dt;
        }
      }
      carros[i].ms[q] = ms;
      carros[i].ritmo[q] = ritmo;
      carros[i].visto[q] = e.visto;
    }
    // Mediana do campo: só quem está na pista e fora do box.
    const rs = [];
    const vs = [];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const c = m.carros[i];
      if (!c.presente[q] || c.sup[q] !== K.SUP_NA_PISTA || c.pit[q]) continue;
      if (Number.isNaN(carros[i].ritmo[q])) continue;
      rs.push(carros[i].ritmo[q]);
      vs.push(carros[i].ms[q]);
    }
    if (rs.length < K.CAMPO_MIN_CARROS) {
      campoRitmo[q] = 0;
      campoMs[q] = 0;
    } else {
      rs.sort((a, b) => a - b);
      vs.sort((a, b) => a - b);
      // O Rust usa `v[len / 2]` aqui, sem a média do par central. Espelhado como está.
      campoRitmo[q] = rs[Math.floor(rs.length / 2)];
      campoMs[q] = vs[Math.floor(vs.length / 2)];
    }
  }
  return { salto, carros, campoRitmo, campoMs };
}

// ═══════════════════ Passe 2: o espelho de `ObservadorFrente` ═══════════════════

/// Distância PARA A FRENTE, de `de` até `para`, em metros (0..comprimento).
function adiante(dePct, paraPct, comprimentoM) {
  let d = paraPct - dePct;
  if (d < 0) d += 1;
  return d * comprimentoM;
}

/// Distância COM SINAL: positiva à frente, negativa atrás, dentro de meia volta. É a forma
/// certa de guardar um gap que pode inverter — e ele inverte, que é o caso "o jogador
/// passou".
function comSinal(dePct, paraPct, comprimentoM) {
  let d = paraPct - dePct;
  if (d > 0.5) d -= 1;
  if (d < -0.5) d += 1;
  return d * comprimentoM;
}

/// O espelho de `spotter_frente.rs`. Uma instância por jogador simulado.
///
/// A diferença de responsabilidade em relação ao Rust: aqui o passo de aviso NÃO decide
/// quem fala. Ele registra TODO episódio elegível em TODO quadro em que a janela se abre,
/// e a arbitragem — o mais próximo, um por episódio — é feita depois, em `disparar()`.
/// É o que torna a varredura de parâmetros barata: um piso de permanência atrasa o
/// disparo, e sem os candidatos dos quadros seguintes não haveria como saber para quando.
class EspelhoFrente {
  constructor(K) {
    this.K = K;
    this.carros = Array.from({ length: K.MAX_CARROS }, () => ({
      episodio: null,
      aguardandoNormalizar: false,
    }));
    this.proximoId = 1;
    this.encerrados = [];
    this.candidatos = [];
    /// Episódios abertos por quadro, para a taxa-base: [ [carIdx, ...], ... ]
    this.abertosPorQuadro = [];
  }

  zerar() {
    for (const c of this.carros) {
      c.episodio = null;
      c.aguardandoNormalizar = false;
    }
  }

  /// Um quadro. `jog` traz o ponto de vista do jogador simulado.
  observar(m, d, q, jog) {
    const K = this.K;
    if (d.salto[q]) {
      this.zerar();
      this.abertosPorQuadro.push(null);
      return;
    }
    const comprimento = m.comprimento[q];
    if (comprimento <= 0) {
      this.abertosPorQuadro.push(null);
      return;
    }
    const emCorrida = m.estado[q] === K.ESTADO_CORRIDA;

    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (i === jog.idx) continue;
      if (!m.carros[i].presente[q]) continue;
      this.passoEpisodio(m, d, q, i, jog, emCorrida, comprimento);
    }

    // Quem sumiu do array. O guincho do iRacing não avisa — o carro simplesmente deixa de
    // aparecer em `cars[]`. Sem esta varredura um episódio aberto num carro guinchado
    // nunca fecharia, e a duração registrada passaria a contar o tempo de ausência: 150 s
    // em vez de 4,7 para o `#25` de Okayama.
    this.fecharAusentes(m, d, q);

    this.registrarCandidatos(m, d, q, jog, emCorrida, comprimento);

    let abertos = null;
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (!this.carros[i].episodio) continue;
      (abertos ??= []).push(i);
    }
    this.abertosPorQuadro.push(abertos);
  }

  passoEpisodio(m, d, q, i, jog, emCorrida, comprimento) {
    const K = this.K;
    const carro = m.carros[i];
    const sup = carro.sup[q];
    const pit = carro.pit[q] === 1;
    const vel = d.carros[i].vel[q];
    const pico = d.carros[i].pico[q];
    const gap = comSinal(jog.pct, carro.pct[q], comprimento);
    const estado = this.carros[i];

    if (estado.episodio) {
      const ep = estado.episodio;
      ep.duracaoS = m.t[q] - ep.inicioS;
      ep.gapFimM = gap;
      ep.posicaoFim = carro.pos[q];
      ep.posicaoJogadorFim = jog.pos;
      if (!Number.isNaN(vel) && vel < ep.minimaKmh) ep.minimaKmh = vel;
      if (!ep.materiais.includes(carro.mat[q])) ep.materiais.push(carro.mat[q]);

      // A velocidade NÃO entra no encerramento, de propósito: a perda de ritmo é o filtro
      // de ENTRADA, o que separa a escapada do corte de grama. Depois de aberto, um carro
      // que recupera ritmo ainda enterrado na grama é o mesmo obstáculo.
      let desfecho = null;
      if (!emCorrida) desfecho = DESFECHO.sessao;
      else if (sup === K.SUP_FORA_DO_MUNDO) desfecho = DESFECHO.sumiu;
      else if (pit || sup === K.SUP_NA_CAIXA || sup === K.SUP_ENTRANDO_BOX) desfecho = DESFECHO.box;
      else if (gap < 0 && ep.gapInicioM > 0) desfecho = DESFECHO.ultrapassado;
      else if (ep.tipo === "fora" && sup === K.SUP_NA_PISTA) desfecho = DESFECHO.retomou;
      else if (ep.tipo === "parado" && !Number.isNaN(vel) && vel >= K.PARADO_KMH)
        desfecho = DESFECHO.retomou;

      if (desfecho) {
        ep.desfecho = desfecho;
        ep.fimS = m.t[q];
        this.encerrados.push(ep);
        estado.episodio = null;
        // Fechou, mas o carro pode continuar exatamente na mesma situação: `ultrapassado`
        // é encerramento do ponto de vista do JOGADOR. Sem a trava, um episódio novo
        // nasceria 50 ms depois, e em 10 s de escapada seriam centenas de idênticos.
        estado.aguardandoNormalizar = true;
      }
      return;
    }

    // Abertura. Só em corrida, e só para quem ESTAVA ANDANDO — é essa regra, e não o
    // `SessionState`, que mata a largada parada.
    if (!emCorrida || pico < K.PICO_MIN_KMH) {
      estado.aguardandoNormalizar = false;
      return;
    }
    if (Number.isNaN(vel)) return;

    let tipo = null;
    if (sup === K.SUP_FORA_DA_PISTA && vel < K.FRACAO_RITMO * pico) tipo = "fora";
    else if (sup === K.SUP_NA_PISTA && !pit && vel < K.PARADO_KMH) tipo = "parado";
    else {
      estado.aguardandoNormalizar = false; // condição normal: a trava cai aqui
      return;
    }
    if (estado.aguardandoNormalizar) return;

    const [pIdx, pM, pS] = this.perseguidor(m, d, q, i, comprimento);
    estado.episodio = {
      id: this.proximoId++,
      carIdx: i,
      tipo,
      inicioS: m.t[q],
      fimS: m.t[q],
      duracaoS: 0,
      picoKmh: pico,
      minimaKmh: vel,
      superficie: sup,
      materiais: [carro.mat[q]],
      perseguidorIdx: pIdx,
      perseguidorM: pM,
      perseguidorS: pS,
      gapInicioM: gap,
      gapFimM: gap,
      posicaoInicio: carro.pos[q],
      posicaoFim: carro.pos[q],
      posicaoJogadorInicio: jog.pos,
      posicaoJogadorFim: jog.pos,
      desfecho: null,
    };
  }

  /// A duração é contada até a última vez em que o carro foi VISTO, não até agora. É a
  /// diferença entre "ficou 4 s parado e foi guinchado" e "ficou 150 s parado".
  fecharAusentes(m, d, q) {
    const agora = m.t[q];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const visto = d.carros[i].visto[q];
      if (Number.isNaN(visto)) continue;
      if (agora - visto < this.K.AUSENCIA_MAX_S) continue;
      const estado = this.carros[i];
      estado.aguardandoNormalizar = false;
      if (!estado.episodio) continue;
      const ep = estado.episodio;
      ep.duracaoS = visto - ep.inicioS;
      ep.fimS = visto;
      ep.desfecho = DESFECHO.sumiu;
      this.encerrados.push(ep);
      estado.episodio = null;
    }
  }

  /// Quem vem atrás do obstáculo, mais próximo, andando. É medido ENTRE CARROS e não a
  /// partir do jogador: numa captura com o jogador no box, tudo medido a partir dele
  /// seria zero.
  perseguidor(m, d, q, obst, comprimento) {
    const pctObst = m.carros[obst].pct[q];
    let melhor = null;
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (i === obst || !m.carros[i].presente[q]) continue;
      const c = m.carros[i];
      if (c.sup[q] !== this.K.SUP_NA_PISTA || c.pit[q]) continue;
      const v = d.carros[i].vel[q];
      if (Number.isNaN(v) || v < 30) continue;
      const dist = adiante(c.pct[q], pctObst, comprimento);
      if (dist <= 0 || dist > comprimento / 2) continue;
      if (!melhor || dist < melhor[1]) melhor = [i, dist, dist / (v / 3.6)];
    }
    return melhor ?? [null, null, null];
  }

  /// Todo episódio que, NESTE quadro, está dentro da janela de aviso. A escolha de quem
  /// fala é de `disparar()`.
  registrarCandidatos(m, d, q, jog, emCorrida, comprimento){
    const K = this.K;
    if (!jog.naPista || !emCorrida || jog.velMs <= 1) return;
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (!m.carros[i].presente[q]) continue;
      const ep = this.carros[i].episodio;
      if (!ep) continue;
      const dist = adiante(jog.pct, m.carros[i].pct[q], comprimento);
      if (dist > K.DIST_MAX_M) continue;
      const tta = dist / jog.velMs;
      if (tta < K.TTA_MIN_S || tta > K.TTA_MAX_S) continue;
      this.candidatos.push({
        quadro: q,
        t: m.t[q],
        epId: ep.id,
        tipo: ep.tipo,
        carIdx: i,
        dist,
        tta,
        duracaoEp: m.t[q] - ep.inicioS,
        velObst: d.carros[i].vel[q],
      });
    }
  }

  /// Os que a captura terminou sem fechar. Contam como episódio, com o que já duraram.
  fechar(m) {
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const ep = this.carros[i].episodio;
      if (!ep) continue;
      ep.desfecho = DESFECHO.aberto;
      this.encerrados.push(ep);
      this.carros[i].episodio = null;
    }
    this.encerrados.sort((a, b) => a.inicioS - b.inicioS);
    return this.encerrados;
  }
}

// ═══════════════════ O espelho de `spotter_lento.rs` ═══════════════════

/// Carro LENTO à frente. O irmão do `frente`, com duas diferenças que mudam tudo: a
/// referência é o ritmo do CAMPO naquele ponto da pista (mapa por trecho), e o tempo até
/// chegar é de FECHAMENTO, não `distância / velocidade do jogador`.
///
/// Como no `frente`, a arbitragem do aviso sai daqui e vai para `disparar()`. A diferença
/// é que aqui as duas chaves — lento e muito lento — disputam o MESMO tique: o grau sai da
/// razão no instante do aviso, não de um episódio de outra natureza. Separá-las na
/// arbitragem daria a cada grau um aviso por tique, que é o dobro do que o detector faz.
class EspelhoLento {
  constructor(K) {
    this.K = K;
    this.carros = Array.from({ length: K.MAX_CARROS }, () => ({
      episodio: null,
      aguardandoNormalizar: false,
      lentoDesde: NaN,
    }));
    this.proximoId = 1;
    this.encerrados = [];
    this.candidatos = [];
    this.abertosPorQuadro = [];
    this.fator = NaN;
  }

  zerar() {
    for (const c of this.carros) {
      c.episodio = null;
      c.aguardandoNormalizar = false;
      c.lentoDesde = NaN;
    }
    this.fator = NaN;
  }

  /// A razão final: quanto do ritmo do campo o carro está fazendo naquele ponto da pista.
  razao(d, q, i) {
    const bruta = d.carros[i].razao[q];
    if (Number.isNaN(bruta) || Number.isNaN(this.fator) || this.fator <= 0) return NaN;
    return bruta / this.fator;
  }

  observar(m, d, q, jog) {
    const K = this.K;
    if (d.salto[q]) {
      this.zerar();
      this.abertosPorQuadro.push(null);
      return;
    }
    const comprimento = m.comprimento[q];
    if (comprimento <= 0) {
      this.abertosPorQuadro.push(null);
      return;
    }
    const emCorrida = m.estado[q] === K.ESTADO_CORRIDA;

    // O fator do campo — a mediana das razões brutas AGORA, SEM o jogador. Tirar o carro
    // que se sabe suspeito é de graça: em Okayama o jogador rodou a 22 km/h contra um
    // campo de 133 por quase quatro minutos, e num grid pequeno um outlier desses move a
    // mediana. O pré-cálculo entrega a lista ordenada; aqui só se tira um elemento dela.
    this.fator = medianaSem(d.ordenados[q], jog.idx, K.MIN_CAMPO);

    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (i === jog.idx || !m.carros[i].presente[q]) continue;
      this.passoEpisodio(m, d, q, i, jog, emCorrida, comprimento);
    }
    this.fecharAusentes(m, d, q);
    this.registrarCandidatos(m, d, q, jog, emCorrida, comprimento);

    let abertos = null;
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (!this.carros[i].episodio) continue;
      (abertos ??= []).push(i);
    }
    this.abertosPorQuadro.push(abertos);
  }

  passoEpisodio(m, d, q, i, jog, emCorrida, comprimento) {
    const K = this.K;
    const carro = m.carros[i];
    const sup = carro.sup[q];
    const pit = carro.pit[q] === 1;
    const vel = d.carros[i].vel[q];
    const razao = this.razao(d, q, i);
    const gap = comSinal(jog.pct, carro.pct[q], comprimento);
    const naPista = sup === K.SUP_NA_PISTA && !pit;
    const estado = this.carros[i];

    if (estado.episodio) {
      const ep = estado.episodio;
      ep.duracaoS = m.t[q] - ep.inicioS;
      ep.gapFimM = gap;
      ep.posicaoFim = carro.pos[q];
      if (!Number.isNaN(vel) && vel < ep.minimaKmh) ep.minimaKmh = vel;
      if (!Number.isNaN(razao) && razao < ep.razaoMinima) ep.razaoMinima = razao;

      // As três famílias se excluem AQUI também, e não só na abertura: um carro lento que
      // para vira notícia da família `parado`, e um que sai da pista, da família `fora`.
      let desfecho = null;
      if (!emCorrida) desfecho = DESFECHO.sessao;
      else if (sup === K.SUP_FORA_DO_MUNDO) desfecho = DESFECHO.sumiu;
      else if (pit || sup === K.SUP_NA_CAIXA || sup === K.SUP_ENTRANDO_BOX) desfecho = DESFECHO.box;
      else if (sup === K.SUP_FORA_DA_PISTA) desfecho = DESFECHO.saiu;
      else if (!Number.isNaN(vel) && vel < K.PARADO_KMH) desfecho = DESFECHO.parou;
      else if (gap < 0 && ep.gapInicioM > 0) desfecho = DESFECHO.ultrapassado;
      else if (!Number.isNaN(razao) && razao >= K.CORTE_SAIDA) desfecho = DESFECHO.retomou;

      if (desfecho) {
        ep.desfecho = desfecho;
        ep.fimS = m.t[q];
        this.encerrados.push(ep);
        estado.episodio = null;
        estado.lentoDesde = NaN;
        estado.aguardandoNormalizar = true;
      }
      return;
    }

    const lento =
      emCorrida &&
      naPista &&
      d.carros[i].pico[q] >= K.PICO_MIN_KMH &&
      !Number.isNaN(vel) &&
      vel >= K.PARADO_KMH &&
      !Number.isNaN(razao) &&
      razao < K.CORTE;
    if (!lento) {
      estado.lentoDesde = NaN;
      estado.aguardandoNormalizar = false;
      return;
    }
    if (estado.aguardandoNormalizar) return;
    // A permanência: o carro precisa ter ficado abaixo do corte por PERMANENCIA_S
    // SEGUIDOS. Uma freada forte numa curva lenta derruba a razão por um instante.
    if (Number.isNaN(estado.lentoDesde)) estado.lentoDesde = m.t[q];
    const desde = estado.lentoDesde;
    if (m.t[q] - desde < K.PERMANENCIA_S) return;

    const referencia = d.carros[i].trecho[q];
    estado.episodio = {
      id: this.proximoId++,
      carIdx: i,
      tipo: "lento",
      // O início é quando ele cruzou o corte, não quando o filtro deixou passar.
      inicioS: desde,
      fimS: m.t[q],
      duracaoS: m.t[q] - desde,
      razaoInicio: razao,
      razaoMinima: razao,
      trecho: referencia,
      minimaKmh: vel,
      picoKmh: d.carros[i].pico[q],
      gapInicioM: gap,
      gapFimM: gap,
      posicaoInicio: carro.pos[q],
      posicaoFim: carro.pos[q],
      desfecho: null,
    };
  }

  fecharAusentes(m, d, q) {
    const agora = m.t[q];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const visto = d.carros[i].visto[q];
      if (Number.isNaN(visto)) continue;
      if (agora - visto < this.K.AUSENCIA_MAX_S) continue;
      const estado = this.carros[i];
      estado.aguardandoNormalizar = false;
      estado.lentoDesde = NaN;
      if (!estado.episodio) continue;
      const ep = estado.episodio;
      ep.duracaoS = visto - ep.inicioS;
      ep.fimS = visto;
      ep.desfecho = DESFECHO.sumiu;
      this.encerrados.push(ep);
      estado.episodio = null;
    }
  }

  registrarCandidatos(m, d, q, jog, emCorrida, comprimento) {
    const K = this.K;
    if (!jog.naPista || !emCorrida || jog.velMs <= 1) return;
    const rp = this.razao(d, q, jog.idx);
    if (Number.isNaN(rp) || rp <= 0) return;
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (!m.carros[i].presente[q]) continue;
      const ep = this.carros[i].episodio;
      if (!ep) continue;
      const dist = adiante(jog.pct, m.carros[i].pct[q], comprimento);
      if (dist < K.DIST_MIN_M || dist > K.DIST_MAX_M) continue;
      const rc = this.razao(d, q, i);
      if (Number.isNaN(rc) || rc >= rp) continue;
      // O vão se fecha à DIFERENÇA de ritmo. Comparar velocidades cruas daria fechamento
      // enorme com o jogador na reta e o alvo na curva, sem nada acontecendo.
      const fechamento = jog.velMs * (1 - rc / rp);
      if (fechamento <= 0) continue;
      const tta = dist / fechamento;
      if (tta < K.TTA_MIN_S || tta > K.TTA_MAX_S) continue;
      this.candidatos.push({
        quadro: q,
        t: m.t[q],
        epId: ep.id,
        tipo: rc < K.CORTE_GRAVE ? "muito_lento" : "lento",
        carIdx: i,
        dist,
        tta,
        duracaoEp: m.t[q] - ep.inicioS,
        velObst: d.carros[i].vel[q],
        razao: rc,
      });
    }
  }

  fechar(m) {
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const ep = this.carros[i].episodio;
      if (!ep) continue;
      ep.desfecho = DESFECHO.aberto;
      this.encerrados.push(ep);
      this.carros[i].episodio = null;
    }
    this.encerrados.sort((a, b) => a.inicioS - b.inicioS);
    return this.encerrados;
  }
}

// ═══════════════════ O espelho de `spotter_tras.rs` ═══════════════════

/// Carro chegando por TRÁS. Não é aviso pontual: é ESTADO SUSTENTADO, com entrada,
/// lembretes e liberação. A conta de "avisos por piloto" não descreve isto — o que
/// descreve é quantas vezes o estado entra, quanto dura e quantas falas saem.
class EspelhoTras {
  constructor(K) {
    this.K = K;
    this.ativo = null;
    this.lentoDesde = NaN;
    this.azulDesde = NaN;
    this.viaDeBoxEm = NaN;
    this.ultimoPerseguidor = NaN;
    this.pendente = null;
    this.encerrados = [];
    this.falas = [];
  }

  zerar() {
    this.ativo = null;
    this.lentoDesde = NaN;
    this.azulDesde = NaN;
    this.viaDeBoxEm = NaN;
    this.ultimoPerseguidor = NaN;
    this.pendente = null;
  }

  naPista(m, q, i) {
    const c = m.carros[i];
    return (c.sup[q] === this.K.SUP_NA_PISTA || c.sup[q] === this.K.SUP_FORA_DA_PISTA) && !c.pit[q];
  }

  naViaDeBox(m, q, i) {
    const c = m.carros[i];
    return !!c.pit[q] || c.sup[q] === this.K.SUP_NA_CAIXA || c.sup[q] === this.K.SUP_ENTRANDO_BOX;
  }

  /// Registro de um tique para a trilha (`--trilha`). No-op quando ela está desligada, que é
  /// sempre exceto no diagnóstico — o laço roda 41 vezes sobre 16 mil quadros.
  marcar(t, falta, extra) {
    if (!this.trilha) return;
    this.trilha.push({ t, falta, ...extra });
  }

  observar(m, d, q, jog) {
    const K = this.K;
    if (d.salto[q]) {
      this.zerar();
      this.marcar(m.t[q], "salto de tempo");
      return null;
    }
    if (m.comprimento[q] <= 0) return null;
    const t = m.t[q];

    // O jogador sumiu de `cars[]`? É assim que o guincho se manifesta.
    const visto = d.carros[jog.idx].visto[q];
    if (!Number.isNaN(visto) && t - visto >= K.AUSENCIA_MAX_S) {
      this.encerrar("sumiu do array", visto);
    }
    if (!m.carros[jog.idx].presente[q]) {
      this.marcar(t, "fora de cars[]");
      return this.pendente;
    }

    if (this.naViaDeBox(m, q, jog.idx)) this.viaDeBoxEm = t;

    const amarela = (jog.bandeiras & K.BANDEIRAS_AMARELA) !== 0;
    const campoEmRitmo = d.campoRitmo[q] > 0.05 && d.campoMs[q] >= K.CAMPO_MIN_MS;

    let fim = null;
    if (m.estado[q] !== K.ESTADO_CORRIDA) fim = "sessão acabou";
    else if (amarela) fim = "amarela";
    else if (!jog.noCarro || !this.naPista(m, q, jog.idx)) fim = "saiu da pista";
    if (fim) {
      this.encerrar(fim, t);
      this.lentoDesde = NaN;
      this.azulDesde = NaN;
      this.ultimoPerseguidor = NaN;
      this.marcar(t, `fecha: ${fim}`, { sup: m.carros[jog.idx].sup[q] });
      return this.pendente;
    }
    if (!campoEmRitmo) {
      // Sem mediana não há comparação. O estado aberto continua: quem o fecha é o tráfego
      // ter passado, não a mediana ter oscilado.
      this.lentoDesde = NaN;
      this.marcar(t, "campo sem ritmo", { campoMs: d.campoMs[q], campoRitmo: d.campoRitmo[q] });
      return this.pendente;
    }

    const jogMs = d.carros[jog.idx].ms[q];
    const jogRitmo = d.carros[jog.idx].ritmo[q];
    if (Number.isNaN(jogMs) || Number.isNaN(jogRitmo)) {
      this.marcar(t, "ritmo do jogador indefinido");
      return this.pendente;
    }
    const fracao = jogRitmo / d.campoRitmo[q];
    if (fracao < K.FRACAO_LENTO) {
      if (Number.isNaN(this.lentoDesde)) this.lentoDesde = t;
    } else if (fracao >= K.FRACAO_RETOMOU) {
      this.lentoDesde = NaN;
    }
    if ((jog.bandeiras & K.BANDEIRA_AZUL) !== 0) {
      if (Number.isNaN(this.azulDesde)) this.azulDesde = t;
    } else {
      this.azulDesde = NaN;
    }

    const p = this.perseguidor(m, d, q, jog, jogMs);
    if (p) this.ultimoPerseguidor = t;
    // O retrato do tique, para a trilha. `atras` só é contado quando ela está ligada.
    const foto = {
      fracao,
      lentoHa: Number.isNaN(this.lentoDesde) ? NaN : t - this.lentoDesde,
      jogMs,
      campoMs: d.campoMs[q],
      p,
      atras: this.trilha ? this.atrasDe(m, d, q, jog, jogMs) : null,
    };

    if (!this.ativo) {
      const sustentado = (desde, espera) => !Number.isNaN(desde) && t - desde >= espera;
      const temRitmo = sustentado(this.lentoDesde, K.CONFIRMA_S);
      const temAzul = sustentado(this.azulDesde, K.CONFIRMA_AZUL_S);
      // A ordem do relato é a do Rust, mas o motivo relatado é o que de fato falta: quando
      // faltam os dois, dizer só "sem perseguidor" esconderia metade da resposta.
      if (!p || !(temRitmo || temAzul)) {
        const faltas = [];
        if (!temRitmo && !temAzul) {
          faltas.push(
            Number.isNaN(this.lentoDesde)
              ? `fração ${nf(fracao, 2)} ≥ FRACAO_LENTO`
              : `lento há ${nf(foto.lentoHa)}s de ${nf(K.CONFIRMA_S)}s`,
          );
        }
        if (!p) faltas.push("sem perseguidor na janela");
        this.marcar(t, faltas.join(" + "), foto);
        return this.pendente;
      }
      const origem = temRitmo ? "ritmo" : "azul";
      // A saída do box é o único falso positivo com volume nos dados, e é o piloto que
      // menos precisa ser informado de que está devagar.
      if (!Number.isNaN(this.viaDeBoxEm) && t - this.viaDeBoxEm < K.SAIDA_BOX_S) {
        this.marcar(t, `saída de box há ${nf(t - this.viaDeBoxEm)}s`, foto);
        return this.pendente;
      }
      this.marcar(t, `ABRE (${origem})`, foto);
      const vistos = new Set([p.idx]);
      this.ativo = {
        ep: {
          id: this.encerrados.length + 1,
          origem,
          inicioS: t,
          duracaoS: 0,
          fracaoEntrada: fracao,
          perseguidorIdx: p.idx,
          perseguidorM: p.distanciaM,
          perseguidorS: p.chegaEmS,
          carrosNaJanela: 1,
          falas: 0,
          fim: null,
        },
        entradaAnunciada: false,
        ultimoAnuncio: t,
        intervalo: K.LEMBRETE_S,
        vistos,
      };
      this.pendente = "entrada";
      return this.pendente;
    }

    const a = this.ativo;
    this.marcar(t, "ABERTO", foto);
    a.ep.duracaoS = t - a.ep.inicioS;
    if (p && !a.vistos.has(p.idx)) {
      a.vistos.add(p.idx);
      a.ep.carrosNaJanela += 1;
    }
    const vazio = !Number.isNaN(this.ultimoPerseguidor) && t - this.ultimoPerseguidor >= K.LIBERA_S;
    if (vazio) {
      this.encerrar("trem passou", t);
    } else if (!a.entradaAnunciada) {
      this.pendente = "entrada";
    } else if (p && t - a.ultimoAnuncio >= a.intervalo) {
      this.pendente = "lembrete";
    }
    return this.pendente;
  }

  /// A fala saiu de verdade. Só aqui a entrada para de insistir e o lembrete reagenda.
  confirmarAviso(t) {
    const pendente = this.pendente;
    this.pendente = null;
    if (!pendente) return;
    this.falas.push({ t, chave: pendente });
    const a = this.ativo;
    if (!a) return; // era a liberação: o estado já fechou
    if (pendente === "entrada") {
      a.entradaAnunciada = true;
      a.ultimoAnuncio = t;
      a.ep.falas += 1;
    } else if (pendente === "lembrete") {
      a.ultimoAnuncio = t;
      a.intervalo = Math.min(a.intervalo + this.K.LEMBRETE_PASSO_S, this.K.LEMBRETE_MAX_S);
      a.ep.falas += 1;
    }
  }

  /// Quem está chegando por trás, mais cedo.
  perseguidor(m, d, q, jog, jogMs) {
    const K = this.K;
    const comprimento = m.comprimento[q];
    let melhor = null;
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (i === jog.idx || !m.carros[i].presente[q]) continue;
      if (!this.naPista(m, q, i)) continue;
      const ms = d.carros[i].ms[q];
      if (Number.isNaN(ms)) continue;
      const fechamento = ms - jogMs;
      if (fechamento < K.FECHAMENTO_MIN_MS) continue;
      const distanciaM = adiante(m.carros[i].pct[q], jog.pct, comprimento);
      if (distanciaM <= 0 || distanciaM > K.DIST_MAX_M) continue;
      const chegaEmS = distanciaM / fechamento;
      if (chegaEmS > K.TTA_MAX_S) continue;
      if (!melhor || chegaEmS < melhor.chegaEmS) melhor = { idx: i, distanciaM, chegaEmS };
    }
    return melhor;
  }

  /// Quem está atrás na janela de distância, e por que cada um não conta como perseguidor.
  /// Só a trilha usa isto — é a diferença entre "não abriu" e "não abriu POR ISTO".
  atrasDe(m, d, q, jog, jogMs) {
    const K = this.K;
    const r = { naJanela: 0, lentos: 0, longe: 0, semTempo: 0, maisProximoM: Infinity };
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (i === jog.idx || !m.carros[i].presente[q]) continue;
      if (!this.naPista(m, q, i)) continue;
      const ms = d.carros[i].ms[q];
      if (Number.isNaN(ms)) continue;
      const distanciaM = adiante(m.carros[i].pct[q], jog.pct, m.comprimento[q]);
      if (distanciaM <= 0) continue;
      if (distanciaM < r.maisProximoM) r.maisProximoM = distanciaM;
      if (distanciaM > K.DIST_MAX_M) {
        r.longe += 1;
        continue;
      }
      r.naJanela += 1;
      const fechamento = ms - jogMs;
      if (fechamento < K.FECHAMENTO_MIN_MS) r.lentos += 1;
      else if (distanciaM / fechamento > K.TTA_MAX_S) r.semTempo += 1;
    }
    return r;
  }

  encerrar(fim, ateS) {
    const a = this.ativo;
    if (!a) return;
    this.ativo = null;
    a.ep.duracaoS = ateS - a.ep.inicioS;
    a.ep.fimS = ateS;
    a.ep.fim = fim;
    this.encerrados.push(a.ep);
    // Só o trem ter passado é notícia, e só se a chegada chegou a ser anunciada: "livre
    // atrás" depois de uma bandeirada ou de um guincho responde a pergunta que ninguém fez.
    this.pendente = fim === "trem passou" && a.entradaAnunciada ? "liberacao" : null;
  }

  fecharCaptura(m) {
    if (this.ativo) {
      const a = this.ativo;
      this.ativo = null;
      a.ep.fim = DESFECHO.aberto;
      a.ep.fimS = m.t[m.n - 1];
      this.encerrados.push(a.ep);
    }
    return this.encerrados;
  }
}

/// Roda um jogador simulado sobre o mundo inteiro.
///
/// `idx` negativo é o jogador REAL da gravação (o que a telemetria traz). Qualquer outro é
/// um carro da IA promovido a jogador — sem isso a via do rádio nunca é exercitada, porque
/// nas duas capturas o jogador real passa a corrida no box.
function simular(m, d, K, detector, idx) {
  const esp = detector.criar(K);
  if (TRILHA && TRILHA.detector === detector.nome && TRILHA.carro === idx) esp.trilha = [];
  const jog = { idx: 0, pct: 0, velMs: 0, naPista: false, noCarro: false, pos: 0, bandeiras: 0, real: idx < 0 };
  let ultimoPct = 0;
  for (let q = 0; q < m.n; q += 1) {
    if (idx < 0) {
      jog.idx = m.jogIdx[q];
      jog.pct = m.jogPct[q];
      jog.velMs = m.jogVelMs[q];
      jog.naPista = m.jogNaPista[q] === 1;
      jog.noCarro = m.jogNoCarro[q] === 1;
      jog.pos = m.jogPos[q];
      jog.bandeiras = m.bandeiras[q];
    } else {
      const c = m.carros[idx];
      const cd = d.carros[idx];
      jog.idx = idx;
      // Ausente de `cars[]`: guinchado ou na garagem. Mantém o último ponto conhecido e
      // sai da pista — um jogador que não está no mundo não recebe rádio.
      if (c.presente[q]) ultimoPct = c.pct[q];
      jog.pct = ultimoPct;
      // A velocidade do piloto simulado sai da derivação do próprio detector: `vel` em
      // km/h no `frente` e no `lento`, `ms` em m/s no `tras`. Um detector não empresta a
      // derivação do outro nem para isto.
      const bruta = cd.vel ? cd.vel[q] / 3.6 : cd.ms[q];
      jog.velMs = Number.isNaN(bruta) ? 0 : bruta;
      jog.naPista = c.presente[q] === 1 && c.sup[q] === K.SUP_NA_PISTA && !c.pit[q];
      // No Rust isto é `on_track && !is_replay_playing`, e as duas metades são do canal do
      // HUMANO: `is_replay_playing` diz que o cliente dele está tocando replay, não que um
      // carro da IA parou de correr. Nas duas capturas ela é verdadeira em ~95% dos
      // quadros, porque o humano assistiu à prova — aplicá-la ao piloto simulado calava a
      // família inteira. Mesmo raciocínio da bandeira azul, logo abaixo: o que se sabe do
      // carro da IA é que ele está em `cars[]`.
      jog.noCarro = c.presente[q] === 1;
      jog.pos = c.pos[q];
      // A AZUL é do jogador de verdade e não pode ser simulada carro a carro: ela vive no
      // `session_flags` global, que é o canal do humano. A amarela é do mundo e vale para
      // todos. Dar a azul do humano a um piloto simulado seria inventar sinal.
      jog.bandeiras = AZUL_PARA_TODOS ? m.bandeiras[q] : m.bandeiras[q] & ~(K.BANDEIRA_AZUL ?? 0);
    }
    const chave = esp.observar(m, d, q, jog);
    // O arreio assume que a fala sempre sai: quem arbitra prioridade entre spotters é a
    // camada de voz, que não existe aqui. Sem confirmar, a entrada insistiria para sempre.
    if (chave && esp.confirmarAviso) esp.confirmarAviso(m.t[q]);
  }
  if (esp.fechar) esp.fechar(m);
  if (esp.fecharCaptura) esp.fecharCaptura(m);
  return esp;
}

// ═════════════════ A arbitragem do aviso, e se ele foi útil ═════════════════

/// Quem fala, dado um piso de permanência e um teto de velocidade do obstáculo.
///
/// Reproduz a arbitragem do Rust: um aviso por episódio, o mais próximo primeiro, e o
/// episódio que não falou continua pendente para o quadro seguinte. **Nunca descarta, no
/// máximo adia** — um aviso que some para preservar cadência é o defeito que este projeto
/// já cometeu uma vez.
/// `ignorarCarro` sai da disputa por completo, e não só da contagem: um obstáculo
/// descartado não pode roubar o quadro de outro.
function disparar(candidatos, pool, piso, teto, ignorarCarro = -1) {
  const avisados = new Set();
  const avisos = [];
  let quadroAtual = -1;
  let melhor = null;
  const fecharQuadro = () => {
    if (!melhor) return;
    avisados.add(melhor.epId);
    avisos.push(melhor);
    melhor = null;
  };
  for (const c of candidatos) {
    if (!pool.includes(c.tipo)) continue;
    if (c.carIdx === ignorarCarro) continue;
    if (c.quadro !== quadroAtual) {
      fecharQuadro();
      quadroAtual = c.quadro;
    }
    if (avisados.has(c.epId)) continue;
    if (c.duracaoEp < piso) continue;
    // Velocidade desconhecida passa: o teto corta o que foi MEDIDO acima dele, não o que
    // não se sabe. (`NaN > teto` já é falso; explícito para quem lê.)
    if (!Number.isNaN(c.velObst) && c.velObst > teto) continue;
    if (!melhor || c.dist < melhor.dist) melhor = c;
  }
  fecharQuadro();
  return avisos;
}

/// Quando o piloto chegou ao ponto, ainda havia um problema ali?
///
/// Esta é a métrica que vale, e ela NÃO é "o episódio ainda estava aberto". Um episódio
/// fecha quando o carro volta à pista — e um carro voltando da grama, lento, na
/// trajetória, é exatamente o perigo que o aviso descreveu. Medir fechamento de registro
/// em vez de utilidade já custou uma recomendação errada a este projeto.
///
/// Guinchado (ausente do array) e no box contam como NÃO-problema: o carro saiu do
/// caminho.
function aindaProblema(m, d, K, quadro, idx) {
  const c = m.carros[idx];
  if (!c.presente[quadro]) return false;
  if (c.pit[quadro]) return false;
  if (c.sup[quadro] === K.SUP_FORA_DA_PISTA) return true;
  if (c.sup[quadro] === K.SUP_NA_PISTA) {
    const v = d.carros[idx].vel[quadro];
    const p = d.carros[idx].pico[quadro];
    if (Number.isNaN(v) || p <= 0) return false;
    return v < FRACAO_UTIL * p;
  }
  return false;
}

/// O quadro em que o piloto chega ao ponto do obstáculo. `null` quando a captura acaba
/// antes — esses avisos ficam de fora da conta em vez de virarem "úteis" por omissão.
function quadroDaChegada(m, q0, tta) {
  const alvo = m.t[q0] + tta;
  for (let q = q0; q < m.n; q += 1) if (m.t[q] >= alvo) return q;
  return null;
}

function avaliar(m, d, K, avisos) {
  let uteis = 0;
  let inuteis = 0;
  let semJulgamento = 0;
  for (const a of avisos) {
    const q = quadroDaChegada(m, a.quadro, a.tta);
    if (q === null) {
      a.veredito = "sem chegada gravada";
      semJulgamento += 1;
      continue;
    }
    a.veredito = aindaProblema(m, d, K, q, a.carIdx) ? "útil" : "inútil";
    if (a.veredito === "útil") uteis += 1;
    else inuteis += 1;
  }
  return { total: avisos.length, uteis, inuteis, semJulgamento };
}

// ═══════════════════════════ Taxa-base ═══════════════════════════
//
// Percentual do tempo-carro EM PISTA com obstáculo à frente dentro de cada janela. É o que
// diz se o aviso é informação ou ruído: um que toca uma ou duas vezes por corrida é
// informação; um que toca toda volta o piloto aprende a ignorar.

function taxaBase(m, K, espelho) {
  const denom = { total: 0 };
  const hits = new Map(JANELAS_BASE_M.map((j) => [j, 0]));
  for (let q = 0; q < m.n; q += 1) {
    if (m.estado[q] !== K.ESTADO_CORRIDA) continue;
    const abertos = espelho.abertosPorQuadro[q];
    const comprimento = m.comprimento[q];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const c = m.carros[i];
      if (!c.presente[q] || c.sup[q] !== K.SUP_NA_PISTA || c.pit[q]) continue;
      denom.total += 1;
      if (!abertos) continue;
      let maisProximo = Infinity;
      for (const o of abertos) {
        if (o === i) continue;
        const d = adiante(c.pct[q], m.carros[o].pct[q], comprimento);
        if (d < maisProximo) maisProximo = d;
      }
      for (const j of JANELAS_BASE_M) if (maisProximo <= j) hits.set(j, hits.get(j) + 1);
    }
  }
  return { denom: denom.total, hits };
}

// ═══════════════════════════ Relatório ═══════════════════════════

const nf = (x, casas = 1) =>
  x === null || x === undefined || Number.isNaN(x) ? "—" : x.toFixed(casas).replace(".", ",");
const pct = (parte, total, casas = 0) => (total ? `${((100 * parte) / total).toFixed(casas).replace(".", ",")}%` : "—");

/// Uma captura pode conter o fim de semana inteiro. Cada `session_num` vira um bloco, e só
/// os de corrida entram nas somas — quem decide isso é o `SessionType` do YAML lido pelo
/// índice da sessão, nunca o primeiro que aparecer no arquivo.
function analisarCaptura(arquivo, detectores) {
  const linha = "═".repeat(78);
  const nome = path.basename(arquivo);
  const mb = (fs.statSync(arquivo).size / 1048576).toFixed(1);
  console.log(`\n${linha}\n${nome}  (${mb} MB)`);

  // O arquivo é lido UMA vez e serve todas as sessões e os três detectores: a leitura é
  // crua, e o que cada detector deriva dela é dele.
  const mundos = lerMundos(arquivo);
  if (mundos.length > 1) {
    console.log(
      `  ${mundos.length} sessões no arquivo: ` +
        mundos.map((m) => `#${m.sessaoNum} ${m.tipoSessao || "tipo não registrado"}`).join(", ") +
        ` — analisadas em separado`,
    );
  }
  const sessoes = [];
  for (const m of mundos) {
    sessoes.push({
      sessaoNum: m.sessaoNum,
      tipoSessao: m.tipoSessao,
      ehCorrida: m.ehCorrida,
      ...analisarSessao(m, detectores, mundos.length > 1),
    });
  }
  return { sessoes };
}

function analisarSessao(m, detectores, rotularSessao) {
  if (rotularSessao) {
    console.log(
      `\n  ▸ sessão #${m.sessaoNum} — ${m.tipoSessao || "tipo não registrado"}` +
        (m.ehCorrida ? "" : "  (NÃO é corrida: fora de toda soma e da verificação)"),
    );
  }
  if (m.n === 0) {
    const erro = "sem quadros com cars[] — sessão sem corrida gravada";
    console.log(`  ${erro}. Nada a reproduzir.`);
    return { erro };
  }
  console.log(
    `  ${m.pista || "pista não registrada"} · ${nf(m.comprimentoM, 0)} m · ${m.tipoSessao || "?"} · ` +
      `${nf(m.duracaoS, 0)} s`,
  );
  console.log(
    `  ${m.quadrosTotais} quadros, ${m.n} com cars[] (${nf(m.n / (m.duracaoS || 1))} Hz) · ` +
      `${m.maxIdx + 1} carros · jogador real #${m.jogIdx[0]}`,
  );
  if (m.estado.every((e) => e !== 4)) {
    const erro = `nenhum quadro em verde (${nf(m.duracaoS, 0)} s) — sessão truncada`;
    console.log(`  ⚠ ${erro}. Seguindo.`);
    return { erro };
  }

  const resultados = {};
  for (const det of detectores) {
    console.log(`\n  ── detector \`${det.nome}\` ${"─".repeat(60 - det.nome.length)}`);
    // Os limiares que valeram nesta rodada, lidos do fonte. Sem isto o relatório seria um
    // número sem procedência: daqui a três meses ninguém sabe se aqueles 35% saíram de
    // `FRACAO_RITMO` a 0,60 ou a 0,70.
    console.log(
      `  do fonte: ` +
        Object.entries(det.K)
          .filter(([c]) => !/^(SUP_|ESTADO_|MAX_CARROS|PICO_BALDES|BANDEIRA)/.test(c))
          .map(([c, v]) => `${c}=${nf(v, 2)}`)
          .join("  "),
    );
    const d = det.preparar(m, det.K);
    const r = det.forma === "sustentado" ? rodarSustentado(m, d, det) : rodarDetector(m, d, det);
    resultados[det.nome] = r;
  }

  const Kfrente = detectores.find((x) => x.nome === "frente")?.K;
  const sobre = sobreposicao(m, resultados.lento, resultados.frente, Kfrente);
  if (sobre && sobre.total) {
    console.log(`\n  ── sobreposição \`lento\` × \`frente\` ${"─".repeat(41)}`);
    console.log(
      `  ${sobre.simultaneos} de ${sobre.total} avisos de "lento" (${pct(sobre.simultaneos, sobre.total)}) são sobre um carro\n` +
        `  que o detector \`frente\` já pegaria NO MESMO INSTANTE (episódio aberto de fora/parado).`,
    );
    console.log(
      `  ${sobre.recentes} de ${sobre.total} (${pct(sobre.recentes, sobre.total)}) são de carros que estiveram fora da pista\n` +
        `  ou abaixo de 5 km/h nos 10 s anteriores — a leitura de passado, que é outra pergunta.`,
    );
    for (const [chave, v] of sobre.porFamilia) {
      if (!v.total) continue;
      console.log(`      ${chave.padEnd(12)} ${v.simultaneos}/${v.total} simultâneos (${pct(v.simultaneos, v.total)})`);
    }
    // Sob QUE bandeira o aviso saiu. O próprio fonte do `lento` diz que 84% dos avisos
    // vêm da sanfona da amarela; sem separar, o número da família é a média de dois
    // regimes que não se parecem. A máscara vem do fonte da frente A, que é onde ela mora.
    const mascara = detectores.find((x) => x.nome === "tras")?.K?.BANDEIRAS_AMARELA;
    if (mascara) {
      let sobAmarela = 0;
      let total = 0;
      for (const chave of ["lento", "muito_lento"]) {
        for (const a of resultados.lento.porFamilia.get(chave)?.avisos ?? []) {
          total += 1;
          if (m.bandeiras[a.quadro] & mascara) sobAmarela += 1;
        }
      }
      console.log(
        `  ${sobAmarela} de ${total} (${pct(sobAmarela, total)}) saíram SOB AMARELA ` +
          `(máscara de \`spotter_tras.rs\`)`,
      );
    }
  }

  const carga = cargaSomada(m, resultados);
  const lateral = imprimirLateral(m, detectores.find((x) => x.nome === "frente")?.K, resultados, carga);
  if (LINHA_DO_TEMPO) despejarLinhaDoTempo(m, carga, lateral, LINHA_DO_TEMPO);
  if (carga) {
    imprimirCarga(carga, `${carga.pilotos.length} pilotos da IA, esta corrida`);
    if (carga.doReal) {
      console.log(
        `    · o jogador real #${carga.jogadorReal} fica fora da conta (só é simulado em \`tras\`); ` +
          `ele ouviria ${carga.doReal} fala(s)`,
      );
    }
  }

  // A verificação é escrita contra o detector `frente`. Sem ele não há o que conferir.
  if (!resultados.frente) return { erro: "detector `frente` não reproduzível para esta captura" };
  return { ...resultados.frente, outros: resultados, sobreposicao: sobre, carga, lateral };
}

function rodarDetector(m, d, det) {
  const K = det.K;

  // 1. Os episódios, do ponto de vista do jogador REAL — é a reprodução do que o detector
  //    teria registrado durante a gravação.
  const real = simular(m, d, K, det, -1);
  const episodios = real.encerrados;
  console.log(`  EPISÓDIOS: ${episodios.length}`);
  for (const e of episodios) {
    const cauda = e.materiais
      ? `[${e.materiais.map((x) => MATERIAL[String(x)] ?? `?${x}`).join(", ")}]  ` +
        `perseguidor=${e.perseguidorIdx === null ? "—" : `#${e.perseguidorIdx} a ${nf(e.perseguidorM, 0)} m / ${nf(e.perseguidorS)} s`}`
      : `razão=${nf(e.razaoInicio, 2)}→${nf(e.razaoMinima, 2)}  trecho=${e.trecho}`;
    console.log(
      `    ${e.tipo.padEnd(6)} #${String(e.carIdx).padStart(2)}  t=${nf(e.inicioS)}s  ` +
        `dur=${nf(e.duracaoS, 2)}s  pico=${nf(e.picoKmh, 0)}  min=${nf(e.minimaKmh, 0)} km/h  ` +
        `${cauda}  → ${e.desfecho}`,
    );
  }

  // 2. Cada carro da IA como jogador.
  //
  // O carro do jogador real não vira piloto simulado — ele passa a corrida no box, e um
  // piloto que não anda não exercita via nenhuma. Como OBSTÁCULO ele continua contando por
  // padrão, que é o que o detector faria; ver [`IGNORAR_JOGADOR_REAL`], porque é
  // exatamente aí que mora a única divergência contra os números publicados.
  const jogadorReal = m.jogIdx[0];
  const ignorar = IGNORAR_JOGADOR_REAL ? jogadorReal : -1;
  const pilotos = [];
  for (let i = 0; i <= m.maxIdx; i += 1) {
    if (i === jogadorReal) continue;
    // Só quem de fato correu: um índice que nunca apareceu não é piloto.
    let apareceu = false;
    for (let q = 0; q < m.n && !apareceu; q += 1) if (m.carros[i].presente[q]) apareceu = true;
    if (apareceu) pilotos.push(i);
  }
  const sims = pilotos.map((i) => simular(m, d, K, det, i));

  console.log(`\n  AVISOS (${pilotos.length} carros da IA simulados como jogador)`);
  const porFamilia = new Map();
  for (const pool of det.pools) {
    // O pool inteiro disputa o mesmo tique; a classificação por família vem DEPOIS, do
    // tipo de cada aviso que sobreviveu à arbitragem.
    const avisos = sims.flatMap((s, k) => {
      const meus = disparar(s.candidatos, pool, 0, Infinity, ignorar);
      for (const a of meus) a.piloto = pilotos[k];
      return meus;
    });
    // A outra leitura da mesma corrida. Calculada sempre: um número que muda conforme uma
    // bandeira e não aparece no relatório é como voltar ao script descartável.
    const outra = sims.flatMap((s) =>
      disparar(s.candidatos, pool, 0, Infinity, ignorar < 0 ? jogadorReal : -1),
    );
    avaliar(m, d, K, avisos);
    avaliar(m, d, K, outra);

    for (const chave of pool) {
      const f = det.familias.find((x) => x.chave === chave);
      const meus = avisos.filter((a) => a.tipo === chave);
      const ouvintes = new Map();
      for (const a of meus) ouvintes.set(a.piloto, (ouvintes.get(a.piloto) || 0) + 1);
      const v = contar(meus);
      const vOutra = contar(outra.filter((a) => a.tipo === chave));
      porFamilia.set(`${chave}:alt`, vOutra);
      if (v.total !== vOutra.total) {
        console.log(
          `    · "${f.rotulo}": ${Math.abs(v.total - vOutra.total)} aviso(s) são sobre o carro do ` +
            `jogador real #${jogadorReal} — ${vOutra.total} ${ignorar < 0 ? "sem ele" : "com ele"} (--sem-jogador-real)`,
        );
      }
      porFamilia.set(chave, { avisos: meus, ...v, ouvintes });
      console.log(
        `    ${f.rotulo.padEnd(14)} ${String(v.total).padStart(3)} avisos · ` +
          `${v.uteis} úteis / ${v.inuteis} inúteis (${pct(v.inuteis, v.uteis + v.inuteis)})` +
          (v.semJulgamento ? ` · ${v.semJulgamento} sem chegada gravada` : "") +
          ` · ${nf(v.total / (pilotos.length || 1), 2)} por piloto · ` +
          `${[...ouvintes.values()].filter((n) => n > 1).length} ouviram mais de uma vez`,
      );
      if (meus.length) {
        const dd = meus.map((a) => a.dist).sort((a, b) => a - b);
        const ss = meus.map((a) => a.tta).sort((a, b) => a - b);
        console.log(
          `      faixa: ${nf(dd[0], 0)}–${nf(dd[dd.length - 1], 0)} m, ` +
            `${nf(ss[0])}–${nf(ss[ss.length - 1])} s`,
        );
        for (const u of det.utilidades ?? []) {
          let uteis = 0;
          let julgados = 0;
          // Quantos a métrica alternativa SALVA: o CONTRATO disse inútil e ela diz útil. É o
          // número que decide se a divergência contra a frente B é escolha de métrica.
          let salvos = 0;
          for (const a of meus) {
            const qc = quadroDaChegada(m, a.quadro, a.tta);
            if (qc === null) continue;
            julgados += 1;
            const ok = u.ainda(m, d, K, qc, a.carIdx);
            if (ok) uteis += 1;
            if (ok && a.veredito === "inútil") salvos += 1;
          }
          console.log(
            `      alternativa "${u.rotulo}": ${julgados - uteis} inúteis (${pct(julgados - uteis, julgados)})` +
              ` · ${salvos} que o CONTRATO reprovou e ela aprova`,
          );
        }
      }
      if (DETALHAR_AVISOS) {
        for (const a of [...meus].sort((x, y) => x.t - y.t)) {
          console.log(
            `        t=${nf(a.t)}s  piloto #${String(a.piloto).padStart(2)} → alvo #${String(a.carIdx).padStart(2)}  ` +
              `${nf(a.dist, 0)} m / ${nf(a.tta)} s  ep=${a.epId} aberto há ${nf(a.duracaoEp)}s  ` +
              `alvo a ${nf(a.velObst, 0)} km/h  → ${a.veredito}`,
          );
        }
      }
    }
  }

  // 3. A varredura. Foi ela que mostrou que o piso de permanência NÃO ajuda na família
  //    "fora da pista" — e que contrariou a recomendação anterior.
  console.log(`\n  VARREDURA piso de permanência × teto de velocidade  (avisos / inúteis)`);
  for (const pool of det.pools) {
    const rotulo = pool.map((c) => det.familias.find((f) => f.chave === c).rotulo).join(" + ");
    // Piso e teto só tiram avisos. Sem nenhum na configuração mais permissiva, a tabela
    // inteira é zero — e uma tela de zeros esconde as que têm número.
    if (pool.every((c) => porFamilia.get(c).total === 0)) {
      console.log(`    ${rotulo}: nenhum aviso na configuração aberta — nada a varrer`);
      continue;
    }
    // O piso da varredura é tempo desde o INÍCIO do episódio, e no `lento` o início é o
    // cruzamento do corte — o detector já exige `PERMANENCIA_S` antes de abrir. As linhas
    // abaixo desse valor são, por construção, iguais à primeira: a varredura daqui não
    // substitui variar a constante no fonte.
    if (K.PERMANENCIA_S > 0) {
      console.log(`    (piso de varredura ≤ PERMANENCIA_S=${nf(K.PERMANENCIA_S, 1)} s não muda nada — o detector já o aplica)`);
    }
    console.log(
      `    ${rotulo}` +
        `\n      piso\\teto  ` +
        VARREDURA_TETO_KMH.map((t) => (t === Infinity ? "sem teto" : `${t} km/h`).padStart(12)).join(""),
    );
    for (const piso of VARREDURA_PISO_S) {
      const celulas = VARREDURA_TETO_KMH.map((teto) => {
        const avisos = sims.flatMap((s) => disparar(s.candidatos, pool, piso, teto, ignorar));
        const v = avaliar(m, d, K, avisos);
        return `${v.total}/${pct(v.inuteis, v.uteis + v.inuteis)}`.padStart(12);
      });
      console.log(`      ${`${nf(piso, 1)} s`.padEnd(11)}${celulas.join("")}`);
    }
  }

  // 4. A taxa-base.
  const tb = taxaBase(m, K, real);
  // Denominador: carro-quadro com a corrida em andamento, o carro presente em `cars[]`, na
  // pista e fora do pit road. Está escrito aqui porque é a metade da conta que ninguém
  // registra e que faz o mesmo evento render 0,10% ou 0,13%.
  console.log(
    `\n  TAXA-BASE (obstáculo à frente por tempo-carro em pista — ${tb.denom} amostras carro-quadro,\n` +
      `             em corrida, carro presente, na pista, fora do box)`,
  );
  console.log(
    `    ` + JANELAS_BASE_M.map((j) => `${j} m: ${pct(tb.hits.get(j), tb.denom, 3)}`).join("   "),
  );

  return { mundo: m, derivado: d, real, episodios, porFamilia, pilotos, sims, taxaBase: tb };
}

/// Reconta um punhado de avisos já avaliados. `avaliar` julga; isto só soma.
function contar(avisos) {
  let uteis = 0;
  let inuteis = 0;
  let semJulgamento = 0;
  for (const a of avisos) {
    if (a.veredito === "útil") uteis += 1;
    else if (a.veredito === "inútil") inuteis += 1;
    else semJulgamento += 1;
  }
  return { total: avisos.length, uteis, inuteis, semJulgamento };
}

// ═══════════════ O relatório da forma SUSTENTADA (o detector `tras`) ═══════════════
//
// "Avisos por piloto" não descreve um estado que entra uma vez e dura minutos. O que
// descreve é: quantas vezes entrou, quanto durou, e quantas falas saíram no total. A
// métrica de utilidade do CONTRATO — "quando o piloto chegou ao ponto, ainda havia
// problema ali?" — também não serve, porque não há ponto nenhum aonde chegar: o perigo
// vem por trás, e quem chega é ele. O análogo honesto está em `alcancado`.

/// Alguém de fato alcançou o jogador durante o estado (ou logo depois)?
///
/// É o análogo da métrica de chegada para uma família em que o obstáculo é quem se move. O
/// estado disse "vem gente por trás"; ele foi verdadeiro se alguém chegou perto de verdade.
function alguemAlcancou(m, K, jogIdx, ep, folgaS, limiteM = 10) {
  for (let q = 0; q < m.n; q += 1) {
    if (m.t[q] < ep.inicioS) continue;
    if (m.t[q] > (ep.fimS ?? ep.inicioS + ep.duracaoS) + folgaS) break;
    if (!m.carros[jogIdx].presente[q]) continue;
    const comprimento = m.comprimento[q];
    for (let i = 0; i <= m.maxIdx; i += 1) {
      if (i === jogIdx || !m.carros[i].presente[q]) continue;
      if (m.carros[i].sup[q] !== K.SUP_NA_PISTA || m.carros[i].pit[q]) continue;
      const dm = Math.abs(comSinal(m.carros[jogIdx].pct[q], m.carros[i].pct[q], comprimento));
      if (dm <= limiteM) return true;
    }
  }
  return false;
}

function rodarSustentado(m, d, det) {
  const K = det.K;
  const jogadorReal = m.jogIdx[0];
  const pilotos = [];
  // O jogador real entra como piloto: em Okayama é ELE o acidente que esta família existe
  // para prever, e sem ele o caso não está no acervo.
  if (det.incluiJogadorReal) pilotos.push(-1);
  for (let i = 0; i <= m.maxIdx; i += 1) {
    if (i === jogadorReal) continue;
    let apareceu = false;
    for (let q = 0; q < m.n && !apareceu; q += 1) if (m.carros[i].presente[q]) apareceu = true;
    if (apareceu) pilotos.push(i);
  }
  const sims = pilotos.map((i) => simular(m, d, K, det, i));

  const estados = [];
  sims.forEach((s, k) => {
    const idx = pilotos[k] < 0 ? jogadorReal : pilotos[k];
    for (const ep of s.encerrados) estados.push({ ...ep, piloto: idx, real: pilotos[k] < 0 });
  });
  estados.sort((a, b) => a.inicioS - b.inicioS);

  const falas = sims.flatMap((s, k) => s.falas.map((f) => ({ ...f, piloto: pilotos[k] })));
  const conta = (c) => falas.filter((f) => f.chave === c).length;
  const duracoes = estados.map((e) => e.duracaoS).sort((a, b) => a - b);
  const alcancados = estados.filter((e) => alguemAlcancou(m, K, e.piloto, e, K.LIBERA_S)).length;

  console.log(
    `\n  ESTADOS (${pilotos.length} pilotos simulados` +
      `${det.incluiJogadorReal ? `, incluindo o jogador real #${jogadorReal}` : ""})`,
  );
  console.log(
    `    ${estados.length} entradas · ${nf(estados.length / (pilotos.length || 1), 3)} por piloto · ` +
      `${estados.filter((e) => e.origem === "ritmo").length} por ritmo, ` +
      `${estados.filter((e) => e.origem === "azul").length} por bandeira azul`,
  );
  if (estados.length) {
    console.log(
      `    duração: ${nf(duracoes[0], 1)}–${nf(duracoes[duracoes.length - 1], 1)} s ` +
        `(mediana ${nf(duracoes[Math.floor(duracoes.length / 2)], 1)} s)`,
    );
    console.log(
      `    falas: ${falas.length} no total — ${conta("entrada")} de entrada, ` +
        `${conta("lembrete")} de lembrete, ${conta("liberacao")} de liberação · ` +
        `${nf(falas.length / (pilotos.length || 1), 3)} por piloto`,
    );
    console.log(
      `    alcançado de fato (alguém a ≤10 m durante o estado + ${nf(K.LIBERA_S, 0)} s): ` +
        `${alcancados}/${estados.length} (${pct(estados.length - alcancados, estados.length)} sem ninguém chegando)`,
    );
    for (const e of estados) {
      console.log(
        `      t=${nf(e.inicioS)}s  piloto #${String(e.piloto).padStart(2)}${e.real ? " (jogador real)" : ""}  ` +
          `origem=${e.origem}  dur=${nf(e.duracaoS, 2)}s  fração=${nf(e.fracaoEntrada, 2)}  ` +
          `perseguidor=#${e.perseguidorIdx} a ${nf(e.perseguidorM, 0)} m / ${nf(e.perseguidorS)} s  ` +
          `carros na janela=${e.carrosNaJanela}  falas=${e.falas}  → ${e.fim}`,
      );
    }
  }
  const comTrilha = sims.find((s) => s.trilha);
  if (comTrilha) imprimirTrilha(comTrilha.trilha, det);
  else if (TRILHA && TRILHA.detector === det.nome) {
    console.log(`    ⚠ --trilha: carro #${TRILHA.carro} não foi simulado nesta captura`);
  }
  return { estados, falas, pilotos, sims };
}

/// A trilha de um carro: por que o estado não abriu.
///
/// Duas leituras. A tabulação por motivo cobre a corrida inteira e responde "o que falta,
/// e por quanto tempo". A janela responde "naquele instante específico, o que faltava" —
/// que é a pergunta quando se sabe de um episódio e ele não aparece.
function imprimirTrilha(trilha, det) {
  console.log(`\n    ── trilha \`${det.nome}\` do carro #${TRILHA.carro} ${"─".repeat(34)}`);
  const porMotivo = new Map();
  for (const r of trilha) {
    // A tabulação agrupa pelo TIPO da falta; os segundos que a acompanham variam a cada
    // tique e transformariam a tabela numa lista.
    const chave = r.falta.replace(/[\d,]+s/g, "Ns").replace(/[\d,]+ ≥/, "N ≥");
    const v = porMotivo.get(chave) ?? { n: 0, t0: r.t, t1: r.t };
    v.n += 1;
    v.t1 = r.t;
    porMotivo.set(chave, v);
  }
  console.log(`    ${trilha.length} tiques com o carro no mundo, por motivo:`);
  for (const [chave, v] of [...porMotivo].sort((a, b) => b[1].n - a[1].n)) {
    console.log(`      ${String(v.n).padStart(6)}×  ${chave.padEnd(44)} (até t=${nf(v.t1)}s)`);
  }
  const janela = trilha.filter((r) => r.t >= TRILHA.de && r.t <= TRILHA.ate);
  if (!Number.isFinite(TRILHA.de)) {
    console.log(`    (sem janela: passe --trilha=${det.nome}:${TRILHA.carro}:<t0>-<t1> para o tique a tique)`);
    return;
  }
  console.log(`    tique a tique em ${nf(TRILHA.de, 0)}–${nf(TRILHA.ate, 0)} s, ralado a ~2 Hz:`);
  let ultimo = -Infinity;
  for (const r of janela) {
    if (r.t - ultimo < 0.5) continue;
    ultimo = r.t;
    const atras = r.atras
      ? `atrás: ${r.atras.naJanela} em ${nf(det.K.DIST_MAX_M, 0)}m ` +
        `(${r.atras.lentos} não fecham, ${r.atras.semTempo} fecham devagar), ` +
        `${r.atras.longe} além; mais próximo ${r.atras.maisProximoM === Infinity ? "—" : `${nf(r.atras.maisProximoM, 0)} m`}`
      : "";
    console.log(
      `      t=${nf(r.t, 1).padStart(6)}s  fração=${nf(r.fracao, 2)}  ` +
        `${Number.isNaN(r.lentoHa) ? "lento há —   " : `lento há ${nf(r.lentoHa).padStart(5)}s`}  ` +
        `jog=${nf(r.jogMs * 3.6, 0).padStart(3)} campo=${nf(r.campoMs * 3.6, 0).padStart(3)} km/h  ` +
        `${r.falta}\n        ${atras}`,
    );
  }
}

// ═══════════════════ Carga somada: o que o piloto OUVE ═══════════════════
//
// Cada família mede a si mesma e nenhuma responde a pergunta que decide se o spotter
// informa ou vira ruído: quantas falas o mesmo piloto recebe na mesma corrida, com todas
// ligadas juntas. E a média não responde — ela esconde o piloto que ouve quinze, que é
// justamente quem desliga o spotter. Por isso a distribuição, e o máximo.
//
// Duas ressalvas, ditas aqui porque um número de carga sem elas é otimista:
//
// 1. É um TETO. Não existe camada de voz neste arreio: duas famílias querendo falar no
//    mesmo tique viram duas falas aqui e virariam uma na integração. As colisões são
//    contadas para dizer de quanto é a folga.
// 2. O jogador real fica fora. Ele só é simulado como piloto na família `tras` — nas
//    outras ele passa a corrida no box —, então somá-lo daria uma linha com um terço das
//    famílias medidas. O que ele ouviria aparece à parte.

/// Todas as falas de uma captura, por piloto, separadas por leitura.
function cargaSomada(m, resultados) {
  const rf = resultados.frente;
  if (!rf) return null;
  const jogadorReal = m.jogIdx[0];
  const falas = [];
  const juntar = (r, chaves, familia) => {
    for (const chave of chaves) {
      for (const a of r?.porFamilia?.get(chave)?.avisos ?? []) {
        falas.push({ t: a.t, piloto: a.piloto, familia });
      }
    }
  };
  juntar(rf, ["fora", "parado"], "frente");
  juntar(resultados.lento, ["lento", "muito_lento"], "lento");
  for (const f of resultados.tras?.falas ?? []) {
    falas.push({ t: f.t, piloto: f.piloto < 0 ? jogadorReal : f.piloto, familia: "tras", real: f.piloto < 0 });
  }

  const leituras = [
    { nome: "fora + parado + tras", familias: new Set(["frente", "tras"]) },
    { nome: "… com lento junto", familias: new Set(["frente", "tras", "lento"]) },
  ];
  const saida = [];
  for (const l of leituras) {
    const minhas = falas.filter((f) => l.familias.has(f.familia) && !f.real);
    const porPiloto = new Map(rf.pilotos.map((p) => [p, []]));
    for (const f of minhas) if (porPiloto.has(f.piloto)) porPiloto.get(f.piloto).push(f);
    // Colisão: o mesmo piloto com duas falas no mesmo tique. É a folga que a camada de voz
    // vai recuperar, e sem medi-la a carga aqui parece pior do que será.
    let colisoes = 0;
    for (const lista of porPiloto.values()) {
      const porTique = new Map();
      for (const f of lista) porTique.set(f.t, (porTique.get(f.t) || 0) + 1);
      for (const n of porTique.values()) if (n > 1) colisoes += n - 1;
    }
    saida.push({ nome: l.nome, porPiloto, total: minhas.length, colisoes });
  }
  const doReal = falas.filter((f) => f.real).length;
  return { leituras: saida, doReal, jogadorReal, pilotos: rf.pilotos };
}

const BALDES = [
  { rotulo: "0", dentro: (n) => n === 0 },
  { rotulo: "1–2", dentro: (n) => n >= 1 && n <= 2 },
  { rotulo: "3–5", dentro: (n) => n >= 3 && n <= 5 },
  { rotulo: "mais de 5", dentro: (n) => n > 5 },
];

/// A distribuição de uma leitura sobre pares piloto-corrida.
function distribuicao(contagens) {
  const n = contagens.length;
  const total = contagens.reduce((a, b) => a + b, 0);
  const max = contagens.reduce((a, b) => Math.max(a, b), 0);
  return {
    n,
    total,
    max,
    media: n ? total / n : 0,
    baldes: BALDES.map((b) => ({ rotulo: b.rotulo, n: contagens.filter(b.dentro).length })),
  };
}

function imprimirCarga(carga, titulo, largura = 4) {
  console.log(`\n  ── carga somada ${"─".repeat(largura + 55)}`);
  console.log(`  ${titulo}`);
  for (const l of carga.leituras) {
    const d = distribuicao([...l.porPiloto.values()].map((v) => v.length));
    console.log(
      `    ${l.nome.padEnd(22)} ${String(d.total).padStart(4)} falas · ` +
        `${nf(d.media, 2)} por piloto-corrida · máximo ${d.max}` +
        (l.colisoes ? ` · ${l.colisoes} colisão(ões) de tique` : ""),
    );
    console.log(
      `      ${d.baldes.map((b) => `${b.rotulo}: ${b.n}`).join("   ")}   (de ${d.n} pares piloto-corrida)`,
    );
  }
}

// ═══════════════════ O spotter LATERAL na conta ═══════════════════
//
// `CarLeftRight` é canal de topo da telemetria — do JOGADOR, como `session_flags` e
// `is_replay_playing`. Não existe versão por carro em `cars[]`, então não há como lê-lo
// para os 40 pilotos simulados. E nas duas capturas ele está PARADO em `LR_LIVRE`: o
// humano passou as duas provas no box, e o canal só tem o que dizer com ele na pista.
//
// Daí as duas metades desta seção, e a diferença entre elas importa:
//
//   1. O CRU. Transições de `CarLeftRight` na gravação. É medição direta e é o que o
//      pedido descreve. Nas capturas do acervo o resultado é zero, e o zero é sobre o
//      acervo, não sobre o spotter lateral.
//   2. A RECONSTRUÇÃO. Como o número que decide a tabela de prioridade não pode sair de
//      um canal parado, o arreio reconstrói a vizinhança pela geometria: quantos carros
//      dentro de ±L metros no eixo da pista. A HISTERESE e a cadência saem do fonte
//      (`spotter.rs`), como sempre; a geometria NÃO — ela é do iRacing e não está em
//      lugar nenhum do nosso código. Por isso ela é varrida em vez de fixada, e por isso
//      esta metade é uma reconstrução e não um espelho.
//
// O que a reconstrução não consegue, e conta a favor e contra:
//   · Sem posição lateral, dois carros em fila indiana a 3 m contam como lado a lado. É
//     TETO, e num pelotão sob amarela é teto generoso.
//   · Sem lado, "esquerda → direita" não vira fala e a liberação de um flanco só não se
//     separa. É PISO, na direção contrária.

const LATERAL_FONTE = "src-tauri/src/iracing_sdk/spotter.rs";
const LATERAL_CONSTANTES = [
  "CONFIRMA_LADO_S", "CONFIRMA_LIVRE_S", "LEMBRETE_S", "LEMBRETE_PASSO_S", "LEMBRETE_MAX_S",
];
/// As distâncias longitudinais varridas (m). Não saem de fonte nenhum — são a hipótese.
const LATERAL_JANELAS_M = [3, 5, 7, 10];
/// Janela de colisão entre uma fala lateral e uma das outras famílias (s).
const COLISAO_S = 2.0;

/// Transições do canal cru, e se ele chegou a ser exercitado.
function lateralCru(m) {
  let quadros = 0;
  let naPista = 0;
  let transicoes = 0;
  const valores = new Map();
  let ant = null;
  for (let q = 0; q < m.nBruto; q += 1) {
    if (m.estadoBruto[q] !== 4) continue;
    quadros += 1;
    const v = m.lrBruto[q];
    valores.set(v, (valores.get(v) || 0) + 1);
    if (m.lrNaPista[q]) naPista += 1;
    if (ant !== null && v !== ant) transicoes += 1;
    ant = v;
  }
  return { quadros, naPista, transicoes, valores };
}

/// Quantos carros dentro de ±L m de cada piloto, por tique e por janela.
///
/// Um passe só para todas as janelas: a lista do quadro é ordenada uma vez e cada piloto
/// caminha para os dois lados até a maior janela, contando as menores no caminho.
function vizinhancaGeometrica(m, K, pilotos) {
  const daIdx = new Map(pilotos.map((p, k) => [p, k]));
  const maxL = LATERAL_JANELAS_M[LATERAL_JANELAS_M.length - 1];
  const contagens = LATERAL_JANELAS_M.map(() =>
    pilotos.map(() => new Uint8Array(m.n)),
  );
  const par = [];
  for (let q = 0; q < m.n; q += 1) {
    if (m.estado[q] !== K.ESTADO_CORRIDA) continue;
    const comprimento = m.comprimento[q];
    if (comprimento <= 0) continue;
    par.length = 0;
    for (let i = 0; i <= m.maxIdx; i += 1) {
      const c = m.carros[i];
      if (!c.presente[q] || c.sup[q] !== K.SUP_NA_PISTA || c.pit[q]) continue;
      par.push([c.pct[q], i]);
    }
    par.sort((a, b) => a[0] - b[0]);
    for (let a = 0; a < par.length; a += 1) {
      const k = daIdx.get(par[a][1]);
      if (k === undefined) continue;
      // Para os dois lados da lista ordenada, parando na maior janela. A pista é um
      // anel: o vizinho do primeiro é o último.
      for (const passo of [-1, 1]) {
        for (let d = 1; d < par.length; d += 1) {
          const b = (a + passo * d + par.length * d) % par.length;
          const dm = Math.abs(comSinal(par[a][0], par[b][0], comprimento));
          if (dm > maxL) break;
          for (let j = 0; j < LATERAL_JANELAS_M.length; j += 1) {
            if (dm <= LATERAL_JANELAS_M[j]) contagens[j][k][q] += 1;
          }
        }
      }
    }
  }
  return contagens;
}

/// A máquina de `spotter.rs` sobre uma série de contagens de vizinhos.
///
/// Reproduz confirmação, escalada, liberação e cadência de lembrete. O teste de rádio
/// fica de fora de propósito: sai uma vez por sessão e não é tráfego.
function falasLaterais(m, KL, serie) {
  const falas = [];
  let confirmada = 0;
  let candidata = 0;
  let desde = 0;
  let ultimoAnuncio = 0;
  let intervalo = KL.LEMBRETE_S;
  for (let q = 0; q < m.n; q += 1) {
    const t = m.t[q];
    // 0 vizinhos = livre; 1 = um lado; 2 ou mais = três largos. É a `largura` do fonte.
    const alvo = Math.min(serie[q], 2);
    if (alvo !== candidata) {
      candidata = alvo;
      desde = t;
    }
    if (candidata !== confirmada) {
      const espera = candidata === 0 ? KL.CONFIRMA_LIVRE_S : KL.CONFIRMA_LADO_S;
      if (t - desde >= espera) {
        const anterior = confirmada;
        confirmada = candidata;
        // Entrada/escalada tem prioridade sobre liberação, como no fonte.
        const chave = confirmada > anterior ? "entrada" : "liberacao";
        falas.push({ t, chave });
        ultimoAnuncio = t;
        intervalo = KL.LEMBRETE_S;
        continue;
      }
    }
    if (confirmada > 0 && t - ultimoAnuncio >= intervalo) {
      intervalo = Math.min(intervalo + KL.LEMBRETE_PASSO_S, KL.LEMBRETE_MAX_S);
      falas.push({ t, chave: "lembrete" });
      ultimoAnuncio = t;
    }
  }
  return falas;
}

/// Quantos instantes de `a` têm algum instante de `b` a menos de `COLISAO_S`.
///
/// Contado nos DOIS sentidos pelo relatório, e os dois querem dizer coisas diferentes.
/// "Quantas laterais colidem" mede o quanto o lateral é atrapalhado — e com milhares de
/// falas laterais o percentual é sempre pequeno. "Quantas das outras colidem" mede se
/// `carro_atras` chega a ser ouvido, que é a pergunta que decide a tabela de prioridade.
function colisoes(a, b) {
  if (!a.length || !b.length) return 0;
  const ordenadas = [...b].sort((x, y) => x - y);
  let n = 0;
  for (const t of a) {
    // Busca binária pelo mais próximo: o laço ingênuo aqui é 40 pilotos × milhares de
    // falas e aparece no relógio.
    let lo = 0;
    let hi = ordenadas.length - 1;
    let melhor = Infinity;
    while (lo <= hi) {
      const meio = (lo + hi) >> 1;
      const dt = Math.abs(ordenadas[meio] - t);
      if (dt < melhor) melhor = dt;
      if (ordenadas[meio] < t) lo = meio + 1;
      else hi = meio - 1;
    }
    if (melhor < COLISAO_S) n += 1;
  }
  return n;
}

function imprimirLateral(m, K, resultados, carga) {
  console.log(`\n  ── spotter lateral ${"─".repeat(56)}`);
  const nomes = { 0: "desligado", 1: "livre", 2: "esquerda", 3: "direita", 4: "três largos", 5: "duas à esquerda", 6: "duas à direita" };
  // Campo AUSENTE e canal PARADO são zeros diferentes, e confundi-los engana em direções
  // opostas: o parado sugere que a próxima corrida resolve, o ausente é definitivo para
  // aquela gravação. `car_left_right` só existe a partir do formato 2.
  if (!m.temCanalLateral) {
    console.log(
      `  CRU: \`car_left_right\` NÃO EXISTE nesta gravação (formato ${m.versao ?? "?"}) — o campo\n` +
        `    veio depois. Não é canal parado: é captura de uma versão que não o gravava, e\n` +
        `    nenhuma releitura deste arquivo vai produzi-lo.`,
    );
  } else {
    const cru = lateralCru(m);
    console.log(
      `  CRU (\`CarLeftRight\` da gravação): ${cru.transicoes} transições em ${cru.quadros} quadros em verde, ` +
        `${cru.naPista} deles com o jogador na pista`,
    );
    console.log(
      `    valores vistos: ${[...cru.valores].map(([v, n]) => `${nomes[v] ?? v} ×${n}`).join(", ")}`,
    );
    if (!cru.transicoes) {
      console.log(
        `    · canal PARADO: o campo existe e não mudou. O jogador real passou a prova no box\n` +
          `      e o canal é DELE — zero aqui é sobre esta captura, não sobre o spotter lateral.`,
      );
    }
  }
  if (!carga) return null;
  const KL = lerConstantesRust(LATERAL_FONTE, LATERAL_CONSTANTES);
  console.log(
    `  do fonte (${path.basename(LATERAL_FONTE)}): ` +
      Object.entries(KL).map(([c, v]) => `${c}=${nf(v, 2)}`).join("  "),
  );
  const pilotos = carga.pilotos;
  const contagens = vizinhanciaOuVazio(m, K, pilotos);
  // As falas das outras famílias, por piloto — o denominador da colisão.
  const outrasPorPiloto = new Map(pilotos.map((p) => [p, { tres: [], comLento: [] }]));
  for (const l of carga.leituras) {
    const alvo = l.nome.startsWith("fora") ? "tres" : "comLento";
    for (const [p, lista] of l.porPiloto) outrasPorPiloto.get(p)[alvo] = lista.map((f) => f.t);
  }
  console.log(`  RECONSTRUÍDO — carros dentro de ±L m no eixo da pista (L não sai de fonte nenhum):`);
  const nTres = [...outrasPorPiloto.values()].reduce((a, v) => a + v.tres.length, 0);
  const linhas = [];
  for (let j = 0; j < LATERAL_JANELAS_M.length; j += 1) {
    let total = 0;
    let lembretes = 0;
    let laterCol = 0;
    let tresCol = 0;
    const porPiloto = [];
    const falasPorPiloto = new Map();
    pilotos.forEach((p, k) => {
      const falas = falasLaterais(m, KL, contagens[j][k]);
      falasPorPiloto.set(p, falas);
      total += falas.length;
      lembretes += falas.filter((f) => f.chave === "lembrete").length;
      porPiloto.push(falas.length);
      const ts = falas.map((f) => f.t);
      laterCol += colisoes(ts, outrasPorPiloto.get(p).tres);
      tresCol += colisoes(outrasPorPiloto.get(p).tres, ts);
    });
    const d = distribuicao(porPiloto);
    // `falas` sai junto do agregado por causa do despejo da linha do tempo: recomputá-lo lá
    // custaria a varredura geométrica inteira de novo, que é o trecho caro deste arreio.
    linhas.push({ L: LATERAL_JANELAS_M[j], total, lembretes, d, laterCol, tresCol, nTres, falasPorPiloto });
    console.log(
      `    ±${String(LATERAL_JANELAS_M[j]).padStart(2)} m  ${String(total).padStart(5)} falas ` +
        `(${pct(lembretes, total)} lembrete) · ${nf(d.media, 1).padStart(5)} por piloto-corrida · ` +
        `máximo ${String(d.max).padStart(3)}`,
    );
    console.log(
      `            colisão a menos de ${nf(COLISAO_S, 0)} s: ` +
        `${tresCol} de ${nTres} falas de fora/parado/tras (${pct(tresCol, nTres)}) ` +
        `— e ${pct(laterCol, total)} das laterais`,
    );
  }
  return linhas;
}

// ═══════════════════ Despejo da LINHA DO TEMPO ═══════════════════
//
// O relatório acima responde "quantas falas". A pergunta seguinte — se o rádio INTEIRO cabe num
// par de ouvidos — precisa de outra coisa: o INSTANTE de cada fala, para juntar com as famílias
// que não saem de captura nenhuma (quebra na grade, peça do nosso carro, volta mais rápida) e
// passar tudo pela fila de voz. Ver `scripts/analise-radio.mjs`.
//
// Sai daqui em vez de virar arreio novo porque o espelho do spotter mora aqui, e espelho
// duplicado é o defeito que este arquivo existe para não ter.

/// Qual hipótese geométrica do lateral entra na linha do tempo (índice em `LATERAL_JANELAS_M`).
/// A do meio. O lateral é o maior falador de todos, então a escolha muda o resultado — e por
/// isso ela vai NOMEADA no JSON, não embutida.
const LATERAL_PARA_LINHA = 1; // ±5 m

/// As voltas que um carro cruzou, pelo salto de `lap_dist_pct` de ~1 para ~0.
///
/// Vem da posição e não de um contador de voltas porque `cars[]` não traz o contador — e a
/// travessia é o que interessa: é ela que dá o instante e o tempo de volta.
function voltasCruzadas(m, idx) {
  const c = m.carros[idx];
  const voltas = [];
  let anterior = NaN;
  let ultimoT = null;
  for (let q = 0; q < m.n; q += 1) {
    const p = c.pct[q];
    if (!Number.isFinite(p)) {
      anterior = NaN;
      continue;
    }
    // Salto grande para trás = cruzou a linha. O 0,5 separa a travessia de qualquer
    // oscilação de amostragem.
    if (Number.isFinite(anterior) && anterior - p > 0.5) {
      const t = m.t[q];
      voltas.push({ t, tempoS: ultimoT === null ? null : t - ultimoT });
      ultimoT = t;
    }
    anterior = p;
  }
  return voltas;
}

function despejarLinhaDoTempo(m, carga, lateral, arquivo) {
  if (!carga) return;
  const laterais = lateral?.[LATERAL_PARA_LINHA];
  const leitura = carga.leituras[0]; // fora + parado + tras — as famílias que existem hoje
  const pilotos = carga.pilotos.map((p) => ({
    idx: p,
    falas: [
      ...(leitura.porPiloto.get(p) ?? []).map((f) => ({ t: f.t, familia: f.familia })),
      ...(laterais?.falasPorPiloto?.get(p) ?? []).map((f) => ({
        t: f.t,
        familia: "lateral",
        chave: f.chave,
      })),
    ].sort((a, b) => a.t - b.t),
    voltas: voltasCruzadas(m, p),
  }));
  const doc = {
    captura: m.arquivo,
    sessao: m.sessaoNum,
    pista: m.pista,
    duracaoS: m.duracaoS,
    // O relógio da captura é o `session_time` do iRacing, que não começa em zero na sessão de
    // corrida — num arquivo com classificatório antes, ela começa perto dos mil segundos. Sem
    // este par, todo silêncio seria medido desde a virada do relógio e não desde a largada.
    t0: m.primeiro,
    t1: m.ultimo,
    lateralJanelaM: laterais ? LATERAL_JANELAS_M[LATERAL_PARA_LINHA] : null,
    pilotos,
  };
  const alvo = arquivo.replace(/\.json$/, "") + `.${m.arquivo.replace(/\W+/g, "_")}.${m.sessaoNum}.json`;
  fs.writeFileSync(alvo, JSON.stringify(doc));
  console.log(
    `\n  ── linha do tempo despejada ${"─".repeat(47)}\n` +
      `  ${pilotos.length} piloto(s) · ${pilotos.reduce((a, p) => a + p.falas.length, 0)} falas de spotter · ` +
      `lateral a ±${doc.lateralJanelaM ?? "?"} m\n  → ${alvo}`,
  );
}

/// Envelope do cálculo geométrico — separado só para o relatório não carregar o custo
/// quando não há pilotos a medir.
function vizinhanciaOuVazio(m, K, pilotos) {
  return pilotos.length ? vizinhancaGeometrica(m, K, pilotos) : [];
}

// ═══════════ Sobreposição entre famílias: `lento` contra `frente` ═══════════
//
// Um carro parado é tecnicamente um carro muito lento, e um carro voltando da grama
// também. O número que interessa não é filosófico: é quantos avisos de `lento` são sobre
// um carro que `frente` já pegaria NO MESMO INSTANTE. Ele decide se a família se justifica
// sozinha ou se é caso para a tabela de prioridade da voz.

function sobreposicao(m, rLento, rFrente, K) {
  if (!rLento || !rFrente) return null;
  // Os dois detectores rodaram com a MESMA lista de pilotos, na mesma ordem.
  const idxDe = new Map(rFrente.pilotos.map((p, k) => [p, k]));
  let total = 0;
  let simultaneos = 0;
  let recentes = 0;
  const porFamilia = new Map();
  for (const chave of ["lento", "muito_lento"]) {
    const f = rLento.porFamilia.get(chave);
    if (!f) continue;
    let tf = 0;
    let sf = 0;
    for (const a of f.avisos) {
      total += 1;
      tf += 1;
      const k = idxDe.get(a.piloto);
      const abertos = k === undefined ? null : rFrente.sims[k].abertosPorQuadro[a.quadro];
      const junto = !!abertos && abertos.includes(a.carIdx);
      if (junto) {
        simultaneos += 1;
        sf += 1;
      }
      // A leitura de B, que é outra: "esteve fora da pista ou abaixo de 5 km/h nos 10 s
      // anteriores". Mede passado, não simultaneidade — as duas respondem perguntas
      // diferentes e as duas entram no relatório.
      let antes = false;
      for (let q = a.quadro; q >= 0 && m.t[q] >= m.t[a.quadro] - K.PICO_JANELA_S; q -= 1) {
        const c = m.carros[a.carIdx];
        if (!c.presente[q]) continue;
        const v = rFrente.derivado.carros[a.carIdx].vel[q];
        if (c.sup[q] === K.SUP_FORA_DA_PISTA || (!Number.isNaN(v) && v < K.PARADO_KMH)) {
          antes = true;
          break;
        }
      }
      if (antes) recentes += 1;
    }
    porFamilia.set(chave, { total: tf, simultaneos: sf });
  }
  return { total, simultaneos, recentes, porFamilia };
}

// ═══════════════ Verificação contra os números já publicados ═══════════════
//
// O arreio está certo quando reproduz `docs/spotter-obstaculo.md`. Se algum não bater, ou
// o arreio está errado, ou os números publicados estão — e a segunda hipótese só vale com
// a evidência junto.

const ESPERADO = {
  race_1785885657: {
    pista: "Lime Rock",
    fora: { n: 3, duracoes: [11.8, 4.18, 0.2] },
    parado: { n: 0, duracoes: [] },
  },
  race_1785889561: {
    pista: "Okayama",
    parado: { n: 4, duracoes: [19.7, 7.98, 4.65, 4.18], sumiram: [25, 20] },
  },
};
// Os dois pares são medidos com o carro do jogador real contando como obstáculo — a mesma
// regra para as duas famílias. O doc trazia 51/16% para "parado", que saíra da contagem
// SEM esse carro, enquanto os 40 de "fora" saíram da contagem COM; a tabela misturava dois
// critérios. Ver `--sem-jogador-real` para a outra leitura.
const ESPERADO_SOMA = {
  fora: { avisos: 40, inuteisPct: 35 },
  parado: { avisos: 54, inuteisPct: 15 },
};

function verificar(porArquivo) {
  const linhas = [];
  const ok = (bom, texto) => linhas.push(`  ${bom ? "✔" : "✘"} ${texto}`);
  const quaseIgual = (a, b, tol) => Math.abs(a - b) <= tol;

  for (const [chave, esp] of Object.entries(ESPERADO)) {
    const r = porArquivo.get(chave);
    if (!r) {
      linhas.push(`  · ${esp.pista}: captura não analisada nesta rodada`);
      continue;
    }
    for (const familia of ["fora", "parado"]) {
      const alvo = esp[familia];
      if (!alvo) continue;
      const eps = r.episodios.filter((e) => e.tipo === familia);
      const durs = eps.map((e) => e.duracaoS).sort((a, b) => b - a);
      ok(
        eps.length === alvo.n,
        `${esp.pista}, episódios "${familia}": ${eps.length} (esperado ${alvo.n})` +
          (durs.length ? ` — ${durs.map((d) => nf(d, 2)).join(" / ")} s` : ""),
      );
      if (alvo.duracoes.length && eps.length === alvo.n) {
        const bate = alvo.duracoes.every((d, k) => quaseIgual(d, durs[k] ?? -1, 0.06));
        ok(bate, `${esp.pista}, durações "${familia}" batem com o publicado`);
      }
      if (alvo.sumiram) {
        const sumiram = eps.filter((e) => e.desfecho === DESFECHO.sumiu).map((e) => e.carIdx);
        ok(
          alvo.sumiram.every((i) => sumiram.includes(i)),
          `${esp.pista}, ${alvo.sumiram.map((i) => `#${i}`).join(" e ")} com desfecho "sumiu do array": ` +
            `[${sumiram.map((i) => `#${i}`).join(", ") || "nenhum"}]`,
        );
        const longos = eps.filter((e) => e.duracaoS > 100);
        ok(longos.length === 0, `${esp.pista}, nenhum episódio contando o tempo de ausência (>100 s)`);
      }
    }
  }

  const refs = Object.keys(ESPERADO).map((k) => porArquivo.get(k));
  if (refs.every(Boolean)) {
    for (const [familia, alvo] of Object.entries(ESPERADO_SOMA)) {
      const soma = (sufixo) => {
        let total = 0;
        let uteis = 0;
        let inuteis = 0;
        for (const r of refs) {
          const f = r.porFamilia.get(familia + sufixo);
          if (!f) continue;
          total += f.total;
          uteis += f.uteis;
          inuteis += f.inuteis;
        }
        return { total, pct: (100 * inuteis) / (uteis + inuteis || 1) };
      };
      const s = soma("");
      const alt = soma(":alt");
      ok(
        s.total === alvo.avisos,
        `duas corridas, avisos "${familia}": ${s.total} (esperado ${alvo.avisos})` +
          (s.total === alvo.avisos || alt.total !== alvo.avisos
            ? ""
            : ` — daria ${alt.total} ${IGNORAR_JOGADOR_REAL ? "com" : "sem"} o carro do jogador real como obstáculo`),
      );
      ok(
        quaseIgual(s.pct, alvo.inuteisPct, 1),
        `duas corridas, inúteis "${familia}": ${nf(s.pct, 0)}% (esperado ${alvo.inuteisPct}%)`,
      );
    }
  } else {
    linhas.push(`  · soma das duas corridas: precisa das duas capturas de referência na mesma rodada`);
  }
  return linhas;
}

// ═══════════════════════════ Entrada ═══════════════════════════

const BANDEIRAS = new Set(["--avisos", "--sem-jogador-real", "--azul-para-todos"]);
/// Bandeiras que levam valor. Ficam à parte porque o casamento é por prefixo, e um prefixo
/// frouxo aceitaria o typo que a checagem existe para pegar.
const PREFIXOS = ["--trilha=", "--linha-do-tempo="];

/// Resolve os caminhos da linha de comando em capturas, com a lista do que ficou de fora.
///
/// Cada descarte carrega o motivo. É o ponto da função: passar duas capturas e analisar uma
/// calado — que era o defeito da versão anterior — dá um relatório que parece completo e
/// tem metade dos dados. Um argumento que não vira análise tem de doer.
function reunirAlvos(argumentos) {
  const descartes = [];
  const escolhidos = [];
  const origem = new Map(); // caminho absoluto -> por onde entrou

  const aceitar = (arquivo, entrouPor) => {
    const real = path.resolve(arquivo);
    if (origem.has(real)) {
      descartes.push({
        caminho: arquivo,
        motivo: `veio de \`${entrouPor}\` e já havia entrado por \`${origem.get(real)}\` — mesma captura`,
      });
      return;
    }
    origem.set(real, entrouPor);
    escolhidos.push(real);
  };

  for (const arg of argumentos) {
    if (!fs.existsSync(arg)) {
      // Erro duro, e não descarte: um caminho com typo entre dois válidos derrubaria a
      // verificação somada para uma corrida só, e o relatório sairia verde do mesmo jeito.
      console.error(`Não achei \`${arg}\`.`);
      process.exit(1);
    }
    if (!fs.statSync(arg).isDirectory()) {
      // Arquivo nomeado à mão entra como está: sem filtro de extensão e sem limite de
      // tamanho. Quem digitou o caminho já decidiu.
      aceitar(arg, arg);
      continue;
    }
    for (const nome of fs.readdirSync(arg).sort()) {
      const f = path.join(arg, nome);
      if (!fs.statSync(f).isFile()) continue;
      if (!/\.jsonl(\.gz)?$/.test(nome)) {
        descartes.push({ caminho: f, motivo: "não tem extensão de captura (.jsonl / .jsonl.gz)" });
        continue;
      }
      const mb = fs.statSync(f).size / 1048576;
      if (mb > LIMITE_PASTA_MB) {
        descartes.push({
          caminho: f,
          motivo: `${mb.toFixed(0)} MB, acima do limite de ${LIMITE_PASTA_MB} MB para varredura de pasta — passe o caminho direto para incluir`,
        });
        continue;
      }
      aceitar(f, arg);
    }
  }
  return { escolhidos, descartes };
}

// Constantes de cada detector, do fonte. Um detector sem fonte só é erro se for obrigatório
// — as frentes A e B ainda estão sendo escritas.
const detectores = [];
for (const d of DETECTORES) {
  const existe = fs.existsSync(path.join(RAIZ, d.fonte));
  if (!existe) {
    if (d.obrigatorio) {
      console.error(`Fonte obrigatório ausente: ${d.fonte}`);
      process.exit(1);
    }
    console.log(`· detector \`${d.nome}\`: ${d.fonte} ainda não existe — nada a reproduzir`);
    continue;
  }
  if (!d.criar) {
    console.log(
      `· detector \`${d.nome}\`: fonte presente, espelho ainda não escrito — ` +
        `acrescente \`criar\` na tabela DETECTORES`,
    );
    continue;
  }
  const K = lerConstantesRust(d.fonte, d.constantes, d.expressoes);
  if (K.MAX_CARROS > MAX_LEITURA) {
    // A leitura crua para em MAX_LEITURA. Um detector que enxerga mais longe leria o mundo
    // pela metade e reportaria número menor sem nada parecer errado.
    console.error(
      `Detector \`${d.nome}\`: MAX_CARROS=${K.MAX_CARROS} acima do teto de leitura (${MAX_LEITURA}). ` +
        `Suba MAX_LEITURA no arreio.`,
    );
    process.exit(1);
  }
  detectores.push({ ...d, K });
}

const bandeirasDesconhecidas = process.argv
  .slice(2)
  .filter((a) => a.startsWith("--") && !BANDEIRAS.has(a) && !PREFIXOS.some((p) => a.startsWith(p)));
if (bandeirasDesconhecidas.length) {
  // Uma bandeira com typo não pode virar um argumento ignorado: `--sem-jogador` em vez de
  // `--sem-jogador-real` rodaria a análise inteira com a configuração errada, calada.
  console.error(
    `Bandeira desconhecida: ${bandeirasDesconhecidas.join(", ")}. Conheço ${[...BANDEIRAS].join(", ")}.`,
  );
  process.exit(1);
}

const caminhos = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const padrao = path.join(process.env.APPDATA || "", "com.loop.app", "debug", "race_captures");
const { escolhidos, descartes } = reunirAlvos(caminhos.length ? caminhos : [padrao]);

console.log(`\nENTRADA: ${caminhos.length || 1} caminho(s) → ${escolhidos.length} captura(s) a analisar`);
for (const d of descartes) console.log(`  · não lido: ${path.basename(d.caminho)} — ${d.motivo}`);
if (!escolhidos.length) {
  console.error(`Nenhuma captura para analisar.`);
  process.exit(1);
}

const porArquivo = new Map();
const foraDaVerificacao = [];
for (const a of escolhidos) {
  const nome = path.basename(a);
  let r = null;
  try {
    r = analisarCaptura(a, detectores);
  } catch (e) {
    console.log(`  ⚠ falhou: ${e.message}`);
    foraDaVerificacao.push({ nome, motivo: `erro na leitura — ${e.message}` });
    continue;
  }
  const base = nome.replace(/\.jsonl(\.gz)?$/, "");
  const corridas = r.sessoes.filter((s) => s.ehCorrida);
  for (const s of r.sessoes) {
    const rotulo = r.sessoes.length > 1 ? `${nome} #${s.sessaoNum}` : nome;
    if (!s.ehCorrida) {
      // Dito em voz alta, e não descartado em silêncio: um bloco de classificatório na
      // amostra de corrida é exatamente o defeito que esta separação existe para matar.
      foraDaVerificacao.push({
        nome: rotulo,
        motivo: `sessão \`${s.tipoSessao || "tipo não registrado"}\` — não é corrida`,
      });
      continue;
    }
    if (s.erro) {
      foraDaVerificacao.push({ nome: rotulo, motivo: s.erro });
      continue;
    }
    // A chave só ganha sufixo quando o arquivo traz mais de uma corrida — assim uma captura
    // de sessão única continua casando com a verificação pelo nome de sempre.
    const chave = corridas.length > 1 ? `${base}#${s.sessaoNum}` : base;
    if (porArquivo.has(chave)) {
      // Mesmo nome vindo de pastas diferentes. A verificação casa por nome, então a segunda
      // sobrescreveria a primeira sem deixar rastro.
      foraDaVerificacao.push({ nome: rotulo, motivo: `nome repetido — já havia \`${chave}\`` });
      continue;
    }
    porArquivo.set(chave, s);
  }
}

// A carga das capturas somadas. O par piloto-corrida é a unidade: o mesmo #13 em duas
// corridas são duas linhas, porque a pergunta é o que ele ouve NUMA corrida.
const cargas = [...porArquivo.values()].map((r) => r.carga).filter(Boolean);
if (cargas.length > 1) {
  console.log(`\n${"═".repeat(78)}\nCARGA SOMADA — ${cargas.length} corridas juntas`);
  for (let k = 0; k < cargas[0].leituras.length; k += 1) {
    const contagens = cargas.flatMap((c) => [...c.leituras[k].porPiloto.values()].map((v) => v.length));
    const colisoes = cargas.reduce((a, c) => a + c.leituras[k].colisoes, 0);
    const d = distribuicao(contagens);
    console.log(
      `  ${cargas[0].leituras[k].nome.padEnd(22)} ${String(d.total).padStart(4)} falas · ` +
        `${nf(d.media, 2)} por piloto-corrida · máximo ${d.max}` +
        (colisoes ? ` · ${colisoes} colisão(ões) de tique` : ""),
    );
    console.log(
      `    ${d.baldes.map((b) => `${b.rotulo}: ${b.n}`).join("   ")}   ` +
        `(de ${d.n} pares piloto-corrida)`,
    );
  }
  // A colisão com o lateral, somada. É o número que decide a tabela de prioridade, e ele
  // varia com a hipótese geométrica — por isso sai a varredura inteira e nunca um valor só.
  const laterais = [...porArquivo.values()].map((r) => r.lateral).filter(Boolean);
  if (laterais.length) {
    console.log(`\n  contra o spotter lateral (reconstruído — L é hipótese, não medida):`);
    for (let j = 0; j < LATERAL_JANELAS_M.length; j += 1) {
      const col = laterais.reduce((a, l) => a + l[j].tresCol, 0);
      const base = laterais.reduce((a, l) => a + l[j].nTres, 0);
      const tot = laterais.reduce((a, l) => a + l[j].total, 0);
      console.log(
        `    ±${String(LATERAL_JANELAS_M[j]).padStart(2)} m  ${tot} falas laterais · ` +
          `${col} de ${base} falas de fora/parado/tras a menos de ${nf(COLISAO_S, 0)} s ` +
          `de uma delas (${pct(col, base)})`,
      );
    }
  }
}

console.log(`\n${"═".repeat(78)}\nVERIFICAÇÃO contra docs/spotter-obstaculo.md`);
console.log(
  // Sessões, não capturas: uma captura de fim de semana traz mais de uma, e só as de
  // corrida entram aqui.
  `  base: ${porArquivo.size} sessão(ões) de corrida em ${escolhidos.length} captura(s)` +
    (foraDaVerificacao.length ? `, ${foraDaVerificacao.length} fora:` : ""),
);
for (const f of foraDaVerificacao) console.log(`    · ${f.nome} — ${f.motivo}`);
for (const l of verificar(porArquivo)) console.log(l);
