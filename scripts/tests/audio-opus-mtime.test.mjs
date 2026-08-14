// O conversor do acervo de voz decidia por EXISTÊNCIA: tinha `.opus` ao lado, pulava. Só que
// os geradores de peça reescrevem o `.wav` no lugar, com o mesmo nome — então a peça regravada
// ficava para sempre com o Opus antigo ao lado, o script anunciava "0 a converter" e o app
// tocava no jogo exatamente a fala que tinha acabado de ser corrigida.
//
// Aqui o critério é mtime, e é isso que estes guards medem.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { precisaConverter, pendentesDoAcervo } from "../audio-para-opus.mjs";

/** Cria o par wav/opus num diretório temporário, com os mtimes que o caso pede. */
function acervoDeTeste(pares) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "loop-opus-"));
  for (const { nome, wavEm, opusEm } of pares) {
    const wav = path.join(dir, `${nome}.wav`);
    fs.writeFileSync(wav, "RIFF");
    fs.utimesSync(wav, new Date(wavEm), new Date(wavEm));
    if (opusEm != null) {
      const opus = path.join(dir, `${nome}.opus`);
      fs.writeFileSync(opus, "OggS");
      fs.utimesSync(opus, new Date(opusEm), new Date(opusEm));
    }
  }
  return dir;
}

const T0 = 1_700_000_000_000; // instante fixo: o teste não pode depender do relógio

test("sem Opus ao lado, converte", () => {
  const dir = acervoDeTeste([{ nome: "a", wavEm: T0 }]);
  assert.equal(precisaConverter(path.join(dir, "a.wav"), path.join(dir, "a.opus")), true);
});

test("WAV mais NOVO que o Opus, converte de novo", () => {
  const dir = acervoDeTeste([{ nome: "a", wavEm: T0 + 60_000, opusEm: T0 }]);
  assert.equal(precisaConverter(path.join(dir, "a.wav"), path.join(dir, "a.opus")), true);
});

test("Opus mais novo que o WAV, não mexe", () => {
  const dir = acervoDeTeste([{ nome: "a", wavEm: T0, opusEm: T0 + 60_000 }]);
  assert.equal(precisaConverter(path.join(dir, "a.wav"), path.join(dir, "a.opus")), false);
});

test("mtimes iguais é o caso normal de conversão recém-feita, não reconverte", () => {
  const dir = acervoDeTeste([{ nome: "a", wavEm: T0, opusEm: T0 }]);
  assert.equal(precisaConverter(path.join(dir, "a.wav"), path.join(dir, "a.opus")), false);
});

test("--forcar reconverte mesmo com o Opus em dia", () => {
  const dir = acervoDeTeste([{ nome: "a", wavEm: T0, opusEm: T0 + 60_000 }]);
  assert.equal(precisaConverter(path.join(dir, "a.wav"), path.join(dir, "a.opus"), true), true);
});

test("o acervo separa o que está em dia do que ficou para trás", () => {
  const dir = acervoDeTeste([
    { nome: "em_dia", wavEm: T0, opusEm: T0 + 60_000 },
    { nome: "regravada", wavEm: T0 + 60_000, opusEm: T0 },
    { nome: "nova", wavEm: T0 },
  ]);
  const { wavs, pendentes } = pendentesDoAcervo(dir);
  assert.equal(wavs, 3);
  const nomes = pendentes.map((p) => path.basename(p.entrada)).sort();
  assert.deepEqual(nomes, ["nova.wav", "regravada.wav"]);
});
