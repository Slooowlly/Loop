#!/usr/bin/env node
// O RÁDIO INTEIRO numa linha do tempo só.
//
// Quatro famílias abrem o canal sem ser chamadas — spotter, quebra na grade, peça do nosso carro
// e volta mais rápida. Cada uma foi medida antes de existir, e cada uma foi medida SOZINHA. A
// pergunta que ninguém tinha respondido é a única que o jogador faz: **cabe num par de ouvidos?**
//
// Duas famílias calmas separadas podem ser um rádio insuportável juntas, e o defeito não aparece
// em taxa nenhuma — aparece na FILA. Desde que a fila de anúncios existe (`engenheiroVoz.js`),
// nada é cortado no meio; o que acontece com o excesso é atraso e descarte. Um anúncio que sai
// doze segundos depois do fato é pior que anúncio nenhum, e ninguém sabia se isso acontece.
//
//   node scripts/analise-radio.mjs <linha-do-tempo.json>... [--por-piloto]
//
// A entrada tem duas metades, porque as fontes são de naturezas diferentes:
//
//   1. O SPOTTER vem de captura REAL do iRacing, via
//      `node scripts/analise-spotter.mjs <captura> --linha-do-tempo=<arquivo>`.
//      É telemetria gravada — os instantes são os que aconteceram.
//   2. QUEBRA e PEÇA PRÓPRIA vêm do modelo de desgaste, que só o Rust sabe rodar, via
//      `cargo test --lib despeja_linha_do_tempo -- --ignored --nocapture`.
//      São 30 realizações; cada piloto simulado recebe uma.
//   3. O RÁDIO DE RITMO é derivado das voltas da captura — os tempos são reais.
//
// A DURAÇÃO de cada fala não é estimada: sai do `.opus` que vai tocar. É o que torna a medição
// uma medição — a ocupação do canal é a soma de áudios que existem no disco.
//
// O que ele NÃO modela, e conta a favor do rádio: o push-to-talk. Uma pergunta do piloto ESVAZIA
// a fila, então toda pergunta melhora estes números.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { pausasDoRadio } from "../src/lib/pausasDoRadio.js";
import { duracaoOpus } from "./duracaoAudio.mjs";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const POR_PILOTO = process.argv.includes("--por-piloto");

// ═══════════════ As constantes vêm do fonte, como no arreio do spotter ═══════════════
//
// Copiar `VALIDADE_ANUNCIO_MS` para cá seria medir uma fila que não é a que roda. Se um nome
// mudar, isto falha alto — um padrão silencioso produziria uma tabela inteira de números
// plausíveis e errados.

function lerConstantesJs(relativo, nomes) {
  const fonte = fs.readFileSync(path.join(RAIZ, relativo), "utf8");
  const fora = [];
  const v = {};
  for (const n of nomes) {
    const m = new RegExp(`const\\s+${n}\\s*=\\s*(\\d+)`).exec(fonte);
    if (!m) fora.push(n);
    else v[n] = Number(m[1]);
  }
  if (fora.length) {
    console.error(`Constante(s) ausente(s) em ${relativo}: ${fora.join(", ")}`);
    process.exit(1);
  }
  return v;
}

function lerConstantesRust(relativo, nomes) {
  const fonte = fs.readFileSync(path.join(RAIZ, relativo), "utf8");
  const fora = [];
  const v = {};
  for (const n of nomes) {
    const m = new RegExp(`const\\s+${n}\\s*:\\s*\\w+\\s*=\\s*(-?\\d+)`).exec(fonte);
    if (!m) fora.push(n);
    else v[n] = Number(m[1]);
  }
  if (fora.length) {
    console.error(`Constante(s) ausente(s) em ${relativo}: ${fora.join(", ")}`);
    process.exit(1);
  }
  return v;
}

const K = lerConstantesJs("src/lib/engenheiroVoz.js", [
  "FILA_MAX",
  "VALIDADE_ANUNCIO_MS",
  "PAUSA_ENTRE_FRASES_MS",
  "CESSOES_MAX",
]);
const KR = lerConstantesRust("src-tauri/src/engenheiro/ritmo.rs", ["INTERVALO_VOLTAS"]);
const KTV = lerConstantesRust("src-tauri/src/engenheiro/tempo_volta.rs", [
  "MIN_DECIMOS",
  "MAX_DECIMOS",
  "DECIMOS_DE_APROXIMACAO",
]);

