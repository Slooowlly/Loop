import { describe, expect, it } from "vitest";

import { getBannerImageFocus, getBannerImageSrc } from "./trackBanners";

// O resolvedor da panorâmica do banner da Home. Ele existe SEPARADO das miniaturas
// (`utils/trackImages.js`) porque os conjuntos de arte são diferentes; o que os dois
// compartilham é só a normalização de nome. O modo de falha que importa é silencioso:
// um apelido que deixa de casar não quebra nada — o banner simplesmente cai no
// fundo-gradiente, e a foto some sem erro no console.

const PASTA = "/utilities/tracks/Pistas%20Header";

describe("getBannerImageSrc", () => {
  it("resolve pelo apelido, não só pelo nome oficial", () => {
    // Interlagos e Autódromo José Carlos Pace são a mesma pista, e o calendário usa
    // as duas grafias conforme a origem do dado.
    expect(getBannerImageSrc("Interlagos")).toBe(
      `${PASTA}/${encodeURIComponent("Autódromo José Carlos Pace.jpg")}`,
    );
    expect(getBannerImageSrc("Autódromo José Carlos Pace")).toBe(
      getBannerImageSrc("Interlagos"),
    );
  });

  it("casa por trecho e ignorando acento/caixa — é assim que o iRacing devolve o nome", () => {
    const spa = getBannerImageSrc("Circuit de Spa-Francorchamps - Grand Prix Pits");
    expect(spa).toContain(encodeURIComponent("Circuit de Spa-Francorchamps.jpg"));

    // "Nürburgring" com trema e em caixa alta tem que cair no mesmo arquivo.
    expect(getBannerImageSrc("NÜRBURGRING NORDSCHLEIFE")).toContain(
      encodeURIComponent("Nürburgring Nordschleife.jpg"),
    );
  });

  it("escapa o nome do arquivo — acento e espaço no disco viram URL válida", () => {
    const monza = getBannerImageSrc("Monza");
    expect(monza.startsWith(`${PASTA}/`)).toBe(true);
    // Nada de espaço cru nem de acento cru sobrevive à montagem da URL.
    expect(monza).not.toMatch(/[ ]/);
    expect(monza).toContain("%");
  });

  it("sem panorâmica, cai na miniatura em vez de devolver caminho quebrado", () => {
    const src = getBannerImageSrc("Pista Que Não Existe");
    expect(src).toBeTruthy();
    expect(src.startsWith(PASTA)).toBe(false);
  });

  it("não estoura com pista ausente", () => {
    expect(() => getBannerImageSrc(null)).not.toThrow();
    expect(() => getBannerImageSrc(undefined)).not.toThrow();
    expect(() => getBannerImageSrc("")).not.toThrow();
  });
});

describe("getBannerImageFocus", () => {
  it("devolve o foco calibrado da pista", () => {
    // Brands Hatch é a calibração mais extrema do mapa (corte lá embaixo): serve de
    // sentinela de que o override por pista está mesmo sendo lido.
    expect(getBannerImageFocus("Brands Hatch")).toBe("center 86%");
    expect(getBannerImageFocus("Adelaide Street Circuit")).toBe("67% 55%");
  });

  it("cai no foco padrão quando a entrada não tem calibração própria", () => {
    // Detroit está no mapa sem `focus`.
    expect(getBannerImageFocus("Detroit Grand Prix at Belle Isle")).toBe("center 38%");
  });

  it("cai no foco padrão quando a pista não está no mapa", () => {
    expect(getBannerImageFocus("Pista Que Não Existe")).toBe("center 38%");
    expect(getBannerImageFocus(null)).toBe("center 38%");
  });

  it("devolve sempre um object-position aceitável pelo CSS", () => {
    const pistas = ["Monza", "Suzuka", "Adelaide Street Circuit", "Pista Inventada"];
    pistas.forEach((pista) => {
      expect(getBannerImageFocus(pista)).toMatch(/^(center|\d+%)\s+\d+%$/);
    });
  });
});
