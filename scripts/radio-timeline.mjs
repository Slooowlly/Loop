#!/usr/bin/env node
// A LINHA DO TEMPO DO RÁDIO, escrita: toda fala de uma sessão, na ordem, com o texto.
//
// Lê o `.jsonl` que o app grava em `<app_data>/logs/radio/` (ver `src-tauri/src/radio_registro.rs`)
// e imprime duas coisas: a lista de falas com os dois relógios lado a lado, e o resumo que
// responde as perguntas que o arquivo cru não responde de bate-pronto — quantas falas por
// minuto, quanto do tempo o canal ficou ocupado, o que foi engolido e o que foi decidido sem
// nunca chegar ao ouvido.
//
// O TEXTO das falas do spotter é resolvido aqui, pela chave, contra a fonte única das redações
// (`spotter-falas.mjs`); o do engenheiro, contra `docs/engenheiro-catalogo.json`. Nada de texto
// é copiado para dentro deste script — ele lê de onde a redação mora.
//
// Uso:
//   node scripts/radio-timeline.mjs                 # a sessão mais recente
//   node scripts/radio-timeline.mjs --lista         # que sessões existem
//   node scripts/radio-timeline.mjs <arquivo.jsonl>
//   node scripts/radio-timeline.mjs --sn 1          # só a sessão 1 do fim de semana
//   node scripts/radio-timeline.mjs --canal spotter,quebra
//   node scripts/radio-timeline.mjs --so-perdidas   # o que não chegou ao ouvido
//   node scripts/radio-timeline.mjs --canal portao  # só o silêncio: quando o rádio fechou, e por quê
//
// As marcas da coluna do meio: `·` decidida no Rust, `✎` redigida pelo modelo, `▶` tocou,
// `✗` não saiu inteira, `♪` tem gravação em disco, `○`/`⊘` o portão do rádio abrindo e fechando,
// `↔`/`⚠` uma ida ao servidor. Quando a fala não saiu inteira, a coluna de duração vira
// `tocado/pretendido`.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { FALAS } from "./spotter-falas.mjs";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

// A pasta que o app usa. O identificador é o do `tauri.conf.json`; num Windows com o perfil
// noutro lugar, `APPDATA` responde por isso sozinho.
function pastaDosRegistros() {
  const appdata = process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming");
  return path.join(appdata, "com.loop.app", "logs", "radio");
}

function lerArgumentos(argv) {
  const o = { arquivo: null, lista: false, sn: null, canais: null, soPerdidas: false, largura: 96 };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--lista") o.lista = true;
    else if (a === "--so-perdidas") o.soPerdidas = true;
    else if (a === "--sn") o.sn = Number(argv[(i += 1)]);
    else if (a === "--canal") o.canais = new Set(String(argv[(i += 1)]).split(","));
    else if (a === "--largura") o.largura = Number(argv[(i += 1)]);
    else if (!a.startsWith("--")) o.arquivo = a;
  }
  return o;
}

const opcoes = lerArgumentos(process.argv);

function sessoes() {
  const dir = pastaDosRegistros();
  if (!fs.existsSync(dir)) return [];
  // Por NOME, e não por mtime: o nome carrega o carimbo do boot, e ordenar por ele é o que
  // mantém a ordem cronológica mesmo depois de copiar os arquivos para outra máquina.
  return fs
    .readdirSync(dir)
    .filter((n) => n.startsWith("radio-") && n.endsWith(".jsonl"))
    .sort()
    .map((n) => path.join(dir, n));
}

// ── As redações, cada uma na casa dela ───────────────────────────────────────

/** Catálogo do engenheiro: chave → texto. Vazio quando o JSON não existe. */
function catalogoDoEngenheiro() {
  const p = path.join(raiz, "docs", "engenheiro-catalogo.json");
  if (!fs.existsSync(p)) return new Map();
  try {
    const bruto = JSON.parse(fs.readFileSync(p, "utf8"));
    return new Map(bruto.map((e) => [e.chave, e.texto]));
  } catch {
    return new Map();
  }
}

const CATALOGO = catalogoDoEngenheiro();

