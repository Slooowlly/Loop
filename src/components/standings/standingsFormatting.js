// Formatação visual compartilhada pela classificação: tom do pódio, cor legível de
// equipe e o par de pilotos do card. Extraído de `pages/tabs/StandingsTab.jsx`.
import { getReadableTeamColor as readableTeamColor } from "../../utils/teamColors";

export function podiumClass(index) {
  if (index === 0) return "text-[#ffd700]";
  if (index === 1) return "text-[#c0c0c0]";
  if (index === 2) return "text-[#cd7f32]";
  return "text-text-secondary";
}

// Cor da equipe legível sobre o fundo escuro da tabela; valor inválido cai no
// cinza neutro.
export const getReadableTeamColor = (color) => readableTeamColor(color, { fallback: "#7d8590" });

export function formatTeamDriverName(name) {
  return typeof name === "string" && name.trim().length > 0 ? name.trim() : "-";
}

export function formatTeamDriverPair(team) {
  return `${formatTeamDriverName(team.piloto_1_nome)} / ${formatTeamDriverName(team.piloto_2_nome)}`;
}