// ═══════════════════════ Durações reais, lidas dos `.opus` ══════════════════════

function carregarAcervo() {
  const d = new Map();
  for (const pasta of ["src/assets/engenheiro", "src/assets/spotter"]) {
    const abs = path.join(RAIZ, pasta);
    if (!fs.existsSync(abs)) continue;
    for (const nome of fs.readdirSync(abs)) {
      // O `.opus`, que é o que o app carrega. Os `.wav` masters convivem na mesma pasta e
      // medir os dois duplicaria cada peça no mapa.
      if (!nome.endsWith(".opus")) continue;
      const s = duracaoOpus(path.join(abs, nome));
      // O acervo do engenheiro vence em nome repetido — a mesma regra do `engenheiroVoz.js`.
      if (s) d.set(nome.replace(/\.opus$/, ""), s);
    }
  }
  return d;
}

const ACERVO = carregarAcervo();
if (ACERVO.size < 100) {
  console.error(`Acervo de voz não encontrado (${ACERVO.size} peças). Rode a partir da raiz.`);
  process.exit(1);
}

function mediana(v) {
  if (!v.length) return 0;
  const o = [...v].sort((a, b) => a - b);
  return o[Math.floor(o.length / 2)];
}

function percentil(v, p) {
  if (!v.length) return 0;
  const o = [...v].sort((a, b) => a - b);
  return o[Math.min(o.length - 1, Math.floor((o.length * p) / 100))];
}

/// Duração mediana das peças que casam com um prefixo. É o que resolve o spotter, onde a
/// linha do tempo diz a FAMÍLIA e não a peça — as três variações de uma família têm
/// comprimentos parecidos, e a mediana é a representante honesta.
function medianaPorPrefixo(prefixos) {
  const v = [];
  for (const [chave, s] of ACERVO) {
    if (prefixos.some((p) => chave.startsWith(p))) v.push(s);
  }
  return mediana(v);
}

/// Família de spotter → as peças que ela toca. As chaves são as de `src/assets/spotter/`.
const SPOTTER_PECAS = {
  frente: ["carro_fora_frente", "carro_parado_frente"],
  tras: ["carro_atras", "ainda_atras", "livre_atras"],
  lateral_entrada: ["esquerda", "direita", "duas_esquerda", "duas_direita", "tres_largos"],
  lateral_lembrete: ["ainda_esquerda", "ainda_direita", "ainda_ai"],
  lateral_liberacao: ["livre", "livre_esquerda", "livre_direita"],
};
const DUR_SPOTTER = Object.fromEntries(
  Object.entries(SPOTTER_PECAS).map(([k, p]) => [k, medianaPorPrefixo(p)]),
);
/// O sobrenome que entra no anúncio da volta mais rápida. A peça REAL de duração mediana, e não
/// um valor sintético: a linha do tempo não sabe quem é o carro 17, e a fala existe para 355
/// nomes — medir com o mais curto ou o mais longo seria escolher a resposta. Entrando como chave
/// de verdade, ela ainda atravessa `pausasDoRadio` como um `nm_` atravessa.
const SOBRENOME_MEDIANO = (() => {
  const alvo = medianaPorPrefixo(["nm_"]);
  let melhor = null;
  let dist = Infinity;
  for (const [chave, s] of ACERVO) {
    if (!chave.startsWith("nm_")) continue;
    if (Math.abs(s - alvo) < dist) {
      dist = Math.abs(s - alvo);
      melhor = chave;
    }
  }
  return melhor;
})();

function duracaoDe(chave) {
  return ACERVO.get(chave) ?? 0;
}

/// Quanto uma fala montada OCUPA o canal: as peças mais as pausas entre elas.
function duracaoDaFala(chaves) {
  if (!chaves.length) return 0;
  const pausas = pausasDoRadio(chaves);
  let s = 0;
  for (let i = 0; i < chaves.length; i += 1) {
    s += duracaoDe(chaves[i]);
    if (i < chaves.length - 1) s += (pausas[i] ?? K.PAUSA_ENTRE_FRASES_MS) / 1000;
  }
  return s;
}

