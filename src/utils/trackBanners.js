// Imagens LARGAS/cinematográficas do banner do Home (pasta "Pistas Header").
//
// Conjunto de assets PRÓPRIO, distinto das miniaturas de utils/trackImages.js:
// 62 panorâmicas aqui contra 16 miniaturas lá. Por isso o resolvedor é separado —
// só a normalização de nome é compartilhada. Se a pista não tiver panorâmica, cai
// na miniatura, e o onError do banner segura o visual só com o gradiente.
//
// Os nomes de arquivo refletem exatamente o que está em disco (inclusive grafias
// como "chartlotte"/"outonpark"/"rudsoken").
import { getTrackThumbnailSrc, normalizeTrackName } from "./trackImages";

const BANNER_IMAGE_DIR = "/utilities/tracks/Pistas%20Header";

const BANNER_IMAGE_FILES = [
  { match: ["algarve", "portimao"], file: "Algarve International Circuit.jpg", focus: "center 27%" },
  { match: ["hermanos rodriguez", "rodriguez", "mexico city"], file: "Autódromo Hermanos Rodriguez.jpg", focus: "center 63%" },
  { match: ["jose carlos pace", "interlagos"], file: "Autódromo José Carlos Pace.jpg", focus: "center 28%" },
  { match: ["monza"], file: "Autódromo Nazionale Monza.jpg", focus: "center 36%" },
  { match: ["brands hatch"], file: "Brands Hatch.jpg", focus: "center 86%" },
  { match: ["cadwell"], file: "Cadwell Park Circuit.jpg", focus: "center 15%" },
  { match: ["canadian tire", "mosport"], file: "Canadian Tire Motorsport Park.jpg", focus: "center 70%" },
  { match: ["zandvoort"], file: "Circuit Park Zandvoort.jpg", focus: "center 64%" },
  { match: ["barcelona", "catalunya"], file: "Circuit de Barcelona-Catalunya.jpg", focus: "center 56%" },
  { match: ["magny-cours", "magny cours"], file: "Circuit de Nevers Magny-Cours.jpg", focus: "center 60%" },
  { match: ["francorchamps", "spa-francorchamps"], file: "Circuit de Spa-Francorchamps.jpg", focus: "center 44%" },
  { match: ["le mans", "24 heures", "sarthe", "24 hours"], file: "Circuit des 24 Heures du Mans.jpg", focus: "center 32%" },
  { match: ["circuit of the americas", "cota", "of the americas"], file: "Circuit of the Americas.jpg", focus: "center 34%" },
  { match: ["melbourne", "albert park"], file: "Circuito de Melbourne.jpg", focus: "center 14%" },
  { match: ["detroit", "belle isle"], file: "Detroit Grand Prix at Belle Isle.jpg" },
  { match: ["donington"], file: "Donington Park Racing Circuit.jpg", focus: "center 64%" },
  { match: ["fuji"], file: "Fuji International Speedway.jpg", focus: "center 58%" },
  { match: ["hockenheim"], file: "HockenheimRing.jpg", focus: "center 86%" },
  { match: ["hungaroring", "hungar"], file: "Hungaroring.jpg", focus: "center 39%" },
  { match: ["long beach"], file: "Long Beach Street Circuit.jpg", focus: "center 67%" },
  { match: ["road atlanta"], file: "Michelin Raceway Road Atlanta.jpg" },
  { match: ["mid-ohio", "mid ohio"], file: "Mid-Ohio Sports Car Course.jpg", focus: "center 43%" },
  { match: ["misano"], file: "Misano World Circuit Marco Simoncelli.jpg", focus: "center 52%" },
  { match: ["mount panorama", "bathurst"], file: "Mount Panorama Circuit.jpg", focus: "center 48%" },
  { match: ["nurburgring", "nordschleife"], file: "Nürburgring Nordschleife.jpg", focus: "center 49%" },
  { match: ["red bull ring", "spielberg"], file: "Red Bull Ring – Spielberg.jpg", focus: "center 49%" },
  { match: ["sandown"], file: "Sandown International Motor Raceway.jpg", focus: "center 50%" },
  { match: ["summit point", "jefferson"], file: "Summit Point — Raceway.jpg", focus: "center 52%" },
  { match: ["thruxton"], file: "Thruxton Circuit.jpg", focus: "center 19%" },
  { match: ["virginia", "vir full", "vir patriot"], file: "Virginia Int. Raceway.jpg", focus: "center 51%" },
  { match: ["watkins"], file: "Watkins Glen International.jpg", focus: "center 45%" },
  { match: ["mugello"], file: "autodromo Internazionale del Mugello.jpg", focus: "center 66%" },
  { match: ["charlotte"], file: "chartlotte.jpg", focus: "center 64%" },
  { match: ["daytona"], file: "daytona.jpg", focus: "center 51%" },
  { match: ["laguna seca", "laguna"], file: "laguna seca.jpg", focus: "center 40%" },
  { match: ["ledenon"], file: "ledenon.jpg" },
  { match: ["lime rock", "limerock"], file: "limerockpark.jpg", focus: "center 44%" },
  { match: ["motorsport arena", "oschersleben"], file: "motorsportarena.jpg", focus: "center 58%" },
  { match: ["navarra"], file: "navarra_panorama_1915x821.jpg", focus: "center 66%" },
  { match: ["okayama"], file: "okayama.jpg", focus: "center 37%" },
  { match: ["oran park"], file: "oran park.jpg", focus: "center 47%" },
  { match: ["oulton park", "oulton"], file: "outonpark.jpg", focus: "center 46%" },
  { match: ["road america"], file: "road america.jpg", focus: "center 48%" },
  { match: ["rudskogen"], file: "rudsoken.jpg", focus: "center 59%" },
  { match: ["sebring"], file: "sebring.jpg", focus: "center 68%" },
  { match: ["snetterton"], file: "snetterton.jpg", focus: "center 66%" },
  { match: ["sonoma", "sears point", "infineon"], file: "sonoma.jpg", focus: "center 47%" },
  { match: ["tsukuba"], file: "tsukuba.jpg", focus: "center 56%" },
  { match: ["winton"], file: "winton.jpg", focus: "center 35%" },
  { match: ["suzuka"], file: "Suzuka International Racing Course.jpg", focus: "center 48%" },
  { match: ["silverstone"], file: "Silverstone Circuit.jpg", focus: "center 50%" },
  { match: ["philip island", "phillip island"], file: "Philip Island Grand Prix Circuit.jpg", focus: "center 21%" },
  { match: ["zolder"], file: "Circuit Zolder.jpg", focus: "center 33%" },
  // Arte adicionada recentemente (arquivos .png em disco).
  { match: ["adelaide"], file: "Adelaide Street Circuit.webp", focus: "67% 55%" },
  { match: ["enzo e dino", "imola"], file: "Autódromo Internazionale Enzo e Dino Ferrari.webp", focus: "center 71%" },
  { match: ["barber"], file: "Barber Motorsports Park.webp", focus: "center 19%" },
  { match: ["chicago"], file: "Chicago Street Course.webp", focus: "center 17%" },
  { match: ["gilles villeneuve", "montreal"], file: "Circuit Gilles Villeneuve.webp" },
  { match: ["jerez"], file: "Circuito de Jerez.webp", focus: "center 45%" },
  { match: ["indianapolis", "indy road"], file: "Indianapolis Motor Speedway.webp", focus: "center 63%" },
  { match: ["knockhill"], file: "Knockhill.webp" },
  { match: ["miami"], file: "Miami.webp", focus: "center 65%" },
  // Pistas AINDA sem arte na pasta: o mapa já espera estes arquivos.
  // Basta soltar um arquivo com EXATAMENTE este nome em "Pistas Header/" que o banner
  // passa a usá-lo. Enquanto não existir, cai no fundo premium (fallback).
  { match: ["motegi", "mobility resort"], file: "Mobility Resort Motegi.webp" },
  { match: ["aragon", "motorland"], file: "MotorLand Aragon.webp" },
  { match: ["portland"], file: "Portland International Raceway.webp" },
  { match: ["qualcomm", "coronado", "naval base"], file: "Qualcomm Circuit.webp" },
  { match: ["sachsenring"], file: "Sachsenring.webp" },
  { match: ["the bend", "shell v-power"], file: "The Bend Motorsport Park.webp" },
  { match: ["petersburg", "st. pete", "st pete"], file: "St Petersburg Grand Prix.webp" },
  { match: ["willow springs"], file: "Willow Springs International Raceway.webp" },
];

// Ponto de foco (object-position) do corte do banner. Cada foto tem o "assunto"
// numa altura diferente; sem override, ancoramos em BANNER_FOCUS_DEFAULT. Para
// calibrar uma pista, basta adicionar `focus: "center NN%"` na entrada dela em
// BANNER_IMAGE_FILES (NN menor sobe o corte, maior desce).
const BANNER_FOCUS_DEFAULT = "center 38%";

function findBannerEntry(trackName) {
  const normalizado = normalizeTrackName(trackName);
  return BANNER_IMAGE_FILES.find(({ match }) =>
    match.some((candidate) => normalizado.includes(candidate)),
  );
}

export function getBannerImageSrc(trackName) {
  const entry = findBannerEntry(trackName);
  if (entry) return `${BANNER_IMAGE_DIR}/${encodeURIComponent(entry.file)}`;

  // Sem imagem larga → usa a miniatura de sempre como fallback.
  return getTrackThumbnailSrc(trackName);
}

export function getBannerImageFocus(trackName) {
  return findBannerEntry(trackName)?.focus ?? BANNER_FOCUS_DEFAULT;
}
