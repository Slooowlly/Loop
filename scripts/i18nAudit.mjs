// Detector de strings de UI em português ainda NÃO traduzidas (fora de `t()`/`i18n.t`).
//
// Objetivo: quando você adiciona/edita UI e esquece de passar por i18n, este scanner
// aponta arquivo:linha exatos para traduzir. Roda de 3 jeitos:
//   • no teste `src/i18n/i18nCoverage.test.js` (falha a suíte quando há pendência);
//   • no hook de pre-commit (`--staged`, checa só o que está no stage, LENDO O BLOB DO ÍNDICE);
//   • na mão: `npm run i18n:audit`.
//
// A linha é varrida DEPOIS de tirar dela as chamadas `t(...)`, e não pulada por conter uma. A
// linha traduzida pela metade é o caso mais comum, e era o que sumia.
//
// COMO MARCAR EXCEÇÕES INTENCIONAIS (sem allowlist central):
//   • `i18n-ignore` em qualquer lugar da linha (ou na linha imediatamente acima) → pula a linha.
//     Em JSX use um comentário-expressão: {/* i18n-ignore */}
//   • `i18n-ignore-file` em qualquer lugar do arquivo → pula o arquivo inteiro
//     (ex.: telas de dados-de-exemplo/WIP, ou código morto).
//
// COBERTURA: `.jsx` e `.js` (fora os `*.test.*`). O `.jsx` é varrido pelas formas de JSX
// listadas abaixo; o `.js` é varrido por LITERAL DE TEXTO, porque num helper a copy não tem
// tag em volta — ela é o valor de retorno, o item do array, o rótulo do mapa. Ver
// `varreduraJs` e o baseline em `scripts/i18nBaseline.mjs`.
//
// O QUE ELE ENXERGA dentro do `.jsx` (as cinco formas de escrever copy):
//   1. nó de texto na mesma linha das tags:  <p>Texto</p>
//   2. nó de texto sozinho na linha, entre a abertura e o fechamento (o que o Prettier
//      produz para qualquer rótulo longo)
//   3. atributo de UI em aspas:              title="Texto"
//   4. atributo de UI em template literal:   aria-label={`Texto ${x}`}
//   5. prosa colada a uma expressão:         <span>Criado: {x}</span> / <span>{n} corridas</span>
//
// As formas 2, 4 e 5 entraram em 11/08/2026. Antes disso o auditor via só as formas 1 e 3, e a
// varredura que motivou a extensão encontrou 8 strings de UI em português vivas em produção,
// todas em telas que já estavam traduzidas no resto — o vão pegava justamente a copy longa e a
// copy com número, que são as duas mais comuns.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { execFileSync } from "node:child_process";

import { BASELINE_JS } from "./i18nBaseline.mjs";

