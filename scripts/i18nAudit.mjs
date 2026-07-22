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
// LIMITAÇÃO consciente: varre só `.jsx` (nós de texto JSX + atributos title/aria-label/
// placeholder/alt). Prosa retornada de módulos `.js` (ex.: helpers de fato/display) NÃO
// é coberta aqui — essa continua no olho + no guard de paridade `localeParity.test.js`.

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
