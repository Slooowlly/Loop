import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import OffersSummaryCard from "../OffersSummaryCard";
import WeeklyClosingMovement from "../WeeklyClosingMovement";
import TeamLogoMark from "../../../team/TeamLogoMark";
import { subcatLabel, subcatColor } from "../../preSeasonFormatters.js";

// Coluna direita do v2 — "Sua Janela".
//
// O v1 chamava esta coluna de "Expectativa" e, nas semanas de abertura, ela era
// um "0" gigante e uma caixa dizendo que as movimentações apareceriam depois.
// Aqui ela responde três perguntas o tempo todo: quem você é nesta janela, o que
// se espera de você, e onde a janela está. As ofertas continuam sendo o centro
// quando existem — o bloco delas é o mesmo do v1, com as mesmas chaves.

function PlayerStatusCard({ player, playerTeam, playerCategory, playerSignedThisWindow }) {
  const { t } = useTranslation();
  const categoryLabel = playerCategory ? subcatLabel(playerCategory) : null;
  const categoryColor = playerCategory ? subcatColor(playerCategory) : "var(--accent-primary)";
  const initials = (player?.nome ?? "")
    .split(" ")
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0])
    .join("")
    .toUpperCase();

  // A cor da equipe manda no cartão. O azul fixo era a cor do app, não a de quem
  // ele defende — e este é o único cartão da tela que fala do jogador. Sufixo
  // hexadecimal de alfa, como o resto da pré-temporada faz com `group.color`.
  const accent = playerTeam?.cor_primaria ?? "#58a6ff";

  return (
    <div
      className="mb-2.5 rounded-xl border p-3"
      style={{
        borderColor: `${accent}3d`,
        background: `linear-gradient(140deg, ${accent}1a, rgba(10,15,28,0.2))`,
      }}
    >
      <div className="flex items-center gap-2.5">
        {/* A marca da equipe, não as iniciais do piloto: o nome dele já está ao
            lado em negrito, e o "T2" só repetia essas mesmas letras. A logo é a
            única coisa aqui que responde "por quem eu corro agora". Sem equipe
            (piloto livre), as iniciais voltam como recurso. */}
        {playerTeam?.nome ? (
          <TeamLogoMark
            teamName={playerTeam.nome}
            color={playerTeam.cor_primaria}
            size="sm"
            halo
            testId="preseason-player-team-logo"
          />
        ) : (
          <span
            className="grid h-9 w-9 shrink-0 place-items-center rounded-lg text-[11px] font-black"
            style={{ background: "linear-gradient(135deg,#2b7de0,#1b4b8a)", color: "#dff0ff" }}
          >
            {initials || "—"}
          </span>
        )}
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13.5px] font-bold leading-[1.1]">
            {player?.nome ?? t("preSeason.v2.window.unknownDriver")}
          </p>
          <p className="mt-0.5 truncate text-[9.5px] text-[color:var(--text-muted)]">
            {categoryLabel && <span style={{ color: categoryColor }}>{categoryLabel}</span>}
            {player?.posicao_campeonato > 0 && (
              <span>
                {" · "}
                {t("preSeason.v2.window.lastPosition", { position: player.posicao_campeonato })}
              </span>
            )}
          </p>
        </div>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        {playerSignedThisWindow && playerTeam?.nome ? (
          <span className="flex items-center gap-1.5 rounded-md bg-[#3fb95022] px-1.5 py-0.5 text-[9px] font-black uppercase tracking-[0.08em] text-[color:var(--status-green)]">
            {t("preSeason.v2.window.underContract")}
          </span>
        ) : (
          <span className="rounded-md bg-[#f8514922] px-1.5 py-0.5 text-[9px] font-black uppercase tracking-[0.08em] text-[color:var(--status-red)]">
            {t("preSeason.v2.window.noContract")}
          </span>
        )}
        {playerTeam?.nome && (
          <span
            className="truncate rounded-md px-1.5 py-0.5 text-[9px] font-bold"
            style={{ background: `${accent}1f`, color: accent }}
          >
            {playerSignedThisWindow
              ? playerTeam.nome
              : t("preSeason.v2.window.formerTeam", { team: playerTeam.nome })}
          </span>
        )}
      </div>
    </div>
  );
}

