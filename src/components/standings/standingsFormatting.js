// Formatação visual compartilhada pela classificação: tom do pódio, cor legível de
// equipe e o par de pilotos do card. Extraído de `pages/tabs/StandingsTab.jsx`.

export function podiumClass(index) {
  if (index === 0) return "text-[#ffd700]";
  if (index === 1) return "text-[#c0c0c0]";
  if (index === 2) return "text-[#cd7f32]";
  return "text-text-secondary";
}

// Cor da equipe legível sobre o fundo escuro da tabela: cores muito escuras são
// clareadas; valor inválido cai no cinza neutro.
export function getReadableTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) {
    return "#7d8590";
  }

  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

  if (luminance < 0.32) {
    const mixWithWhite = 0.58;
    const boost = (channel) => Math.round(channel + (255 - channel) * mixWithWhite);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }

  return color;
}

export function formatTeamDriverName(name) {
  return typeof name === "string" && name.trim().length > 0 ? name.trim() : "-";
}

export function formatTeamDriverPair(team) {
  return `${formatTeamDriverName(team.piloto_1_nome)} / ${formatTeamDriverName(team.piloto_2_nome)}`;
}
