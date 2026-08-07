import { useTranslation } from "react-i18next";

import bronzeTrophy from "../../assets/utilities/trophies/bronze.png";
import goldTrophy from "../../assets/utilities/trophies/ouro.png";
import silverTrophy from "../../assets/utilities/trophies/prata.png";
import Tooltip from "../ui/Tooltip";

const trophyImages = {
  ouro: goldTrophy,
  prata: silverTrophy,
  bronze: bronzeTrophy,
};

function TrophyBadge({ trofeu }) {
  const { t } = useTranslation();
  // O tipo vem do banco em português ("ouro"/"prata"/"bronze") e é CHAVE, não
  // texto: um valor fora do mapa cai no ouro, que é o mesmo destino da arte.
  const tipo = trophyImages[trofeu?.tipo] ? trofeu.tipo : "ouro";
  const label = t(`standings.trophy.type.${tipo}`);
  const arte = trophyImages[tipo];

  return (
    <Tooltip
      texto={t(trofeu?.is_defending ? "standings.trophy.titleDefending" : "standings.trophy.title", {
        type: label,
      })}
    >
      <span className="relative inline-flex h-5 w-5 items-center justify-center">
        <img
          src={arte}
          alt={label}
          className="h-4 w-4 object-contain drop-shadow-[0_0_10px_rgba(255,255,255,0.16)]"
        />
        {trofeu?.is_defending ? (
          <span className="absolute -right-1 -top-1 text-[8px] font-bold text-status-green">&#9650;</span>
        ) : null}
      </span>
    </Tooltip>
  );
}

export default TrophyBadge;
