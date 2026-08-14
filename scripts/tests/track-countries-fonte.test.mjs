// O gerador do mapa de países já produziu um arquivo VAZIO em silêncio: `constants/tracks.rs`
// virou fachada de módulo, o catálogo mudou para `constants/tracks/dados.rs`, e o script seguiu
// lendo a fachada. Zero pista casada, zero erro, `trackCountries.js` sobrescrito com um mapa sem
// nenhuma chave — e o sintoma no jogo é só a bandeira que sumiu da tela.
//
// Estes guards fecham as duas metades: a fonte é o arquivo com os dados, e extração que não
// fecha com o que o arquivo declara mata a geração em vez de gravar o buraco.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { SRC, extrairMapa, montarArquivo } from "../gen-track-countries.mjs";

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const FACHADA = join(RAIZ, "src-tauri", "src", "constants", "tracks.rs");

test("a fonte apontada existe e é a que tem os dados, não a fachada", () => {
  assert.ok(existsSync(SRC), `fonte inexistente: ${SRC}`);
  assert.match(SRC.replace(/\\/g, "/"), /constants\/tracks\/dados\.rs$/);
  assert.match(readFileSync(SRC, "utf8"), /\bnome:\s*"/);
});

test("a fonte real gera um mapa cheio, com nome completo, local e nome curto", () => {
  const map = extrairMapa(readFileSync(SRC, "utf8"));
  const chaves = Object.keys(map).length;
  assert.ok(chaves >= 100, `só ${chaves} chaves — o catálogo tem mais de 100 pistas`);
  assert.equal(map["Algarve International Circuit - Grand Prix"], "🇵🇹 Portugal"); // nome completo
  assert.equal(map["Algarve International Circuit"], "🇵🇹 Portugal"); // local (o que o track_name guarda)
  assert.equal(map["Portimão"], "🇵🇹 Portugal"); // nome curto
});

test("a fachada tracks.rs não passa por fonte — é ela que produzia o mapa vazio", () => {
  assert.ok(existsSync(FACHADA));
  assert.throws(() => extrairMapa(readFileSync(FACHADA, "utf8")), /piso|declara/i);
});

test("fonte vazia ou sem entradas morre em vez de gerar mapa vazio", () => {
  assert.throws(() => extrairMapa(""), /piso|declara/i);
  assert.throws(() => extrairMapa("pub static TRACKS: &[TrackInfo] = &[];"), /piso|declara/i);
});

test("entrada que perdeu o país derruba a geração", () => {
  // 60 entradas boas (o piso) e uma sem `pais`: o total declarado passa, o pareamento não.
  const boa = (i) => `TrackInfo { nome: "Pista ${i}", nome_curto: "P${i}", pais: "🇧🇷 Brasil" },\n`;
  let fonte = "";
  for (let i = 0; i < 60; i += 1) fonte += boa(i);
  fonte += `TrackInfo { nome: "Pista sem país", nome_curto: "Órfã" },\n`;
  assert.throws(() => extrairMapa(fonte), /par\(es\) nome→país|perdeu o campo/i);
});

test("o arquivo gerado sai ordenado e com o cabeçalho de não editar à mão", () => {
  const texto = montarArquivo({ Zolder: "🇧🇪 Bélgica", Adelaide: "🇦🇺 Austrália" });
  assert.match(texto, /GERADO por scripts\/gen-track-countries\.mjs/);
  assert.ok(texto.indexOf('"Adelaide"') < texto.indexOf('"Zolder"'), "as chaves saem ordenadas");
});
