#![allow(dead_code)]

use rusqlite::Connection;

use crate::common::time::current_timestamp;

use crate::db::connection::DbError;
use crate::db::queries::drivers::get_driver;
use crate::db::queries::news::insert_news;
use crate::db::queries::rivalries::{
    delete_rivalry, get_all_rivalries, get_rivalries_for_pilot, get_rivalry_by_pair,
    insert_rivalry, update_rivalry_axes,
};
use crate::generators::ids::{next_id, IdType};
use crate::models::rivalry::{
    normalize_pair, perceived_intensity, rivalry_lifecycle, Rivalry, RivalryLifecycle, RivalryType,
};
use crate::news::{NewsImportance, NewsItem, NewsType};

// ── Constantes de domínio ─────────────────────────────────────────────────────

const AXIS_MAX: f64 = 100.0;
const AXIS_MIN: f64 = 0.0;

// ── Passo 9: Thresholds semânticos (sobre intensidade percebida) ──────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RivalryIntensityLevel {
    AtritoLeve, // 0–19
    Inicial,    // 20–39
    Clara,      // 40–59
    Forte,      // 60–79
    Intensa,    // 80–100
}

impl RivalryIntensityLevel {
    pub fn label(&self) -> &'static str {
        match self {
            RivalryIntensityLevel::AtritoLeve => "atrito leve",
            RivalryIntensityLevel::Inicial => "rivalidade inicial",
            RivalryIntensityLevel::Clara => "rivalidade clara",
            RivalryIntensityLevel::Forte => "rivalidade forte",
            RivalryIntensityLevel::Intensa => "rivalidade intensa",
        }
    }
}

pub fn intensity_level(v: f64) -> RivalryIntensityLevel {
    if v < 20.0 {
        RivalryIntensityLevel::AtritoLeve
    } else if v < 40.0 {
        RivalryIntensityLevel::Inicial
    } else if v < 60.0 {
        RivalryIntensityLevel::Clara
    } else if v < 80.0 {
        RivalryIntensityLevel::Forte
    } else {
        RivalryIntensityLevel::Intensa
    }
}

/// Retorna o nível mais alto cruzado *para cima* ao passar de `old` para `new` (percebida).
/// Retorna `None` se nenhum threshold foi cruzado.
fn crossed_threshold(old: f64, new: f64) -> Option<RivalryIntensityLevel> {
    if new <= old {
        return None;
    }
    const THRESHOLDS: [(f64, RivalryIntensityLevel); 4] = [
        (20.0, RivalryIntensityLevel::Inicial),
        (40.0, RivalryIntensityLevel::Clara),
        (60.0, RivalryIntensityLevel::Forte),
        (80.0, RivalryIntensityLevel::Intensa),
    ];
    let mut highest: Option<RivalryIntensityLevel> = None;
    for (threshold, level) in &THRESHOLDS {
        if old < *threshold && new >= *threshold {
            highest = Some(level.clone());
        }
    }
    highest
}

// ── Passo 13: Evento de rivalidade com dois eixos ─────────────────────────────

pub struct RivalryEvent {
    pub piloto_a: String,
    pub piloto_b: String,
    /// Origem do evento — define o tipo da rivalidade se ela for nova.
    pub tipo: RivalryType,
    /// Quanto adicionar ao eixo histórico (memória duradoura).
    pub historical_delta: f64,
    /// Quanto adicionar ao eixo recente (calor atual).
    pub recent_delta: f64,
    /// Temporada corrente — atualiza `temporada_update`.
    pub temporada: i32,
}

// ── Resultado de apply_rivalry_event ─────────────────────────────────────────

pub struct RivalryApplied {
    pub rivalry_id: String,
    /// Intensidade percebida antes do evento.
    pub old_perceived: f64,
    /// Intensidade percebida depois do evento.
    pub new_perceived: f64,
}

// ── Passo 13: Upsert com dois eixos ──────────────────────────────────────────

