export const PRIMARY_FILTER_IDS = ["Corridas", "Pilotos", "Equipes", "Mercado"];
export const FAMOUS_FILTER_IDS = ["Pilotos", "Equipes", "Mercado"];

export function resolveStoryBlocks(story) {
  if (Array.isArray(story?.blocks) && story.blocks.length > 0) {
    return story.blocks.filter((block) => block?.label && block?.text);
  }

  const fallbackBlocks = [
    story?.summary ? { label: "Resumo", text: story.summary } : null,
    story?.body_text ? { label: "Panorama", text: story.body_text } : null,
  ].filter(Boolean);

  return fallbackBlocks;
}

export function buildFallbackPrimaryFilters(scopeType) {
  const ids = scopeType === "famous" ? FAMOUS_FILTER_IDS : PRIMARY_FILTER_IDS;
  return ids.map((id) => ({ id, label: id }));
}

export function isUpcomingRaceFilter(filter, bootstrap) {
  if (filter?.kind !== "race" || !filter?.meta || !bootstrap) return false;
  const round = Number.parseInt(String(filter.meta).replace(/\D+/g, ""), 10);
  if (!Number.isFinite(round)) return false;
  if (bootstrap.season_completed) return false;
  return round >= bootstrap.current_round;
}

export function leadBadgeLabel(importance) {
  if (importance === "Destaque") return "Destaque central";
  if (importance === "Alta") return "Em evidencia";
  return "Leitura do momento";
}

export function toneDotClass(tone) {
  if (tone === "warm") return "bg-status-yellow";
  if (tone === "accent") return "bg-accent-gold";
  return "bg-accent-primary";
}

export function storyToneBadgeClass(tone) {
  if (tone === "gold") return "border-accent-gold/38 bg-[linear-gradient(90deg,rgba(255,212,122,0.16)_0%,rgba(255,212,122,0.08)_100%)] text-accent-gold";
  if (tone === "warm") return "border-status-yellow/35 bg-[linear-gradient(90deg,rgba(240,190,84,0.14)_0%,rgba(240,190,84,0.07)_100%)] text-status-yellow";
  return "border-accent-primary/30 bg-[linear-gradient(90deg,rgba(88,166,255,0.16)_0%,rgba(88,166,255,0.08)_100%)] text-accent-primary";
}

export function contextChipToneClass(filter, primaryFilter, index, isActive) {
  if (primaryFilter !== "Pilotos" || filter.kind !== "driver" || index > 2) return "";

  const medalClasses = [
    "border-podium-gold/30 bg-podium-gold/10 text-podium-gold",
    "border-podium-silver/30 bg-podium-silver/10 text-podium-silver",
    "border-podium-bronze/30 bg-podium-bronze/10 text-podium-bronze",
  ];

  if (isActive) {
    return `${medalClasses[index]} shadow-[inset_0_0_0_1px_rgba(255,255,255,0.08)]`;
  }

  return medalClasses[index];
}

export function contextChipStyle(filter, isActive) {
  if (filter.kind !== "team" || !filter.color_primary) return undefined;

  return {
    borderColor: isActive ? `${filter.color_primary}66` : `${filter.color_primary}40`,
    background: isActive
      ? `linear-gradient(90deg, ${hexToRgba(filter.color_primary, 0.18)}, ${hexToRgba(filter.color_secondary || filter.color_primary, 0.08)})`
      : `linear-gradient(90deg, ${hexToRgba(filter.color_primary, 0.1)}, rgba(255,255,255,0.03))`,
  };
}

export function hexToRgba(color, alpha) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) return `rgba(88,166,255,${alpha})`;
  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function getReadableTeamColor(color) {
  if (!color || !/^#([0-9a-f]{6})$/i.test(color)) return "#d0d7e2";
  const hex = color.slice(1);
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
  if (luminance < 0.32) {
    const mixWithWhite = 0.62;
    const boost = (channel) => Math.round(channel + (255 - channel) * mixWithWhite);
    return `rgb(${boost(r)}, ${boost(g)}, ${boost(b)})`;
  }
  return color;
}
