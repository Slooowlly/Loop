use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

mod baseline;
mod seed_incidentes;

/// Trava do schema-ouro (só em teste): descreve o schema de forma normalizada e
/// compara com um fixture versionado.
#[cfg(test)]
mod schema_ouro;

use baseline::BASELINE_DDL;
use seed_incidentes::seed_incident_catalog;

// ── Versão atual do schema ────────────────────────────────────────────────────

const CURRENT_VERSION: u32 = 53;

// ── API pública ───────────────────────────────────────────────────────────────

/// Registro declarativo das migrações: `(versão, função)`. ÚNICA fonte de verdade
/// da ordem e do conjunto — adicionar uma migração = UMA linha aqui (e bumpar
/// `CURRENT_VERSION`).
///
/// As 53 migrações incrementais originais foram colapsadas numa baseline única,
/// registrada na versão 53: um banco novo já nasce carimbado como 53 e
/// `run_pending` continua coerente. Saves antigos NÃO são migrados — a baseline
/// só sabe criar um banco do zero.
const MIGRATIONS: &[(u32, fn(&Connection) -> Result<(), DbError>)] = &[(53, migrate_baseline)];

/// Aplica todas as migrações num banco novo (versão 0 → CURRENT_VERSION).
pub fn run_all(conn: &Connection) -> Result<(), DbError> {
    for (version, migrate) in MIGRATIONS {
        migrate(conn)?;
        set_schema_version(conn, *version)?;
    }
    Ok(())
}

/// Aplica apenas as migrações pendentes num banco existente.
pub fn run_pending(conn: &Connection) -> Result<(), DbError> {
    let version = get_schema_version(conn)?;
    for (target, migrate) in MIGRATIONS {
        if version < *target {
            migrate(conn)?;
            set_schema_version(conn, *target)?;
        }
    }
    Ok(())
}

// ── Baseline ──────────────────────────────────────────────────────────────────

/// Cria o schema final inteiro de uma vez e semeia o que o jogo precisa para
/// funcionar: a tabela `meta` (contadores e configuração) e o catálogo de
/// incidentes. Todo o DDL é `IF NOT EXISTS`, então reaplicar é inofensivo.
fn migrate_baseline(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(BASELINE_DDL)?;
    seed_meta(conn)?;
    seed_incident_catalog(conn)?;
    Ok(())
}

// ── Helpers de versão ─────────────────────────────────────────────────────────

pub fn get_schema_version(conn: &Connection) -> Result<u32, DbError> {
    // A tabela meta pode não existir ainda num banco vazio.
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if !exists {
        return Ok(0);
    }

    conn.query_row(
        "SELECT value FROM meta WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    )
    .map_err(DbError::Sqlite)
    .and_then(|v| {
        v.parse::<u32>()
            .map_err(|_| DbError::InvalidData(format!("schema_version invalida em meta: '{v}'")))
    })
}

fn set_schema_version(conn: &Connection, version: u32) -> Result<(), DbError> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![version.to_string()],
    )?;
    Ok(())
}

// ── Seed inicial da tabela meta ───────────────────────────────────────────────

fn seed_meta(conn: &Connection) -> Result<(), DbError> {
    let seeds = [
        ("next_driver_id", "1"),
        ("next_team_id", "1"),
        ("next_season_id", "1"),
        ("next_race_id", "1"),
        ("next_contract_id", "1"),
        ("next_news_id", "1"),
        ("next_rivalry_id", "1"),
        ("next_team_rivalry_id", "1"),
        ("current_season", "1"),
        ("current_year", "2024"),
        ("career_start_year", "2024"),
        ("difficulty", "Normal"),
    ];

    for (key, value) in &seeds {
        conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guarda-corpo do registro declarativo: as versões precisam ser
    /// estritamente crescentes e a última tem de bater com `CURRENT_VERSION`.
    /// Se alguém adicionar uma migração e esquecer de bumpar a versão (ou
    /// registrar fora de ordem), este teste quebra na hora.
    #[test]
    fn migrations_table_is_sorted_and_matches_current_version() {
        for par in MIGRATIONS.windows(2) {
            assert!(
                par[0].0 < par[1].0,
                "MIGRATIONS fora de ordem: {} vem antes de {}",
                par[0].0,
                par[1].0
            );
        }
        assert_eq!(
            MIGRATIONS.last().map(|(v, _)| *v),
            Some(CURRENT_VERSION),
            "última migração registrada deve ser CURRENT_VERSION ({CURRENT_VERSION})"
        );
    }

    #[test]
    fn test_run_pending_rejects_invalid_schema_version_without_replaying_migrations() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', 'quebrada');

            CREATE TABLE race_results (
                id TEXT PRIMARY KEY,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL
            );

            INSERT INTO race_results (id, race_id, piloto_id, equipe_id)
            VALUES ('legacy', 'R001', 'P001', 'T001');
            ",
        )
        .expect("legacy schema");

        let err = run_pending(&conn).expect_err("invalid schema version should fail");
        assert!(
            matches!(err, DbError::InvalidData(_)),
            "expected invalid-data error, got {err:?}"
        );

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM race_results", [], |row| row.get(0))
            .expect("existing race_results should remain untouched");
        assert_eq!(count, 1);
    }

    /// Um banco novo já nasce carimbado na versão atual, e reabrir (run_pending)
    /// não reaplica nada nem quebra.
    #[test]
    fn banco_novo_nasce_na_versao_atual_e_reabrir_e_noop() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        run_all(&conn).expect("schema");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);

        run_pending(&conn).expect("reabrir não deve migrar nada");
        assert_eq!(get_schema_version(&conn).expect("versão"), CURRENT_VERSION);
    }

    /// As seeds da baseline precisam sobreviver ao colapso: sem elas o jogo
    /// abre sem contadores e sem catálogo de incidentes.
    #[test]
    fn baseline_semeia_meta_e_catalogo_de_incidentes() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        run_all(&conn).expect("schema");

        let incidentes: i64 = conn
            .query_row("SELECT COUNT(*) FROM incident_catalog", [], |row| row.get(0))
            .expect("contagem do catálogo");
        assert_eq!(incidentes, 54);

        for chave in ["next_driver_id", "current_year", "difficulty"] {
            let existe: Option<String> = conn
                .query_row("SELECT value FROM meta WHERE key = ?1", [chave], |row| {
                    row.get(0)
                })
                .optional()
                .expect("consulta meta");
            assert!(existe.is_some(), "meta.{chave} não foi semeada");
        }
    }

    #[test]
    fn test_team_season_archive_schema() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        run_all(&conn).expect("schema");

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'team_season_archive'",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(table_count, 1);

        for column in [
            "team_id",
            "season_number",
            "ano",
            "categoria",
            "classe",
            "posicao_campeonato",
            "pontos",
            "vitorias",
            "podios",
            "poles",
            "corridas",
            "titulos_construtores",
            "piloto_1_id",
            "piloto_2_id",
            "snapshot_json",
        ] {
            assert!(
                column_exists(&conn, "team_season_archive", column),
                "missing team_season_archive.{column}"
            );
        }
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info");
        let mut rows = stmt.query([]).expect("rows");
        while let Some(row) = rows.next().expect("row") {
            let name: String = row.get("name").expect("name");
            if name == column {
                return true;
            }
        }
        false
    }
}
