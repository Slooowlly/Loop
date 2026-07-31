import { useLayoutEffect, useRef, useState } from "react";

// Mede o tamanho real de um elemento e reage a mudanças de viewport.
//
// O gráfico do atlas v2 calcula as posições em pixels da própria área de plotagem
// (requisito 16 do design: nada de largura fixa nem de porcentagem da página), então
// precisa saber quanto espaço o card recebeu de fato depois do grid resolver.
//
// `fallback` cobre ambientes sem ResizeObserver (jsdom nos testes): o componente
// ainda renderiza uma geometria coerente em vez de colapsar para zero.
export function useElementSize(fallback = { width: 960, height: 520 }) {
  const ref = useRef(null);
  const [size, setSize] = useState(fallback);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return undefined;

    function measure() {
      const { width, height } = element.getBoundingClientRect();
      if (width > 0 && height > 0) {
        setSize((current) => (current.width === width && current.height === height ? current : { width, height }));
      }
    }

    measure();

    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }

    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  return [ref, size];
}

export default useElementSize;
