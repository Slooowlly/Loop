import { useTranslation } from "react-i18next";
import { formatTenureCounter } from "../preSeasonFormatters.js";

export default function TeamDriverRow({ driverName, tenureSeasons, isPrimarySlot = false, accent = "#58a6ff" }) {
  const { t } = useTranslation();
  const isOpenSlot = !driverName;

  // Vaga aberta: chip tracejado na cor da categoria (lê como oportunidade, não como
  // "erro"/vazio como o antigo "Sem piloto" vermelho).
  if (isOpenSlot) {
    return (
      <div className="flex items-center py-2">
        <span
          className="flex w-full items-center gap-2 rounded-lg border border-dashed px-3 py-1.5 text-body font-semibold"
          style={{ borderColor: `${accent}66`, color: accent, background: `${accent}12` }}
        >
          <span className="text-[14px] font-bold leading-none opacity-80">+</span>
          {t("preSeason.roster.openSlot")}
        </span>
      </div>
    );
  }

  const tenureCounter = formatTenureCounter(tenureSeasons);
  // Pips de tempo de casa: 1 pip por temporada (teto de 5); o rótulo numérico
  // mantém a precisão exata. Estreante (1ª temp.) segue com o badge dedicado.
  const pipCount = Math.min(Math.max(tenureSeasons ?? 0, 0), 5);
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <div className="flex min-w-0 flex-1 items-center">
        <p className={`truncate leading-[1.1] ${isPrimarySlot ? "text-[15px] font-bold text-[color:var(--text-primary)]" : "text-[14px] font-semibold text-[color:var(--text-primary)]"}`}>
            {driverName}
        </p>
      </div>
      {tenureCounter && (
        tenureCounter.isNewcomer ? (
          <span className="shrink-0 rounded-md border border-[#58a6ff55] bg-[#58a6ff1f] px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.08em] text-[#79b8ff]">
            {tenureCounter.label}
          </span>
        ) : (
          <span className="flex shrink-0 items-center gap-2">
            <span className="flex items-center gap-[3px]" aria-hidden="true">
              {Array.from({ length: pipCount }).map((_, i) => (
                <span key={i} className="h-1.5 w-1.5 rounded-full" style={{ background: accent }} />
              ))}
            </span>
            <span className="text-[11px] font-semibold tabular-nums text-[color:var(--text-muted)]">
              {tenureCounter.label}
            </span>
          </span>
        )
      )}
    </div>
  );
}