// ═══════════════════════ O espelho de `engenheiro::ritmo` ═══════════════════════
//
// Espelho, e não reaproveitamento, porque o observador é Rust e a linha do tempo é JS. É
// pequeno o bastante para caber sob os olhos, e as constantes saem do fonte.

function decimosDe(s) {
  return Math.round(s * 10);
}

const EPSILON_S = 0.001;

function observadorDeRitmo() {
  return {
    ultimoDono: -1,
    ultimaMelhorS: 0,
    ultimaAnunciada: -Infinity,
    ultimaAproximacao: -Infinity,
    tomadas: 0,
    iniciado: false,
  };
}

function observar(o, p) {
  if (p.melhorS <= 0 || p.donoIdx < 0) return null;
  if (!o.iniciado) {
    o.iniciado = true;
    o.ultimoDono = p.donoIdx;
    o.ultimaMelhorS = p.melhorS;
    return null;
  }
  const trocou = p.donoIdx !== o.ultimoDono;
  // MELHOROU, e não "trocou de dono" — ver o cabeçalho de `engenheiro/ritmo.rs`.
  const melhorou = o.ultimaMelhorS <= 0 || p.melhorS < o.ultimaMelhorS - EPSILON_S;
  o.ultimoDono = p.donoIdx;
  if (melhorou) o.ultimaMelhorS = p.melhorS;
  const passou = (ultima) => agora - ultima >= KR.INTERVALO_VOLTAS;
  const agora = p.volta;

  if (trocou && p.eMinha) {
    const chave = ["tv_tomamos", "tv_tomamos_2", "tv_tomamos_3"][o.tomadas % 3];
    o.tomadas += 1;
    o.ultimaAnunciada = p.volta;
    return { pecas: [chave], tipo: "tomamos" };
  }
  if (melhorou && !p.eMinha && passou(o.ultimaAnunciada)) {
    const d = decimosDe(p.melhorS);
    if (d >= KTV.MIN_DECIMOS && d <= KTV.MAX_DECIMOS) {
      o.ultimaAnunciada = p.volta;
      // O sobrenome entra pelo representante mediano do pool — ver `SOBRENOME_MEDIANO`.
      return { pecas: ["tv_melhor_e_do", SOBRENOME_MEDIANO, `t_${d}`], tipo: "de_outro" };
    }
  }
  if (!p.eMinha && p.minhaS > 0 && passou(o.ultimaAproximacao)) {
    const falta = decimosDe(p.minhaS - p.melhorS);
    if (falta >= 1 && falta <= KTV.DECIMOS_DE_APROXIMACAO) {
      o.ultimaAproximacao = p.volta;
      return { pecas: [`tv_faltam_${falta}`], tipo: "aproximando" };
    }
  }
  return null;
}

/// As falas de ritmo de UM piloto, sobre as voltas reais da captura.
function falasDeRitmo(pilotos, alvo) {
  // Todas as voltas completadas da corrida, em ordem de chegada — é assim que a melhor volta
  // troca de dono ao vivo.
  const todas = [];
  for (const p of pilotos) {
    for (const v of p.voltas) if (v.tempoS > 0) todas.push({ t: v.t, idx: p.idx, s: v.tempoS });
  }
  todas.sort((a, b) => a.t - b.t);

  const o = observadorDeRitmo();
  const out = [];
  let melhorS = 0;
  let donoIdx = -1;
  let minhaVolta = 0;
  let minhaUltima = 0;
  for (const v of todas) {
    if (melhorS <= 0 || v.s < melhorS) {
      melhorS = v.s;
      donoIdx = v.idx;
    }
    if (v.idx !== alvo.idx) continue;
    minhaVolta += 1;
    minhaUltima = v.s;
    const f = observar(o, {
      volta: minhaVolta,
      minhaS: minhaUltima,
      melhorS,
      donoIdx,
      eMinha: donoIdx === alvo.idx,
    });
    if (f) out.push({ t: v.t, origem: "ritmo", pecas: f.pecas, tipo: f.tipo });
  }
  return out;
}

