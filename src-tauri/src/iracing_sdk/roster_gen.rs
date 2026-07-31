//! Geração de **AI roster** do iRacing a partir do grid da carreira.
//!
//! Converte nossos pilotos (com seus times) no JSON que o iRacing lê em
//! `Documentos/iRacing/airosters/<nome>/roster.json`. Como o `driverName` vira o
//! `UserName` que o SDK devolve, o pós-corrida casa IA→nosso piloto pelo nome.
//!
//! Regras de aparência (confirmadas com o usuário):
//! - **Carro**: padrão por TIME (sorteado do pool aprovado), cor do time. O time
//!   do jogador usa o padrão mais simples (0). Padrões variam entre times.
//! - **Macacão**: padrão por TIME (igual para os dois pilotos), cor do time.
//! - **Capacete**: padrão por PILOTO (varia entre companheiros), cor do time.
//! - Companheiros se diferenciam por **número** (fixo na temporada) + **sponsors**.
//!
//! Mapeamento de atributos (`docs/iracing/ai-roster-format.md`):
//! skill→driverSkill, aggression→driverAggression, confianca→driverOptimism,
//! smoothness→driverSmoothness, idade→driverAge, team.pit_crew_quality→pitCrewSkill,
//! team.pit_strategy_risk→strategyRiskiness.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use serde::Serialize;

use crate::models::driver::Driver;

/// Padrão "simples" do carro (mais perto do carro sólido do jogador).
const SIMPLE_CAR_PATTERN: i64 = 0;
/// Pools de padrão de macacão/capacete (valores válidos vistos no roster real;
/// o usuário aceitou "qualquer estilo" para esses).
const SUIT_PATTERNS: &[i64] = &[
    1, 2, 6, 7, 9, 10, 11, 12, 14, 15, 19, 20, 22, 23, 24, 26, 28, 33, 35,
];
const HELMET_PATTERNS: &[i64] = &[
    2, 4, 8, 9, 10, 11, 14, 15, 20, 26, 29, 33, 45, 48, 50, 52, 60, 62, 64, 68,
];

/// O que o roster precisa do time (extraído de `Team` pelo chamador).
pub struct TeamInfo {
    /// Id do time — agrupa companheiros e dá o padrão estável do time.
    pub team_id: String,
    /// Cor primária do time (`cor_primaria`), com ou sem `#`.
    pub color: String,
    /// Cor secundária do time (`cor_secundaria`).
    pub color2: String,
    pub pit_crew: f64,
    pub strategy: f64,
    /// Se é o time do JOGADOR (usa o padrão simples no carro).
    pub is_player_team: bool,
}

/// Carro do conteúdo grátis + repertório de pintura aprovado (espelha
/// `docs/iracing/free-content.json`).
pub struct CarSpec {
    pub car_path: &'static str,
    pub car_id: i64,
    pub car_class_id: i64,
    /// Padrões de pintura de carro aprovados (curados pelo usuário).
    pub patterns: &'static [i64],
    /// Pool de IDs de sponsor do iRacing.
    pub sponsors: &'static [i64],
}

/// Resolve um dos carros grátis (com seus pools) por chave curta.
pub fn car_spec(key: &str) -> Option<CarSpec> {
    Some(match key {
        "mx5" => CarSpec {
            car_path: "mx5\\mx52016",
            car_id: 67,
            car_class_id: 74,
            patterns: &[0, 4, 5, 8, 13],
            sponsors: &[367, 11, 1],
        },
        "gr86" => CarSpec {
            car_path: "toyotagr86",
            car_id: 160,
            car_class_id: 4012,
            patterns: &[0, 2, 4, 5, 7, 9, 11, 14, 16, 21],
            sponsors: &[434, 413, 2, 130, 410, 3, 72],
        },
        "bmwm2" => CarSpec {
            car_path: "bmwm2g87",
            car_id: 216,
            car_class_id: 4108,
            patterns: &[0, 3, 5, 6, 13, 19, 23],
            sponsors: &[7, 410, 362, 363, 2, 324, 290, 411, 486, 253],
        },
        _ => return None,
    })
}

/// Arquivo de roster (raiz do JSON).
#[derive(Serialize)]
pub struct RosterFile {
    pub drivers: Vec<RosterDriver>,
}