/**
 * O texto de uma chave, de qualquer das duas famílias.
 *
 * O que soou é a VARIAÇÃO (`esquerda_2`), e a redação mora na chave base (`esquerda`) — o
 * sufixo é o rodízio de gravações, não uma fala diferente. Sem o descarte do sufixo, toda linha
 * tocada do spotter aparece como `⟨chave⟩` enquanto a decisão logo acima aparece escrita, o que
 * faz o arquivo parecer não ter as falas que ele tem. A tentativa exata vem primeiro: há chaves
 * legítimas terminando em número (`cl_restam_2`).
 */
function textoDaChave(chave) {
  const exata = FALAS[chave] ?? CATALOGO.get(chave);
  if (exata) return exata;
  const base = String(chave).replace(/_\d+$/, "");
  if (base === chave) return null;
  return FALAS[base] ?? CATALOGO.get(base) ?? null;
}

/**
 * O texto de uma fala. O registro já traz a frase quando ela existe do lado do Rust (quebra,
 * ritmo, aviso, classificação) ou do servidor (resposta, ocasião). Quando não traz, ela é
 * montada das chaves — que é o caso do spotter, onde a chave É a fala.
 *
 * As duas linhas que NÃO são fala ganham uma frase montada aqui: o portão do rádio e a ida ao
 * servidor. Elas entram na mesma lista de propósito — o que se quer ler é o silêncio no meio das
 * falas, e um segundo relatório ao lado obrigaria a casar dois relógios com o dedo.
 */
function textoDaFala(linha) {
  const d = linha.detalhe ?? {};
  if (linha.canal === "portao") {
    const antes = d.de ? ` ⟨${d.de} durou ${d.durou_s ?? "?"}s⟩` : "";
    return `${linha.texto || "Portão."}${antes}`;
  }
  if (linha.canal === "servidor") {
    const erro = d.erro ? ` — ${String(d.erro).slice(0, 70)}` : "";
    const chars = typeof d.chars === "number" ? ` ${d.chars} caracteres` : "";
    return `${d.rota ?? "?"} ${d.ms ?? "?"} ms${chars}${erro}`;
  }
  if (linha.texto) return linha.texto;
  const partes = (linha.chaves ?? []).map((c) => textoDaChave(c) ?? `⟨${c}⟩`);
  return partes.join(" ");
}

// ── Leitura ──────────────────────────────────────────────────────────────────

function carregar(arquivo) {
  const linhas = fs
    .readFileSync(arquivo, "utf8")
    .split(/\r?\n/)
    .filter((l) => l.trim())
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return null;
      }
    })
    .filter(Boolean);
  const cabecalho = linhas.find((l) => l.kind === "header") ?? null;
  const falas = linhas.filter((l) => l.kind !== "header");
  costurarDuracoes(falas);
  return { cabecalho, falas };
}

/**
 * Costura o FECHAMENTO na abertura.
 *
 * Uma fala de peças (quebra, aviso, ritmo, resposta) não sabe quanto vai durar no instante em que
 * começa: são várias gravações com pausas e com as esperas que o spotter impõe no meio. O front
 * escreve a abertura no instante do som — que é a linha que casa com a decisão do Rust — e uma
 * linha de `fim` quando a fala termina, com a duração. Sem esta costura, a ocupação do canal
 * contaria zero segundo para tudo o que não é o spotter e a fala longa.
 */
function costurarDuracoes(falas) {
  const abertas = new Map();
  for (const f of falas) {
    if (f.fase !== "tocada") continue;
    if (f.desfecho === "ok") {
      abertas.set(f.canal, f);
    } else if (f.desfecho === "fim") {
      const abre = abertas.get(f.canal);
      if (abre && !abre.detalhe?.dur_s && f.detalhe?.dur_s) {
        abre.detalhe = { ...(abre.detalhe ?? {}), dur_s: f.detalhe.dur_s };
      }
      abertas.delete(f.canal);
    }
  }
}

// ── Impressão ────────────────────────────────────────────────────────────────

