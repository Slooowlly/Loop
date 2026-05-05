import bronzeTrophy from "../../assets/utilities/trophies/bronze.png";
import goldTrophy from "../../assets/utilities/trophies/ouro.png";
import silverTrophy from "../../assets/utilities/trophies/prata.png";

const trophyImages = {
  ouro: goldTrophy,
  prata: silverTrophy,
  bronze: bronzeTrophy,
};

function TrophyBadge({ trofeu }) {
  const src = trophyImages[trofeu?.tipo] ?? goldTrophy;
  const label = trofeu?.tipo ?? "troféu";

  return (
    <span
      className="relative inline-flex h-5 w-5 items-center justify-center"
      title={`Troféu ${label}${trofeu?.is_defending ? " (campeão defensor)" : ""}`}
    >
      <img
        src={src}
        alt={label}
        className="h-4 w-4 object-contain drop-shadow-[0_0_10px_rgba(255,255,255,0.16)]"
      />
      {trofeu?.is_defending ? (
        <span className="absolute -right-1 -top-1 text-[8px] font-bold text-status-green">&#9650;</span>
      ) : null}
    </span>
  );
}

export default TrophyBadge;
