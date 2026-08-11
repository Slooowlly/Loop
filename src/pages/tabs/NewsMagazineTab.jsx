import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import MagazineCover from "../../components/news/MagazineCover";
import MagazineMailbox from "../../components/news/MagazineMailbox";
import PreseasonSpread from "../../components/news/PreseasonSpread";
import RaceEditionSpread from "../../components/news/RaceEditionSpread";
import {
  useCategoryCalendar,
  useDriverStandings,
  useRaceBulletin,
  useSeasonPreview,
  useTeamsStandings,
  useWorldNotes,
} from "../../components/news/useMagazineData";
import useTempoDeTela from "../../hooks/useTempoDeTela";
import useCareerStore from "../../stores/useCareerStore";
import { categoryLabel } from "../../utils/formatters";

import "./NewsMagazineTab.css";

// ─────────────────────────────────────────────────────────────────────────────
// Construtores, edições e o boletim da matéria são REAIS (vindos do backend). O
// boletim vem por IA (`enrich_race_news_ai`); o servidor escolhe o provedor — ver
// `narrative/client.rs`. Sem boletim gerado, o texto cai no placeholder.
// ─────────────────────────────────────────────────────────────────────────────

function NewsMagazineTab() {
  const { t } = useTranslation();
  // Telemetria de produto: a revista é a tela mais cara do Loop (matéria de corrida,
  // prévia de temporada, rodapé do mundo, tudo gerado por IA). Quanto tempo ela fica
  // aberta é o que diz se essa geração está sendo lida.
  useTempoDeTela("noticias");
  const careerId = useCareerStore((s) => s.careerId);
  const playerTeam = useCareerStore((s) => s.playerTeam);
  const season = useCareerStore((s) => s.season);
  const language = useCareerStore((s) => s.language);

  const category = playerTeam?.categoria ?? null;
  const year = season?.ano ?? "";

  const standings = useTeamsStandings(careerId, category);
  const driverStandings = useDriverStandings(careerId, category);
  const { calendar, loaded: calendarLoaded } = useCategoryCalendar(careerId, category);
  const worldNotes = useWorldNotes(careerId);

  const [edIdx, setEdIdx] = useState(0);
  const [flipping, setFlipping] = useState(false);

  // Edições = corridas concluídas, da mais recente para a mais antiga.
  const editions = useMemo(() => {
    return calendar
      .filter((r) => r.status === "Concluida")
      .sort((a, b) => (b.rodada ?? 0) - (a.rodada ?? 0));
  }, [calendar]);

  const totalRounds = calendar.length;
  const safeIdx = Math.min(edIdx, Math.max(0, editions.length - 1));
  const ed = editions[safeIdx] ?? null;

  // Pré-temporada: só há matéria de expectativas enquanto NENHUMA corrida foi disputada.
  // Assim que a 1ª etapa conclui, `editions` deixa de ser vazio e este bloco some.
  const isPreseason = editions.length === 0;

  const bulletin = useRaceBulletin(careerId, season?.id, ed?.rodada);
  // `isPreseason` só vale depois do calendário voltar — antes disso ele é sempre
  // `true` e pediria a prévia da temporada em toda abertura da aba, mesmo no meio
  // de uma temporada já em curso.
  const preview = useSeasonPreview(careerId, category, season?.id, isPreseason && calendarLoaded);

  // Pista de abertura (menor rodada do calendário) — foto e legenda do spread.
  const openingRace = useMemo(() => {
    if (!calendar.length) return null;
    return [...calendar].sort((a, b) => (a.rodada ?? 0) - (b.rodada ?? 0))[0] ?? null;
  }, [calendar]);

  function goEdition(delta) {
    const next = safeIdx + delta;
    if (next < 0 || next >= editions.length || flipping) return;
    setFlipping(true);
    setTimeout(() => {
      setEdIdx(next);
      setFlipping(false);
    }, 200);
  }

  const catLabel = category ? categoryLabel(category) : "";
  const kicker = ed ? `${catLabel} · ${ed.track_name}` : catLabel;

  const footMeta = ed
    ? `${t("newsMagazine.foot.edition", { round: ed.rodada })}${totalRounds ? t("newsMagazine.foot.ofTotal", { total: totalRounds }) : ""}${year ? t("newsMagazine.foot.seasonSuffix", { year }) : ""}`
    : "";

  // Antes da 1ª corrida a revista abre com "O Que Esperar" (spread aberto, não o
  // "livro fechado"). Só recai no livro fechado se nem categoria houver (edge).
  const showPreseason = isPreseason && !!category;

  // Enquanto o calendário não volta, `editions` é vazio e a revista abriria no
  // spread de pré-temporada — curto — para só depois trocar pela edição da
  // corrida. É o "pisca" de revista minimizada ao entrar na aba. Segura a
  // escolha até o dado chegar; a folha em branco reserva a altura.
  const abrindo = !calendarLoaded;

  const magClass = [
    "mag",
    flipping ? "flipping" : "",
    abrindo ? "mag--abrindo" : ed || showPreseason ? "" : "mag--cover",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className="newsmag">
      {/* ═══════════ A REVISTA ═══════════ */}
      <article className={magClass}>
        {abrindo ? null : ed ? (
          <RaceEditionSpread
            ed={ed}
            year={year}
            catLabel={catLabel}
            kicker={kicker}
            footMeta={footMeta}
            bulletin={bulletin}
            language={language}
            standings={standings}
            driverStandings={driverStandings}
            playerTeam={playerTeam}
            worldNotes={worldNotes}
            onGoEdition={goEdition}
            canGoPrev={safeIdx < editions.length - 1}
            canGoNext={safeIdx > 0}
          />
        ) : showPreseason ? (
          <PreseasonSpread
            catLabel={catLabel}
            year={year}
            preview={preview}
            openingRace={openingRace}
            standings={standings}
            driverStandings={driverStandings}
            playerTeam={playerTeam}
            worldNotes={worldNotes}
          />
        ) : (
          <MagazineCover catLabel={catLabel} year={year} />
        )}
      </article>

      {/* ═══════════ CAIXA DE E-MAIL ═══════════ */}
      <MagazineMailbox careerId={careerId} />
    </div>
  );
}

export default NewsMagazineTab;
