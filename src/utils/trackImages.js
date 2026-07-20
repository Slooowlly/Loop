// Resolve o caminho da imagem de uma pista (public/utilities/tracks).
// Espelha a lógica usada em CalendarTab/Header: casa por nome normalizado e,
// em fallback, por track_id; por último tenta "<nome>.png".

// Fallback por track_id (ids reais do iRacing). O casamento por nome tem
// prioridade em getTrackImageSrc; este mapa só cobre colisões de nome.
const TRACK_IMAGES = {
  9: "/utilities/tracks/summitpoint.webp",
  353: "/utilities/tracks/limerock.jpeg",
  586: "/utilities/tracks/lagunaseca.webp",
  166: "/utilities/tracks/okayama.webp",
  181: "/utilities/tracks/oultonpark.jpeg",
  182: "/utilities/tracks/oultonpark.jpeg",
  180: "/utilities/tracks/oultonpark.jpeg",
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

function normalizeTrackName(trackName) {
  return (trackName ?? "")
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

export function getTrackImageSrc(trackName, trackId) {
  const normalized = normalizeTrackName(trackName);
  const entry = TRACK_IMAGE_FILES.find(({ match }) =>
    match.some((candidate) => normalized.includes(candidate)),
  );
  if (entry) {
    return `/utilities/tracks/${encodeURIComponent(entry.file)}`;
  }
  if (trackId != null && TRACK_IMAGES[trackId]) {
    return TRACK_IMAGES[trackId];
  }
  return `/utilities/tracks/${encodeURIComponent(trackName ?? "")}.webp`;
}