/// Uma entrada de piloto no roster, com as chaves camelCase do iRacing.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterDriver {
    pub driver_name: String,
    pub car_number: String,
    pub car_design: String,
    pub suit_design: String,
    pub helmet_design: String,
    pub car_path: String,
    pub car_id: i64,
    pub car_class_id: i64,
    pub sponsor1: i64,
    pub sponsor2: i64,
    pub number_design: String,
    pub driver_skill: i64,
    pub driver_aggression: i64,
    pub driver_optimism: i64,
    pub driver_smoothness: i64,
    pub pit_crew_skill: i64,
    pub strategy_riskiness: i64,
    pub driver_age: i64,
    pub id: String,
    pub row_index: i64,
}

/// Estilo de número padrão (do roster curado do usuário).
const NUMBER_DESIGN: &str = "0,0,FFFFFF,777777,000000";

/// Esquema embutido do JOGADOR (Opção A — aplicação manual): padrão simples (0)
/// + cor do time. Mesmas cores secundárias do design da IA.
pub const DESIGN_PATTERN: &str = "0";
pub const DESIGN_COLOR2: &str = "000000";
pub const DESIGN_COLOR3: &str = "FFFFFF";

/// Normaliza uma cor `#RRGGBB`/`RRGGBB` → `RRGGBB` maiúsculo; fallback branco.
/// Público para reuso na pintura do jogador (mesmo tratamento da IA).
pub fn normalize_hex(color: &str) -> String {
    let s = color.trim().trim_start_matches('#');
    if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        s.to_uppercase()
    } else {
        "FFFFFF".to_string()
    }
}

/// 0–100 `f64` → inteiro do roster, dentro da faixa válida.
fn attr(value: f64) -> i64 {
    value.round().clamp(0.0, 100.0) as i64
}

// ─── CURVA DE SKILL EM DOIS TRECHOS ──────────────────────────────────────────
// O iRacing estica linearmente o grid pra preencher [min_skill, max_skill]. Isso
// AMPLIFICA todo gap real pelo fator (banda/span). Num grid apertado (rookie: todos
// ~30–49) a banda larga inflava os gaps ~2,6× — 3 pontos reais viravam ~8 efetivos e o
// líder abria 15s. A curva conserta em DOIS trechos:
//   • TOPO (os melhores, fração `SKILL_CURVE_FRONT_FRAC`): gaps FIÉIS (k_top≈1 → o líder
//     abre só o que a skill real dele diz; briga limpa na frente).
//   • CAUDA (do "corte" pra baixo): gaps ESTICADOS (k_bottom>1) pra AFUNDAR o fundo de
//     grid — no rookie o piloto ~30 tem que ser GENUINAMENTE ruim, não só um pouco atrás.
// A banda da season re-ancora essa FORMA no sweet spot do tier, então o roster só precisa
// do formato (lo/hi/corte/k) e a season passa o `top_anchor` = sweet spot real.

/// Fração do topo do grid (por skill) que mantém gaps FIÉIS. 0,36 ≈ top-4 num grid de 11
/// (rookie) → "do 5º pra baixo estica". Escala com o tamanho do grid.
pub const SKILL_CURVE_FRONT_FRAC: f64 = 0.36;
/// Amplificação DESEJADA da cauda (>1 afunda o fundo de grid). É um TETO: a curva reduz
/// automaticamente se esse valor faria o pior piloto cair ABAIXO do próprio skill real
/// (ver o cap em `skill_curve_from`). Subir = fundo mais afundado nos grids apertados.
pub const SKILL_CURVE_K_BOTTOM: f64 = 3.0;

/// Parâmetros da curva (derivados das skills da IA + o sweet spot do tier).
pub struct SkillCurve {
    /// Maior skill real da IA no grid (o melhor → `top_anchor`).
    pub hi: f64,
    /// Skill do último piloto FIEL (sorted desc). Abaixo dele, a cauda estica.
    pub boundary: f64,
    /// Skill efetiva do melhor da IA = sweet spot do tier. Season e roster passam o MESMO
    /// valor pra a curva (e o cabo da cauda) baterem dos dois lados.
    pub top_anchor: f64,
    /// Inclinação do TOPO. `min(1, (sweet − lo)/(hi − lo))`: quando o campo é mais APERTADO
    /// que a banda (rookie: sweet acima das skills), trava em 1 (fiel, sem amplificar o
    /// líder); quando é mais LARGO (elite: sweet abaixo do topo real), comprime (<1) pra o
    /// pelotão caber perto do sweet — ex.: 98→80, 90→77. Auto, sem "if de tier".
    pub k_top: f64,
    /// Amplificação EFETIVA da cauda, já CAPADA pra o pior piloto não cair abaixo do skill
    /// real dele (o piso é o menor skill do grid). Rookie (grid apertado, sweet alto) afunda
    /// o fundo; elite (grid largo, sweet baixo) vira ~linear com o topo.
    pub k_bottom: f64,
    /// Menor skill real da IA — o piso do grid.
    pub lo: f64,
}

