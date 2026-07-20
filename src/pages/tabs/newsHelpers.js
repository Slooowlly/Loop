// Helper de cor de equipe para a revista de notícias (NewsMagazineTab). Único
// sobrevivente do antigo módulo de helpers da NewsTab (aposentada): clareia cores
// de equipe muito escuras para permanecerem legíveis sobre o fundo escuro da revista.
export function getReadableTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) return "#d0d7e2";
  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  if (luminance < 0.32) {
    const mixWithWhite = 0.62;
    const boost = (channel) => Math.round(channel + (255 - channel) * mixWithWhite);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }
  return color;
}
