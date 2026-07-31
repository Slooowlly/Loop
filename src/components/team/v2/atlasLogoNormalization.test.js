import { describe, expect, it } from "vitest";

import {
  LOGO_FRAME_HEIGHT,
  LOGO_FRAME_WIDTH,
  LOGO_TARGET_AREA,
  normalizedLogoLayout,
} from "./atlasLogoNormalization";

// Caixa opaca de um arquivo SEM padding: o conteudo ocupa o arquivo inteiro.
function caixaCheia(naturalWidth, naturalHeight) {
  return { x: 0, y: 0, width: naturalWidth, height: naturalHeight, naturalWidth, naturalHeight };
}

// Area do conteudo visivel depois do layout — e ela que o olho compara.
function areaVisivel(layout, box) {
  const escala = layout.width / box.naturalWidth;
  return box.width * escala * (box.height * escala);
}

describe("normalizedLogoLayout", () => {
  it("da a MESMA area a um escudo quadrado e a um letreiro deitado", () => {
    // O ponto da normalizacao. M2 Cup e ~1:1, Global Endurance e ~2:1; encaixar os
    // dois numa caixa comum igualava a caixa, nao o tamanho.
    const escudo = caixaCheia(690, 645);
    const letreiro = caixaCheia(854, 430);

    const areaEscudo = areaVisivel(normalizedLogoLayout(escudo), escudo);
    const areaLetreiro = areaVisivel(normalizedLogoLayout(letreiro), letreiro);

    expect(areaEscudo).toBeCloseTo(LOGO_TARGET_AREA, 0);
    expect(areaLetreiro).toBeCloseTo(LOGO_TARGET_AREA, 0);
  });

  it("mantem os dois extremos dentro da moldura", () => {
    for (const box of [caixaCheia(690, 645), caixaCheia(854, 430), caixaCheia(638, 490)]) {
      const layout = normalizedLogoLayout(box);
      const escala = layout.width / box.naturalWidth;
      expect(box.width * escala).toBeLessThanOrEqual(LOGO_FRAME_WIDTH + 0.01);
      expect(box.height * escala).toBeLessThanOrEqual(LOGO_FRAME_HEIGHT + 0.01);
    }
  });

  it("prefere encostar na moldura a cortar um brasao de proporcao extrema", () => {
    // Area-alvo pediria algo mais alto do que a moldura comporta: o limite da
    // moldura vence, e o conteudo aparece inteiro, menor.
    const fitaVertical = caixaCheia(40, 900);
    const layout = normalizedLogoLayout(fitaVertical);
    const escala = layout.width / fitaVertical.naturalWidth;

    expect(fitaVertical.height * escala).toBeCloseTo(LOGO_FRAME_HEIGHT, 5);
    expect(areaVisivel(layout, fitaVertical)).toBeLessThan(LOGO_TARGET_AREA);
  });

  it("centra o conteudo VISIVEL, nao o arquivo", () => {
    // A MX-5 Cup tem 46% do arquivo vazio, quase tudo embaixo. Centrar o arquivo
    // empurraria a marca para cima; centrar a caixa opaca a deixa no lugar.
    const comPaddingEmbaixo = { x: 0, y: 0, width: 884, height: 500, naturalWidth: 884, naturalHeight: 708 };
    const layout = normalizedLogoLayout(comPaddingEmbaixo);
    const escala = layout.width / comPaddingEmbaixo.naturalWidth;

    const centroVisivelY = layout.top + (comPaddingEmbaixo.y + comPaddingEmbaixo.height / 2) * escala;
    expect(centroVisivelY).toBeCloseTo(LOGO_FRAME_HEIGHT / 2, 5);

    // E o arquivo inteiro fica descentrado — e o que prova que a correcao ocorreu.
    const centroDoArquivoY = layout.top + layout.height / 2;
    expect(centroDoArquivoY).toBeGreaterThan(LOGO_FRAME_HEIGHT / 2);
  });

  it("devolve nulo quando nao ha medida utilizavel", () => {
    expect(normalizedLogoLayout(null)).toBeNull();
    expect(normalizedLogoLayout({ x: 0, y: 0, width: 0, height: 0, naturalWidth: 0, naturalHeight: 0 })).toBeNull();
  });
});
