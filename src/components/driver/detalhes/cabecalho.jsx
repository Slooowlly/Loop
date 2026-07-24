import { useTranslation } from "react-i18next";

import { formatAttributeName } from "./formatadores.js";

export function nationalityForFlag(perfil, detail) {
  if (perfil?.bandeira && perfil?.nacionalidade) {
    return `${perfil.bandeira} ${perfil.nacionalidade}`;
  }

  if (perfil?.nacionalidade) {
    return perfil.nacionalidade;
  }

  return detail?.nacionalidade || "";
}

export function BadgePill({ badge }) {
  const variants = {
    player: "bg-[#58a6ff]/18 text-[#58a6ff]",
    success: "bg-[#3fb950]/18 text-[#3fb950]",
    warning: "bg-[#d29922]/18 text-[#d29922]",
    info: "bg-white/10 text-[#c9d1d9]",
  };

  return (
    <span
      className={[
        "rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.18em]",
        variants[badge?.variant] || variants.info,
      ].join(" ")}
    >
      {badge?.label}
    </span>
  );
}

export function FavoriteStarButton({ active, disabled, onClick }) {
  const { t } = useTranslation();
  const favoriteLabel = active ? t("driverDetail.favorite.remove") : t("driverDetail.favorite.add");
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-pressed={active}
      title={favoriteLabel}
      aria-label={favoriteLabel}
      className={[
        "flex h-7 w-7 items-center justify-center rounded-lg border text-[15px] leading-none transition-all",
        active
          ? "border-[#fbbf24]/50 bg-[#fbbf24]/15 text-[#fbbf24] shadow-[0_0_10px_-3px_#fbbf24]"
          : "border-white/10 bg-white/[0.04] text-[#7d8590] hover:border-[#fbbf24]/40 hover:text-[#fbbf24]",
        disabled ? "cursor-not-allowed opacity-60" : "",
      ].join(" ")}
    >
      {active ? "★" : "☆"}
    </button>
  );
}

export function PersonalityCard({ personality }) {
  const { t } = useTranslation();
  if (!personality) {
    return (
      <div className="glass-light rounded-xl p-3">
        <p className="text-xs text-[#7d8590]">{t("driverDetail.personality.empty")}</p>
      </div>
    );
  }

  return (
    <div className="glass-light flex items-start gap-3 rounded-xl p-3">
      <span className="text-lg leading-none">{personality.emoji}</span>
      <div>
        <div className="text-sm font-semibold text-[#e6edf3]">{personality.tipo}</div>
        <div className="mt-1 text-[11px] text-[#7d8590]">{personality.descricao}</div>
      </div>
    </div>
  );
}

export function TagRow({ tag }) {
  return (
    <div className="flex items-center gap-2 py-0.5">
      <span
        className="h-2 w-2 flex-shrink-0 rounded-full"
        style={{ backgroundColor: tag.color }}
      />
      <span className="text-sm text-[#e6edf3]">{tag.tag_text}</span>
      <span className="ml-auto text-[10px] italic text-[#6e7681]">
        {formatAttributeName(tag.attribute_name)}
      </span>
    </div>
  );
}
