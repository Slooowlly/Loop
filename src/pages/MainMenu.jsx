import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";

import useCareerStore from "../stores/useCareerStore";
import { formatDateTime } from "../utils/formatters";

// Parametros visuais da tela inicial (ajustados em debug e fixados).
const CFG = {
  speedMax: 47, // topo da velocidade das particulas
  speedBias: 8, // expoente: maior = rapidas mais raras
  density: 30000, // divisor de area: menor = mais particulas
  light: 0.55, // intensidade base da luz
  particleAlpha: 2.45, // brilho das particulas
  parallax: 0.4, // intensidade do parallax
  barH: 1.5, // altura das barras do letterbox (vh)
  anim: 0.5, // duracao da animacao (s); corte no meio
  zoom: 1.35, // zoom de saida
  zoomTarget: "bg", // alvo do zoom: bg | text | both
  color: "92,228,255", // #5ce4ff (rgb)
  flicker: 0.8, // oscilacao tipo tocha
  fx: 2.6, // intensidade geral (multiplica luz + particulas)
  dark: 1, // opacidade do escurecimento
  darkAngle: 40, // direcao do escurecimento (graus)
  darkExtent: 118, // alcance do escurecimento (% da tela)
};

// Oscilacao de intensidade tipo tocha: flicker lento + labareda a cada 8s.
function torch(t, amt) {
  const flicker =
    0.08 * Math.sin(t * 0.9) +
    0.05 * Math.sin(t * 2.1 + 1.7) +
    0.04 * Math.sin(t * 3.7 + 0.5);
  const phase = t % 8;
  const surge = Math.exp(-((phase - 1) ** 2) / (2 * 0.6 * 0.6));
  return 1 + amt * (flicker + 0.9 * surge);
}

function PlayIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M8 5v14l11-7z" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

function FolderIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </svg>
  );
}

function GearIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

