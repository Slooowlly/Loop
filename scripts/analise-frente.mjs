#!/usr/bin/env node
// Valida, contra CORRIDA JÁ GRAVADA, se dá para saber o que está à frente.
//
// A alternativa era um painel de depuração ao vivo e uma sessão nova de pista. Não
// precisa: as capturas em `debug/race_captures` já trazem o array `cars[]` inteiro e
// o `TrackLength` na string de sessão. O que aqui se responde é a lista de cenários
// da fase 5 — distância, wrap na linha, superfície, parado, box, fora do mundo —
// sem ligar o sim.

import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import readline from "node:readline";

const DIR =
  process.argv[2] ||
  path.join(process.env.APPDATA || "", "com.loop.app", "debug", "race_captures");

// irsdk_TrkLoc
const SUP = { "-1": "ForaDoMundo", 0: "ForaDaPista", 1: "NaCaixa", 2: "EntrandoBox", 3: "NaPista" };

// irsdk_TrkSurf — o QUE o carro está pisando. É isto que separa uma escapada na área
// asfaltada (inofensiva, acontece toda volta) de um carro enterrado na brita.
const MAT = {
  "-1": "ausente", 0: "NotInWorld", 1: "asfalto1", 2: "asfalto2", 3: "asfalto3",
  4: "asfalto4", 5: "concreto", 6: "pedra", 7: "terra", 8: "grama1", 9: "grama2",
  10: "grama3", 11: "grama4", 12: "areia", 13: "brita1", 14: "brita2", 15: "grama5",
  16: "brita3", 17: "brita4", 18: "brita5", 19: "brita6", 20: "brita7", 21: "brita8",
};

// irsdk_SessionState
const ESTADO = {
  0: "Invalido", 1: "NoCarro", 2: "Aquecimento", 3: "Formacao",
  4: "Correndo", 5: "Bandeirada", 6: "Desaquecimento",
};

/// Os canais de que o detector de obstáculo depende. O inventário diz se existem.
const CANAIS_CRITICOS = [
  "CarIdxLapDistPct", "CarIdxEstTime", "CarIdxTrackSurface", "CarIdxTrackSurfaceMaterial",
  "CarIdxOnPitRoad", "CarIdxRPM", "CarIdxGear", "CarIdxSessionFlags", "CarIdxPaceFlags",
  "SessionState", "SessionTick", "PaceMode", "SessionFlags",
];

/** Janela em que um carro à frente ainda é assunto do spotter (m). */
const JANELA_M = 300;
/** Abaixo disto o carro é "parado" (km/h). */
const PARADO_KMH = 5;

/// Linhas do arquivo. Vários `.gz` estão TRUNCADOS — o app foi fechado no meio da
/// gravação —, e o gunzip normal aborta neles. `Z_SYNC_FLUSH` entrega o que deu para
/// descomprimir em vez de nada, que é exatamente o que se quer de um log.
async function* linhas(arquivo) {
  if (arquivo.endsWith(".gz")) {
    const bruto = zlib.gunzipSync(fs.readFileSync(arquivo), {
      finishFlush: zlib.constants.Z_SYNC_FLUSH,
    });
    yield* bruto.toString("utf8").split(/\r?\n/);
    return;
  }
  yield* readline.createInterface({
    input: fs.createReadStream(arquivo),
    crlfDelay: Infinity,
  });
}

/** Distância até `alvo`, medida PARA A FRENTE ao longo da pista. Sempre em [0, volta). */
function metrosAFrente(pctJogador, pctAlvo, comprimentoM) {
  let d = pctAlvo - pctJogador;
  if (d < 0) d += 1;
  return d * comprimentoM;
}

