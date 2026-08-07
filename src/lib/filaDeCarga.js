// Teto de carregamentos de áudio simultâneos.
//
// Existe por um motivo medido: o acervo do engenheiro tem 3.943 peças (324 MB), e um
// `Promise.all` sobre ele dispara milhares de `fetch` no mesmo tique. O navegador não
// enfileira isso — ele RECUSA, e o console enche de `ERR_INSUFFICIENT_RESOURCES`.
//
// A recusa não distingue quem pediu, e é isso que torna o teto necessário em vez de
// apenas desejável: as 51 falas do spotter são poucas, pequenas e urgentes, e morriam
// junto com a avalanche do engenheiro — o sintoma aparecia na pasta que não tinha culpa.
//
// O teto é COMPARTILHADO pelos dois módulos de voz de propósito. Um teto por módulo
// deixaria cada um dentro do seu limite e a soma fora dele, que é exatamente o caso que
// derrubou o spotter.
const TETO = 6;

let ativos = 0;
const espera = [];

function executar(fn) {
  return Promise.resolve()
    .then(fn)
    .finally(() => {
      ativos -= 1;
      const proximo = espera.shift();
      if (proximo) {
        ativos += 1;
        proximo();
      }
    });
}

/**
 * Roda `fn` assim que houver vaga. Devolve a promessa dela — quem chama não percebe a
 * espera, só que ela demora mais quando há fila.
 */
export function comLimite(fn) {
  if (ativos < TETO) {
    ativos += 1;
    return executar(fn);
  }
  return new Promise((resolve, reject) => {
    espera.push(() => executar(fn).then(resolve, reject));
  });
}
