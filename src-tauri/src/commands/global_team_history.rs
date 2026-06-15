use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::commands::career_types::{
    GlobalTeamHistoryBand, GlobalTeamHistoryFamily, GlobalTeamHistoryFamilyBand,
    GlobalTeamHistoryPayload, GlobalTeamHistoryPoint, GlobalTeamHistoryTeamRow,
};
use crate::config::app_config::AppConfig;
use crate::constants::historical_timeline::{
    category_start_year, class_start_year as timeline_class_start_year,
};
use crate::db::connection::Database;

const DEFAULT_FAMILY: &str = "mazda";
const DEFAULT_START_YEAR: i32 = 2000;
const DEFAULT_MAX_YEAR: i32 = 2025;
const DEFAULT_WINDOW_SIZE: i32 = 8;
const MIN_WINDOW_SIZE: i32 = 4;
const MAX_WINDOW_SIZE: i32 = 32;

#[derive(Debug, Clone)]
struct TeamHistoryBandDef {
    key: &'static str,
    label: &'static str,
    category: &'static str,
    class_name: Option<&'static str>,
    is_special: bool,
}

#[derive(Debug, Clone)]
struct TeamHistoryFamilyDef {
    id: &'static str,
    label: &'static str,
    bands: &'static [TeamHistoryBandDef],
}

#[derive(Debug, Clone)]
struct TeamArchiveRow {
    team_id: String,
    nome: String,
    nome_curto: String,
    cor_primaria: String,
    cor_secundaria: String,
    year: i32,
    category: String,
    class_name: Option<String>,
    position: i32,
    points: i32,
    wins: i32,
    titles: i32,
}