/// Monta a curva a partir das skills da IA (só IA — o jogador não entra na banda) e do
/// sweet spot do tier (valor efetivo do melhor). `k_top` comprime o topo quando o sweet
/// fica abaixo do topo real; `k_bottom` é limitado pra o PIOR piloto aterrissar no máximo
/// no próprio skill real, nunca abaixo.
pub fn skill_curve_from(ai_skills: &[f64], top_anchor: f64) -> SkillCurve {
    let hi = ai_skills.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let lo = ai_skills.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = if hi.is_finite() { hi } else { top_anchor };
    let lo = if lo.is_finite() { lo } else { top_anchor };
    let mut sorted: Vec<f64> = ai_skills.to_vec();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let boundary = if n <= 1 {
        lo
    } else {
        let front = ((n as f64) * SKILL_CURVE_FRONT_FRAC).round().max(1.0) as usize;
        sorted[front.min(n) - 1]
    };
    // Inclinação do topo: nunca amplifica (cap em 1). Quando o sweet < topo real, comprime
    // pra caber o campo na banda [lo, sweet] (melhor→sweet, pior→lo, ~linear).
    let real_span = hi - lo;
    let k_top = if real_span > 0.0 {
        ((top_anchor - lo) / real_span).min(1.0).max(0.0)
    } else {
        1.0
    };
    // Efetivo do corte (fim do trecho fiel). O k_bottom máximo é o que leva o pior piloto
    // EXATAMENTE ao skill real dele (piso = lo): k_cap = (corte_efetivo − lo)/(corte − lo).
    let boundary_eff = top_anchor - k_top * (hi - boundary);
    let tail_real = boundary - lo;
    let k_bottom = if tail_real > 0.0 {
        let cap = (boundary_eff - lo) / tail_real;
        SKILL_CURVE_K_BOTTOM.min(cap).max(0.0)
    } else {
        SKILL_CURVE_K_BOTTOM
    };
    SkillCurve {
        hi,
        boundary,
        top_anchor,
        k_top,
        k_bottom,
        lo,
    }
}

/// Skill efetiva pela curva de 2 trechos. Melhor da IA → `top_anchor`; cada ponto real
/// abaixo custa `k_top` até o corte e `k_bottom` (capado) na cauda.
pub fn skill_curve(real: f64, c: &SkillCurve) -> f64 {
    if real >= c.boundary {
        c.top_anchor - c.k_top * (c.hi - real)
    } else {
        c.top_anchor - c.k_top * (c.hi - c.boundary) - c.k_bottom * (c.boundary - real)
    }
}

