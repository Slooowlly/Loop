export function formatDate(isoString) {
  if (!isoString) return "-";
  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) return "-";

  return date.toLocaleDateString("pt-BR", {
    day: "2-digit",
    month: "short",
    year: "numeric",
  });
}

export function formatCompactDate(isoString) {
  if (!isoString) return "-";
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(isoString);
  if (!match) return "-";
  return `${match[3]}/${match[2]}/${match[1]}`;
}

export function formatNextRaceCountdown(daysUntilNextEvent) {
  if (daysUntilNextEvent == null) return "Sem corrida pendente";
  if (daysUntilNextEvent <= 0) return "Próxima corrida hoje";
  if (daysUntilNextEvent === 1) return "Próxima corrida amanhã";
  if (daysUntilNextEvent <= 7) {
    return `Próxima corrida em ${daysUntilNextEvent} dias`;
  }

  if (daysUntilNextEvent < 28) {
    const weeks = Math.max(1, Math.floor(daysUntilNextEvent / 7));
    return weeks === 1
      ? "Próxima corrida em 1 semana"
      : `Próxima corrida em ${weeks} semanas`;
  }

  if (daysUntilNextEvent < 56) {
    return "Próxima corrida em 1 mês";
  }

  const months = Math.max(2, Math.floor(daysUntilNextEvent / 28));
  return `Próxima corrida em ${months} meses`;
}

export function formatSurfaceSeasonLabel(seasonLike) {
  const year = seasonLike?.ano ?? seasonLike?.year ?? null;
  if (year == null) return "-";
  return `Ano ${year}`;
}

