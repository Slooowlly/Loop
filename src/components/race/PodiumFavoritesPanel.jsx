import { useTranslation } from "react-i18next";

import RivalMarker from "../driver/RivalMarker";
import TeamLogoMark from "../team/TeamLogoMark";
import { getTeamGlow } from "../../utils/teamColors";
import { getFavoriteMedalTone, getReadableTeamColor } from "./raceGridContext";

// Coluna 2 da Sala de Estratégia: aviso de contrato expirando, as três metas do fim
// de semana e o ranking de favoritos ao pódio (com a forma recente em chips).
function PodiumFavoritesPanel({
  goals,
  favorites,
  isLoading,
  hoveredDriverId,
  contractWarning,
  showContractWarning,
}) {
  const { t } = useTranslation();

  return (
    <div className="xl:col-span-4 flex flex-col gap-5 xl:h-[calc(100vh-17rem)] xl:min-h-[650px]">
      {/* Aviso de contrato expirando */}
      {showContractWarning && (
        <div className="bg-amber-900/30 border border-amber-500/40 rounded-2xl px-4 py-3 flex items-start gap-3">
          <span className="text-amber-400 text-base leading-none mt-0.5">⚠</span>
          <div>
            <p className="text-[10px] uppercase tracking-[0.15em] text-amber-400 font-bold mb-0.5">{t("nextRaceTab.labels.contractExpiring")}</p>
            <p className="text-xs text-amber-100 leading-relaxed">
              {t("nextRaceTab.labels.contractWarningPrefix")} <span className="font-semibold">{contractWarning.equipe_nome}</span> {t("nextRaceTab.labels.contractWarningSuffix")}
            </p>
          </div>
        </div>
      )}

      {/* Metas */}
      <div className="grid grid-cols-3 gap-3">
        <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
          <span className="text-2xl mb-1.5 block leading-none">👥</span>
          <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">{t("nextRaceTab.labels.goalTeam")}</p>
          <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
            {goals[0]?.value}
          </p>
        </div>
        <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
          <span className="text-xl mb-1.5 block leading-none">👤</span>
          <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">{t("nextRaceTab.labels.goalPersonal")}</p>
          <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
            {goals[1]?.value}
          </p>
        </div>
        <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-2xl p-4 text-center flex flex-col justify-start items-center">
          <span className="text-xl mb-1.5 block leading-none">🏆</span>
          <p className="text-[9px] uppercase font-bold text-gray-500 tracking-wider">{t("nextRaceTab.labels.goalTitle")}</p>
          <p className="text-[10px] text-white font-semibold mt-1 leading-tight">
            {goals[2]?.value}
          </p>
        </div>
      </div>

      {/* Favoritos ao Pódio */}
      <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-6 flex-1 flex flex-col min-h-0">
        <p className="text-[12px] font-bold uppercase tracking-[0.2em] text-[#58a6ff] mb-5">{t("nextRaceTab.labels.podiumFavorites")}</p>

        <div className="space-y-4 flex-1 overflow-y-auto custom-scrollbar pr-1">
          {isLoading ? (
            <p className="text-sm text-gray-400">{t("nextRaceTab.labels.buildingAnalysis")}</p>
          ) : (
            favorites.map((driver, index) => {
              const medalTone = getFavoriteMedalTone(index);
              const isJogador = driver.is_jogador;
              const isMentionHovered = hoveredDriverId != null && driver.id === hoveredDriverId;

              return (
                <div
                  key={driver.id}
                  className={`border rounded-2xl p-4 flex flex-col xl:flex-row gap-3 xl:gap-0 justify-between xl:items-center transition hover:bg-white/5 ${
                    isJogador ? "bg-[#58a6ff]/10 border-[#58a6ff]/30" : "bg-black/20 border-white/5"
                  }`}
                  style={
                    isMentionHovered
                      ? (() => {
                          const tone = getTeamGlow(driver.equipe_cor);
                          return { borderColor: tone.solid, boxShadow: `0 0 18px ${tone.glow}` };
                        })()
                      : undefined
                  }
                >
                  <div className="flex items-center gap-4">
                    <span className={`font-black w-8 text-center text-[30px] ${isJogador ? "text-[#58a6ff]" : medalTone}`}>
                      {index + 1}
                    </span>
                    <TeamLogoMark
                      teamName={driver.equipe_nome}
                      color={driver.equipe_cor}
                      size="sm"
                      testId="strategy-favorite-team-logo"
                    />
                    <div>
                      <p className="text-base font-bold text-white leading-none mb-1.5 flex items-center gap-1.5">
                        {driver.nome}
                        <RivalMarker driverId={driver.id} />
                      </p>
                      <p
                        className="text-[11px] font-bold uppercase"
                        style={{ color: getReadableTeamColor(driver.equipe_cor) }}
                      >
                        {driver.equipe_nome}
                      </p>
                    </div>
                  </div>
                  <div className="flex gap-1.5 justify-end xl:ml-0 overflow-x-auto custom-scrollbar pb-1 xl:pb-0">
                    {driver.formChips.map((chip, chipIdx) => {
                      let customStyle = "bg-gray-500/10 text-gray-400 border-gray-500/30";
                      if (chip.label === "P1") customStyle = "bg-[#f5c76d]/10 text-[#f5c76d] border-[#f5c76d]/30";
                      else if (chip.label === "P2") customStyle = "bg-[#d8dfef]/10 text-[#d8dfef] border-[#d8dfef]/30";
                      else if (chip.label === "P3") customStyle = "bg-[#cf8d63]/10 text-[#cf8d63] border-[#cf8d63]/30";
                      else if (chip.label.includes("DNF")) customStyle = "bg-red-500/10 text-red-500 border-red-500/30";
                      else if (chip.label.startsWith("P") && parseInt(chip.label.substring(1)) <= 6)
                        customStyle = "bg-[#58a6ff]/10 text-[#58a6ff] border-[#58a6ff]/30";

                      return (
                        <span
                          key={chipIdx}
                          className={`border px-2 py-1 rounded text-[10px] whitespace-nowrap font-bold ${customStyle}`}
                        >
                          {chip.label}
                        </span>
                      );
                    })}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}

export default PodiumFavoritesPanel;