function MainMenu({ intro = false }) {
  const navigate = useNavigate();
  const loadCareer = useCareerStore((state) => state.loadCareer);

  const stageRef = useRef(null);
  const canvasRef = useRef(null);
  const glowRef = useRef(null);

  const [recentSave, setRecentSave] = useState(null);
  const [entered, setEntered] = useState(false);
  const [exiting, setExiting] = useState(false);
  const [logoStep, setLogoStep] = useState(0); // intro: 0 inicial, 1 zoom-in, 2 zoom-out
  const [introDone, setIntroDone] = useState(!intro);

  const prefersReduced =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  // Navega no meio da animacao (zoom ainda em movimento), nao no fim.
  const CUT_MS = prefersReduced ? 0 : Math.round(CFG.anim * 1000 * 0.5);

  // Save mais recente para o cartao "Continuar".
  useEffect(() => {
    let alive = true;
    invoke("list_saves")
      .then((saves) => {
        if (!alive || !Array.isArray(saves) || saves.length === 0) return;
        const sorted = [...saves].sort(
          (a, b) => new Date(b.last_played || 0) - new Date(a.last_played || 0),
        );
        setRecentSave(sorted[0]);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  // Entrada: com intro (icone com zoom sobre o fundo borrado) ou direta.
  useEffect(() => {
    if (!intro) {
      const id = window.setTimeout(() => setEntered(true), 30);
      return () => window.clearTimeout(id);
    }
    if (prefersReduced) {
      setEntered(true);
      setIntroDone(true);
      return undefined;
    }
    const t1 = window.setTimeout(() => setLogoStep(1), 30); // zoom-in
    const t2 = window.setTimeout(() => {
      setLogoStep(2); // zoom-out do icone
      setEntered(true); // tira o blur e revela o menu
    }, 1200);
    const t3 = window.setTimeout(() => setIntroDone(true), 1900); // remove o overlay
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
      window.clearTimeout(t3);
    };
  }, [intro, prefersReduced]);

  // Fundo animado: particulas (direita -> esquerda, velocidades mistas),
  // parallax por profundidade e luz tipo tocha.
  useEffect(() => {
    const canvas = canvasRef.current;
    const stage = stageRef.current;
    const glow = glowRef.current;
    if (!canvas || !stage) return undefined;

    const ctx = canvas.getContext("2d");
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    let W = 0;
    let H = 0;

    function size() {
      const r = canvas.getBoundingClientRect();
      W = r.width;
      H = r.height;
      canvas.width = W * dpr;
      canvas.height = H * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    size();

    const ps = [];
    function mk() {
      const big = Math.random() < 0.14;
      // Velocidade continua e independente do tamanho: a maioria lenta, algumas rapidas.
      const speed = big
        ? 2 + Math.random() * 4
        : 4 + Math.pow(Math.random(), CFG.speedBias) * CFG.speedMax;
      return {
        x: Math.random() * W,
        y: Math.random() * H,
        r: big ? 3 + Math.random() * 5 : 0.6 + Math.random() * 1.8,
        vx: -speed,
        vy: (Math.random() - 0.4) * (1.5 + speed * 0.12),
        sway: Math.random() * 6.28,
        a: big ? 0.08 + Math.random() * 0.12 : 0.18 + Math.random() * 0.35,
        blur: big,
        // Camada de profundidade: bokeh ao fundo (move pouco), nitidas a frente (move mais).
        depth: big ? 0.2 + Math.random() * 0.25 : 0.5 + Math.random() * 0.8,
      };
    }
    // Densidade proporcional a area: mesma concentracao em qualquer janela.
    function targetCount() {
      return Math.max(50, Math.min(240, Math.round((W * H) / CFG.density)));
    }
    function ensureCount() {
      const t = targetCount();
      while (ps.length < t) ps.push(mk());
      if (ps.length > t) ps.length = t;
    }
    ensureCount();

    let mx = 0;
    let my = 0;
    let cx = 0;
    let cy = 0;
    let ox = 0;
    let oy = 0;
    let clock = 0;
    let flick = 1;
    let last = performance.now();
    let raf = 0;
    let running = false;

    function onMove(e) {
      const r = stage.getBoundingClientRect();
      mx = (e.clientX - r.left) / r.width - 0.5;
      my = (e.clientY - r.top) / r.height - 0.5;
    }
    function onLeave() {
      mx = 0;
      my = 0;
    }

    function draw() {
      const rgb = CFG.color;
      const lightEff = CFG.light * CFG.fx;
      const pa = CFG.particleAlpha * CFG.fx;
      ctx.clearRect(-40, -40, W + 80, H + 80);
      ctx.globalCompositeOperation = "lighter";
      const sg = ctx.createRadialGradient(W * 0.92, H * -0.04, 0, W * 0.92, H * -0.04, W * 0.38);
      sg.addColorStop(0, `rgba(${rgb},${0.16 * lightEff * flick})`);
      sg.addColorStop(1, `rgba(${rgb},0)`);
      ctx.fillStyle = sg;
      ctx.fillRect(-40, -40, W + 80, H + 80);
      for (let i = 0; i < ps.length; i += 1) {
        const p = ps[i];
        const dx = p.x + ox * p.depth;
        const dy = p.y + oy * p.depth;
        const alpha = p.a * pa;
        if (p.blur) {
          const g = ctx.createRadialGradient(dx, dy, 0, dx, dy, p.r * 2.2);
          g.addColorStop(0, `rgba(${rgb},${alpha})`);
          g.addColorStop(1, `rgba(${rgb},0)`);
          ctx.fillStyle = g;
          ctx.beginPath();
          ctx.arc(dx, dy, p.r * 2.2, 0, 6.283);
          ctx.fill();
        } else {
          ctx.fillStyle = `rgba(${rgb},${alpha})`;
          ctx.beginPath();
          ctx.arc(dx, dy, p.r, 0, 6.283);
          ctx.fill();
        }
      }
      ctx.globalCompositeOperation = "source-over";
    }

    function frame(t) {
      const dt = Math.min((t - last) / 1000, 0.05);
      last = t;
      clock += dt;
      flick = torch(clock, CFG.flicker);
      cx += (mx - cx) * 0.06;
      cy += (my - cy) * 0.06;
      ox = cx * 44 * CFG.parallax;
      oy = cy * 28 * CFG.parallax;
      // Glow e a camada mais ao fundo (move pouco). As particulas movem por profundidade (no draw).
      if (glow) {
        glow.style.transform = `translate(${ox * 0.4}px, ${oy * 0.4}px)`;
        glow.style.opacity = String(CFG.light * CFG.fx * flick);
      }
      if (ps.length !== targetCount()) ensureCount();
      for (let i = 0; i < ps.length; i += 1) {
        const p = ps[i];
        p.sway += dt * 0.45;
        p.x += p.vx * dt;
        p.y += (p.vy + Math.sin(p.sway) * 1.6) * dt;
        if (p.x + p.r < -20) {
          p.x = W + 20;
          p.y = Math.random() * H;
        }
        if (p.y < -20) p.y = H + 10;
        if (p.y > H + 20) p.y = -10;
      }
      draw();
      raf = requestAnimationFrame(frame);
    }

    function start() {
      if (running) return;
      running = true;
      last = performance.now();
      raf = requestAnimationFrame(frame);
    }
    function stop() {
      running = false;
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
    }
    function onVisibility() {
      if (document.hidden) stop();
      else start();
    }

    function onResize() {
      size();
      ensureCount();
      if (prefersReduced) draw();
    }

    window.addEventListener("resize", onResize);
    stage.addEventListener("pointermove", onMove);
    stage.addEventListener("pointerleave", onLeave);

    if (prefersReduced) {
      draw();
    } else {
      document.addEventListener("visibilitychange", onVisibility);
      window.addEventListener("focus", start);
      window.addEventListener("blur", stop);
      start();
    }

    return () => {
      stop();
      window.removeEventListener("resize", onResize);
      stage.removeEventListener("pointermove", onMove);
      stage.removeEventListener("pointerleave", onLeave);
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("focus", start);
      window.removeEventListener("blur", stop);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [prefersReduced]);

  // Saida cinematografica: barras abrem + zoom, dai navega.
  function leaveTo(path) {
    if (exiting) return;
    setExiting(true);
    window.setTimeout(() => navigate(path), CUT_MS);
  }

  async function handleContinue() {
    if (!recentSave || exiting) return;
    setExiting(true);
    try {
      await Promise.all([
        loadCareer(recentSave.career_id),
        new Promise((resolve) => setTimeout(resolve, CUT_MS)),
      ]);
      navigate("/dashboard");
    } catch {
      setExiting(false);
    }
  }

  const continueSub = recentSave
    ? [recentSave.category_name, recentSave.last_played ? formatDateTime(recentSave.last_played) : null]
        .filter(Boolean)
        .join(" · ")
    : "";

  const introBlur = intro && !entered;
  const shellClass = `mm-shell${entered ? " is-entered" : ""}${exiting ? " is-exiting" : ""}${
    introBlur ? " is-intro" : ""
  }`;
  const introCls = `mm-intro${logoStep === 1 ? " s-in" : logoStep === 2 ? " s-exit" : ""}`;
  const zoomBg = CFG.zoomTarget === "text" ? 1 : CFG.zoom;
  const zoomText = CFG.zoomTarget === "bg" ? 1 : CFG.zoom;
  const shellStyle = {
    "--mm-bar-h": `${CFG.barH}vh`,
    "--mm-anim": `${CFG.anim}s`,
    "--mm-zoom-bg": zoomBg,
    "--mm-zoom-text": zoomText,
    "--mm-glow": CFG.light * CFG.fx,
    "--mm-c": CFG.color,
  };
  const e = CFG.darkExtent;
  const shadeStyle = {
    opacity: CFG.dark,
    background: `linear-gradient(${CFG.darkAngle}deg, rgba(0,0,0,1) 0%, rgba(0,0,0,0.92) ${(e * 0.2).toFixed(1)}%, rgba(0,0,0,0.5) ${(e * 0.45).toFixed(1)}%, rgba(0,0,0,0.15) ${(e * 0.7).toFixed(1)}%, rgba(0,0,0,0) ${e}%)`,
  };

  return (
    <div className={shellClass} ref={stageRef} style={shellStyle}>
      <div className="mm-bg">
        <div className="mm-glow" ref={glowRef} />
        <canvas className="mm-canvas" ref={canvasRef} />
      </div>
      <div className="mm-shade" style={shadeStyle} />

      <div className="mm-bar mm-bar-top" />
      <div className="mm-bar mm-bar-bottom" />

      <div className="mm-menu">
        <p className="mm-eyebrow">Carreira</p>
        <h1 className="mm-title">LOOP</h1>
        <p className="mm-season">Temporada {new Date().getFullYear()}</p>

        <div className="mm-list">
          {recentSave ? (
            <button type="button" className="mm-card mm-hero" onClick={handleContinue}>
              <div className="mm-hero-inner">
                <span className="mm-hero-icon">
                  <img className="mm-hero-logo" src="/utilities/LOGO%20NOVA.png" alt="" />
                </span>
                <div className="mm-hero-body">
                  <div className="mm-hero-eyebrow">Continuar carreira</div>
                  <div className="mm-hero-name">{recentSave.player_name}</div>
                  <div className="mm-hero-sub">{continueSub}</div>
                </div>
                <span className="mm-hero-play">
                  <PlayIcon />
                </span>
              </div>
            </button>
          ) : null}

          <button type="button" className="mm-card mm-row" onClick={() => leaveTo("/new-career")}>
            <span className="mm-row-icon">
              <PlusIcon />
            </span>
            <span>Nova carreira</span>
          </button>

          <button type="button" className="mm-card mm-row" onClick={() => leaveTo("/load-save")}>
            <span className="mm-row-icon">
              <FolderIcon />
            </span>
            <span>Carregar save</span>
          </button>

          <button type="button" className="mm-card mm-row" onClick={() => leaveTo("/settings")}>
            <span className="mm-row-icon">
              <GearIcon />
            </span>
            <span>Configurações</span>
          </button>
        </div>
      </div>

      {intro && !introDone ? (
        <div className={introCls}>
          <div className="mm-intro-glow" />
          <img className="mm-intro-logo" src="/utilities/LOGO%20NOVA.png" alt="Loop" />
        </div>
      ) : null}
    </div>
  );
}

export default MainMenu;
