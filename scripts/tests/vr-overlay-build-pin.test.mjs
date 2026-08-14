// A layer de VR tem DOIS caminhos de build que ninguém executa junto, e é isso que os torna
// perigosos: o `scripts/build-vr-layer.mjs` (o que roda no `beforeBuildCommand` do Tauri e no
// CI) e o `vr-overlay/CMakeLists.txt` (o que quem mexe em C++ abre no Visual Studio). Cada
// ajuste feito num deles some no outro, e a divergência só aparece dentro do headset — a
// única superfície do app sem screenshot e sem log de build.
//
// Este guard trava os dois pontos em que a divergência é silenciosa e cara:
//
//   • o PIN dos headers do OpenXR, que precisa ser o mesmo SHA nos dois. E precisa ser SHA,
//     não tag: tag no git é ponteiro móvel, e uma release reapontada trocaria os headers do
//     build sem uma linha de diff em lugar nenhum;
//   • o `/utf-8`, sem o qual o cl lê o .cpp na codepage do sistema e todo acento das
//     mensagens vira mojibake em %TEMP%\iracer_overlay_layer.log — justo o arquivo que a
//     gente lê pra descobrir por que o overlay não apareceu.
//
// Também cobra que a escolha de formato de swapchain não volte a aceitar `formats[0]`: mandar
// o buffer RGBA num formato desconhecido não dá erro nenhum, só pinta lixo colorido no VR.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const script = ler("scripts/build-vr-layer.mjs");
const cmake = ler("vr-overlay/CMakeLists.txt");
const layerCpp = ler("vr-overlay/src/overlay_layer.cpp");

const SHA = /^[0-9a-f]{40}$/;

test("os dois caminhos de build pinam o MESMO commit do OpenXR-SDK", () => {
  const doScript = /const OPENXR_SHA = "([^"]+)"/.exec(script);
  assert.ok(doScript, "OPENXR_SHA sumiu de scripts/build-vr-layer.mjs");

  const doCmake = /GIT_TAG\s+(\S+)/.exec(cmake);
  assert.ok(doCmake, "GIT_TAG sumiu de vr-overlay/CMakeLists.txt");

  assert.match(doScript[1], SHA, "o pin do script precisa ser um SHA de 40 hex, não uma tag");
  assert.match(doCmake[1], SHA, "o GIT_TAG do CMake precisa ser um SHA de 40 hex, não uma tag");
  assert.equal(
    doCmake[1],
    doScript[1],
    "o CMake e o script de build compilariam contra headers diferentes do OpenXR",
  );
});

test("o script busca o SHA pinado e reconcilia clone existente", () => {
  // `git clone --branch <sha>` não existe: o git só clona por ref. Buscar o commit exige
  // init + fetch + checkout, e um clone que já estava em outro commit (feito antes do pin,
  // ou com a tag ainda móvel) precisa ser trazido pro pin — senão a máquina de quem clonou
  // antes compila contra headers diferentes dos do CI, em silêncio.
  assert.match(script, /"fetch",\s*"--depth",\s*"1",\s*"origin",\s*OPENXR_SHA/);
  assert.match(script, /"checkout",[\s\S]{0,40}OPENXR_SHA/);
  assert.ok(
    /rev-parse", "HEAD"/.test(script),
    "o script precisa conferir o HEAD do clone existente contra o pin",
  );
});

test("os dois caminhos de build compilam com /utf-8", () => {
  assert.ok(/\/utf-8/.test(script), "falta /utf-8 na linha do cl em scripts/build-vr-layer.mjs");
  assert.ok(
    /target_compile_options\([^)]*\/utf-8/.test(cmake),
    "falta /utf-8 nas opções do MSVC em vr-overlay/CMakeLists.txt",
  );
});

test("a layer não aceita um formato de swapchain que ela não saiba preencher", () => {
  // O fallback antigo era `return formats[0]` — o primeiro que o runtime oferecesse, que pode
  // ser 10-bit, half-float ou typeless. O UpdateSubresource aceita o buffer do mesmo jeito e o
  // resultado é lixo colorido dentro do headset, sem uma linha de erro em lugar nenhum.
  assert.ok(
    !/return formats\[0\]/.test(layerCpp),
    "PickColorFormat voltou a cair no primeiro formato ofertado pelo runtime",
  );
  assert.ok(
    /kKnownFormats/.test(layerCpp),
    "a lista de formatos 32-bit conhecidos (kKnownFormats) sumiu de overlay_layer.cpp",
  );
});
