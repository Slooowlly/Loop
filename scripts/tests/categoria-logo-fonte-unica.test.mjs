// O brasão de categoria tem um dicionário só, e ele aponta para arquivo que existe.
//
// Este mapa vivia copiado em cinco arquivos do frontend (torre do overlay, atlas de equipes,
// pré-temporada, convocação e calendário), cada cópia conhecendo um subconjunto diferente de
// categorias. Categoria nova entrava em duas telas e nascia sem brasão nas outras três, e a
// falha é MUDA: `?? null` devolve nada, o componente não desenha o selo e ninguém vê erro.
//
// Unificado em `src/utils/categoryLogos.js` em 11/08/2026. Este guard trava as duas metades:
// que a cópia não volte, e que todo id declarado tenha arte em disco nas duas variantes.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const fonte = ler("src/utils/categoryLogos.js");

/// Os ids e nomes de arquivo declarados no dicionário único.
function dicionario() {
  const bloco = /const ARQUIVO_POR_CATEGORIA = \{([\s\S]*?)\n\};/.exec(fonte);
  assert.ok(bloco, "ARQUIVO_POR_CATEGORIA sumiu de src/utils/categoryLogos.js");
  const pares = [...bloco[1].matchAll(/^\s*(\w+): "([^"]+)",/gm)].map(([, id, arq]) => [id, arq]);
  // Um guard que não acha o que procura precisa gritar. Se a extração passar a casar zero
  // entradas, os testes abaixo passariam vazios e a proteção sumiria em silêncio.
  assert.ok(pares.length >= 10, `só ${pares.length} categorias extraídas — a extração furou`);
  return pares;
}

test("toda categoria do dicionário tem arte nas duas variantes", () => {
  // As duas pastas servem a coisas diferentes e nenhuma é opcional: `recortadas/` é o selo
  // pequeno (atlas, torre) e a raiz é o selo grande do calendário. Arte só numa delas deixa
  // metade das telas sem brasão.
  const faltando = [];
  for (const [id, arquivo] of dicionario()) {
    for (const pasta of ["public/utilities/categorias", "public/utilities/categorias/recortadas"]) {
      const alvo = path.join(raiz, pasta, `${arquivo}.webp`);
      if (!fs.existsSync(alvo)) faltando.push(`${id} -> ${pasta}/${arquivo}.webp`);
    }
  }
  assert.deepEqual(faltando, [], `categorias sem arte em disco: ${faltando}`);
});

test("as 9 categorias da escada do Rust têm brasão no frontend", () => {
  // O id vem do banco, não do JS. A lista canônica é `CATEGORIES` em constants/categories.rs;
  // uma categoria que existe lá e não aqui é exatamente o caso que a duplicação escondia —
  // e agora ele quebra o guard em vez de nascer sem selo em três telas.
  const rs = ler("src-tauri/src/constants/categories.rs");
  const tabela = /pub static CATEGORIES: \[CategoryConfig; (\d+)\] = \[([\s\S]*?)\n\];/.exec(rs);
  assert.ok(tabela, "pub static CATEGORIES sumiu de constants/categories.rs");
  const ids = [...tabela[2].matchAll(/^\s*id: "([a-z0-9_]+)",/gm)].map(([, id]) => id);
  assert.equal(ids.length, Number(tabela[1]), "a extração não achou todas as entradas da escada");
  const conhecidos = new Set(dicionario().map(([id]) => id));
  const semLogo = ids.filter((id) => !conhecidos.has(id));
  assert.deepEqual(semLogo, [], `categorias do Rust sem brasão no frontend: ${semLogo}`);
});

test("nenhum outro arquivo do frontend remonta o caminho do brasão na mão", () => {
  // O modo de reincidência é copiar a string do caminho para uma tela nova em vez de importar.
  // Só `categoryLogos.js` pode montar `/utilities/categorias/...`; o resto consulta.
  const proibidos = [];
  const varrer = (dir) => {
    for (const nome of fs.readdirSync(path.join(raiz, dir))) {
      const rel = `${dir}/${nome}`;
      if (fs.statSync(path.join(raiz, rel)).isDirectory()) {
        varrer(rel);
        // O teste que assevera o `src` renderizado PRECISA do literal — é a asserção dele.
      } else if (/\.(js|jsx)$/.test(nome) && !/\.test\.(js|jsx)$/.test(nome) && rel !== "src/utils/categoryLogos.js") {
        const texto = fs.readFileSync(path.join(raiz, rel), "utf8");
        // Só linhas de CÓDIGO: o caminho citado dentro de comentário é documentação, não cópia.
        for (const linha of texto.split("\n")) {
          const semComentario = linha.replace(/\/\/.*$/, "");
          if (/["'`]\/utilities\/categorias\//.test(semComentario)) proibidos.push(`${rel}: ${linha.trim()}`);
        }
      }
    }
  };
  varrer("src");
  assert.deepEqual(
    proibidos,
    [],
    `caminho de brasão remontado fora do dicionário único:\n${proibidos.join("\n")}`,
  );
});
