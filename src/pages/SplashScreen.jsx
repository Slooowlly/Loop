import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

function SplashScreen() {
  const navigate = useNavigate();

  useEffect(() => {
    const timer = setTimeout(() => navigate("/menu"), 1800);
    return () => clearTimeout(timer);
  }, []);

  return (
    <div className="entry-shell px-4">
      <div className="entry-backdrop" />
      <div className="entry-glow left-[12%] top-[12%] h-56 w-56 bg-cyan-400/16" />
      <div className="entry-glow bottom-[14%] right-[10%] h-64 w-64 bg-sky-500/14" />

      <div className="entry-panel text-center">
        <img
          src="/logo-nova.png"
          alt="Logo iRacerApp"
          className="h-28 w-28 object-contain drop-shadow-[0_16px_40px_rgba(88,166,255,0.14)]"
        />

        <div className="space-y-2">
          <p className="text-[11px] font-semibold uppercase tracking-[0.35em] text-accent-primary/70">
            Launcher
          </p>
          <h1 className="text-3xl font-semibold tracking-[0.08em] text-text-primary">
            iRacerApp
          </h1>
          <p className="text-xs uppercase tracking-[0.28em] text-text-secondary">
            v0.1.0
          </p>
        </div>

        <p className="animate-pulse text-[11px] uppercase tracking-[0.22em] text-text-muted">
          Carregando...
        </p>
      </div>
    </div>
  );
}

export default SplashScreen;
