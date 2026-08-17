#!/usr/bin/env node
// AUDITOR DA GRAVAÇÃO DE CORRIDA: o que foi escrito, e o que deveria ter sido.
//
// A captura (`iracing_sdk/race_capture.rs`) está SEMPRE ligada e escreve a corrida inteira
// em `<app_data>/debug/race_captures/race_*.jsonl.gz`. Ela é a fonte de quase tudo que se
// mede no Loop, e nunca foi conferida: um arquivo de 0,8 MB com 60 mil quadros parece uma
// corrida bem gravada mesmo quando metade dos canais vem zerada.
//
// O modo de falha que motivou este script já aconteceu em produção e está registrado em
// `iracing_sdk/canais.rs`: a leitura casa cada canal por NOME num `match`, e nome que não
// existe cai no `_ => {}` calado. `PitRepairNeeded` não existe (o nome real é
// `PitRepairLeft`), o campo ficou zerado para sempre, e o sintoma foi o dano do carro
// simplesmente não aparecer. **De fora, um canal ausente e um canal que vale zero são
// exatamente a mesma coisa.** Este script separa os dois.
//
// O que ele confere, em quatro frentes:
//
//   1. ESTRUTURA — o arquivo tem cabeçalho, inventário, YAML de sessão e o bloco `history`?
//      O gzip fechou? Sem `history` a corrida não virou dado derivado; sem o último YAML,
//      não virou resultado.
//   2. CONTINUIDADE — a 60 Hz um quadro dura 16,7 ms. Buraco maior que isso é quadro que
//      não foi escrito, e é onde uma medida de duração passa a mentir.
//   3. CANAIS MORTOS — o cruzamento das três listas: os canais que o Rust CURA
//      (`canais.rs`), os que o sim PUBLICA nesta build (o bloco `vars` da captura), e os
//      campos que de fato VARIARAM na corrida. Um canal curado fora do inventário nunca
//      será lido; um canal no inventário cujo campo ficou constante a corrida inteira é o
//      sintoma do bug acima.
//   4. O DERIVADO CONTRA O CRU — as voltas do `history` batem com a maior volta vista nos
//      quadros? Os carros do `car_laps` cobrem os que apareceram em `cars[]`?
//
// Uso:
//   node scripts/captura-auditar.mjs                  # a captura mais recente
//   node scripts/captura-auditar.mjs --lista
//   node scripts/captura-auditar.mjs <arquivo.jsonl.gz>
//   node scripts/captura-auditar.mjs --todas          # todas as capturas do disco, uma linha cada
//   node scripts/captura-auditar.mjs --constantes     # lista TODO campo constante, não só o suspeito
//
// Lê em fluxo e não guarda quadro nenhum: só os agregados. Uma corrida de enduro passa dos
// 250 MB descomprimidos, e um auditor que precisa carregar o arquivo não audita justamente
// a corrida mais cara de gravar de novo.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import readline from "node:readline";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function pastaDasCapturas() {
  const appdata = process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming");
  return path.join(appdata, "com.loop.app", "debug", "race_captures");
}

// ─── As duas listas que moram no Rust ────────────────────────────────────────
//
// Lidas do código-fonte como texto, e não copiadas para cá. É o mesmo padrão dos guards de
// `scripts/tests/`: uma segunda cópia da lista envelheceria calada, e um auditor que audita
// contra uma lista velha aprova o que devia reprovar.

