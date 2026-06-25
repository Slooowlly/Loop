import { useEffect, useRef } from "react";

// Campo de partículas reutilizável — mesmo visual do MainMenu (partículas da
// direita p/ esquerda, parallax por profundidade e brilho tipo tocha), porém
// isolado e sem o aparato de intro/letterbox do menu. Serve de fundo "glass":
// o painel por cima borra as partículas via backdrop-filter.
const CFG = {
  speedMax: 47, // topo da velocidade das partículas
  speedBias: 8, // expoente: maior = rápidas mais raras
  density: 30000, // divisor de área: menor = mais partículas
  light: 0.55, // intensidade base da luz
  particleAlpha: 2.45, // brilho das partículas
  parallax: 0.4, // intensidade do parallax
  color: "92,228,255", // #5ce4ff (rgb)
  flicker: 0.8, // oscilação tipo tocha
  fx: 2.6, // intensidade geral (multiplica luz + partículas)
};

// Oscilação de intensidade tipo tocha: flicker lento + labareda a cada 8s.
function torch(t, amt) {
  const flicker =
    0.08 * Math.sin(t * 0.9) +
    0.05 * Math.sin(t * 2.1 + 1.7) +
    0.04 * Math.sin(t * 3.7 + 0.5);
  const phase = t % 8;
  const surge = Math.exp(-((phase - 1) ** 2) / (2 * 0.6 * 0.6));
  return 1 + amt * (flicker + 0.9 * surge);
}

function ParticleBackdrop({ blur = 44 }) {
  const rootRef = useRef(null);
  const canvasRef = useRef(null);
  const glowRef = useRef(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const stage = rootRef.current;
    const glow = glowRef.current;
    if (!canvas || !stage) return undefined;

    const prefersReduced =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

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
        depth: big ? 0.2 + Math.random() * 0.25 : 0.5 + Math.random() * 0.8,
      };
    }
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
  }, []);

  return (
    <div
      ref={rootRef}
      className="absolute inset-0 z-0 overflow-hidden"
      style={{ background: "#040810" }}
      aria-hidden="true"
    >
      <div className="mm-glow" ref={glowRef} />
      <canvas
        className="mm-canvas"
        ref={canvasRef}
        style={{
          // Folga grande além da tela (sobrescreve o -4%/108% da classe) para que
          // o blur forte não revele bordas escuras nos cantos.
          inset: "-18%",
          width: "136%",
          height: "136%",
          filter: blur ? `blur(${blur}px)` : undefined,
        }}
      />
    </div>
  );
}

export default ParticleBackdrop;
