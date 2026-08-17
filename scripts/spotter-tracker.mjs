#!/usr/bin/env node
// O TRACKER DO SPOTTER: o que ele falou, o que ele engoliu, e com que informação decidiu.
//
// Duas fontes já existiam e nunca se olharam:
//
//   • `<app_data>/logs/radio/*.jsonl` — toda fala, em duas fases (`decidida` no Rust,
//     `tocada` no front) e com o desfecho de quem não saiu. Ver `radio_registro.rs`.
//   • `<app_data>/debug/race_captures/race_*.jsonl.gz` — a corrida inteira a 60 Hz, sempre
//     ligada. Ver `race_capture.rs`.
//
// Separadas, cada uma responde metade da pergunta. O registro diz "carro_fora_frente às
// 412,3 s" e não diz onde o jogador estava nem a que velocidade; a captura diz o mundo
// inteiro e não diz o que o rádio fez com ele. Juntas por `(sn, t)`, respondem a pergunta
// que se faz depois de uma corrida teste: **isso que eu ouvi fazia sentido, e o que eu não
// ouvi por que não saiu?**
//
// A terceira fonte é nova e é o canal `spotter_diario` do mesmo arquivo do rádio (ver
// `iracing_sdk/spotter_diario.rs`): a RECUSA, com o motivo e a folga que faltou para o
// limiar. É o que transforma "o rádio ficou calado" em "houve 34 candidatos e 31 foram
// recusados por menos de 12 m".
//
// Uso:
//   node scripts/spotter-tracker.mjs                     # a sessão mais recente
//   node scripts/spotter-tracker.mjs --lista             # que sessões existem
//   node scripts/spotter-tracker.mjs --radio <arquivo.jsonl> [--captura <arquivo.jsonl.gz>]
//   node scripts/spotter-tracker.mjs --familia frente,tras
//   node scripts/spotter-tracker.mjs --so-recusas        # só o que não saiu
//   node scripts/spotter-tracker.mjs --sem-mundo         # pula a captura (bem mais rápido)
//
// A captura é lida em FLUXO e só os quadros dos instantes pedidos ficam na memória: o
// arquivo passa dos 250 MB descomprimido, e um tracker que estoura a memória em corrida
// longa não serve para a corrida que mais interessa medir.
//
// NÃO é um teste de `npm run test:structure`, pela mesma razão do `analise-spotter.mjs`: as
// duas fontes moram em `%APPDATA%` e não estão no repositório.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import zlib from "node:zlib";

import { FALAS } from "./spotter-falas.mjs";

// ─── Onde as fontes moram ────────────────────────────────────────────────────

function pastaDoApp() {
  const appdata = process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming");
  return path.join(appdata, "com.loop.app");
}

const PASTA_RADIO = path.join(pastaDoApp(), "logs", "radio");
const PASTA_CAPTURA = path.join(pastaDoApp(), "debug", "race_captures");

function listar(pasta, filtro) {
  if (!fs.existsSync(pasta)) return [];
  return fs
    .readdirSync(pasta)
    .filter(filtro)
    .map((n) => {
      const completo = path.join(pasta, n);
      return { nome: n, caminho: completo, mtime: fs.statSync(completo).mtimeMs };
    })
    .sort((a, b) => b.mtime - a.mtime);
}

const listarRadios = () => listar(PASTA_RADIO, (n) => n.startsWith("radio-") && n.endsWith(".jsonl"));
const listarCapturas = () => listar(PASTA_CAPTURA, (n) => n.startsWith("race_") && n.endsWith(".jsonl.gz"));

// ─── Leitura do registro do rádio ────────────────────────────────────────────

function lerRadio(caminho) {
  const linhas = [];
  let cabecalho = null;
  for (const bruta of fs.readFileSync(caminho, "utf8").split("\n")) {
    if (!bruta.trim()) continue;
    let l;
    try {
      l = JSON.parse(bruta);
    } catch {
      // Uma linha truncada é o app tendo morrido no meio da escrita. Descartar em silêncio
      // esconderia isso, então ela conta como linha ruim no bloco ENTRADA.
      linhas.push({ ruim: true });
      continue;
    }
    if (l.kind === "header") cabecalho = l;
    else linhas.push(l);
  }
  return { cabecalho, linhas };
}

