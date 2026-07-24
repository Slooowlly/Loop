// Constantes visuais da tela de resultado V1 (RaceResultView).

export const CATEGORY_SUMMARY_LOGOS = {
  mazda: "/utilities/categorias/recortadas/MX5%20CUP.webp",
  mazda_amador: "/utilities/categorias/recortadas/MX5%20CUP.webp",
  mazda_rookie: "/utilities/categorias/recortadas/MX5%20ROOKIE.webp",
  toyota: "/utilities/categorias/recortadas/GR%20CUP.webp",
  toyota_amador: "/utilities/categorias/recortadas/GR%20CUP.webp",
  toyota_rookie: "/utilities/categorias/recortadas/GR%20ROOKIE.webp",
  bmw: "/utilities/categorias/recortadas/M2%20CUP.webp",
  bmw_m2: "/utilities/categorias/recortadas/M2%20CUP.webp",
  gt4: "/utilities/categorias/recortadas/GT4.webp",
  gt3: "/utilities/categorias/recortadas/GT3.webp",
  production_challenger: "/utilities/categorias/recortadas/PRODUCTION.webp",
  endurance: "/utilities/categorias/recortadas/ENDURANCE.webp",
  lmp2: "/utilities/categorias/recortadas/LMP2.webp",
};

export const CATEGORY_SUMMARY_FITS = {
  mazda: {
    frameClassName: "overflow-hidden",
    imageStyle: {
      clipPath: "inset(0 0 8% 0)",
    },
  },
  mazda_amador: {
    frameClassName: "overflow-hidden",
    imageStyle: {
      clipPath: "inset(0 0 8% 0)",
    },
  },
};

// Avaliação do cérebro (race_eval) → rótulo + cor + emoji.
export const ASSESSMENT = {
  MuitoAcima: { label: "Muito acima do esperado", color: "text-green-400", emoji: "🔥" },
  Acima: { label: "Acima do esperado", color: "text-green-400", emoji: "✅" },
  Dentro: { label: "Dentro do esperado", color: "text-[#58a6ff]", emoji: "🎯" },
  Abaixo: { label: "Abaixo do esperado", color: "text-amber-400", emoji: "⚠️" },
  MuitoAbaixo: { label: "Muito abaixo do esperado", color: "text-red-400", emoji: "🔻" },
};

// Chip de confiança da telemetria (alta/media/baixa).
export const CONFIDENCE = {
  alta: { label: "Confiança alta", color: "text-green-400 border-green-500/30 bg-green-500/10" },
  media: { label: "Confiança média", color: "text-amber-400 border-amber-500/30 bg-amber-500/10" },
  baixa: { label: "Confiança baixa", color: "text-gray-400 border-white/15 bg-white/5" },
};

// ── Estratégia de pneus (inferida por paradas + clima; seco/chuva) ──────────
export const COMPOUND = {
  Dry: { label: "Seco", cls: "bg-amber-500/15 text-amber-300 border-amber-500/30" },
  Wet: { label: "Chuva", cls: "bg-sky-500/15 text-sky-300 border-sky-500/30" },
  Unknown: { label: "—", cls: "bg-gray-500/15 text-gray-400 border-gray-500/30" },
};

// Estimativa do bloco de troca de pneus (s). Não exato — vai de ~20 a 22.
export const TIRE_EST_SECS = 21;
