import { useEffect, useState } from "react";

import { measureOpaqueBox } from "./atlasLogoNormalization";

// Medida por arquivo, viva pelo tempo do processo: são ~10 brasões e a caixa
// opaca de um .webp não muda.
//
// Três estados, e a diferença entre os dois últimos importa: ausente = nunca
// medido; `null` = medido e não deu (canvas indisponível, arquivo ilegível);
// objeto = medida boa. Sem essa distinção não dá para saber se vale esperar mais
// um instante ou se já é hora de desenhar do jeito simples.
const opaqueBoxCache = new Map();

// Exposto só para os testes: sem isto, um caso que popula o cache contamina o
// seguinte.
export function resetLogoMeasurementCache() {
  opaqueBoxCache.clear();
}

// Devolve `{ box, measured }` para o brasão de `src`.
//
// A medida sai do CACHE durante a renderização, não de um estado sincronizado por
// efeito. É o que mata o pisca-pisca ao trocar de família: o `useState` só rodava
// o inicializador na montagem, então um `src` novo caía num primeiro frame sem
// medida — e o desenho sem medida é o encaixe simples, que é maior. Aparecia o
// brasão grande por um frame e ele encolhia logo em seguida.
//
// O estado aqui não guarda a medida; ele só existe para forçar uma re-renderização
// quando uma medição nova chega.
export function useNormalizedLogo(src) {
  const [, bumpVersion] = useState(0);

  useEffect(() => {
    if (!src) return undefined;
    if (opaqueBoxCache.has(src)) return undefined;
    if (typeof Image === "undefined") return undefined;

    let alive = true;
    const finish = (measured) => {
      opaqueBoxCache.set(src, measured);
      if (alive) bumpVersion((version) => version + 1);
    };

    const image = new Image();
    image.decoding = "async";
    image.onload = () => finish(measureOpaqueBox(image));
    // Falha de carga também é resposta: fecha o assunto e libera o desenho simples,
    // em vez de deixar o brasão escondido para sempre.
    image.onerror = () => finish(null);
    image.src = src;

    return () => {
      alive = false;
    };
  }, [src]);

  const cached = src ? opaqueBoxCache.get(src) : null;
  return { box: cached ?? null, measured: !src || opaqueBoxCache.has(src) };
}
