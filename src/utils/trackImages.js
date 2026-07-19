// Resolve o caminho da imagem de uma pista (public/utilities/tracks).
// Espelha a lógica usada em CalendarTab/Header: casa por nome normalizado e,
// em fallback, por track_id; por último tenta "<nome>.png".

// Fallback por track_id (ids reais do iRacing). O casamento por nome tem
// prioridade em getTrackImageSrc; este mapa só cobre colisões de nome.
const TRACK_IMAGES = {
  9: "/utilities/tracks/summitpoint.png",
  353: "/utilities/tracks/limerock.jpeg",
  586: "/utilities/tracks/lagunaseca.png",
  166: "/utilities/tracks/okayama.png",
  181: "/utilities/tracks/oultonpark.jpeg",
  182: "/utilities/tracks/oultonpark.jpeg",
  180: "/utilities/tracks/oultonpark.jpeg",
  324: "/utilities/tracks/Tsukuba.png",
  449: "/utilities/tracks/motorsport arena.png",
  451: "/utilities/tracks/rudskogen.jpeg",
  489: "/utilities/tracks/ledenon.png",
  202: "/utilities/tracks/oranpark.png",
  440: "/utilities/tracks/winton.jpeg",
  515: "/utilities/tracks/Navarra.png",
  554: "/utilities/tracks/charlotte.png",
  465: "/utilities/tracks/virginia.jpeg",
};

const TRACK_IMAGE_FILES = [
  { match: ["charlotte"], file: "charlotte.png" },
  { match: ["laguna seca"], file: "lagunaseca.png" },
  { match: ["lime rock"], file: "limerock.jpeg" },
  { match: ["okayama"], file: "okayama.png" },
  { match: ["oulton"], file: "oultonpark.jpeg" },
  { match: ["snetterton"], file: "snetterton.jpeg" },
  { match: ["summit point", "jefferson"], file: "summitpoint.png" },
  { match: ["tsukuba"], file: "Tsukuba.png" },
  { match: ["virginia international raceway", "vir full", "vir patriot"], file: "virginia.jpeg" },
  { match: ["ledenon"], file: "ledenon.png" },
  { match: ["oschersleben", "motorsport arena"], file: "motorsport arena.png" },
  { match: ["navarra"], file: "Navarra.png" },
  { match: ["oran park"], file: "oranpark.png" },
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
  return `/utilities/tracks/${encodeURIComponent(trackName ?? "")}.png`;
}
