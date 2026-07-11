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
            let (d_skill, d_aggression, d_optimism, d_smoothness) = match behavior {
                None => (a.skill, a.aggression, a.confianca, a.smoothness),
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
                        base_skill: a.skill - track_pen,
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
                    (
                        out.skill + wet_rerank,
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
        // optimism ← confianca; skill direto.
        assert_eq!(bia.driver_optimism, 90);
        assert_eq!(bia.driver_skill, 80);
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