/// `2026-08-11 10:37:34.243` no relógio local vira epoch em ms.
function relParaEpoch(rel) {
  if (typeof rel !== "string") return null;
  const t = Date.parse(rel.replace(" ", "T"));
  return Number.isNaN(t) ? null : t;
}

// ─── Casamento entre o registro e a captura ──────────────────────────────────

/// Qual captura estava gravando enquanto este registro de rádio era escrito.
///
/// O casamento é pelo RELÓGIO DE PAREDE, e não pelo tempo de sessão: `t` reinicia a cada
/// sessão do fim de semana e não serve para dizer de que corrida um arquivo fala. A captura
/// declara o início no nome e no `stamp` do cabeçalho, e o fim é a última escrita do
/// arquivo. Um registro cujo intervalo cai dentro desse par é o registro daquela corrida.
///
/// Devolve `{ caminho, motivo }`, com o motivo escrito também quando NÃO acha: um tracker
/// que silenciosamente roda sem o mundo produz um relatório que parece completo e está pela
/// metade.
function casarCaptura(radio, capturas) {
  const tempos = radio.linhas.map((l) => relParaEpoch(l.rel)).filter((t) => t !== null);
  if (tempos.length === 0) return { caminho: null, motivo: "o registro do rádio não tem nenhuma linha datada" };
  const inicio = Math.min(...tempos);
  const fim = Math.max(...tempos);

  let melhor = null;
  for (const c of capturas) {
    const m = c.nome.match(/^race_(\d+)/);
    if (!m) continue;
    const abertura = Number(m[1]) * 1000;
    // Uma captura cobre o registro quando os dois intervalos se cruzam. O fim da captura é
    // a última escrita dela, que é o que existe sem abrir o arquivo.
    const sobreposicao = Math.min(fim, c.mtime) - Math.max(inicio, abertura);
    if (sobreposicao <= 0) continue;
    if (!melhor || sobreposicao > melhor.sobreposicao) melhor = { ...c, sobreposicao };
  }
  if (!melhor) {
    return {
      caminho: null,
      motivo: `nenhuma captura cobre ${new Date(inicio).toLocaleString()} a ${new Date(fim).toLocaleString()}`,
    };
  }
  return { caminho: melhor.caminho, nome: melhor.nome, motivo: null };
}

// ─── Leitura da captura, em fluxo ────────────────────────────────────────────

/// Comprimento da pista em metros, tirado do YAML de sessão.
///
/// É o número que converte `lap_dist_pct` em metros, e sem ele toda distância deste
/// relatório sairia em fração de volta, que não é a unidade em que os limiares do spotter
/// foram calibrados.
function comprimentoDoYaml(yaml) {
  const m = /TrackLength:\s*([\d.]+)\s*km/.exec(yaml || "");
  return m ? Number(m[1]) * 1000 : null;
}

/// Distância de `de` até `para` andando para a frente, fechando o círculo da volta.
/// A mesma conta do `spotter_base::adiante`, e pela mesma razão: quem está em 0,98
/// perseguindo quem está em 0,02 está 4% atrás, e não 96% à frente.
function adiante(de, para, comprimento) {
  let d = para - de;
  if (d < 0) d += 1;
  return d * comprimento;
}

