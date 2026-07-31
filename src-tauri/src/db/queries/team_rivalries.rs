#![allow(dead_code)]
//! Camada de persistência da rivalidade entre EQUIPES. Espelha
//! [`crate::db::queries::rivalries`] trocando o par de piloto pelo par de time. A tabela
//! nova NÃO carrega a coluna legada `intensidade` (a percebida é sempre derivada dos dois
//! eixos em tempo de leitura). Ver `db::migrations::migrate_v49`.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;
use crate::models::team_rivalry::{TeamRivalry, TeamRivalryType};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn row_to_team_rivalry(row: &rusqlite::Row) -> rusqlite::Result<TeamRivalry> {
    let tipo_raw: String = row.get(5)?;
    let tipo = match tipo_raw.trim() {
        "Campeonato" => TeamRivalryType::Campeonato,
        "Mercado" => TeamRivalryType::Mercado,
        "Pista" => TeamRivalryType::Pista,
        "Herdada" => TeamRivalryType::Herdada,
        other => {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "TeamRivalryType inválido: '{other}'"
            )))
        }
    };
    Ok(TeamRivalry {
        id: row.get(0)?,
        team1_id: row.get(1)?,
        team2_id: row.get(2)?,
        historical_intensity: row.get(3)?,
        recent_activity: row.get(4)?,
        tipo,
        criado_em: row.get(6)?,
        ultima_atualizacao: row.get(7)?,
        temporada_update: row.get(8)?,
    })
}

const SELECT_COLS: &str = "id, team1_id, team2_id, historical_intensity, recent_activity, \
     tipo, criado_em, ultima_atualizacao, temporada_update";

// ── Leitura ───────────────────────────────────────────────────────────────────

/// Busca a rivalidade pelo par normalizado (`team1_id < team2_id`).
pub fn get_team_rivalry_by_pair(
    conn: &Connection,
    team1_id: &str,
    team2_id: &str,
) -> Result<Option<TeamRivalry>, DbError> {
    let sql =
        format!("SELECT {SELECT_COLS} FROM team_rivalries WHERE team1_id = ?1 AND team2_id = ?2");
    conn.query_row(&sql, params![team1_id, team2_id], row_to_team_rivalry)
        .optional()
        .map_err(DbError::from)
}

/// Retorna todas as rivalidades de um time, ordenadas por percebida decrescente.
pub fn get_team_rivalries_for_team(
    conn: &Connection,
    team_id: &str,
) -> Result<Vec<TeamRivalry>, DbError> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM team_rivalries
         WHERE team1_id = ?1 OR team2_id = ?1
         ORDER BY (historical_intensity * 0.4 + recent_activity * 0.6) DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let iter = stmt.query_map(params![team_id], row_to_team_rivalry)?;
    let mut out = Vec::new();
    for r in iter {
        out.push(r?);
    }
    Ok(out)
}

/// Retorna todas as rivalidades de equipe (usado no decaimento de fim de temporada).
pub fn get_all_team_rivalries(conn: &Connection) -> Result<Vec<TeamRivalry>, DbError> {
    let sql = format!("SELECT {SELECT_COLS} FROM team_rivalries");
    let mut stmt = conn.prepare(&sql)?;
    let iter = stmt.query_map([], row_to_team_rivalry)?;
    let mut out = Vec::new();
    for r in iter {
        out.push(r?);
    }
    Ok(out)
}

// ── Escrita ───────────────────────────────────────────────────────────────────

/// Insere uma rivalidade de equipe nova. Exige par normalizado (`team1_id < team2_id`).
pub fn insert_team_rivalry(conn: &Connection, rivalry: &TeamRivalry) -> Result<(), DbError> {
    if rivalry.team1_id == rivalry.team2_id {
        return Err(DbError::InvalidData(
            "rivalidade de equipe invalida: team1_id e team2_id nao podem ser iguais".to_string(),
        ));
    }
    if rivalry.team1_id > rivalry.team2_id {
        return Err(DbError::InvalidData(
            "rivalidade de equipe invalida: par deve estar normalizado (team1_id < team2_id)"
                .to_string(),
        ));
    }
    conn.execute(
        "INSERT INTO team_rivalries
             (id, team1_id, team2_id, historical_intensity,
              recent_activity, tipo, criado_em, ultima_atualizacao, temporada_update)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            rivalry.id,
            rivalry.team1_id,
            rivalry.team2_id,
            rivalry.historical_intensity,
            rivalry.recent_activity,
            rivalry.tipo.as_str(),
            rivalry.criado_em,
            rivalry.ultima_atualizacao,
            rivalry.temporada_update,
        ],
    )?;
    Ok(())
}

/// Atualiza os dois eixos e o timestamp de uma rivalidade existente. O tipo original é
/// preservado (identidade narrativa).
pub fn update_team_rivalry_axes(
    conn: &Connection,
    id: &str,
    historical_intensity: f64,
    recent_activity: f64,
    ultima_atualizacao: &str,
    temporada_update: i32,
) -> Result<(), DbError> {
    let affected = conn.execute(
        "UPDATE team_rivalries
         SET historical_intensity = ?1, recent_activity = ?2,
             ultima_atualizacao = ?3, temporada_update = ?4
         WHERE id = ?5",
        params![
            historical_intensity,
            recent_activity,
            ultima_atualizacao,
            temporada_update,
            id,
        ],
    )?;
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "rivalidade de equipe nao encontrada: {id}"
        )));
    }
    Ok(())
}

