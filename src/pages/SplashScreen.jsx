import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

function SplashScreen() {
  const navigate = useNavigate();
  const [step, setStep] = useState(0); // 0 = inicial, 1 = zoom-in, 2 = zoom-push/saida

  useEffect(() => {
    const a = window.setTimeout(() => setStep(1), 30); // entra com zoom
    const b = window.setTimeout(() => setStep(2), 1200); // zoom final
    const c = window.setTimeout(() => navigate("/menu"), 1750); // revela o menu no fim
    return () => {
      window.clearTimeout(a);
      window.clearTimeout(b);
      window.clearTimeout(c);
    };
  }, []);

  const cls =
    step === 0 ? "splash-logo" : step === 1 ? "splash-logo s-in" : "splash-logo s-exit";

  return (
    <div className="splash-shell">
      <img className={cls} src="/utilities/LOGO%20NOVA.png" alt="Loop" />
    </div>
  );
}

export default SplashScreen;
