// Detector de strings de UI em português ainda NÃO traduzidas (fora de `t()`/`i18n.t`).
//
// Objetivo: quando você adiciona/edita UI e esquece de passar por i18n, este scanner
// aponta arquivo:linha exatos para traduzir. Roda de 3 jeitos:
//   • no teste `src/i18n/i18nCoverage.test.js` (falha a suíte quando há pendência);
//   • no hook de pre-commit (`--staged`, checa só os .jsx no stage);
//   • na mão: `npm run i18n:audit`.
//
// COMO MARCAR EXCEÇÕES INTENCIONAIS (sem allowlist central):
//   • `i18n-ignore` em qualquer lugar da linha (ou na linha imediatamente acima) → pula a linha.
//     Em JSX use um comentário-expressão: {/* i18n-ignore */}
//   • `i18n-ignore-file` em qualquer lugar do arquivo → pula o arquivo inteiro
//     (ex.: telas de dados-de-exemplo/WIP, ou código morto).
//
// LIMITAÇÃO consciente: varre só `.jsx`. Prosa retornada de módulos `.js` (ex.: helpers de
// fato/display) NÃO é coberta aqui — essa continua no olho + no guard de paridade
// `localeParity.test.js` + no guard de acentos `scripts/tests/portuguese-copy-accents`.
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
import { execSync } from "node:child_process";

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

function listJsxFiles() {
  return readdirSync(SRC, { recursive: true })
    .map((p) => (typeof p === "string" ? p : p.toString()))
    .filter((p) => p.endsWith(".jsx") && !p.endsWith(".test.jsx"))
    .map((p) => join(SRC, p));
}

function stagedJsxFiles() {
  const out = execSync("git diff --cached --name-only --diff-filter=ACM", { cwd: ROOT, encoding: "utf8" });
  return out
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => s.startsWith("src/") && s.endsWith(".jsx") && !s.endsWith(".test.jsx"))
    .map((s) => join(ROOT, s));
}

function scanFile(absPath) {
  let src;
  try {
    src = readFileSync(absPath, "utf8");
  } catch {
    return []; // staged mas deletado / ilegível
  }
  if (IGNORE_FILE.test(src)) return [];
  const lines = src.split("\n");
  const found = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const prev = lines[i - 1] ?? "";
    if (/^\s*(\/\/|\*|\/\*)/.test(line)) continue; // linha de comentário
    if (/\bt\(/.test(line) || /i18n\.t/.test(line)) continue; // já traduzida
    if (IGNORE_LINE.test(line) || IGNORE_LINE.test(prev)) continue; // marcada
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
        found.push({ file: relative(ROOT, absPath).split(sep).join("/"), line: i + 1, text: text.trim() });
      }
    }
  }
  return found;
}

/** Retorna [{file, line, text}] de UI em PT fora de t(). `opts.staged` limita aos .jsx no stage. */
export function runAudit(opts = {}) {
  const files = opts.staged ? stagedJsxFiles() : listJsxFiles();
  return files.flatMap(scanFile);
}

// CLI: `node scripts/i18nAudit.mjs [--staged]`
if (import.meta.url === `file://${process.argv[1]}` || process.argv[1]?.endsWith("i18nAudit.mjs")) {
  const staged = process.argv.includes("--staged");
  const violations = runAudit({ staged });
  if (violations.length === 0) {
    console.log(`✓ i18n: nenhuma string de UI em português fora de t()${staged ? " (arquivos no stage)" : ""}.`);
    process.exit(0);
  }
  console.error(`\n✗ i18n: ${violations.length} string(s) de UI em português ainda não traduzida(s):\n`);
  for (const v of violations) console.error(`  ${v.file}:${v.line}  "${v.text}"`);
  console.error(`\nEnvolva em t("chave") (+ adicione a chave nos dois common.json), ou marque como`);
  console.error(`intencional com {/* i18n-ignore */} na linha (ou // i18n-ignore-file no topo do arquivo).\n`);
  process.exit(1);
}
