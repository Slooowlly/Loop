// O VOLUME DO RÁDIO — um só para as duas bocas.
//
// O spotter e o engenheiro são a mesma pessoa no mesmo canal. Dois controles seriam duas
// formas de deixá-los em alturas diferentes, e a primeira vez que isso acontecesse o
// jogador ouviria o mesmo sujeito trocar de rádio no meio da corrida.
//
// ## Por que o padrão não é 1,0
//
// Medido nos 3.216 arquivos do acervo: RMS 0,175 e pico 0,966 — quase escala cheia, e
// achatado por um compressor 8:1. Esse nível está CERTO dentro do arquivo: é o que faz a
// voz atravessar o ruído de motor sem sumir, e é o que o gerador precisa entregar para a
// colagem de peças não ter degrau de volume.
//
// O erro era tocá-lo a ganho 1 por cima do jogo. Um rádio de equipe não chega ao ouvido
// no mesmo nível do motor; ele chega abaixo, e é a compressão — não o volume — que o
// mantém inteligível. Daí o padrão ser atenuação, e daí ele ser ajustável: quanto, exatamente,
// depende do volume do jogo, do fone e do ouvido de quem joga, e nenhuma dessas três coisas
// dá para medir daqui.

const CHAVE = "loop.volumeRadio";

/** −8 dB. Ponto de partida, não verdade: quem decide é o ouvido, no controle das opções. */
export const VOLUME_PADRAO = 0.4;

const ouvintes = new Set();
let atual = null;

function ler() {
  try {
    const bruto = localStorage.getItem(CHAVE);
    if (bruto === null) return VOLUME_PADRAO;
    const v = Number(bruto);
    // Um valor corrompido no storage não pode virar `NaN` num `gain.value` — o nó inteiro
    // emudeceria, e o sintoma seria o rádio parar de funcionar sem nada no console.
    return Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : VOLUME_PADRAO;
  } catch {
    return VOLUME_PADRAO;
  }
}

/** O volume corrente, de 0 a 1. */
export function volumeRadio() {
  if (atual === null) atual = ler();
  return atual;
}

/** Troca o volume e avisa as duas vozes. */
export function definirVolume(v) {
  const limpo = Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : VOLUME_PADRAO;
  atual = limpo;
  try {
    localStorage.setItem(CHAVE, String(limpo));
  } catch {
    /* sem persistência: vale para esta sessão */
  }
  for (const cb of ouvintes) {
    try {
      cb(limpo);
    } catch {
      /* problema de quem assinou */
    }
  }
}

/** Assina as trocas. Devolve a função que desassina. */
export function aoMudarVolume(cb) {
  ouvintes.add(cb);
  return () => ouvintes.delete(cb);
}

/**
 * Aplica o volume num `GainNode` sem estalo.
 *
 * O `gain.value` direto é um degrau na amostra seguinte, e um degrau no meio de uma fala
 * estala. Vinte milissegundos de rampa somem no ouvido e não somem no alto-falante.
 */
export function aplicarEm(no, ctx, v) {
  if (!no) return;
  const alvo = v ?? volumeRadio();
  if (ctx && typeof no.gain?.linearRampToValueAtTime === "function") {
    try {
      no.gain.cancelScheduledValues(ctx.currentTime);
      no.gain.setValueAtTime(no.gain.value, ctx.currentTime);
      no.gain.linearRampToValueAtTime(alvo, ctx.currentTime + 0.02);
      return;
    } catch {
      /* contexto sem agendamento (teste): cai no valor direto */
    }
  }
  no.gain.value = alvo;
}