/// Percorre a captura uma vez e guarda o quadro mais próximo de cada instante pedido.
///
/// `alvos` é um `Set` de `sn:t`. A tolerância é de meio tique a 60 Hz para cada lado; o
/// registro do rádio carimba com o `session_time` do mesmo amostrador que escreveu a
/// captura, então o casamento é exato na esmagadora maioria dos casos e a tolerância existe
/// para o arredondamento de três casas do registro.
async function lerCaptura(caminho, alvos) {
  const quadros = new Map();
  let comprimento = null;
  let carsUltimo = [];
  let frames = 0;

  // O `cars[]` só é gravado a 20 Hz (ver `race_capture.rs`), então quem lê tem de carregar
  // o último visto adiante. Sem isto, quatro de cada cinco falas viriam sem vizinhança
  // nenhuma e o relatório diria "não havia ninguém por perto" quando havia.
  //
  // `Z_SYNC_FLUSH` no fim do fluxo é o que torna a captura de uma corrida EM ANDAMENTO
  // legível. A escrita dá flush a cada 60 quadros e só finaliza o fluxo gzip no `stop()`,
  // então o arquivo da sessão que está rodando agora está sempre truncado do ponto de vista
  // do formato. Sem esta opção o `zlib` estoura com `Z_BUF_ERROR` no último bloco e joga
  // fora a corrida inteira que acabou de ser gravada, que é justamente a que se quer olhar.
  const gunzip = zlib.createGunzip({ finishFlush: zlib.constants.Z_SYNC_FLUSH });
  let truncada = false;
  gunzip.on("error", () => {
    truncada = true;
  });
  const fluxo = readline.createInterface({
    input: fs.createReadStream(caminho).pipe(gunzip),
    crlfDelay: Infinity,
  });

  const procurados = new Map();
  for (const chave of alvos) {
    const [sn, t] = chave.split(":");
    const lista = procurados.get(sn) || [];
    lista.push({ chave, t: Number(t) });
    procurados.set(sn, lista);
  }

  for await (const linha of fluxo) {
    if (!linha) continue;
    let l;
    try {
      l = JSON.parse(linha);
    } catch {
      continue;
    }
    if (l.kind === "session") {
      comprimento = comprimentoDoYaml(l.yaml) ?? comprimento;
      continue;
    }
    if (l.kind !== "frame" || !l.tele) continue;
    frames += 1;
    const tele = l.tele;
    if (Array.isArray(tele.cars) && tele.cars.length > 0) carsUltimo = tele.cars;

    const lista = procurados.get(String(tele.session_num));
    if (!lista) continue;
    for (const alvo of lista) {
      const dist = Math.abs(tele.session_time - alvo.t);
      if (dist > 0.05) continue;
      const guardado = quadros.get(alvo.chave);
      if (guardado && guardado.erro <= dist) continue;
      quadros.set(alvo.chave, { erro: dist, tele, cars: carsUltimo });
    }
  }
  return { quadros, comprimento, frames, truncada };
}

/// O mundo em volta do jogador num quadro: o vizinho da frente e o de trás, em metros.
///
/// São as duas distâncias que decidem quase tudo no spotter, e as duas que o registro do
/// rádio não carrega. `null` quando a captura não tinha `cars[]` ainda, que é o começo de
/// toda sessão.
function vizinhanca(quadro, comprimento) {
  if (!quadro || !comprimento) return null;
  const { tele, cars } = quadro;
  const eu = cars.find((c) => c.is_player || c.idx === tele.player_car_idx);
  const meuPct = eu ? eu.lap_dist_pct : tele.lap_dist_pct;
  let frente = null;
  let tras = null;
  for (const c of cars) {
    if (c.idx === tele.player_car_idx || c.is_player) continue;
    // Fora do mundo (garagem, guincho) não é vizinho de ninguém.
    if (c.track_surface === -1 || c.lap_dist_pct < 0) continue;
    const aFrente = adiante(meuPct, c.lap_dist_pct, comprimento);
    const atras = comprimento - aFrente;
    if (!frente || aFrente < frente.m) frente = { idx: c.idx, m: aFrente, sup: c.track_surface };
    if (!tras || atras < tras.m) tras = { idx: c.idx, m: atras, sup: c.track_surface };
  }
  return { frente, tras, carros: cars.length };
}

// ─── Impressão ───────────────────────────────────────────────────────────────

const SUPERFICIE = { "-1": "fora do mundo", 0: "fora da pista", 1: "na caixa", 2: "via de box", 3: "pista" };

function seg(t) {
  if (t === null || t === undefined) return "     ";
  const m = Math.floor(t / 60);
  const s = (t % 60).toFixed(1).padStart(4, "0");
  return `${String(m).padStart(2, " ")}:${s}`;
}

/// A marca da coluna do meio, no mesmo vocabulário do `radio-timeline.mjs`.
function marca(l) {
  if (l.canal === "spotter_diario") return "·";
  if (l.fase === "decidida") return "→";
  if (l.desfecho === "ok") return "▶";
  return "✗";
}

function textoDaFala(chaves) {
  for (const c of chaves || []) {
    // A variação (`esquerda_2`) tem a mesma redação-base da chave; o pacote resolve a
    // gravação, e aqui só interessa o que foi dito.
    const base = c.replace(/_\d+$/, "");
    if (FALAS[base]) return FALAS[base];
  }
  return "";
}

