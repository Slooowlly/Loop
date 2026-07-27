// Dono único da resolução de MINIATURA de pista (public/utilities/tracks) e da
// normalização de nome que serve de chave de lookup.
//
// A normalização é usada também pelo banner do Header (utils/trackBanners.js) e
// por qualquer coisa que precise casar nome de pista com asset — manter uma só
// evita o cenário em que uma pista nova aparece numa tela e some na outra.

// Fallback por track_id (ids reais do iRacing). O casamento por nome tem
// prioridade em getTrackThumbnailSrc; este mapa só cobre colisões de nome.
const TRACK_IMAGES = {
  9: "/utilities/tracks/summitpoint.webp",
  353: "/utilities/tracks/limerock.jpeg",
  586: "/utilities/tracks/lagunaseca.webp",
  166: "/utilities/tracks/okayama.webp",
  180: "/utilities/tracks/oultonpark.jpeg",
  181: "/utilities/tracks/oultonpark.jpeg",
  182: "/utilities/tracks/oultonpark.jpeg",
  324: "/utilities/tracks/Tsukuba.webp",
  449: "/utilities/tracks/motorsport arena.webp",
  451: "/utilities/tracks/rudskogen.jpeg",
  489: "/utilities/tracks/ledenon.webp",
  202: "/utilities/tracks/oranpark.webp",
  440: "/utilities/tracks/winton.jpeg",
  515: "/utilities/tracks/Navarra.webp",
  554: "/utilities/tracks/charlotte.webp",
  465: "/utilities/tracks/virginia.jpeg",
};

const TRACK_IMAGE_FILES = [
  { match: ["charlotte"], file: "charlotte.webp" },
  { match: ["laguna seca"], file: "lagunaseca.webp" },
  { match: ["lime rock"], file: "limerock.jpeg" },
  { match: ["okayama"], file: "okayama.webp" },
  { match: ["oulton"], file: "oultonpark.jpeg" },
  { match: ["snetterton"], file: "snetterton.jpeg" },
  { match: ["summit point", "jefferson"], file: "summitpoint.webp" },
  { match: ["tsukuba"], file: "Tsukuba.webp" },
  { match: ["virginia international raceway", "vir full", "vir patriot"], file: "virginia.jpeg" },
  { match: ["ledenon"], file: "ledenon.webp" },
  { match: ["oschersleben", "motorsport arena"], file: "motorsport arena.webp" },
  { match: ["navarra"], file: "Navarra.webp" },
  { match: ["oran park"], file: "oranpark.webp" },
  { match: ["rudskogen"], file: "rudskogen.jpeg" },
  { match: ["winton"], file: "winton.jpeg" },
];

// NFD + remoção de diacríticos + minúscula. Deliberadamente NÃO mexe em espaço,
// hífen nem pontuação: as tabelas de match acima (e as do banner) foram escritas
// contra esse contrato, e afrouxá-lo mudaria silenciosamente quais pistas casam.
export function normalizeTrackName(trackName) {
  return (trackName ?? "")
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

function trackAssetPath(file) {
  if (!file) return null;
  const prefixo = "/utilities/tracks/";
  const nome = file.startsWith(prefixo) ? file.slice(prefixo.length) : file;
  return `${prefixo}${encodeURIComponent(nome)}`;
}

// `aoFalhar` decide o que acontece quando nem o nome nem o id acham arquivo:
// - "chutar" (padrão): devolve `<nome>.webp`, que quase sempre 404 e deixa o
//   onError da <img> assumir. É o comportamento histórico das telas de notícia.
// - "nulo": devolve null, para quem sabe desenhar um placeholder (EventRow).
export function getTrackThumbnailSrc(trackName, trackId, { aoFalhar = "chutar" } = {}) {
  const normalizado = normalizeTrackName(trackName);
  const entry = TRACK_IMAGE_FILES.find(({ match }) =>
    match.some((candidate) => normalizado.includes(candidate)),
  );
  if (entry) return trackAssetPath(entry.file);

  if (trackId != null && TRACK_IMAGES[trackId]) {
    return trackAssetPath(TRACK_IMAGES[trackId]);
  }

  if (aoFalhar === "nulo") return null;
  return `/utilities/tracks/${encodeURIComponent(trackName ?? "")}.webp`;
}

// Assinatura histórica, mantida para os call sites que passam (nome, id).
export function getTrackImageSrc(trackName, trackId) {
  return getTrackThumbnailSrc(trackName, trackId);
}
