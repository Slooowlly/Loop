import { useTranslation } from "react-i18next";
import Tooltip from "../../ui/Tooltip";
import { formatTenureCounter } from "../preSeasonFormatters.js";

export default function TeamDriverRow({
  driverName,
  tenureSeasons,
  contratoVence = false,
  aposentado = false,
  isPrimarySlot = false,
  accent = "#58a6ff",
}) {
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
      <div className="flex min-w-0 flex-1 items-center gap-1.5">
        <p className={`truncate leading-[1.1] ${isPrimarySlot ? "text-[15px] font-bold text-[color:var(--text-primary)]" : "text-[14px] font-semibold text-[color:var(--text-primary)]"}`}>
            {driverName}
        </p>
        {/* Aposentado ainda sentado: correu a temporada inteira e pendura o capacete na
            virada. Vem antes do contrato vencendo porque decide o assento sozinho — quem
            se aposenta sai independentemente de quanto contrato ainda tinha. */}
        {aposentado ? (
          <Tooltip texto={t("preSeason.roster.retiring")}>
            <span
              className="shrink-0 text-[11px] leading-none"
              aria-label={t("preSeason.roster.retiring")}
            >
              {"\u{1F6AA}"}
            </span>
          </Tooltip>
        ) : null}
        {/* Contrato vencendo: só aparece na foto da semana 1, quando o assento ainda
            está ocupado mas pode sumir na virada. Depois das pré-passes o campo é falso. */}
        {contratoVence && !aposentado && (
          <Tooltip texto={t("preSeason.roster.contractExpiring")}>
            <span
              className="shrink-0 text-[11px] leading-none"
              aria-label={t("preSeason.roster.contractExpiring")}
            >
              {"\u{1F4C4}"}
            </span>
          </Tooltip>
        )}
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
