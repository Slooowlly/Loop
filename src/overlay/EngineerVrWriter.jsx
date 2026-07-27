import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import useCareerStore from "../stores/useCareerStore";
import { RADIO_VR_W, RADIO_VR_H, drawRadioCard } from "./radioCanvas";
import { useBreakdownFeed, usePlayerWarnings } from "./useBreakdownFeed";
import { estaNoTauri } from "../lib/tauri";

// Escritor do RÁDIO DA EQUIPE no VR: quando uma quebra é anunciada, desenha o card
// num canvas e manda os pixels (~10 Hz) pro backend, que os coloca no 2º mapeamento
// (`Local\iRacerEngineerFrame`) lido pela OpenXR API layer — um quad independente da
// torre. Fora da janela da mensagem, manda um frame TRANSPARENTE (o quad some).
//
// MODO DEMO: honra o MESMO flag das Configurações (`overlay_demo_enabled`, botão "demo
// do rádio") que o rádio de MONITOR já usa. Ligou o demo → cicla os exemplos do backend
// (`overlay_demo_messages`, com os pilotos reais do grid) direto no quad do VR, pra você
// achar/posicionar o overlay sem esperar uma quebra real. Um toggle só nas Configurações
// aparece no monitor E no VR. Fora do demo, mostra as quebras reais da corrida.
//
// Componente invisível: só alimenta o overlay do VR. Roda só dentro do Tauri.

const HOLD_MS = 6000; // quanto a mensagem fica no ar
const FADE_MS = 450; // fade-out no fim
const DEMO_STEP_MS = 6000; // troca de exemplo no demo

// Fallback caso o backend não tenha lista (ex.: sem carreira carregada) — garante que
// algo aparece no VR pra posicionar mesmo assim.
const DEMO_FALLBACK = [
  { id: 1, severity: "light", text: "Teste do rádio no VR — problema no motor", detail: "posicione o quad onde ficar bom" },
  { id: 2, severity: "heavy", text: "Problema grave nos freios", detail: "Ctrl-dir + T mira no rádio · setas movem" },
  { id: 3, severity: "dnf", text: "Carro retirado da corrida — abandono", detail: "" },
];
const WARN_FALLBACK = [
  { id: 1, severity: "warn", text: "Atenção no seu motor — algo estranho no rádio", detail: "pode falhar a qualquer momento" },
];

// Mistura os avisos pessoais (severity "warn") no rodízio das quebras pra o card de
// "Seu carro · Atenção" também aparecer no demo (1 aviso a cada 2 quebras).
function mixPools(msgs, warns) {
  if (!warns.length) return msgs;
  if (!msgs.length) return warns;
  const out = [];
  let wi = 0;
  for (let i = 0; i < msgs.length; i += 1) {
    out.push(msgs[i]);
    if ((i + 1) % 2 === 0 && wi < warns.length) out.push(warns[wi++]);
  }
  while (wi < warns.length) out.push(warns[wi++]);
  return out;
}

