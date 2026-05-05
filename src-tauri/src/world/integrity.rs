use rusqlite::{params, Connection, OptionalExtension};

use crate::constants::categories::{get_all_categories, get_category_config, is_especial};
use crate::constants::historical_timeline::is_category_active_in_year;
use crate::models::license::driver_has_required_license_for_category;

pub const MIN_HISTORICAL_RESULT_SEASONS: i64 = 1;
pub const MIN_HISTORICAL_RESULTS_PER_EXISTING_CATEGORY: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldAuditSeverity {
    Error,
    #[allow(dead_code)]
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldAuditIssue {
    pub code: String,
    pub message: String,
    pub severity: WorldAuditSeverity,
}

#[derive(Debug, Clone, Default)]
pub struct WorldAuditReport {
    pub errors: Vec<WorldAuditIssue>,
    #[allow(dead_code)]
    pub warnings: Vec<WorldAuditIssue>,
}

impl WorldAuditReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn audit_historical_world(
    conn: &Connection,
    playable_year: i32,
) -> Result<WorldAuditReport, String> {
    let mut report = WorldAuditReport::default();

    audit_active_season(conn, playable_year, &mut report)?;
    audit_player_absent(conn, &mut report)?;
    audit_playable_calendar(conn, &mut report)?;
    audit_historical_results(conn, playable_year, &mut report)?;
    audit_driver_archive(conn, playable_year, &mut report)?;
    audit_active_contracts(conn, playable_year, &mut report)?;
    audit_retired_snapshots(conn, &mut report)?;
    audit_meta_counters(conn, &mut report)?;

    Ok(report)
}

fn audit_active_season(
    conn: &Connection,
    playable_year: i32,
    report: &mut WorldAuditReport,
) -> Result<(), String> {
    let active_count = count(
        conn,
        "SELECT COUNT(*) FROM seasons WHERE status IN ('EmAndamento', 'Ativa')",
        [],
    )?;
    if active_count != 1 {
        report.error(
            "active_season_count",
            format!("Esperada 1 temporada ativa, encontradas {active_count}."),
        );
        return Ok(());
    }

    let active_year: i32 = conn
        .query_row(
            "SELECT ano FROM seasons WHERE status IN ('EmAndamento', 'Ativa') LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("Falha ao ler ano da temporada ativa: {e}"))?;
    if active_year != playable_year {
        report.error(
            "active_season_year",
            format!("Ano jogavel esperado {playable_year}, encontrado {active_year}."),
        );
    }
    Ok(())
}

fn audit_player_absent(conn: &Connection, report: &mut WorldAuditReport) -> Result<(), String> {
    let player_count = count(
        conn,
        "SELECT COUNT(*) FROM drivers WHERE is_jogador = 1",
        [],
    )?;
    if player_count > 0 {
        report.error(
            "player_present_before_finalization",
            "O jogador nao pode existir antes da finalizacao do draft.",
        );
    }
    Ok(())
}

fn audit_playable_calendar(conn: &Connection, report: &mut WorldAuditReport) -> Result<(), String> {
    let pending_count = count(
        conn,
        "SELECT COUNT(*)
         FROM calendar c
         JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
         WHERE s.status IN ('EmAndamento', 'Ativa')
           AND c.status = 'Pendente'",
        [],
    )?;
    if pending_count == 0 {
        report.error(
            "missing_playable_calendar",
            "Temporada jogavel nao possui calendario pendente.",
        );
    }
    Ok(())
}

fn audit_historical_results(
    conn: &Connection,
    playable_year: i32,
    report: &mut WorldAuditReport,
) -> Result<(), String> {
    let historical_result_seasons = count(
        conn,
        "SELECT COUNT(DISTINCT s.numero)
         FROM race_results rr
         JOIN calendar c ON c.id = rr.race_id
         JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
         WHERE s.ano < ?1",
        params![playable_year],
    )?;
    if historical_result_seasons < MIN_HISTORICAL_RESULT_SEASONS {
        report.error(
            "insufficient_historical_result_seasons",
            format!(
                "Resultados historicos cobrem {historical_result_seasons} temporada(s); minimo {MIN_HISTORICAL_RESULT_SEASONS}."
            ),
        );
    }

    let categories_below_threshold =
        active_historical_categories_below_result_threshold(conn, playable_year)?;
    if categories_below_threshold > 0 {
        report.error(
            "insufficient_historical_results_by_category",
            format!("{categories_below_threshold} categoria(s) historicas nao possuem resultados suficientes."),
        );
    }

    Ok(())
}

fn active_historical_categories_below_result_threshold(
    conn: &Connection,
    playable_year: i32,
) -> Result<i64, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.categoria, s.ano, COUNT(rr.id) AS result_count
             FROM calendar c
             JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             LEFT JOIN race_results rr ON rr.race_id = c.id
             WHERE s.ano < ?1
               AND c.status = 'Concluida'
             GROUP BY c.categoria, s.ano",
        )
        .map_err(|e| format!("Falha ao preparar auditoria de resultados por categoria: {e}"))?;
    let rows = stmt
        .query_map(params![playable_year], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar resultados por categoria: {e}"))?;

    let mut below = 0_i64;
    for row in rows {
        let (category, year, result_count) =
            row.map_err(|e| format!("Falha ao mapear resultados por categoria: {e}"))?;
        if is_category_active_in_year(&category, year)
            && result_count < MIN_HISTORICAL_RESULTS_PER_EXISTING_CATEGORY
        {
            below += 1;
        }
    }
    Ok(below)
}

fn audit_driver_archive(
    conn: &Connection,
    playable_year: i32,
    report: &mut WorldAuditReport,
) -> Result<(), String> {
    let missing_archive = count(
        conn,
        "SELECT COUNT(*)
         FROM (
             SELECT DISTINCT rr.piloto_id
             FROM race_results rr
             JOIN calendar c ON c.id = rr.race_id
             JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE s.ano < ?1
         ) veteran
         WHERE NOT EXISTS (
             SELECT 1 FROM driver_season_archive dsa
             WHERE dsa.piloto_id = veteran.piloto_id
         )",
        params![playable_year],
    )?;
    if missing_archive > 0 {
        report.error(
            "veteran_without_driver_archive",
            format!("{missing_archive} piloto(s) com resultado historico nao possuem driver_season_archive."),
        );
    }
    Ok(())
}

fn audit_active_contracts(
    conn: &Connection,
    playable_year: i32,
    report: &mut WorldAuditReport,
) -> Result<(), String> {
    let active_categories = active_regular_categories_for_year(playable_year);
    let category_filter = active_category_filter(&active_categories);
    let broken_contracts = count(
        conn,
        &format!(
            "SELECT COUNT(*)
         FROM contracts c
         LEFT JOIN drivers d ON d.id = c.piloto_id
         LEFT JOIN teams t ON t.id = c.equipe_id
         WHERE c.status = 'Ativo'
           AND c.tipo = 'Regular'
           {category_filter}
           AND (d.id IS NULL OR t.id IS NULL)"
        ),
        rusqlite::params_from_iter(active_categories.iter()),
    )?;
    if broken_contracts > 0 {
        report.error(
            "active_contract_with_missing_reference",
            format!(
                "{broken_contracts} contrato(s) ativo(s) apontam para equipe/piloto inexistente."
            ),
        );
    }

    let incomplete_teams = count(
        conn,
        &format!(
            "SELECT COUNT(DISTINCT t.id)
         FROM teams t
         JOIN contracts c ON c.equipe_id = t.id
         WHERE t.ativa = 1
           AND c.status = 'Ativo'
           AND c.tipo = 'Regular'
           {category_filter}
           AND (t.piloto_1_id IS NULL OR t.piloto_2_id IS NULL)"
        ),
        rusqlite::params_from_iter(active_categories.iter()),
    )?;
    if incomplete_teams > 0 {
        let sample = incomplete_team_sample(conn, &active_categories)?;
        report.error(
            "active_team_without_two_drivers",
            format!(
                "{incomplete_teams} equipe(s) ativa(s) com contrato regular nao possuem N1/N2. Exemplos: {sample}."
            ),
        );
    }

    let missing_category = count(
        conn,
        &format!(
            "SELECT COUNT(*)
         FROM contracts c
         JOIN drivers d ON d.id = c.piloto_id
         WHERE c.status = 'Ativo'
           AND c.tipo = 'Regular'
           {category_filter}
           AND (d.categoria_atual IS NULL OR TRIM(d.categoria_atual) = '')"
        ),
        rusqlite::params_from_iter(active_categories.iter()),
    )?;
    if missing_category > 0 {
        report.error(
            "active_driver_without_category",
            format!(
                "{missing_category} piloto(s) ativo(s) contratado(s) nao possuem categoria atual."
            ),
        );
    }

    audit_required_licenses(conn, &active_categories, report)?;
    Ok(())
}

fn incomplete_team_sample(
    conn: &Connection,
    active_categories: &[String],
) -> Result<String, String> {
    let category_filter = active_category_filter(active_categories);
    conn.query_row(
        &format!(
            "SELECT COALESCE(GROUP_CONCAT(team_label, ', '), 'indisponivel')
             FROM (
                 SELECT DISTINCT t.id || '/' || t.categoria AS team_label
                 FROM teams t
                 JOIN contracts c ON c.equipe_id = t.id
                 WHERE t.ativa = 1
                   AND c.status = 'Ativo'
                   AND c.tipo = 'Regular'
                   {category_filter}
                   AND (t.piloto_1_id IS NULL OR t.piloto_2_id IS NULL)
                 ORDER BY t.id ASC
                 LIMIT 5
             )"
        ),
        rusqlite::params_from_iter(active_categories.iter()),
        |row| row.get::<_, String>(0),
    )
    .map_err(|e| format!("Falha ao detalhar equipes incompletas: {e}"))
}

fn audit_required_licenses(
    conn: &Connection,
    active_categories: &[String],
    report: &mut WorldAuditReport,
) -> Result<(), String> {
    let category_filter = active_category_filter(active_categories);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT DISTINCT c.piloto_id, c.categoria
             FROM contracts c
             JOIN drivers d ON d.id = c.piloto_id
             WHERE c.status = 'Ativo'
               AND c.tipo = 'Regular'
               {category_filter}
               AND TRIM(c.categoria) <> ''",
        ))
        .map_err(|e| format!("Falha ao preparar auditoria de licencas: {e}"))?;
    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(active_categories.iter()),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|e| format!("Falha ao consultar licencas: {e}"))?;

    let mut missing = 0;
    for row in rows {
        let (driver_id, category_id) = row.map_err(|e| format!("Falha ao mapear licenca: {e}"))?;
        if get_category_config(&category_id)
            .and_then(|config| config.licenca_necessaria)
            .is_none()
        {
            continue;
        }
        if !driver_has_required_license_for_category(conn, &driver_id, &category_id)? {
            missing += 1;
        }
    }

    if missing > 0 {
        report.error(
            "active_driver_without_required_license",
            format!("{missing} piloto(s) ativo(s) nao possuem licenca exigida."),
        );
    }
    Ok(())
}