// ═══════════════════════ A fila de voz, como ela roda ═══════════════════════
//
// Reprodução de `engenheiroVoz.js`. As três regras que decidem o resultado:
//
//   · o spotter INTERROMPE — a peça em curso do engenheiro para e é repetida INTEIRA depois
//     (até `CESSOES_MAX`, daí sai por cima);
//   · anúncio ESPERA a vez, nunca corta;
//   · quem espera demais morre — `VALIDADE_ANUNCIO_MS` no momento em que o canal abriria.

/// Une os avisos do spotter em intervalos ocupados. Lá, o mais NOVO ganha: um aviso que chega
/// no meio de outro trunca o anterior.
function ocupacaoDoSpotter(eventos) {
  const iv = [];
  for (const e of eventos) {
    if (iv.length && iv[iv.length - 1].fim > e.t) iv[iv.length - 1].fim = e.t;
    iv.push({ ini: e.t, fim: e.t + e.dur });
  }
  return iv;
}

/// Quando o spotter cala, a partir de `t` — ou `t` se ele já está calado.
function livreDoSpotter(iv, t) {
  for (const s of iv) {
    if (s.ini <= t && t < s.fim) return s.fim;
    if (s.ini > t) break;
  }
  return t;
}

/// Toca uma fala montada a partir de `inicio`, cedendo ao spotter peça a peça.
function tocar(chaves, inicio, spotter) {
  const pausas = pausasDoRadio(chaves);
  let t = inicio;
  let cortes = 0;
  for (let i = 0; i < chaves.length; i += 1) {
    const dur = duracaoDe(chaves[i]);
    if (!dur) continue;
    for (let cessoes = 0; ; cessoes += 1) {
      t = livreDoSpotter(spotter, t);
      const colisao = spotter.find((s) => s.ini > t && s.ini < t + dur);
      if (!colisao || cessoes >= K.CESSOES_MAX) {
        t += dur; // sem interrupção, ou já cedeu o bastante e sai por cima
        break;
      }
      cortes += 1;
      t = colisao.ini; // parou aqui; repete a peça inteira depois
    }
    if (i < chaves.length - 1) t += (pausas[i] ?? K.PAUSA_ENTRE_FRASES_MS) / 1000;
  }
  return { fim: t, cortes };
}

function simular(anuncios, spotterEventos) {
  const spotter = ocupacaoDoSpotter(spotterEventos);
  const pendentes = [];
  const tocadas = [];
  let livre = 0;
  let piorPilha = 0;
  let descarteFila = 0;
  let descarteValidade = 0;

  const drenar = (ateT) => {
    while (pendentes.length) {
      const item = pendentes[0];
      const inicio = Math.max(livre, item.t);
      if (inicio > ateT) break;
      pendentes.shift();
      if ((inicio - item.t) * 1000 > K.VALIDADE_ANUNCIO_MS) {
        descarteValidade += 1;
        continue;
      }
      const r = tocar(item.pecas, inicio, spotter);
      tocadas.push({ ...item, inicio, fim: r.fim, cortes: r.cortes, atraso: inicio - item.t });
      livre = r.fim;
    }
  };

  for (const ev of anuncios) {
    drenar(ev.t);
    if (pendentes.length >= K.FILA_MAX) {
      descarteFila += 1;
      continue;
    }
    pendentes.push(ev);
    piorPilha = Math.max(piorPilha, pendentes.length);
  }
  drenar(Infinity);

  return { tocadas, piorPilha, descarteFila, descarteValidade, spotter };
}

// ═══════════════════════ Entrada ═══════════════════════

const arquivos = process.argv.slice(2).filter((a) => !a.startsWith("--"));
if (!arquivos.length) {
  console.error(
    "Uso: node scripts/analise-radio.mjs <linha-do-tempo.json>... [--por-piloto]\n" +
      "  Gere a linha do tempo com:\n" +
      "    node scripts/analise-spotter.mjs <captura.jsonl.gz> --linha-do-tempo=<saida.json>\n" +
      "    cargo test --lib despeja_linha_do_tempo -- --ignored --nocapture",
  );
  process.exit(1);
}