export default function EngineerVrWriter() {
  const careerId = useCareerStore((s) => s.careerId);
  const [demo, setDemo] = useState(false);

  // Poll do flag de demo (fonte no backend, botão das Configurações).
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    let stopped = false;
    const tick = () =>
      invoke("overlay_demo_enabled")
        .then((v) => {
          if (!stopped) setDemo(Boolean(v));
        })
        .catch(() => {});
    tick();
    const t = setInterval(tick, 1500);
    return () => {
      stopped = true;
      clearInterval(t);
    };
  }, []);

  // Lista de exemplos do backend (mesma fonte do rádio de monitor). Fallback local.
  const [demoMsgs, setDemoMsgs] = useState([]);
  useEffect(() => {
    if (!demo || !estaNoTauri()) {
      setDemoMsgs([]);
      return undefined;
    }
    let stop = false;
    invoke("overlay_demo_messages", { careerId: careerId || "" })
      .then((list) => {
        if (!stop) setDemoMsgs(Array.isArray(list) && list.length ? list : DEMO_FALLBACK);
      })
      .catch(() => {
        if (!stop) setDemoMsgs(DEMO_FALLBACK);
      });
    return () => {
      stop = true;
    };
  }, [demo, careerId]);

  // Avisos pessoais de exemplo ("Seu carro · Atenção", severity "warn") — mesma fonte
  // do rádio de monitor. Entram misturados no rodízio pra também aparecerem no demo.
  const [demoWarns, setDemoWarns] = useState([]);
  useEffect(() => {
    if (!demo || !estaNoTauri()) {
      setDemoWarns([]);
      return undefined;
    }
    let stop = false;
    invoke("overlay_demo_warnings")
      .then((list) => {
        if (!stop) setDemoWarns(Array.isArray(list) && list.length ? list : WARN_FALLBACK);
      })
      .catch(() => {
        if (!stop) setDemoWarns(WARN_FALLBACK);
      });
    return () => {
      stop = true;
    };
  }, [demo]);

  // Avança um exemplo a cada 6 s (id crescente → o quad sempre troca).
  const [demoTick, setDemoTick] = useState(0);
  useEffect(() => {
    if (!demo) return undefined;
    setDemoTick((n) => n + 1); // mostra o primeiro na hora
    const t = setInterval(() => setDemoTick((n) => n + 1), DEMO_STEP_MS);
    return () => clearInterval(t);
  }, [demo]);

  // Quebra real E aviso pessoal (ambos pausados no demo) → armam a janela de exibição.
  const message = useBreakdownFeed(demo ? null : careerId);
  const warning = usePlayerWarnings(!demo && Boolean(careerId));
  const activeRef = useRef({ msg: null, until: 0 });
  useEffect(() => {
    if (!message) return;
    activeRef.current = { msg: message, until: Date.now() + HOLD_MS };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [message?.id]);
  useEffect(() => {
    if (!warning) return;
    activeRef.current = { msg: warning, until: Date.now() + HOLD_MS };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [warning?.id]);

  // Card do demo corrente, num ref pro loop de escrita ler sem re-montar o timer.
  const demoRef = useRef(null);
  useEffect(() => {
    const base = demoMsgs.length ? demoMsgs : demo ? DEMO_FALLBACK : [];
    const pool = demo ? mixPools(base, demoWarns) : [];
    demoRef.current =
      demo && pool.length ? { ...pool[demoTick % pool.length], id: demoTick } : null;
  }, [demo, demoMsgs, demoWarns, demoTick]);

  useEffect(() => {
    if (!estaNoTauri()) return undefined;

    const canvas = document.createElement("canvas");
    canvas.width = RADIO_VR_W;
    canvas.height = RADIO_VR_H;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    let lastKey = null; // evita reenviar o mesmo frame ocioso à toa

    const timer = setInterval(async () => {
      let toDraw = null;
      if (demoRef.current) {
        // Demo: card fixo (sem fade) pra posicionar; troca a cada 6 s.
        toDraw = { ...demoRef.current, alpha: 1 };
      } else {
        const { msg, until } = activeRef.current;
        const now = Date.now();
        if (msg && now < until) {
          const remain = until - now;
          const alpha = remain < FADE_MS ? remain / FADE_MS : 1;
          toDraw = { ...msg, alpha };
        }
      }

      // Quando ocioso, só manda a limpeza UMA vez (não fica cuspindo transparente).
      const key = toDraw ? `${toDraw.id}:${Math.round(toDraw.alpha * 12)}` : "idle";
      if (key === "idle" && lastKey === "idle") return;
      lastKey = key;

      try {
        drawRadioCard(ctx, toDraw);
        const img = ctx.getImageData(0, 0, RADIO_VR_W, RADIO_VR_H);
        await invoke("vr_engineer_write_frame", new Uint8Array(img.data.buffer));
      } catch {
        // Falha ao escrever um frame não é fatal: tenta de novo no próximo tick.
      }
    }, 100);

    return () => clearInterval(timer);
  }, []);

  return null;
}