function imprimirFalas(linhas, quadros, comprimento) {
  console.log("\n═══ FALAS ═══");
  console.log("  sessão   marca  chave                     desfecho      mundo");
  for (const l of linhas) {
    const chave = (l.chaves || []).join(",") || "—";
    const q = quadros.get(`${l.sn}:${l.t}`);
    let mundo = "";
    if (q) {
      const v = vizinhanca(q, comprimento);
      const partes = [
        `v${Math.round(q.tele.speed_kmh)}`,
        `p${q.tele.position}`,
        `L${q.tele.lap}`,
        SUPERFICIE[String(q.tele.track_surface)] || `sup${q.tele.track_surface}`,
      ];
      if (q.tele.car_left_right) partes.push(`clr=${q.tele.car_left_right}`);
      if (v?.frente) partes.push(`frente #${v.frente.idx} ${Math.round(v.frente.m)}m`);
      if (v?.tras) partes.push(`trás #${v.tras.idx} ${Math.round(v.tras.m)}m`);
      mundo = partes.join("  ");
    }
    console.log(
      `  ${seg(l.t)}    ${marca(l)}    ${chave.padEnd(24).slice(0, 24)}  ${String(l.desfecho).padEnd(12)}  ${mundo}`,
    );
    const texto = textoDaFala(l.chaves);
    if (texto && l.fase !== "decidida") console.log(`              "${texto}"`);
  }
}

/// As recusas, agrupadas. É a leitura que responde de calibração, e por isso ela é sobre a
/// FOLGA e não sobre a contagem: cem recusas a 900 m do limiar são um detector saudável, e
/// cem recusas a 3 m são um limiar mal posto.
function imprimirRecusas(diario) {
  console.log("\n═══ RECUSAS (o que o detector viu e descartou) ═══");
  if (diario.length === 0) {
    console.log("  Nenhuma. O diário não anotou nada nesta sessão.");
    console.log("  Isto quer dizer que nenhum detector chegou a ter candidato, e não que");
    console.log("  eles estavam desligados: o portão de sessão também é anotado.");
    return;
  }
  // A FOLGA MUDOU DE SIGNIFICADO em 17/08/2026. Até ali o diário fechava a linha na
  // transição do motivo e gravava a folga daquele instante, que fica perto de zero por
  // construção; hoje ele guarda a MENOR do episódio, e acompanha de `durou_s` e `tiques`.
  // Ler um arquivo velho com a régua nova é concluir que todo limiar está no limite.
  const antigo = diario.length > 0 && diario.every((l) => l.detalhe?.d?.durou_s === undefined);
  if (antigo) {
    console.log("  ⚠ Registro anterior a 17/08/2026: a folga aqui é a do INSTANTE DA TRANSIÇÃO,");
    console.log("    e não a menor do episódio. Ela fica perto de zero por construção, então");
    console.log("    não a leia como limiar no limite.");
  }

  const grupos = new Map();
  for (const l of diario) {
    const familia = (l.chaves || [])[0] || "?";
    const chave = `${familia}/${l.desfecho}`;
    const g = grupos.get(chave) || { n: 0, folgas: [], alvos: new Set() };
    g.n += 1;
    const d = l.detalhe?.d || {};
    if (typeof d.folga === "number") g.folgas.push(d.folga);
    if (typeof l.detalhe?.alvo === "number" && l.detalhe.alvo >= 0) g.alvos.add(l.detalhe.alvo);
    grupos.set(chave, g);
  }
  const ordenados = [...grupos.entries()].sort((a, b) => b[1].n - a[1].n);
  console.log("  família/motivo                 n   carros   folga (mín / mediana / máx)");
  for (const [chave, g] of ordenados) {
    let folga = "";
    if (g.folgas.length > 0) {
      const f = [...g.folgas].sort((a, b) => a - b);
      const mediana = f[Math.floor(f.length / 2)];
      folga = `${f[0]} / ${mediana} / ${f[f.length - 1]}`;
    }
    console.log(
      `  ${chave.padEnd(28)} ${String(g.n).padStart(3)}   ${String(g.alvos.size || "—").padStart(6)}   ${folga}`,
    );
  }
}