pub fn apply_rivalry_event(
    conn: &Connection,
    event: &RivalryEvent,
) -> Result<RivalryApplied, DbError> {
    let pair = match normalize_pair(&event.piloto_a, &event.piloto_b) {
        Some(p) => p,
        None => {
            return Ok(RivalryApplied {
                rivalry_id: String::new(),
                old_perceived: 0.0,
                new_perceived: 0.0,
            });
        }
    };

    let now = current_timestamp();

    match get_rivalry_by_pair(conn, &pair.piloto1_id, &pair.piloto2_id)? {
        Some(existing) => {
            let old_perceived = existing.perceived_intensity();
            let new_historical = clamp(existing.historical_intensity + event.historical_delta);
            let new_recent = clamp(existing.recent_activity + event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            update_rivalry_axes(
                conn,
                &existing.id,
                new_historical,
                new_recent,
                &now,
                event.temporada,
            )?;
            Ok(RivalryApplied {
                rivalry_id: existing.id,
                old_perceived,
                new_perceived,
            })
        }
        None => {
            let id = next_id(conn, IdType::Rivalry)?;
            let new_historical = clamp(event.historical_delta);
            let new_recent = clamp(event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            let piloto1_id = pair.piloto1_id.clone();
            let piloto2_id = pair.piloto2_id.clone();
            let rivalry = Rivalry {
                id: id.clone(),
                piloto1_id: piloto1_id.clone(),
                piloto2_id: piloto2_id.clone(),
                historical_intensity: new_historical,
                recent_activity: new_recent,
                tipo: event.tipo.clone(),
                criado_em: now.clone(),
                ultima_atualizacao: now,
                temporada_update: event.temporada,
            };
            match insert_rivalry(conn, &rivalry) {
                Ok(()) => Ok(RivalryApplied {
                    rivalry_id: id,
                    old_perceived: 0.0,
                    new_perceived,
                }),
                Err(err) if is_rivalry_pair_constraint(&err) => {
                    let existing = get_rivalry_by_pair(conn, &piloto1_id, &piloto2_id)?
                        .ok_or_else(|| {
                            DbError::InvalidData(format!(
                                "Par de rivalidade '{}' x '{}' conflitou no insert, mas nao foi encontrado no reload",
                                piloto1_id, piloto2_id
                            ))
                        })?;
                    let old_perceived = existing.perceived_intensity();
                    let new_historical =
                        clamp(existing.historical_intensity + event.historical_delta);
                    let new_recent = clamp(existing.recent_activity + event.recent_delta);
                    let new_perceived = perceived_intensity(new_historical, new_recent);
                    update_rivalry_axes(
                        conn,
                        &existing.id,
                        new_historical,
                        new_recent,
                        &current_timestamp(),
                        event.temporada,
                    )?;
                    Ok(RivalryApplied {
                        rivalry_id: existing.id,
                        old_perceived,
                        new_perceived,
                    })
                }
                Err(err) => Err(err),
            }
        }
    }
}

// ── Passo 12: Leitura por piloto ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PilotRivalrySummary {
    pub rivalry_id: String,
    pub rival_id: String,
    pub historical_intensity: f64,
    pub recent_activity: f64,
    /// Calculado em tempo de leitura (0.4*hist + 0.6*rec).
    pub perceived_intensity: f64,
    pub tipo: RivalryType,
    pub ultima_atualizacao: String,
}

pub fn get_pilot_rivalries(
    conn: &Connection,
    pilot_id: &str,
) -> Result<Vec<PilotRivalrySummary>, DbError> {
    let rivalries = get_rivalries_for_pilot(conn, pilot_id)?;
    let summaries = rivalries
        .into_iter()
        .map(|r| {
            let rival_id = if r.piloto1_id == pilot_id {
                r.piloto2_id.clone()
            } else {
                r.piloto1_id.clone()
            };
            let perceived = r.perceived_intensity();
            PilotRivalrySummary {
                rivalry_id: r.id,
                rival_id,
                historical_intensity: r.historical_intensity,
                recent_activity: r.recent_activity,
                perceived_intensity: perceived,
                tipo: r.tipo,
                ultima_atualizacao: r.ultima_atualizacao,
            }
        })
        .collect();
    Ok(summaries)
}

pub fn remove_rivalry(conn: &Connection, rivalry_id: &str) -> Result<(), DbError> {
    delete_rivalry(conn, rivalry_id)
}

// ── Passo 10: Geração de notícia (atualizado para percebida) ──────────────────

fn build_rivalry_news_item(
    id: String,
    applied: &RivalryApplied,
    tipo: &RivalryType,
    nome_a: &str,
    nome_b: &str,
    categoria_id: &str,
    temporada: i32,
    rodada: i32,
    piloto_a_id: &str,
    piloto_b_id: &str,
    team_id: Option<&str>,
    _driver_midia: &std::collections::HashMap<String, f64>,
) -> Option<NewsItem> {
    let crossed = crossed_threshold(applied.old_perceived, applied.new_perceived)?;
    let importance = match crossed {
        RivalryIntensityLevel::Inicial => NewsImportance::Media,
        RivalryIntensityLevel::Clara => NewsImportance::Alta,
        RivalryIntensityLevel::Forte | RivalryIntensityLevel::Intensa => NewsImportance::Destaque,
        RivalryIntensityLevel::AtritoLeve => NewsImportance::Media,
    };
    let origem = match tipo {
        RivalryType::Companheiros => "companheiros de equipe",
        RivalryType::Campeonato => "disputa de campeonato",
        RivalryType::Colisao => "incidente em pista",
        RivalryType::Pista => "duelos em pista",
    };
    let nivel_label = crossed.label();
    let titulo = format!("{nome_a} x {nome_b}: {nivel_label}");
    let texto = format!(
        "{nome_a} e {nome_b} elevaram a tensao da rivalidade após {origem} na rodada {rodada}."
    );

    Some(NewsItem {
        id,
        tipo: NewsType::Rivalidade,
        icone: NewsType::Rivalidade.icone().to_string(),
        titulo,
        texto,
        rodada: Some(rodada),
        semana_pretemporada: None,
        temporada,
        categoria_id: Some(categoria_id.to_string()),
        categoria_nome: None,
        importancia: importance,
        timestamp: chrono::Local::now().timestamp(),
        driver_id: Some(piloto_a_id.to_string()),
        driver_id_secondary: Some(piloto_b_id.to_string()),
        team_id: team_id.map(str::to_string),
    })
}

fn load_rivalry_driver_midia(
    conn: &Connection,
    piloto_a_id: &str,
    piloto_b_id: &str,
) -> std::collections::HashMap<String, f64> {
    let mut driver_midia = std::collections::HashMap::new();

    for driver_id in [piloto_a_id, piloto_b_id] {
        if let Ok(driver) = get_driver(conn, driver_id) {
            driver_midia.insert(driver_id.to_string(), driver.atributos.midia);
        }
    }

    driver_midia
}

fn persist_rivalry_news(
    conn: &Connection,
    applied: &RivalryApplied,
    tipo: &RivalryType,
    nome_a: &str,
    nome_b: &str,
    categoria_id: &str,
    temporada: i32,
    rodada: i32,
    piloto_a_id: &str,
    piloto_b_id: &str,
    team_id: Option<&str>,
    driver_midia: &std::collections::HashMap<String, f64>,
) -> Result<(), DbError> {
    let Some(item) = build_rivalry_news_item(
        next_id(conn, IdType::News)?,
        applied,
        tipo,
        nome_a,
        nome_b,
        categoria_id,
        temporada,
        rodada,
        piloto_a_id,
        piloto_b_id,
        team_id,
        driver_midia,
    ) else {
        return Ok(());
    };

    insert_news(conn, &item)?;
    Ok(())
}

// ── Passo 6: Rivalidade por hierarquia interna ────────────────────────────────

/// Avalia transição de status hierárquico e aplica evento de rivalidade com dois eixos.
///
/// Deltas semânticos (Passo 13):
/// - Inversão:                       historical=8,  recent=18  → percebido ≈14
/// - Transição → Crise (nova):       historical=5,  recent=14  → percebido ≈10
/// - Transição → Reavaliação (nova): historical=3,  recent=10  → percebido ≈7
pub fn process_hierarchy_rivalry(
    conn: &Connection,
    n1_id: &str,
    n2_id: &str,
    old_status_str: &str,
    new_status_str: &str,
    inversao: bool,
    categoria_id: &str,
    team_id: &str,
    rodada: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::models::team::TeamHierarchyClimate;

    let old_status = TeamHierarchyClimate::from_str(old_status_str);
    let new_status = TeamHierarchyClimate::from_str(new_status_str);

    let (h_delta, r_delta): (f64, f64) = if inversao {
        (8.0, 18.0)
    } else if new_status == TeamHierarchyClimate::Crise && old_status != TeamHierarchyClimate::Crise
    {
        (5.0, 14.0)
    } else if new_status == TeamHierarchyClimate::Reavaliacao
        && !matches!(
            old_status,
            TeamHierarchyClimate::Reavaliacao | TeamHierarchyClimate::Crise
        )
    {
        (3.0, 10.0)
    } else {
        return Ok(());
    };

    let applied = apply_rivalry_event(
        conn,
        &RivalryEvent {
            piloto_a: n1_id.to_string(),
            piloto_b: n2_id.to_string(),
            tipo: RivalryType::Companheiros,
            historical_delta: h_delta,
            recent_delta: r_delta,
            temporada,
        },
    )?;

    if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
        let nome_a = get_driver(conn, n1_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| n1_id.to_string());
        let nome_b = get_driver(conn, n2_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| n2_id.to_string());
        let driver_midia = load_rivalry_driver_midia(conn, n1_id, n2_id);

        persist_rivalry_news(
            conn,
            &applied,
            &RivalryType::Companheiros,
            &nome_a,
            &nome_b,
            categoria_id,
            temporada,
            rodada,
            n1_id,
            n2_id,
            Some(team_id),
            &driver_midia,
        )?;
    }

    Ok(())
}