const CAMINHO_QUEBRA =
  process.env.LOOP_RADIO_SAIDA || path.join(process.env.TEMP || "/tmp", "loop_radio_timeline.json");
if (!fs.existsSync(CAMINHO_QUEBRA)) {
  console.error(
    `Linha do tempo de quebra ausente: ${CAMINHO_QUEBRA}\n` +
      `  cd src-tauri && cargo test --lib despeja_linha_do_tempo -- --ignored --nocapture`,
  );
  process.exit(1);
}
const QUEBRA = JSON.parse(fs.readFileSync(CAMINHO_QUEBRA, "utf8"));

function nf(v, casas = 2) {
  return v.toFixed(casas).replace(".", ",");
}

console.log(`\nACERVO: ${ACERVO.size} peças lidas do disco`);
console.log(
  `  spotter (mediana por família): ` +
    Object.entries(DUR_SPOTTER)
      .map(([k, v]) => `${k} ${nf(v)}s`)
      .join("  "),
);
console.log(
  `  do fonte: ` +
    Object.entries(K)
      .map(([c, v]) => `${c}=${v}`)
      .join("  ") +
    `  INTERVALO_VOLTAS=${KR.INTERVALO_VOLTAS}`,
);
console.log(
  `  quebra: ${QUEBRA.corridas.length} realizações de ${QUEBRA.carros} carros × ${QUEBRA.voltas} voltas`,
);

const linhas = [];
for (const a of arquivos) {
  const doc = JSON.parse(fs.readFileSync(a, "utf8"));
  linhas.push(doc);
}

