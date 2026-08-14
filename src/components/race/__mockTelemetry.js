// i18n-ignore-file — telemetria de mentira, atrás de um flag de dev. É dado de exemplo, e
// dado de exemplo não se traduz.
// Telemetria FAKE para conferir o frontend do cockpit sem precisar correr no iRacing.
// Só usada atrás de um flag de dev no RaceResultViewV2. Determinística (sem random),
// então a tela fica estável entre renders. Inclui `__mockWeather` para a faixa de clima.

const RACE_LAPS = 16;
const NAMES = ["Henrique Barbosa", "David Meier", "Você", "Pablo Sanchez", "Jack Harris", "Caleb Davis"];
const PLAYER_IDX = 2;
const RIVAL_IDX = 3;
const BASE_PACE = [88.6, 88.9, 89.3, 89.1, 89.8, 90.2]; // s/volta por carro

// Ritmo determinístico de um carro numa volta (varia suave + um pico de erro do jogador).
function paceFor(idx, lap) {
  let t = BASE_PACE[idx] + Math.sin((lap + idx) * 1.2) * 0.55 + Math.cos(lap * 0.7 + idx) * 0.22;
  if (idx === PLAYER_IDX && lap === 9) t += 2.4; // "erro mais caro" na volta 9
  return t;
}

function buildCharts() {
  const cars = NAMES.map((name, idx) => ({ idx, name, is_player: idx === PLAYER_IDX, points: [] }));
  const cum = NAMES.map(() => 0);
  for (let lap = 1; lap <= RACE_LAPS; lap++) {
    NAMES.forEach((_, idx) => { cum[idx] += paceFor(idx, lap); });
    const order = NAMES.map((_, idx) => idx).sort((a, b) => cum[a] - cum[b]);
    const leader = cum[order[0]];
    order.forEach((idx, pos) => {
      cars[idx].points.push({ lap, position: pos + 1, gap: cum[idx] - leader });
    });
  }

  const lap_times = [];
  for (let lap = 1; lap <= RACE_LAPS; lap++) lap_times.push({ lap, time_s: paceFor(PLAYER_IDX, lap) });

  // Gap pro rival (jogador − rival, em s): >0 rival à frente, <0 você à frente.
  const rival_gap = [];
  for (let lap = 1; lap <= RACE_LAPS; lap++) {
    const me = cars[PLAYER_IDX].points[lap - 1];
    const ri = cars[RIVAL_IDX].points[lap - 1];
    rival_gap.push({ lap, gap_s: me.gap - ri.gap });
  }

  return {
    cars,
    lap_times,
    car_lap_times: [],
    rival_gap,
    rival_name: NAMES[RIVAL_IDX],
    yellow_laps: [9, 10],
  };
}

// Estratégias de pneu: corrida seca que molha na 2ª metade → trocas pra Wet, e uma
// parada só de combustível (mostra o ícone ⛽). Equipe e nome preenchidos.
const TIRE_STRATEGIES = [
  {
    car_idx: 0, pilot_name: "Henrique Barbosa", team_name: "Sunday Speed Club",
    start_compound: "Dry", tire_changes: 1, wrong_tire: false,
    stints: [
      { from_lap: 1, compound: "Dry", changed: false, confidence: 1 },
      { from_lap: 7, compound: "Wet", changed: true, confidence: 0.9 },
    ],
    stops: [{ lap: 7, box_secs: 23.1, tire_change: true, track_wet: true }],
    summary: "Trocou para chuva na volta 7.",
  },
  {
    car_idx: PLAYER_IDX, pilot_name: "Você", team_name: "Thunderline Academy",
    start_compound: "Dry", tire_changes: 1, wrong_tire: false,
    stints: [
      { from_lap: 1, compound: "Dry", changed: false, confidence: 1 },
      { from_lap: 8, compound: "Wet", changed: true, confidence: 0.95 },
    ],
    stops: [
      { lap: 8, box_secs: 24.3, tire_change: true, track_wet: true },
      { lap: 12, box_secs: 6.8, tire_change: false, track_wet: true },
    ],
    summary: "Chuva na volta 8 + reabastecimento na 12.",
  },
  {
    car_idx: RIVAL_IDX, pilot_name: "Pablo Sanchez", team_name: "Track Day Heroes",
    start_compound: "Dry", tire_changes: 1, wrong_tire: false,
    stints: [
      { from_lap: 1, compound: "Dry", changed: false, confidence: 1 },
      { from_lap: 9, compound: "Wet", changed: true, confidence: 0.9 },
    ],
    stops: [{ lap: 9, box_secs: 25.0, tire_change: true, track_wet: true }],
    summary: "Demorou pra trocar — apostou no seco.",
  },
  {
    car_idx: 4, pilot_name: "Jack Harris", team_name: "Grid Start Racing School",
    start_compound: "Dry", tire_changes: 0, wrong_tire: true,
    stints: [{ from_lap: 1, compound: "Dry", changed: false, confidence: 1 }],
    stops: [{ lap: 10, box_secs: 7.2, tire_change: false, track_wet: true }],
    summary: "Só combustível — ficou no seco na chuva (aposta furada).",
  },
];