// ── Passo 7: Rivalidade por disputa de campeonato ─────────────────────────────

/// Detecta disputas apertadas nas últimas rodadas e reforça rivalidades entre líderes.
///
/// Deltas (Passo 13): historical=4, recent=10 → percebido ≈7.6
/// Política: só últimas 3 rodadas, só top-3, gap ≤ 20 pontos.
pub fn process_championship_rivalry(
    conn: &Connection,
    categoria_id: &str,
    rodada_atual: i32,
    total_rounds: i32,
    temporada: i32,
) -> Result<(), DbError> {
    if rodada_atual < total_rounds - 2 {
        return Ok(());
    }

    use crate::db::queries::drivers::get_drivers_by_category;

    let mut drivers = get_drivers_by_category(conn, categoria_id)?;
    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    drivers.truncate(3);

    for i in 0..drivers.len() {
        for j in (i + 1)..drivers.len() {
            let gap = (drivers[i].stats_temporada.pontos - drivers[j].stats_temporada.pontos).abs();
            if gap > 20.0 {
                continue;
            }

            let applied = apply_rivalry_event(
                conn,
                &RivalryEvent {
                    piloto_a: drivers[i].id.clone(),
                    piloto_b: drivers[j].id.clone(),
                    tipo: RivalryType::Campeonato,
                    historical_delta: 4.0,
                    recent_delta: 10.0,
                    temporada,
                },
            )?;

            if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
                let driver_midia = load_rivalry_driver_midia(conn, &drivers[i].id, &drivers[j].id);
                persist_rivalry_news(
                    conn,
                    &applied,
                    &RivalryType::Campeonato,
                    &drivers[i].nome,
                    &drivers[j].nome,
                    categoria_id,
                    temporada,
                    rodada_atual,
                    &drivers[i].id,
                    &drivers[j].id,
                    None,
                    &driver_midia,
                )?;
            }
        }
    }

    Ok(())
}