export function formatDateTime(isoString) {
  if (!isoString) return "-";
  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) return "-";

  return date.toLocaleDateString("pt-BR", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function difficultyLabel(id) {
  const labels = {
    facil: "Fácil",
    medio: "Médio",
    dificil: "Difícil",
    lendario: "Lendário",
  };
  return labels[id] || id;
}

export function categoryLabel(id) {
  const labels = {
    mazda_rookie: "Mazda MX-5 Rookie Cup",
    toyota_rookie: "Toyota GR86 Rookie Cup",
    mazda_amador: "Mazda MX-5 Championship",
    toyota_amador: "Toyota GR86 Cup",
    bmw_m2: "BMW M2 CS Racing",
    production_challenger: "Production Car Challenger",
    gt4: "GT4 Series",
    gt3: "GT3 Championship",
    lmp2: "LMP2 Prototype Championship",
    endurance: "Endurance Championship",
  };
  return labels[id] || id;
}

export function formatCategoryName(id) {
  return categoryLabel(id);
}

export function getCategoryTier(id) {
  const tiers = {
    mazda_rookie: 1,
    toyota_rookie: 1,
    mazda_amador: 2,
    toyota_amador: 2,
    bmw_m2: 3,
    production_challenger: 3,
    gt4: 4,
    gt3: 5,
    lmp2: 6,
    endurance: 7,
  };
  return tiers[id] || 0;
}

export function formatLicenseLevel(level) {
  if (level === null || level === undefined || level < 0) return "Sem licença";
  
  const levels = {
    0: "Amadora",
    1: "Pro",
    2: "Super Pro",
    3: "Elite",
    4: "Super Elite"
  };
  
  return levels[level] || "Sem licença";
}

export function formatLapTime(ms) {
  if (!ms || ms <= 0) return "-";

  const totalSeconds = ms / 1000;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toFixed(3).padStart(6, "0")}`;
}

export function formatGap(ms) {
  if (!ms || ms <= 0) return "-";
  return `+${(ms / 1000).toFixed(3)}`;
}

export function formatSalary(value) {
  if (!value && value !== 0) return "-";
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(value);
}

export function formatRoleLabel(value) {
  if (value === "Numero1" || value === "N1") return "N1";
  if (value === "Numero2" || value === "N2") return "N2";
  return value || "-";
}

export function formatPreseasonPhase(value) {
  const labels = {
    ContractExpiry: "Contratos",
    Transfers: "Transferências",
    PlayerProposals: "Suas Propostas",
    RookiePlacement: "Rookies",
    Finalization: "Finalização",
    Complete: "Completa",
  };

  return labels[value] || value || "-";
}

export function formatSeasonPhase(value) {
  const labels = {
    BlocoRegular: "Bloco Regular",
    JanelaConvocacao: "Convocação",
    BlocoEspecial: "Bloco Especial",
    PosEspecial: "Pós-Especial",
  };
  return labels[value] || value || "—";
}

export function formatAttributeName(value) {
  const labels = {
    skill: "Skill",
    consistencia: "Consistência",
    racecraft: "Racecraft",
    defesa: "Defesa",
    ritmo_classificacao: "Quali",
    gestao_pneus: "Pneus",
    adaptabilidade: "Adaptabilidade",
    mentalidade: "Mentalidade",
    confianca: "Confiança",
    smoothness: "Smoothness",
    experiencia: "Experiência",
    fitness: "Fitness",
    fator_chuva: "Chuva",
    habilidade_largada: "Largada",
    agressividade: "Agressividade",
    aggression: "Agressividade",
    midia: "Mídia",
    desenvolvimento: "Desenvolvimento",
  };

  const formatted = labels[value] || value;
  return formatted.charAt(0).toUpperCase() + formatted.slice(1);
}

const FLAG_CODE_BY_EMOJI = {
  "\u{1F1E6}\u{1F1F7}": "ar",
  "\u{1F1E6}\u{1F1F9}": "at",
  "\u{1F1E6}\u{1F1FA}": "au",
  "\u{1F1E7}\u{1F1EA}": "be",
  "\u{1F1E7}\u{1F1F7}": "br",
  "\u{1F1E8}\u{1F1E6}": "ca",
  "\u{1F1E8}\u{1F1ED}": "ch",
  "\u{1F1E8}\u{1F1F3}": "cn",
  "\u{1F1E9}\u{1F1EA}": "de",
  "\u{1F1E9}\u{1F1F0}": "dk",
  "\u{1F1EA}\u{1F1F8}": "es",
  "\u{1F1EB}\u{1F1EE}": "fi",
  "\u{1F1EB}\u{1F1F7}": "fr",
  "\u{1F1EC}\u{1F1E7}": "gb",
  "\u{1F1ED}\u{1F1FA}": "hu",
  "\u{1F1EE}\u{1F1F9}": "it",
  "\u{1F1EF}\u{1F1F5}": "jp",
  "\u{1F1F2}\u{1F1FD}": "mx",
  "\u{1F1F3}\u{1F1F1}": "nl",
  "\u{1F1F3}\u{1F1F4}": "no",
  "\u{1F1F5}\u{1F1F1}": "pl",
  "\u{1F1F5}\u{1F1F9}": "pt",
  "\u{1F1F7}\u{1F1FA}": "ru",
  "\u{1F1F8}\u{1F1EA}": "se",
  "\u{1F1FA}\u{1F1F8}": "us",
};

const FLAG_EMOJI_BY_CODE = Object.fromEntries(
  Object.entries(FLAG_CODE_BY_EMOJI).map(([emoji, code]) => [code, emoji]),
);

const FLAG_CODE_BY_LABEL = {
  argentina: "ar",
  austria: "at",
  australia: "au",
  belgica: "be",
  belgium: "be",
  brasil: "br",
  brasileira: "br",
  brasileiro: "br",
  brazil: "br",
  canada: "ca",
  suica: "ch",
  suisse: "ch",
  switzerland: "ch",
  china: "cn",
  alemanha: "de",
  germania: "de",
  germany: "de",
  dinamarca: "dk",
  espanha: "es",
  finlandia: "fi",
  franca: "fr",
  france: "fr",
  reino_unido: "gb",
  inglaterra: "gb",
  gra_bretanha: "gb",
  great_britain: "gb",
  united_kingdom: "gb",
  hungria: "hu",
  italia: "it",
  japao: "jp",
  mexico: "mx",
  holanda: "nl",
  paises_baixos: "nl",
  noruega: "no",
  polonia: "pl",
  portugal: "pt",
  portuguesa: "pt",
  portugues: "pt",
  russia: "ru",
  suecia: "se",
  eua: "us",
  estados_unidos: "us",
  usa: "us",
  united_states: "us",
};

function normalizeNationalityLabel(value) {
  return String(value ?? "")
    .trim()
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

export function extractFlag(nacionalidade) {
  if (!nacionalidade) return "\u{1F3C1}";

  const firstPart = nacionalidade.trim().split(/\s+/)[0] || "";
  if (FLAG_CODE_BY_EMOJI[firstPart]) {
    return firstPart;
  }

  const code = extractNationalityCode(nacionalidade);
  return FLAG_EMOJI_BY_CODE[code] || "\u{1F3C1}";
}

export function extractNationalityLabel(nacionalidade) {
  if (!nacionalidade) return "";
  const parts = nacionalidade.trim().split(/\s+/);

  if (parts.length <= 1) {
    return "";
  }

  const firstPart = parts[0] || "";
  return FLAG_CODE_BY_EMOJI[firstPart] || FLAG_EMOJI_BY_CODE[firstPart.toLowerCase()]
    ? parts.slice(1).join(" ")
    : nacionalidade;
}

export function extractNationalityCode(nacionalidade) {
  if (!nacionalidade) return null;

  const firstPart = nacionalidade.trim().split(/\s+/)[0]?.toLowerCase() || "";
  if (FLAG_EMOJI_BY_CODE[firstPart]) {
    return firstPart;
  }

  return FLAG_CODE_BY_EMOJI[firstPart] || FLAG_CODE_BY_LABEL[normalizeNationalityLabel(nacionalidade)] || null;
}