// Raiz do repo: sobe a partir do CWD até achar um package.json (funciona rodando de
// qualquer subpasta, no vitest, no hook e no `npm run`). Evita depender de import.meta.url,
// que no ambiente do vitest nem sempre é um file:// válido.
function findRepoRoot(start) {
  let dir = start;
  for (let i = 0; i < 30; i++) {
    if (existsSync(join(dir, "package.json"))) return dir;
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return start;
}

const ROOT = findRepoRoot(process.cwd());
const SRC = join(ROOT, "src");

// PT: acento OU palavra-função/UI que praticamente só existe em português. Mantido
// conservador de propósito — falso-negativo (deixa passar) é preferível a ruído; se algo
// escapar, a próxima edição/varredura pega. Amplie a lista quando notar um vão.
const PT = /[ãçõáéíóúâêà]|(^|[^a-zA-Z])(de|da|do|dos|das|em|no|na|com|para|por|sem|à|às|ao|aos|mundial|equipe|equipes|piloto|pilotos|ranking|corrida|corridas|temporada|hist[oó]rico|montando|panorama|geral|volta|voltas|parada|paradas|ritmo|melhor|posi[cç][aã]o|composto|largada|torre|tempos|fechar|voltar|salvar|cancelar|confirmar|nenhum|nenhuma|escolher|selecionar|pr[oó]ximo|anterior|detalhes|resumo|abrir|falha|erro|sucesso|carreira|jogador|semana|calend[aá]rio|rodada|pontos|pne?us?|combust[ií]vel|clima|chuva|seco|abandono|paddock|campe[aã]o|campe[oõ]es|t[ií]tulo|t[ií]tulos|vit[oó]ria|derrota|elenco|contrato|contratos|sal[aá]rio|mercado|proposta|convoca[cç][aã]o|prévia|briefing|manuten[cç][aã]o|criando|preparando|processando|salvando|enviando|gerando|buscando|aguardando|definindo|atualizando|abrindo|carregando|removendo|aplicando|calculando|iniciando|encerrando|avan[cç]ando|entrando|montagem|feito|falhou|pronto|dispon[ií]vel|indispon[ií]vel)([^a-zA-Z]|$)/i;

const IGNORE_LINE = /i18n-ignore(?!-file)/;
const IGNORE_FILE = /i18n-ignore-file/;

// nós de texto `>Texto<` e literais de atributos de UI
const TEXT_NODE = />\s*([A-Za-zÀ-ú][^<>{}]*[A-Za-zÀ-ú])\s*</g;
const UI_ATTR = /(?:title|aria-label|placeholder|alt)\s*=\s*"([^"]*[A-Za-zÀ-ú][^"]*)"/g;

// O mesmo atributo de UI, escrito como TEMPLATE LITERAL. Este é o vão que o `UI_ATTR` acima
// deixava aberto: assim que a copy ganha uma interpolação — `aria-label={`Ver títulos de
// ${nome}`}` — as aspas viram crase, o padrão de aspas para de casar e a prosa some do
// auditor. Foi assim que dois aria-label em PT sobreviveram até 11/08/2026 numa tela que
// já estava traduzida por inteiro no resto.
const UI_ATTR_TEMPLATE = /(?:title|aria-label|placeholder|alt)\s*=\s*\{`([^`]*[A-Za-zÀ-ú][^`]*)`\}/g;

// Texto de UI escrito como template literal no CORPO do JSX: `<p>{`${n} de ${t} pilotos`}</p>`.
// O `TEXT_NODE` não o vê de propósito (ele exclui `{` e `}` para não confundir expressão com
// prosa), então a forma mais natural de montar uma frase com número escapava inteira.
const CHILDREN_TEMPLATE = /[>{]\s*\{`([^`]*[A-Za-zÀ-ú][^`]*)`\}/g;

// Prosa COLADA numa expressão na mesma linha: `<span>Criado: {formatDateTime(x)}</span>`. O
// `TEXT_NODE` exige `>` e `<` com só letras no meio, então a metade de prosa some junto com a
// chave. Cobre os dois lados da expressão, porque `{n} corridas no calendário` é tão copy
// quanto `Criado: {x}`.
const ANTES_DA_EXPRESSAO = />\s*([A-Za-zÀ-ú][^<>{}]{2,}?)\s*\{/g;
const DEPOIS_DA_EXPRESSAO = /\}\s*([A-Za-zÀ-ú][^<>{}]{2,}?)\s*</g;

/// Descarta o que é CÓDIGO e não prosa. O padrão de "texto colado a expressão" também casa o
/// `>` de uma arrow function (`(e) => e.id === x`), e um falso positivo por linha em cada
/// callback tornaria o auditor inutilizável no pre-commit.
function pareceCodigo(texto) {
  return /[=()[\];.]|=>|&&|\|\|/.test(texto);
}

/// Nó de texto que ocupa a LINHA INTEIRA, com a tag de abertura na linha de cima e a de
/// fechamento na de baixo:
///
///     <button ...>
///       Voltar para Classificação      <- esta linha
///     </button>
///
/// É a forma que o Prettier dá a qualquer rótulo que não caiba na largura da linha, e era o
/// maior dos vãos: o `TEXT_NODE` exige `>` e `<` na MESMA linha, então toda copy longa o
/// bastante para quebrar escapava do auditor inteiro.
function noDeTextoSozinho(lines, i) {
  const texto = lines[i].trim();
  if (texto.length < 4) return null;
  if (!/^[A-Za-zÀ-ú][^<>{}"'`]*$/.test(texto)) return null;
  if (!/>\s*$/.test(lines[i - 1] ?? "")) return null;
  if (!/^\s*<\//.test(lines[i + 1] ?? "")) return null;
  return [texto];
}

/// A parte FIXA de um template literal: o que sobra depois de tirar as interpolações. É só
/// isso que cabe traduzir; `${row.nome}` é dado, não copy.
function parteFixa(texto) {
  return texto.replace(/\$\{[^}]*\}/g, " ");
}

/// A chamada de tradução, trocada por um MARCADOR antes da varredura.
///
/// Antes, a linha inteira era pulada quando continha `t(`. A conta parecia boa e não era: uma
/// linha traduzida PELA METADE é o caso mais comum de todos, e era exatamente o que sumia.
/// `<span>{t("titulos")}: {n} vitórias</span>` tem uma chave e duas palavras cruas, e o auditor
/// dava a linha por resolvida por causa da chave.
///
/// O marcador é um identificador sem acento e fora da lista de palavras PT, e mantém as chaves
/// em volta (`{t("x")}` vira `{TRADUZIDO}`), então os padrões de "prosa colada a expressão"
/// continuam vendo a fronteira e pegam a metade que ficou de fora.
const MARCADOR = "TRADUZIDO";

/// Fim da chamada iniciada no parêntese `abre`, contando aninhamento e ignorando parêntese
/// dentro de literal de texto (`t("Vencedor (provisório)")` fecharia cedo demais).
function fimDaChamada(linha, abre) {
  let nivel = 0;
  let aspa = null;
  for (let i = abre; i < linha.length; i += 1) {
    const c = linha[i];
    if (aspa) {
      if (c === "\\") i += 1;
      else if (c === aspa) aspa = null;
      continue;
    }
    if (c === '"' || c === "'" || c === "`") aspa = c;
    else if (c === "(") nivel += 1;
    else if (c === ")") {
      nivel -= 1;
      if (nivel === 0) return i + 1;
    }
  }
  return -1; // chamada que atravessa a linha: melhor deixar como está
}

/** Troca cada `t(...)` / `i18n.t(...)` pelo marcador, preservando o resto da linha. */
export function mascararTraduzido(linha) {
  const re = /(^|[^A-Za-z0-9_$])(?:i18n\.)?t\(/g;
  let saida = "";
  let cursor = 0;
  let m;
  while ((m = re.exec(linha)) !== null) {
    const abre = m.index + m[0].length - 1;
    const fim = fimDaChamada(linha, abre);
    if (fim < 0) break;
    saida += linha.slice(cursor, m.index + m[1].length) + MARCADOR;
    cursor = fim;
    re.lastIndex = fim;
  }
  return saida + linha.slice(cursor);
}

// ── varredura do `.js` ───────────────────────────────────────────────────────────────────
//
// Num helper `.js` a copy não tem tag em volta: ela é o valor de retorno, o item do array, o
// rótulo do mapa. Então aqui o alvo é o LITERAL DE TEXTO, e o trabalho todo é separar prosa de
// código. Os descartes abaixo são estruturais e valem para o arquivo inteiro; o passivo que
// sobrou depois deles está listado, string por string, em `scripts/i18nBaseline.mjs`.

/// Extensão de asset: `"Autódromo Nazionale Monza.jpg"` é NOME DE ARQUIVO, não copy.
const ARQUIVO_DE_ASSET = /\.(jpe?g|png|webp|svg|gif|mp3|ogg|opus|wav|json|css|jsx?|mjs|rs|yml)$/i;

/// Argumento de `console.*` é diagnóstico de desenvolvimento: nunca chega na tela, e traduzir
/// log só atrapalha quem lê o console. A linha inteira é pulada.
const DIAGNOSTICO = /console\.[a-z]+\s*\(/;

/// Um literal por vez, andando na linha e respeitando aspas/crase/escape. Para no `//` e no
/// `/*` fora de string, que é o que tira o comentário de fim de linha da varredura — sem isso
/// todo comentário em português viraria pendência de tradução.
function literaisDaLinha(linha) {
  const achados = [];
  let i = 0;
  while (i < linha.length) {
    const c = linha[i];
    if (c === "/" && (linha[i + 1] === "/" || linha[i + 1] === "*")) break;
    if (c === '"' || c === "'" || c === "`") {
      let j = i + 1;
      let texto = "";
      while (j < linha.length && linha[j] !== c) {
        if (linha[j] === "\\") {
          j += 2;
          texto += " ";
          continue;
        }
        texto += linha[j];
        j += 1;
      }
      if (j >= linha.length) break; // literal que atravessa a linha: fica de fora
      achados.push({
        texto,
        antes: linha.slice(Math.max(0, i - 1), i),
        depois: linha.slice(j + 1, j + 2),
      });
      i = j + 1;
      continue;
    }
    i += 1;
  }
  return achados;
}

/// Literal que é CHAVE, IDENTIFICADOR ou DADO, e não copy.
function ehCodigo({ texto, antes, depois }) {
  if (depois === ":") return true; // chave de objeto: `"corrida": ...`
  if (antes === ".") return true; // acesso por membro
  if (/^[\w.\-/@#%]+$/.test(texto)) return true; // token único (id, classe, caminho, evento)
  return ARQUIVO_DE_ASSET.test(texto);
}

/// Interpolação vira espaço e o espaço em excesso colapsa, para o texto do baseline não mudar
/// quando alguém renomeia a variável dentro do template.
function normalizar(texto) {
  return parteFixa(texto).replace(/\s+/g, " ").trim();
}

function varreduraJs(rel, lines, { aplicarBaseline = true } = {}) {
  const permitido = aplicarBaseline ? (BASELINE_JS[rel] ?? []) : [];
  const found = [];
  for (let i = 0; i < lines.length; i++) {
    const bruta = lines[i];
    if (/^\s*(\/\/|\*|\/\*)/.test(bruta)) continue; // linha de comentário
    if (IGNORE_LINE.test(bruta) || IGNORE_LINE.test(lines[i - 1] ?? "")) continue;
    if (/^\s*import\s|require\(/.test(bruta)) continue; // caminho de módulo
    if (DIAGNOSTICO.test(bruta)) continue;
    for (const lit of literaisDaLinha(mascararTraduzido(bruta))) {
      if (ehCodigo(lit)) continue;
      const text = normalizar(lit.texto);
      if (!text) continue;
      if (!PT.test(` ${text} `)) continue;
      if (permitido.includes(text)) continue;
      found.push({ file: rel, line: i + 1, text });
    }
  }
  return found;
}

function listarArquivosDeUI() {
  return readdirSync(SRC, { recursive: true })
    .map((p) => (typeof p === "string" ? p : p.toString()))
    .filter((p) => ehArquivoDeUI(`src/${p.split(sep).join("/")}`))
    .map((p) => join(SRC, p));
}

function ehArquivoDeUI(rel) {
  if (!rel.startsWith("src/")) return false;
  if (/\.test\.jsx?$/.test(rel)) return false;
  return rel.endsWith(".jsx") || rel.endsWith(".js");
}

/// Os arquivos de UI no stage (.jsx e .js), LIDOS DO ÍNDICE.
///
/// A diferença importa no hook de pre-commit, que é onde este modo roda. O que vai ser commitado
/// é o blob do índice, e ele pode não ser o que está no disco: `git add` de um arquivo seguido de
/// mais edições, `git add -p` de um pedaço só, `git stash` de meio caminho. Lendo o disco, o hook
/// julgava um texto que ninguém ia commitar — deixando passar a string crua que estava no índice,
/// e bloqueando o commit por uma que ainda estava sendo escrita.
function stagedBlobs() {
  const out = execFileSync("git", ["diff", "--cached", "--name-only", "--diff-filter=ACM"], {
    cwd: ROOT,
    encoding: "utf8",
  });
  return out
    .split("\n")
    .map((s) => s.trim())
    .filter(ehArquivoDeUI)
    .map((rel) => {
      try {
        return { rel, src: execFileSync("git", ["show", `:${rel}`], { cwd: ROOT, encoding: "utf8" }) };
      } catch {
        return null; // sumiu do índice entre o `diff` e o `show`
      }
    })
    .filter(Boolean);
}

export function scanTexto(rel, src) {
  if (IGNORE_FILE.test(src)) return [];
  const lines = src.split("\n");
  if (rel.endsWith(".js")) return varreduraJs(rel, lines);
  const found = [];
  for (let i = 0; i < lines.length; i++) {
    const bruta = lines[i];
    const prev = lines[i - 1] ?? "";
    if (/^\s*(\/\/|\*|\/\*)/.test(bruta)) continue; // linha de comentário
    if (IGNORE_LINE.test(bruta) || IGNORE_LINE.test(prev)) continue; // marcada
    // A chamada de tradução sai da linha; o que sobra é o que ainda NÃO passou por `t()`.
    const line = mascararTraduzido(bruta);
    const hits = [
      ...[...line.matchAll(TEXT_NODE)].map((m) => m[1]),
      ...[...line.matchAll(UI_ATTR)].map((m) => m[1]),
      ...[...line.matchAll(UI_ATTR_TEMPLATE)].map((m) => parteFixa(m[1])),
      ...[...line.matchAll(CHILDREN_TEMPLATE)].map((m) => parteFixa(m[1])),
      ...[...line.matchAll(ANTES_DA_EXPRESSAO)].map((m) => m[1]).filter((s) => !pareceCodigo(s)),
      ...[...line.matchAll(DEPOIS_DA_EXPRESSAO)].map((m) => m[1]).filter((s) => !pareceCodigo(s)),
      ...(noDeTextoSozinho(lines, i) ?? []),
    ];
    for (const text of hits) {
      if (PT.test(` ${text} `)) {
        found.push({ file: rel, line: i + 1, text: text.trim() });
      }
    }
  }
  return found;
}

function scanFile(absPath) {
  let src;
  try {
    src = readFileSync(absPath, "utf8");
  } catch {
    return []; // sumiu do disco / ilegível
  }
  return scanTexto(relative(ROOT, absPath).split(sep).join("/"), src);
}

/**
 * Retorna [{file, line, text}] de UI em PT fora de t().
 *
 * `opts.staged` limita aos arquivos de UI no stage e lê o BLOB DO ÍNDICE, que é o texto que vai
 * ser commitado — não o do disco, que pode já estar adiante dele.
 */
export function runAudit(opts = {}) {
  if (opts.staged) return stagedBlobs().flatMap(({ rel, src }) => scanTexto(rel, src));
  return listarArquivosDeUI().flatMap(scanFile);
}

/**
 * Entradas do baseline que o auditor NÃO encontra mais — string traduzida, arquivo apagado,
 * texto reescrito. Baseline que sobra é baseline que envelhece: a linha morta continua
 * liberando um texto que ninguém escreve mais, e o próximo que reintroduzir a mesma frase passa
 * batido. Só faz sentido na varredura completa (o `--staged` vê um recorte dos arquivos).
 */
export function baselineOrfa() {
  const orfas = [];
  for (const [rel, textos] of Object.entries(BASELINE_JS)) {
    const vivos = new Set(textosCrusDoArquivo(rel));
    for (const texto of textos) {
      if (!vivos.has(texto)) orfas.push({ file: rel, text: texto });
    }
  }
  return orfas;
}

/// Tudo que a varredura de `.js` apontaria no arquivo se o baseline não existisse.
function textosCrusDoArquivo(rel) {
  let src;
  try {
    src = readFileSync(join(ROOT, rel), "utf8");
  } catch {
    return []; // arquivo apagado ou renomeado: toda entrada dele é órfã
  }
  if (IGNORE_FILE.test(src)) return [];
  return varreduraJs(rel, src.split("\n"), { aplicarBaseline: false }).map((v) => v.text);
}

// CLI: `node scripts/i18nAudit.mjs [--staged]`
if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith("i18nAudit.mjs")) {
  const staged = process.argv.includes("--staged");
  const violations = runAudit({ staged });
  const orfas = staged ? [] : baselineOrfa();
  if (violations.length === 0 && orfas.length === 0) {
    console.log(`✓ i18n: nenhuma string de UI em português fora de t()${staged ? " (arquivos no stage)" : ""}.`);
    process.exit(0);
  }
  if (orfas.length > 0) {
    console.error(`\n✗ i18n: ${orfas.length} entrada(s) do baseline de .js que o auditor não acha mais:\n`);
    for (const o of orfas) console.error(`  ${o.file}  "${o.text}"`);
    console.error(`\nApague a(s) linha(s) em scripts/i18nBaseline.mjs — baseline que sobra volta a liberar`);
    console.error(`a frase quando alguém reintroduzir ela.\n`);
  }
  if (violations.length === 0) process.exit(1);
  console.error(`\n✗ i18n: ${violations.length} string(s) de UI em português ainda não traduzida(s):\n`);
  for (const v of violations) console.error(`  ${v.file}:${v.line}  "${v.text}"`);
  console.error(`\nEnvolva em t("chave") (+ adicione a chave nos dois common.json), ou marque como`);
  console.error(`intencional com {/* i18n-ignore */} na linha (ou // i18n-ignore-file no topo do arquivo).\n`);
  process.exit(1);
}