/// Os desfechos em que a fala NUNCA chegou a soar. Ver `spotterVoice.js`.
///
/// `cortada` fica de fora de propósito, e é o detalhe que faz a contabilidade fechar: ela é
/// escrita para uma fala que JÁ tinha registrado `ok` ao começar, então somá-la às demais
/// conta a mesma fala duas vezes. Uma cortada é uma fala que soou pela metade, e não uma
/// fala perdida. `adiada` também fica fora, por outro motivo: é estado de passagem, e a
/// mesma fala volta com o próprio desfecho quando a vez dela chega.
const NUNCA_SOOU = new Set(["validade", "sem_audio", "voz_desligada", "atropelada"]);

function imprimirResumo(falas, diario, duracaoS) {
  console.log("\n═══ RESUMO ═══");
  const decididas = falas.filter((l) => l.fase === "decidida");
  const tocadas = falas.filter((l) => l.fase === "tocada");
  const comecaram = tocadas.filter((l) => l.desfecho === "ok");
  const cortadas = tocadas.filter((l) => l.desfecho === "cortada");
  const adiadas = tocadas.filter((l) => l.desfecho === "adiada");
  const mortas = tocadas.filter((l) => NUNCA_SOOU.has(l.desfecho));
  const minutos = duracaoS > 0 ? duracaoS / 60 : 0;

  // Ocupação do canal contando o que de fato soou: uma fala cortada ocupou até o corte, e
  // não a duração inteira que ela pretendia ter.
  const perdidoNoCorte = cortadas.reduce((s, l) => s + (l.detalhe?.restou_s || 0), 0);
  const ocupado = comecaram.reduce((s, l) => s + (l.detalhe?.dur_s || 0), 0) - perdidoNoCorte;

  console.log(`  Duração da sessão medida .... ${seg(duracaoS)} (${minutos.toFixed(1)} min)`);
  console.log(`  Decididas no Rust ........... ${decididas.length}`);
  console.log(`  Começaram a soar ............ ${comecaram.length}`);
  console.log(`  Dessas, cortadas no meio .... ${cortadas.length} (${perdidoNoCorte.toFixed(1)} s não ouvidos)`);
  console.log(`  Adiadas e depois retomadas .. ${adiadas.length}`);
  console.log(`  Nunca soaram ................ ${mortas.length}`);
  if (minutos > 0) console.log(`  Falas por minuto ............ ${(comecaram.length / minutos).toFixed(1)}`);
  if (duracaoS > 0) {
    console.log(`  Canal ocupado ............... ${((ocupado / duracaoS) * 100).toFixed(1)}% do tempo`);
  }
  console.log(`  Recusas anotadas ............ ${diario.length}`);

  const porDesfecho = new Map();
  for (const l of tocadas.filter((l) => l.desfecho !== "ok")) {
    porDesfecho.set(l.desfecho, (porDesfecho.get(l.desfecho) || 0) + 1);
  }
  if (porDesfecho.size > 0) {
    console.log("\n  O que o canal fez com as falas:");
    for (const [d, n] of [...porDesfecho.entries()].sort((a, b) => b[1] - a[1])) {
      console.log(`    ${String(d).padEnd(16)} ${n}`);
    }
  }

  // A DECISÃO SEM FALA. É o número que diz se a ponte entre o Rust e o áudio está inteira:
  // uma decisão que nunca começou a soar e também não morreu num desfecho sumiu no caminho.
  const orfas = decididas.length - comecaram.length - mortas.length;
  if (orfas > 0) {
    const plural = orfas === 1 ? "decisão" : "decisões";
    console.log(`\n  ⚠ ${orfas} ${plural} sem NENHUM desfecho no front.`);
    console.log("    O canal não as engoliu: elas não chegaram à camada de voz.");
    console.log("    O suspeito de sempre é a janela coberta estrangulando o poll do webview.");
  }

  const porFamilia = new Map();
  for (const l of comecaram) {
    const base = ((l.chaves || [])[0] || "?").replace(/_\d+$/, "");
    porFamilia.set(base, (porFamilia.get(base) || 0) + 1);
  }
  if (porFamilia.size > 0) {
    console.log("\n  O que soou, por chave:");
    for (const [c, n] of [...porFamilia.entries()].sort((a, b) => b[1] - a[1])) {
      console.log(`    ${c.padEnd(24)} ${n}`);
    }
  }
}

// ─── Linha de comando ────────────────────────────────────────────────────────

