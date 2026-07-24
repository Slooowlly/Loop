import i18n from "../../i18n/index.js";
import { categoryLabel } from "../../utils/formatters";

// Cabeçalhos e avisos das categorias MULTICLASSE (production/endurance): a faixa que
// separa as classes e os placeholders de "ainda não rodou".

export function SpecialClassHeader({ section, sticky = false }) {
  return (
    <div
      className={[
        "flex items-center justify-center gap-3 py-2.5",
        sticky ? "sticky left-0 z-10 w-[min(760px,calc(100vw-3rem))]" : "w-full",
      ].join(" ")}
    >
      <span
        className="h-px flex-1"
        style={{
          background: `linear-gradient(90deg, transparent 0%, ${section.color}4d 58%, ${section.color}c2 100%)`,
        }}
      />
      <span
        className="shrink-0 px-3 text-center text-[17px] font-black uppercase leading-none tracking-[0.22em]"
        style={{
          color: section.color,
          textShadow: `0 0 18px ${section.color}55`,
        }}
      >
        {section.label}
      </span>
      <span
        className="h-px flex-1"
        style={{
          background: `linear-gradient(90deg, ${section.color}c2 0%, ${section.color}4d 42%, transparent 100%)`,
        }}
      />
    </div>
  );
}

function specialPendingMessage(phase) {
  if (phase === "JanelaConvocacao") {
    return i18n.t("standings.special.callupWindow");
  }
  if (phase === "BlocoEspecial") {
    return i18n.t("standings.special.specialOpen");
  }
  return i18n.t("standings.special.afterRegular");
}

export function SpecialPendingNotice({ category, phase }) {
  return (
    <div className="mt-6 rounded-3xl border border-white/10 bg-white/[0.035] p-6 text-center shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
      <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-accent-primary">
        {categoryLabel(category)}
      </p>
      <h3 className="mt-3 text-xl font-semibold text-text-primary">
        {i18n.t("standings.special.notYetTitle")}
      </h3>
      <p className="mx-auto mt-3 max-w-xl text-sm leading-6 text-text-secondary">
        {specialPendingMessage(phase)}
      </p>
    </div>
  );
}

export function SpecialPendingTeamsNotice() {
  return (
    <div className="rounded-2xl border border-white/8 bg-white/[0.025] px-4 py-5 text-sm leading-6 text-text-secondary">
      {i18n.t("standings.special.teamsPending")}
    </div>
  );
}
