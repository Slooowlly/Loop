import { useNavigate } from "react-router-dom";

function MainMenu() {
  const navigate = useNavigate();

  return (
    <div className="entry-shell px-4">
      <div className="entry-backdrop" />
      <div className="entry-glow left-[10%] top-[14%] h-64 w-64 bg-cyan-400/14" />
      <div className="entry-glow bottom-[12%] right-[8%] h-72 w-72 bg-sky-500/12" />

      <div className="entry-panel text-center">
        <img
          src="/logo-nova.png"
          alt="Logo iRacerApp"
          className="h-24 w-24 object-contain drop-shadow-[0_16px_36px_rgba(88,166,255,0.12)]"
        />

        <div className="space-y-2">
          <p className="text-[11px] font-semibold uppercase tracking-[0.35em] text-accent-primary/70">
            Menu Principal
          </p>
          <h1 className="text-2xl font-semibold tracking-[0.05em] text-text-primary">
            Bem-vindo de volta
          </h1>
        </div>

        <div className="flex w-full flex-col gap-3 pt-2">
          <button
            onClick={() => navigate("/new-career")}
            className="entry-action"
          >
            NOVA CARREIRA
          </button>

          <button
            onClick={() => navigate("/load-save")}
            className="entry-action"
          >
            CARREGAR SAVE
          </button>

          <button
            onClick={() => navigate("/settings")}
            className="entry-action"
          >
            CONFIGURACOES
          </button>
        </div>
      </div>
    </div>
  );
}

export default MainMenu;