function lerArgumentos(argv) {
  const op = { familias: null, soRecusas: false, semMundo: false, lista: false, radio: null, captura: null };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--lista") op.lista = true;
    else if (a === "--so-recusas") op.soRecusas = true;
    else if (a === "--sem-mundo") op.semMundo = true;
    else if (a === "--radio") op.radio = argv[(i += 1)];
    else if (a === "--captura") op.captura = argv[(i += 1)];
    else if (a === "--familia") op.familias = new Set(String(argv[(i += 1)] || "").split(","));
    else if (!a.startsWith("--")) op.radio = a;
  }
  return op;
}

async function principal() {
  const op = lerArgumentos(process.argv.slice(2));

  if (op.lista) {
    console.log(`Registros de rádio em ${PASTA_RADIO}`);
    for (const r of listarRadios()) console.log(`  ${r.nome}   ${new Date(r.mtime).toLocaleString()}`);
    console.log(`\nCapturas em ${PASTA_CAPTURA}`);
    for (const c of listarCapturas()) console.log(`  ${c.nome}   ${new Date(c.mtime).toLocaleString()}`);
    return;
  }

  const radios = listarRadios();
  const caminhoRadio = op.radio || radios[0]?.caminho;
  if (!caminhoRadio || !fs.existsSync(caminhoRadio)) {
    console.error(`Nenhum registro de rádio em ${PASTA_RADIO}. Rode o app com o iRacing aberto primeiro.`);
    process.exitCode = 1;
    return;
  }

  const radio = lerRadio(caminhoRadio);
  const ruins = radio.linhas.filter((l) => l.ruim).length;
  const linhas = radio.linhas.filter((l) => !l.ruim);
  const spotter = linhas.filter((l) => l.canal === "spotter");
  let diario = linhas.filter((l) => l.canal === "spotter_diario");
  if (op.familias) {
    diario = diario.filter((l) => op.familias.has((l.chaves || [])[0]));
  }

  // ENTRADA antes de tudo, e com o que NÃO entrou. Um argumento engolido calado é como um
  // teste que não roda: parece verde.
  console.log("═══ ENTRADA ═══");
  console.log(`  Rádio ...... ${path.basename(caminhoRadio)}  (app ${radio.cabecalho?.app ?? "?"})`);
  console.log(`  Linhas ..... ${linhas.length} boas, ${spotter.length} do spotter, ${diario.length} de recusa`);
  if (ruins > 0) console.log(`  ⚠ ${ruins} linhas truncadas (o app morreu no meio de uma escrita)`);
  if (diario.length === 0) {
    console.log("  ⚠ Zero recusas: este registro é anterior ao diário do spotter, ou o");
    console.log("    detector nunca chegou a rodar. Uma corrida com o diário no ar sempre");
    console.log("    anota ao menos os portões de sessão.");
  }

  let quadros = new Map();
  let comprimento = null;
  if (!op.semMundo) {
    const capturas = listarCapturas();
    const casada = op.captura ? { caminho: op.captura, nome: path.basename(op.captura) } : casarCaptura({ linhas }, capturas);
    if (!casada.caminho) {
      console.log(`  Captura .... nenhuma (${casada.motivo})`);
      console.log("               O relatório sai sem a coluna do mundo.");
    } else {
      console.log(`  Captura .... ${casada.nome}`);
      const alvos = new Set(spotter.filter((l) => l.t !== null).map((l) => `${l.sn}:${l.t}`));
      const lido = await lerCaptura(casada.caminho, alvos);
      quadros = lido.quadros;
      comprimento = lido.comprimento;
      console.log(
        `               ${lido.frames} quadros, pista de ${comprimento ? Math.round(comprimento) : "?"} m, ` +
          `${quadros.size}/${alvos.size} instantes casados`,
      );
      if (lido.truncada) {
        console.log("               A captura está truncada, o que é o normal de uma corrida");
        console.log("               ainda em andamento. Foi lido tudo o que já chegou ao disco.");
      }
      if (quadros.size < alvos.size) {
        console.log("               Os que faltam são falas fora do intervalo desta captura,");
        console.log("               ou de uma sessão que ela não gravou.");
      }
    }
  }

  if (!op.soRecusas) imprimirFalas(spotter, quadros, comprimento);
  imprimirRecusas(diario);

  const tempos = linhas.map((l) => l.t).filter((t) => typeof t === "number");
  const duracao = tempos.length > 1 ? Math.max(...tempos) - Math.min(...tempos) : 0;
  imprimirResumo(spotter, diario, duracao);
}

principal().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
