import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel, extractNationalityLabel } from "../../utils/formatters";
import { getTrackImageSrc, parseDisplayDate } from "../../utils/calendarShared.js";
import { trackCountryLabel } from "../../utils/trackCountry.js";
import FlagIcon from "../ui/FlagIcon";

// O CARTÃO DA PRÓXIMA ETAPA.
//
// A corrida que decide a temporada morava numa linha de 62 px, idêntica às outras
// quatro da lista e sem nenhuma ação — o jogador lia a data e não tinha o que fazer
// com ela. Aqui ela ocupa o topo do painel, com a arte da pista ao fundo, a contagem
// regressiva colada no evento (e não solta no bloco de estatísticas ao lado) e o
// botão que leva à corrida.
//
// O botão é o MESMO avanço do cabeçalho (`startCalendarAdvance`), sem trocar de aba:
// quem clica já está no calendário e é justamente onde a animação dos dias acontece.
// Ele só aparece quando o cartão é a próxima corrida DO JOGADOR — uma etapa de bloco
// especial de outra categoria continua sendo só informação.
function NextRaceHero({
  race,
  isNext,
  daysUntilNext,
  totalRounds,
  onSelect,
  onAdvance,
  advancing = false,
  t,
}) {
  const parsed = parseDisplayDate(race.display_date);
  const image = getTrackImageSrc(race);
  const color = getCategoryColor(race.categoria, "#E73F47");
  const country = trackCountryLabel(race.track_name);
  const isSpecial = Boolean(race._isSpecialRace) || race.season_phase === "BlocoEspecial";
  const duration = Number(race.duracao_corrida_min) > 0 ? `${race.duracao_corrida_min} min` : t("calendar.tbd");

  // A contagem só vale para a próxima corrida do jogador: `days_until_next_event` conta
  // até O PRÓXIMO EVENTO dele, então pendurá-la numa etapa de outra categoria mentiria.
  let countdown = null;
  if (isNext && daysUntilNext != null) {
    countdown = daysUntilNext <= 0
      ? t("calendar.v2.today")
      : t("calendar.v2.heroInDays", { count: daysUntilNext });
  }

  return (
    <div>
      <button
        type="button"
        onClick={() => onSelect?.(race)}
        title={race.track_name}
        className="relative block h-[190px] w-full overflow-hidden text-left transition-glass focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-primary/70"
      >
        {image ? (
          <img
            src={image}
            alt={race.track_name}
            className="absolute inset-0 h-full w-full object-cover"
            draggable={false}
          />
        ) : (
          <div className="absolute inset-0" style={{ backgroundColor: `${color}26` }} />
        )}
        {/* O véu é o que torna o texto legível sobre qualquer foto de pista — sem ele o
            nome branco some nas artes claras (Lime Rock ao sol) e nada acusa. */}
        <div className="absolute inset-0 bg-gradient-to-b from-[rgba(4,8,18,0.30)] via-[rgba(4,8,18,0.72)] to-[rgba(4,8,18,0.95)]" />

        <div className="absolute inset-x-4 top-3.5 flex items-center justify-between gap-2">
          <span
            className="kcal shrink-0 rounded-full border px-2.5 py-1 text-[9px] font-bold uppercase tracking-[0.14em] text-white"
            style={{ borderColor: `${color}75`, backgroundColor: `${color}2e` }}
          >
            {totalRounds > 0 && race.rodada != null
              ? t("calendar.v2.heroRound", { n: race.rodada, total: totalRounds })
              : t("calendar.v2.raceNumber", { n: race.rodada })}
          </span>
          {countdown && (
            <span className="shrink-0 rounded-full border border-accent-primary/40 bg-accent-primary/15 px-2.5 py-1 text-[9px] font-extrabold uppercase tracking-[0.14em] text-accent-hover">
              {countdown}
            </span>
          )}
          {!countdown && isSpecial && (
            <span className="shrink-0 rounded-full border border-status-purple/40 bg-status-purple/15 px-2.5 py-1 text-[9px] font-extrabold uppercase tracking-[0.14em] text-status-purple">
              {t("calendar.v2.special")}
            </span>
          )}
        </div>

        <div className="absolute inset-x-4 bottom-4">
          {/* `h4`, e não `h3`: o `h3` da tela é o mês em foco, e um segundo `h3` no
              painel disputaria o mesmo papel de título da aba. */}
          <h4 className="kcal text-[25px] font-bold uppercase italic leading-none tracking-tight text-white drop-shadow-[0_2px_14px_rgba(0,0,0,0.7)]">
            {race.track_name}
          </h4>
          {country && (
            <div className="mt-2 flex items-center gap-2 text-[12px] font-medium text-text-secondary">
              <FlagIcon nacionalidade={country} className="shrink-0" />
              <span className="truncate">{extractNationalityLabel(country)}</span>
            </div>
          )}
          <div className="mt-3 flex gap-5 border-t border-white/[0.12] pt-3">
            <HeroMeta label={t("calendar.v2.heroStart")} value={race.horario || t("calendar.tbd")} />
            <HeroMeta label={t("calendar.detail.duration")} value={duration} />
            <HeroMeta label={t("calendar.v2.heroCategory")} value={categoryLabel(race.categoria)} truncate />
          </div>
        </div>
      </button>

      {isNext && onAdvance && (
        <button
          type="button"
          onClick={onAdvance}
          disabled={advancing}
          className="mx-3.5 mb-3.5 mt-3.5 block w-[calc(100%-28px)] rounded-2xl bg-gradient-to-b from-accent-hover to-[#3d8ce0] py-3 text-[12px] font-extrabold uppercase tracking-[0.1em] text-[#05131f] shadow-[0_8px_22px_rgba(88,166,255,0.30)] transition-glass hover:brightness-110 disabled:opacity-50"
        >
          {advancing ? t("nav.advance.advancing") : t("calendar.v2.heroGoToRace")}
        </button>
      )}
    </div>
  );
}

function HeroMeta({ label, value, truncate = false }) {
  return (
    <div className="min-w-0">
      <div className={`kcal text-[15px] font-bold leading-none text-text-primary ${truncate ? "truncate" : ""}`}>
        {value}
      </div>
      <div className="mt-1.5 text-[8.5px] font-bold uppercase tracking-[0.14em] text-text-muted">
        {label}
      </div>
    </div>
  );
}

export default NextRaceHero;
