// O contrato das chaves do spotter, lido como texto nos três lugares onde ele vive.
//
// Uma fala do spotter atravessa três arquivos que não se conhecem: o Rust emite uma
// chave, `spotter-pack.mjs` grava um `.wav` com aquele nome, e `spotterVoice.js` decide
// quem pode interromper quem. Nada no build liga os três — uma chave nova que só existe
// em dois deles compila, passa nos testes e falha em silêncio na pista, que é o pior
// lugar possível para descobrir.
//
// Este guard fecha o triângulo.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const pack = ler("scripts/spotter-pack.mjs");
const voz = ler("src/lib/spotterVoice.js");
const spotterRs = ler("src-tauri/src/iracing_sdk/spotter.rs");
const frenteRs = ler("src-tauri/src/iracing_sdk/spotter_frente.rs");
const trasRs = ler("src-tauri/src/iracing_sdk/spotter_tras.rs");
const bandeiraRs = ler("src-tauri/src/iracing_sdk/spotter_bandeira.rs");
const voltarRs = ler("src-tauri/src/iracing_sdk/spotter_voltar.rs");
const boxeRs = ler("src-tauri/src/iracing_sdk/spotter_boxe.rs");
const climaRs = ler("src-tauri/src/iracing_sdk/spotter_clima.rs");
// `spotter_lento.rs` fica DE FORA de propósito: o módulo existe, compila e é testado, mas
// não está fiado em `spotter.rs` e não emite fala nenhuma hoje. Incluí-lo aqui exigiria
// gravar `.wav` para uma família que decidimos não usar ainda — ver a ordem de confiança
// em `docs/spotter-obstaculo.md`. Quando ele for fiado, acrescente-o nesta linha.

