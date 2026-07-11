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
