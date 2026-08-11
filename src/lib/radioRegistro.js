// REGISTRO DO RÁDIO, lado do front: a fase `tocada` da linha do tempo.
//
// O Rust registra a DECISÃO, no tique de 60 Hz, com o tempo de sessão exato. Aqui registra-se o
// que aconteceu com aquela decisão depois: o áudio começou, foi cortado no meio, cedeu ao
// spotter, ficou esperando a vez, venceu por validade, morreu no portão do momento. Só esta
// ponta sabe disso, e é ela que responde a pergunta que a decisão não responde — o jogador
// ouviu?
//
// Três regras, todas pela mesma razão de fundo (a medição não pode custar nada ao áudio):
//
//   1. **Nunca espera.** Nenhum `await` no caminho de tocar. O `invoke` sai e é esquecido.
//   2. **Nunca quebra.** Todo erro é engolido. Um registro que estoura no meio de uma disputa
//      derrubaria a fala que ele existe para medir.
//   3. **Nunca importa o Tauri fora dele.** O `import` do `invoke` é dinâmico e só acontece
//      dentro do shell: assim `spotterVoice.test.js` e os outros seguem rodando em jsdom puro.
//
// O TEXTO das falas do spotter não passa por aqui. Ele vive no gerador do pacote
// (`scripts/spotter-pack.mjs`), que é a fonte única das redações, e quem resolve chave em texto
// é `scripts/radio-timeline.mjs` na hora de imprimir. O que sobe daqui é a chave da variação que
// de fato tocou, que é a informação exata do que soou.

import { estaNoTauri } from "./tauri";

// O `invoke`, resolvido uma vez e guardado. `null` enquanto não foi buscado, `false` quando o
// módulo não veio (fora do shell, ou import falhou) — e aí nunca se tenta de novo.
let enviar = null;

/** Ligado por padrão. Desligável por localStorage para uma sessão em que o disco importe. */
const CHAVE_LIGADO = "loop.radioRegistro";

export function estaLigado() {
  try {
    return localStorage.getItem(CHAVE_LIGADO) !== "0";
  } catch {
    return true;
  }
}

export function ligar(on) {
  try {
    localStorage.setItem(CHAVE_LIGADO, on ? "1" : "0");
  } catch {
    /* sem persistência */
  }
}

/**
 * **Registra uma fala.** Devolve nada, de propósito: nenhum chamador deve poder esperar por isto.
 *
 * `canal` é a boca que falou (`spotter`, `quebra`, `ritmo`, `aviso`, `resposta`, `ocasiao`),
 * `fase` é sempre `tocada` daqui, e `desfecho` é o que aconteceu de fato.
 */
export function registrar(canal, { fase = "tocada", chaves = [], texto = "", desfecho = "ok", detalhe = null } = {}) {
  if (!estaLigado() || !estaNoTauri()) return;
  const registro = {
    canal,
    fase,
    chaves: Array.isArray(chaves) ? chaves : [chaves].filter(Boolean),
    texto: texto ?? "",
    desfecho,
    detalhe,
  };
  if (enviar === false) return;
  if (enviar) {
    despachar(registro);
    return;
  }
  // Primeira fala da sessão: busca o `invoke` e guarda o que estava pendente para não perder
  // justamente a primeira, que é a que diz quando o rádio abriu.
  pendentes.push(registro);
  if (buscando) return;
  buscando = true;
  import("@tauri-apps/api/core")
    .then((mod) => {
      enviar = typeof mod?.invoke === "function" ? mod.invoke : false;
      if (enviar) for (const r of pendentes) despachar(r);
      pendentes = [];
    })
    .catch(() => {
      enviar = false;
      pendentes = [];
    });
}

let pendentes = [];
let buscando = false;

function despachar(registro) {
  try {
    const p = enviar("radio_registrar", { registro });
    if (p && typeof p.catch === "function") p.catch(() => {});
  } catch {
    /* a medição nunca atrapalha a fala */
  }
}
