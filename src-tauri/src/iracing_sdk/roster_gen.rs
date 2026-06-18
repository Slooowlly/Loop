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

/// Monta o roster a partir do grid (pilotos + time de cada um) e do mapa de
/// números fixos por piloto. `id_factory` gera o GUID de cada entrada.
pub fn build_roster(
    entries: &[(Driver, Option<TeamInfo>)],
    car: &CarSpec,
    numbers: &HashMap<String, i64>,
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
                driver_skill: attr(a.skill),
                driver_aggression: attr(a.aggression),
                driver_optimism: attr(a.confianca),
                driver_smoothness: attr(a.smoothness),
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
            (driver("D-ana", "Ana", 10.0, 40.0, 30.0), Some(team("T1", "#e63946", "#000000", true))),
            (driver("D-bia", "Bia", 50.0, 80.0, 90.0), Some(team("T1", "#e63946", "#000000", true))),
        ];
        let car = car_spec("mx5").unwrap();
        let mut n = 0;
        let roster = build_roster(&entries, &car, &numbers, || {
            n += 1;
            format!("ID-{n}")
        });

        // Número fixo do mapa (não a posição).
        let ana = roster.drivers.iter().find(|d| d.driver_name == "Ana").unwrap();
        let bia = roster.drivers.iter().find(|d| d.driver_name == "Bia").unwrap();
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
        let r1 = build_roster(&entries, &car, &numbers, || "id".to_string());
        let r2 = build_roster(&entries, &car, &numbers, || "id".to_string());
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
