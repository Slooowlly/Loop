import { invoke } from "@tauri-apps/api/core";

// Falha que a tela ENGOLE de propósito, com rastro em disco.
//
// O padrão antigo era `.catch(() => {})` cru, e ele misturava duas coisas que só se
// distinguem lendo o comentário ao lado: "isto pode falhar e não custa nada" e "isto
// falhou, a feature não vai acontecer, e ninguém soube". A segunda é o modo de falha
// caro do produto — a bandeira amarela automática que não instalou, o modo janela que
// o overlay precisava — porque chega ao suporte como "não funciona" e sem uma linha
// para conferir.
//
// O canal do rastro é o `loop.log` do backend, e não o console: o app é uma GUI sem
// terminal na máquina do jogador, então `console.warn` ali some. O jogador já sabe
// anexar esse arquivo (a aba de diagnóstico abre o Explorer nele), e o Rust deduplica
// por rótulo — uma falha que se mantém escreve uma linha, e não uma por tick.
//
// O que este helper NÃO faz, de propósito: não mostra toast, não dispara estado e não
// escreve no console. Quem precisa avisar o jogador decide isso no lugar da chamada,
// que é o único ponto que sabe o que a falha custou a ele.

/// O texto que vai para o log. `Error` vira a mensagem; o resto vira `String(...)`,
/// que é como o erro do `invoke` chega (o Rust devolve `Err(String)`).
function mensagemDe(falha) {
  if (falha instanceof Error) return falha.message || String(falha);
  if (typeof falha === "string") return falha;
  try {
    return String(falha);
  } catch {
    return "falha sem mensagem";
  }
}

/// O registro é o último recurso da cadeia: se ELE falhar, não há para onde dizer.
/// Uma função nomeada em vez de `() => {}` para o guard estrutural não a confundir
/// com o padrão que este arquivo existe para substituir.
function semDestino() {}

/// Manda a linha ao `loop.log`. Blindada porque roda dentro de um caminho de falha:
/// fora do Tauri (testes, navegador) `invoke` pode nem existir, e uma exceção aqui
/// transformaria um erro engolido num erro visível — o oposto do combinado.
function registrar(rotulo, falha) {
  try {
    const envio = invoke("diagnostico_registrar", {
      rotulo: String(rotulo),
      mensagem: mensagemDe(falha),
    });
    if (envio && typeof envio.catch === "function") envio.catch(semDestino);
  } catch {
    // idem: sem canal, o silêncio é o comportamento antigo, e ele já era o aceito aqui.
  }
}

/**
 * Engole a falha de `promessa` para a UI e deixa o rastro no log de diagnóstico.
 *
 * Resolve com o valor em caso de sucesso e com `undefined` em caso de falha, então
 * um `.then` encadeado depois dela precisa tolerar `undefined` — que é a mesma
 * disciplina que o `.catch(() => {})` já exigia.
 *
 * @param {Promise<T>|T} promessa  a operação best-effort
 * @param {string} rotulo  identifica a linha no log; use o nome do comando ou da
 *                         operação, estável entre execuções (é a chave da dedup)
 * @returns {Promise<T|undefined>}
 * @template T
 */
export function bestEffort(promessa, rotulo) {
  return Promise.resolve(promessa).then(undefined, (falha) => {
    registrar(rotulo, falha);
    return undefined;
  });
}

/**
 * Igual ao [`bestEffort`], e ainda devolve a falha a quem chamou.
 *
 * Para o caso em que a UI segue viva mas o jogador precisa saber: a exportação da
 * etapa continua, e a linha discreta na tela diz que a macro de bandeira não entrou.
 * O rastro vai para o log do mesmo jeito, sem que o chamador precise repetir isso.
 *
 * @param {Promise<T>|T} promessa
 * @param {string} rotulo
 * @returns {Promise<{ ok: boolean, valor?: T, falha?: unknown }>}
 * @template T
 */
export function bestEffortComRetorno(promessa, rotulo) {
  return Promise.resolve(promessa).then(
    (valor) => ({ ok: true, valor }),
    (falha) => {
      registrar(rotulo, falha);
      return { ok: false, falha };
    },
  );
}
