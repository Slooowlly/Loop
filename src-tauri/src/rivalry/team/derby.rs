//! Tier 2: Moral de derby (pulso per-race).
//!
//! Para todo par de times com rivalidade viva presente na corrida, o que teve o melhor
//! carro à frente do rival ganha moral; o outro perde. Movimento NOVO de moral no meio da
//! temporada (hoje a moral só roda no offseason) — sutil, escalado pela percebida, simétrico
//! jogador+IA. A moral já é sentida na pista (`morale_pace_delta`) → vira ritmo na corrida
//! seguinte. Loop fechado sem tocar em mercado.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::team_rivalries::get_all_team_rivalries;
use crate::db::queries::teams as team_queries;
use crate::models::rivalry::{rivalry_lifecycle, RivalryLifecycle};

/// Base do pulso de moral de derby (multiplicado por 0.5 + percebida/100).
const DERBY_MORALE_BASE: f64 = 0.015;
/// Piso/teto da moral (mesma banda que `advance_team_morale` respeita).
const MORALE_FLOOR: f64 = 0.5;
const MORALE_CEIL: f64 = 1.5;
/// Percebida mínima para um par gerar pulso de derby (abaixo, rivalidade fria demais).
const DERBY_MIN_PERCEIVED: f64 = 20.0;

/// Aplica o pulso de moral de derby de uma corrida. `team_best_finish` = melhor posição de
/// chegada de cada time nesta corrida (menor = melhor).
pub fn apply_derby_morale(
    conn: &Connection,
    team_best_finish: &HashMap<String, i32>,
) -> Result<(), DbError> {
    let all = get_all_team_rivalries(conn)?;
    // Acumula o delta por time (um time pode viver vários derbies na mesma corrida).
    let mut morale_delta: HashMap<String, f64> = HashMap::new();
    for rv in all {
        if matches!(
            rivalry_lifecycle(rv.historical_intensity, rv.recent_activity),
            RivalryLifecycle::Extinta
        ) {
            continue;
        }
        let perceived = rv.perceived_intensity();
        if perceived < DERBY_MIN_PERCEIVED {
            continue;
        }
        let (Some(&pa), Some(&pb)) = (
            team_best_finish.get(&rv.team1_id),
            team_best_finish.get(&rv.team2_id),
        ) else {
            continue;
        };
        if pa == pb {
            continue;
        }
        let delta = DERBY_MORALE_BASE * (0.5 + perceived / 100.0);
        let (winner, loser) = if pa < pb {
            (&rv.team1_id, &rv.team2_id)
        } else {
            (&rv.team2_id, &rv.team1_id)
        };
        *morale_delta.entry(winner.clone()).or_insert(0.0) += delta;
        *morale_delta.entry(loser.clone()).or_insert(0.0) -= delta;
    }

    for (team_id, delta) in morale_delta {
        if delta.abs() < 1e-9 {
            continue;
        }
        let Some(mut team) = team_queries::get_team_by_id(conn, &team_id)? else {
            continue;
        };
        team.morale = (team.morale + delta).clamp(MORALE_FLOOR, MORALE_CEIL);
        team_queries::update_team(conn, &team)?;
    }
    Ok(())
}
