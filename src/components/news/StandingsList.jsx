import TeamLogoMark from "../team/TeamLogoMark";
import { getTeamGlow } from "../../utils/teamColors";

// Lista de classificação da página direita da revista (construtores, pilotos ou
// favoritos). `hoveredId` acende a linha correspondente ao nome que o boletim
// está destacando; `onHover` só é passado nas listas em que a linha também
// realça de volta o nome no texto.
function StandingsList({ rows, hoveredId, onHover, logoTestId }) {
  return (
    <div className={`res-list${rows.length > 12 ? " res-list--split" : ""}`}>
      {rows.map((r) => {
        const glow = hoveredId != null && r.id === hoveredId;
        const tone = glow ? getTeamGlow(r.color) : null;
        const hoverProps = onHover
          ? {
              onMouseEnter: () => onHover(r.id),
              onMouseLeave: () => onHover(null),
            }
          : {};
        return (
          <div
            key={r.id ?? r.pos}
            className={r.me ? "res-row me" : "res-row"}
            style={
              tone ? { background: tone.soft, boxShadow: `inset 0 0 0 1.5px ${tone.solid}` } : undefined
            }
            {...hoverProps}
          >
            <span className="rp">{r.pos}</span>
            <TeamLogoMark teamName={r.teamName ?? r.name} color={r.color} size="xs" testId={logoTestId} />
            <span className="rn">{r.name}</span>
            <span className="rpts">{r.pts ?? r.skill}</span>
          </div>
        );
      })}
    </div>
  );
}

export default StandingsList;