/// Os canais que a leitura CURA do SDK (`canais.rs::CANAIS_CURADOS`).
function canaisCurados() {
  const src = fs.readFileSync(path.join(raiz, "src-tauri", "src", "iracing_sdk", "canais.rs"), "utf8");
  const bloco = /pub const CANAIS_CURADOS:\s*\[&str;\s*\d+\]\s*=\s*\[([\s\S]*?)\];/.exec(src);
  if (!bloco) return null;
  return [...bloco[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/// O mapa canal do SDK -> campo da telemetria, extraído do `match` de `imp/leitura.rs`.
///
/// É o que torna o diagnóstico preciso em vez de aproximado: sem ele o auditor diria "o
/// campo `pit_repair_needed` está constante" e caberia a quem lê descobrir de que canal ele
/// vem. Com ele, a linha já sai dizendo qual nome do SDK falhou.
function mapaCanalCampo() {
  const src = fs.readFileSync(path.join(raiz, "src-tauri", "src", "iracing_sdk", "imp", "leitura.rs"), "utf8");
  const porCampo = new Map();
  for (const m of src.matchAll(/"([A-Za-z0-9_]+)"\s*=>\s*(?:t|car)\.([a-z0-9_]+)\s*=/g)) {
    const [, canal, campo] = m;
    if (!porCampo.has(campo)) porCampo.set(campo, []);
    porCampo.get(campo).push(canal);
  }
  return porCampo;
}

// ─── Sentinelas conhecidas ───────────────────────────────────────────────────

// ─── A classificação dos campos, e por que ela existe ────────────────────────
//
// A primeira versão deste script tratava "campo constante a corrida inteira" como suspeita e
// pronto. Rodada sobre as onze capturas do disco, ela apontou 19 suspeitos numa sessão em que
// os 19 estavam certos: temperatura fixa numa sessão de três minutos, `session_num` constante
// dentro de uma sessão, `position` valendo 0 na classificação (que é do SDK, e está medido).
//
// Um auditor que aponta 19 falsos em toda execução treina quem o lê a ignorar a lista, e aí
// ele deixa de servir justamente no dia em que o vigésimo for verdadeiro. A constância só é
// diagnóstica quando o campo TINHA de ter variado, e isso depende do campo e da corrida.
//
// Daí os grupos abaixo. Cada um traz a condição sob a qual a constância vira alarme.

/// Ambiente e clima. Constante é o padrão: a maioria das corridas roda com tempo fixo.
const AMBIENTE = new Set([
  "air_temp", "track_temp", "track_temp_crew", "relative_humidity", "wind_dir", "wind_ms",
  "skies", "precipitation", "track_wetness", "fog_level", "weather_declared_wet",
]);

/// Parâmetros da sessão. Constantes DENTRO de uma sessão por definição.
const SESSAO = new Set([
  "session_num", "session_state", "session_time_total", "session_laps_total",
  "session_laps_remain", "session_laps_remain_ex", "pace_mode", "pits_open",
  "player_car_idx", "cam_car_idx", "cam_camera_number", "cam_camera_state", "cam_group_number",
  "session_tick", "idx", "is_player",
]);

/// Sentinelas medidas, com a razão. Cada linha é uma investigação já fechada.
const SENTINELA = {
  tire_compound: "zerado em série de composto único (medido no MX-5)",
  f2_time: "vem populado e congelado em corrida de IA (medido)",
  replay_frame_num: "só anda em replay",
  replay_session_num: "só anda em replay",
  replay_session_time: "só anda em replay",
  is_replay_playing: "só muda em replay",
  team_incident_count: "sessão offline não tem equipe",
  tow_time: "só anda com o carro sendo guinchado",
  pit_repair_needed: "zero enquanto o carro não tem dano",
  pit_opt_repair_needed: "zero enquanto o carro não tem dano opcional",
  fast_repairs_used: "zero enquanto nenhum fast repair foi usado",
  pace_line: "-1 fora da volta de formação",
  pace_row: "-1 fora da volta de formação",
  pace_flags: "zero fora da volta de formação",
  is_in_garage: "falso a corrida inteira quando o piloto não volta à garagem",
};

/// Pilotagem. Constante aqui é grave, e só faz sentido perguntar com o jogador na pista.
const PILOTAGEM = new Set([
  "speed_ms", "speed_kmh", "rpm", "gear", "throttle", "brake", "clutch",
  "steering_angle_rad", "lap_dist_pct", "fuel_level", "lat_accel", "long_accel",
  "vert_accel", "yaw_rate", "pitch_rate", "roll_rate", "track_surface",
]);

/// Progresso da corrida. Constante importa quando houve voltas completas de verdade.
const CORRIDA = new Set([
  "lap", "lap_completed", "position", "class_position", "last_lap_time", "lap_current_time",
  "best_lap_time", "best_lap_num", "incident_count", "driver_incident_count", "session_flags",
  "on_pit_road", "player_on_pit_road", "on_track",
]);

/// A constância deste campo é alarme nesta captura?
///
/// Devolve `{ alarme, razao }`. `razao` preenchida é a explicação de por que ela NÃO é.
function julgarConstante(campo, r) {
  if (SENTINELA[campo]) return { alarme: false, razao: SENTINELA[campo] };
  if (AMBIENTE.has(campo)) return { alarme: false, razao: "ambiente fixo, o padrão das corridas" };
  if (SESSAO.has(campo)) return { alarme: false, razao: "parâmetro da sessão, constante por definição" };
  if (PILOTAGEM.has(campo)) {
    if (!r.esteveNaPista) return { alarme: false, razao: "o jogador não esteve na pista" };
    // O carro parado é a resposta inteira: acelerador, freio, marcha e volante ficam no
    // repouso porque ninguém pilotou, e não porque o canal deixou de ser lido. Acontece
    // sempre que a captura pega uma corrida assistida em vez de disputada.
    if (!r.pilotou) return { alarme: false, razao: "o carro nunca se moveu: ninguém pilotou" };
    return { alarme: true, razao: null };
  }
  if (CORRIDA.has(campo)) {
    // `position` vale 0 em treino e classificação, e isso é do SDK e está medido. Numa
    // CORRIDA a mesma constância é bug, e é por isso que o julgamento precisa saber o tipo
    // da sessão em vez de calar o campo para sempre numa tabela de sentinelas.
    if (!r.ehCorrida) return { alarme: false, razao: `sessão de ${r.tipoSessao ?? "tipo desconhecido"}` };
    // Sem cruzar a linha duas vezes, posição e tempo de volta não têm por que mudar.
    if (r.voltasCompletas < 2) return { alarme: false, razao: "menos de duas voltas completas" };
    return { alarme: true, razao: null };
  }
  // Campo que ninguém classificou. Sai listado, sem alarme: a resposta é classificá-lo aqui,
  // e não inventar um veredito sobre um campo cuja natureza este script não conhece.
  return { alarme: false, razao: "não classificado" };
}

// ─── Leitura ─────────────────────────────────────────────────────────────────

const CARS_HZ_PADRAO = 20;

/// Banda de taxa efetiva MEDIDA nas onze capturas do disco em 16/08/2026: 53,6 a 58,4 Hz nas
/// que passam de meio minuto.
///
/// O alvo nominal não é 60 e nunca foi: o amostrador dorme `SAMPLER_PERIOD_MS = 16` e ainda
/// faz o trabalho do tique, e o relógio do Windows tem resolução de ~15,6 ms. Um limiar posto
/// em 60 reprovaria toda captura saudável do acervo.
const HZ_PISO = 48;

/// Captura curta demais para a taxa média significar alguma coisa. Abaixo disto, meia dúzia de
/// quadros perdidos na abertura já derruba a média.
const CURTA_S = 30;

async function auditar(caminho) {
  const r = {
    caminho,
    nome: path.basename(caminho),
    header: null,
    vars: null,
    sessions: 0,
    ultimoYaml: null,
    history: null,
    frames: 0,
    framesComCars: 0,
    truncada: false,
    // Continuidade
    tPrimeiro: null,
    tUltimo: null,
    buracos: [],
    saltosAtras: 0,
    // Campos: primeiro valor visto e se ele mudou alguma vez
    primeiro: new Map(),
    mudou: new Set(),
    // O mesmo para o `cars[]`, num carro qualquer que não seja o jogador
    primeiroCar: new Map(),
    mudouCar: new Set(),
    esteveNaPista: false,
    maiorVolta: 0,
    voltasCompletas: 0,
    idxVistos: new Set(),
    sessionNums: new Set(),
    ehCorrida: false,
    pilotou: false,
    tipoSessao: null,
    // A janela em que `cars[]` de fato existiu. A taxa do array tem de ser medida sobre ela,
    // e não sobre a sessão inteira: o começo de toda captura é o carro na garagem com o
    // vetor vazio, e dividir pela sessão toda dava 11 Hz num alvo de 20 sem nada errado.
    tPrimeiroCars: null,
    tUltimoCars: null,
    // O relógio INDEPENDENTE. `session_tick` é o contador monotônico do sim e não reinicia
    // com a sessão, ao contrário do `session_time`. É a única testemunha de um trecho que a
    // captura deixou de escrever: ver `mudos` em `imprimir`.
    tickAnterior: null,
    mudos: [],
  };

  // Gunzip ESTRITO, de propósito, com o erro do fim tratado como informação.
  //
  // O trailer do gzip só é escrito pelo `stop()` do `race_capture`, que roda na borda de
  // desconexão do sim ou no fechamento da janela. Um arquivo sem trailer é uma captura que
  // não foi fechada, e essa é exatamente a pergunta que o auditor precisa responder: o bloco
  // `history` é anexado no MESMO ponto, então "sem history" e "sem trailer" são um fato só, e
  // reportá-los como dois alarmes contaria o mesmo problema duas vezes.
  //
  // `Z_SYNC_FLUSH` engoliria o erro e faria toda captura em andamento parecer fechada. O modo
  // estrito estoura no último bloco, e a essa altura tudo o que veio antes já foi entregue.
  const gunzip = zlib.createGunzip();
  const entrada = caminho.endsWith(".gz") ? fs.createReadStream(caminho).pipe(gunzip) : fs.createReadStream(caminho);
  const fluxo = readline.createInterface({ input: entrada, crlfDelay: Infinity });

  let tAnterior = null;
  try {
  for await (const linha of fluxo) {
    if (!linha) continue;
    let l;
    try {
      l = JSON.parse(linha);
    } catch {
      continue;
    }
    if (l.kind === "header") {
      r.header = l;
      continue;
    }
    if (l.kind === "vars") {
      r.vars = l.vars;
      continue;
    }
    if (l.kind === "session") {
      r.sessions += 1;
      r.ultimoYaml = l.yaml;
      continue;
    }
    if (l.kind === "history") {
      r.history = l.data;
      continue;
    }
    if (l.kind !== "frame" || !l.tele) continue;

    const tele = l.tele;
    r.frames += 1;
    const t = tele.session_time;
    if (r.tPrimeiro === null) r.tPrimeiro = t;
    r.tUltimo = t;
    if (tAnterior !== null) {
      const dt = t - tAnterior;
      if (dt < 0) r.saltosAtras += 1;
      // Um quadro a 60 Hz dura 16,7 ms. Meio segundo é folga larga: abaixo disso está o
      // jitter normal do agendador do Windows, e acima disso é quadro que não foi escrito.
      else if (dt > 0.5) r.buracos.push({ em: tAnterior, dt });
    }
    const tDoQuadroAnterior = tAnterior;
    tAnterior = t;

    // O TRECHO MUDO. `session_tick` anda a 60 Hz e não reinicia; `session_time` reinicia a
    // cada sessão. Quando o tique pula muito mais do que o tempo de sessão andou, o sim
    // rodou e a captura não escreveu — e como o `session_time` gravado continua monotônico,
    // o arquivo resultante parece perfeitamente contínuo. É o único jeito de ver o buraco.
    const tk = tele.session_tick;
    if (typeof tk === "number") {
      if (r.tickAnterior !== null && tDoQuadroAnterior !== null) {
        const dTick = tk - r.tickAnterior;
        const andouNoRelogio = t - tDoQuadroAnterior;
        const andouNoSim = dTick / 60;
        // Meio segundo de folga cobre o jitter do amostrador; o caso real mediu 594 s.
        if (dTick > 0 && andouNoSim - andouNoRelogio > 0.5) {
          r.mudos.push({
            tick: r.tickAnterior,
            segundos: andouNoSim - andouNoRelogio,
            sn: tele.session_num,
            t,
          });
        }
      }
      r.tickAnterior = tk;
    }

    if (typeof tele.session_num === "number") r.sessionNums.add(tele.session_num);
    if (tele.on_track) r.esteveNaPista = true;
    if (typeof tele.lap === "number" && tele.lap > r.maiorVolta) r.maiorVolta = tele.lap;
    if (typeof tele.lap_completed === "number" && tele.lap_completed > r.voltasCompletas) {
      r.voltasCompletas = tele.lap_completed;
    }

    for (const [campo, valor] of Object.entries(tele)) {
      if (campo === "cars") continue;
      if (!r.primeiro.has(campo)) r.primeiro.set(campo, valor);
      else if (!r.mudou.has(campo) && r.primeiro.get(campo) !== valor) r.mudou.add(campo);
    }

    if (Array.isArray(tele.cars) && tele.cars.length > 0) {
      r.framesComCars += 1;
      if (r.tPrimeiroCars === null) r.tPrimeiroCars = t;
      r.tUltimoCars = t;
      // A pergunta é "este canal chegou a ser lido?", e a resposta é sim assim que QUALQUER
      // carro mostrar um valor diferente. Por isso a comparação atravessa os carros em vez
      // de seguir um só.
      //
      // A primeira versão seguia a linha do próprio jogador dentro de `cars[]`, e essa é a
      // pior escolha possível: é a linha com os quirks medidos (`position` vale 0 na
      // classificação, e o jogador some do vetor quando sai da pista). Ela produzia cinco
      // alarmes falsos por captura, todos sobre canais que a IA preenchia normalmente.
      for (const c of tele.cars) {
        r.idxVistos.add(c.idx);
        for (const [campo, valor] of Object.entries(c)) {
          if (!r.primeiroCar.has(campo)) r.primeiroCar.set(campo, valor);
          else if (!r.mudouCar.has(campo) && r.primeiroCar.get(campo) !== valor) r.mudouCar.add(campo);
        }
      }
    }
  }
  } catch (e) {
    // Fluxo truncado é o esperado numa captura ainda aberta. Qualquer outro erro é problema
    // de leitura de verdade e não pode passar por captura em andamento.
    if (e?.code === "Z_BUF_ERROR" || /unexpected end of file/i.test(e?.message ?? "")) r.truncada = true;
    else throw e;
  }
  return r;
}

// ─── Impressão ───────────────────────────────────────────────────────────────

function seg(t) {
  if (t === null || t === undefined) return "?";
  const m = Math.floor(t / 60);
  return `${m}:${(t % 60).toFixed(1).padStart(4, "0")}`;
}

function doYaml(yaml, chave) {
  const m = new RegExp(`${chave}:\\s*(.+)`).exec(yaml || "");
  return m ? m[1].trim() : null;
}

/// O `SessionType` de uma sessão do fim de semana ("Race", "Open Qualify", "Practice").
///
/// Ancorado no `- SessionNum:` que abre cada item da lista, e não no nome solto: o YAML traz
/// `CurrentSessionNum: 0` logo acima, e um casamento por substring pegaria esse primeiro e
/// devolveria o tipo da sessão errada.
function tipoDaSessao(yaml, num) {
  if (!yaml) return null;
  const re = new RegExp(`\\n\\s*-\\s*SessionNum:\\s*${num}\\b[\\s\\S]*?SessionType:\\s*(.+)`);
  const m = re.exec(yaml);
  return m ? m[1].trim() : null;
}

function imprimir(r, opcoes, curados, porCampo) {
  const problemas = [];
  const duracao = r.tUltimo !== null && r.tPrimeiro !== null ? r.tUltimo - r.tPrimeiro : 0;

  // O tipo das sessões que esta captura cobriu. É o que separa `position` constante em
  // classificação (quirk do SDK, medido) de `position` constante em corrida (bug).
  const tipos = [...r.sessionNums].map((n) => tipoDaSessao(r.ultimoYaml, n)).filter(Boolean);
  r.tipoSessao = tipos.join(", ") || null;
  r.ehCorrida = tipos.some((t) => /race/i.test(t));

  console.log(`\n═══ ${r.nome} ═══`);

  // 1. ESTRUTURA
  console.log("\n── Estrutura");
  console.log(`  Formato .......... versão ${r.header?.version ?? "?"}, cars a ${r.header?.cars_hz ?? "?"} Hz`);
  console.log(`  Pista ............ ${doYaml(r.ultimoYaml, "TrackDisplayName") ?? "?"}`);
  console.log(`  Sessão ........... ${r.tipoSessao ?? "tipo não identificado no YAML"}`);
  console.log(`  Quadros .......... ${r.frames} em ${seg(duracao)} de sessão`);
  console.log(`  Versões do YAML .. ${r.sessions}`);

  // A CAPTURA DEGENERADA. Um punhado de quadros é o sim tendo caído logo depois de conectar,
  // e cobrar dela inventário, YAML e histórico produziria uma lista de alarmes sobre um
  // arquivo que só tem um fato a contar: ele não chegou a gravar nada.
  if (r.frames < 60) {
    console.log(`\n  Captura vazia: ${r.frames} quadro(s). O sim caiu ou fechou logo após conectar.`);
    console.log("\n── Veredito");
    console.log("  Nada a auditar. Não há gravação aqui.");
    return 0;
  }

  if (!r.header) problemas.push("sem cabeçalho: o arquivo não diz de que versão do formato ele fala");
  // O inventário nasceu na versão 3 do formato (ver `FORMAT_VERSION` em `race_capture.rs`).
  // Cobrá-lo de um arquivo da versão 1 é cobrar um campo que não existia quando ele foi
  // escrito, e isso reprova o passado em vez de auditar o presente.
  const versao = r.header?.version ?? 0;
  if (!r.vars) {
    if (versao >= 3) problemas.push("SEM INVENTÁRIO de canais: não dá para saber o que o sim publicava nesta build");
    else console.log(`  Inventário ....... não existe no formato v${versao} (nasceu na v3)`);
  } else {
    console.log(`  Inventário ....... ${r.vars.length} canais publicados pelo sim`);
  }
  if (r.sessions === 0) problemas.push("nenhum YAML de sessão: a captura não sabe que corrida é esta");

  // O `history` é anexado no MESMO ponto em que o gzip ganha o trailer (ver `auditar`). Sem
  // trailer, a captura simplesmente não chegou lá, e cobrar o histórico dela seria cobrar um
  // fim que ainda não aconteceu. COM trailer e sem histórico, aí sim passou pelo fechamento
  // sem anexar o dado derivado, e isso é perda de verdade.
  console.log(`  Fechamento ....... ${r.truncada ? "em aberto (sem trailer do gzip)" : "completo"}`);
  if (r.truncada) {
    console.log("     A captura não passou pelo `stop()`: o sim e o app seguem abertos, ou");
    console.log("     foram encerrados à força. O histórico é anexado nesse mesmo ponto.");
  } else if (!r.history) {
    problemas.push("captura FECHADA sem o bloco `history`: a corrida não virou dado derivado");
  }
  // O último YAML é o que traz `ResultsPositions`. Sem ele a corrida foi gravada sem desfecho.
  if (r.ultimoYaml && !/ResultsPositions/.test(r.ultimoYaml)) {
    problemas.push("o último YAML não tem `ResultsPositions`: a captura não pegou o resultado publicado");
  }

  // 2. CONTINUIDADE
  console.log("\n── Continuidade");
  const hz = duracao > 0 ? r.frames / duracao : 0;
  const janelaCars =
    r.tUltimoCars !== null && r.tPrimeiroCars !== null ? r.tUltimoCars - r.tPrimeiroCars : 0;
  const hzCars = janelaCars > 0 ? r.framesComCars / janelaCars : 0;
  const alvoCars = r.header?.cars_hz ?? CARS_HZ_PADRAO;
  console.log(`  Taxa efetiva ..... ${hz.toFixed(1)} Hz (banda medida do acervo: 53 a 58)`);
  // O TETO do `cars[]` é aritmético, e fica abaixo do alvo. O escritor grava o array quando
  // passaram >= 1/alvo segundos desde o último, e só pode fazer isso num quadro: com o
  // amostrador a ~56 Hz e o alvo em 20, o primeiro quadro a cruzar os 50 ms é o terceiro, o
  // que dá 18,7 Hz. Com o jitter empurrando para o quarto quadro, cai para 14. Comparar o
  // medido contra os 20 nominais reprovaria toda captura saudável do acervo.
  const tetoCars = hz > 0 ? hz / Math.ceil(hz / alvoCars) : alvoCars;
  console.log(
    `  Taxa do cars[] ... ${hzCars.toFixed(1)} Hz sobre ${seg(janelaCars)} com carros ` +
      `(alvo ${alvoCars}, teto real ${tetoCars.toFixed(1)})`,
  );
  console.log(`  Saltos para trás . ${r.saltosAtras} (troca de sessão ou rebobinada)`);
  const perdido = r.buracos.reduce((s, b) => s + b.dt, 0);
  console.log(`  Buracos > 0,5 s .. ${r.buracos.length}, somando ${perdido.toFixed(1)} s`);
  for (const b of [...r.buracos].sort((a, b) => b.dt - a.dt).slice(0, 5)) {
    console.log(`     ${seg(b.em)} ficou ${b.dt.toFixed(1)} s sem quadro`);
  }
  // O TRECHO MUDO, antes de qualquer outra coisa: é o defeito que faz todas as outras
  // medidas desta seção mentirem. Um arquivo que perdeu dez minutos no meio ainda mostra
  // `session_time` perfeitamente contínuo, porque o escritor só admite valor crescente.
  const mudo = r.mudos.reduce((s, m) => s + m.segundos, 0);
  if (r.mudos.length > 0) {
    console.log(`  TRECHOS MUDOS .... ${r.mudos.length}, somando ${mudo.toFixed(0)} s de sim sem quadro`);
    for (const m of [...r.mudos].sort((a, b) => b.segundos - a.segundos).slice(0, 5)) {
      console.log(`     ${m.segundos.toFixed(0)} s no tique ${m.tick}, reaparecendo em sn ${m.sn} t ${m.t}`);
    }
    problemas.push(
      `${mudo.toFixed(0)} s de sim rodaram SEM QUADRO NENHUM (${r.mudos.length} trecho(s)): o ` +
        `\`session_tick\` andou e o \`session_time\` não. O arquivo parece contínuo e não é.`,
    );
  }

  const curta = duracao < CURTA_S;
  if (curta) {
    console.log(`  (captura de menos de ${CURTA_S} s: a taxa média não é conclusiva)`);
  } else if (hz > 0 && hz < HZ_PISO) {
    // Abaixo do piso o amostrador está perdendo um quadro em cada cinco, e toda duração
    // medida na captura passa a ter erro maior que a janela em que o spotter decide.
    problemas.push(`taxa efetiva de ${hz.toFixed(1)} Hz, abaixo da banda de ${HZ_PISO} a 58 medida no acervo`);
  }
  // Metade do alvo é o piso: abaixo disso não é mais quantização, é o array deixando de ser
  // escrito em quadros que o cruzaram.
  if (!curta && janelaCars > CURTA_S && hzCars > 0 && hzCars < alvoCars * 0.5) {
    problemas.push(
      `cars[] a ${hzCars.toFixed(1)} Hz com teto real de ${tetoCars.toFixed(1)}: a vizinhança foi gravada rala`,
    );
  }
  if (!curta && perdido > duracao * 0.05) {
    problemas.push(`${perdido.toFixed(0)} s sem quadro nenhum, ${((perdido / duracao) * 100).toFixed(0)}% da sessão`);
  }

  // 3. CANAIS MORTOS
  console.log("\n── Canais");
  if (r.vars && curados) {
    const publicados = new Set(r.vars.map((v) => v.name ?? v.nome));
    const ausentes = curados.filter((c) => !publicados.has(c));
    if (ausentes.length === 0) {
      console.log(`  Curados ausentes . nenhum: os ${curados.length} canais existem nesta build`);
    } else {
      console.log(`  Curados ausentes . ${ausentes.length}`);
      for (const c of ausentes) console.log(`     ${c}`);
      problemas.push(`${ausentes.length} canais curados NÃO existem nesta build do sim: ${ausentes.join(", ")}`);
    }
  } else {
    console.log("  Curados ausentes . não conferido (falta o inventário ou a lista do Rust)");
  }

  // O campo que nunca mudou, julgado pelo grupo a que pertence. Ver `julgarConstante`.
  const linhaDe = (campo, valor, prefixo) => {
    const canais = porCampo.get(campo);
    const de = canais ? ` ← ${canais.join(", ")}` : "";
    return `     ${(prefixo + campo).padEnd(30)} = ${JSON.stringify(valor)}${de}`;
  };

  const relatar = (rotulo, primeiro, mudou, prefixo) => {
    const constantes = [...primeiro.keys()].filter((c) => !mudou.has(c));
    const julgados = constantes.map((c) => ({ campo: c, ...julgarConstante(c, r) }));
    const alarmes = julgados.filter((j) => j.alarme);
    console.log(`  ${rotulo} ${constantes.length} constantes de ${primeiro.size}, ${alarmes.length} a explicar`);
    for (const j of alarmes) console.log(linhaDe(j.campo, primeiro.get(j.campo), prefixo));
    if (opcoes.constantes) {
      for (const j of julgados.filter((x) => !x.alarme)) {
        console.log(`${linhaDe(j.campo, primeiro.get(j.campo), prefixo)}   (${j.razao})`);
      }
    }
    return alarmes;
  };

  if (!r.esteveNaPista) {
    console.log("     O jogador nunca esteve na pista nesta captura: os campos de pilotagem");
    console.log("     são legitimamente constantes e ficam fora do julgamento.");
  }
  // OS CONTROLES. Acelerador, freio e volante constantes ao mesmo tempo não são três
  // defeitos e nem um: são a assinatura de uma corrida ASSISTIDA, com o jogador parado no
  // grid enquanto a IA corre. Medido em `race_1786410466_1`, onde os três saíram junto de
  // `brake = 1` e `gear = 0`, que é o carro parado com o freio pisado.
  //
  // Sem este caso especial o auditor cospe sete linhas sobre um fato só, e nenhuma delas diz
  // o fato. A velocidade não serve de porteiro aqui porque ela varia com o carro sendo
  // rebocado ou empurrado pelo sim.
  const controles = ["throttle", "brake", "steering_angle_rad"];
  r.pilotou = controles.some((c) => r.mudou.has(c));
  if (r.esteveNaPista && !r.pilotou) {
    console.log("     O jogador não tocou nos controles: acelerador, freio e volante ficaram");
    console.log("     parados a captura inteira. É corrida assistida, não gravação defeituosa.");
  }
  const alarmes = relatar("Campos do jogador ", r.primeiro, r.mudou, "");
  if (alarmes.length > 0) {
    problemas.push(
      `${alarmes.length} campos do jogador constantes sem explicação: ${alarmes.map((a) => a.campo).join(", ")}`,
    );
  }
  if (r.framesComCars > 0) {
    const alarmesCar = relatar("Campos por carro  ", r.primeiroCar, r.mudouCar, "cars[].");
    if (alarmesCar.length > 0) {
      problemas.push(
        `${alarmesCar.length} campos por carro constantes sem explicação: ${alarmesCar.map((a) => a.campo).join(", ")}`,
      );
    }
  }
  if (!opcoes.constantes) {
    console.log("     (--constantes lista também os constantes já explicados, com a razão)");
  }

  // 4. O DERIVADO CONTRA O CRU
  console.log("\n── O derivado contra o cru");
  if (!r.history) {
    console.log("  Não há bloco `history` para comparar.");
  } else {
    const h = r.history;
    const voltasJogador = h.player_laps?.length ?? 0;
    console.log(`  Voltas do jogador  ${voltasJogador} no histórico, maior \`lap\` nos quadros: ${r.maiorVolta}`);
    console.log(`  Trace do líder ... ${h.laps?.length ?? 0} voltas`);
    console.log(`  Carros no cars[] . ${r.idxVistos.size} vistos, ${h.cars_meta?.length ?? 0} no resumo por carro`);
    console.log(`  Voltas por carro . ${h.car_laps?.length ?? 0} registros`);
    console.log(`  Quali ............ ${h.qualy_laps?.length ?? 0} voltas`);
    console.log(`  Desfecho ......... ${h.finished ? h.outcome || "encerrada" : "em andamento"}`);
    console.log(`  Identidade ....... track ${h.track_id ?? 0}, subsession ${h.subsession_id ?? 0}`);

    // A volta em andamento não entra em `player_laps`, então o histórico fica UMA atrás da
    // maior volta vista. Diferença maior que isso é volta que se perdeu no caminho.
    if (r.maiorVolta > 1 && voltasJogador < r.maiorVolta - 1) {
      problemas.push(
        `o histórico tem ${voltasJogador} voltas do jogador e os quadros chegaram à volta ${r.maiorVolta}`,
      );
    }
    if (r.idxVistos.size > 0 && (h.cars_meta?.length ?? 0) === 0) {
      problemas.push(`${r.idxVistos.size} carros apareceram no cars[] e o resumo por carro veio vazio`);
    }
    // `track_id` e `subsession_id` entraram no histórico depois; num arquivo antigo eles
    // vêm ausentes, e ausente não é zerado.
    if (versao >= 3) {
      if (!h.track_id) problemas.push("`track_id` zerado no histórico: a corrida não sabe em que pista foi");
      if (!h.subsession_id) problemas.push("`subsession_id` zerado: o evento não tem identidade única");
    }
    if (r.esteveNaPista && (h.car_laps?.length ?? 0) === 0) {
      problemas.push("`car_laps` vazio: a base da adaptação não foi gravada");
    }
  }

  // VEREDITO
  console.log("\n── Veredito");
  if (problemas.length === 0) {
    console.log("  Nada a apontar. A gravação está inteira.");
  } else {
    for (const p of problemas) console.log(`  ⚠ ${p}`);
  }
  return problemas.length;
}

// ─── Linha de comando ────────────────────────────────────────────────────────

function listarCapturas() {
  const pasta = pastaDasCapturas();
  if (!fs.existsSync(pasta)) return [];
  return fs
    .readdirSync(pasta)
    .filter((n) => n.startsWith("race_") && (n.endsWith(".jsonl.gz") || n.endsWith(".jsonl")))
    .map((n) => {
      const c = path.join(pasta, n);
      return { nome: n, caminho: c, mtime: fs.statSync(c).mtimeMs, bytes: fs.statSync(c).size };
    })
    .sort((a, b) => b.mtime - a.mtime);
}

async function principal() {
  const argv = process.argv.slice(2);
  const opcoes = {
    lista: argv.includes("--lista"),
    todas: argv.includes("--todas"),
    constantes: argv.includes("--constantes"),
  };
  const alvo = argv.find((a) => !a.startsWith("--"));

  const capturas = listarCapturas();
  if (opcoes.lista) {
    console.log(`Capturas em ${pastaDasCapturas()}`);
    for (const c of capturas) {
      console.log(`  ${c.nome.padEnd(30)} ${(c.bytes / 1024 / 1024).toFixed(1)} MB   ${new Date(c.mtime).toLocaleString()}`);
    }
    return;
  }

  const curados = canaisCurados();
  const porCampo = mapaCanalCampo();
  if (!curados) {
    console.log("⚠ Não consegui ler CANAIS_CURADOS de `canais.rs`. A conferência de canais fica de fora.");
  }

  const alvos = opcoes.todas ? capturas.map((c) => c.caminho) : [alvo || capturas[0]?.caminho].filter(Boolean);
  if (alvos.length === 0) {
    console.error(`Nenhuma captura em ${pastaDasCapturas()}.`);
    process.exitCode = 1;
    return;
  }

  let comProblema = 0;
  for (const caminho of alvos) {
    const r = await auditar(caminho);
    if (imprimir(r, opcoes, curados, porCampo) > 0) comProblema += 1;
  }
  if (alvos.length > 1) {
    console.log(`\n═══ ${comProblema} de ${alvos.length} capturas com algo a apontar ═══`);
  }
}

principal().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
