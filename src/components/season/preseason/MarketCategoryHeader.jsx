import {
  subcatLabel,
  subcatColor,
  subcatLogo,
  subcatLogoFit,
} from "../preSeasonFormatters.js";

export default function MarketCategoryHeader({ categoryKey, detail }) {
  const label = subcatLabel(categoryKey);
  const color = subcatColor(categoryKey);
  const logo = subcatLogo(categoryKey);
  const logoFit = subcatLogoFit(categoryKey);

  return (
    <div
      data-testid={`preseason-category-header-${categoryKey}`}
      className="mb-5 flex flex-col items-center justify-center gap-3 rounded-xl px-4 py-6 text-center"
      style={{
        background: `linear-gradient(135deg, ${color}22 0%, ${color}0a 100%)`,
        borderLeft: `3px solid ${color}`,
        boxShadow: `0 0 18px ${color}18`,
      }}
    >
      {logo ? (
        <div className={`flex w-full items-start justify-center overflow-hidden ${logoFit.frameClassName}`}>
          <img
            data-testid="preseason-category-logo"
            src={logo}
            alt={label}
            className="h-full w-auto max-w-none object-contain"
            style={logoFit.imageStyle}
            draggable={false}
          />
        </div>
      ) : (
        <span
          className="text-[17px] font-bold uppercase tracking-[0.18em]"
          style={{ color }}
        >
          {label}
        </span>
      )}
      <span
        data-testid="preseason-category-count"
        className="shrink-0 rounded-full border px-3 py-1 text-[11px] font-bold uppercase tracking-[0.12em]"
        style={{
          color,
          borderColor: `${color}55`,
          backgroundColor: `${color}14`,
        }}
      >
        {detail}
      </span>
    </div>
  );
}