/// Remove uma rivalidade de equipe pelo id.
pub fn delete_team_rivalry(conn: &Connection, id: &str) -> Result<(), DbError> {
    let affected = conn.execute("DELETE FROM team_rivalries WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(DbError::NotFound(format!(
            "rivalidade de equipe nao encontrada: {id}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        // A v49 criou team_rivalries com FOREIGN KEY para teams(id), então as
        // equipes do par precisam existir antes de qualquer insert.
        conn.execute_batch(
            "INSERT INTO teams (id, nome, categoria) VALUES
                ('T001', 'Equipe 1', 'gt3'),
                ('T002', 'Equipe 2', 'gt3'),
                ('T005', 'Equipe 5', 'gt3'),
                ('T010', 'Equipe 10', 'gt3');",
        )
        .unwrap();
        conn
    }

    fn sample_rivalry() -> TeamRivalry {
        TeamRivalry {
            id: "TRV001".to_string(),
            team1_id: "T001".to_string(),
            team2_id: "T002".to_string(),
            historical_intensity: 20.0,
            recent_activity: 30.0,
            tipo: TeamRivalryType::Campeonato,
            criado_em: "2026-01-01T00:00:00".to_string(),
            ultima_atualizacao: "2026-01-01T00:00:00".to_string(),
            temporada_update: 1,
        }
    }

    #[test]
    fn insert_e_leitura_por_par() {
        let conn = setup_db();
        insert_team_rivalry(&conn, &sample_rivalry()).unwrap();
        let loaded = get_team_rivalry_by_pair(&conn, "T001", "T002")
            .unwrap()
            .expect("deveria existir");
        assert_eq!(loaded.id, "TRV001");
        assert_eq!(loaded.tipo, TeamRivalryType::Campeonato);
        assert!((loaded.historical_intensity - 20.0).abs() < 1e-9);
    }

    #[test]
    fn insert_rejeita_par_nao_normalizado() {
        let conn = setup_db();
        let mut r = sample_rivalry();
        r.team1_id = "T010".to_string();
        r.team2_id = "T002".to_string();
        let err = insert_team_rivalry(&conn, &r).expect_err("par nao normalizado deve falhar");
        assert!(matches!(err, DbError::InvalidData(_)));
    }

    #[test]
    fn insert_rejeita_par_igual() {
        let conn = setup_db();
        let mut r = sample_rivalry();
        r.team1_id = "T005".to_string();
        r.team2_id = "T005".to_string();
        let err = insert_team_rivalry(&conn, &r).expect_err("par igual deve falhar");
        assert!(matches!(err, DbError::InvalidData(_)));
    }

    #[test]
    fn tabela_rejeita_par_duplicado() {
        let conn = setup_db();
        insert_team_rivalry(&conn, &sample_rivalry()).unwrap();
        let mut dup = sample_rivalry();
        dup.id = "TRV002".to_string();
        let res = insert_team_rivalry(&conn, &dup);
        assert!(res.is_err(), "par duplicado nao deve ser permitido");
    }

    #[test]
    fn leitura_por_time_pega_os_dois_lados() {
        let conn = setup_db();
        insert_team_rivalry(&conn, &sample_rivalry()).unwrap();
        assert_eq!(get_team_rivalries_for_team(&conn, "T001").unwrap().len(), 1);
        assert_eq!(get_team_rivalries_for_team(&conn, "T002").unwrap().len(), 1);
        assert_eq!(get_team_rivalries_for_team(&conn, "T999").unwrap().len(), 0);
    }

    #[test]
    fn update_axes_not_found_para_id_inexistente() {
        let conn = setup_db();
        let err = update_team_rivalry_axes(&conn, "TRV404", 10.0, 10.0, "2026-01-01T00:00:00", 1)
            .expect_err("id inexistente deve falhar");
        assert!(matches!(err, DbError::NotFound(_)));
    }

    #[test]
    fn delete_not_found_para_id_inexistente() {
        let conn = setup_db();
        let err = delete_team_rivalry(&conn, "TRV404").expect_err("id inexistente deve falhar");
        assert!(matches!(err, DbError::NotFound(_)));
    }

    #[test]
    fn get_by_pair_erro_para_tipo_invalido() {
        let conn = setup_db();
        insert_team_rivalry(&conn, &sample_rivalry()).unwrap();
        conn.execute(
            "UPDATE team_rivalries SET tipo = 'quebrado' WHERE id = 'TRV001'",
            [],
        )
        .unwrap();
        let err =
            get_team_rivalry_by_pair(&conn, "T001", "T002").expect_err("tipo invalido deve falhar");
        assert!(err.to_string().contains("TeamRivalryType"));
    }
}
