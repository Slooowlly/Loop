import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { estaNoTauri } from "../lib/tauri";
import {
  CANAL_AVISO,
  CANAL_CLASSIFICACAO,
  CANAL_QUEBRA,
  CANAL_RITMO,
  ouvirCutucao,
} from "../lib/cutucaoDoRadio";
import { passoDoCursor } from "./cursorDoFeed";

// Os canais do RÁDIO DA EQUIPE. Cada um lê uma lista que só cresce e precisa mostrar cada
// mensagem uma vez; quem decide o que já foi visto é o cursor (`cursorDoFeed`), e quem decide o
// que soa é a camada de voz.
//
// Na 1ª leitura o cursor só ancora (não reexibe o que aconteceu antes de a tela abrir) — a
// partir daí, cada mensagem nova aparece uma vez.

/**
 * O laço comum aos três canais: poll + cutucão, sobre o mesmo cursor.
 *
 * Os três eram três cópias do mesmo `useEffect`, e a cópia é o que faz um canal ganhar correção
 * que o irmão não ganha. O que difere entre eles é dado, e está nos parâmetros.
 *
 * `comando` é o `invoke`; `canal` é o nome que o Rust cutuca; `careerId`, quando existe, vira o
 * argumento do comando E a chave que reinicia o cursor (cada carreira recomeça do zero).
 */
function usarFeed({
  comando,
  canal,
  careerId = null,
  ativo,
  intervalMs,
  umaPorVez = false,
  limparAoParar = false,
}) {
  const [message, setMessage] = useState(null);

  useEffect(() => {
    if (!estaNoTauri() || !ativo) {
      if (limparAoParar) setMessage(null);
      return undefined;
    }
    // Estado do cursor DENTRO do efeito: ele nasce e morre com a assinatura do canal, e um
    // `useRef` sobrevivendo a uma troca de carreira levaria o cursor velho para o save novo.
    let seen = -1;
    let primed = false;
    let stopped = false;
    // O cursor é uma leitura seguida de escrita, com um `await` no meio. Duas leituras em voo
    // ao mesmo tempo leriam o mesmo `seen` e entregariam a mesma mensagem duas vezes — e agora
    // há duas portas batendo no mesmo tique. Quem chega no meio de uma leitura marca a vez e é
    // atendido no fim dela; o log só cresce, então nada se perde na espera.
    let emVoo = false;
    let pendente = false;

    const tick = async () => {
      if (emVoo) {
        pendente = true;
        return;
      }
      emVoo = true;
      try {
        const feed = await invoke(comando, careerId ? { careerId } : undefined);
        if (stopped || !Array.isArray(feed)) return;
        const passo = passoDoCursor({ feed, seen, primed, umaPorVez });
        primed = passo.primed;
        seen = passo.seen;
        if (passo.mostrar) setMessage(passo.mostrar);
      } catch {
        /* sem sessão / sem save — silencioso */
      } finally {
        emVoo = false;
        if (pendente && !stopped) {
          pendente = false;
          tick();
        }
      }
    };

    tick();
    const timer = setInterval(tick, intervalMs);
    // O EMPURRÃO. Não substitui o poll nem mexe no ritmo dele: o poll é a rede, e os dois
    // passam pelo mesmo cursor. Ver `lib/cutucaoDoRadio.js`.
    const pararCutucao = ouvirCutucao(canal, tick);
    return () => {
      stopped = true;
      clearInterval(timer);
      pararCutucao();
    };
  }, [comando, canal, careerId, ativo, intervalMs, umaPorVez, limparAoParar]);

  return message;
}

// QUEBRAS da grade (`get_breakdown_feed`).
//
// `umaPorVez`: drena a MAIS ANTIGA ainda não vista, não a mais nova. Pular direto para o fim
// descartava quebras — duas caindo entre dois polls e a primeira sumia, sem card e sem áudio,
// contra a regra de que toda quebra fala. O Rust já FUNDE as simultâneas da mesma volta, então
// o que sobra para drenar aqui é raro, e drenar de uma em uma mantém card e fala na mesma ordem.
export function useBreakdownFeed(careerId, { intervalMs = 700 } = {}) {
  return usarFeed({
    comando: "get_breakdown_feed",
    canal: CANAL_QUEBRA,
    careerId: careerId || null,
    ativo: Boolean(careerId),
    intervalMs,
    umaPorVez: true,
  });
}

