// Cor de equipe legível como TEXTO sobre fundo escuro: cores com luminância baixa
// são misturadas com branco até voltarem a ser lidas. Cada tela passa seu próprio
// `fallback` (o que mostrar quando não há cor de equipe) e, se precisar, seu `mix`.
// Os defaults aqui são os de 4 dos 6 pontos de uso — quem diverge diz isso na chamada.
export function getReadableTeamColor(color, { fallback = "#c9d1d9", mix = 0.58 } = {}) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return fallback;
  }

  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

  if (luminance < 0.32) {
    const boost = (channel) => Math.round(channel + (255 - channel) * mix);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }

  return color;
}

// Destaque (glow) por cor de equipe. Cores muito escuras (ex.: Thunderline Academy)
// ficam quase invisíveis como brilho sobre um fundo escuro, então clareamos para um
// tom claro/branco quando a luminância é baixa. Retorna variações prontas:
//   solid → borda / linha (opaco)
//   soft  → fundo tênue da linha realçada
//   glow  → brilho externo (box-shadow) de cards
export function getTeamGlow(color) {
  let r = 201;
  let g = 209;
  let b = 217; // fallback cinza-claro (#c9d1d9) quando a cor é inválida/ausente
  if (typeof color === "string" && /^#([0-9a-f]{6})$/i.test(color)) {
    r = parseInt(color.slice(1, 3), 16);
    g = parseInt(color.slice(3, 5), 16);
    b = parseInt(color.slice(5, 7), 16);
    const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
    if (luminance < 0.35) {
      const mix = 0.6; // mistura com branco: clareia sem perder totalmente o matiz
      r = Math.round(r + (255 - r) * mix);
      g = Math.round(g + (255 - g) * mix);
      b = Math.round(b + (255 - b) * mix);
    }
  }
  return {
    solid: `rgb(${r}, ${g}, ${b})`,
    soft: `rgba(${r}, ${g}, ${b}, 0.20)`,
    glow: `rgba(${r}, ${g}, ${b}, 0.45)`,
  };
}
