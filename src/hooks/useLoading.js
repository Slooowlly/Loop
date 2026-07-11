import { useEffect, useRef, useState } from "react";

// Evita o "flash" de placeholder de carregamento ao trocar de aba.
//
// Cada aba do dashboard remonta do zero (`key={activeTab}`), então seu estado
// `loading` volta a `true` e o placeholder ("Carregando classificação...") pisca
// por um instante até o fetch local (sqlite, poucos ms) terminar.
//
// Este hook só reporta `true` se o carregamento persistir além de `delay` ms.
// Fetches rápidos (o caso comum) resolvem antes disso, então o placeholder nunca
// aparece e a troca de aba fica limpa — mas se algo estiver realmente lento, o
// placeholder ainda surge depois do atraso, como esperado.
export default function useDeferredLoading(loading, delay = 220) {
  const [visible, setVisible] = useState(false);
  const timeoutRef = useRef(null);

  useEffect(() => {
    if (!loading) {
      setVisible(false);
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
      return undefined;
    }

    timeoutRef.current = setTimeout(() => setVisible(true), delay);
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };
  }, [loading, delay]);

  return visible;
}