// RÁDIO DE RITMO (`get_pace_feed`) — a volta mais rápida da corrida e a nossa aproximação dela.
// Mesmo mecanismo, canal SEPARADO: os dois crescem em ritmos próprios e um id só embaralharia
// os cursores. Quem junta os dois é quem desenha o card.
//
// Aqui vale a mais NOVA, e é o único canal em que vale: o ritmo é um estado, não uma fila.
// "Estamos a dois décimos" já não interessa quando a leitura seguinte diz um décimo — anunciar
// a intermediária seria informar um gap que não existe mais.
export function usePaceFeed(careerId, { intervalMs = 900 } = {}) {
  return usarFeed({
    comando: "get_pace_feed",
    canal: CANAL_RITMO,
    careerId: careerId || null,
    ativo: Boolean(careerId),
    intervalMs,
  });
}

// AVISO pessoal do jogador (peça DELE na zona de risco, quebra, castigo da classificação) — do
// comando `get_player_warnings`, sem carreira, é o próprio piloto. Card DISTINTO no overlay.
//
// `umaPorVez` como as quebras, e pelo mesmo motivo: são eventos DISCRETOS, cada um com um
// desfecho próprio, e não um estado que a leitura seguinte substitui. O Rust não os funde como
// funde as quebras da mesma volta, então duas caindo entre duas leituras faziam a primeira
// desaparecer — e este é o canal do carro do jogador, onde cada aviso é sobre ele mesmo.
export function usePlayerWarnings(active, { intervalMs = 800 } = {}) {
  return usarFeed({
    comando: "get_player_warnings",
    canal: CANAL_AVISO,
    ativo: Boolean(active),
    intervalMs,
    umaPorVez: true,
    limparAoParar: true,
  });
}

// ENGENHEIRO NA CLASSIFICAÇÃO (`get_classificacao_feed`) — a despedida antes da volta lançada e
// o comentário da volta que morreu. Sem carreira, como os avisos: nenhuma fala desta família
// nomeia piloto ou equipe, então ela não depende do save.
//
// `umaPorVez` porque são eventos DISCRETOS, e o par "perdeu essa" + "ainda dá pra mais uma" sai
// como uma fala só do Rust.
//
// O intervalo é o mesmo dos irmãos, e não mais curto: esta é a fala com o prazo mais apertado do
// rádio (a despedida termina pouco antes da linha), mas quem cumpre esse prazo é o CUTUCÃO, que
// chega no instante em que o log cresce. Apertar o poll para compensar seria pagar leitura por
// leitura numa sessão em que o log passa a maior parte do tempo parado.
export function useClassificacaoFeed(active, { intervalMs = 800 } = {}) {
  return usarFeed({
    comando: "get_classificacao_feed",
    canal: CANAL_CLASSIFICACAO,
    ativo: Boolean(active),
    intervalMs,
    umaPorVez: true,
    limparAoParar: true,
  });
}

// Estado LATCH (não stream): algum comando de quebra (`!black`/`!dq`) não chegou ao iRacing
// nesta corrida — fullscreen exclusivo ou trava de foco bloquearam o envio. Canal SEPARADO
// dos avisos de peça (que são stream por id) justamente pra não mascará-los. Devolve um
// booleano persistente enquanto durar a corrida; o overlay mostra um banner acionável.
//
// SEM cutucão: um latch não tem novidade a perder. Ele é verdadeiro enquanto durar a corrida, e
// uma leitura atrasada mostra o mesmo banner um pouco depois.
export function useChatSendBlocked(active, { intervalMs = 1500 } = {}) {
  const [blocked, setBlocked] = useState(false);

  useEffect(() => {
    if (!estaNoTauri() || !active) {
      setBlocked(false);
      return undefined;
    }
    let stopped = false;
    const tick = async () => {
      try {
        const on = await invoke("iracing_chat_blocked");
        if (!stopped) setBlocked(Boolean(on));
      } catch {
        /* sem sessão — silencioso */
      }
    };
    tick();
    const timer = setInterval(tick, intervalMs);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [active, intervalMs]);

  return blocked;
}