const MAZDA_BANDS: [TeamHistoryBandDef; 3] = [
    TeamHistoryBandDef {
        key: "production_mazda",
        label: "Mazda Production",
        category: "production_challenger",
        class_name: Some("mazda"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "mazda_amador",
        label: "Mazda Championship",
        category: "mazda_amador",
        class_name: None,
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "mazda_rookie",
        label: "Mazda Rookie",
        category: "mazda_rookie",
        class_name: None,
        is_special: false,
    },
];

const TOYOTA_BANDS: [TeamHistoryBandDef; 3] = [
    TeamHistoryBandDef {
        key: "production_toyota",
        label: "Toyota Production",
        category: "production_challenger",
        class_name: Some("toyota"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "toyota_amador",
        label: "Toyota Cup",
        category: "toyota_amador",
        class_name: None,
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "toyota_rookie",
        label: "Toyota Rookie",
        category: "toyota_rookie",
        class_name: None,
        is_special: false,
    },
];

const BMW_BANDS: [TeamHistoryBandDef; 2] = [
    TeamHistoryBandDef {
        key: "production_bmw",
        label: "BMW Production",
        category: "production_challenger",
        class_name: Some("bmw"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "bmw_m2",
        label: "BMW M2",
        category: "bmw_m2",
        class_name: None,
        is_special: false,
    },
];

const GT4_BANDS: [TeamHistoryBandDef; 2] = [
    TeamHistoryBandDef {
        key: "endurance_gt4",
        label: "GT4 Endurance",
        category: "endurance",
        class_name: Some("gt4"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "gt4",
        label: "GT4",
        category: "gt4",
        class_name: None,
        is_special: false,
    },
];

const GT3_BANDS: [TeamHistoryBandDef; 2] = [
    TeamHistoryBandDef {
        key: "endurance_gt3",
        label: "GT3 Endurance",
        category: "endurance",
        class_name: Some("gt3"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "gt3",
        label: "GT3",
        category: "gt3",
        class_name: None,
        is_special: false,
    },
];

const LMP2_BANDS: [TeamHistoryBandDef; 1] = [TeamHistoryBandDef {
    key: "endurance_lmp2",
    label: "LMP2",
    category: "endurance",
    class_name: Some("lmp2"),
    is_special: false,
}];

const FAMILY_DEFS: [TeamHistoryFamilyDef; 6] = [
    TeamHistoryFamilyDef {
        id: "mazda",
        label: "Mazda",
        bands: &MAZDA_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "toyota",
        label: "Toyota",
        bands: &TOYOTA_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "bmw",
        label: "BMW",
        bands: &BMW_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "gt4",
        label: "GT4",
        bands: &GT4_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "gt3",
        label: "GT3",
        bands: &GT3_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "lmp2",
        label: "LMP2",
        bands: &LMP2_BANDS,
    },
];

pub(crate) fn get_global_team_history_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    family: Option<&str>,
    start_year: Option<i32>,
    window_size: Option<i32>,
) -> Result<GlobalTeamHistoryPayload, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    build_global_team_history(
        &db.conn,
        family.unwrap_or(DEFAULT_FAMILY),
        start_year.unwrap_or(DEFAULT_START_YEAR),
        window_size.unwrap_or(DEFAULT_WINDOW_SIZE),
    )
}

pub(crate) fn build_global_team_history(
    conn: &Connection,
    family: &str,
    start_year: i32,
    window_size: i32,
) -> Result<GlobalTeamHistoryPayload, String> {
    let family_def = resolve_family(family);
    let window_size = window_size.clamp(MIN_WINDOW_SIZE, MAX_WINDOW_SIZE);
    let (min_year, max_year) = history_year_bounds(conn)?;
    let latest_start = (max_year - window_size + 1).max(min_year);
    let window_start = start_year.clamp(min_year, latest_start);
    let window_end = (window_start + window_size - 1).min(max_year);
    let archive_rows = load_archive_rows(conn, window_start, window_end)?;
    let archive_rows = dedupe_archive_rows_for_family(family_def, archive_rows);
    let bands = family_def
        .bands
        .iter()
        .map(|band| build_band_payload(band, &archive_rows, window_start, window_end))
        .collect::<Vec<_>>();

    Ok(GlobalTeamHistoryPayload {
        selected_family: family_def.id.to_string(),
        min_year,
        max_year,
        window_start,
        window_end,
        window_size,
        families: FAMILY_DEFS.iter().map(family_payload).collect(),
        bands,
    })
}

/// Year the band's (category, class) combination first existed.
///
/// For shared categories like `endurance`, a class can debut after the
/// category itself: the GT4 class only joined endurance in 2002 even though
/// the endurance category started in 2000. The band's start is therefore the
/// later of the two so the "category did not exist" band reflects the class.
fn band_start_year(band: &TeamHistoryBandDef) -> i32 {
    let category_start = category_start_year(band.category);
    match band.class_name {
        Some(class_name) => {
            category_start.max(timeline_class_start_year(band.category, Some(class_name)))
        }
        None => category_start,
    }
}

fn resolve_family(family: &str) -> &'static TeamHistoryFamilyDef {
    FAMILY_DEFS
        .iter()
        .find(|value| value.id == family)
        .unwrap_or(&FAMILY_DEFS[0])
}

fn family_payload(family: &TeamHistoryFamilyDef) -> GlobalTeamHistoryFamily {
    GlobalTeamHistoryFamily {
        id: family.id.to_string(),
        label: family.label.to_string(),
        bands: family.bands.iter().map(family_band_payload).collect(),
    }
}

fn family_band_payload(band: &TeamHistoryBandDef) -> GlobalTeamHistoryFamilyBand {
    GlobalTeamHistoryFamilyBand {
        key: band.key.to_string(),
        label: band.label.to_string(),
        category: band.category.to_string(),
        class_name: band.class_name.map(str::to_string),
        starts_year: band_start_year(band),
        is_special: band.is_special,
    }
}

fn history_year_bounds(conn: &Connection) -> Result<(i32, i32), String> {
    let archive_max = conn
        .query_row("SELECT MAX(ano) FROM team_season_archive", [], |row| {
            row.get::<_, Option<i32>>(0)
        })
        .optional()
        .map_err(|e| format!("Falha ao consultar anos do historico de equipes: {e}"))?
        .flatten();
    let active_year = conn
        .query_row(
            "SELECT ano FROM seasons WHERE status = 'Ativa' ORDER BY numero DESC LIMIT 1",
            [],
            |row| row.get::<_, i32>(0),
        )
        .optional()
        .unwrap_or(None);
    let max_year = archive_max
        .or(active_year)
        .unwrap_or(DEFAULT_MAX_YEAR)
        .max(DEFAULT_MAX_YEAR);
    Ok((DEFAULT_START_YEAR, max_year))
}

fn load_archive_rows(
    conn: &Connection,
    window_start: i32,
    window_end: i32,
) -> Result<Vec<TeamArchiveRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                a.team_id,
                COALESCE(t.nome, a.team_id) AS nome,
                COALESCE(NULLIF(TRIM(t.nome_curto), ''), COALESCE(t.nome, a.team_id)) AS nome_curto,
                COALESCE(NULLIF(TRIM(t.cor_primaria), ''), '#58a6ff') AS cor_primaria,
                COALESCE(NULLIF(TRIM(t.cor_secundaria), ''), '#0d1727') AS cor_secundaria,
                a.ano,
                a.categoria,
                a.classe,
                COALESCE(a.posicao_campeonato, 999) AS posicao_campeonato,
                COALESCE(a.pontos, 0.0) AS pontos,
                COALESCE(a.vitorias, 0) AS vitorias,
                COALESCE(a.titulos_construtores, 0) AS titulos_construtores
             FROM team_season_archive a
             LEFT JOIN teams t ON t.id = a.team_id
             WHERE a.ano BETWEEN ?1 AND ?2
             ORDER BY a.ano ASC, a.categoria ASC, posicao_campeonato ASC, a.team_id ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico mundial de equipes: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![window_start, window_end], |row| {
            Ok(TeamArchiveRow {
                team_id: row.get(0)?,
                nome: row.get(1)?,
                nome_curto: row.get(2)?,
                cor_primaria: row.get(3)?,
                cor_secundaria: row.get(4)?,
                year: row.get(5)?,
                category: row.get(6)?,
                class_name: row.get(7)?,
                position: row.get(8)?,
                points: row.get::<_, f64>(9)?.round() as i32,
                wins: row.get(10)?,
                titles: row.get(11)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar historico mundial de equipes: {e}"))?;

    let mut collected = Vec::new();
    for row in rows {
        collected.push(row.map_err(|e| format!("Falha ao ler historico mundial de equipes: {e}"))?);
    }
    Ok(collected)
}

fn dedupe_archive_rows_for_family(
    family: &TeamHistoryFamilyDef,
    archive_rows: Vec<TeamArchiveRow>,
) -> Vec<TeamArchiveRow> {
    // Visual guard rail: if historical imports ever provide two snapshots for
    // the same team/year in a family, Atlas chooses the highest band instead
    // of showing the team twice. The model should still avoid duplicates.
    let mut selected_by_team_year: HashMap<(String, i32), (usize, usize)> = HashMap::new();

    for (row_index, row) in archive_rows.iter().enumerate() {
        let Some(band_index) = family.bands.iter().position(|band| band_matches(band, row)) else {
            continue;
        };
        let key = (row.team_id.clone(), row.year);
        match selected_by_team_year.get_mut(&key) {
            Some((current_band_index, current_row_index)) => {
                if band_index < *current_band_index {
                    *current_band_index = band_index;
                    *current_row_index = row_index;
                }
            }
            None => {
                selected_by_team_year.insert(key, (band_index, row_index));
            }
        }
    }

    let selected_indices = selected_by_team_year
        .into_values()
        .map(|(_, row_index)| row_index)
        .collect::<std::collections::HashSet<_>>();

    archive_rows
        .into_iter()
        .enumerate()
        .filter_map(|(index, row)| selected_indices.contains(&index).then_some(row))
        .collect()
}

fn build_band_payload(
    band: &TeamHistoryBandDef,
    archive_rows: &[TeamArchiveRow],
    window_start: i32,
    window_end: i32,
) -> GlobalTeamHistoryBand {
    let mut by_team: HashMap<String, Vec<&TeamArchiveRow>> = HashMap::new();
    for row in archive_rows.iter().filter(|row| band_matches(band, row)) {
        by_team.entry(row.team_id.clone()).or_default().push(row);
    }

    let mut rows = by_team
        .into_values()
        .filter_map(|rows| build_team_row(band, rows, window_start, window_end))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.base_position
            .cmp(&right.base_position)
            .then_with(|| left.nome.cmp(&right.nome))
    });

    GlobalTeamHistoryBand {
        key: band.key.to_string(),
        label: band.label.to_string(),
        category: band.category.to_string(),
        class_name: band.class_name.map(str::to_string),
        starts_year: band_start_year(band),
        is_special: band.is_special,
        rows,
    }
}

fn band_matches(band: &TeamHistoryBandDef, row: &TeamArchiveRow) -> bool {
    if row.category != band.category {
        return false;
    }
    match band.class_name {
        Some(class_name) => normalize_opt(row.class_name.as_deref()) == Some(class_name),
        None => true,
    }
}

fn normalize_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn build_team_row(
    band: &TeamHistoryBandDef,
    mut rows: Vec<&TeamArchiveRow>,
    _window_start: i32,
    _window_end: i32,
) -> Option<GlobalTeamHistoryTeamRow> {
    rows.sort_by(|left, right| {
        left.year
            .cmp(&right.year)
            .then_with(|| left.position.cmp(&right.position))
    });
    rows.dedup_by(|left, right| left.year == right.year);
    let first = rows.first()?;
    let last = rows.last().unwrap_or(first);
    let slot = if band.is_special {
        "special"
    } else {
        "regular"
    };
    let points = rows
        .iter()
        .map(|row| GlobalTeamHistoryPoint {
            year: row.year,
            slot: slot.to_string(),
            position: row.position,
            points: row.points,
            wins: row.wins,
            titles: row.titles,
        })
        .collect();

    Some(GlobalTeamHistoryTeamRow {
        team_id: first.team_id.clone(),
        nome: first.nome.clone(),
        nome_curto: first.nome_curto.clone(),
        cor_primaria: first.cor_primaria.clone(),
        cor_secundaria: first.cor_secundaria.clone(),
        base_position: first.position,
        delta: first.position - last.position,
        points,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                nome_curto TEXT NOT NULL DEFAULT '',
                categoria TEXT NOT NULL,
                classe TEXT,
                cor_primaria TEXT NOT NULL DEFAULT '#58a6ff',
                cor_secundaria TEXT NOT NULL DEFAULT '#0d1727'
            );
            CREATE TABLE team_season_archive (
                team_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                categoria TEXT NOT NULL,
                classe TEXT,
                posicao_campeonato INTEGER,
                pontos REAL NOT NULL DEFAULT 0.0,
                vitorias INTEGER NOT NULL DEFAULT 0,
                podios INTEGER NOT NULL DEFAULT 0,
                poles INTEGER NOT NULL DEFAULT 0,
                corridas INTEGER NOT NULL DEFAULT 0,
                titulos_construtores INTEGER NOT NULL DEFAULT 0,
                piloto_1_id TEXT,
                piloto_2_id TEXT,
                snapshot_json TEXT NOT NULL DEFAULT '{}',
                archived_at TEXT NOT NULL DEFAULT ''
            );
            ",
        )
        .expect("schema");
        conn
    }

    fn seed_team_history(conn: &Connection) {
        conn.execute_batch(
            "
            INSERT INTO teams (id, nome, nome_curto, categoria, classe, cor_primaria, cor_secundaria)
            VALUES
                ('T_SUNDAY', 'Sunday Speed Club', 'SSC', 'production_challenger', 'mazda', '#5ee7a8', '#114b5f'),
                ('T_DUAL', 'Dual Exit Racing', 'DXR', 'mazda_amador', NULL, '#ff6b6b', '#70141d');

            INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores
            ) VALUES
                ('T_SUNDAY', 1, 2020, 'mazda_rookie', NULL, 1, 104, 4, 6, 2, 8, 1),
                ('T_SUNDAY', 2, 2021, 'mazda_amador', NULL, 2, 96, 2, 5, 1, 8, 0),
                ('T_SUNDAY', 3, 2022, 'production_challenger', 'mazda', 2, 92, 2, 4, 1, 6, 0),
                ('T_SUNDAY', 4, 2023, 'production_challenger', 'mazda', 1, 108, 3, 5, 2, 6, 1),
                ('T_SUNDAY', 5, 2023, 'mazda_amador', NULL, 1, 118, 4, 5, 2, 8, 1),
                ('T_SUNDAY', 6, 2024, 'mazda_amador', NULL, 3, 74, 0, 2, 0, 8, 0),
                ('T_DUAL', 1, 2020, 'mazda_amador', NULL, 1, 112, 4, 5, 1, 8, 1),
                ('T_DUAL', 2, 2021, 'mazda_amador', NULL, 1, 120, 5, 7, 3, 8, 1);
            ",
        )
        .expect("seed");
    }

    #[test]
    fn build_global_team_history_returns_filtered_family_bands_and_split_slots() {
        let conn = setup_conn();
        seed_team_history(&conn);

        let payload = build_global_team_history(&conn, "mazda", 2020, 4).expect("payload");

        assert_eq!(payload.selected_family, "mazda");
        assert_eq!(payload.window_start, 2020);
        assert_eq!(payload.window_end, 2023);
        assert!(payload.families.iter().any(|family| family.id == "mazda"));
        assert!(payload.families.iter().any(|family| family.id == "lmp2"));

        let production = payload
            .bands
            .iter()
            .find(|band| band.key == "production_mazda")
            .expect("production band");
        assert!(!production.is_special);
        assert_eq!(production.label, "Mazda Production");
        assert_eq!(production.class_name.as_deref(), Some("mazda"));
        assert_eq!(production.rows[0].cor_primaria, "#5ee7a8");
        assert_eq!(production.rows[0].points[0].slot, "regular");

        let cup = payload
            .bands
            .iter()
            .find(|band| band.key == "mazda_amador")
            .expect("cup band");
        assert_eq!(cup.rows[0].points[0].slot, "regular");
        assert_eq!(cup.rows[0].nome, "Dual Exit Racing");
    }

    #[test]
    fn build_global_team_history_labels_real_production_endurance_and_lmp2_bands() {
        let conn = setup_conn();
        seed_team_history(&conn);
        conn.execute_batch(
            "
            INSERT INTO teams (id, nome, nome_curto, categoria, classe, cor_primaria, cor_secundaria)
            VALUES
                ('T_GT3', 'GT3 Team', 'GT3', 'endurance', 'gt3', '#58a6ff', '#0b2545'),
                ('T_GT4', 'GT4 Team', 'GT4', 'endurance', 'gt4', '#5ee7a8', '#114b5f'),
                ('T_LMP2', 'LMP2 Team', 'LMP', 'endurance', 'lmp2', '#f2c46d', '#3a2610');

            INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores
            ) VALUES
                ('T_GT3', 10, 2024, 'endurance', 'gt3', 1, 140, 4, 6, 2, 8, 1),
                ('T_GT4', 10, 2024, 'endurance', 'gt4', 1, 122, 3, 5, 1, 8, 1),
                ('T_LMP2', 10, 2024, 'endurance', 'lmp2', 1, 130, 3, 6, 2, 8, 1);
            ",
        )
        .expect("seed extra families");

        let gt3_payload = build_global_team_history(&conn, "gt3", 2024, 4).expect("gt3");
        let gt3_endurance = gt3_payload
            .bands
            .iter()
            .find(|band| band.key == "endurance_gt3")
            .expect("gt3 endurance");
        assert_eq!(gt3_endurance.label, "GT3 Endurance");
        assert!(!gt3_endurance.is_special);
        assert_eq!(gt3_endurance.starts_year, 2005);

        let gt4_payload = build_global_team_history(&conn, "gt4", 2024, 4).expect("gt4");
        let gt4_endurance = gt4_payload
            .bands
            .iter()
            .find(|band| band.key == "endurance_gt4")
            .expect("gt4 endurance");
        assert_eq!(gt4_endurance.label, "GT4 Endurance");
        assert!(!gt4_endurance.is_special);
        assert_eq!(gt4_endurance.starts_year, 2007);

        let lmp2_payload = build_global_team_history(&conn, "lmp2", 2024, 4).expect("lmp2");
        assert_eq!(lmp2_payload.selected_family, "lmp2");
        assert_eq!(lmp2_payload.bands.len(), 1);
        assert_eq!(lmp2_payload.bands[0].label, "LMP2");
        assert_eq!(lmp2_payload.bands[0].category, "endurance");
        assert_eq!(lmp2_payload.bands[0].class_name.as_deref(), Some("lmp2"));
        assert!(!lmp2_payload.bands[0].is_special);
        assert_eq!(lmp2_payload.bands[0].rows[0].points[0].slot, "regular");
    }

    #[test]
    fn build_global_team_history_has_one_point_per_team_year_across_real_divisions() {
        let conn = setup_conn();
        seed_team_history(&conn);

        let payload = build_global_team_history(&conn, "mazda", 2020, 5).expect("payload");
        let mut seen = std::collections::HashSet::new();
        for band in &payload.bands {
            for row in &band.rows {
                for point in &row.points {
                    assert!(
                        seen.insert((row.team_id.clone(), point.year)),
                        "team {} duplicated in year {}",
                        row.team_id,
                        point.year
                    );
                }
            }
        }

        let production = payload
            .bands
            .iter()
            .find(|band| band.key == "production_mazda")
            .expect("production band");
        let cup = payload
            .bands
            .iter()
            .find(|band| band.key == "mazda_amador")
            .expect("cup band");
        assert!(production.rows.iter().any(|row| {
            row.team_id == "T_SUNDAY" && row.points.iter().any(|point| point.year == 2022)
        }));
        assert!(cup.rows.iter().any(|row| {
            row.team_id == "T_SUNDAY" && row.points.iter().any(|point| point.year == 2024)
        }));
        assert!(!cup.rows.iter().any(|row| {
            row.team_id == "T_SUNDAY" && row.points.iter().any(|point| point.year == 2023)
        }));
    }

    #[test]
    fn atlas_history_source_stays_decoupled_from_legacy_entries() {
        let source = include_str!("global_team_history.rs");
        let legacy_table = concat!("special", "_team", "_entries");

        assert!(
            !source.contains(legacy_table),
            "Atlas global history must not read legacy special entries"
        );
    }

    #[test]
    fn build_global_team_history_collapses_duplicate_team_year_band_snapshots() {
        let conn = setup_conn();
        seed_team_history(&conn);
        conn.execute(
            "INSERT INTO team_season_archive (
                team_id, season_number, ano, categoria, classe, posicao_campeonato,
                pontos, vitorias, podios, poles, corridas, titulos_construtores
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                "T_DUAL",
                99,
                2020,
                "mazda_amador",
                Option::<String>::None,
                3,
                80.0,
                1,
                2,
                0,
                8,
                0
            ],
        )
        .expect("duplicate snapshot");

        let payload = build_global_team_history(&conn, "mazda", 2020, 4).expect("payload");
        let cup = payload
            .bands
            .iter()
            .find(|band| band.key == "mazda_amador")
            .expect("cup band");
        let dual = cup
            .rows
            .iter()
            .find(|row| row.team_id == "T_DUAL")
            .expect("dual row");
        let points_2020 = dual
            .points
            .iter()
            .filter(|point| point.year == 2020)
            .count();

        assert_eq!(points_2020, 1);
        assert_eq!(dual.points[0].position, 1);
    }
}
