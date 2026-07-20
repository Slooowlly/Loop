import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Puxa o feed de quebras ao vivo (`get_breakdown_feed`) e devolve a mensagem mais NOVA
// que ainda não foi mostrada (por id crescente). null quando não há novidade.
//
// Na 1ª leitura só "prima" o cursor (não reexibe quebras que já aconteceram antes do
// overlay abrir) — a partir daí, cada quebra nova aparece uma vez.

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function useBreakdownFeed(careerId, { intervalMs = 700 } = {}) {
  const [message, setMessage] = useState(null);
  const seenRef = useRef(-1); // maior id já exibido
  const primedRef = useRef(false); // já ancorou o cursor na 1ª leitura?

  useEffect(() => {
    if (!IN_TAURI || !careerId) return undefined;
    // Cada carreira recomeça o cursor.
    seenRef.current = -1;
    primedRef.current = false;

    let stopped = false;
    let timer = null;

    const tick = async () => {
      try {
        const feed = await invoke("get_breakdown_feed", { careerId });
        if (stopped || !Array.isArray(feed) || feed.length === 0) return;
        const newest = feed[feed.length - 1];
        if (!primedRef.current) {
          // 1ª leitura: ancora sem exibir (não repete o que já rolou).
          primedRef.current = true;
          seenRef.current = newest.id;
          return;
        }
        if (newest.id > seenRef.current) {
          seenRef.current = newest.id;
          setMessage(newest);
        }
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

// AVISO pessoal do jogador (peça DELE na zona de risco) — mesmo mecanismo (mais nova por id,
// prime na 1ª leitura), mas do comando `get_player_warnings` (sem carreira, é o próprio piloto).
// `active` liga/desliga o poll. Card DISTINTO no overlay.
export function usePlayerWarnings(active, { intervalMs = 800 } = {}) {
  const [message, setMessage] = useState(null);
  const seenRef = useRef(-1);
  const primedRef = useRef(false);

  useEffect(() => {
    if (!IN_TAURI || !active) {
      setMessage(null);
      return undefined;
    }
    seenRef.current = -1;
    primedRef.current = false;

    let stopped = false;
    const tick = async () => {
      try {
        const feed = await invoke("get_player_warnings");
        if (stopped || !Array.isArray(feed) || feed.length === 0) return;
        const newest = feed[feed.length - 1];
        if (!primedRef.current) {
          primedRef.current = true;
          seenRef.current = newest.id;
          return;
        }
        if (newest.id > seenRef.current) {
          seenRef.current = newest.id;
          setMessage(newest);
        }
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
