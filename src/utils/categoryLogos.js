// Logos de categoria em duas variantes: "recortada" (arte sem margem, usada em
// cabecalhos e paineis) e "solta" (arte original, usada no calendario).

const CROPPED_CATEGORY_LOGOS = {
  mazda: "/utilities/categorias/recortadas/MX5%20CUP.png",
  mazda_amador: "/utilities/categorias/recortadas/MX5%20CUP.png",
  mazda_rookie: "/utilities/categorias/recortadas/MX5%20ROOKIE.png",
  toyota: "/utilities/categorias/recortadas/GR%20CUP.png",
  toyota_amador: "/utilities/categorias/recortadas/GR%20CUP.png",
  toyota_rookie: "/utilities/categorias/recortadas/GR%20ROOKIE.png",
  bmw: "/utilities/categorias/recortadas/M2%20CUP.png",
  bmw_m2: "/utilities/categorias/recortadas/M2%20CUP.png",
  gt4: "/utilities/categorias/recortadas/GT4.png",
  gt3: "/utilities/categorias/recortadas/GT3.png",
  production_challenger: "/utilities/categorias/recortadas/PRODUCTION.png",
  endurance: "/utilities/categorias/recortadas/ENDURANCE.png",
  lmp2: "/utilities/categorias/recortadas/LMP2.png",
};

const CATEGORY_LOGOS = {
  mazda_rookie: "/utilities/categorias/MX5%20ROOKIE.png",
  toyota_rookie: "/utilities/categorias/GR%20ROOKIE.png",
  mazda_amador: "/utilities/categorias/MX5%20CUP.png",
  toyota_amador: "/utilities/categorias/GR%20CUP.png",
  bmw_m2: "/utilities/categorias/M2%20CUP.png",
  production_challenger: "/utilities/categorias/PRODUCTION.png",
  gt4: "/utilities/categorias/GT4.png",
  gt3: "/utilities/categorias/GT3.png",
  lmp2: "/utilities/categorias/LMP2.png",
  endurance: "/utilities/categorias/ENDURANCE.png",
};

export function getCroppedCategoryLogo(category, fallback = null) {
  if (typeof category !== "string") return fallback;
  return CROPPED_CATEGORY_LOGOS[category] ?? fallback;
}

export function getCategoryLogo(category, fallback = null) {
  if (typeof category !== "string") return fallback;
  return CATEGORY_LOGOS[category] ?? fallback;
}

export { CROPPED_CATEGORY_LOGOS, CATEGORY_LOGOS };
