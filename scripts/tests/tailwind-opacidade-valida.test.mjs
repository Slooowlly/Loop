// O Tailwind v3 só gera o modificador de opacidade que estiver na escala do tema
// (`0, 5, 10, ... 100`). Uma classe como `border-white/8` não é erro em lugar
// nenhum: ela some do CSS em silêncio, e o elemento fica com o que herdou. Em
// borda isso quase engana, porque a cor herdada é o `border.DEFAULT` do preflight
// e ela lembra um branco de 8%. Em `bg` some o painel, e em `stroke="currentColor"`
// a grade discreta vira branco total (ver o comentário do EfficiencyScatter).
//
// Em 12/08/2026 havia 144 dessas no `src/`. A saída é o valor arbitrário
// (`border-white/[0.08]`), que o Tailwind gera para qualquer número e que o
// código já usava na maioria dos lugares.
//
// O segundo teste não confia em leitura de texto: ele manda o Tailwind compilar
// as utilities de verdade e cobra que cada classe encontrada esteja no CSS. É o
// que pega o caso que a varredura estática não vê, o da cor que não existe —
// `bg-accent-secondary/12` tinha opacidade inválida E cor não declarada.
import test from "node:test";
import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import postcss from "postcss";
import tailwindcss from "tailwindcss";
import resolveConfig from "tailwindcss/resolveConfig.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");
const srcRoot = path.join(projectRoot, "src");

// Utilitários que aceitam cor e, portanto, aceitam o modificador de opacidade.
// `w`, `h`, `basis` e afins ficam de fora de propósito: em `w-1/3` a barra é
// fração de largura, não opacidade.
const UTILITARIOS_DE_COR = [
  "bg",
  "text",
  "border",
  "border-[trblxyse]",
  "divide",
  "divide-[xy]",
  "ring",
  "ring-offset",
  "from",
  "via",
  "to",
  "shadow",
  "fill",
  "stroke",
  "outline",
  "decoration",
  "placeholder",
  "caret",
  "accent",
].join("|");

// A cor precisa começar por letra para o padrão não morder `from-0%` nem
// `w-1/3`. Cor em colchete (`bg-[#f2c46d]/70`) fica fora porque o alvo aqui é a
// cor nomeada do tema.
//
// O prefixo de variante entra na captura porque o Tailwind emite a classe
// inteira: de `hover:bg-status-yellow/30` sai `.hover\:bg-status-yellow\/30:hover`,
// e a classe base sozinha não existe no CSS.
const VARIANTES = "(?:(?:[a-zA-Z0-9_-]+|\\[[^\\]\\s]*\\])(?:\\[[^\\]\\s]*\\])?:)*";
const CLASSE_COM_OPACIDADE = new RegExp(
  `(?<![\\w:.-])(${VARIANTES})((?:${UTILITARIOS_DE_COR})-[a-zA-Z][a-zA-Z0-9-]*)\\/(\\[[^\\]]+\\]|\\d{1,3})\\b`,
  "g",
);

// Linha de comentário não é classe. Os comentários do CarPartsRadar e do
// EfficiencyScatter citam `text-white/8` e `text-white/6` justamente para contar
// como esse bug se manifestou — acusá-los apagaria o registro da falha.
function ehComentario(linha) {
  const inicio = linha.trimStart();
  return inicio.startsWith("//") || inicio.startsWith("*") || inicio.startsWith("/*");
}

async function arquivosDeFonte(diretorio) {
  const entradas = await readdir(diretorio, { withFileTypes: true });
  const encontrados = [];
  for (const entrada of entradas) {
    const alvo = path.join(diretorio, entrada.name);
    if (entrada.isDirectory()) encontrados.push(...(await arquivosDeFonte(alvo)));
    else if (/\.(jsx|js)$/.test(entrada.name)) encontrados.push(alvo);
  }
  return encontrados;
}

async function varrerClasses() {
  const achados = [];
  for (const arquivo of await arquivosDeFonte(srcRoot)) {
    const linhas = (await readFile(arquivo, "utf8")).split("\n");
    linhas.forEach((linha, indice) => {
      if (ehComentario(linha)) return;
      for (const casamento of linha.matchAll(CLASSE_COM_OPACIDADE)) {
        achados.push({
          classe: `${casamento[1]}${casamento[2]}/${casamento[3]}`,
          utilitario: `${casamento[1]}${casamento[2]}`,
          modificador: casamento[3],
          local: `${path.relative(projectRoot, arquivo).replace(/\\/g, "/")}:${indice + 1}`,
        });
      }
    });
  }
  return achados;
}

async function escalaDeOpacidade() {
  const { default: configDoProjeto } = await import(
    new URL(`file://${path.join(projectRoot, "tailwind.config.js").replace(/\\/g, "/")}`).href
  );
  return new Set(Object.keys(resolveConfig(configDoProjeto).theme.opacity));
}

test("nenhuma classe usa opacidade fora da escala do Tailwind", async () => {
  const achados = await varrerClasses();
  const escala = await escalaDeOpacidade();

  assert.ok(achados.length > 0, "nenhuma classe com opacidade encontrada em src/ — o parser ficou cego");

  const invalidas = achados.filter(
    (achado) => !achado.modificador.startsWith("[") && !escala.has(achado.modificador),
  );

  const relatorio = invalidas
    .map((achado) => {
      const numero = Number(achado.modificador);
      const decimal = numero < 10 ? `0.0${numero}` : `0.${achado.modificador}`;
      return `  ${achado.local}  ${achado.classe} -> ${achado.utilitario}/[${decimal}]`;
    })
    .join("\n");

  assert.equal(
    invalidas.length,
    0,
    `opacidade fora da escala (${[...escala].join(", ")}) não gera CSS nenhum.\n` +
      `Use o valor arbitrário equivalente:\n${relatorio}`,
  );
});

test("toda classe de cor com opacidade chega ao CSS compilado", async () => {
  const achados = await varrerClasses();
  const { default: configDoProjeto } = await import(
    new URL(`file://${path.join(projectRoot, "tailwind.config.js").replace(/\\/g, "/")}`).href
  );

  const resultado = await postcss([
    tailwindcss({
      ...configDoProjeto,
      content: [path.join(srcRoot, "**/*.{js,jsx,ts,tsx}").replace(/\\/g, "/")],
    }),
  ]).process("@tailwind utilities;", { from: undefined });

  const css = resultado.css;
  const ausentes = [];
  const jaVistas = new Set();

  for (const achado of achados) {
    if (jaVistas.has(achado.classe)) continue;
    jaVistas.add(achado.classe);
    // O seletor sai do Tailwind com os caracteres especiais escapados:
    // `.border-white\/\[0\.08\]`.
    const seletor = `.${achado.classe.replace(/[.[\]/#()%+,:&>~*=!]/g, (caractere) => `\\${caractere}`)}`;
    if (!css.includes(seletor)) ausentes.push(`  ${achado.local}  ${achado.classe}`);
  }

  assert.equal(
    ausentes.length,
    0,
    "estas classes não existem no CSS compilado — a cor não está declarada no tema, " +
      `ou a opacidade está fora da escala:\n${ausentes.join("\n")}`,
  );
});