function ForecastCard({ forecast, totalOffers, isOpeningWeek, playerSignedThisWindow }) {
  const { t } = useTranslation();

  if (playerSignedThisWindow && !forecast) {
    return (
      <div className="mb-2.5 rounded-xl border border-dashed border-white/[0.12] bg-black/15 p-4 text-center text-body text-[color:var(--text-secondary)]">
        {t("preSeason.forecast.notInMarket")}
      </div>
    );
  }

  // Nas semanas de abertura a expectativa é uma FAIXA (o backend só sabe estimar);
  // depois dela o número passa a ser o de ofertas reais na mesa.
  const headline = isOpeningWeek
    ? forecast
      ? forecast.min === forecast.max
        ? `${forecast.max}`
        : `${forecast.min}–${forecast.max}`
      : `${totalOffers}`
    : `${totalOffers}`;

  return (
    <div className="mb-2.5 rounded-xl border border-white/[0.08] bg-black/15 p-3">
      <div className="mb-2 flex items-center gap-1.5">
        <span className="h-1.5 w-1.5 rounded-full bg-[color:var(--accent-primary)]" />
        <span className="text-[9px] font-black uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
          {isOpeningWeek ? t("preSeason.v2.window.forecastTitle") : t("preSeason.offers.title")}
        </span>
      </div>
      <div className="flex items-baseline gap-2.5">
        <span className="text-[30px] font-black leading-none tabular-nums">{headline}</span>
        <span className="text-[10.5px] leading-[1.3] text-[color:var(--text-secondary)]">
          {isOpeningWeek
            ? t("preSeason.v2.window.forecastCaption")
            : t("preSeason.forecast.seatCount", { count: totalOffers })}
        </span>
      </div>
      {/* Nas semanas de assinatura a expectativa continua viva: são as equipes que
          colocariam o jogador na shortlist AGORA, mas ainda têm gente na frente.
          Sem esta linha, "zero proposta" e "zero interesse" seriam indistinguíveis. */}
      {!isOpeningWeek && forecast && (
        <p className="mt-2 border-t border-white/[0.06] pt-2 text-[10.5px] leading-[1.3] text-[color:var(--text-secondary)]">
          {t("preSeason.forecast.exact", { count: forecast.max })}
        </p>
      )}
    </div>
  );
}

// Diário: onde a janela está e o que vem a seguir. Nas semanas de fotografia é
// o único conteúdo com substância desta coluna — e é ele que explica por que o
// jogador não pode assinar ainda.
//
// Some depois da primeira visita: a janela funciona sempre do mesmo jeito, então
// o diário ensina UM processo, não conta o estado de uma temporada. Lido uma vez,
// vira moldura ocupando meia coluna. Fica a um clique de voltar, porque tirar
// informação para sempre sem porta de retorno é outro tipo de erro.
export const DIARY_SEEN_KEY = "loop.preseason.diarySeen";

function useWindowDiary() {
  const [visible, setVisible] = useState(() => {
    try {
      return window.localStorage.getItem(DIARY_SEEN_KEY) !== "1";
    } catch {
      return true;
    }
  });

  useEffect(() => {
    try {
      window.localStorage.setItem(DIARY_SEEN_KEY, "1");
    } catch {
      // Janela isolada / modo privado: o diário volta a aparecer na próxima vez.
    }
  }, []);

  return { visible, toggle: () => setVisible((atual) => !atual) };
}

