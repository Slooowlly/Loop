import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { estaNoTauri } from "../lib/tauri";
import { passoDoCursor } from "./cursorDoFeed";

// Puxa o feed de quebras ao vivo (`get_breakdown_feed`) e devolve a mensagem mais NOVA
// que ainda não foi mostrada (por id crescente). null quando não há novidade.
//
// Na 1ª leitura só "prima" o cursor (não reexibe quebras que já aconteceram antes do
// overlay abrir) — a partir daí, cada quebra nova aparece uma vez.

export function useBreakdownFeed(careerId, { intervalMs = 700 } = {}) {
  const [message, setMessage] = useState(null);
  const seenRef = useRef(-1); // maior id já exibido
  const primedRef = useRef(false); // já ancorou o cursor na 1ª leitura?

  useEffect(() => {
    if (!estaNoTauri() || !careerId) return undefined;
    // Cada carreira recomeça o cursor.
    seenRef.current = -1;
    primedRef.current = false;

    let stopped = false;
    let timer = null;

    const tick = async () => {
      try {
        const feed = await invoke("get_breakdown_feed", { careerId });
        if (stopped || !Array.isArray(feed)) return;
        // `umaPorVez`: drena a MAIS ANTIGA ainda não vista, não a mais nova. Pular direto
        // para o fim descartava quebras — duas caindo entre dois polls e a primeira sumia,
        // sem card e sem áudio, contra a regra de que toda quebra fala. O Rust já FUNDE as
        // simultâneas da mesma volta, então o que sobra para drenar aqui é raro, e drenar de
        // uma em uma mantém card e fala na mesma ordem.
        const passo = passoDoCursor({
          feed,
          seen: seenRef.current,
          primed: primedRef.current,
          umaPorVez: true,
        });
        primedRef.current = passo.primed;
        seenRef.current = passo.seen;
        if (passo.mostrar) setMessage(passo.mostrar);
      } catch {
        /* sem sessão / sem save — silencioso */
      }
    };

    tick();
    timer = setInterval(tick, intervalMs);
    return () => {
      stopped = true;
      if (timer) clearInterval(timer);
    };
  }, [careerId, intervalMs]);

  return message;
}

// RÁDIO DE RITMO (`get_pace_feed`) — a volta mais rápida da corrida e a nossa aproximação
// dela. Mesmo mecanismo do feed de quebras, canal SEPARADO: os dois crescem em ritmos próprios
// e um id só embaralharia os cursores. Quem junta os dois é quem desenha o card.
export function usePaceFeed(careerId, { intervalMs = 900 } = {}) {
  const [message, setMessage] = useState(null);
  const seenRef = useRef(-1);
  const primedRef = useRef(false);

  useEffect(() => {
    if (!estaNoTauri() || !careerId) return undefined;
    seenRef.current = -1;
    primedRef.current = false;

    let stopped = false;
    const tick = async () => {
      try {
        const feed = await invoke("get_pace_feed", { careerId });
        if (stopped || !Array.isArray(feed)) return;
        // Vale a mais nova: o ritmo é um estado, não uma fila. "Estamos a dois décimos" já
        // não interessa quando a leitura seguinte diz um décimo.
        const passo = passoDoCursor({
          feed,
          seen: seenRef.current,
          primed: primedRef.current,
        });
        primedRef.current = passo.primed;
        seenRef.current = passo.seen;
        if (passo.mostrar) setMessage(passo.mostrar);
      } catch {
        /* sem sessão / sem save — silencioso */
      }
    };
    tick();
    const timer = setInterval(tick, intervalMs);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [careerId, intervalMs]);

  return message;
}

// AVISO pessoal do jogador (peça DELE na zona de risco) — mesmo mecanismo (mais nova por id,
// prime na 1ª leitura), mas do comando `get_player_warnings` (sem carreira, é o próprio piloto).
// `active` liga/desliga o poll. Card DISTINTO no overlay.
export function usePlayerWarnings(active, { intervalMs = 800 } = {}) {
  const [message, setMessage] = useState(null);
  const seenRef = useRef(-1);
  const primedRef = useRef(false);

  useEffect(() => {
    if (!estaNoTauri() || !active) {
      setMessage(null);
      return undefined;
    }
    seenRef.current = -1;
    primedRef.current = false;

    let stopped = false;
    const tick = async () => {
      try {
        const feed = await invoke("get_player_warnings");
        if (stopped || !Array.isArray(feed)) return;
        // Este é o canal que mais sofria com a âncora adiada: os avisos do nosso carro são
        // poucos e espaçados, então "o primeiro de cada tentativa" é, muitas vezes, o único.
        const passo = passoDoCursor({
          feed,
          seen: seenRef.current,
          primed: primedRef.current,
        });
        primedRef.current = passo.primed;
        seenRef.current = passo.seen;
        if (passo.mostrar) setMessage(passo.mostrar);
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

  return message;
}

// Estado LATCH (não stream): algum comando de quebra (`!black`/`!dq`) não chegou ao iRacing
// nesta corrida — fullscreen exclusivo ou trava de foco bloquearam o envio. Canal SEPARADO
// dos avisos de peça (que são stream por id) justamente pra não mascará-los. Devolve um
// booleano persistente enquanto durar a corrida; o overlay mostra um banner acionável.
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
