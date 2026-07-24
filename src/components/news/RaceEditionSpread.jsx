import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import MagazineCredits from "./MagazineCredits";
import StandingsList from "./StandingsList";
import WorldNotes from "./WorldNotes";
import { renderBulletinBody } from "./bulletinText";
import { getTrackImageSrc } from "../../utils/trackImages";
import { isPortuguese, localizedAiError } from "../../utils/aiFallback";

// Spread de uma edição já disputada: matéria da corrida à esquerda, foto da pista
// e classificação à direita, notas do mundo e navegação entre edições no rodapé.
function RaceEditionSpread({
  ed,
  year,
  catLabel,
  kicker,
  footMeta,
  bulletin,
  language,
  standings,
  driverStandings,
  playerTeam,
  worldNotes,
  onGoEdition,
  canGoPrev,
  canGoNext,
}) {
  const { t } = useTranslation();
  // Classificação ao lado do boletim: alterna entre pilotos (padrão) e equipes.
  const [standingsView, setStandingsView] = useState("pilotos");
  // Piloto realçado ao passar o mouse no nome dele no boletim → acende a equipe dele
  // (view construtores) ou ele mesmo (view pilotos) na classificação.
  const [hoveredDriverId, setHoveredDriverId] = useState(null);

  // Construtores: grid completo da categoria, por posição no campeonato.
  const construtores = useMemo(() => {
    return standings.map((tm) => ({
      id: tm.id,
      pos: tm.posicao,
      name: tm.nome,
      pts: tm.pontos,
      color: tm.cor_primaria || "#888",
      me: playerTeam?.id != null && tm.id === playerTeam.id,
    }));
  }, [standings, playerTeam]);

  // Pilotos: grid completo da categoria, por posição no campeonato.
  const pilotos = useMemo(() => {
    return [...driverStandings]
      .sort((a, b) => (a.posicao_campeonato ?? 999) - (b.posicao_campeonato ?? 999))
      .map((d) => ({
        id: d.id,
        pos: d.posicao_campeonato,
        name: d.nome,
        pts: d.pontos,
        teamName: d.equipe_nome,
        color: d.equipe_cor || "#888",
        me: d.is_jogador,
      }));
  }, [driverStandings]);

  // Nomes que o boletim pode mencionar + resolução do piloto realçado → equipe dele.
  const mentionDrivers = useMemo(
    () => driverStandings.map((d) => ({ id: d.id, nome: d.nome })),
    [driverStandings],
  );
  const hoveredTeamId = useMemo(
    () => driverStandings.find((d) => d.id === hoveredDriverId)?.equipe_id ?? null,
    [driverStandings, hoveredDriverId],
  );

  return (
    <>
      <div className="spread">
        {/* PÁGINA ESQUERDA */}
        <div className="page page-l">
          <div className="flag" />
          <div className="kicker">{kicker}</div>
          <h1 className="display">
            <span className="l1">{ed.track_name}</span>
            <span className="l2">
              {t("newsMagazine.page.stageSeasonLine", { round: ed.rodada, year })}
            </span>
          </h1>
          <span className="ai-tag">
            {bulletin?.loading
              ? t("newsMagazine.aiTag.generating")
              : bulletin?.story
              ? t("newsMagazine.aiTag.bulletin")
              : t("newsMagazine.aiTag.comingSoon")}
          </span>

          <div className="prose-cols">
            <h3 className="subhead">{t("newsMagazine.page.bulletinHead")}</h3>
            {bulletin?.story ? (
              renderBulletinBody(
                bulletin.story,
                mentionDrivers,
                bulletin.teams,
                hoveredDriverId,
                setHoveredDriverId,
              )
            ) : bulletin?.loading ? (
              <p>{t("newsMagazine.page.generatingBulletin")}</p>
            ) : !isPortuguese(language) ? (
              <p style={{ fontStyle: "italic", opacity: 0.6 }}>{localizedAiError(language)}</p>
            ) : (
              <>
                <p>
                  O relato completo desta etapa será gerado pela IA a partir do que aconteceu na pista — sua
                  largada, ultrapassagens, disputa pela ponta e o resultado final.
                </p>
                <p>
                  Por enquanto, acompanhe ao lado a{" "}
                  {/* i18n-ignore — placeholder PT-only do gate isPortuguese: só renderiza quando o idioma É português */}
                  <span className="teamname">classificação de construtores</span> atualizada e, abaixo, as
                  mensagens diretas a você na caixa de entrada.
                </p>
              </>
            )}
          </div>
        </div>

        {/* PÁGINA DIREITA */}
        <div className="page page-r">
          <MagazineCredits catLabel={catLabel} />
          <img className="photo" src={getTrackImageSrc(ed.track_name, ed.track_id)} alt={ed.track_name} />
          <p className="cap">{`${ed.track_name}${ed.display_date ? ` — ${ed.display_date}` : ""}`}</p>

          <div className="r-grid r-grid-single">
            <div>
              <div className="nm-standings-head">
                <h3 className="subhead">
                  {standingsView === "construtores"
                    ? t("newsMagazine.standings.constructors")
                    : t("newsMagazine.standings.drivers")}{" "}
                  · {year}
                </h3>
                <div className="nm-toggle">
                  <button
                    type="button"
                    className={`nm-toggle-btn${standingsView === "pilotos" ? " active" : ""}`}
                    onClick={() => setStandingsView("pilotos")}
                  >
                    {t("newsMagazine.standings.drivers")}
                  </button>
                  <button
                    type="button"
                    className={`nm-toggle-btn${standingsView === "construtores" ? " active" : ""}`}
                    onClick={() => setStandingsView("construtores")}
                  >
                    {t("newsMagazine.standings.constructors")}
                  </button>
                </div>
              </div>

              {standingsView === "construtores" ? (
                construtores.length > 0 ? (
                  <StandingsList
                    rows={construtores}
                    hoveredId={hoveredTeamId}
                    logoTestId="news-team-logo"
                  />
                ) : (
                  <p>{t("newsMagazine.standings.teamsUnavailable")}</p>
                )
              ) : pilotos.length > 0 ? (
                <StandingsList
                  rows={pilotos}
                  hoveredId={hoveredDriverId}
                  onHover={setHoveredDriverId}
                  logoTestId="news-driver-team-logo"
                />
              ) : (
                <p>{t("newsMagazine.standings.driversUnavailable")}</p>
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
          <div className="mag-nav">
            <button
              type="button"
              className="navbtn"
              onClick={() => onGoEdition(1)}
              disabled={!canGoPrev}
              title={t("newsMagazine.foot.prevEdition")}
              aria-label={t("newsMagazine.foot.prevEdition")}
            >
              ‹
            </button>
            <button
              type="button"
              className="navbtn"
              onClick={() => onGoEdition(-1)}
              disabled={!canGoNext}
              title={t("newsMagazine.foot.nextEdition")}
              aria-label={t("newsMagazine.foot.nextEdition")}
            >
              ›
            </button>
          </div>
        </div>
        <div className="foot-meta">{footMeta}</div>
      </div>
    </>
  );
}

export default RaceEditionSpread;