function WindowDiary({ currentWeek, totalWeeks, signingsStartWeek, isComplete, onHide }) {
  const { t } = useTranslation();
  const steps = [
    { week: 1, key: "snapshot", gate: signingsStartWeek > 1 },
    ...(signingsStartWeek > 2 ? [{ week: 2, key: "departures", gate: true }] : []),
    { week: signingsStartWeek, key: "marketOpen", gate: false },
    ...(totalWeeks > signingsStartWeek + 1
      ? [{ week: signingsStartWeek + 1, key: "aiFills", gate: false, span: true }]
      : []),
    { week: totalWeeks, key: "closing", gate: false },
  ];

  return (
    <div className="rounded-xl border border-white/[0.08] bg-black/15 p-3">
      <div className="mb-2.5 flex items-center gap-1.5">
        <span className="h-1.5 w-1.5 rounded-full bg-[color:var(--text-muted)]" />
        <span className="flex-1 text-[9px] font-black uppercase tracking-[0.2em] text-[color:var(--text-muted)]">
          {t("preSeason.v2.window.diaryTitle")}
        </span>
        <button
          type="button"
          onClick={onHide}
          data-testid="preseason-diary-hide"
          className="transition-glass -my-1 -mr-1 rounded px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-[color:var(--text-muted)] hover:bg-white/[0.08] hover:text-[color:var(--text-secondary)]"
        >
          {t("preSeason.v2.window.diaryHide")}
        </button>
      </div>
      <div className="relative pl-4">
        <span className="absolute bottom-1 left-1 top-1 w-px bg-white/10" />
        {steps.map((step, index) => {
          const isNow = !isComplete
            && (step.span
              ? currentWeek >= step.week && currentWeek < totalWeeks
              : currentWeek === step.week);
          const isPast = currentWeek > step.week && !isNow;
          return (
            <div
              key={`${step.key}-${index}`}
              className={`relative ${index === steps.length - 1 ? "" : "pb-2.5"} ${
                isNow || isPast ? "" : "opacity-45"
              }`}
            >
              <span
                className="absolute -left-[15px] top-[3px] h-2.5 w-2.5 rounded-full border-2"
                style={{
                  background: isNow ? "var(--accent-primary)" : "#0b1420",
                  borderColor: isNow
                    ? "var(--accent-primary)"
                    : step.gate
                      ? "rgba(210,153,34,0.7)"
                      : "rgba(255,255,255,0.18)",
                  boxShadow: isNow ? "0 0 10px rgba(88,166,255,0.9)" : undefined,
                }}
              />
              <p
                className="text-[9px] font-black uppercase tracking-[0.12em]"
                style={{ color: isNow ? "var(--accent-primary)" : "var(--text-muted)" }}
              >
                {step.span
                  ? t("preSeason.v2.window.diaryWeekRange", { from: step.week, to: totalWeeks - 1 })
                  : t("preSeason.v2.window.diaryWeek", { week: step.week })}
                {isNow && ` · ${t("preSeason.v2.window.diaryNow")}`}
              </p>
              <p className="mt-0.5 text-[10.5px] leading-[1.35] text-[color:var(--text-secondary)]">
                {t(`preSeason.v2.window.diary.${step.key}`)}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default function PlayerWindowPanel({
  player,
  playerTeam,
  playerCategory,
  playerProposals,
  playerOffers,
  playerSignedThisWindow,
  isComplete,
  isAdvancingWeek,
  isOpeningWeek,
  interestForecast,
  totalOffers,
  offersByCategory,
  weeklyClosingGroups,
  currentWeek,
  totalWeeks,
  signingsStartWeek,
  openOffersFor,
  setTransferDetail,
}) {
  const { t } = useTranslation();
  const diary = useWindowDiary();

  // Nas semanas de abertura o mercado ainda não contrata, então mesmo com oferta
  // na lista o lugar é da expectativa.
  const temDecisaoNaMesa =
    !isOpeningWeek && (playerOffers.length > 0 || playerProposals.length > 0);

  return (
    <aside className="glass animate-drawer-in flex min-h-0 flex-col rounded-2xl">
      <div className="flex items-center justify-between border-b border-white/[0.07] px-3.5 py-3 lg:px-4">
        <p className="text-[9px] font-black uppercase tracking-[0.22em] text-[color:var(--text-secondary)]">
          {t("preSeason.v2.window.title")}
        </p>
        <span
          className={`rounded-full border px-2 py-0.5 text-[9px] font-black uppercase tracking-[0.14em] ${
            playerSignedThisWindow
              ? "border-[#3fb95055] bg-[#3fb9501a] text-[color:var(--status-green)]"
              : isOpeningWeek
                ? "border-[#d2992255] bg-[#d2992218] text-[color:var(--status-yellow)]"
                : "border-[#58a6ff55] bg-[#58a6ff1a] text-[color:var(--accent-primary)]"
          }`}
        >
          {playerSignedThisWindow
            ? t("preSeason.v2.window.statusSigned")
            : isOpeningWeek
              ? t("preSeason.v2.window.statusWaiting")
              : t("preSeason.v2.window.statusOpen")}
        </span>
      </div>

      <div className="scroll-area min-h-0 flex-1 overflow-y-auto px-3 py-3 lg:px-3.5">

        <PlayerStatusCard
          player={player}
          playerTeam={playerTeam}
          playerCategory={playerCategory}
          playerSignedThisWindow={playerSignedThisWindow}
        />

        {/* Ofertas e propostas: UM bloco só, e ele não decide nada. O número grande
            é a âncora, os trilhos coloridos dizem de onde vêm, e o clique leva ao
            modal — a única tela onde aceitar, recusar e assinar existem. Sem nada
            na mesa (ou nas semanas de abertura) o lugar volta a ser da expectativa. */}
        {temDecisaoNaMesa ? (
          <OffersSummaryCard
            groups={offersByCategory}
            totalOffers={totalOffers}
            proposals={playerProposals}
            onSelect={openOffersFor}
          />
        ) : (
          <ForecastCard
            forecast={interestForecast}
            totalOffers={totalOffers}
            isOpeningWeek={isOpeningWeek}
            playerSignedThisWindow={playerSignedThisWindow}
          />
        )}

        {!isOpeningWeek && !temDecisaoNaMesa && (
          <div className="mb-2.5 rounded-xl border border-dashed border-white/[0.12] bg-black/15 p-4 text-center text-body text-[color:var(--text-secondary)]">
            {playerSignedThisWindow
              ? t("preSeason.offers.emptySigned")
              : isComplete
                ? t("preSeason.offers.emptyClosed")
                : t("preSeason.offers.emptyNone")}
          </div>
        )}

        {/* Movimentações da semana que acabou de fechar (bloco do v1, sem mudança). */}
        {weeklyClosingGroups.length > 0 && (
          <div
            data-testid="weekly-closing-market"
            className="mb-2.5 rounded-xl border border-white/[0.08] bg-black/[0.18] px-3 py-3"
          >
            <p className="text-[9px] font-black uppercase tracking-[0.18em] text-[color:var(--text-muted)]">
              {t("preSeason.weeklyClosing.title")}
            </p>
            <div className="mt-2.5 space-y-3">
              {weeklyClosingGroups.map((group) => (
                <section key={group.category} className="space-y-2">
                  <div
                    className="flex items-center justify-center rounded-lg border px-3 py-1.5"
                    style={{
                      borderColor: `${group.color}30`,
                      background: `linear-gradient(135deg, ${group.color}16 0%, rgba(255,255,255,0.025) 100%)`,
                    }}
                  >
                    <p
                      className="text-center text-[11px] font-black uppercase tracking-[0.16em]"
                      style={{ color: group.color }}
                    >
                      {group.label}
                    </p>
                  </div>
                  <div className="space-y-2">
                    {group.events.map((event, index) => (
                      <WeeklyClosingMovement
                        key={`${event.event_type}-${event.driver_id ?? event.driver_name}-${index}`}
                        event={event}
                        color={group.color}
                        onSelect={setTransferDetail}
                      />
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </div>
        )}

        {diary.visible ? (
          <WindowDiary
            currentWeek={currentWeek}
            totalWeeks={totalWeeks}
            signingsStartWeek={signingsStartWeek}
            isComplete={isComplete}
            onHide={diary.toggle}
          />
        ) : (
          <button
            type="button"
            onClick={diary.toggle}
            data-testid="preseason-diary-show"
            className="transition-glass w-full rounded-lg border border-dashed border-white/10 px-3 py-1.5 text-[9.5px] font-bold uppercase tracking-[0.14em] text-[color:var(--text-muted)] hover:border-white/20 hover:text-[color:var(--text-secondary)]"
          >
            {t("preSeason.v2.window.diaryShow")}
          </button>
        )}
      </div>
    </aside>
  );
}