/** As chaves declaradas no bloco `const FALAS = { ... }` do gerador. */
function falasDoPacote() {
  const bloco = /const FALAS = \{([\s\S]*?)\n\};/.exec(pack);
  assert.ok(bloco, "não achei o bloco FALAS em scripts/spotter-pack.mjs");
  return [...bloco[1].matchAll(/^\s{2}([a-z_0-9]+):\s*"/gm)].map((m) => m[1]);
}

/** As chaves que o Rust emite: `pub const CHAVE_* : &str = "..."` mais as da vizinhança. */
function chavesDoRust() {
  const consts = [
    ...`${spotterRs}${frenteRs}${trasRs}${bandeiraRs}${voltarRs}${boxeRs}${climaRs}`.matchAll(
      /CHAVE_[A-Z_]+: &str = "([a-z_0-9]+)"/g,
    ),
  ].map(
    (m) => m[1],
  );
  const vizinhanca = [...spotterRs.matchAll(/Vizinhanca::\w+ => "([a-z_0-9]+)"/g)].map((m) => m[1]);
  return [...new Set([...consts, ...vizinhanca])].filter((c) => c !== "desligado");
}

/** As chaves com prioridade declarada em spotterVoice.js. */
function chavesComPrioridade() {
  const bloco = /const PRIORIDADE = \{([\s\S]*?)\n\};/.exec(voz);
  assert.ok(bloco, "não achei a tabela PRIORIDADE em src/lib/spotterVoice.js");
  return [...bloco[1].matchAll(/^\s{2}([a-z_0-9]+):\s*(\d+)/gm)].map((m) => [m[1], Number(m[2])]);
}

test("toda chave que o Rust emite tem uma fala escrita no pacote", () => {
  const falas = new Set(falasDoPacote());
  const faltando = chavesDoRust().filter((c) => !falas.has(c));
  assert.deepEqual(faltando, [], `chaves sem redação em spotter-pack.mjs: ${faltando}`);
});

test("toda chave que o Rust emite tem prioridade declarada na voz", () => {
  // Sem prioridade, a chave cai no padrão — que é o do LEMBRETE, o degrau mais baixo.
  // Um aviso de segurança novo herdando a prioridade de um preenchimento de silêncio é
  // exatamente o defeito que não se percebe ouvindo.
  const prioridades = new Map(chavesComPrioridade());
  const semPrioridade = chavesDoRust()
    .filter((c) => c !== "teste")
    .filter((c) => !prioridades.has(c));
  assert.deepEqual(semPrioridade, [], `chaves sem prioridade: ${semPrioridade}`);
});

test("a ordem de prioridade respeita o projeto do rádio", () => {
  const p = new Map(chavesComPrioridade());
  const entrada = ["esquerda", "direita", "tres_largos", "duas_esquerda", "duas_direita"];
  const liberacao = ["livre", "livre_esquerda", "livre_direita", "livre_atras"];
  const lembrete = ["ainda_esquerda", "ainda_direita", "ainda_ai", "ainda_atras"];
  const retorno = ["segura_volta", "pode_voltar"];

  const min = (lista) => Math.min(...lista.map((c) => p.get(c)));
  const max = (lista) => Math.max(...lista.map((c) => p.get(c)));

  // O RETORNO está acima de tudo, inclusive do lateral. Com o estado aberto o piloto está
  // parado ou de ré, cego, decidindo se entra na pista: um "esquerda" cortando um "segura"
  // é a troca exata que mata. As duas do par ficam no mesmo degrau — a liberação aqui é
  // tão urgente quanto a ocupação, porque segurar o piloto parado a mais também custa.
  assert.ok(min(retorno) > max(entrada), "entrada lateral acima do retorno à pista");
  assert.equal(p.get("segura_volta"), p.get("pode_voltar"), "o par do retorno em degraus diferentes");
  // O carro que CHEGOU ao lado é o que machuca agora — nada passa na frente dele.
  assert.ok(min(entrada) > p.get("carro_fora_frente"), "obstáculo à frente acima da entrada lateral");
  // O obstáculo é notícia; a liberação e o lembrete não podem atropelá-lo.
  assert.ok(p.get("carro_fora_frente") > max(liberacao), "liberação acima do obstáculo");
  // As duas famílias de obstáculo à frente são o mesmo assunto: mesmo degrau, sempre.
  // Separá-las faria a mais baixa esperar a mais alta terminar por um carro que já não é
  // o mais próximo — o detector só anuncia um obstáculo por vez, e é o primeiro que o
  // piloto vai encontrar.
  for (const c of ["carro_parado_frente", "carro_saindo_box"]) {
    assert.equal(p.get(c), p.get("carro_fora_frente"), `${c} em degrau diferente do obstáculo`);
  }
  assert.ok(min(liberacao) > max(lembrete), "lembrete acima da liberação");

  // O tráfego por trás fica ENTRE o obstáculo e a liberação, e nenhuma família rara pode
  // dividir degrau com o lateral. Medido: `carro_atras` sai 8 vezes em 82 pilotos-corrida
  // e o lateral fala dezenas a centenas de vezes por corrida — no mesmo degrau, "mais nova
  // ganha" apagaria a rara por volume, sem erro de arbitragem nenhum.
  assert.ok(p.get("carro_fora_frente") > p.get("carro_atras"), "trás acima do obstáculo à frente");
  assert.ok(p.get("carro_atras") > max(liberacao), "liberação acima do tráfego por trás");
  assert.ok(min(entrada) > p.get("carro_atras"), "tráfego por trás acima da entrada lateral");
});

test("a liberação de trás não tem variação, como a liberação lateral", () => {
  // Entendida errado, a liberação faz o piloto voltar a atacar com alguém ainda em cima
  // dele. É o único lugar do pacote onde variar troca clareza por variedade sem ganho.
  const falas = new Set(falasDoPacote());
  for (const c of ["livre", "livre_atras", "pode_voltar"]) {
    assert.ok(falas.has(c), `falta a redação de ${c}`);
    assert.ok(!falas.has(`${c}_2`), `${c} não deveria ter variação`);
  }
});

test("toda fala do pacote tem quem a emita", () => {
  // A direção inversa do primeiro teste, e ela faltava. Sem isto, uma fala pode sobreviver
  // à remoção do detector que a emitia: continua gravada, continua no bundle, e nunca toca.
  // O sintoma é caro de achar porque é silêncio — indistinguível de "não houve o evento".
  //
  // `spotter_lento.rs` entra na conta de propósito: as chaves dele existem no fonte e o
  // módulo não está fiado, então elas NÃO devem ter redação no pacote. Se um dia tiverem
  // sem que o módulo entre em `spotter.rs`, é este teste que reclama.
  const emitidas = new Set(chavesDoRust());
  const orfas = [...new Set(falasDoPacote().map((c) => c.replace(/_\d+$/, "")))]
    .filter((c) => c !== "teste")
    .filter((c) => !emitidas.has(c));
  assert.deepEqual(orfas, [], `falas que ninguém emite: ${orfas}`);
});

test("as variações do pacote pertencem a uma chave base que existe", () => {
  const falas = falasDoPacote();
  const bases = new Set(falas.filter((c) => !/_\d+$/.test(c)));
  const orfas = falas.filter((c) => /_\d+$/.test(c)).filter((c) => !bases.has(c.replace(/_\d+$/, "")));
  assert.deepEqual(orfas, [], `variações sem chave base: ${orfas}`);
});
