#!/usr/bin/env node
// AUDIÇÃO da fala de quebra montada com as peças de PRODUÇÃO.
//
// `audicao-quebra.mjs` gerou peças novas só para decidir se a colagem serve. Este monta com o
// que está em `src/assets/engenheiro/` e com as pausas de `pausasDoRadio` — é a fala exata
// que sai na corrida, nada gerado na hora.
//
// A diferença importa: as peças de produção já vêm com a cadeia de rádio gravada dentro, e
// colar duas peças JÁ filtradas é justamente o que a POC disse para não fazer com material
// novo. Aqui não há escolha (é o que o app faz em tempo real, um buffer atrás do outro), e é
// por isso que esta audição precisa existir separada da outra.
//
// Uso: node scripts/tts-poc/audicao-quebra-montada.mjs

import fs from "node:fs";
import path from "node:path";

import { escreverWav, lerWav } from "./filtro-radio.mjs";
import { pausasDoRadio } from "../../src/lib/pausasDoRadio.js";

const ORIGEM = path.join("src", "assets", "engenheiro");
const DESTINO = path.join("docs", "tts-poc", "audicao-quebra-montada");

// As formas que o montador do Rust produz, uma de cada tipo. As chaves são as mesmas que
// `quebra::montar` devolve — copiadas dos testes dele.
const FORMAS = [
  {
    chave: "rival-heavy",
    pecas: ["ab_rival", "nm_cooper", "qb_heavy_engine_0"],
    nota: "rival, quebra grave",
  },
  {
    chave: "nemesis-dnf",
    pecas: ["ab_nemesis", "nm_wisniewski", "qb_dnf_gearbox_1", "co_ummenos"],
    nota: "nêmesis abandona — com coda",
  },
  {
    chave: "lider-dnf",
    pecas: ["nm_cooper", "ap_lider", "qb_dnf_engine_0", "co_otima"],
    nota: "líder do campeonato abandona",
  },
  {
    chave: "pontos-dnf",
    pecas: ["nm_silva", "ap_frente", "qb_dnf_suspension_2", "co_ajuda"],
    nota: "vizinho de pontos à frente abandona",
  },
  {
    chave: "companheiro-heavy",
    pecas: ["ab_companheiro", "nm_bianchi", "qb_heavy_brakes_2"],
    nota: "companheiro de equipe, sem coda",
  },
  {
    chave: "equipe-dnf",
    pecas: ["ab_piloto2", "eq_kitsune", "qb_dnf_cooling_0"],
    nota: "sem vínculo — pela equipe, sem nome",
  },
  {
    chave: "equipe-longa",
    pecas: ["ab_piloto1", "eq_grid_start", "qb_heavy_underbody_1"],
    nota: "nome de equipe longo, o pior caso de duração",
  },
  {
    chave: "leve-desconhecido",
    pecas: ["ab_piloto2", "eq_ferrari", "qb_light_brakes_0"],
    nota: "quebra LEVE de desconhecido — antes ficava muda",
  },
  {
    chave: "atrito",
    pecas: ["ab_piloto1", "eq_prema", "qb_dnf_electronics_2", "co_atrito"],
    nota: "o quarto abandono da corrida — comentário de atrito",
  },
  // As duas do NOSSO carro são peça única: frase inteira, sem emenda nenhuma. Entram nesta
  // audição mesmo assim porque o que se julga aqui é o conjunto — elas dividem o mesmo canal
  // com as de cima, e é ouvindo em sequência que se percebe se a voz é a mesma pessoa.
  {
    chave: "meu-carro",
    pecas: ["meu_suspension_1"],
    nota: "peça NOSSA na janela de risco",
  },
  {
    chave: "poupar",
    pecas: ["meu_poupar"],
    nota: "peça nossa no limite + corrida derrubando gente",
  },
  // ── Rádio de RITMO ──
  {
    chave: "ritmo-volta",
    pecas: ["tv_volta_em", "t_924"],
    nota: "resposta ao push-to-talk: qual foi minha volta",
  },
  {
    chave: "ritmo-volta-e-falta",
    pecas: ["tv_volta_em", "t_924", "tv_faltam_4"],
    nota: "a volta E o quanto falta para a melhor da corrida",
  },
  {
    chave: "ritmo-melhor-de-outro",
    pecas: ["tv_melhor_e_do", "nm_cooper", "t_924"],
    nota: "a volta mais rápida da corrida trocou de dono",
  },
  {
    chave: "ritmo-melhor-longa",
    pecas: ["tv_melhor_e_do", "nm_wisniewski", "t_1876"],
    nota: "pista longa (3:07,6) com sobrenome difícil",
  },
  {
    chave: "ritmo-tomamos",
    pecas: ["tv_tomamos"],
    nota: "NÓS cravamos a volta mais rápida",
  },
  // ── Duas quebras num instante só ──
  {
    chave: "dupla-dnf",
    pecas: ["nm_cooper", "conj_e", "nm_bianchi", "qb_dupla_dnf_0"],
    nota: "dois abandonos na mesma volta, numa fala só",
  },
  {
    chave: "dupla-grave",
    pecas: ["nm_silva", "conj_e", "nm_wisniewski", "qb_dupla_heavy_1"],
    nota: "duas quebras graves, com sobrenome difícil",
  },
  {
    chave: "dupla-leve",
    pecas: ["nm_takahashi", "conj_e", "nm_roux", "qb_dupla_light_0"],
    nota: "duas leves — a forma mais curta da fusão",
  },
];
fs.mkdirSync(DESTINO, { recursive: true });

let taxa = 0;
const faltando = new Set();
for (const forma of FORMAS) {
  const partes = forma.pecas.map((c) => {
    const f = path.join(ORIGEM, `${c}.wav`);
    if (!fs.existsSync(f)) {
      faltando.add(c);
      return null;
    }
    const { amostras, taxa: t } = lerWav(f);
    taxa = t;
    return amostras;
  });
  if (partes.some((p) => p === null)) continue;

  const pausas = pausasDoRadio(forma.pecas).map((ms) => Math.round((ms / 1000) * taxa));
  const total = partes.reduce((s, a) => s + a.length, 0) + pausas.reduce((s, n) => s + n, 0);
  const junto = new Float32Array(total);
  let cursor = 0;
  partes.forEach((a, i) => {
    junto.set(a, cursor);
    cursor += a.length + (pausas[i] ?? 0);
  });
  // SEM `aplicarRadio`: as peças de produção já saíram do gerador com a cadeia gravada
  // dentro, e filtrar de novo empilharia dois compressores.
  escreverWav(path.join(DESTINO, `${forma.chave}.wav`), junto, taxa);
  console.log(
    `  ${forma.chave.padEnd(20)} ${(junto.length / taxa).toFixed(2)}s  ` +
      `${forma.pecas.length} peças, pausas [${pausasDoRadio(forma.pecas).join(", ")}] ms  — ${forma.nota}`,
  );
}

console.log(`\n${DESTINO}`);
if (faltando.size) console.warn(`  ⚠  peças ausentes: ${[...faltando].join(", ")}`);
