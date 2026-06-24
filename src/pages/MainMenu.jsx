import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";

import useCareerStore from "../stores/useCareerStore";
import { formatDateTime } from "../utils/formatters";

function WheelIcon() {
  return (
    <svg
      width="26"
      height="26"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="9" />
      <circle cx="12" cy="12" r="2.2" fill="currentColor" stroke="none" />
      <path d="M12 9.8V3.2" />
      <path d="M10.1 13.6 4.6 18.2" />
      <path d="M13.9 13.6 19.4 18.2" />
    </svg>
  );
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

function MainMenu() {
  const navigate = useNavigate();
  const loadCareer = useCareerStore((state) => state.loadCareer);

  const stageRef = useRef(null);
  const canvasRef = useRef(null);
  const glowRef = useRef(null);

  const [recentSave, setRecentSave] = useState(null);
  const [entered, setEntered] = useState(false);
  const [exiting, setExiting] = useState(false);

  const prefersReduced =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const EXIT_MS = prefersReduced ? 0 : 700;

  // Save mais recente para o cartão "Continuar".
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

  // Entrada cinematografica: barras pretas fecham e o menu surge.
  useEffect(() => {
    const id = window.setTimeout(() => setEntered(true), 30);
    return () => window.clearTimeout(id);
  }, []);

  // Fundo animado: partículas (direita -> esquerda, velocidades mistas) + parallax só no fundo.
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
      // Velocidade continua e independente do tamanho: a maioria lenta, algumas rapidas,
      // tudo no meio do caminho tambem (sem os dois "baldes"). Particulas iguais podem
      // ter velocidades diferentes.
      const speed = big
        ? 2 + Math.random() * 4
        : 4 + Math.pow(Math.random(), 4) * 30;
      return {
        x: Math.random() * W,
        y: Math.random() * H,
        r: big ? 3 + Math.random() * 5 : 0.6 + Math.random() * 1.8,
        vx: -speed,
        vy: (Math.random() - 0.4) * (1.5 + speed * 0.12),
        sway: Math.random() * 6.28,
        a: big ? 0.08 + Math.random() * 0.12 : 0.18 + Math.random() * 0.35,
        blur: big,
      };
    }
    // Densidade proporcional a area: mesma concentracao do template em qualquer janela.
    function targetCount() {
      return Math.max(50, Math.min(170, Math.round((W * H) / 14000)));
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
      ctx.clearRect(-30, -30, W + 60, H + 60);
      ctx.globalCompositeOperation = "lighter";
      const sg = ctx.createRadialGradient(W * 0.92, H * -0.04, 0, W * 0.92, H * -0.04, W * 0.38);
      sg.addColorStop(0, "rgba(190,230,255,0.12)");
      sg.addColorStop(1, "rgba(111,212,255,0)");
      ctx.fillStyle = sg;
      ctx.fillRect(-30, -30, W + 60, H + 60);
      for (let i = 0; i < ps.length; i += 1) {
        const p = ps[i];
        if (p.blur) {
          const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, p.r * 2.2);
          g.addColorStop(0, `rgba(111,212,255,${p.a})`);
          g.addColorStop(1, "rgba(111,212,255,0)");
          ctx.fillStyle = g;
          ctx.beginPath();
          ctx.arc(p.x, p.y, p.r * 2.2, 0, 6.283);
          ctx.fill();
        } else {
          ctx.fillStyle = `rgba(198,233,255,${p.a})`;
          ctx.beginPath();
          ctx.arc(p.x, p.y, p.r, 0, 6.283);
          ctx.fill();
        }
      }
      ctx.globalCompositeOperation = "source-over";
    }

    function frame(t) {
      const dt = Math.min((t - last) / 1000, 0.05);
      last = t;
      cx += (mx - cx) * 0.06;
      cy += (my - cy) * 0.06;
      if (glow) glow.style.transform = `translate(${cx * 30}px, ${cy * 20}px)`;
      canvas.style.transform = `translate(${cx * 18}px, ${cy * 12}px)`;
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

    const reduce =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    function onResize() {
      size();
      ensureCount();
      if (reduce) draw();
    }

    window.addEventListener("resize", onResize);
    stage.addEventListener("pointermove", onMove);
    stage.addEventListener("pointerleave", onLeave);

    if (reduce) {
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
  }, []);

  // Saida cinematografica: barras abrem + menu da zoom, dai navega.
  function leaveTo(path) {
    if (exiting) return;
    setExiting(true);
    window.setTimeout(() => navigate(path), EXIT_MS);
  }

  async function handleContinue() {
    if (!recentSave || exiting) return;
    setExiting(true);
    try {
      await Promise.all([
        loadCareer(recentSave.career_id),
        new Promise((resolve) => setTimeout(resolve, EXIT_MS)),
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

  const shellClass = `mm-shell${entered ? " is-entered" : ""}${exiting ? " is-exiting" : ""}`;

  return (
    <div className={shellClass} ref={stageRef}>
      <div className="mm-glow" ref={glowRef} />
      <canvas className="mm-canvas" ref={canvasRef} />
      <div className="mm-shade" />

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
                  <WheelIcon />
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
    </div>
  );
}

export default MainMenu;