const RELOGIO = (rel) => String(rel ?? "").slice(11); // só hh:mm:ss.mmm
const SEG = (t) => (typeof t === "number" ? `${t.toFixed(1)}s`.padStart(8) : "     —  ");

/** O rótulo do desfecho, curto. `ok` não vira rótulo: o normal não precisa de marca. */
function marca(f) {
  // O portão do rádio: abriu ou fechou. Marca própria porque não é fala nenhuma — é o motivo
  // pelo qual não houve fala, que é a metade do arquivo que não se lê na lista de frases.
  if (f.canal === "portao") return f.desfecho === "aberto" ? "○" : "⊘";
  if (f.canal === "servidor") return f.desfecho === "ok" ? "↔" : "⚠";
  if (f.fase === "decidida") return "·";
  if (f.fase === "redigida") return "✎";
  if (f.desfecho === "ok") return "▶";
  // A fala que RECOMEÇOU do zero. Marca própria porque não é perda: é o mesmo texto saindo outra
  // vez, e ler isso como falha esconderia o defeito real, que é o rádio repetindo dezesseis
  // segundos de dossiê.
  if (f.desfecho === "repetida") return "↻";
  return "✗";
}

function imprimirFalas(falas, largura) {
  for (const f of falas) {
    const texto = textoDaFala(f);
    const cabe = texto.replace(/\n/g, " ⁄ ").slice(0, largura);
    const sufixo = texto.length > largura ? "…" : "";
    const desfecho = f.desfecho && f.desfecho !== "ok" ? `  [${f.desfecho}]` : "";
    const volta = typeof f.volta === "number" && f.volta > 0 ? `v${f.volta}` : "  ";
    // `tocado/pretendido` quando a fala não saiu inteira. Os dois juntos, e não só o que soou:
    // 1,4s é ótimo numa fala de 1,5s e é meia informação numa de 9s.
    const d = f.detalhe ?? {};
    const dur =
      d.tocado_s != null && d.dur_s != null
        ? ` ${d.tocado_s}/${d.dur_s}s`
        : d.dur_s
          ? ` ${d.dur_s}s`
          : d.tocado_s != null
            ? ` ${d.tocado_s}s`
            : "";
    // O ♪ marca a fala que tem gravação guardada. Vale à parte do texto porque é o que
    // responde "posso reouvir esta?" — e só a fala do modelo pode.
    const audio = f.audio ? "  ♪" : "";
    console.log(
      `${RELOGIO(f.rel)} ${SEG(f.t)} sn${f.sn ?? "-"} ${volta.padStart(3)} ` +
        `${marca(f)} ${String(f.canal).padEnd(13)}${dur.padEnd(11)} ${cabe}${sufixo}${desfecho}${audio}`,
    );
  }
}

/**
 * O resumo. Cada número aqui existe para responder uma pergunta que a lista não responde:
 * quantas falas por minuto o rádio produz, quanto do tempo ele ocupa o canal, e quantas
 * decisões morreram sem virar som.
 */