fn active_regular_categories_for_year(playable_year: i32) -> Vec<String> {
    get_all_categories()
        .iter()
        .filter(|category| !is_especial(category.id))
        .filter(|category| is_category_active_in_year(category.id, playable_year))
        .map(|category| category.id.to_string())
        .collect()
}

fn active_category_filter(active_categories: &[String]) -> String {
    if active_categories.is_empty() {
        "AND 1 = 0".to_string()
    } else {
        format!(
            "AND c.categoria IN ({})",
            vec!["?"; active_categories.len()].join(", ")
        )
    }
}

fn audit_retired_snapshots(conn: &Connection, report: &mut WorldAuditReport) -> Result<(), String> {
    let invalid_retired = count(
        conn,
        "SELECT COUNT(*)
         FROM retired
         WHERE TRIM(temporada_aposentadoria) = ''
            OR TRIM(categoria_final) = ''
            OR TRIM(estatisticas) = ''
            OR TRIM(estatisticas) = '{}'",
        [],
    )?;
    if invalid_retired > 0 {
        report.error(
            "invalid_retired_snapshot",
            format!("{invalid_retired} aposentado(s) sem ano, categoria final ou estatisticas."),
        );
    }

    let active_retired = count(
        conn,
        "SELECT COUNT(*)
         FROM retired r
         JOIN contracts c ON c.piloto_id = r.piloto_id
         WHERE c.status = 'Ativo' AND c.tipo = 'Regular'",
        [],
    )?;
    if active_retired > 0 {
        report.error(
            "retired_driver_with_active_contract",
            format!("{active_retired} aposentado(s) possuem contrato regular ativo."),
        );
    }
    Ok(())
}

