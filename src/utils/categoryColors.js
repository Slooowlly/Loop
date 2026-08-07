const CATEGORY_COLORS = {
  mazda_rookie: "#FFD400",
  toyota_rookie: "#FFD400",
  mazda_amador: "#E73F47",
  toyota_amador: "#E73F47",
  bmw_m2: "#E00010",
  production_challenger: "#8020D0",
  gt4: "#2070F0",
  gt3: "#00F0F0",
  lmp2: "#F2CC60",
  endurance: "#3fb950",
  // Divisões do bloco especial (`categoria:classe`). A Endurance é uma categoria
  // só, mas GT3 e LMP2 são máquinas diferentes e o piloto TROCA entre elas — na
  // escada de categorias essa troca precisa aparecer, senão catorze anos de
  // Endurance viram um bloco chapado. O verde é o da categoria; o dourado do
  // LMP2 já existia na paleta e não tinha como ser alcançado, porque `lmp2`
  // nunca é uma categoria por si, só uma classe dentro da Endurance.
  "endurance:gt3": "#3fb950",
  "endurance:lmp2": "#F2CC60",
};

// As classes da Production (`mazda`/`toyota`/`bmw`) não ganham cor própria de
// propósito: são marcas paralelas do MESMO degrau, e trocar de uma para outra
// não é subir nem descer. Caem na cor da categoria pelo prefixo.
export function getCategoryColor(category, fallback = "#58a6ff") {
  const key = String(category ?? "").trim();
  if (!key) return fallback;
  if (CATEGORY_COLORS[key]) return CATEGORY_COLORS[key];
  return CATEGORY_COLORS[key.split(":")[0]] ?? fallback;
}

export { CATEGORY_COLORS };
