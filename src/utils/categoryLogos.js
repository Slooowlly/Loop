// Fonte única do brasão de cada categoria.
//
// O mapa categoria → arquivo de logo vivia copiado em cinco lugares: a torre do overlay
// (`overlay/towerCanvas.js`), o atlas de equipes (`team/v2/atlasCategoryLogos.js`), a
// pré-temporada (`season/preSeasonFormatters.js`), a convocação (`season/convocacao/
// constantes.js`) e o calendário (`utils/calendarShared.js`). Cada cópia conhecia um
// subconjunto diferente de categorias, então categoria nova entrava em algumas telas e
// aparecia sem brasão nas outras — falha muda, que só o olho pega.
//
// Aqui o dicionário é um só, indexado pelo id do banco. As duas VARIANTES de arte continuam
// existindo porque servem a coisas diferentes:
//
//   • `recortadas/` — o brasão sem a moldura branca em volta. É a que serve para selo pequeno
//     (20 px no card do atlas, coluna da torre): na versão solta a moldura come quase todo o
//     quadro e sobra um borrão.
//   • a raiz `categorias/` — a arte cheia, com moldura. É a que o calendário usa nos tickets,
//     onde o selo é grande o bastante para a moldura virar acabamento em vez de desperdício.
//
// Os aliases sem sufixo (`mazda`, `toyota`, `bmw`) existem porque a pré-temporada e o atlas
// agrupam as duas divisões de uma marca num filtro só e consultam pelo id do grupo.

/// Nome do arquivo (sem extensão, sem escapar) por id de categoria.
const ARQUIVO_POR_CATEGORIA = {
  mazda: "MX5 CUP",
  mazda_amador: "MX5 CUP",
  mazda_rookie: "MX5 ROOKIE",
  toyota: "GR CUP",
  toyota_amador: "GR CUP",
  toyota_rookie: "GR ROOKIE",
  bmw: "M2 CUP",
  bmw_m2: "M2 CUP",
  gt4: "GT4",
  gt3: "GT3",
  lmp2: "LMP2",
  production: "PRODUCTION",
  production_challenger: "PRODUCTION",
  endurance: "ENDURANCE",
};

/// O nome do arquivo de brasão da categoria, ou `null` quando ela não tem arte.
export function categoryLogoFile(category) {
  return ARQUIVO_POR_CATEGORIA[category] ?? null;
}

/// A URL do brasão. `recortado` (o padrão) escolhe a arte sem moldura.
export function categoryLogoSrc(category, { recortado = true } = {}) {
  const arquivo = categoryLogoFile(category);
  if (!arquivo) return null;
  const pasta = recortado ? "/utilities/categorias/recortadas/" : "/utilities/categorias/";
  return `${pasta}${encodeURIComponent(arquivo)}.webp`;
}

/// Um objeto id → URL, para os pontos que consultam por chave em vez de chamar a função.
function mapaDeUrls({ recortado }) {
  return Object.fromEntries(
    Object.keys(ARQUIVO_POR_CATEGORIA).map((id) => [id, categoryLogoSrc(id, { recortado })]),
  );
}

export const CATEGORY_LOGOS_RECORTADOS = mapaDeUrls({ recortado: true });
export const CATEGORY_LOGOS_COM_MOLDURA = mapaDeUrls({ recortado: false });

/// Os ids que têm brasão, para guard e para varredura.
export const CATEGORIAS_COM_LOGO = Object.keys(ARQUIVO_POR_CATEGORIA);