fn audit_meta_counters(conn: &Connection, report: &mut WorldAuditReport) -> Result<(), String> {
    for (key, prefix, tables) in [
        ("next_driver_id", "P", &["drivers"][..]),
        ("next_team_id", "T", &["teams"][..]),
        ("next_season_id", "S", &["seasons"][..]),
        ("next_race_id", "R", &["calendar", "races"][..]),
        ("next_contract_id", "C", &["contracts"][..]),
    ] {
        let stored = meta_counter(conn, key)?;
        let observed = observed_next_counter(conn, prefix, tables)?;
        if stored < observed {
            report.error(
                "stale_meta_next_id",
                format!("{key}={stored} esta abaixo do proximo ID observado {observed}."),
            );
        }
    }
    Ok(())
}

fn meta_counter(conn: &Connection, key: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao ler meta {key}: {e}"))?
    .ok_or_else(|| format!("Meta obrigatoria ausente: {key}"))
}

fn observed_next_counter(conn: &Connection, prefix: &str, tables: &[&str]) -> Result<i64, String> {
    let mut observed = 1_i64;
    for table in tables {
        let sql = format!("SELECT id FROM {table}");
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("Falha ao preparar leitura de IDs em {table}: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("Falha ao listar IDs em {table}: {e}"))?;
        for row in rows {
            let id = row.map_err(|e| format!("Falha ao mapear ID em {table}: {e}"))?;
            if let Some(value) = parse_canonical_id(&id, prefix) {
                observed = observed.max(value + 1);
            }
        }
    }
    Ok(observed)
}

fn parse_canonical_id(id: &str, prefix: &str) -> Option<i64> {
    let suffix = id.strip_prefix(prefix)?;
    if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    suffix.parse::<i64>().ok()
}

fn count<P>(conn: &Connection, sql: &str, params: P) -> Result<i64, String>
where
    P: rusqlite::Params,
{
    conn.query_row(sql, params, |row| row.get(0))
        .map_err(|e| format!("Falha ao executar auditoria: {e}; SQL: {sql}"))
}

impl WorldAuditReport {
    fn error(&mut self, code: &str, message: impl Into<String>) {
        self.errors.push(WorldAuditIssue {
            code: code.to_string(),
            message: message.into(),
            severity: WorldAuditSeverity::Error,
        });
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::migrations;

    #[test]
    fn audit_accepts_complete_historical_draft_shape() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);

        let report = audit_historical_world(&conn, 2025).expect("audit should run");

        assert!(report.is_valid(), "{report:?}");
        assert!(report.errors.is_empty());
    }

    #[test]
    fn audit_rejects_player_before_finalization() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);
        conn.execute("UPDATE drivers SET is_jogador = 1 WHERE id = 'P001'", [])
            .expect("mark player");

        let report = audit_historical_world(&conn, 2025).expect("audit");

        assert_error(&report, "player_present_before_finalization");
    }

    #[test]
    fn audit_rejects_missing_historical_race_results() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);
        conn.execute("DELETE FROM race_results", [])
            .expect("delete race results");

        let report = audit_historical_world(&conn, 2025).expect("audit");

        assert_error(&report, "insufficient_historical_result_seasons");
    }

    #[test]
    fn audit_rejects_veteran_without_driver_archive() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);
        conn.execute("DELETE FROM driver_season_archive", [])
            .expect("delete archive");

        let report = audit_historical_world(&conn, 2025).expect("audit");

        assert_error(&report, "veteran_without_driver_archive");
    }

    #[test]
    fn audit_rejects_active_team_without_two_drivers() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);
        conn.execute("UPDATE teams SET piloto_2_id = NULL WHERE id = 'T001'", [])
            .expect("remove n2");

        let report = audit_historical_world(&conn, 2025).expect("audit");

        assert_error(&report, "active_team_without_two_drivers");
    }

    #[test]
    fn audit_rejects_retired_snapshot_without_retirement_year() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);
        conn.execute(
            "INSERT INTO retired (
                piloto_id, nome, temporada_aposentadoria, categoria_final, estatisticas, motivo
             ) VALUES ('P099', 'Retired Driver', '', 'mazda_rookie', '{\"corridas\":1}', 'Teste')",
            [],
        )
        .expect("insert invalid retired");

        let report = audit_historical_world(&conn, 2025).expect("audit");

        assert_error(&report, "invalid_retired_snapshot");
    }

    #[test]
    fn audit_rejects_stale_meta_next_ids() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);
        conn.execute(
            "UPDATE meta SET value = '2' WHERE key = 'next_driver_id'",
            [],
        )
        .expect("stale counter");

        let report = audit_historical_world(&conn, 2025).expect("audit");

        assert_error(&report, "stale_meta_next_id");
    }

    #[test]
    fn audit_uses_named_historical_result_sufficiency_rule() {
        let conn = setup_integrity_conn();
        seed_complete_minimal_world(&conn);
        conn.execute("DELETE FROM race_results WHERE race_id = 'R001'", [])
            .expect("remove historical result");

        let report = audit_historical_world(&conn, 2025).expect("audit");

        assert_error(&report, "insufficient_historical_result_seasons");
    }

    fn setup_integrity_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");
        conn
    }

    fn seed_complete_minimal_world(conn: &Connection) {
        conn.execute_batch(
            "
            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES
                ('S001', 1, 2024, 'Finalizada', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
                ('S002', 2, 2025, 'EmAndamento', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

            INSERT INTO drivers (id, nome, idade, nacionalidade, genero, categoria_atual, status, ano_inicio_carreira)
            VALUES
                ('P001', 'Piloto Um', 25, 'Brasil', 'M', 'mazda_rookie', 'Ativo', 2020),
                ('P002', 'Piloto Dois', 24, 'Brasil', 'M', 'mazda_rookie', 'Ativo', 2021);

            INSERT INTO teams (
                id, nome, nome_curto, categoria, ativa, piloto_1_id, piloto_2_id,
                hierarquia_n1_id, hierarquia_n2_id, created_at, updated_at
            ) VALUES (
                'T001', 'Equipe Um', 'E1', 'mazda_rookie', 1, 'P001', 'P002',
                'P001', 'P002', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
            );

            INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, status, papel,
                salario, salario_anual, temporada_inicio, temporada_fim, tipo, categoria, created_at
            ) VALUES
                ('C001', 'P001', 'Piloto Um', 'T001', 'Equipe Um', 'Ativo', 'Numero1', 1000, 1000, '2', '3', 'Regular', 'mazda_rookie', CURRENT_TIMESTAMP),
                ('C002', 'P002', 'Piloto Dois', 'T001', 'Equipe Um', 'Ativo', 'Numero2', 1000, 1000, '2', '3', 'Regular', 'mazda_rookie', CURRENT_TIMESTAMP);

            INSERT INTO calendar (
                id, temporada_id, season_id, rodada, pista, categoria, status, nome,
                track_name, track_config
            ) VALUES
                ('R001', 'S001', 'S001', 1, 'Laguna Seca', 'mazda_rookie', 'Concluida', 'R1', 'Laguna Seca', 'default'),
                ('R002', 'S002', 'S002', 1, 'Road Atlanta', 'mazda_rookie', 'Pendente', 'R1', 'Road Atlanta', 'default');

            INSERT INTO races (id, temporada_id, calendar_id, rodada, pista, data, clima, status)
            VALUES ('R001', 'S001', 'R001', 1, 'Laguna Seca', '2024-01-01', 'Seco', 'Concluida');

            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_largada, posicao_final, voltas_completadas, pontos)
            VALUES
                ('R001', 'P001', 'T001', 1, 1, 10, 25.0),
                ('R001', 'P002', 'T001', 2, 2, 10, 18.0);

            INSERT INTO standings (temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas)
            VALUES
                ('S001', 'P001', 'T001', 'mazda_rookie', 1, 25.0, 1, 1, 1, 1),
                ('S001', 'P002', 'T001', 'mazda_rookie', 2, 18.0, 0, 1, 0, 1);

            INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES
                ('P001', 1, 2024, 'Piloto Um', 'mazda_rookie', 1, 25.0, '{\"corridas\":1}'),
                ('P002', 1, 2024, 'Piloto Dois', 'mazda_rookie', 2, 18.0, '{\"corridas\":1}');

            UPDATE meta SET value = '3' WHERE key = 'next_driver_id';
            UPDATE meta SET value = '2' WHERE key = 'next_team_id';
            UPDATE meta SET value = '3' WHERE key = 'next_season_id';
            UPDATE meta SET value = '3' WHERE key = 'next_race_id';
            UPDATE meta SET value = '3' WHERE key = 'next_contract_id';
            UPDATE meta SET value = '2' WHERE key = 'current_season';
            UPDATE meta SET value = '2025' WHERE key = 'current_year';
            ",
        )
        .expect("seed minimal world");
    }

    fn assert_error(report: &WorldAuditReport, code: &str) {
        assert!(
            report.errors.iter().any(|issue| issue.code == code),
            "expected error {code}, got {:?}",
            report.errors
        );
    }
}
