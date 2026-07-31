import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import MagazineCredits from "./MagazineCredits";
import StandingsList from "./StandingsList";
import WorldNotes from "./WorldNotes";
import { renderBulletinBody } from "./bulletinText";
import { getTrackImageSrc } from "../../utils/trackImages";

// "O Que Esperar": matéria de PRÉ-TEMPORADA. Só existe antes da 1ª corrida —
// substitui o "livro fechado" por um spread aberto. Ver docs/season-preview-design.md.
function PreseasonSpread({
  catLabel,
  year,
  preview,
  openingRace,
  standings,
  driverStandings,
  playerTeam,
  worldNotes,
}) {
  const { t } = useTranslation();
  const [hoveredDriverId, setHoveredDriverId] = useState(null);

  // Coluna direita da pré-temporada: as equipes da categoria com o campeonato
  // ainda ZERADO. Potencial é dado interno — não vira tabela pública.
  const construtores = useMemo(() => {
    return standings.map((tm, i) => ({
      id: tm.id,
      pos: tm.posicao ?? i + 1,
      name: tm.nome,
      pts: tm.pontos ?? 0,
      color: tm.cor_primaria || "#888",
      me: playerTeam?.id != null && tm.id === playerTeam.id,
    }));
  }, [standings, playerTeam]);

  const mentionDrivers = useMemo(
    () => driverStandings.map((d) => ({ id: d.id, nome: d.nome })),
    [driverStandings],
  );

  return (
    <>
      <div className="spread">
        {/* PÁGINA ESQUERDA — a matéria "O Que Esperar" */}
        <div className="page page-l">
          <div className="flag" />
          <div className="kicker">
            {catLabel} · {t("newsMagazine.preseason.kicker")}
          </div>
          <h1 className="display">
            <span className="l1">{preview?.headline || t("newsMagazine.preseason.titleLine")}</span>
            <span className="l2">
              {preview?.standfirst || t("newsMagazine.preseason.seasonLine", { year })}
            </span>
          </h1>
          <span className="ai-tag">
            {preview?.loading
              ? t("newsMagazine.preseason.aiTag.generating")
              : preview?.body
              ? t("newsMagazine.preseason.aiTag.ready")
              : t("newsMagazine.preseason.aiTag.comingSoon")}
          </span>

          <div className="prose-cols">
            <h3 className="subhead">{t("newsMagazine.preseason.articleHead")}</h3>
            {preview?.body ? (
              renderBulletinBody(
                preview.body,
                mentionDrivers,
                preview.teams,
                hoveredDriverId,
                setHoveredDriverId,
              )
            ) : preview?.loading ? (
              <p>{t("newsMagazine.preseason.generating")}</p>
            ) : (
              <p>{t("newsMagazine.preseason.placeholder")}</p>
            )}
          </div>
        </div>

        {/* PÁGINA DIREITA — pista de abertura + construtores zerados */}
        <div className="page page-r">
          <MagazineCredits catLabel={catLabel} />
          {openingRace ? (
            <img
              className="photo"
              src={getTrackImageSrc(openingRace.track_name, openingRace.track_id)}
              alt={openingRace.track_name}
            />
          ) : (
            <div className="photo" />
          )}
          <p className="cap">
            {openingRace
              ? t("newsMagazine.preseason.caption", { track: openingRace.track_name })
              : t("newsMagazine.preseason.captionGeneric")}
          </p>

          <div className="r-grid r-grid-single">
            <div>
              <div className="nm-standings-head">
                <h3 className="subhead">
                  {t("newsMagazine.standings.constructors")}
                  {year ? <> · {year}</> : null}
                </h3>
              </div>
              {construtores.length > 0 ? (
                <StandingsList rows={construtores} logoTestId="news-team-logo" />
              ) : (
                <p>{t("newsMagazine.standings.teamsUnavailable")}</p>
              )}
            </div>
          </div>
        </div>
      </div>

      <WorldNotes notes={worldNotes} />

      <div className="mag-foot">
        <div className="foot-left">
          <div className="brand">
            GRID<span>·</span>MAGAZINE
          </div>
        </div>
        <div className="foot-meta">
          {year ? t("newsMagazine.preseason.footSeason", { year }) : t("newsMagazine.preseason.foot")}
        </div>
      </div>
    </>
  );
}

export default PreseasonSpread;
