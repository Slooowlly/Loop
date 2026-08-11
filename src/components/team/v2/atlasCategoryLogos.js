// Logo da categoria por trás de cada card de ranking.
//
// São as versões `recortadas`: as soltas têm uma moldura branca enorme em volta
// do brasão, que num selo de 20px vira quase só espaço vazio.
//
// O dicionário em si mora em `utils/categoryLogos.js` desde 11/08/2026 — este arquivo
// virou só o nome que o atlas usa. Antes eram cinco cópias quase iguais espalhadas pelo
// frontend, cada uma conhecendo um subconjunto diferente de categorias.
//
// Nenhuma calibragem por marca aqui, de propósito. As alturas fixas que existiam
// nesta lista acertavam um arquivo e erravam o próximo — o tamanho de cada brasão
// é medido em runtime a partir do conteúdo visível dele, em
// `atlasLogoNormalization.js`. Arquivo novo entra sabendo se comportar.
import { CATEGORY_LOGOS_RECORTADOS, categoryLogoSrc } from "../../../utils/categoryLogos";

export const ATLAS_CATEGORY_LOGOS = CATEGORY_LOGOS_RECORTADOS;

export function atlasCategoryLogo(category) {
  return categoryLogoSrc(category);
}
