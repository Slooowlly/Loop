const CATEGORY_COLORS = {
  mazda_rookie: "#FFD400",
  toyota_rookie: "#FFD400",
  mazda_amador: "#E73F47",
  toyota_amador: "#E73F47",
  bmw_m2: "#E00010",
  production_challenger: "#8020D0",
  gt4: "#2070F0",
  gt3: "#00F0F0",
  endurance: "#3fb950",
};

export function getCategoryColor(category, fallback = "#58a6ff") {
  return CATEGORY_COLORS[category] ?? fallback;
}

export { CATEGORY_COLORS };
