import { useTranslation } from "react-i18next";

const toneHex = {
  danger: "#f85149",
  warning: "#d29922",
  neutral: "#8b949e",
  info: "#58a6ff",
  success: "#3fb950",
  elite: "#bc8cff",
};

function StardomMeter({ label, value, nivel, tone }) {
  const color = toneHex[tone] || toneHex.neutral;
  const width = Math.max(0, Math.min(Number(value) || 0, 100));

  return (
    <div className="grid gap-1.5">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-[#7d8590]">
          {label}
        </span>
        <span className="text-sm font-bold" style={{ color }}>
          {nivel}
        </span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-[#21262d]">
        <div className="h-full rounded-full" style={{ width: `${width}%`, backgroundColor: color }} />
      </div>
      <div className="text-right font-mono text-[11px] text-[#7d8590]">{width}/100</div>
    </div>
  );
}

export function StardomSection({ SectionComponent, detail }) {
  const { t } = useTranslation();
  const stardom = detail.estrelato;
  if (!stardom) return null;

  return (
    <SectionComponent title={t("driverDetail.stardom.title")}>
      <div className="glass-light rounded-xl p-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <StardomMeter label={t("driverDetail.stardom.fame")} value={stardom.fama} nivel={stardom.nivel_fama} tone={stardom.tom_fama} />
          <StardomMeter
            label={t("driverDetail.stardom.charisma")}
            value={stardom.carisma}
            nivel={stardom.nivel_carisma}
            tone={stardom.tom_carisma}
          />
        </div>
        {stardom.resumo ? (
          <div className="mt-4 rounded-xl border border-white/6 bg-black/10 p-3 text-sm text-[#c9d1d9]">
            {stardom.resumo}
          </div>
        ) : null}
        <div className="mt-2 text-[11px] leading-relaxed text-[#7d8590]">
          {t("driverDetail.stardom.blurbPre")}<span className="text-[#e6edf3]">{t("driverDetail.stardom.blurbFame")}</span>{t("driverDetail.stardom.blurbMid")}{" "}
          <span className="text-[#e6edf3]">{t("driverDetail.stardom.blurbCharisma")}</span>{t("driverDetail.stardom.blurbPost")}
        </div>
      </div>
    </SectionComponent>
  );
}
