//! Gatilhos que criam ou reforçam rivalidades: hierarquia interna, disputa de
//! campeonato e colisões em pista (extraído de `rivalry/mod.rs`).

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::drivers::get_driver;
use crate::models::rivalry::{normalize_pair, RivalryType};

use super::eventos::{apply_rivalry_event, RivalryEvent};
use super::intensidade::crossed_threshold;
use super::noticias::{load_rivalry_driver_midia, persist_rivalry_news};

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
