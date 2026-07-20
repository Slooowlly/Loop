import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./EngineerRadio.css";
import { useBreakdownFeed, usePlayerWarnings } from "./useBreakdownFeed";
import TowerCanvasView from "./TowerCanvasView";
import { OVERLAY_MOCK } from "./overlayMockData";

// Overlay do RÁDIO DA EQUIPE: um card central, texto simples, como se o engenheiro
// avisasse por rádio sobre problemas de peça na grade (quebra ao vivo). Mostra UMA
// mensagem por vez, com fade; a próxima substitui a atual.

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// Deixa o detalhe (causa, que vem em minúsculo) apresentável na MESMA linha: só a 1ª
// maiúscula, sem ponto final — direto. Ex.: "câmbio quebrou" -> "Câmbio quebrou".
function capDetail(s) {
  if (!s) return "";
  const t = s.trim();
  return t ? t.charAt(0).toUpperCase() + t.slice(1) : "";
}

// Card de UMA mensagem. `message = { id, severity, text, detail }` ou null.
export function EngineerRadioCard({ message, holdMs = 6000 }) {
  const [shown, setShown] = useState(null);
  const [visible, setVisible] = useState(false);
  const timer = useRef(null);

  useEffect(() => {
    if (!message) return undefined;
    setShown(message);
    setVisible(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setVisible(false), holdMs);
    return () => clearTimeout(timer.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [message?.id, holdMs]);

  if (!shown) return null;
  const sev = shown.severity === "dnf" ? "dnf" : shown.severity === "heavy" ? "heavy" : "light";
  const tag = sev === "dnf" ? "Abandono" : "Rádio da equipe";

  return (
    <div
      className={`engineer-radio ${visible ? "in" : "out"} sev-${sev}`}
      onTransitionEnd={() => {
        if (!visible) setShown(null); // some do DOM só depois do fade
      }}
    >
      <div className="er-tag">
        <span className="er-dot" />
        {tag}
      </div>
      <div className="er-text" key={shown.id}>
        <RevealWords text={shown.text} delayStep={38} startDelay={0} />
        {capDetail(shown.detail) ? (
          <span className="er-detail">
            {" "}
            <RevealWords
              text={`— ${capDetail(shown.detail)}`}
              delayStep={26}
              startDelay={shown.text.trim().split(/\s+/).length * 38 + 150}
            />
          </span>
        ) : null}
      </div>
    </div>
  );
}

// Revela o texto PALAVRA A PALAVRA (fade+blur escalonado) — mesmo efeito do debrief do
// engenheiro. Cada palavra é um span com `animationDelay` crescente; o CSS `.pw-word` anima.
function RevealWords({ text, delayStep = 42, startDelay = 0 }) {
  let wi = 0;
  return String(text)
    .split(/(\s+)/)
    .map((tok, i) => {
      if (tok === "") return null;
      if (/^\s+$/.test(tok)) return tok;
      const delay = startDelay + wi * delayStep;
      wi += 1;
      return (
        <span key={i} className="radio-word" style={{ animationDelay: `${delay}ms` }}>
          {tok}
        </span>
      );
    });
}

// Card do AVISO PESSOAL (peça do jogador na zona de risco): interface DISTINTA do rádio —
// moldura âmbar pulsante, ícone de atenção, voz em 2ª pessoa, no alto da tela. Mesmo
// mecanismo de fade/hold do card de rádio. Fica mais tempo (é um alerta acionável).
export function PlayerWarningCard({ message, holdMs = 8000 }) {
  const [shown, setShown] = useState(null);
  const [visible, setVisible] = useState(false);
  const timer = useRef(null);

  useEffect(() => {
    if (!message) return undefined;
    setShown(message);
    setVisible(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setVisible(false), holdMs);
    return () => clearTimeout(timer.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [message?.id, holdMs]);

  if (!shown) return null;
  return (
    <div
      className={`player-warning ${visible ? "in" : "out"}`}
      onTransitionEnd={() => {
        if (!visible) setShown(null);
      }}
    >
      <div className="pw-tag">
        <span className="pw-icon">⚠</span>
        Seu carro · Atenção
      </div>
      <div className="pw-text" key={`t${shown.id}`}>
        <RevealWords text={shown.text} delayStep={44} startDelay={140} />
      </div>
    </div>
  );
}

// Container AO VIVO: descobre a carreira ativa (mesma fonte da torre) e puxa o feed.
// Se o MODO DEMO estiver ligado (botão nas Configurações / env), o card cicla exemplos
// LOCALMENTE — sem depender de carreira, sessão ou volta — pra você achar/posicionar o
// overlay na hora. Fora do demo, mostra as quebras reais da corrida.
export function EngineerRadioLive() {
  const [careerId, setCareerId] = useState(null);
  const [demo, setDemo] = useState(false);
  const [demoTick, setDemoTick] = useState(0);

  useEffect(() => {
    if (!IN_TAURI) return undefined;
    let stopped = false;
    const tick = async () => {
      try {
        const [id, on] = await Promise.all([
          invoke("overlay_active_career"),
          invoke("overlay_demo_enabled").catch(() => false),
        ]);
        if (stopped) return;
        setCareerId(id || null);
        setDemo(Boolean(on));
      } catch {
        /* ignore */
      }
    };
    tick();
    const t = setInterval(tick, 1500);
    return () => {
      stopped = true;
      clearInterval(t);
    };
  }, []);

  // Ao ligar o demo, busca a lista de exemplos do backend (FONTE ÚNICA das redações —
  // toda peça × leve/grave × 3 opções, com os PILOTOS REAIS do grid). Fallback: os MOCK.
  const [demoMsgs, setDemoMsgs] = useState([]);
  useEffect(() => {
    if (!demo || !IN_TAURI) {
      setDemoMsgs([]);
      return undefined;
    }
    let stop = false;
    invoke("overlay_demo_messages", { careerId: careerId || "" })
      .then((list) => {
        if (!stop) setDemoMsgs(Array.isArray(list) && list.length ? list : MOCK_MESSAGES);
      })
      .catch(() => {
        if (!stop) setDemoMsgs(MOCK_MESSAGES);
      });
    return () => {
      stop = true;
    };
  }, [demo, careerId]);

  // Demo: avança um exemplo a cada 6 s (id crescente → o card sempre troca).
  useEffect(() => {
    if (!demo) return undefined;
    setDemoTick((n) => n + 1); // mostra o primeiro na hora
    const t = setInterval(() => setDemoTick((n) => n + 1), 6000);
    return () => clearInterval(t);
  }, [demo]);

  // AVISO pessoal (card distinto): no demo busca o tour de avisos e cicla (defasado do rádio).
  const [demoWarns, setDemoWarns] = useState([]);
  const [demoWarnTick, setDemoWarnTick] = useState(0);
  useEffect(() => {
    if (!demo || !IN_TAURI) {
      setDemoWarns([]);
      return undefined;
    }
    let stop = false;
    invoke("overlay_demo_warnings")
      .then((l) => {
        if (!stop && Array.isArray(l)) setDemoWarns(l);
      })
      .catch(() => {});
    return () => {
      stop = true;
    };
  }, [demo]);
  useEffect(() => {
    if (!demo) return undefined;
    setDemoWarnTick((n) => n + 1);
    const t = setInterval(() => setDemoWarnTick((n) => n + 1), 7000);
    return () => clearInterval(t);
  }, [demo]);

  const liveMessage = useBreakdownFeed(demo ? null : careerId); // no demo, pausa o feed real
  const warning = usePlayerWarnings(!demo && Boolean(careerId)); // aviso pessoal real
  const pool = demoMsgs.length ? demoMsgs : MOCK_MESSAGES;
  const demoMessage = demo ? { ...pool[demoTick % pool.length], id: demoTick } : null;
  const demoWarning =
    demo && demoWarns.length ? { ...demoWarns[demoWarnTick % demoWarns.length], id: demoWarnTick } : null;

  // No demo o hold é um pouco maior que o intervalo (6 s) pra não piscar entre trocas.
  return (
    <>
      <EngineerRadioCard message={demo ? demoMessage : liveMessage} holdMs={demo ? 6600 : 6000} />
      <PlayerWarningCard message={demo ? demoWarning : warning} />
    </>
  );
}

// ── Preview no navegador: cicla mensagens fictícias sobre um xadrez. ──
// `driver` casa com o `name` do carro na torre → o flash pisca a linha CERTA
// (mesmo piloto da mensagem). Todos estão na janela visível da torre mock.
const MOCK_MESSAGES = [
  {
    id: 1,
    driver: "Neil Cooper",
    severity: "light",
    text: "Neil Cooper apresenta um problema no motor",
    detail: "superaquecimento",
  },
  {
    id: 2,
    driver: "Alexandr Fescov",
    severity: "heavy",
    text: "Alexandr Fescov apresenta problemas graves nos freios",
    detail: "superaquecimento",
  },
  {
    id: 3,
    driver: "Marius Rieck",
    severity: "dnf",
    text: "Marius Rieck foi retirado da corrida devido a problemas no câmbio",
    detail: "",
  },
  {
    id: 4,
    driver: "Rick Van Zwiet",
    severity: "heavy",
    text: "Rick Van Zwiet apresenta problemas graves na suspensão",
    detail: "braço de suspensão trincado",
  },
];

export function EngineerRadioPreview() {
  const [i, setI] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setI((v) => (v + 1) % MOCK_MESSAGES.length), 4500);
    return () => clearInterval(t);
  }, []);

  // Marca `flash` na linha do piloto da mensagem ATUAL (o resto apaga) — demonstra a
  // sincronia: o rádio anuncia e a mesma pessoa pisca na torre. A severidade define a cor
  // do flash (âmbar/vermelho/preto), então casa o alert/flag do carro com a mensagem.
  const current = MOCK_MESSAGES[i];
  const towerData = {
    ...OVERLAY_MOCK,
    classes: OVERLAY_MOCK.classes.map((c) => ({
      ...c,
      cars: c.cars.map((car) => {
        if (car.name !== current.driver) return { ...car, flash: false };
        const s = current.severity; // "light" | "heavy" | "dnf"
        return {
          ...car,
          flash: true,
          alert: s === "dnf" ? null : s, // leve/grave → triângulo; DNF não tem
          flag: s === "dnf" ? "black" : car.flag,
        };
      }),
    })),
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "#11151a",
        backgroundImage:
          "linear-gradient(45deg, #1b2027 25%, transparent 25%), linear-gradient(-45deg, #1b2027 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #1b2027 75%), linear-gradient(-45deg, transparent 75%, #1b2027 75%)",
        backgroundSize: "24px 24px",
        backgroundPosition: "0 0, 0 12px, 12px -12px, -12px 0px",
      }}
    >
      {/* Os DOIS overlays juntos, como aparecem na tela: a torre no canto + o rádio no meio. */}
      <div style={{ position: "absolute", top: 12, left: 12 }}>
        <TowerCanvasView
          data={towerData}
          animate
          style={{ borderRadius: 10, boxShadow: "0 10px 40px rgba(0,0,0,0.55)" }}
        />
      </div>
      <EngineerRadioCard message={current} holdMs={3800} />
    </div>
  );
}