// ── Passo 14: Decaimento de fim de temporada ──────────────────────────────────

/// Aplica decaimento anual a todas as rivalidades.
///
/// Regras:
/// - Rivalidade ativa na temporada atual (temporada_update == temporada_atual):
///   recent_activity *= 0.5  |  historical_intensity inalterado
/// - Rivalidade inativa (temporada_update < temporada_atual):
///   recent_activity *= 0.2  |  historical_intensity *= 0.85
/// - Se o ciclo de vida resultante for Extinta → remove do banco.
///
/// Deve ser chamada uma vez no pipeline de fim de temporada.
pub fn apply_season_end_rivalry_decay(
    conn: &Connection,
    temporada_atual: i32,
) -> Result<(), DbError> {
    let all = get_all_rivalries(conn)?;
    let now = current_timestamp();

    for r in all {
        let (new_historical, new_recent) = if r.temporada_update == temporada_atual {
            // Ativa nesta temporada: esfria o calor recente pela metade
            (r.historical_intensity, r.recent_activity * 0.5)
        } else {
            // Inativa: recente esfria bastante, histórico decai levemente
            (r.historical_intensity * 0.85, r.recent_activity * 0.2)
        };

        if matches!(
            rivalry_lifecycle(new_historical, new_recent),
            RivalryLifecycle::Extinta
        ) {
            delete_rivalry(conn, &r.id)?;
        } else {
            update_rivalry_axes(
                conn,
                &r.id,
                new_historical,
                new_recent,
                &now,
                r.temporada_update, // temporada_update não muda no decaimento
            )?;
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn clamp(v: f64) -> f64 {
    v.clamp(AXIS_MIN, AXIS_MAX)
}

fn is_rivalry_pair_constraint(err: &DbError) -> bool {
    matches!(
        err,
        DbError::Sqlite(rusqlite::Error::SqliteFailure(inner, _))
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

// ── Passo 15: Mapeamento Factual de Colisão ───────────────────────────────────

pub fn process_collisions_rivalry(
    conn: &Connection,
    incidents: &[crate::simulation::incidents::IncidentResult],
    categoria_id: &str,
    rodada: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::simulation::incidents::{IncidentSeverity, IncidentType};
    use std::collections::HashMap;

    let mut collision_pairs: HashMap<(String, String), (f64, f64)> = HashMap::new();

    for inc in incidents {
        if inc.incident_type == IncidentType::Collision {
            if let Some(linked_id) = &inc.linked_pilot_id {
                let Some(pair) = normalize_pair(&inc.pilot_id, linked_id) else {
                    continue;
                };
                let p1 = pair.piloto1_id;
                let p2 = pair.piloto2_id;

                let (h, r) = if inc.severity == IncidentSeverity::Critical {
                    (7.0, 18.0)
                } else if inc.is_dnf {
                    (5.0, 14.0)
                } else if inc.severity == IncidentSeverity::Major || inc.positions_lost >= 3 {
                    (3.0, 10.0)
                } else {
                    (2.0, 8.0)
                };

                let current = collision_pairs.entry((p1, p2)).or_insert((0.0, 0.0));
                if h > current.0 {
                    current.0 = h;
                    current.1 = r;
                }
            }
        }
    }

    for ((p1, p2), (h, r)) in collision_pairs {
        let applied = apply_rivalry_event(
            conn,
            &RivalryEvent {
                piloto_a: p1.clone(),
                piloto_b: p2.clone(),
                tipo: RivalryType::Colisao,
                historical_delta: h,
                recent_delta: r,
                temporada,
            },
        )?;

        if crossed_threshold(applied.old_perceived, applied.new_perceived).is_some() {
            let nome_a = get_driver(conn, &p1)
                .map(|d| d.nome)
                .unwrap_or_else(|_| p1.clone());
            let nome_b = get_driver(conn, &p2)
                .map(|d| d.nome)
                .unwrap_or_else(|_| p2.clone());
            let driver_midia = load_rivalry_driver_midia(conn, &p1, &p2);

            persist_rivalry_news(
                conn,
                &applied,
                &RivalryType::Colisao,
                &nome_a,
                &nome_b,
                categoria_id,
                temporada,
                rodada,
                &p1,
                &p2,
                None,
                &driver_midia,
            )?;
        }
    }

    Ok(())
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;
    use crate::db::queries::drivers::insert_driver;
    use crate::db::queries::news::get_news_by_type;
    use crate::db::queries::seasons::insert_season;
    use crate::models::driver::Driver;
    use crate::models::season::Season;
    use crate::news::NewsType;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        insert_season(&conn, &Season::new("S001".to_string(), 1, 2024)).unwrap();
        for (id, nome) in [
            ("P001", "Piloto1"),
            ("P002", "Piloto2"),
            ("P003", "Piloto3"),
            ("P020", "Piloto20"),
        ] {
            let mut d =
                Driver::create_player(id.to_string(), nome.to_string(), "BR".to_string(), 25);
            d.is_jogador = false;
            insert_driver(&conn, &d).unwrap();
        }
        conn
    }

    fn event(a: &str, b: &str, tipo: RivalryType, h: f64, r: f64) -> RivalryEvent {
        RivalryEvent {
            piloto_a: a.to_string(),
            piloto_b: b.to_string(),
            tipo,
            historical_delta: h,
            recent_delta: r,
            temporada: 1,
        }
    }

    // ── Passos 1-5 (regressão) ────────────────────────────────────────────────

    #[test]
    fn cria_rivalidade_nova() {
        let conn = setup_db();
        // h=10, r=20 → perceived = 0.4*10 + 0.6*20 = 16.0
        let applied = apply_rivalry_event(
            &conn,
            &event("P020", "P003", RivalryType::Colisao, 10.0, 20.0),
        )
        .unwrap();
        assert!((applied.new_perceived - 16.0).abs() < 1e-9);
        assert!(applied.old_perceived.abs() < 1e-9);

        let summaries = get_pilot_rivalries(&conn, "P003").unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].rival_id, "P020");
    }

    #[test]
    fn reforco_acumula_nos_dois_eixos() {
        let conn = setup_db();
        // 1ª aplicação: h=10, r=20
        apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Campeonato, 10.0, 20.0),
        )
        .unwrap();
        // 2ª aplicação: h=10, r=20 → acumulado h=20, r=40
        // perceived = 0.4*20 + 0.6*40 = 8 + 24 = 32
        let applied = apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Campeonato, 10.0, 20.0),
        )
        .unwrap();
        assert!((applied.new_perceived - 32.0).abs() < 1e-9);
    }

    #[test]
    fn clamp_nao_passa_de_100() {
        let conn = setup_db();
        apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Pista, 70.0, 70.0),
        )
        .unwrap();
        // h=70, r=70 → perceived=70; depois h=100(clamped), r=100 → perceived=100
        let applied = apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Pista, 70.0, 70.0),
        )
        .unwrap();
        assert!((applied.new_perceived - 100.0).abs() < 1e-9);
    }

    #[test]
    fn tipo_original_preservado_no_reforco() {
        let conn = setup_db();
        apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Campeonato, 10.0, 10.0),
        )
        .unwrap();
        apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Colisao, 10.0, 10.0),
        )
        .unwrap();

        let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
        assert_eq!(summaries[0].tipo, RivalryType::Campeonato);
    }

    #[test]
    fn mesmo_piloto_ignorado() {
        let conn = setup_db();
        apply_rivalry_event(
            &conn,
            &RivalryEvent {
                piloto_a: "P001".to_string(),
                piloto_b: "P001".to_string(),
                tipo: RivalryType::Pista,
                historical_delta: 50.0,
                recent_delta: 50.0,
                temporada: 1,
            },
        )
        .unwrap();
        assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
    }

    // ── Passo 9: Thresholds ───────────────────────────────────────────────────

    #[test]
    fn intensity_level_faixas_corretas() {
        assert_eq!(intensity_level(0.0), RivalryIntensityLevel::AtritoLeve);
        assert_eq!(intensity_level(19.9), RivalryIntensityLevel::AtritoLeve);
        assert_eq!(intensity_level(20.0), RivalryIntensityLevel::Inicial);
        assert_eq!(intensity_level(39.9), RivalryIntensityLevel::Inicial);
        assert_eq!(intensity_level(40.0), RivalryIntensityLevel::Clara);
        assert_eq!(intensity_level(60.0), RivalryIntensityLevel::Forte);
        assert_eq!(intensity_level(80.0), RivalryIntensityLevel::Intensa);
        assert_eq!(intensity_level(100.0), RivalryIntensityLevel::Intensa);
    }

    #[test]
    fn crossed_threshold_detecta_threshold_correto() {
        assert_eq!(
            crossed_threshold(15.0, 25.0),
            Some(RivalryIntensityLevel::Inicial)
        );
        assert_eq!(
            crossed_threshold(35.0, 45.0),
            Some(RivalryIntensityLevel::Clara)
        );
        // Salta dois thresholds — retorna o mais alto
        assert_eq!(
            crossed_threshold(15.0, 65.0),
            Some(RivalryIntensityLevel::Forte)
        );
        // Sem cruzamento (já na faixa)
        assert_eq!(crossed_threshold(25.0, 35.0), None);
        // Decaimento: sem cruzamento
        assert_eq!(crossed_threshold(50.0, 30.0), None);
    }

    // ── Passo 6: Hierarquia ───────────────────────────────────────────────────

    #[test]
    fn hierarchy_rivalry_crise_cria_evento() {
        let conn = setup_db();
        process_hierarchy_rivalry(
            &conn, "P001", "P002", "tensao", "crise", false, "gt3", "T001", 5, 1,
        )
        .unwrap();

        let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
        assert_eq!(summaries.len(), 1);
        // h=5, r=14 → perceived = 0.4*5 + 0.6*14 = 2 + 8.4 = 10.4
        assert!((summaries[0].perceived_intensity - 10.4).abs() < 1e-9);
    }

    #[test]
    fn hierarchy_rivalry_inversao_maior_delta() {
        let conn = setup_db();
        process_hierarchy_rivalry(
            &conn,
            "P001",
            "P002",
            "crise",
            "reavaliacao",
            true,
            "gt3",
            "T001",
            5,
            1,
        )
        .unwrap();

        let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
        // h=8, r=18 → perceived = 0.4*8 + 0.6*18 = 3.2 + 10.8 = 14.0
        assert!((summaries[0].perceived_intensity - 14.0).abs() < 1e-9);
    }

    #[test]
    fn hierarchy_rivalry_estado_estavel_nao_gera_evento() {
        let conn = setup_db();
        process_hierarchy_rivalry(
            &conn,
            "P001",
            "P002",
            "estavel",
            "competitivo",
            false,
            "gt3",
            "T001",
            5,
            1,
        )
        .unwrap();
        assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
    }

    #[test]
    fn hierarchy_rivalry_crise_persistente_nao_spam() {
        let conn = setup_db();
        process_hierarchy_rivalry(
            &conn, "P001", "P002", "crise", "crise", false, "gt3", "T001", 5, 1,
        )
        .unwrap();
        assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
    }

    // ── Passo 7: Campeonato ───────────────────────────────────────────────────

    #[test]
    fn championship_rivalry_ultimas_rodadas_gap_pequeno() {
        let conn = setup_db();
        conn.execute(
            "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 50.0 WHERE id = 'P001'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 45.0 WHERE id = 'P002'",
            [],
        )
        .unwrap();

        process_championship_rivalry(&conn, "gt3", 8, 10, 1).unwrap();

        let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
        assert_eq!(summaries.len(), 1);
        // h=4, r=10 → perceived = 0.4*4 + 0.6*10 = 1.6 + 6.0 = 7.6
        assert!((summaries[0].perceived_intensity - 7.6).abs() < 1e-9);
    }

    #[test]
    fn championship_rivalry_muito_cedo_nao_gera() {
        let conn = setup_db();
        conn.execute(
            "UPDATE drivers SET temp_pontos = 50.0 WHERE id = 'P001'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE drivers SET temp_pontos = 45.0 WHERE id = 'P002'",
            [],
        )
        .unwrap();

        process_championship_rivalry(&conn, "gt3", 3, 10, 1).unwrap();
        assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
    }

    #[test]
    fn championship_rivalry_gap_grande_nao_gera() {
        let conn = setup_db();
        conn.execute(
            "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 100.0 WHERE id = 'P001'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = 20.0  WHERE id = 'P002'",
            [],
        )
        .unwrap();

        process_championship_rivalry(&conn, "gt3", 9, 10, 1).unwrap();
        assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
    }

    #[test]
    fn championship_rivalry_limita_ao_top3() {
        let conn = setup_db();
        for (id, pontos) in [
            ("P001", 60.0),
            ("P002", 55.0),
            ("P003", 50.0),
            ("P020", 49.0),
        ] {
            conn.execute(
                "UPDATE drivers SET categoria_atual = 'gt3', temp_pontos = ?2 WHERE id = ?1",
                rusqlite::params![id, pontos],
            )
            .unwrap();
        }

        process_championship_rivalry(&conn, "gt3", 9, 10, 1).unwrap();

        assert!(
            get_pilot_rivalries(&conn, "P020").unwrap().is_empty(),
            "o 4o colocado nao deve entrar na regra de rivalidade de campeonato"
        );
    }

    // ── Passo 14: Decaimento ──────────────────────────────────────────────────

    #[test]
    fn decay_rivalidade_ativa_esfria_recente() {
        let conn = setup_db();
        // Criar rivalidade na temporada 1
        apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Campeonato, 20.0, 40.0),
        )
        .unwrap();

        // Decaimento de fim da temporada 1 (rivalidade foi ativa nesta temporada)
        apply_season_end_rivalry_decay(&conn, 1).unwrap();

        let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
        assert_eq!(summaries.len(), 1);
        // h permanece 20, r = 40 * 0.5 = 20
        // perceived = 0.4*20 + 0.6*20 = 8 + 12 = 20.0
        assert!((summaries[0].historical_intensity - 20.0).abs() < 1e-9);
        assert!((summaries[0].recent_activity - 20.0).abs() < 1e-9);
    }

    #[test]
    fn decay_rivalidade_inativa_decai_nos_dois_eixos() {
        let conn = setup_db();
        // Criar rivalidade na temporada 1
        apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Campeonato, 20.0, 40.0),
        )
        .unwrap();

        // Decaimento de fim da temporada 2 (rivalidade foi criada em t1, agora é t2)
        apply_season_end_rivalry_decay(&conn, 2).unwrap();

        let summaries = get_pilot_rivalries(&conn, "P001").unwrap();
        assert_eq!(summaries.len(), 1);
        // h = 20 * 0.85 = 17.0, r = 40 * 0.2 = 8.0
        assert!((summaries[0].historical_intensity - 17.0).abs() < 1e-9);
        assert!((summaries[0].recent_activity - 8.0).abs() < 1e-9);
    }

    #[test]
    fn decay_rivalidade_extinta_e_removida() {
        let conn = setup_db();
        // Criar rivalidade fraca (h=3, r=5) e simular que está inativa há tempos
        apply_rivalry_event(&conn, &event("P001", "P002", RivalryType::Pista, 3.0, 5.0)).unwrap();

        // Após decaimento inativo: h = 3*0.85 = 2.55, r = 5*0.2 = 1.0
        // lifecycle: perceived = 0.4*2.55 + 0.6*1.0 = 1.02 + 0.6 = 1.62 < 5; h=2.55 < 10 → Extinta
        apply_season_end_rivalry_decay(&conn, 5).unwrap();

        assert!(get_pilot_rivalries(&conn, "P001").unwrap().is_empty());
    }

    #[test]
    fn hierarchy_rivalry_crossing_threshold_persists_news() {
        let conn = setup_db();
        apply_rivalry_event(
            &conn,
            &event("P001", "P002", RivalryType::Companheiros, 15.0, 20.0),
        )
        .unwrap();

        process_hierarchy_rivalry(
            &conn, "P001", "P002", "tensao", "crise", false, "gt3", "T001", 5, 1,
        )
        .unwrap();

        let news = get_news_by_type(&conn, &NewsType::Rivalidade, 10).unwrap();
        assert_eq!(news.len(), 1);
        assert_eq!(news[0].driver_id.as_deref(), Some("P001"));
        assert_eq!(news[0].driver_id_secondary.as_deref(), Some("P002"));
        assert_eq!(news[0].team_id.as_deref(), Some("T001"));
    }

    #[test]
    fn rivalries_table_rejects_duplicate_pair() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO rivalries
                 (id, piloto1_id, piloto2_id, intensidade, historical_intensity,
                  recent_activity, tipo, criado_em, ultima_atualizacao, temporada_update)
             VALUES ('R001', 'P001', 'P002', 10.0, 10.0, 10.0, 'Campeonato', '1', '1', 1)",
            [],
        )
        .unwrap();

        let duplicate = conn.execute(
            "INSERT INTO rivalries
                 (id, piloto1_id, piloto2_id, intensidade, historical_intensity,
                  recent_activity, tipo, criado_em, ultima_atualizacao, temporada_update)
             VALUES ('R002', 'P001', 'P002', 20.0, 20.0, 20.0, 'Colisao', '2', '2', 1)",
            [],
        );

        assert!(duplicate.is_err(), "par duplicado nao deve ser permitido");
    }
}
