// O CUTUCÃO dos canais do engenheiro, lado do front.
//
// O Rust avisa, no instante em que o canal ganha novidade, que há o que buscar — ver
// `race_monitor::cutucao`. O payload é só o NOME do canal: quem monta a mensagem continua sendo
// o mesmo `invoke` que o poll já fazia, com o mesmo cursor, a mesma ordem e os mesmos ids.
//
// Existe porque o poll é o elo frágil: em corrida a janela do webview fica coberta pelo
// simulador ou minimizada, e o navegador estrangula o `setInterval`. O número está medido e
// escrito em `EngenheiroVozAuto.jsx` — 800 ms viraram 20 segundos. Evento do Tauri não passa
// pelo estrangulador de timers, que é o que faz esta porta valer a pena.
//
// O poll NÃO sai de cena e a frequência dele NÃO muda: os dois caminhos batem no mesmo cursor,
// e o que chegar em dobro é descartado uma vez só. É uma segunda porta, não a troca de uma
// frágil por outra — a mesma decisão que o spotter já tinha tomado em `useSpotter.js`.

import { estaNoTauri } from "./tauri";

export const EVENTO_CUTUCAO = "radio-cutucao";

/** Os nomes, iguaizinhos aos do Rust (`race_monitor::cutucao`). Divergir aqui emudece o canal. */
export const CANAL_QUEBRA = "quebra";
export const CANAL_RITMO = "ritmo";
export const CANAL_AVISO = "aviso";
export const CANAL_OCASIAO = "ocasiao";
export const CANAL_CLASSIFICACAO = "classificacao";

/**
 * Ouve o cutucão de UM canal. Devolve a função que desassina.
 *
 * O `listen` é assíncrono e a limpeza de um `useEffect` é síncrona: sem a bandeira `morto`, uma
 * desmontagem que acontece antes de o ouvinte existir o deixaria órfão — vivo e sem ninguém para
 * removê-lo. Em dev o StrictMode monta duas vezes de propósito, e o resultado seria cada cutucão
 * virar duas leituras.
 */
export function ouvirCutucao(canal, aoCutucar) {
  if (!estaNoTauri()) return () => {};
  let parar = null;
  let morto = false;
  import("@tauri-apps/api/event")
    .then(({ listen }) =>
      listen(EVENTO_CUTUCAO, ({ payload }) => {
        if (payload === canal) aoCutucar();
      }),
    )
    .then((un) => {
      if (morto) un();
      else parar = un;
    })
    .catch(() => {
      /* sem ponte — o poll continua respondendo */
    });
  return () => {
    morto = true;
    if (parar) parar();
  };
}
