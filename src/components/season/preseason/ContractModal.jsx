import { useTranslation } from "react-i18next";
import { formatSalaryAnnual, extractNationalityLabel } from "../../../utils/formatters";
import TeamLogoMark from "../../team/TeamLogoMark";
import FlagIcon from "../../ui/FlagIcon";
import { BOND_LEVEL_COLORS } from "../preSeasonFormatters.js";

// Contrato — documento A4 de assinatura.
export default function ContractModal({ offer, playerName, isSigning, isAdvancingWeek, onClose, onSign }) {
  const { t } = useTranslation();
  const accent = offer.team_color || "#58a6ff";
  const countryLabel = extractNationalityLabel(offer.team_country) || offer.team_country || "";
  const bondLevel = offer.bond_level ?? 1;
  const bondColor = BOND_LEVEL_COLORS[Math.min(bondLevel, BOND_LEVEL_COLORS.length) - 1];
  const hasHistory = bondLevel >= 2;
  const dur = offer.offer_duration ?? 1;
  const isProject = dur >= 2;
  const docRef = String(offer.seat_id ?? "").replace(/[^a-zA-Z0-9]/g, "").slice(-6).toUpperCase() || "000000";
  const signName = playerName || t("preSeason.contract.driverFallback");
  // Paleta do documento (folha escura, tinta clara — combina com o resto do app).
  const paper = "#0e1319";
  const ink = "var(--text-primary)";
  const inkSoft = "var(--text-secondary)";
  const inkMute = "var(--text-muted)";
  const hair = "rgba(255,255,255,0.08)";
  const money = "var(--status-green)";
  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/75 p-4 backdrop-blur-sm"
      onClick={(e) => { if (isSigning) return; if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        className="animate-scale-in relative flex max-h-[94vh] w-full max-w-[600px] flex-col overflow-hidden rounded-[14px] shadow-[0_40px_120px_-24px_rgba(0,0,0,0.85)] ring-1 ring-white/10"
        style={{ background: paper }}
      >
        {/* Faixa de cor da equipe no topo da folha */}
        <div className="h-1.5 w-full shrink-0" style={{ background: accent }} />

        {/* Botão fechar (canto) */}
        <button
          type="button"
          onClick={onClose}
          disabled={isSigning}
          className="absolute right-3 top-4 z-10 rounded-lg bg-white/5 px-3 py-2 text-body font-bold transition-colors hover:bg-white/10 disabled:opacity-40"
          style={{ color: inkSoft }}
          aria-label={t("preSeason.actions.close")}
        >
          ✕
        </button>

        {/* Folha rolável, com moldura interna estilo documento */}
        <div className="scroll-area relative flex-1 overflow-y-auto">
          {/* Marca d'água: logo gigante da equipe ao fundo */}
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center opacity-[0.04]">
            <div className="scale-[3.2]">
              <TeamLogoMark teamName={offer.team_name} color="#ffffff" size="hero" />
            </div>
          </div>

          {/* Moldura interna (margem do documento) */}
          <div className="pointer-events-none absolute inset-4 rounded-md border" style={{ borderColor: hair }} />

          <div className="relative px-8 py-8 sm:px-10">
            {/* ── Timbre: logo + identidade da equipe ── */}
            <header className="flex flex-col items-center text-center">
              <TeamLogoMark teamName={offer.team_name} color={accent} size="lg" />
              <h2 className="mt-3 text-[22px] font-black leading-tight" style={{ color: ink }}>
                {offer.team_name}
              </h2>
              <div className="mt-1 flex items-center justify-center gap-1.5 text-[11px]" style={{ color: inkMute }}>
                <FlagIcon nacionalidade={offer.team_country} className="h-3.5 w-5" />
                {countryLabel && <span>{countryLabel}</span>}
                {offer.team_founded_year ? <span>{t("preSeason.foundedSince", { year: offer.team_founded_year })}</span> : null}
              </div>
            </header>

            {/* ── Título do documento ── */}
            <div className="mt-6 flex items-center gap-3">
              <div className="h-px flex-1" style={{ background: `linear-gradient(to right, transparent, ${accent}88)` }} />
              <div className="text-center">
                <p className="text-[15px] font-black uppercase tracking-[0.3em]" style={{ color: ink }}>
                  {t("preSeason.contract.title")}
                </p>
                <p className="mt-1 text-[9px] font-semibold uppercase tracking-[0.24em]" style={{ color: accent }}>
                  {t("preSeason.contract.categoryRef", { category: offer.category_label || offer.category, ref: docRef })}
                </p>
              </div>
              <div className="h-px flex-1" style={{ background: `linear-gradient(to left, transparent, ${accent}88)` }} />
            </div>

            {/* ── Preâmbulo ── */}
            <p className="mt-5 text-[12px] leading-relaxed" style={{ color: inkSoft }}>
              <span className="font-bold" style={{ color: ink }}>{offer.team_name}</span>
              {" "}{t("preSeason.contract.preamblePart1")} <span className="font-bold" style={{ color: ink }}>{signName}</span>{" "}
              {t("preSeason.contract.preamblePart2")}
            </p>

            {/* ── Cláusulas ── */}
            <div className="mt-5 space-y-0">
              {/* I — Função */}
              <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                <div className="flex items-baseline justify-between gap-3">
                  <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                    {t("preSeason.contract.clause1")}
                  </p>
                  <p className="text-right text-body font-bold" style={{ color: accent }}>
                    {offer.role === "N1" ? t("preSeason.contract.roleN1") : t("preSeason.contract.roleN2")}
                  </p>
                </div>
              </div>

              {/* II — Remuneração */}
              <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                <div className="flex items-baseline justify-between gap-3">
                  <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                    {t("preSeason.contract.clause2")}
                  </p>
                  <p className="num-medium text-title-md font-black" style={{ color: money }}>
                    {formatSalaryAnnual(offer.salary)}
                  </p>
                </div>
              </div>

              {/* III — Vigência */}
              <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                <div className="flex items-baseline justify-between gap-3">
                  <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                    {t("preSeason.contract.clause3")}
                  </p>
                  <p className="num-medium text-body font-bold" style={{ color: isProject ? money : ink }}>
                    {t("preSeason.offers.card.contractDuration", { count: dur })}
                  </p>
                </div>
              </div>

              {/* IV — Projeto esportivo (foco do time) */}
              <div className="py-3.5" style={{ borderTop: `1px solid ${hair}` }}>
                <div className="flex items-baseline justify-between gap-3">
                  <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                    {t("preSeason.contract.clause4")}
                  </p>
                  <p className="text-right text-body font-bold" style={{ color: accent }}>
                    {offer.team_focus || t("preSeason.contract.focusFallback")}
                  </p>
                </div>
              </div>

              {/* V — Relação com a equipe (vínculo) */}
              <div className="py-3.5" style={{ borderTop: `1px solid ${hair}`, borderBottom: `1px solid ${hair}` }}>
                <div className="flex items-center justify-between gap-3">
                  <p className="text-[10px] font-black uppercase tracking-[0.2em]" style={{ color: inkMute }}>
                    {t("preSeason.contract.clause5")}
                  </p>
                  <div className="flex items-center gap-2">
                    <div className="flex gap-0.5">
                      {Array.from({ length: 6 }).map((_, i) => (
                        <span
                          key={i}
                          className="h-2 w-4 rounded-full"
                          style={{ background: i < bondLevel ? bondColor : "#21262d" }}
                        />
                      ))}
                    </div>
                    <span className="text-body-sm font-bold" style={{ color: hasHistory ? bondColor : inkMute }}>
                      {offer.bond_label || t("preSeason.contract.bondFallback")}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* ── Área de assinatura ── */}
            <div className="mt-8 grid grid-cols-2 gap-8">
              {/* Piloto (você) — assinado com animação manuscrita ao aceitar */}
              <div>
                <div
                  className="flex h-14 items-end justify-center overflow-hidden border-b-2 border-dashed"
                  style={{ borderColor: `${accent}88` }}
                >
                  {isSigning ? (
                    <span
                      className="animate-signature truncate pb-0.5 text-[24px] leading-none"
                      style={{ fontFamily: "'Segoe Script','Brush Script MT','Comic Sans MS',cursive", color: accent }}
                    >
                      {signName}
                    </span>
                  ) : (
                    <span className="pb-1.5 text-[11px] italic" style={{ color: inkMute }}>
                      {t("preSeason.contract.signHint")}
                    </span>
                  )}
                </div>
                <p className="mt-2 text-center text-[9px] uppercase tracking-[0.22em]" style={{ color: inkMute }}>
                  {t("preSeason.contract.driverRole")}
                </p>
                <p className="text-center text-[12px] font-bold" style={{ color: ink }}>{signName}</p>
              </div>
              {/* Equipe (já assinado) */}
              <div>
                <div className="flex h-14 items-end justify-center border-b-2" style={{ borderColor: "rgba(255,255,255,0.25)" }}>
                  <span
                    className="truncate pb-0.5 text-[24px] leading-none"
                    style={{ fontFamily: "'Segoe Script','Brush Script MT','Comic Sans MS',cursive", color: accent }}
                  >
                    {offer.team_name}
                  </span>
                </div>
                <p className="mt-2 text-center text-[9px] uppercase tracking-[0.22em]" style={{ color: inkMute }}>
                  {t("preSeason.contract.teamRole")}
                </p>
                <p className="text-center text-[12px] font-bold" style={{ color: ink }}>{offer.team_name}</p>
              </div>
            </div>
          </div>
        </div>

        {/* ── Rodapé de ações ── */}
        <div className="flex shrink-0 gap-3 border-t border-white/10 bg-black/30 px-6 py-4">
          <button
            type="button"
            onClick={onClose}
            disabled={isSigning}
            className="transition-glass glass-light rounded-lg px-4 py-2.5 text-body font-bold text-[color:var(--text-secondary)] hover:text-[color:var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            {t("preSeason.actions.back")}
          </button>
          <button
            type="button"
            onClick={() => onSign(offer)}
            disabled={isAdvancingWeek || isSigning}
            className="transition-glass glow-blue flex flex-1 items-center justify-center gap-2 rounded-lg border border-[#58a6ff66] bg-[#58a6ff33] px-4 py-2.5 text-body font-bold text-[color:var(--accent-primary)] hover:bg-[#58a6ff55] disabled:cursor-not-allowed disabled:opacity-60"
          >
            <span className="text-[15px] leading-none">✒️</span>
            {isSigning ? t("preSeason.contract.signing") : t("preSeason.contract.sign")}
          </button>
        </div>
      </div>
    </div>
  );
}
