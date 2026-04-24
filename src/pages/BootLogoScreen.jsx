import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

function BootLogoScreen() {
  const navigate = useNavigate();

  useEffect(() => {
    const timeoutId = setTimeout(() => {
      navigate("/splash");
    }, 2000);

    return () => {
      clearTimeout(timeoutId);
    };
  }, [navigate]);

  return (
    <div className="entry-shell">
      <div className="entry-backdrop" />
      <div className="entry-glow h-[24rem] w-[24rem] bg-cyan-400/18" />
      <div className="entry-glow h-[18rem] w-[18rem] translate-x-20 translate-y-16 bg-sky-500/16" />

      <div className="relative z-10 flex animate-[bootLogoReveal_2000ms_ease-out_forwards] flex-col items-center justify-center">
        <img
          src="/utilities/logo-nova.png"
          alt="Logo Loop"
          className="h-56 w-56 object-contain drop-shadow-[0_24px_70px_rgba(88,166,255,0.18)] sm:h-72 sm:w-72"
        />
      </div>
    </div>
  );
}

export default BootLogoScreen;