/// Escolhe deterministicamente um item do pool a partir de uma semente estável
/// (id de time/piloto) — mantém a aparência consistente ao longo da temporada.
fn pick(pool: &[i64], seed: &str) -> i64 {
    if pool.is_empty() {
        return 0;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    pool[(hasher.finish() % pool.len() as u64) as usize]
}

/// Dados por piloto do Tier 2 Batch B (o comando preenche do banco).
#[derive(Clone, Copy)]
pub struct DriverCtx {
    pub contract_last_year: bool,
    pub teammate_points: Option<f64>,
    /// +1 promovido (subiu de categoria), -1 rebaixado (caiu), 0 nada.
    pub category_move: i32,
    pub team_morale: f64,
    pub injury_return: bool,
    pub honeymoon: bool,
    pub crashed_out_last_race: bool,
    pub not_at_fault_dnfs: u32,
    pub track_crash: bool,
    /// Cruzou a linha lado a lado com o mesmo rival em ≥2 das últimas corridas.
    pub nemesis: bool,
    /// Trocou de equipe nesta virada de temporada (tinha outro time antes).
    pub switched_teams: bool,
    /// Campeão da categoria na temporada passada.
    pub reigning_champion: bool,
    /// Primeira corrida da carreira (nunca largou).
    pub career_debut: bool,
    /// DNFs mecânicos (Mechanical/Operational) nas últimas corridas.
    pub mechanical_dnfs: u32,
    /// Nível do vínculo com a equipe atual (1–6; ver [`crate::market::bond::bond_level`]).
    pub bond_level: u8,
    /// Handicap de lesão ATIVA: fração do pace perdida por uma lesão em recuperação
    /// (`skill_penalty × corridas_restantes/total`, 0–1). 0 = sem lesão ativa.
    pub injury_active_penalty: f64,
}

/// Contexto compartilhado da corrida-alvo para a camada de comportamento por corrida
/// (ver [`crate::iracing_sdk::behavior`]). O comando preenche do banco; aqui só usamos.
pub struct BehaviorContext {
    pub current_season: i32,
    pub track_id: u32,
    pub track_length_km: f64,
    /// `pais` (bandeira) da pista — comparado à nacionalidade do piloto (casa).
    pub track_flag: String,
    /// Pontos de TODOS da categoria (inclui o jogador) — base do cálculo do título.
    pub title_points: Vec<f64>,
    pub races_left: u32,
    /// Interesse "de local" do evento (0..1) — pressão de casa cheia (universal).
    pub event_stakes: f64,
    /// Total de corridas da temporada (p/ desgaste de fim de temporada).
    pub season_length: u32,
    /// Pontos do vencedor (P1 + volta rápida).
    pub max_points: f64,
    pub field_size: u32,
    /// Skills de todos do grid — para o percentil de domínio no grid.
    pub grid_skills: Vec<f64>,
    pub is_wet: bool,
    pub rain_intensity: f64,
    /// Intensidade da chuva (enum) — para o re-rank de skill por piloto na chuva.
    pub rain_level: crate::iracing_sdk::weather::RainIntensity,
    pub temp_c: f64,
    /// Semente do evento (career_id + event_id) — varia o "humor do dia" por piloto.
    pub seed_base: u64,
    /// id do piloto → últimas posições finais (forma).
    pub recent_positions: HashMap<String, Vec<u32>>,
    /// id do piloto → percentil no ranking mundial (0–1).
    pub global_percentile: HashMap<String, f64>,
    /// id do piloto → dados do Tier 2 Batch B.
    pub driver_ctx: HashMap<String, DriverCtx>,
    /// Sweet spot do tier (efetivo do melhor da IA) na pista alvo — âncora da curva de
    /// skill. MESMO valor que a season usa no `max_skill` (pré-chuva), pra a forma bater
    /// dos dois lados. Já com o delta da BANDA do carro (você vs a média do campo).
    pub ai_sweet_spot: f64,
    /// id do piloto → **spread por-IA do carro** (skill, zero-mean): quanto o carro daquela
    /// IA desvia da média do campo na pista alvo (Sistema de Nível do Carro → export). Somado
    /// ao skill final; junto do `ai_sweet_spot` já rebaixado, reconstrói `adv(IA) − adv(você)`.
    /// Ver [`crate::iracing_sdk::car_difficulty`].
    pub car_spread_nudge: HashMap<String, f64>,
    /// id do piloto → **bônus de rivalidade** (skill) contra o JOGADOR: Nemesis +2,
    /// Rivais +1. Faz o rival correr mais forte na pista do jogador (Pressão de Duelo,
    /// lado export). Vazio quando não há rivalidade de interesse.
    pub rival_skill_bonus: HashMap<String, f64>,
}

/// Percentil de skill dentro do grid (0 pior … 1 melhor).
fn grid_percentile(skill: f64, grid: &[f64]) -> f64 {
    if grid.len() <= 1 {
        return 0.5;
    }
    let at_or_below = grid.iter().filter(|&&s| s <= skill).count() as f64;
    (at_or_below - 1.0).max(0.0) / (grid.len() as f64 - 1.0)
}

/// Mesma bandeira (2 chars de indicador regional) = corrida em casa.
fn same_flag(a: &str, b: &str) -> bool {
    !a.is_empty() && a.chars().take(2).eq(b.chars().take(2))
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in s.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Monta o roster a partir do grid (pilotos + time de cada um) e do mapa de números
/// fixos por piloto. `behavior` (None = atributos crus) aplica a camada de
/// comportamento por corrida. `id_factory` gera o GUID de cada entrada.
pub fn build_roster(
    entries: &[(Driver, Option<TeamInfo>)],
    car: &CarSpec,
    numbers: &HashMap<String, i64>,
    behavior: Option<&BehaviorContext>,
    mut id_factory: impl FnMut() -> String,
) -> RosterFile {
    // Ordena por pontos só para o rowIndex (ordem de exibição no editor).
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| {
        entries[b]
            .0
            .stats_temporada
            .pontos
            .partial_cmp(&entries[a].0.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Curva de skill em 2 trechos (topo fiel + cauda com cap no piso real). Derivada das
    // skills da IA do próprio grid e ancorada no MESMO sweet spot que a season usa — assim
    // o cap da cauda (pior piloto ≥ skill real dele) bate exatamente dos dois lados. Sem
    // behavior (testes) cai num sweet spot neutro (100).
    let ai_skills: Vec<f64> = entries.iter().map(|(d, _)| d.atributos.skill).collect();
    let sweet = behavior.map(|bc| bc.ai_sweet_spot).unwrap_or(100.0);
    let curve = skill_curve_from(&ai_skills, sweet);

    let drivers = order
        .iter()
        .enumerate()
        .map(|(row, &i)| {
            let (driver, team) = &entries[i];

            let c1 = team
                .as_ref()
                .map(|t| normalize_hex(&t.color))
                .unwrap_or_else(|| "FFFFFF".to_string());
            let c2 = team
                .as_ref()
                .map(|t| normalize_hex(&t.color2))
                .unwrap_or_else(|| "000000".to_string());
            let team_id = team.as_ref().map(|t| t.team_id.as_str()).unwrap_or("none");
            let is_player_team = team.as_ref().map(|t| t.is_player_team).unwrap_or(false);
            let pit_crew = team.as_ref().map(|t| t.pit_crew).unwrap_or(50.0);
            let strategy = team.as_ref().map(|t| t.strategy).unwrap_or(50.0);

            // Carro: padrão por time (0 para o time do jogador), cor do time.
            let car_pattern = if is_player_team {
                SIMPLE_CAR_PATTERN
            } else {
                pick(car.patterns, team_id)
            };
            // Macacão: padrão por time (igual para os dois). Capacete: por piloto.
            let suit_pattern = pick(SUIT_PATTERNS, &format!("{team_id}|suit"));
            let helmet_pattern = pick(HELMET_PATTERNS, &driver.id);

            let design = |pattern: i64| format!("{pattern},{c1},{c2},FFFFFF");

            let number = numbers.get(&driver.id).copied().unwrap_or((row + 1) as i64);
            let sponsor1 = pick(car.sponsors, &format!("{}|s1", driver.id));
            let sponsor2 = pick(car.sponsors, &format!("{}|s2", driver.id));

            let a = &driver.atributos;
            // Camada de comportamento por corrida (só no export): o skill quase não
            // se move (já com a penalidade de conhecimento de pista), mas os atributos
            // secundários variam MUITO conforme o contexto. None = atributos crus.
            // Skill de referência do piloto já pela CURVA de 2 trechos (topo fiel, cauda
            // afundada). É a base pra tudo: sem behavior sai direto, com behavior recebe a
            // penalidade de pista + os nudges por cima.
            let curved_skill = skill_curve(a.skill, &curve);
            let (d_skill, d_aggression, d_optimism, d_smoothness) = match behavior {
                None => (curved_skill, a.aggression, a.confianca, a.smoothness),
                Some(bc) => {
                    use crate::simulation::{pressure, track_knowledge};
                    let knowledge = track_knowledge::from_history(
                        &driver.historico_circuitos,
                        bc.track_id as i64,
                    );
                    let track_pen = track_knowledge::track_knowledge_penalty(
                        &knowledge,
                        bc.track_length_km,
                        a.adaptabilidade,
                        bc.current_season,
                    );
                    let title = pressure::title_context(
                        driver.stats_temporada.pontos,
                        &bc.title_points,
                        bc.races_left,
                        bc.max_points,
                    );
                    let dctx = bc.driver_ctx.get(&driver.id);
                    let inputs = crate::iracing_sdk::behavior::BehaviorInputs {
                        base_aggression: a.aggression,
                        base_optimism: a.confianca,
                        base_smoothness: a.smoothness,
                        base_skill: curved_skill - track_pen,
                        mentality: a.mentalidade,
                        resilience: pressure::pressure_resilience(a.mentalidade, a.experiencia),
                        title,
                        races_left: bc.races_left,
                        event_stakes: bc.event_stakes,
                        recent_positions: bc
                            .recent_positions
                            .get(&driver.id)
                            .cloned()
                            .unwrap_or_default(),
                        field_size: bc.field_size,
                        season_length: bc.season_length,
                        track: knowledge,
                        is_wet: bc.is_wet,
                        fator_chuva: a.fator_chuva,
                        rain_intensity: bc.rain_intensity,
                        temp_c: bc.temp_c,
                        age: driver.idade,
                        global_rank_percentile: bc
                            .global_percentile
                            .get(&driver.id)
                            .copied()
                            .unwrap_or(0.5),
                        grid_rank_percentile: grid_percentile(a.skill, &bc.grid_skills),
                        home_race: same_flag(&driver.nacionalidade, &bc.track_flag),
                        career_wins: driver.stats_carreira.vitorias,
                        season_points: driver.stats_temporada.pontos,
                        contract_last_year: dctx.map(|d| d.contract_last_year).unwrap_or(false),
                        teammate_points: dctx.and_then(|d| d.teammate_points),
                        category_move: dctx.map(|d| d.category_move).unwrap_or(0),
                        team_morale: dctx.map(|d| d.team_morale).unwrap_or(1.0),
                        all_points: bc.title_points.clone(),
                        max_points: bc.max_points,
                        injury_return: dctx.map(|d| d.injury_return).unwrap_or(false),
                        honeymoon: dctx.map(|d| d.honeymoon).unwrap_or(false),
                        crashed_out_last_race: dctx
                            .map(|d| d.crashed_out_last_race)
                            .unwrap_or(false),
                        not_at_fault_dnfs: dctx.map(|d| d.not_at_fault_dnfs).unwrap_or(0),
                        track_crash: dctx.map(|d| d.track_crash).unwrap_or(false),
                        nemesis: dctx.map(|d| d.nemesis).unwrap_or(false),
                        switched_teams: dctx.map(|d| d.switched_teams).unwrap_or(false),
                        reigning_champion: dctx.map(|d| d.reigning_champion).unwrap_or(false),
                        career_debut: dctx.map(|d| d.career_debut).unwrap_or(false),
                        mechanical_dnfs: dctx.map(|d| d.mechanical_dnfs).unwrap_or(0),
                        // Fama = `midia` (2ª moeda), já no piloto. Vínculo/lesão-ativa vêm do
                        // `DriverCtx` (o comando busca no banco). São EXPORTS de valores já
                        // calculados — não recomputamos fama/bond/lesão aqui.
                        fame: a.midia,
                        bond_level: dctx.map(|d| d.bond_level).unwrap_or(1),
                        injury_active_penalty: dctx.map(|d| d.injury_active_penalty).unwrap_or(0.0),
                        seed: bc.seed_base ^ fnv1a(&driver.id),
                    };
                    let out = crate::iracing_sdk::behavior::compute(&inputs);
                    // RE-RANK POR PILOTO NA CHUVA (opção B): desvio da média (fator 50).
                    // A penalidade na BANDA (min/max skill, na season) já baixa o pelotão
                    // todo pelo valor médio (pace absoluto cai); aqui só o DESVIO: quem é
                    // bom de chuva sofre MENOS que a média (sobe), quem é ruim sofre MAIS
                    // (cai). Soma 0 no piloto médio → não mexe no pace absoluto.
                    let wet_rerank = if bc.is_wet {
                        use crate::iracing_sdk::weather::rain_skill_penalty;
                        (rain_skill_penalty(50.0, bc.rain_level)
                            - rain_skill_penalty(a.fator_chuva, bc.rain_level))
                            as f64
                    } else {
                        0.0
                    };
                    // SPREAD POR-IA DO CARRO (Sistema de Nível do Carro → export): quanto o
                    // carro desta IA desvia da média do campo na pista. Zero-mean, cavalga o
                    // roster como o re-rank de chuva; o delta da BANDA (você vs campo) já foi
                    // aplicado no `ai_sweet_spot`. Carro ausente → 0.
                    let car_spread = bc.car_spread_nudge.get(&driver.id).copied().unwrap_or(0.0);
                    // Pressão de Duelo (export): o rival do jogador rende mais contra ele.
                    let rival_bonus = bc.rival_skill_bonus.get(&driver.id).copied().unwrap_or(0.0);
                    (
                        out.skill + wet_rerank + car_spread + rival_bonus,
                        out.aggression,
                        out.optimism,
                        out.smoothness,
                    )
                }
            };
            RosterDriver {
                driver_name: driver.nome.clone(),
                car_number: number.to_string(),
                car_design: design(car_pattern),
                suit_design: design(suit_pattern),
                helmet_design: design(helmet_pattern),
                car_path: car.car_path.to_string(),
                car_id: car.car_id,
                car_class_id: car.car_class_id,
                sponsor1,
                sponsor2,
                number_design: NUMBER_DESIGN.to_string(),
                driver_skill: attr(d_skill),
                driver_aggression: attr(d_aggression),
                driver_optimism: attr(d_optimism),
                driver_smoothness: attr(d_smoothness),
                pit_crew_skill: attr(pit_crew),
                strategy_riskiness: attr(strategy),
                driver_age: driver.idade as i64,
                id: id_factory(),
                row_index: row as i64,
            }
        })
        .collect();

    RosterFile { drivers }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driver(id: &str, nome: &str, pontos: f64, skill: f64, confianca: f64) -> Driver {
        let mut d = Driver::new(
            id.to_string(),
            nome.to_string(),
            "BR".to_string(),
            "M".to_string(),
            25,
            2024,
        );
        d.atributos.skill = skill;
        d.atributos.confianca = confianca;
        d.stats_temporada.pontos = pontos;
        d
    }

    fn team(id: &str, cor: &str, cor2: &str, player: bool) -> TeamInfo {
        TeamInfo {
            team_id: id.to_string(),
            color: cor.to_string(),
            color2: cor2.to_string(),
            pit_crew: 50.0,
            strategy: 50.0,
            is_player_team: player,
        }
    }

    #[test]
    fn time_do_jogador_usa_padrao_simples_e_companheiros_compartilham() {
        let mut numbers = HashMap::new();
        numbers.insert("D-ana".to_string(), 7);
        numbers.insert("D-bia".to_string(), 9);
        let entries = vec![
            // Mesmo time do jogador: ambos padrão 0, mesma cor.
            (
                driver("D-ana", "Ana", 10.0, 40.0, 30.0),
                Some(team("T1", "#e63946", "#000000", true)),
            ),
            (
                driver("D-bia", "Bia", 50.0, 80.0, 90.0),
                Some(team("T1", "#e63946", "#000000", true)),
            ),
        ];
        let car = car_spec("mx5").unwrap();
        let mut n = 0;
        let roster = build_roster(&entries, &car, &numbers, None, || {
            n += 1;
            format!("ID-{n}")
        });

        // Número fixo do mapa (não a posição).
        let ana = roster
            .drivers
            .iter()
            .find(|d| d.driver_name == "Ana")
            .unwrap();
        let bia = roster
            .drivers
            .iter()
            .find(|d| d.driver_name == "Bia")
            .unwrap();
        assert_eq!(ana.car_number, "7");
        assert_eq!(bia.car_number, "9");
        // Time do jogador → padrão de carro 0 para os dois.
        assert!(ana.car_design.starts_with("0,"));
        assert!(bia.car_design.starts_with("0,"));
        // Mesma cor (E63946) nos dois.
        assert!(ana.car_design.contains("E63946"));
        assert!(bia.car_design.contains("E63946"));
        // optimism ← confianca (não passa pela curva de skill).
        assert_eq!(bia.driver_optimism, 90);
        // skill passa pela curva de 2 trechos: Bia é a melhor do grid → topo (100 no
        // roster; a banda da season re-ancora no sweet spot do tier).
        assert_eq!(bia.driver_skill, 100);
    }

    #[test]
    fn curva_de_skill_topo_fiel_e_cauda_esticada() {
        // Grid rookie real: apertado (30..49). Ancorado no sweet spot 82.
        let ai = [
            49.0, 45.0, 43.0, 43.0, 40.0, 40.0, 38.0, 36.0, 35.0, 32.0, 30.0,
        ];
        let c = skill_curve_from(&ai, 82.0);
        assert_eq!(c.hi, 49.0);
        assert_eq!(c.boundary, 43.0); // top-4 fiel (round(11*0.36)=4)

        // Topo: gaps FIÉIS (1 ponto real = 1 ponto efetivo).
        assert!((skill_curve(49.0, &c) - 82.0).abs() < 1e-9); // melhor → sweet spot
        assert!((skill_curve(45.0, &c) - 78.0).abs() < 1e-9); // 4 reais abaixo → 4 abaixo
        assert!((skill_curve(43.0, &c) - 76.0).abs() < 1e-9); // no corte

        // Cauda: gaps ESTICADOS (k_bottom=3 afunda o fundo).
        assert!((skill_curve(40.0, &c) - 67.0).abs() < 1e-9); // 3 reais → 9 abaixo do corte
        assert!((skill_curve(30.0, &c) - 37.0).abs() < 1e-9); // pior piloto: genuinamente ruim

        // O gap real de 3 pontos vale 3 no topo mas 9 na cauda (o fundo despenca).
        let gap_topo = skill_curve(49.0, &c) - skill_curve(46.0, &c);
        let gap_cauda = skill_curve(40.0, &c) - skill_curve(37.0, &c);
        assert!((gap_topo - 3.0).abs() < 1e-9);
        assert!((gap_cauda - 9.0).abs() < 1e-9);
    }

    #[test]
    fn cap_da_cauda_segura_o_pior_no_skill_real_em_grid_largo() {
        // Grid GT3 real: largo (59..96). O k_bottom desejado (3) afundaria o pior a ~10;
        // o cap segura ele NO skill real (59), nunca abaixo — regra do usuário.
        let ai = [
            96.0, 89.0, 88.0, 88.0, 87.0, 87.0, 86.0, 86.0, 83.0, 83.0, 80.0, 79.0, 78.0, 75.0,
            74.0, 72.0, 72.0, 70.0, 69.0, 69.0, 66.0, 65.0, 65.0, 63.0, 62.0, 61.0, 61.0, 59.0,
        ];
        let c = skill_curve_from(&ai, 95.0); // GT3 sweet spot 95
        assert!(c.k_bottom < 1.0); // cauda quase fiel (cap mordeu forte)
        assert!(c.k_bottom > 0.0);
        // Pior piloto (59) NÃO cai abaixo do skill real dele.
        let pior = skill_curve(59.0, &c);
        assert!(
            (pior - 59.0).abs() < 0.5,
            "pior efetivo {pior} deveria ~= 59"
        );
        // Melhor → sweet spot; grid fica ~fiel (não afunda como no rookie).
        assert!((skill_curve(96.0, &c) - 95.0).abs() < 1e-9);
    }

    #[test]
    fn curva_comprime_topo_quando_sweet_abaixo_do_real() {
        // Regime ELITE: skills reais ALTOS (68..98), sweet BAIXO (80). O topo comprime —
        // 98→80, 90→~77 (não fiel 72, nem grudado em 80). k_top < 1.
        let ai = [98.0, 95.0, 90.0, 88.0, 85.0, 80.0, 75.0, 70.0, 68.0];
        let c = skill_curve_from(&ai, 80.0);
        assert!(c.k_top < 1.0, "k_top {} deveria comprimir", c.k_top);
        assert!((skill_curve(98.0, &c) - 80.0).abs() < 1e-9); // melhor → sweet
        let p90 = skill_curve(90.0, &c);
        assert!(
            (p90 - 77.0).abs() < 0.6,
            "90 deveria virar ~77, virou {p90}"
        );
        // Pior (68) nunca abaixo do próprio skill real.
        assert!(skill_curve(68.0, &c) >= 68.0 - 0.5);
    }

    #[test]
    fn time_da_ia_usa_padrao_do_pool_estavel() {
        let numbers = HashMap::new();
        let entries = vec![(
            driver("D-x", "Xis", 10.0, 50.0, 50.0),
            Some(team("T9", "#3a86ff", "#222222", false)),
        )];
        let car = car_spec("mx5").unwrap();
        let r1 = build_roster(&entries, &car, &numbers, None, || "id".to_string());
        let r2 = build_roster(&entries, &car, &numbers, None, || "id".to_string());
        // Determinístico: mesma entrada → mesmo padrão.
        assert_eq!(r1.drivers[0].car_design, r2.drivers[0].car_design);
        // Padrão pertence ao pool aprovado do MX-5.
        let pat: i64 = r1.drivers[0]
            .car_design
            .split(',')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!([0, 4, 5, 8, 13].contains(&pat));
    }
}
