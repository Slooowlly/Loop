export const CATEGORY_LABELS = {
  production_challenger: "Production",
  endurance: "Endurance",
};

// O bloco especial só tem estas duas categorias, mas o brasão vem do dicionário único
// (`utils/categoryLogos.js`) — assim uma categoria especial nova já nasce com arte aqui.
export { CATEGORY_LOGOS_RECORTADOS as CATEGORY_LOGOS } from "../../../utils/categoryLogos";

export const CATEGORY_COLORS = {
  all: "rgba(255,255,255,0.35)",
  production_challenger: "#9030E0",
  endurance: "#30D010",
};

export const CATEGORY_FILTERS = [
  { id: "all", label: "Todas", color: CATEGORY_COLORS.all },
  {
    id: "production_challenger",
    label: "Production",
    color: CATEGORY_COLORS.production_challenger,
  },
  { id: "endurance", label: "Endurance", color: CATEGORY_COLORS.endurance },
];

export const CATEGORY_ORDER = ["production_challenger", "endurance"];
export const CANDIDATE_GROUP_ORDER = [
  "lmp2",
  "gt3",
  "gt4",
  "bmw_m2",
  "toyota_amador",
  "mazda_amador",
];

export const CANDIDATE_GROUP_LABELS = {
  lmp2: "LMP2 Prototype Championship",
  gt3: "GT3 Championship",
  gt4: "GT4 Series",
  bmw_m2: "BMW",
  toyota_amador: "Toyota Cup",
  mazda_amador: "Mazda Cup",
};

export const CLASS_COLORS = {
  mazda: "#C8102E",
  toyota: "#E8841A",
  bmw: "#6B4FBB",
  gt4: "#58a6ff",
  gt3: "#f85149",
  lmp2: "#d29922",
  geral: "#8b949e",
};

export const CANDIDATE_GROUP_COLORS = {
  lmp2: "#d29922",
  gt3: "#f85149",
  gt4: "#58a6ff",
  bmw_m2: "#6B4FBB",
  toyota_amador: "#E8841A",
  mazda_amador: "#C8102E",
};

export const DAILY_LOG_CLASS_ORDER = ["lmp2", "gt3", "gt4", "bmw", "toyota", "mazda", "geral"];
export const TEAM_CLASS_ORDER = {
  production_challenger: ["bmw", "toyota", "mazda"],
  endurance: ["lmp2", "gt3", "gt4"],
};

export const LICENSE_COLORS = {
  R: { text: "#9ba3ae", bg: "rgba(155,163,174,0.12)" },
  A: { text: "#3fb950", bg: "rgba(63,185,80,0.12)" },
  P: { text: "#58a6ff", bg: "rgba(88,166,255,0.12)" },
  SP: { text: "#FF8000", bg: "rgba(255,128,0,0.12)" },
  E: { text: "#bc8cff", bg: "rgba(188,140,255,0.12)" },
  SE: { text: "#ffd700", bg: "rgba(255,215,0,0.12)" },
};
