// Logo da categoria por trás de cada card de ranking.
//
// São as versões `recortadas`: as soltas têm uma moldura branca enorme em volta
// do brasão, que num selo de 20px vira quase só espaço vazio.
//
// NOTA: este mapa já existe quase igual em `race/raceresult/constants.js`,
// `season/convocacao/constantes.js` e `season/preSeasonFormatters.js`, e a versão
// sem recorte está em `utils/calendarShared.js`. Copiado para cá seguindo o
// README do v2 (nada aqui importa do v1, e acoplar a tela de resultado de corrida
// ao atlas seria pior) — mas os quatro mereciam virar um só em `utils/`.
const MX5_CUP = "/utilities/categorias/recortadas/MX5%20CUP.webp";
const MX5_ROOKIE = "/utilities/categorias/recortadas/MX5%20ROOKIE.webp";
const GR_CUP = "/utilities/categorias/recortadas/GR%20CUP.webp";
const GR_ROOKIE = "/utilities/categorias/recortadas/GR%20ROOKIE.webp";
const M2_CUP = "/utilities/categorias/recortadas/M2%20CUP.webp";
const GT4 = "/utilities/categorias/recortadas/GT4.webp";
const GT3 = "/utilities/categorias/recortadas/GT3.webp";
const PRODUCTION = "/utilities/categorias/recortadas/PRODUCTION.webp";
const ENDURANCE = "/utilities/categorias/recortadas/ENDURANCE.webp";
const LMP2 = "/utilities/categorias/recortadas/LMP2.webp";

// Nenhuma calibragem por marca aqui, de propósito. As alturas fixas que existiam
// nesta lista acertavam um arquivo e erravam o próximo — o tamanho de cada brasão
// é medido em runtime a partir do conteúdo visível dele, em
// `atlasLogoNormalization.js`. Arquivo novo entra sabendo se comportar.
export const ATLAS_CATEGORY_LOGOS = {
  mazda: MX5_CUP,
  mazda_amador: MX5_CUP,
  mazda_rookie: MX5_ROOKIE,
  toyota: GR_CUP,
  toyota_amador: GR_CUP,
  toyota_rookie: GR_ROOKIE,
  bmw: M2_CUP,
  bmw_m2: M2_CUP,
  gt4: GT4,
  gt3: GT3,
  production_challenger: PRODUCTION,
  endurance: ENDURANCE,
  lmp2: LMP2,
};

export function atlasCategoryLogo(category) {
  return ATLAS_CATEGORY_LOGOS[category] ?? null;
}