async function analisar(arquivo) {
  const r = {
    arquivo: path.basename(arquivo),
    versao: null,
    pista: null,
    comprimentoM: null,
    tipoSessao: null,
    frames: 0,
    framesComCars: 0,
    framesNaPista: 0,
    hz: null,
    superficies: new Map(),
    f2Nulo: 0,
    f2Total: 0,
    estCrescente: 0,
    estTotal: 0,
    // Cenários
    saltos: [], // pulo de distância do carro da frente que a fórmula não explica
    cruzamentos: 0, // frames em que o jogador cruzou a linha
    paradosNaPista: new Map(), // idx -> amostras
    foraDaPista: new Map(),
    naCaixa: new Map(),
    foraDoMundo: new Map(),
    maiorSaltoM: 0,
    resolucaoPct: null,
    velocidades: [], // (derivada, real) do próprio jogador — a prova da derivação
    vars: null, // inventário do SDK (formato 3+)
    yamls: 0,
    motivosDeSaida: null,
    materiais: new Map(),
    estados: new Map(),
    episodios: [], // cada saída de pista / parada, com começo, fim e desfecho
  };
  /** Episódios abertos: idx -> { tipo, t0, material, estado, pctInicial } */
  const abertos = new Map();

  let anterior = null; // { t, pctJogador, porCarro: Map(idx -> pct) }
  let alvoAnterior = null; // { idx, dist }
  let primeiroT = null;
  let ultimoT = null;

  for await (const linha of linhas(arquivo)) {
    if (!linha) continue;
    let o;
    try {
      o = JSON.parse(linha);
    } catch {
      continue; // última linha de um .gz truncado
    }

    if (o.kind === "header") {
      r.versao = o.version;
      r.resolucaoPct = o.casas ? 10 ** -o.casas : null;
      continue;
    }
    if (o.kind === "vars") {
      r.vars = new Map(o.vars.map((v) => [v.nome, v]));
      continue;
    }
    if (o.kind === "session") {
      // A partir do formato 3 o YAML é regravado a cada mudança. A pista vem da
      // primeira cópia; os RESULTADOS (com o motivo de abandono) só existem na última.
      r.yamls += 1;
      const m = /TrackLength:\s*([\d.]+)\s*km/.exec(o.yaml);
      if (m) r.comprimentoM = parseFloat(m[1]) * 1000;
      const n = /TrackDisplayName:\s*(.+)/.exec(o.yaml);
      if (n) r.pista = n[1].trim();
      const s = /SessionType:\s*(.+)/.exec(o.yaml);
      if (s) r.tipoSessao = s[1].trim();
      const saidas = [...o.yaml.matchAll(/ReasonOutStr:\s*(.+)/g)].map((x) => x[1].trim());
      if (saidas.length) r.motivosDeSaida = saidas;
      continue;
    }
    if (o.kind !== "frame") continue;

    const t = o.tele;
    r.frames += 1;
    if (primeiroT === null) primeiroT = t.session_time;
    ultimoT = t.session_time;
    if (t.on_track) r.framesNaPista += 1;
    if (!Array.isArray(t.cars) || t.cars.length === 0) continue;
    r.framesComCars += 1;

    const idxJogador = t.player_car_idx ?? 0;
    const pctJogador = t.lap_dist_pct;

    const porCarro = new Map();
    for (const c of t.cars) {
      porCarro.set(c.idx, c.lap_dist_pct);
      const nome = SUP[String(c.track_surface)] ?? `?${c.track_surface}`;
      r.superficies.set(nome, (r.superficies.get(nome) || 0) + 1);
      r.f2Total += 1;
      if (c.f2_time === 0) r.f2Nulo += 1;
    }

    // est_time deveria crescer junto com lap_dist_pct dentro do mesmo frame.
    const ord = t.cars
      .filter((c) => c.est_time > 0)
      .sort((a, b) => a.lap_dist_pct - b.lap_dist_pct);
    for (let i = 1; i < ord.length; i += 1) {
      r.estTotal += 1;
      if (ord[i].est_time >= ord[i - 1].est_time) r.estCrescente += 1;
    }

    // ── Velocidade derivada por Δpct, e a mesma coisa medida de verdade ────────
    if (anterior && r.comprimentoM) {
      const dt = t.session_time - anterior.t;
      if (dt > 0.001 && dt < 1.0) {
        if (t.on_track && !t.player_on_pit_road) {
          let dp = pctJogador - anterior.pctJogador;
          if (dp < -0.5) dp += 1; // cruzou a linha
          const derivada = (dp * r.comprimentoM) / dt * 3.6;
          const real = t.speed_kmh ?? t.speed_ms * 3.6;
          if (real > 1) r.velocidades.push([derivada, real]);
        }

        // Cada carro: em que estado está, e se isso abre ou fecha um EPISÓDIO.
        for (const c of t.cars) {
          if (c.idx === idxJogador) continue;
          const antes = anterior.porCarro.get(c.idx);
          if (antes === undefined) continue;
          let dp = c.lap_dist_pct - antes;
          if (dp < -0.5) dp += 1;
          const kmh = ((dp * r.comprimentoM) / dt) * 3.6;
          const sup = c.track_surface;
          const conta = (mapa) => mapa.set(c.idx, (mapa.get(c.idx) || 0) + 1);

          let tipo = null; // null = nada de anormal
          let desfecho = null;
          if (sup === -1) {
            conta(r.foraDoMundo);
            desfecho = "sumiu do mundo";
          } else if (sup === 1 || sup === 2 || c.on_pit_road) {
            conta(r.naCaixa);
            desfecho = "foi para o box";
          } else if (sup === 0) {
            conta(r.foraDaPista);
            tipo = "fora da pista";
          } else if (Math.abs(kmh) < PARADO_KMH) {
            conta(r.paradosNaPista);
            tipo = "parado";
          } else {
            desfecho = "retomou";
          }

          if (c.track_surface_material !== undefined) {
            const nome = MAT[String(c.track_surface_material)] ?? `?${c.track_surface_material}`;
            r.materiais.set(nome, (r.materiais.get(nome) || 0) + 1);
          }

          const aberto = abertos.get(c.idx);
          if (tipo) {
            if (!aberto) {
              abertos.set(c.idx, {
                idx: c.idx,
                tipo,
                t0: t.session_time,
                estado: t.session_state,
                materiais: new Set(),
                minKmh: kmh,
              });
            } else {
              if (aberto.tipo !== tipo) aberto.tipo = `${aberto.tipo} + ${tipo}`;
              aberto.minKmh = Math.min(aberto.minKmh, Math.abs(kmh));
            }
            const atual = abertos.get(c.idx);
            if (c.track_surface_material > 0) {
              atual.materiais.add(MAT[String(c.track_surface_material)] ?? c.track_surface_material);
            }
          } else if (aberto) {
            abertos.delete(c.idx);
            r.episodios.push({
              ...aberto,
              materiais: [...aberto.materiais],
              t1: t.session_time,
              duracao: t.session_time - aberto.t0,
              desfecho: desfecho || "retomou",
            });
          }
        }
      }
    }

    // ── O carro mais próximo à frente, e se a distância dele é estável ─────────
    if (r.comprimentoM) {
      let melhor = null;
      for (const c of t.cars) {
        if (c.idx === idxJogador) continue;
        if (c.track_surface === -1 || c.on_pit_road) continue;
        const d = metrosAFrente(pctJogador, c.lap_dist_pct, r.comprimentoM);
        if (d > 0.5 && d < JANELA_M && (!melhor || d < melhor.dist)) melhor = { idx: c.idx, dist: d };
      }
      if (melhor && alvoAnterior && melhor.idx === alvoAnterior.idx && anterior) {
        const dt = t.session_time - anterior.t;
        const salto = Math.abs(melhor.dist - alvoAnterior.dist);
        // Ninguém se afasta mais que a soma das duas velocidades no intervalo.
        const teto = 120 * dt + 3; // 120 m/s de aproximação relativa + folga
        if (salto > teto) {
          r.saltos.push({ t: t.session_time, idx: melhor.idx, salto, dt });
          r.maiorSaltoM = Math.max(r.maiorSaltoM, salto);
        }
      }
      alvoAnterior = melhor;
    }

    const nomeEstado = ESTADO[String(t.session_state)] ?? `?${t.session_state}`;
    r.estados.set(nomeEstado, (r.estados.get(nomeEstado) || 0) + 1);

    if (anterior && pctJogador < anterior.pctJogador - 0.5) r.cruzamentos += 1;
    anterior = { t: t.session_time, pctJogador, porCarro };
  }

  // Episódios que a captura terminou sem fechar contam como "ainda lá no fim".
  for (const a of abertos.values()) {
    r.episodios.push({
      ...a,
      materiais: [...a.materiais],
      t1: ultimoT,
      duracao: ultimoT - a.t0,
      desfecho: "ainda lá quando a captura acabou",
    });
  }

  const dur = ultimoT - primeiroT;
  r.hz = dur > 0 ? r.frames / dur : null;
  r.duracaoS = dur;
  return r;
}