for (const doc of linhas) {
  console.log(`\n${"═".repeat(78)}`);
  console.log(
    `${doc.captura} #${doc.sessao} — ${doc.pista} · ${nf(doc.duracaoS, 0)} s · ` +
      `${doc.pilotos.length} pilotos · lateral reconstruído a ±${doc.lateralJanelaM} m`,
  );

  const porPiloto = [];
  let cortadas = 0;
  let cortadasProprias = 0;
  let maxVoltas = 0;
  doc.pilotos.forEach((p, k) => {
    // Cada piloto simulado recebe UMA realização do modelo de quebra. Não é a corrida dele —
    // é uma corrida plausível daquele grid, e é o que dá a distribuição em vez de um número.
    const real = QUEBRA.corridas[k % QUEBRA.corridas.length];
    const voltas = p.voltas.filter((v) => v.tempoS > 0);
    if (voltas.length < 3) return; // piloto que não correu: nada a medir
    maxVoltas = Math.max(maxVoltas, voltas.length);

    // Volta → instante. A linha do tempo do Rust é por VOLTA; a da captura, por segundo. É
    // aqui que as duas se encontram, e é por isso que a corrida medida tem o número de voltas
    // da captura, não as 18 do arreio.
    //
    // `fase` espalha o evento DENTRO da volta. Duas quebras na mesma volta não são a mesma
    // rajada: cada carro cruza a linha na sua vez, e o monitor avalia carro a carro. Colapsá-las
    // no instante da volta do jogador inventaria uma pilha na fila que a corrida não produz — e
    // a pilha é justamente o que se está medindo.
    const instante = (volta, fase = 0) => {
      const i = Math.min(voltas.length - 1, Math.max(0, volta - 1));
      return voltas[i].t + fase * (voltas[i].tempoS || 0);
    };
    const fase = (carro) => ((carro ?? 1) - 1) / QUEBRA.carros;
    // A prova do modelo pode ser mais longa que a da captura. Amassar o excedente contra a
    // última volta empilharia dez voltas de quebra num instante só — a mesma rajada inventada
    // que o `fase` acabou de desfazer. Corta-se a prova, e o corte é CONTADO: um teto silencioso
    // aqui leria como "cobriu tudo".
    const dentro = (q) => q.volta <= voltas.length;
    cortadas += real.quebras.filter((q) => !dentro(q)).length;
    cortadasProprias += real.proprias.filter((q) => !dentro(q)).length;
    const anuncios = [
      ...real.quebras
        .filter((q) => q.pecas.length && dentro(q))
        .map((q) => ({ t: instante(q.volta, fase(q.carro)), origem: "quebra", pecas: q.pecas })),
      // A peça do NOSSO carro é avaliada na nossa própria cruzada — fase zero, por definição.
      ...real.proprias
        .filter(dentro)
        .map((q) => ({ t: instante(q.volta), origem: "propria", pecas: q.pecas })),
      ...falasDeRitmo(doc.pilotos, p),
    ].sort((a, b) => a.t - b.t);

    const spotterEventos = p.falas.map((f) => ({
      t: f.t,
      dur:
        f.familia === "lateral"
          ? DUR_SPOTTER[`lateral_${f.chave === "entrada" ? "entrada" : f.chave === "lembrete" ? "lembrete" : "liberacao"}`]
          : DUR_SPOTTER[f.familia] || DUR_SPOTTER.frente,
      familia: f.familia,
    }));

    const r = simular(anuncios, spotterEventos);
    const duracaoMin = (doc.duracaoS || 1) / 60;
    const ocupado =
      r.spotter.reduce((a, s) => a + (s.fim - s.ini), 0) +
      r.tocadas.reduce((a, f) => a + (f.fim - f.inicio), 0);
    // O maior silêncio entre falas — a outra ponta da mesma pergunta. Rádio que não cala é
    // ruim; rádio que some por sete minutos é um rádio que o jogador esquece que existe.
    const marcos = [
      ...r.spotter.map((s) => [s.ini, s.fim]),
      ...r.tocadas.map((f) => [f.inicio, f.fim]),
    ].sort((a, b) => a[0] - b[0]);
    let maiorSilencio = 0;
    // Começa na largada, não na virada do relógio de sessão — ver `t0` no despejo.
    let fim = doc.t0 ?? 0;
    for (const [ini, f] of marcos) {
      if (ini > fim) maiorSilencio = Math.max(maiorSilencio, ini - fim);
      fim = Math.max(fim, f);
    }
    // A cauda conta. Um rádio que cala nos últimos cinco minutos é tão notável quanto um que
    // cala no meio, e ignorar o trecho final esconderia justamente o fim de prova.
    if (doc.t1) maiorSilencio = Math.max(maiorSilencio, doc.t1 - fim);

    // A composição é o que decide o que cortar, se for para cortar. E ela separa o que foi
    // MEDIDO do que é hipótese: o lateral vem da reconstrução geométrica (±L m), não do canal
    // `CarLeftRight`, que nas duas capturas está parado porque o humano passou a prova no box.
    const conta = (lista, f) => lista.filter(f).length;
    porPiloto.push({
      idx: p.idx,
      lateral: conta(spotterEventos, (e) => e.familia === "lateral"),
      spotterMedido: conta(spotterEventos, (e) => e.familia !== "lateral"),
      quebra: conta(r.tocadas, (f) => f.origem === "quebra"),
      propria: conta(r.tocadas, (f) => f.origem === "propria"),
      ritmo: conta(r.tocadas, (f) => f.origem === "ritmo"),
      spotter: spotterEventos.length,
      anuncios: anuncios.length,
      tocados: r.tocadas.length,
      porMinuto: (spotterEventos.length + r.tocadas.length) / duracaoMin,
      ocupacao: ocupado / (doc.duracaoS || 1),
      atrasoMediano: mediana(r.tocadas.map((f) => f.atraso)),
      atrasoP90: percentil(r.tocadas.map((f) => f.atraso), 90),
      atrasoMax: r.tocadas.reduce((a, f) => Math.max(a, f.atraso), 0),
      cortes: r.tocadas.reduce((a, f) => a + f.cortes, 0),
      piorPilha: r.piorPilha,
      descarteFila: r.descarteFila,
      descarteValidade: r.descarteValidade,
      maiorSilencio,
    });
  });

  if (!porPiloto.length) {
    console.log("  nenhum piloto com voltas bastante para medir.");
    continue;
  }
  if (cortadas || cortadasProprias) {
    console.log(
      `  ⚠ a prova do modelo tem ${QUEBRA.voltas} voltas e a captura ${maxVoltas} — ` +
        `${cortadas} quebra(s) e ${cortadasProprias} aviso(s) além do fim ficaram de fora.\n` +
        `    Para casar: LOOP_RADIO_VOLTAS=${maxVoltas} LOOP_RADIO_CARROS=<grade> cargo test --lib ` +
        `despeja_linha_do_tempo -- --ignored`,
    );
  }

  const col = (f) => porPiloto.map(f);
  const linha = (rotulo, vals, casas = 2, sufixo = "") =>
    console.log(
      `  ${rotulo.padEnd(30)} mediana ${nf(mediana(vals), casas).padStart(7)}${sufixo}` +
        `   p90 ${nf(percentil(vals, 90), casas).padStart(7)}${sufixo}` +
        `   pior ${nf(Math.max(...vals), casas).padStart(7)}${sufixo}`,
    );

  console.log(`\n  ── por piloto-corrida (${porPiloto.length} pilotos) ${"─".repeat(30)}`);
  linha("falas por minuto", col((p) => p.porMinuto));
  linha("canal ocupado", col((p) => p.ocupacao * 100), 1, "%");
  linha("maior silêncio", col((p) => p.maiorSilencio), 0, "s");
  linha("atraso do anúncio (mediana)", col((p) => p.atrasoMediano), 1, "s");
  linha("atraso do anúncio (pior)", col((p) => p.atrasoMax), 1, "s");
  linha("pior pilha na fila", col((p) => p.piorPilha), 0);
  linha("descartados por validade", col((p) => p.descarteValidade), 0);
  linha("descartados por fila cheia", col((p) => p.descarteFila), 0);
  linha("peças repetidas (cedeu)", col((p) => p.cortes), 0);

  console.log(`\n  ── de onde vem a conversa (mediana por piloto-corrida) ${"─".repeat(19)}`);
  const composicao = [
    ["spotter lateral (HIPÓTESE — geometria, não canal)", (p) => p.lateral],
    ["spotter medido (fora/parado/trás)", (p) => p.spotterMedido],
    ["quebra na grade", (p) => p.quebra],
    ["peça do nosso carro", (p) => p.propria],
    ["volta mais rápida", (p) => p.ritmo],
  ];
  const totalMed = composicao.reduce((a, [, f]) => a + mediana(col(f)), 0) || 1;
  for (const [rotulo, f] of composicao) {
    const m = mediana(col(f));
    console.log(
      `  ${rotulo.padEnd(50)} ${String(m).padStart(4)}   ${nf((100 * m) / totalMed, 0).padStart(3)}%`,
    );
  }
  // Sem o lateral, a mesma corrida em falas por minuto. É o piso: o que sobra é medido.
  const semLateral = col((p) => (p.spotterMedido + p.tocados) / ((doc.duracaoS || 1) / 60));
  console.log(
    `  ${"sem o lateral, falas por minuto".padEnd(50)} ` +
      `mediana ${nf(mediana(semLateral))}   pior ${nf(Math.max(...semLateral))}`,
  );

  const somaAnuncios = porPiloto.reduce((a, p) => a + p.anuncios, 0);
  const somaTocados = porPiloto.reduce((a, p) => a + p.tocados, 0);
  const somaSpotter = porPiloto.reduce((a, p) => a + p.spotter, 0);
  console.log(
    `\n  totais: ${somaSpotter} falas de spotter · ${somaAnuncios} anúncios do engenheiro, ` +
      `${somaTocados} tocados (${nf((100 * somaTocados) / (somaAnuncios || 1), 0)}%)`,
  );

  if (POR_PILOTO) {
    console.log(`\n  ── piloto a piloto ${"─".repeat(50)}`);
    for (const p of porPiloto.sort((a, b) => b.porMinuto - a.porMinuto)) {
      console.log(
        `    #${String(p.idx).padStart(2)}  ${nf(p.porMinuto, 1).padStart(5)}/min · ` +
          `${nf(p.ocupacao * 100, 0).padStart(3)}% ocupado · ${p.spotter} spotter + ${p.tocados} anúncios · ` +
          `atraso pior ${nf(p.atrasoMax, 1)}s · ${p.descarteValidade} vencidos`,
      );
    }
  }
}
