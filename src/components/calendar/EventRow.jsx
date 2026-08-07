import { getCategoryColor } from "../../utils/categoryColors";
import { extractNationalityLabel } from "../../utils/formatters";
import { getTrackImageSrc, parseDisplayDate } from "../../utils/calendarShared.js";
import { trackCountryLabel } from "../../utils/trackCountry.js";
import FlagIcon from "../ui/FlagIcon";

function EventRow({ race, monthShort, isNext, onSelect, t }) {
  const parsed = parseDisplayDate(race.display_date);
  const image = getTrackImageSrc(race);
  const color = getCategoryColor(race.categoria);
  const country = trackCountryLabel(race.track_name);
  const isSpecial = Boolean(race._isSpecialRace) || race.season_phase === "BlocoEspecial";

  return (
    <button
      type="button"
      onClick={() => onSelect?.(race)}
      title={race.track_name}
      className="flex w-full items-center gap-3 rounded-2xl bg-white/[0.05] px-3 py-2.5 text-left transition-glass hover:bg-white/[0.08] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-primary/70"
    >
      <div className="w-[44px] shrink-0 rounded-xl bg-black/20 py-2 text-center">
        <div className="kcal text-[18px] font-bold italic leading-none tabular-nums text-text-primary">
          {parsed ? String(parsed.day).padStart(2, "0") : "--"}
        </div>
        <div className="mt-1 text-[8px] font-bold uppercase tracking-[0.12em] text-text-muted">
          {parsed ? monthShort[parsed.month] : ""}
        </div>
      </div>
      <div className="h-10 w-[52px] shrink-0 overflow-hidden rounded-lg bg-black/40 ring-1 ring-inset ring-white/10">
        {image ? (
          <img src={image} alt={race.track_name} className="h-full w-full object-cover" draggable={false} />
        ) : (
          <div className="grid h-full w-full place-items-center" style={{ backgroundColor: `${color}26` }}>
            <span className="h-2 w-2 rounded-full" style={{ backgroundColor: color }} />
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-semibold text-text-primary">{race.track_name}</div>
        {/* O rótulo cru ("🇳🇴 Noruega") vira arte no FlagIcon porque o Windows não tem
            fonte de emoji de bandeira — o Chromium desenhava os indicadores regionais
            como as letras "no".
            Variante `boxed` (padrão), não `natural`: bandeira nacional tem proporção
            oficial e elas não batem entre si (de 1:1 a 2:1), então no modo natural a
            altura comum produzia larguras diferentes por linha. A caixa fixa de 16x24
            com `object-contain` iguala o tamanho sem recortar o desenho.
            O texto vai em `text-secondary`, não `muted`: o muted é o token de rótulo
            apagado (o "FEV" do bloco de data ao lado) e a 11px deixava o país quase
            ilegível. O nome da pista segue em `primary`, então a hierarquia fica. */}
        {country && (
          <div className="mt-0.5 flex items-center gap-1.5 text-[11px] font-medium text-text-secondary">
            <FlagIcon nacionalidade={country} className="shrink-0" />
            <span className="truncate">{extractNationalityLabel(country)}</span>
          </div>
        )}
      </div>
      <div className="shrink-0 text-right">
        {race.horario && (
          <div className="text-[12px] font-semibold tabular-nums text-text-secondary">{race.horario}</div>
        )}
        {/* Só rótulo que informa: "Corrida" em toda linha de um calendário de corridas
            não distingue nada — e em vermelho ainda sugeria alerta. */}
        {isNext ? (
          <div className="mt-0.5 text-[8px] font-extrabold uppercase tracking-[0.08em] text-accent-hover">
            {t("calendar.v2.next")}
          </div>
        ) : isSpecial ? (
          <div className="mt-0.5 text-[8px] font-extrabold uppercase tracking-[0.08em] text-status-purple">
            {t("calendar.v2.special")}
          </div>
        ) : null}
      </div>
    </button>
  );
}

export default EventRow;