function imprimirResumo(falas) {
  const tocadas = falas.filter((f) => f.fase === "tocada" && f.desfecho === "ok");
  const decididas = falas.filter((f) => f.fase === "decidida");
  // `fim` é fechamento e `repetida` é a mesma fala saindo de novo. Nenhum dos dois é perda, e
  // contá-los aqui diria que metade do rádio foi engolida.
  const ESCRITURACAO = new Set(["ok", "fim", "repetida"]);
  const engolidas = falas.filter((f) => f.fase === "tocada" && !ESCRITURACAO.has(f.desfecho));

  const porCanal = new Map();
  for (const f of tocadas) {
    const c = porCanal.get(f.canal) ?? { n: 0, s: 0 };
    c.n += 1;
    c.s += f.detalhe?.dur_s ?? 0;
    porCanal.set(f.canal, c);
  }
  const porDesfecho = new Map();
  for (const f of engolidas) {
    porDesfecho.set(f.desfecho, (porDesfecho.get(f.desfecho) ?? 0) + 1);
  }

  // A janela é medida no relógio de sessão quando ele existe, e no de parede quando não.
  const ts = falas.map((f) => f.t).filter((t) => typeof t === "number");
  const janela_s = ts.length > 1 ? Math.max(...ts) - Math.min(...ts) : 0;
  const falado_s = tocadas.reduce((s, f) => s + (f.detalhe?.dur_s ?? 0), 0);

  console.log(`\n── resumo ──`);
  console.log(`  falas tocadas          ${tocadas.length}`);
  console.log(`  decisões registradas   ${decididas.length}`);
  console.log(`  engolidas              ${engolidas.length}`);
  if (janela_s > 0) {
    console.log(`  janela de sessão       ${(janela_s / 60).toFixed(1)} min`);
    console.log(
      `  taxa                   ${((tocadas.length / janela_s) * 60).toFixed(1)} falas/min`,
    );
    if (falado_s > 0) {
      console.log(
        `  canal ocupado          ${falado_s.toFixed(0)}s de ${janela_s.toFixed(0)}s ` +
          `(${((falado_s / janela_s) * 100).toFixed(1)}%)`,
      );
    }
  }

  if (porCanal.size) {
    console.log(`\n  por canal (tocadas):`);
    for (const [canal, c] of [...porCanal].sort((a, b) => b[1].n - a[1].n)) {
      const seg = c.s ? `  ${c.s.toFixed(1)}s` : "";
      console.log(`    ${canal.padEnd(14)} ${String(c.n).padStart(4)}${seg}`);
    }
  }
  if (porDesfecho.size) {
    console.log(`\n  o que não saiu inteiro:`);
    for (const [d, n] of [...porDesfecho].sort((a, b) => b[1] - a[1])) {
      console.log(`    ${d.padEnd(14)} ${String(n).padStart(4)}`);
    }
  }

  // OS RECOMEÇOS. Quando o spotter abre o canal no meio, a fala do engenheiro não continua de
  // onde parou: ela toca inteira outra vez. Sem esta conta, dezesseis segundos de dossiê ouvidos
  // duas vezes aparecem no resumo como dezesseis segundos ouvidos uma vez.
  const cedidas = falas.filter((f) => f.desfecho === "cedeu");
  const repetidas = falas.filter((f) => f.desfecho === "repetida");
  if (cedidas.length || repetidas.length) {
    console.log(`\n  o canal disputado:`);
    console.log(`    cedeu ao spotter       ${cedidas.length}`);
    console.log(`    recomeçou do zero      ${repetidas.length}`);
    const porCima = repetidas.filter((f) => f.detalhe?.por_cima).length;
    if (porCima) console.log(`    saiu POR CIMA          ${porCima}  (duas vozes ao mesmo tempo)`);
    for (const c of cedidas) {
      const d = c.detalhe ?? {};
      const onde = d.peca != null ? ` peça ${d.peca + 1}/${d.pecas}` : "";
      // `de Xs` só quando a duração pretendida é conhecida: na sequência de peças a linha do
      // recomeço é da frase, e a frase não carrega a duração da fala inteira.
      const total = d.dur_s ? ` de ${d.dur_s}s` : "";
      console.log(
        `    ${RELOGIO(c.rel)} ${String(c.canal).padEnd(13)}${onde} ouviu ${d.tocado_s ?? "?"}s${total}`,
      );
    }
  }

  // O SILÊNCIO, medido. Enquanto o portão do momento estava fechado, a fala não solicitada
  // nem chegava a existir — sem estas linhas, uma corrida inteira calada por duelo é igual no
  // arquivo a uma corrida em que ninguém teve nada a dizer.
  const portao = falas.filter((f) => f.canal === "portao");
  if (portao.length) {
    const tempo = new Map();
    for (const p of portao) {
      const de = p.detalhe?.de;
      const s = p.detalhe?.durou_s;
      if (!de || typeof s !== "number") continue;
      tempo.set(de, (tempo.get(de) ?? 0) + s);
    }
    const total = [...tempo.values()].reduce((a, b) => a + b, 0);
    const fechado = [...tempo].filter(([e]) => e !== "aberto").reduce((a, [, s]) => a + s, 0);
    // A ABERTURA não é virada: ela é o retrato de entrada na sessão, com `de` nulo. Contá-la junto
    // faria uma sessão que nunca virou parecer ter virado uma vez.
    const viradas = portao.filter((p) => p.detalhe?.de).length;
    const aberturas = portao.length - viradas;
    const plural = viradas === 1 ? "virada" : "viradas";
    const conta = aberturas
      ? `${viradas} ${plural}, ${aberturas} entrada${aberturas > 1 ? "s" : ""} em sessão`
      : `${viradas} ${plural}`;
    console.log(`\n  o portão do rádio (${conta}):`);
    for (const [estado, s] of [...tempo].sort((a, b) => b[1] - a[1])) {
      const pct = total > 0 ? ` (${((s / total) * 100).toFixed(0)}%)` : "";
      console.log(`    ${estado.padEnd(14)} ${s.toFixed(0).padStart(6)}s${pct}`);
    }
    if (total > 0) {
      console.log(`    ${"FECHADO".padEnd(14)} ${fechado.toFixed(0).padStart(6)}s de ${total.toFixed(0)}s`);
    }
    // O estado CORRENTE não entra na conta: ele só é medido quando vira, e a última virada de
    // uma sessão nunca acontece. Numa sessão curta isso é a maior parte do tempo.
    const caladas = falas.filter((f) => f.desfecho === "suprimida").length;
    if (caladas) console.log(`    falas caladas por ele  ${caladas}`);
  }

  // O SERVIDOR. A fala do modelo depende de uma ida à rede, e um engenheiro mudo por síntese
  // lenta era indistinguível de um engenheiro mudo por decisão — nenhum dos dois deixava linha.
  const idas = [
    ...falas
      .filter((f) => f.canal === "servidor")
      .map((f) => ({ rota: f.detalhe?.rota ?? "?", ms: f.detalhe?.ms, desfecho: f.desfecho })),
    // O sucesso da redação não tem linha própria: o `ms` viaja no detalhe da própria fala (ver
    // `ptt_voz.rs`). Sem juntá-lo aqui, o resumo mostraria só as falhas — e diria que a resposta
    // do engenheiro nunca funcionou.
    ...falas
      .filter((f) => f.fase === "redigida" && typeof f.detalhe?.ms === "number")
      .map((f) => ({ rota: "ptt-responder", ms: f.detalhe.ms, desfecho: "ok" })),
  ];
  if (idas.length) {
    const porRota = new Map();
    for (const i of idas) {
      const r = porRota.get(i.rota) ?? { n: 0, ms: [], falhas: new Map() };
      r.n += 1;
      if (typeof i.ms === "number") r.ms.push(i.ms);
      if (i.desfecho !== "ok") r.falhas.set(i.desfecho, (r.falhas.get(i.desfecho) ?? 0) + 1);
      porRota.set(i.rota, r);
    }
    console.log(`\n  idas ao servidor:`);
    for (const [rota, r] of [...porRota].sort((a, b) => b[1].n - a[1].n)) {
      const ordenados = r.ms.slice().sort((a, b) => a - b);
      // Mediana e PIOR caso. A média esconde exatamente o que interessa: uma ida de 20 s no meio
      // de dez de 2 s é a única que o jogador sentiu, e ela desaparece em qualquer média.
      // Mediana INFERIOR: com duas idas, `n/2` pegaria a pior das duas e a chamaria de mediana,
      // que é o jeito mais fácil de fazer um servidor saudável parecer quebrado.
      const meio = ordenados.length ? ordenados[Math.floor((ordenados.length - 1) / 2)] : null;
      const pior = ordenados.length ? ordenados[ordenados.length - 1] : null;
      const tempos = meio != null ? `  mediana ${meio} ms  pior ${pior} ms` : "";
      console.log(`    ${rota.padEnd(16)} ${String(r.n).padStart(4)}${tempos}`);
      for (const [d, n] of r.falhas) console.log(`      ${d.padEnd(14)} ${n}`);
    }
  }

  // AS GRAVAÇÕES. Só as falas do modelo têm arquivo; as do acervo são as peças de
  // `src/assets/`, apontadas pela chave. O caminho vai impresso para reouvir ser um clique.
  const comAudio = falas.filter((f) => f.audio);
  if (comAudio.length) {
    const dir = path.join(pastaDosRegistros(), "audio");
    console.log(`\n  gravações guardadas (${comAudio.length}) em ${dir}`);
    for (const f of comAudio) {
      console.log(`    ${f.audio}  ${textoDaFala(f).slice(0, 58)}`);
    }
  }

  // DECIDIDA SEM TOCADA. É a pergunta que o registro de duas fases existe para responder: o
  // Rust concluiu que havia o que dizer e o jogador não ouviu nada. Casa por chave dentro de
  // uma janela curta — ids não atravessam a ponte, e o par certo é sempre o mais próximo.
  const perdidas = [];
  for (const d of decididas) {
    const chave = (d.chaves ?? [])[0];
    if (!chave) continue;
    const achou = falas.some(
      (f) =>
        f.fase === "tocada" &&
        // `ok` e nada mais: uma fala adiada ou cortada não chegou inteira ao ouvido, e contá-la
        // como entregue esconderia justamente o que se quer ver. Quando a adiada sai de fato,
        // ela produz uma segunda linha `ok` e o par se fecha ali.
        f.desfecho === "ok" &&
        (f.chaves ?? []).some((c) => c === chave || c.startsWith(`${chave}_`)) &&
        typeof f.t === "number" &&
        typeof d.t === "number" &&
        f.t >= d.t - 0.5 &&
        f.t <= d.t + 8,
    );
    if (!achou) perdidas.push(d);
  }
  if (perdidas.length) {
    console.log(`\n  DECIDIDAS QUE NÃO VIRARAM SOM (${perdidas.length}):`);
    for (const p of perdidas) {
      console.log(
        `    ${RELOGIO(p.rel)} ${SEG(p.t)} ${String(p.canal).padEnd(13)} ${textoDaFala(p).slice(0, 60)}`,
      );
    }
  }
}

