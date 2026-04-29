#![allow(dead_code)]

use rusqlite::{params, Connection};

use crate::db::connection::DbError;
use crate::db::queries::teams as team_queries;
use crate::models::team::Team;

#[derive(Debug, Clone)]
pub struct NewSpecialTeamEntry {
    pub team_id: String,
    pub source_category: String,
    pub qualified_via: String,
    pub guaranteed_next_year: bool,
}

#[derive(Debug, Clone)]
pub struct SpecialTeamEntry {
    pub season_id: String,
    pub special_category: String,
    pub class_name: String,
    pub team_id: String,
    pub source_category: String,
    pub qualified_via: String,
    pub guaranteed_next_year: bool,
}

pub fn replace_entries_for_class(
    conn: &Connection,
    season_id: &str,
    special_category: &str,
    class_name: &str,
    entries: &[NewSpecialTeamEntry],
) -> Result<(), DbError> {
    conn.execute(
        "DELETE FROM special_team_entries
         WHERE season_id = ?1 AND special_category = ?2 AND class_name = ?3",
        params![season_id, special_category, class_name],
    )?;

    for entry in entries {
        conn.execute(
            "INSERT INTO special_team_entries (
                season_id, special_category, class_name, team_id,
                source_category, qualified_via, guaranteed_next_year, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)",
            params![
                season_id,
                special_category,
                class_name,
                entry.team_id,
                entry.source_category,
                entry.qualified_via,
                i64::from(entry.guaranteed_next_year),
            ],
        )?;
    }

    Ok(())
}

pub fn get_entries_for_class(
    conn: &Connection,
    season_id: &str,
    special_category: &str,
    class_name: &str,
) -> Result<Vec<SpecialTeamEntry>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT season_id, special_category, class_name, team_id,
                source_category, qualified_via, guaranteed_next_year
         FROM special_team_entries
         WHERE season_id = ?1 AND special_category = ?2 AND class_name = ?3
         ORDER BY guaranteed_next_year DESC, qualified_via ASC, team_id ASC",
    )?;
    let rows = stmt.query_map(params![season_id, special_category, class_name], |row| {
        Ok(SpecialTeamEntry {
            season_id: row.get(0)?,
            special_category: row.get(1)?,
            class_name: row.get(2)?,
            team_id: row.get(3)?,
            source_category: row.get(4)?,
            qualified_via: row.get(5)?,
            guaranteed_next_year: row.get::<_, i64>(6)? != 0,
        })
    })?;

    collect_entries(rows)
}

pub fn get_entry_teams_for_class(
    conn: &Connection,
    season_id: &str,
    special_category: &str,
    class_name: &str,
) -> Result<Vec<Team>, DbError> {
    let entries = get_entries_for_class(conn, season_id, special_category, class_name)?;
    hydrate_entry_teams(conn, entries)
}

pub fn get_entry_teams_for_category(
    conn: &Connection,
    season_id: &str,
    special_category: &str,
) -> Result<Vec<Team>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT season_id, special_category, class_name, team_id,
                source_category, qualified_via, guaranteed_next_year
         FROM special_team_entries
         WHERE season_id = ?1 AND special_category = ?2
         ORDER BY class_name ASC, guaranteed_next_year DESC, qualified_via ASC, team_id ASC",
    )?;
    let rows = stmt.query_map(params![season_id, special_category], |row| {
        Ok(SpecialTeamEntry {
            season_id: row.get(0)?,
            special_category: row.get(1)?,
            class_name: row.get(2)?,
            team_id: row.get(3)?,
            source_category: row.get(4)?,
            qualified_via: row.get(5)?,
            guaranteed_next_year: row.get::<_, i64>(6)? != 0,
        })
    })?;

    hydrate_entry_teams(conn, collect_entries(rows)?)
}

pub fn get_previous_guaranteed_team_ids(
    conn: &Connection,
    current_season_number: i32,
    special_category: &str,
    class_name: &str,
) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT e.team_id
         FROM special_team_entries e
         INNER JOIN seasons s ON s.id = e.season_id
         WHERE s.numero = ?1
           AND e.special_category = ?2
           AND e.class_name = ?3
           AND e.guaranteed_next_year = 1
         ORDER BY e.updated_at ASC, e.team_id ASC",
    )?;
    let rows = stmt.query_map(
        params![
            current_season_number.saturating_sub(1),
            special_category,
            class_name
        ],
        |row| row.get::<_, String>(0),
    )?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

pub fn update_guarantees_for_class(
    conn: &Connection,
    season_id: &str,
    special_category: &str,
    class_name: &str,
    top_count: usize,
) -> Result<usize, DbError> {
    conn.execute(
        "UPDATE special_team_entries
         SET guaranteed_next_year = 0, updated_at = CURRENT_TIMESTAMP
         WHERE season_id = ?1 AND special_category = ?2 AND class_name = ?3",
        params![season_id, special_category, class_name],
    )?;

    let team_ids = top_team_ids_from_results(conn, season_id, special_category, class_name)?;
    let mut updated = 0;
    for team_id in team_ids.into_iter().take(top_count) {
        updated += conn.execute(
            "UPDATE special_team_entries
             SET guaranteed_next_year = 1, updated_at = CURRENT_TIMESTAMP
             WHERE season_id = ?1 AND special_category = ?2 AND class_name = ?3 AND team_id = ?4",
            params![season_id, special_category, class_name, team_id],
        )?;
    }
    Ok(updated)
}

fn top_team_ids_from_results(
    conn: &Connection,
    season_id: &str,
    special_category: &str,
    class_name: &str,
) -> Result<Vec<String>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
             r.equipe_id,
             COALESCE(SUM(r.pontos), 0.0) AS total_points,
             SUM(CASE WHEN r.posicao_final = 1 AND r.dnf = 0 THEN 1 ELSE 0 END) AS total_wins,
             t.nome
         FROM race_results r
         INNER JOIN calendar c ON c.id = r.race_id
         INNER JOIN special_team_entries e
           ON e.season_id = COALESCE(c.season_id, c.temporada_id)
          AND e.special_category = c.categoria
          AND e.team_id = r.equipe_id
         INNER JOIN teams t ON t.id = r.equipe_id
         WHERE COALESCE(c.season_id, c.temporada_id) = ?1
           AND c.categoria = ?2
           AND e.class_name = ?3
           AND r.equipe_id <> ''
         GROUP BY r.equipe_id, t.nome
         ORDER BY total_points DESC, total_wins DESC, t.nome ASC",
    )?;
    let rows = stmt.query_map(params![season_id, special_category, class_name], |row| {
        row.get::<_, String>(0)
    })?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

fn hydrate_entry_teams(
    conn: &Connection,
    entries: Vec<SpecialTeamEntry>,
) -> Result<Vec<Team>, DbError> {
    let mut teams = Vec::new();
    for entry in entries {
        if let Some(mut team) = team_queries::get_team_by_id(conn, &entry.team_id)? {
            team.classe = Some(entry.class_name);
            teams.push(team);
        }
    }
    teams.sort_by(|a, b| {
        b.car_performance
            .partial_cmp(&a.car_performance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.nome.cmp(&b.nome))
    });
    Ok(teams)
}

fn collect_entries(
    rows: rusqlite::MappedRows<
        '_,
        impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<SpecialTeamEntry>,
    >,
) -> Result<Vec<SpecialTeamEntry>, DbError> {
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries)
}