const num = (x, c = 1) => (x === null || x === undefined ? "—" : x.toFixed(c));

// Aceita um arquivo só ou a pasta inteira — na pasta, uma captura de 250 MB de
// julho arrasta a rodada toda quando o que interessa é a última.
const arquivos = fs.statSync(DIR).isDirectory()
  ? fs
      .readdirSync(DIR)
      .filter((f) => f.endsWith(".jsonl") || f.endsWith(".jsonl.gz"))
      .map((f) => path.join(DIR, f))
  : [DIR];

for (const a of arquivos) {
  const r = await analisar(a);
  const mb = (fs.statSync(a).size / 1048576).toFixed(1);
  console.log(`\n${"═".repeat(78)}\n${r.arquivo}  (v${r.versao}, ${mb} MB)`);
  console.log(
    `  ${r.pista || "pista não registrada"} · ${num(r.comprimentoM, 0)} m · ${r.tipoSessao || "?"} · ` +
      `${num(r.duracaoS, 0)} s`,
  );
  console.log(
    `  ${r.frames} frames (${num(r.hz)} Hz), ${r.framesComCars} com cars[], ${r.framesNaPista} com o jogador na pista`,
  );

  const sup = [...r.superficies.entries()].sort((a, b) => b[1] - a[1]);
  console.log(`  superfícies: ${sup.map(([k, v]) => `${k}=${v}`).join("  ")}`);
  console.log(
    `  f2_time zerado: ${r.f2Total ? ((100 * r.f2Nulo) / r.f2Total).toFixed(0) : "—"}%   ` +
      `est_time na ordem de lap_dist_pct: ${r.estTotal ? ((100 * r.estCrescente) / r.estTotal).toFixed(1) : "—"}%`,
  );

  if (r.velocidades.length > 20) {
    const erros = r.velocidades.map(([d, v]) => Math.abs(d - v));
    erros.sort((a, b) => a - b);
    const p = (q) => erros[Math.floor(q * (erros.length - 1))];
    console.log(
      `  velocidade derivada vs. real (${r.velocidades.length} amostras): ` +
        `mediana ${num(p(0.5))} km/h de erro, p95 ${num(p(0.95))}, pior ${num(p(1))}`,
    );
  }

  console.log(`  cruzamentos da linha pelo jogador: ${r.cruzamentos}`);
  console.log(
    `  saltos inexplicados na distância do carro da frente: ${r.saltos.length}` +
      (r.saltos.length ? ` (maior ${num(r.maiorSaltoM)} m)` : "  ✔"),
  );
  for (const s of r.saltos.slice(0, 3)) {
    console.log(`      t=${num(s.t)}s idx=${s.idx} salto ${num(s.salto)} m em ${num(s.dt, 3)} s`);
  }

  const resumo = (mapa) =>
    mapa.size ? [...mapa.entries()].sort((a, b) => b[1] - a[1]).map(([i, n]) => `#${i}:${n}`).join(" ") : "nenhum";
  console.log(`  PARADOS na pista: ${resumo(r.paradosNaPista)}`);
  console.log(`  FORA da pista:    ${resumo(r.foraDaPista)}`);
  console.log(`  no BOX:           ${resumo(r.naCaixa)}`);
  console.log(`  fora do MUNDO:    ${resumo(r.foraDoMundo)}`);

  const est = [...r.estados.entries()].sort((a, b) => b[1] - a[1]);
  console.log(`  estados de sessão: ${est.map(([k, v]) => `${k}=${v}`).join("  ")}`);

  if (r.materiais.size) {
    const mat = [...r.materiais.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8);
    console.log(`  materiais pisados: ${mat.map(([k, v]) => `${k}=${v}`).join("  ")}`);
  }
  if (r.yamls > 1) console.log(`  cópias do YAML da sessão: ${r.yamls}`);
  if (r.motivosDeSaida) {
    const c = new Map();
    for (const m of r.motivosDeSaida) c.set(m, (c.get(m) || 0) + 1);
    console.log(`  motivos de saída: ${[...c.entries()].map(([k, v]) => `${k}×${v}`).join("  ")}`);
  }

  // ── O inventário do SDK: o que existe nesta build, não o que a gente supõe ──
  if (r.vars) {
    const faltando = CANAIS_CRITICOS.filter((n) => !r.vars.has(n));
    console.log(
      `  inventário do SDK: ${r.vars.size} variáveis` +
        (faltando.length ? `  ⚠ AUSENTES: ${faltando.join(", ")}` : "  ✔ todos os canais críticos presentes"),
    );
  }

  // ── Os episódios: a resposta às perguntas de quando/quanto/onde/desfecho ──
  if (r.episodios.length) {
    // Antes da largada o grid inteiro está imóvel na pista. Isso não é obstáculo, e
    // separar aqui é o que impede a estatística de ser dominada por não-eventos.
    const emCorrida = r.episodios.filter((e) => e.estado === 4);
    const fora = r.episodios.length - emCorrida.length;
    console.log(`  EPISÓDIOS: ${r.episodios.length} (${emCorrida.length} com a corrida em andamento, ${fora} fora dela)`);
    for (const e of emCorrida.sort((a, b) => b.duracao - a.duracao).slice(0, 12)) {
      console.log(
        `      #${String(e.idx).padStart(2)} ${e.tipo.padEnd(22)} ` +
          `t=${num(e.t0)}s por ${num(e.duracao, 2)}s → ${e.desfecho}` +
          (e.materiais.length ? `  [${e.materiais.join(", ")}]` : ""),
      );
    }
  }
}