// ── Execução ─────────────────────────────────────────────────────────────────

const lista = sessoes();

if (opcoes.lista) {
  if (!lista.length) {
    console.log(`Nenhum registro em ${pastaDosRegistros()}`);
  }
  for (const p of lista) {
    const { falas } = carregar(p);
    const tocadas = falas.filter((f) => f.fase === "tocada" && f.desfecho === "ok").length;
    console.log(`${path.basename(p)}  ${String(falas.length).padStart(5)} linhas  ${tocadas} tocadas`);
  }
  process.exit(0);
}

const arquivo = opcoes.arquivo ?? lista[lista.length - 1];
if (!arquivo || !fs.existsSync(arquivo)) {
  console.error(
    `Nenhum registro de rádio encontrado.\n` +
      `  Pasta: ${pastaDosRegistros()}\n` +
      `  O arquivo nasce na PRIMEIRA fala de uma sessão — abra o app, entre numa sessão do ` +
      `iRacing e rode de novo.`,
  );
  process.exit(1);
}

const { cabecalho, falas } = carregar(arquivo);
// O fechamento já foi costurado na abertura pela leitura: mostrá-lo aqui listaria cada fala duas
// vezes, e a segunda sem texto nenhum.
let mostrar = falas.filter((f) => f.desfecho !== "fim");
if (opcoes.sn !== null && !Number.isNaN(opcoes.sn)) mostrar = mostrar.filter((f) => f.sn === opcoes.sn);
if (opcoes.canais) mostrar = mostrar.filter((f) => opcoes.canais.has(f.canal));
if (opcoes.soPerdidas) mostrar = mostrar.filter((f) => f.desfecho && f.desfecho !== "ok");

console.log(`── ${path.basename(arquivo)}`);
if (cabecalho) console.log(`   Loop ${cabecalho.app} (${cabecalho.os}) — aberto em ${cabecalho.rel}`);
console.log(
  `   relógio      sessão  sn volta  canal                      fala\n` +
    `   ${"─".repeat(90)}`,
);
imprimirFalas(mostrar, opcoes.largura);
imprimirResumo(falas);