const PLAYER_TIRE = TIRE_STRATEGIES.find((s) => s.car_idx === PLAYER_IDX);

// Clima FAKE para a faixa (mesmo shape do get_race_weather_timeline): seca que molha.
const MOCK_WEATHER = {
  scenario: "Pista seca que molha na 2ª metade",
  intensity: "CHUVA",
  is_wet_race: true,
  points: [
    { frac: 0.0, event_type: 0 },
    { frac: 0.35, event_type: 2 },
    { frac: 0.45, event_type: 3 },
    { frac: 0.55, event_type: 6 },
    { frac: 0.62, event_type: 7 },
    { frac: 1.0, event_type: 7 },
  ],
};

export const MOCK_TELEMETRY = {
  has_telemetry: true,
  laps_seen: RACE_LAPS,
  race_laps: RACE_LAPS,
  last_lap_seen: RACE_LAPS,
  confidence: "alta",
  is_partial: false,
  pace: null,
  rival: null,
  position_flow: null,
  mistake: { lap: 9, confidence: "media" },
  best_moment: { lap: 4, confidence: "alta" },
  charts: buildCharts(),
  tire_strategies: TIRE_STRATEGIES,
  player_tire: PLAYER_TIRE,
  __mockWeather: MOCK_WEATHER,
  // DEV: companheiro de equipe fake (pra o Gap abrir nele por padrão).
  __mockTeammateName: "David Meier",
};

// ───────────────────────── Quebras de peça FAKE (Fase 7) ─────────────────────────

/// Receita das quebras fake: uma por estado visual que a UI sabe desenhar. A ordem importa —
/// é a prioridade com que os papéis são preenchidos a partir do grid REAL da tela.
const FAKE_BREAKDOWNS = [
  {
    role: "player",
    part: "gearbox",
    part_name: "Câmbio",
    lap: 11,
    severity: "heavy",
    penalty_secs: 17,
    label: "câmbio perdeu a 3ª marcha",
  },
  {
    // Segunda peça do MESMO piloto: exercita o tooltip de várias linhas e o chip com duas
    // peças separadas por vírgula.
    role: "player",
    part: "brakes",
    part_name: "Freios",
    lap: 14,
    severity: "light",
    penalty_secs: 4,
    label: "freio dianteiro perdendo mordida",
  },
  {
    role: "dnf",
    part: "engine",
    part_name: "Motor",
    lap: 8,
    severity: "dnf",
    penalty_secs: null,
    label: "motor fundiu por superaquecimento",
  },
  {
    role: "other",
    part: "suspension",
    part_name: "Suspensão",
    lap: 6,
    severity: "heavy",
    penalty_secs: 12,
    label: "braço de suspensão trincado",
  },
  {
    role: "other",
    part: "electronics",
    part_name: "Eletrônica",
    lap: 3,
    severity: "light",
    penalty_secs: 3,
    label: "chicote molhado falhando intermitente",
  },
];

/// Monta quebras fake a partir do grid REAL que está na tela.
///
/// Derivar dos resultados de verdade (em vez de uma lista de nomes chumbada) é o que faz o 🔧
/// acender na linha certa da tabela e a métrica "perdido no box" somar — as duas casam por
/// `driver_id`/`pilot_id`. Com nomes inventados, a tela mostraria os chips e mais nada.
///
/// Determinístico: mesmo resultado → mesmas quebras, sem random, então a tela não pisca a cada
/// render. Só DEV: chamado atrás do flag de dados fake do RaceResultViewV2.
export function buildMockBreakdowns(raceResults) {
  const rows = Array.isArray(raceResults) ? raceResults : [];
  if (rows.length === 0) return [];

  const player = rows.find((r) => r.is_jogador) ?? null;
  const retired = rows.find((r) => r.is_dnf && !r.is_jogador) ?? null;
  // Preenche os papéis "other" com quem sobrou, do fim do grid pra frente (quem quebrou tende
  // a estar lá atrás) e sem repetir quem já foi escalado.
  const usados = new Set([player?.pilot_id, retired?.pilot_id].filter(Boolean));
  const outros = rows
    .filter((r) => !usados.has(r.pilot_id))
    .slice(-2)
    .reverse();

  let proximoOutro = 0;
  const out = [];
  for (const receita of FAKE_BREAKDOWNS) {
    let alvo = null;
    if (receita.role === "player") alvo = player;
    else if (receita.role === "dnf") alvo = retired;
    else alvo = outros[proximoOutro++] ?? null;
    // Grid sem jogador, sem abandono ou sem gente sobrando: o papel simplesmente não aparece.
    if (!alvo) continue;
    out.push({
      driver_id: alvo.pilot_id,
      driver_name: alvo.pilot_name,
      part: receita.part,
      part_name: receita.part_name,
      lap: receita.lap,
      severity: receita.severity,
      penalty_secs: receita.penalty_secs,
      label: receita.label,
      is_player: !!alvo.is_jogador,
    });
  }
  return out.sort((a, b) => a.lap - b.lap);
}
