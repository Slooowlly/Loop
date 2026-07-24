import { Fragment } from "react";

import RivalMarker from "../driver/RivalMarker";
import { driverMentionClass, segmentDriverMentions } from "../../utils/driverMentions";
import { getReadableTeamColor } from "../../pages/tabs/newsHelpers";

// Colore os nomes de equipes citados no boletim de IA com a cor do time.
// `teams` é o mapa nome→cor (hex) das equipes da corrida.
export function colorizeTeams(text, teams) {
  if (!teams || typeof teams !== "object") return text;
  const names = Object.keys(teams)
    .filter(Boolean)
    .sort((a, b) => b.length - a.length);
  if (!names.length) return text;
  const escaped = names.map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`(${escaped.join("|")})`, "g");
  return text.split(re).map((part, i) =>
    teams[part] ? (
      <span key={i} style={{ color: getReadableTeamColor(teams[part]), fontWeight: 600 }}>
        {part}
      </span>
    ) : (
      part
    ),
  );
}

// Renderiza um parágrafo do boletim combinando duas camadas: nomes de PILOTO viram
// spans interativos (hover acende o piloto/equipe na classificação ao lado) e o
// restante passa por `colorizeTeams` (nomes de EQUIPE na cor do time).
export function renderBulletinParagraph(text, mentionDrivers, teams, hoveredDriverId, onHover) {
  const segments = segmentDriverMentions(text, mentionDrivers);
  if (!segments.length) {
    return colorizeTeams(text, teams);
  }
  return segments.map((seg, i) => {
    if (seg.type === "driver") {
      const isActive = hoveredDriverId === seg.id;
      return (
        <span
          key={i}
          onMouseEnter={() => onHover(seg.id)}
          onMouseLeave={() => onHover(null)}
          className={driverMentionClass(isActive, "text-[#58a6ff]", "text-white hover:text-[#58a6ff]")}
        >
          {seg.text}
          <RivalMarker driverId={seg.id} className="ml-0.5 align-middle" />
        </span>
      );
    }
    return <Fragment key={i}>{colorizeTeams(seg.text, teams)}</Fragment>;
  });
}

// Quebra o corpo da matéria em parágrafos e renderiza cada um com as duas camadas.
export function renderBulletinBody(body, mentionDrivers, teams, hoveredDriverId, onHover) {
  return body
    .split(/\n\s*\n/)
    .filter(Boolean)
    .map((para, i) => (
      <p key={i}>
        {renderBulletinParagraph(para, mentionDrivers, teams, hoveredDriverId, onHover)}
      </p>
    ));
}
