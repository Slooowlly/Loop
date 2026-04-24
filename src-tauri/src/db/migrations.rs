use rusqlite::{Connection, OptionalExtension};

use crate::db::connection::DbError;

// ── Versão atual do schema ────────────────────────────────────────────────────

const CURRENT_VERSION: u32 = 27;

// ── API pública ───────────────────────────────────────────────────────────────

/// Aplica todas as migrações num banco novo (versão 0 → CURRENT_VERSION).
pub fn run_all(conn: &Connection) -> Result<(), DbError> {
    migrate_v1(conn)?;
    migrate_v2(conn)?;
    migrate_v3(conn)?;
    migrate_v4(conn)?;
    migrate_v5(conn)?;
    migrate_v6(conn)?;
    migrate_v7(conn)?;
    migrate_v8(conn)?;
    migrate_v9(conn)?;
    migrate_v10(conn)?;
    migrate_v11(conn)?;
    migrate_v12(conn)?;
    migrate_v13(conn)?;
    migrate_v14(conn)?;
    migrate_v15(conn)?;
    migrate_v16(conn)?;
    migrate_v17(conn)?;
    migrate_v18(conn)?;
    migrate_v19(conn)?;
    migrate_v20(conn)?;
    migrate_v21(conn)?;
    migrate_v22(conn)?;
    migrate_v23(conn)?;
    migrate_v24(conn)?;
    migrate_v25(conn)?;
    migrate_v26(conn)?;
    migrate_v27(conn)?;
    set_schema_version(conn, CURRENT_VERSION)?;
    Ok(())
}

/// Aplica apenas as migrações pendentes num banco existente.
pub fn run_pending(conn: &Connection) -> Result<(), DbError> {
    let version = get_schema_version(conn)?;
    if version < 1 {
        migrate_v1(conn)?;
        set_schema_version(conn, 1)?;
    }
    if version < 2 {
        migrate_v2(conn)?;
        set_schema_version(conn, 2)?;
    }
    if version < 3 {
        migrate_v3(conn)?;
        set_schema_version(conn, 3)?;
    }
    if version < 4 {
        migrate_v4(conn)?;
        set_schema_version(conn, 4)?;
    }
    if version < 5 {
        migrate_v5(conn)?;
        set_schema_version(conn, 5)?;
    }
    if version < 6 {
        migrate_v6(conn)?;
        set_schema_version(conn, 6)?;
    }
    if version < 7 {
        migrate_v7(conn)?;
        set_schema_version(conn, 7)?;
    }
    if version < 8 {
        migrate_v8(conn)?;
        set_schema_version(conn, 8)?;
    }
    if version < 9 {
        migrate_v9(conn)?;
        set_schema_version(conn, 9)?;
    }
    if version < 10 {
        migrate_v10(conn)?;
        set_schema_version(conn, 10)?;
    }
    if version < 11 {
        migrate_v11(conn)?;
        set_schema_version(conn, 11)?;
    }
    if version < 12 {
        migrate_v12(conn)?;
        set_schema_version(conn, 12)?;
    }
    if version < 13 {
        migrate_v13(conn)?;
        set_schema_version(conn, 13)?;
    }
    if version < 14 {
        migrate_v14(conn)?;
        set_schema_version(conn, 14)?;
    }
    if version < 15 {
        migrate_v15(conn)?;
        set_schema_version(conn, 15)?;
    }
    if version < 16 {
        migrate_v16(conn)?;
        set_schema_version(conn, 16)?;
    }
    if version < 17 {
        migrate_v17(conn)?;
        set_schema_version(conn, 17)?;
    }
    if version < 18 {
        migrate_v18(conn)?;
        set_schema_version(conn, 18)?;
    }
    if version < 19 {
        migrate_v19(conn)?;
        set_schema_version(conn, 19)?;
    }
    if version < 20 {
        migrate_v20(conn)?;
        set_schema_version(conn, 20)?;
    }
    if version < 21 {
        migrate_v21(conn)?;
        set_schema_version(conn, 21)?;
    }
    if version < 22 {
        migrate_v22(conn)?;
        set_schema_version(conn, 22)?;
    }
    if version < 23 {
        migrate_v23(conn)?;
        set_schema_version(conn, 23)?;
    }
    if version < 24 {
        migrate_v24(conn)?;
        set_schema_version(conn, 24)?;
    }
    if version < 25 {
        migrate_v25(conn)?;
        set_schema_version(conn, 25)?;
    }
    if version < 26 {
        migrate_v26(conn)?;
        set_schema_version(conn, 26)?;
    }
    if version < 27 {
        migrate_v27(conn)?;
        set_schema_version(conn, 27)?;
    }
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

// ── Migração v1 — schema completo ─────────────────────────────────────────────

fn migrate_v1(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(MIGRATION_V1_DDL)?;
    seed_meta(conn)?;
    Ok(())
}

fn migrate_v2(conn: &Connection) -> Result<(), DbError> {
    ensure_column(conn, "teams", "nome_curto", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(
        conn,
        "teams",
        "cor_primaria",
        "TEXT NOT NULL DEFAULT '#FFFFFF'",
    )?;
    ensure_column(
        conn,
        "teams",
        "cor_secundaria",
        "TEXT NOT NULL DEFAULT '#000000'",
    )?;
    ensure_column(
        conn,
        "teams",
        "pais_sede",
        "TEXT NOT NULL DEFAULT 'Unknown'",
    )?;
    ensure_column(
        conn,
        "teams",
        "ano_fundacao",
        "INTEGER NOT NULL DEFAULT 2024",
    )?;
    ensure_column(conn, "teams", "ativa", "INTEGER NOT NULL DEFAULT 1")?;
    ensure_column(conn, "teams", "marca", "TEXT")?;
    ensure_column(conn, "teams", "classe", "TEXT")?;
    ensure_column(conn, "teams", "piloto_1_id", "TEXT REFERENCES drivers(id)")?;
    ensure_column(conn, "teams", "piloto_2_id", "TEXT REFERENCES drivers(id)")?;
    ensure_column(conn, "teams", "facilities", "REAL NOT NULL DEFAULT 50.0")?;
    ensure_column(conn, "teams", "engineering", "REAL NOT NULL DEFAULT 50.0")?;
    ensure_column(conn, "teams", "morale", "REAL NOT NULL DEFAULT 1.0")?;
    ensure_column(conn, "teams", "aerodinamica", "REAL NOT NULL DEFAULT 50.0")?;
    ensure_column(conn, "teams", "motor", "REAL NOT NULL DEFAULT 50.0")?;
    ensure_column(conn, "teams", "chassi", "REAL NOT NULL DEFAULT 50.0")?;
    ensure_column(conn, "teams", "hierarquia_n1_id", "TEXT")?;
    ensure_column(conn, "teams", "hierarquia_n2_id", "TEXT")?;
    ensure_column(
        conn,
        "teams",
        "hierarquia_tensao",
        "REAL NOT NULL DEFAULT 0.0",
    )?;
    ensure_column(
        conn,
        "teams",
        "stats_vitorias",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(conn, "teams", "stats_podios", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "teams", "stats_poles", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "teams", "stats_pontos", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(
        conn,
        "teams",
        "stats_melhor_resultado",
        "INTEGER NOT NULL DEFAULT 99",
    )?;
    ensure_column(
        conn,
        "teams",
        "historico_vitorias",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "teams",
        "historico_podios",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "teams",
        "historico_poles",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "teams",
        "historico_pontos",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "teams",
        "historico_titulos_pilotos",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "teams",
        "temporada_atual",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    ensure_column(conn, "teams", "updated_at", "TEXT NOT NULL DEFAULT ''")?;

    conn.execute_batch(
        "
        UPDATE teams
        SET
            nome_curto = CASE
                WHEN nome_curto IS NULL OR TRIM(nome_curto) = '' THEN nome
                ELSE nome_curto
            END,
            cor_primaria = CASE
                WHEN cor_primaria IS NULL OR TRIM(cor_primaria) = '' THEN '#FFFFFF'
                ELSE cor_primaria
            END,
            cor_secundaria = CASE
                WHEN cor_secundaria IS NULL OR TRIM(cor_secundaria) = '' THEN '#000000'
                ELSE cor_secundaria
            END,
            pais_sede = CASE
                WHEN pais_sede IS NULL OR TRIM(pais_sede) = '' THEN 'Unknown'
                ELSE pais_sede
            END,
            ano_fundacao = CASE
                WHEN ano_fundacao IS NULL OR ano_fundacao <= 0 THEN CAST(strftime('%Y', 'now') AS INTEGER)
                ELSE ano_fundacao
            END,
            ativa = COALESCE(ativa, 1),
            hierarquia_status = CASE
                WHEN hierarquia_status IS NULL OR TRIM(hierarquia_status) = '' OR hierarquia_status = 'Independente' THEN 'estavel'
                ELSE LOWER(hierarquia_status)
            END,
            stats_vitorias = CASE
                WHEN stats_vitorias = 0 THEN COALESCE(temp_vitorias, 0)
                ELSE stats_vitorias
            END,
            stats_pontos = CASE
                WHEN stats_pontos = 0 THEN CAST(ROUND(COALESCE(temp_pontos, 0.0)) AS INTEGER)
                ELSE stats_pontos
            END,
            historico_vitorias = CASE
                WHEN historico_vitorias = 0 THEN COALESCE(carreira_vitorias, 0)
                ELSE historico_vitorias
            END,
            temporada_atual = CASE
                WHEN temporada_atual <= 0 THEN CAST(COALESCE((SELECT value FROM meta WHERE key = 'current_season'), '1') AS INTEGER)
                ELSE temporada_atual
            END,
            created_at = CASE
                WHEN created_at IS NULL OR TRIM(created_at) = '' THEN CURRENT_TIMESTAMP
                ELSE created_at
            END,
            updated_at = CASE
                WHEN updated_at IS NULL OR TRIM(updated_at) = '' THEN COALESCE(NULLIF(created_at, ''), CURRENT_TIMESTAMP)
                ELSE updated_at
            END;

        CREATE INDEX IF NOT EXISTS idx_teams_ativa ON teams(ativa);
        ",
    )?;

    Ok(())
}

fn migrate_v3(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "contracts")? {
        return Ok(());
    }

    ensure_column(conn, "contracts", "piloto_nome", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "contracts", "equipe_nome", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(
        conn,
        "contracts",
        "duracao_anos",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    ensure_column(
        conn,
        "contracts",
        "salario_anual",
        "REAL NOT NULL DEFAULT 0.0",
    )?;
    ensure_column(conn, "contracts", "categoria", "TEXT NOT NULL DEFAULT ''")?;
    ensure_column(conn, "contracts", "created_at", "TEXT NOT NULL DEFAULT ''")?;

    conn.execute_batch(
        "
        UPDATE contracts
        SET
            piloto_nome = CASE
                WHEN piloto_nome IS NULL OR TRIM(piloto_nome) = '' THEN COALESCE(
                    (SELECT nome FROM drivers WHERE drivers.id = contracts.piloto_id),
                    piloto_id
                )
                ELSE piloto_nome
            END,
            equipe_nome = CASE
                WHEN equipe_nome IS NULL OR TRIM(equipe_nome) = '' THEN COALESCE(
                    (SELECT nome FROM teams WHERE teams.id = contracts.equipe_id),
                    equipe_id
                )
                ELSE equipe_nome
            END,
            duracao_anos = CASE
                WHEN MAX(
                    1,
                    CAST(temporada_fim AS INTEGER) - CAST(temporada_inicio AS INTEGER) + 1
                ) > COALESCE(duracao_anos, 0) THEN MAX(
                    1,
                    CAST(temporada_fim AS INTEGER) - CAST(temporada_inicio AS INTEGER) + 1
                )
                WHEN duracao_anos IS NULL OR duracao_anos <= 0 THEN 1
                ELSE duracao_anos
            END,
            salario_anual = CASE
                WHEN salario_anual IS NULL OR salario_anual <= 0 THEN COALESCE(salario, 0.0)
                ELSE salario_anual
            END,
            categoria = CASE
                WHEN categoria IS NULL OR TRIM(categoria) = '' THEN COALESCE(
                    (SELECT categoria FROM teams WHERE teams.id = contracts.equipe_id),
                    ''
                )
                ELSE categoria
            END,
            created_at = CASE
                WHEN created_at IS NULL OR TRIM(created_at) = '' THEN CURRENT_TIMESTAMP
                ELSE created_at
            END,
            papel = CASE
                WHEN papel IN ('Numero1', 'N1', 'Titular') THEN 'Numero1'
                WHEN papel IN ('Numero2', 'N2', 'Reserva', 'Junior') THEN 'Numero2'
                ELSE 'Numero2'
            END;

        CREATE INDEX IF NOT EXISTS idx_contracts_categoria ON contracts(categoria);
        ",
    )?;

    Ok(())
}

fn migrate_v4(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "seasons")? {
        ensure_column(
            conn,
            "seasons",
            "rodada_atual",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        ensure_column(conn, "seasons", "created_at", "TEXT NOT NULL DEFAULT ''")?;
        ensure_column(conn, "seasons", "updated_at", "TEXT NOT NULL DEFAULT ''")?;

        conn.execute_batch(
            "
            UPDATE seasons
            SET
                status = CASE
                    WHEN status IS NULL OR TRIM(status) = '' OR status = 'Ativa' THEN 'EmAndamento'
                    WHEN status = 'Finalizada' THEN 'Finalizada'
                    ELSE status
                END,
                rodada_atual = CASE
                    WHEN rodada_atual IS NULL OR rodada_atual <= 0 THEN 1
                    ELSE rodada_atual
                END,
                created_at = CASE
                    WHEN created_at IS NULL OR TRIM(created_at) = '' THEN CURRENT_TIMESTAMP
                    ELSE created_at
                END,
                updated_at = CASE
                    WHEN updated_at IS NULL OR TRIM(updated_at) = '' THEN COALESCE(NULLIF(created_at, ''), CURRENT_TIMESTAMP)
                    ELSE updated_at
                END;
            ",
        )?;
    }

    if table_exists(conn, "calendar")? {
        ensure_column(conn, "calendar", "season_id", "TEXT")?;
        ensure_column(conn, "calendar", "nome", "TEXT NOT NULL DEFAULT ''")?;
        ensure_column(conn, "calendar", "track_id", "INTEGER NOT NULL DEFAULT 0")?;
        ensure_column(conn, "calendar", "track_name", "TEXT NOT NULL DEFAULT ''")?;
        ensure_column(conn, "calendar", "track_config", "TEXT NOT NULL DEFAULT ''")?;
        ensure_column(
            conn,
            "calendar",
            "temperatura",
            "REAL NOT NULL DEFAULT 25.0",
        )?;
        ensure_column(conn, "calendar", "voltas", "INTEGER NOT NULL DEFAULT 10")?;
        ensure_column(
            conn,
            "calendar",
            "duracao_corrida_min",
            "INTEGER NOT NULL DEFAULT 60",
        )?;
        ensure_column(
            conn,
            "calendar",
            "duracao_classificacao_min",
            "INTEGER NOT NULL DEFAULT 15",
        )?;
        ensure_column(
            conn,
            "calendar",
            "status",
            "TEXT NOT NULL DEFAULT 'Pendente'",
        )?;
        ensure_column(conn, "calendar", "horario", "TEXT NOT NULL DEFAULT '14:00'")?;

        conn.execute_batch(
            "
            UPDATE calendar
            SET
                season_id = CASE
                    WHEN season_id IS NULL OR TRIM(season_id) = '' THEN temporada_id
                    ELSE season_id
                END,
                nome = CASE
                    WHEN nome IS NULL OR TRIM(nome) = '' THEN ('Rodada ' || rodada || ' - ' || COALESCE(NULLIF(pista, ''), categoria))
                    ELSE nome
                END,
                track_name = CASE
                    WHEN track_name IS NULL OR TRIM(track_name) = '' THEN COALESCE(NULLIF(pista, ''), '')
                    ELSE track_name
                END,
                track_config = CASE
                    WHEN track_config IS NULL OR TRIM(track_config) = '' THEN ''
                    ELSE track_config
                END,
                clima = CASE
                    WHEN clima = 'Seco' THEN 'Dry'
                    WHEN clima = 'ChuvaLeve' THEN 'Damp'
                    WHEN clima = 'ChuvaForte' THEN 'HeavyRain'
                    WHEN clima = 'Nublado' THEN 'Damp'
                    ELSE COALESCE(NULLIF(clima, ''), 'Dry')
                END,
                duracao_corrida_min = CASE
                    WHEN duracao_corrida_min IS NULL OR duracao_corrida_min <= 0 THEN COALESCE(duracao, 60)
                    WHEN duracao_corrida_min = 60 AND COALESCE(duracao, 60) <> 60 THEN duracao
                    ELSE duracao_corrida_min
                END,
                status = CASE
                    WHEN status IS NULL OR TRIM(status) = '' THEN 'Pendente'
                    ELSE status
                END,
                horario = CASE
                    WHEN horario IS NULL OR TRIM(horario) = '' THEN '14:00'
                    ELSE horario
                END;

            CREATE INDEX IF NOT EXISTS idx_calendar_season_id ON calendar(season_id);
            CREATE INDEX IF NOT EXISTS idx_calendar_categoria ON calendar(categoria);
            ",
        )?;
    }

    Ok(())
}

fn migrate_v5(conn: &Connection) -> Result<(), DbError> {
    // The previous schema for race_results required a foreign key to `races(id)`
    // However, the application uses `calendar` entries as races and `races` table is entirely unused.
    // Since `race_results` was never populated prior to this update (no insert queries existed),
    // we can safely drop and recreate it to fix the foreign keys and add new module 25 columns.
    let had_legacy_rows =
        rebuild_table_preserving_rows(conn, "race_results", "race_results_legacy")?;

    conn.execute_batch(
        "
        CREATE TABLE race_results (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            race_id             TEXT NOT NULL,
            piloto_id           TEXT NOT NULL,
            equipe_id           TEXT NOT NULL,
            posicao_largada     INTEGER NOT NULL DEFAULT 0,
            posicao_final       INTEGER NOT NULL DEFAULT 0,
            voltas_completadas  INTEGER NOT NULL DEFAULT 0,
            dnf                 INTEGER NOT NULL DEFAULT 0,
            pontos              REAL NOT NULL DEFAULT 0.0,
            tempo_total         REAL NOT NULL DEFAULT 0.0,
            fastest_lap         INTEGER NOT NULL DEFAULT 0,
            dnf_reason          TEXT,
            dnf_segment         TEXT,
            incidents_count     INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (race_id)    REFERENCES calendar(id),
            FOREIGN KEY (piloto_id)  REFERENCES drivers(id),
            FOREIGN KEY (equipe_id)  REFERENCES teams(id)
        );
        CREATE INDEX IF NOT EXISTS idx_race_results_race ON race_results(race_id);
        CREATE INDEX IF NOT EXISTS idx_race_results_piloto ON race_results(piloto_id);
        ",
    )?;

    if had_legacy_rows {
        copy_legacy_rows(
            conn,
            "race_results_legacy",
            "race_results",
            &[
                "race_id",
                "piloto_id",
                "equipe_id",
                "posicao_largada",
                "posicao_final",
                "voltas_completadas",
                "dnf",
                "pontos",
                "tempo_total",
                "fastest_lap",
                "dnf_reason",
                "dnf_segment",
                "incidents_count",
            ],
            &[
                column_expr(conn, "race_results_legacy", &["race_id"], "''")?,
                column_expr(conn, "race_results_legacy", &["piloto_id"], "''")?,
                column_expr(conn, "race_results_legacy", &["equipe_id"], "''")?,
                column_expr(conn, "race_results_legacy", &["posicao_largada"], "0")?,
                column_expr(conn, "race_results_legacy", &["posicao_final"], "0")?,
                column_expr(conn, "race_results_legacy", &["voltas_completadas"], "0")?,
                column_expr(conn, "race_results_legacy", &["dnf"], "0")?,
                column_expr(conn, "race_results_legacy", &["pontos"], "0.0")?,
                column_expr(conn, "race_results_legacy", &["tempo_total"], "0.0")?,
                column_expr(conn, "race_results_legacy", &["fastest_lap"], "0")?,
                column_expr(conn, "race_results_legacy", &["dnf_reason"], "NULL")?,
                column_expr(conn, "race_results_legacy", &["dnf_segment"], "NULL")?,
                column_expr(conn, "race_results_legacy", &["incidents_count"], "0")?,
            ],
        )?;
    }

    Ok(())
}

fn migrate_v6(conn: &Connection) -> Result<(), DbError> {
    let had_legacy_rows = rebuild_table_preserving_rows(conn, "injuries", "injuries_legacy")?;

    conn.execute_batch(
        "
        CREATE TABLE injuries (
            id                  TEXT PRIMARY KEY,
            pilot_id            TEXT NOT NULL,
            type                TEXT NOT NULL,
            modifier            REAL NOT NULL DEFAULT 1.0,
            races_total         INTEGER NOT NULL,
            races_remaining     INTEGER NOT NULL,
            skill_penalty       REAL NOT NULL DEFAULT 0.0,
            season              INTEGER NOT NULL,
            race_occurred       TEXT NOT NULL,
            active              INTEGER NOT NULL DEFAULT 1,
            FOREIGN KEY (pilot_id) REFERENCES drivers(id)
        );
        CREATE INDEX IF NOT EXISTS idx_injuries_pilot_id ON injuries(pilot_id);
        CREATE INDEX IF NOT EXISTS idx_injuries_active ON injuries(active);
        ",
    )?;

    if had_legacy_rows {
        let season_expr = if table_has_any_column(conn, "injuries_legacy", &["temporada_id"])? {
            "COALESCE((SELECT numero FROM seasons WHERE id = injuries_legacy.temporada_id), 0)"
                .to_string()
        } else {
            column_expr(conn, "injuries_legacy", &["season"], "0")?
        };

        copy_legacy_rows(
            conn,
            "injuries_legacy",
            "injuries",
            &[
                "id",
                "pilot_id",
                "type",
                "modifier",
                "races_total",
                "races_remaining",
                "skill_penalty",
                "season",
                "race_occurred",
                "active",
            ],
            &[
                column_expr(
                    conn,
                    "injuries_legacy",
                    &["id"],
                    "lower(hex(randomblob(16)))",
                )?,
                column_expr(conn, "injuries_legacy", &["pilot_id", "piloto_id"], "''")?,
                column_expr(conn, "injuries_legacy", &["type", "tipo"], "'Leve'")?,
                column_expr(conn, "injuries_legacy", &["modifier"], "1.0")?,
                column_expr(
                    conn,
                    "injuries_legacy",
                    &["races_total", "corridas_restantes"],
                    "0",
                )?,
                column_expr(
                    conn,
                    "injuries_legacy",
                    &["races_remaining", "corridas_restantes"],
                    "0",
                )?,
                column_expr(conn, "injuries_legacy", &["skill_penalty"], "0.0")?,
                season_expr,
                column_expr(
                    conn,
                    "injuries_legacy",
                    &["race_occurred", "descricao"],
                    "''",
                )?,
                column_expr(conn, "injuries_legacy", &["active"], "1")?,
            ],
        )?;
    }

    Ok(())
}

fn migrate_v7(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "teams")? {
        ensure_column(
            conn,
            "teams",
            "hierarquia_duelos_total",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            conn,
            "teams",
            "hierarquia_duelos_n2_vencidos",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            conn,
            "teams",
            "hierarquia_sequencia_n2",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            conn,
            "teams",
            "hierarquia_sequencia_n1",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            conn,
            "teams",
            "hierarquia_inversoes_temporada",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
    }
    Ok(())
}

fn migrate_v8(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "rivalries")? {
        // Adiciona os dois eixos de intensidade ao modelo dual
        ensure_column(
            conn,
            "rivalries",
            "historical_intensity",
            "REAL NOT NULL DEFAULT 0.0",
        )?;
        ensure_column(
            conn,
            "rivalries",
            "recent_activity",
            "REAL NOT NULL DEFAULT 0.0",
        )?;
        // Temporada do último reforço — base para decisão de decaimento
        ensure_column(
            conn,
            "rivalries",
            "temporada_update",
            "INTEGER NOT NULL DEFAULT 0",
        )?;

        // Migra dados existentes: histórico recebe intensidade antiga; recente recebe 30% como calor residual
        conn.execute_batch(
            "UPDATE rivalries SET
                 historical_intensity = intensidade,
                 recent_activity      = ROUND(intensidade * 0.3, 2)
             WHERE historical_intensity = 0.0 AND intensidade > 0.0;",
        )?;
    }

    Ok(())
}

fn migrate_v9(conn: &Connection) -> Result<(), DbError> {
    // Guards necessários para testes de migração parcial que criam schemas sem todas as tabelas.
    if table_exists(conn, "contracts")? {
        // Tipo de contrato: Regular (padrão) ou Especial (sazonal de meio de ano)
        ensure_column(conn, "contracts", "tipo", "TEXT NOT NULL DEFAULT 'Regular'")?;
    }
    if table_exists(conn, "drivers")? {
        // Categoria especial ativa do piloto — separada da categoria regular de carreira
        ensure_column(conn, "drivers", "categoria_especial_ativa", "TEXT")?;
    }
    if table_exists(conn, "seasons")? {
        // Fase da temporada: BlocoRegular | JanelaConvocacao | BlocoEspecial
        ensure_column(
            conn,
            "seasons",
            "fase",
            "TEXT NOT NULL DEFAULT 'BlocoRegular'",
        )?;
    }
    Ok(())
}

fn migrate_v10(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "contracts")? {
        // Classe do contrato especial (ex: "gt3", "mazda").
        // NULL em contratos regulares; preenchido em contratos especiais de categorias multi-classe.
        ensure_column(conn, "contracts", "classe", "TEXT")?;
    }
    Ok(())
}

fn migrate_v11(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "calendar")? {
        // Semana do ano (1-52) — unidade temporal interna do sistema.
        // week_of_year continua sendo a unidade temporal interna,
        // mas as corridas são geradas a partir de janelas mensais do calendário anual.
        // Linhas existentes ficam com 0 (semana não atribuída — saves antigos).
        ensure_column(
            conn,
            "calendar",
            "week_of_year",
            "INTEGER NOT NULL DEFAULT 0",
        )?;

        // Fase da temporada em que o evento ocorre.
        // BlocoRegular para categorias regulares; BlocoEspecial para especiais.
        ensure_column(
            conn,
            "calendar",
            "season_phase",
            "TEXT NOT NULL DEFAULT 'BlocoRegular'",
        )?;
    }
    Ok(())
}

fn migrate_v12(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "calendar")? {
        // Papel narrativo fixo da corrida dentro da temporada (ThematicSlot).
        // NULL para saves pré-v12 — lidos como NaoClassificado pelo domain layer.
        // Sem DEFAULT: NULL é o estado semântico correto para saves legados.
        ensure_column(conn, "calendar", "thematic_slot", "TEXT")?;
    }
    Ok(())
}

fn migrate_v13(conn: &Connection) -> Result<(), DbError> {
    // Novas colunas em race_results para contexto narrativo
    if table_exists(conn, "race_results")? {
        ensure_column(
            conn,
            "race_results",
            "gap_to_winner_ms",
            "REAL NOT NULL DEFAULT 0.0",
        )?;
        ensure_column(
            conn,
            "race_results",
            "final_tire_wear",
            "REAL NOT NULL DEFAULT 1.0",
        )?;
    }

    // Nova tabela para histórico de DNFs por pista (feature de redenção)
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS track_dnf_history (
            id            TEXT PRIMARY KEY,
            piloto_id     TEXT NOT NULL,
            track_name    TEXT NOT NULL,
            season_num    INTEGER NOT NULL,
            round         INTEGER NOT NULL,
            dnf_reason    TEXT NOT NULL,
            collision_with TEXT,
            created_at    TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_track_dnf_piloto_track
            ON track_dnf_history(piloto_id, track_name);
        ",
    )?;

    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<(), DbError> {
    if !table_has_column(conn, table_name, column_name)? {
        conn.execute_batch(&format!(
            "ALTER TABLE {} ADD COLUMN {} {};",
            table_name, column_name, definition
        ))?;
    }

    Ok(())
}

fn table_has_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, DbError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let name: String = row.get("name")?;
        if name == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn table_exists(conn: &Connection, table_name: &str) -> Result<bool, DbError> {
    let exists = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        rusqlite::params![table_name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists > 0)
}

fn table_has_any_column(
    conn: &Connection,
    table_name: &str,
    candidate_columns: &[&str],
) -> Result<bool, DbError> {
    for column in candidate_columns {
        if table_has_column(conn, table_name, column)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn column_expr(
    conn: &Connection,
    table_name: &str,
    candidate_columns: &[&str],
    fallback_expr: &str,
) -> Result<String, DbError> {
    for column in candidate_columns {
        if table_has_column(conn, table_name, column)? {
            return Ok((*column).to_string());
        }
    }
    Ok(fallback_expr.to_string())
}

fn rebuild_table_preserving_rows(
    conn: &Connection,
    table_name: &str,
    legacy_name: &str,
) -> Result<bool, DbError> {
    if !table_exists(conn, table_name)? {
        return Ok(false);
    }

    conn.execute_batch(&format!("DROP TABLE IF EXISTS {legacy_name};"))?;
    conn.execute_batch(&format!(
        "ALTER TABLE {table_name} RENAME TO {legacy_name};"
    ))?;
    Ok(true)
}

fn copy_legacy_rows(
    conn: &Connection,
    source_table: &str,
    target_table: &str,
    target_columns: &[&str],
    select_exprs: &[String],
) -> Result<(), DbError> {
    let sql = format!(
        "INSERT INTO {target_table} ({})
         SELECT {}
         FROM {source_table};",
        target_columns.join(", "),
        select_exprs.join(", "),
    );
    conn.execute_batch(&sql)?;
    conn.execute_batch(&format!("DROP TABLE IF EXISTS {source_table};"))?;
    Ok(())
}

fn migrate_v14(conn: &Connection) -> Result<(), DbError> {
    // 1. Tabela de catálogo de incidentes
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS incident_catalog (
            id                TEXT PRIMARY KEY,
            vehicle_class     TEXT NOT NULL,
            race_format       TEXT NOT NULL,
            incident_source   TEXT NOT NULL,
            trigger_type      TEXT NOT NULL,
            severity_context  TEXT NOT NULL,
            weight_sprint     INTEGER NOT NULL DEFAULT 0,
            weight_endurance  INTEGER NOT NULL DEFAULT 0,
            dnf_template      TEXT NOT NULL,
            non_dnf_template  TEXT,
            description_short TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_incident_catalog_class_format
            ON incident_catalog(vehicle_class, race_format);
        CREATE INDEX IF NOT EXISTS idx_incident_catalog_source
            ON incident_catalog(incident_source);
        ",
    )?;

    // 2. Seed data
    seed_incident_catalog(conn)?;

    // 3. Novos campos em race_results
    if table_exists(conn, "race_results")? {
        ensure_column(conn, "race_results", "dnf_catalog_id", "TEXT")?;
        ensure_column(conn, "race_results", "damage_origin_segment", "TEXT")?;
    }

    Ok(())
}

fn migrate_v15(conn: &Connection) -> Result<(), DbError> {
    let had_legacy_rows = rebuild_table_preserving_rows(
        conn,
        "driver_season_results_archive",
        "driver_season_results_archive_legacy",
    )?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS driver_season_archive (
            id                  INTEGER PRIMARY KEY AUTOINCREMENT,
            piloto_id           TEXT    NOT NULL,
            season_number       INTEGER NOT NULL,
            ano                 INTEGER NOT NULL,
            nome                TEXT    NOT NULL,
            categoria           TEXT    NOT NULL DEFAULT '',
            posicao_campeonato  INTEGER,
            pontos              REAL,
            snapshot_json       TEXT    NOT NULL,
            archived_at         TEXT    NOT NULL DEFAULT (datetime('now')),
            UNIQUE(piloto_id, season_number)
        );
        CREATE INDEX IF NOT EXISTS idx_driver_season_archive_piloto
            ON driver_season_archive(piloto_id);
        CREATE INDEX IF NOT EXISTS idx_driver_season_archive_season
            ON driver_season_archive(season_number, categoria);
        ",
    )?;

    if had_legacy_rows {
        copy_legacy_rows(
            conn,
            "driver_season_results_archive_legacy",
            "driver_season_archive",
            &[
                "piloto_id",
                "season_number",
                "ano",
                "nome",
                "categoria",
                "posicao_campeonato",
                "pontos",
                "snapshot_json",
                "archived_at",
            ],
            &[
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["piloto_id"],
                    "''",
                )?,
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["season_number", "temporada_numero"],
                    "0",
                )?,
                column_expr(conn, "driver_season_results_archive_legacy", &["ano"], "0")?,
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["nome"],
                    "''",
                )?,
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["categoria"],
                    "''",
                )?,
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["posicao_campeonato"],
                    "NULL",
                )?,
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["pontos"],
                    "NULL",
                )?,
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["snapshot_json"],
                    "'{}'",
                )?,
                column_expr(
                    conn,
                    "driver_season_results_archive_legacy",
                    &["archived_at"],
                    "datetime('now')",
                )?,
            ],
        )?;
    }
    Ok(())
}

fn migrate_v16(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "teams")? {
        ensure_column(conn, "teams", "categoria_anterior", "TEXT")?;
    }
    Ok(())
}

fn migrate_v17(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "rivalries")? {
        conn.execute_batch(
            "
            UPDATE rivalries
            SET
                intensidade = (
                    SELECT MAX(r2.intensidade)
                    FROM rivalries r2
                    WHERE r2.piloto1_id = rivalries.piloto1_id
                      AND r2.piloto2_id = rivalries.piloto2_id
                ),
                historical_intensity = (
                    SELECT MAX(r2.historical_intensity)
                    FROM rivalries r2
                    WHERE r2.piloto1_id = rivalries.piloto1_id
                      AND r2.piloto2_id = rivalries.piloto2_id
                ),
                recent_activity = (
                    SELECT MAX(r2.recent_activity)
                    FROM rivalries r2
                    WHERE r2.piloto1_id = rivalries.piloto1_id
                      AND r2.piloto2_id = rivalries.piloto2_id
                ),
                temporada_update = (
                    SELECT MAX(r2.temporada_update)
                    FROM rivalries r2
                    WHERE r2.piloto1_id = rivalries.piloto1_id
                      AND r2.piloto2_id = rivalries.piloto2_id
                ),
                ultima_atualizacao = (
                    SELECT MAX(r2.ultima_atualizacao)
                    FROM rivalries r2
                    WHERE r2.piloto1_id = rivalries.piloto1_id
                      AND r2.piloto2_id = rivalries.piloto2_id
                )
            WHERE id IN (
                SELECT MIN(id)
                FROM rivalries
                GROUP BY piloto1_id, piloto2_id
            );

            DELETE FROM rivalries
            WHERE id NOT IN (
                SELECT MIN(id)
                FROM rivalries
                GROUP BY piloto1_id, piloto2_id
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_rivalries_pair_unique
                ON rivalries(piloto1_id, piloto2_id);
            ",
        )?;
    }
    Ok(())
}

fn migrate_v18(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "contracts")? {
        return Ok(());
    }

    let duplicate: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT piloto_id, tipo, COUNT(*)
             FROM contracts
             WHERE status = 'Ativo'
             GROUP BY piloto_id, tipo
             HAVING COUNT(*) > 1
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    if let Some((piloto_id, tipo, total)) = duplicate {
        return Err(DbError::Migration(format!(
            "contratos ativos duplicados para piloto '{piloto_id}' e tipo '{tipo}' ({total} registros)"
        )));
    }

    conn.execute_batch(
        "
        CREATE UNIQUE INDEX IF NOT EXISTS idx_contracts_active_pilot_tipo
            ON contracts(piloto_id, tipo)
            WHERE status = 'Ativo';
        ",
    )?;

    Ok(())
}

fn migrate_v19(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "seasons")? {
        return Ok(());
    }

    conn.execute(
        "UPDATE seasons SET status = 'EmAndamento' WHERE status = 'Ativa'",
        [],
    )?;

    let duplicate: Option<i64> = conn
        .query_row(
            "SELECT COUNT(*)
             FROM seasons
             WHERE status = 'EmAndamento'
             HAVING COUNT(*) > 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(total) = duplicate {
        return Err(DbError::Migration(format!(
            "temporadas ativas duplicadas encontradas ({total} registros)"
        )));
    }

    conn.execute_batch(
        "
        CREATE UNIQUE INDEX IF NOT EXISTS idx_seasons_single_active
            ON seasons(status)
            WHERE status = 'EmAndamento';
        ",
    )?;

    Ok(())
}

fn migrate_v20(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "race_results")? {
        return Ok(());
    }

    let duplicate: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT race_id, piloto_id, COUNT(*) AS total
             FROM race_results
             GROUP BY race_id, piloto_id
             HAVING COUNT(*) > 1
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    if let Some((race_id, piloto_id, total)) = duplicate {
        return Err(DbError::Migration(format!(
            "resultados duplicados em race_results para corrida '{race_id}' e piloto '{piloto_id}' ({total} registros)"
        )));
    }

    conn.execute_batch(
        "
        CREATE UNIQUE INDEX IF NOT EXISTS idx_race_results_unique_race_pilot
            ON race_results(race_id, piloto_id);
        ",
    )?;

    Ok(())
}

fn migrate_v21(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "injuries")? {
        return Ok(());
    }

    let duplicate_active = conn
        .query_row(
            "
            SELECT pilot_id, COUNT(*)
            FROM injuries
            WHERE active = 1
            GROUP BY pilot_id
            HAVING COUNT(*) > 1
            LIMIT 1
            ",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;

    if let Some((pilot_id, count)) = duplicate_active {
        return Err(DbError::Migration(format!(
            "lesoes ativas duplicadas em injuries para piloto '{pilot_id}' ({count} registros)"
        )));
    }

    conn.execute_batch(
        "
        CREATE UNIQUE INDEX IF NOT EXISTS idx_injuries_single_active_per_pilot
        ON injuries(pilot_id)
        WHERE active = 1;
        ",
    )?;

    Ok(())
}

fn migrate_v22(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "standings")? {
        return Ok(());
    }

    let equipe_id_is_not_null = {
        let mut stmt = conn.prepare("PRAGMA table_info(standings)")?;
        let mut rows = stmt.query([])?;
        let mut equipe_id_is_not_null = None;
        while let Some(row) = rows.next()? {
            let column_name: String = row.get(1)?;
            if column_name == "equipe_id" {
                equipe_id_is_not_null = Some(row.get::<_, i64>(3)? != 0);
                break;
            }
        }
        equipe_id_is_not_null.unwrap_or(false)
    };

    if !equipe_id_is_not_null {
        return Ok(());
    }

    conn.execute_batch(
        "
        ALTER TABLE standings RENAME TO standings_old;

        CREATE TABLE standings (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            temporada_id TEXT NOT NULL,
            piloto_id    TEXT NOT NULL,
            equipe_id    TEXT,
            categoria    TEXT NOT NULL,
            posicao      INTEGER NOT NULL DEFAULT 0,
            pontos       REAL NOT NULL DEFAULT 0.0,
            vitorias     INTEGER NOT NULL DEFAULT 0,
            podios       INTEGER NOT NULL DEFAULT 0,
            poles        INTEGER NOT NULL DEFAULT 0,
            corridas     INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (temporada_id) REFERENCES seasons(id),
            FOREIGN KEY (piloto_id)    REFERENCES drivers(id),
            FOREIGN KEY (equipe_id)    REFERENCES teams(id)
        );

        INSERT INTO standings (
            id, temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
        )
        SELECT
            id, temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
        FROM standings_old;

        DROP TABLE standings_old;
        ",
    )?;

    Ok(())
}

fn migrate_v23(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "teams")? {
        return Ok(());
    }

    ensure_column(
        conn,
        "teams",
        "car_build_profile",
        "TEXT NOT NULL DEFAULT 'balanced'",
    )?;

    conn.execute(
        "UPDATE teams
         SET car_build_profile = 'balanced'
         WHERE car_build_profile IS NULL OR TRIM(car_build_profile) = ''",
        [],
    )?;

    Ok(())
}

fn migrate_v24(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "teams")? {
        return Ok(());
    }

    ensure_column(
        conn,
        "teams",
        "pit_strategy_risk",
        "REAL NOT NULL DEFAULT 50.0",
    )?;
    ensure_column(
        conn,
        "teams",
        "pit_crew_quality",
        "REAL NOT NULL DEFAULT 50.0",
    )?;

    conn.execute(
        "UPDATE teams
         SET pit_strategy_risk = 50.0
         WHERE pit_strategy_risk IS NULL",
        [],
    )?;
    conn.execute(
        "UPDATE teams
         SET pit_crew_quality = 50.0
         WHERE pit_crew_quality IS NULL",
        [],
    )?;

    Ok(())
}

fn migrate_v25(conn: &Connection) -> Result<(), DbError> {
    if !table_exists(conn, "teams")? {
        return Ok(());
    }

    ensure_column(conn, "teams", "cash_balance", "REAL NOT NULL DEFAULT 0.0")?;
    ensure_column(conn, "teams", "debt_balance", "REAL NOT NULL DEFAULT 0.0")?;
    ensure_column(
        conn,
        "teams",
        "financial_state",
        "TEXT NOT NULL DEFAULT 'stable'",
    )?;
    ensure_column(
        conn,
        "teams",
        "season_strategy",
        "TEXT NOT NULL DEFAULT 'balanced'",
    )?;
    ensure_column(
        conn,
        "teams",
        "last_round_income",
        "REAL NOT NULL DEFAULT 0.0",
    )?;
    ensure_column(
        conn,
        "teams",
        "last_round_expenses",
        "REAL NOT NULL DEFAULT 0.0",
    )?;
    ensure_column(conn, "teams", "last_round_net", "REAL NOT NULL DEFAULT 0.0")?;
    ensure_column(
        conn,
        "teams",
        "parachute_payment_remaining",
        "REAL NOT NULL DEFAULT 0.0",
    )?;

    conn.execute_batch(
        "
        UPDATE teams
        SET cash_balance = 0.0
        WHERE cash_balance IS NULL;

        UPDATE teams
        SET debt_balance = 0.0
        WHERE debt_balance IS NULL;

        UPDATE teams
        SET financial_state = 'stable'
        WHERE financial_state IS NULL OR TRIM(financial_state) = '';

        UPDATE teams
        SET season_strategy = 'balanced'
        WHERE season_strategy IS NULL OR TRIM(season_strategy) = '';

        UPDATE teams
        SET last_round_income = 0.0
        WHERE last_round_income IS NULL;

        UPDATE teams
        SET last_round_expenses = 0.0
        WHERE last_round_expenses IS NULL;

        UPDATE teams
        SET last_round_net = 0.0
        WHERE last_round_net IS NULL;

        UPDATE teams
        SET parachute_payment_remaining = 0.0
        WHERE parachute_payment_remaining IS NULL;
        ",
    )?;

    Ok(())
}

fn migrate_v26(conn: &Connection) -> Result<(), DbError> {
    if table_exists(conn, "player_special_offers")? {
        ensure_column(
            conn,
            "player_special_offers",
            "available_from_day",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        ensure_column(
            conn,
            "player_special_offers",
            "selected_for_day",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(conn, "player_special_offers", "resolved_day", "INTEGER")?;
    }

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS special_window_state (
            season_id        TEXT PRIMARY KEY,
            current_day      INTEGER NOT NULL DEFAULT 1,
            total_days       INTEGER NOT NULL DEFAULT 7,
            status           TEXT NOT NULL DEFAULT 'Aberta',
            active_offer_id  TEXT,
            player_result    TEXT,
            created_at       TEXT NOT NULL DEFAULT '',
            updated_at       TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS special_window_assignments (
            id               TEXT PRIMARY KEY,
            season_id        TEXT NOT NULL,
            special_category TEXT NOT NULL,
            class_name       TEXT NOT NULL,
            team_id          TEXT NOT NULL,
            driver_id        TEXT NOT NULL,
            papel            TEXT NOT NULL,
            reveal_day       INTEGER NOT NULL DEFAULT 1,
            revealed         INTEGER NOT NULL DEFAULT 0,
            new_badge_day    INTEGER,
            is_player        INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS special_window_candidate_pool (
            season_id           TEXT NOT NULL,
            driver_id           TEXT NOT NULL,
            driver_name         TEXT NOT NULL,
            origin_category     TEXT NOT NULL,
            license_level       INTEGER,
            desirability        INTEGER NOT NULL DEFAULT 0,
            production_eligible INTEGER NOT NULL DEFAULT 0,
            endurance_eligible  INTEGER NOT NULL DEFAULT 0,
            status              TEXT NOT NULL DEFAULT 'Livre',
            PRIMARY KEY (season_id, driver_id)
        );

        CREATE TABLE IF NOT EXISTS special_window_daily_log (
            id               TEXT PRIMARY KEY,
            season_id        TEXT NOT NULL,
            day_number       INTEGER NOT NULL,
            event_type       TEXT NOT NULL,
            message          TEXT NOT NULL,
            special_category TEXT,
            class_name       TEXT,
            team_id          TEXT,
            driver_id        TEXT,
            created_at       TEXT NOT NULL DEFAULT ''
        );

        CREATE INDEX IF NOT EXISTS idx_special_window_assignments_season
            ON special_window_assignments(season_id, reveal_day, revealed);
        CREATE INDEX IF NOT EXISTS idx_special_window_candidate_pool_season
            ON special_window_candidate_pool(season_id, status);
        CREATE INDEX IF NOT EXISTS idx_special_window_daily_log_season
            ON special_window_daily_log(season_id, day_number);
        ",
    )?;

    if table_exists(conn, "special_window_assignments")? {
        ensure_column(
            conn,
            "special_window_assignments",
            "new_badge_day",
            "INTEGER",
        )?;
    }

    Ok(())
}

fn migrate_v27(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS special_team_entries (
            season_id            TEXT NOT NULL,
            special_category     TEXT NOT NULL,
            class_name           TEXT NOT NULL,
            team_id              TEXT NOT NULL,
            source_category      TEXT NOT NULL,
            qualified_via        TEXT NOT NULL,
            guaranteed_next_year INTEGER NOT NULL DEFAULT 0,
            created_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at           TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (season_id, special_category, class_name, team_id),
            FOREIGN KEY (season_id) REFERENCES seasons(id),
            FOREIGN KEY (team_id) REFERENCES teams(id)
        );

        CREATE INDEX IF NOT EXISTS idx_special_team_entries_lookup
            ON special_team_entries(season_id, special_category, class_name);
        CREATE INDEX IF NOT EXISTS idx_special_team_entries_team
            ON special_team_entries(team_id);
        ",
    )?;

    Ok(())
}

fn seed_incident_catalog(conn: &Connection) -> Result<(), DbError> {
    // INSERT OR IGNORE para idempotência
    // Formato: (id, vehicle_class, race_format, incident_source, trigger_type,
    //           severity_context, weight_sprint, weight_endurance,
    //           dnf_template, non_dnf_template, description_short)
    let entries: &[(
        &str,
        &str,
        &str,
        &str,
        &str,
        &str,
        i64,
        i64,
        &str,
        Option<&str>,
        &str,
    )] = &[
        // ═══ STREETBASED SPRINT MECHANICAL SPONTANEOUS ═══
        (
            "SB_S_MEC_01",
            "StreetBased",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            100,
            0,
            "{driver} abandona com problema no câmbio – sincronizador da 3ª marcha falhou",
            Some("{driver} com dificuldade no câmbio – perdeu ritmo"),
            "Câmbio – sincronizador",
        ),
        (
            "SB_S_MEC_02",
            "StreetBased",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            70,
            0,
            "{driver} abandona por embreagem queimada após largada",
            Some("{driver} sentindo a embreagem patinar – ritmo comprometido"),
            "Embreagem queimada",
        ),
        (
            "SB_S_MEC_03",
            "StreetBased",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            40,
            0,
            "{driver} abandona por falha nos freios – disco rachado",
            Some("{driver} com freios comprometidos – perdendo posições"),
            "Freio – pastilha/disco",
        ),
        (
            "SB_S_MEC_04",
            "StreetBased",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            20,
            0,
            "{driver} abandona por superaquecimento do motor",
            Some("{driver} com temperatura do motor elevada – reduzindo ritmo"),
            "Superaquecimento",
        ),
        (
            "SB_S_MEC_05",
            "StreetBased",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            10,
            0,
            "{driver} abandona por perda de potência – falha no motor",
            Some("{driver} com perda de potência – ritmo comprometido"),
            "Motor – perda de potência",
        ),
        // ═══ STREETBASED ENDURANCE MECHANICAL SPONTANEOUS ═══
        (
            "SB_E_MEC_01",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            100,
            "{driver} abandona com problemas no câmbio",
            Some("{driver} com câmbio engasgando – perdeu ritmo"),
            "Câmbio – sincronizador/garfo",
        ),
        (
            "SB_E_MEC_02",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            70,
            "{driver} abandona por falha no motor – dano interno",
            Some("{driver} com motor perdendo força"),
            "Motor – biela/bronzina",
        ),
        (
            "SB_E_MEC_03",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            70,
            "{driver} abandona por embreagem gasta – sem tração",
            Some("{driver} com embreagem desgastada – tração comprometida"),
            "Embreagem desgastada",
        ),
        (
            "SB_E_MEC_04",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            50,
            "{driver} abandona por superaquecimento – falha no arrefecimento",
            Some("{driver} monitorando temperatura elevada – ritmo reduzido"),
            "Superaquecimento – radiador/bomba",
        ),
        (
            "SB_E_MEC_05",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            40,
            "{driver} abandona por falha elétrica – bateria descarregou",
            Some("{driver} com problemas elétricos intermitentes"),
            "Alternador/bateria",
        ),
        (
            "SB_E_MEC_06",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            40,
            "{driver} abandona por perda de freios",
            Some("{driver} com freios degradados – frenando mais cedo"),
            "Freio – disco/fluido ferveu",
        ),
        (
            "SB_E_MEC_07",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            20,
            "{driver} abandona por quebra do semi-eixo",
            Some("{driver} sentindo vibração na transmissão"),
            "Semi-eixo/cubo de roda",
        ),
        (
            "SB_E_MEC_08",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            10,
            "{driver} abandona por falha na alimentação de combustível",
            Some("{driver} com motor falhando intermitentemente"),
            "Bomba de combustível",
        ),
        // ═══ RACESPEC SPRINT MECHANICAL SPONTANEOUS ═══
        (
            "RS_S_MEC_01",
            "RaceSpec",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            100,
            0,
            "{driver} abandona por falha no câmbio – ficou preso em uma marcha",
            Some("{driver} com câmbio travando – perdeu posições"),
            "Câmbio – garfo/atuador",
        ),
        (
            "RS_S_MEC_02",
            "RaceSpec",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            70,
            0,
            "{driver} abandona por embreagem queimada",
            Some("{driver} com embreagem patinando"),
            "Embreagem queimada",
        ),
        (
            "RS_S_MEC_03",
            "RaceSpec",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            40,
            0,
            "{driver} abandona por falha nos freios – disco rachado",
            Some("{driver} com freios comprometidos"),
            "Freio – disco rachado",
        ),
        (
            "RS_S_MEC_04",
            "RaceSpec",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            20,
            0,
            "{driver} abandona por falha eletrônica – carro em modo de proteção",
            Some("{driver} com eletrônica instável – ritmo irregular"),
            "Eletrônica – sensor/ECU",
        ),
        (
            "RS_S_MEC_05",
            "RaceSpec",
            "Sprint",
            "Mechanical",
            "Spontaneous",
            "Both",
            10,
            0,
            "{driver} abandona por superaquecimento do motor",
            Some("{driver} com temperatura elevada – gerenciando ritmo"),
            "Superaquecimento",
        ),
        // ═══ RACESPEC ENDURANCE MECHANICAL SPONTANEOUS ═══
        (
            "RS_E_MEC_01",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            100,
            "{driver} abandona por problemas no câmbio",
            Some("{driver} com câmbio apresentando falhas"),
            "Câmbio – garfo/engrenagem/óleo",
        ),
        (
            "RS_E_MEC_02",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            70,
            "{driver} abandona por embreagem gasta",
            Some("{driver} com embreagem desgastada"),
            "Embreagem desgastada",
        ),
        (
            "RS_E_MEC_03",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            60,
            "{driver} abandona por falha no motor",
            Some("{driver} com motor perdendo rendimento"),
            "Motor – turbo/biela",
        ),
        (
            "RS_E_MEC_04",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            50,
            "{driver} abandona por falha elétrica – alternador parou de carregar",
            Some("{driver} com problemas elétricos recorrentes"),
            "Alternador/bateria",
        ),
        (
            "RS_E_MEC_05",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            40,
            "{driver} abandona por superaquecimento",
            Some("{driver} gerenciando temperatura elevada"),
            "Superaquecimento – radiador/bomba",
        ),
        (
            "RS_E_MEC_06",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            40,
            "{driver} abandona por perda de freios",
            Some("{driver} com freios degradados"),
            "Freio – disco/fluido",
        ),
        (
            "RS_E_MEC_07",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            20,
            "{driver} abandona por falha no diferencial",
            Some("{driver} com diferencial apresentando ruídos"),
            "Diferencial – vazamento/travamento",
        ),
        (
            "RS_E_MEC_08",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            20,
            "{driver} abandona por falha eletrônica",
            Some("{driver} com eletrônica intermitente"),
            "Eletrônica – sensor ABS/TC/ECU",
        ),
        (
            "RS_E_MEC_09",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "Both",
            0,
            10,
            "{driver} abandona por falha na alimentação de combustível",
            Some("{driver} com motor falhando"),
            "Bomba de combustível",
        ),
        // ═══ ERRO DE COMBUSTÍVEL ENDURANCE (Mechanical/Spontaneous) ═══
        // Resolução 1: usa Mechanical para ser selecionado pelo roll_mechanical existente.
        (
            "SB_E_PIT_02",
            "StreetBased",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "DnfOnly",
            0,
            30,
            "{driver} ficou sem combustível na pista",
            None,
            "Erro de cálculo de combustível",
        ),
        (
            "RS_E_PIT_02",
            "RaceSpec",
            "Endurance",
            "Mechanical",
            "Spontaneous",
            "DnfOnly",
            0,
            30,
            "{driver} ficou sem combustível na pista",
            None,
            "Erro de cálculo de combustível",
        ),
        // ═══ STREETBASED POST-COLLISION (Both formats) ═══
        (
            "SB_COL_01",
            "StreetBased",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            100,
            100,
            "{driver} abandona por pneu cortado após contato",
            Some("{driver} com pneu danificado após contato – perdeu posições"),
            "Pneu cortado",
        ),
        (
            "SB_COL_02",
            "StreetBased",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            70,
            70,
            "{driver} abandona por dano na suspensão após contato – convergência comprometida",
            Some("{driver} com suspensão desalinhada após contato – perdendo ritmo"),
            "Suspensão desalinhada",
        ),
        (
            "SB_COL_03",
            "StreetBased",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            40,
            70,
            "{driver} abandona por superaquecimento – radiador danificado após contato",
            Some("{driver} com temperatura subindo após contato – gerenciando dano"),
            "Radiador furado por detrito",
        ),
        (
            "SB_COL_04",
            "StreetBased",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            40,
            40,
            "{driver} abandona por roda danificada após contato",
            Some("{driver} com roda entortada após contato – vibração no carro"),
            "Roda entortada",
        ),
        (
            "SB_COL_05",
            "StreetBased",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            10,
            40,
            "{driver} abandona por vazamento no arrefecimento após contato",
            Some("{driver} com vazamento detectado após contato"),
            "Mangueira de arrefecimento solta",
        ),
        // ═══ RACESPEC POST-COLLISION (Both formats) ═══
        (
            "RS_COL_01",
            "RaceSpec",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            100,
            100,
            "{driver} abandona por pneu cortado após contato",
            Some("{driver} com pneu danificado após contato"),
            "Pneu cortado",
        ),
        (
            "RS_COL_02",
            "RaceSpec",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            100,
            100,
            "{driver} abandona por dano aerodinâmico – perda crítica de downforce",
            Some("{driver} com dano aerodinâmico após contato – carro instável"),
            "Splitter/difusor danificado",
        ),
        (
            "RS_COL_03",
            "RaceSpec",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            70,
            70,
            "{driver} abandona por dano na suspensão após contato",
            Some("{driver} com suspensão comprometida após contato"),
            "Suspensão desalinhada/entortada",
        ),
        (
            "RS_COL_04",
            "RaceSpec",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            70,
            100,
            "{driver} abandona por superaquecimento dos freios – duto bloqueado",
            Some("{driver} com freios superaquecendo – duto de ar bloqueado"),
            "Duto de freio bloqueado",
        ),
        (
            "RS_COL_05",
            "RaceSpec",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            40,
            70,
            "{driver} abandona por superaquecimento – radiador danificado após contato",
            Some("{driver} com temperatura subindo após contato"),
            "Radiador/intercooler furado",
        ),
        (
            "RS_COL_06",
            "RaceSpec",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            40,
            40,
            "{driver} abandona por roda danificada após contato",
            Some("{driver} com roda entortada após contato"),
            "Roda entortada",
        ),
        (
            "RS_COL_07",
            "RaceSpec",
            "Both",
            "PostCollision",
            "PostCollision",
            "Both",
            20,
            40,
            "{driver} abandona por perda de ABS/controle de tração após contato",
            Some("{driver} sem assistências eletrônicas após contato – cuidado redobrado"),
            "Sensor ABS/TC arrancado",
        ),
        (
            "RS_COL_08",
            "RaceSpec",
            "Endurance",
            "PostCollision",
            "PostCollision",
            "Both",
            0,
            40,
            "{driver} recebe penalidade por iluminação danificada",
            Some("{driver} com faróis danificados – penalidade aplicada"),
            "Farol/luz traseira quebrada",
        ),
        // ═══ STREETBASED DRIVER ERROR SPONTANEOUS ═══
        (
            "SB_S_ERR_01",
            "StreetBased",
            "Both",
            "DriverError",
            "Spontaneous",
            "NonDnfOnly",
            80,
            60,
            "",
            Some("{driver} escapou na saída da curva e perdeu posições"),
            "Saída de pista",
        ),
        (
            "SB_S_ERR_02",
            "StreetBased",
            "Both",
            "DriverError",
            "Spontaneous",
            "NonDnfOnly",
            70,
            50,
            "",
            Some("{driver} rodou na chicane e perdeu posições"),
            "Rodada na chicane",
        ),
        (
            "SB_S_ERR_03",
            "StreetBased",
            "Both",
            "DriverError",
            "Spontaneous",
            "DnfOnly",
            40,
            30,
            "{driver} abandona após erro de pilotagem – saiu da pista",
            None,
            "Erro fatal – saída de pista",
        ),
        // ═══ RACESPEC DRIVER ERROR SPONTANEOUS ═══
        (
            "RS_S_ERR_01",
            "RaceSpec",
            "Both",
            "DriverError",
            "Spontaneous",
            "NonDnfOnly",
            80,
            60,
            "",
            Some("{driver} perdeu a traseira e caiu posições"),
            "Perda de traseira",
        ),
        (
            "RS_S_ERR_02",
            "RaceSpec",
            "Both",
            "DriverError",
            "Spontaneous",
            "NonDnfOnly",
            70,
            50,
            "",
            Some("{driver} errou o ponto de frenagem e perdeu posições"),
            "Frenagem tardia",
        ),
        (
            "RS_S_ERR_03",
            "RaceSpec",
            "Both",
            "DriverError",
            "Spontaneous",
            "DnfOnly",
            40,
            30,
            "{driver} abandona após erro de pilotagem",
            None,
            "Erro fatal – bateu na barreira",
        ),
        // ═══ OPERATIONAL POST SPIN STALL ═══
        (
            "SB_S_PIT_01",
            "StreetBased",
            "Sprint",
            "Operational",
            "PostSpinStall",
            "DnfOnly",
            40,
            0,
            "{driver} rodou e não conseguiu religar o motor",
            None,
            "Rodada com stall",
        ),
        (
            "SB_S_PIT_02",
            "StreetBased",
            "Sprint",
            "Operational",
            "PostSpinStall",
            "DnfOnly",
            60,
            0,
            "{driver} rodou e cravou na brita – não conseguiu sair",
            None,
            "Cravou na brita",
        ),
        (
            "SB_E_PIT_01",
            "StreetBased",
            "Endurance",
            "Operational",
            "PostSpinStall",
            "DnfOnly",
            0,
            40,
            "{driver} rodou e não conseguiu religar",
            None,
            "Rodada com stall",
        ),
        (
            "RS_S_PIT_01",
            "RaceSpec",
            "Sprint",
            "Operational",
            "PostSpinStall",
            "DnfOnly",
            40,
            0,
            "{driver} rodou e não conseguiu religar",
            None,
            "Rodada com stall",
        ),
        (
            "RS_S_PIT_02",
            "RaceSpec",
            "Sprint",
            "Operational",
            "PostSpinStall",
            "DnfOnly",
            60,
            0,
            "{driver} rodou e perdeu tempo de volta – não conseguiu sair da brita",
            None,
            "Cravou na brita",
        ),
        (
            "RS_E_PIT_01",
            "RaceSpec",
            "Endurance",
            "Operational",
            "PostSpinStall",
            "DnfOnly",
            0,
            40,
            "{driver} rodou e não conseguiu religar",
            None,
            "Rodada com stall",
        ),
    ];

    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO incident_catalog
         (id, vehicle_class, race_format, incident_source, trigger_type,
          severity_context, weight_sprint, weight_endurance,
          dnf_template, non_dnf_template, description_short)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
    )?;

    for e in entries {
        stmt.execute(rusqlite::params![
            e.0, e.1, e.2, e.3, e.4, e.5, e.6, e.7, e.8, e.9, e.10
        ])?;
    }

    Ok(())
}

// ── DDL das 19 tabelas ────────────────────────────────────────────────────────

const MIGRATION_V1_DDL: &str = "

-- ── meta: configuração interna do banco ──────────────────────────────────────
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ── config: configurações do usuário (espelha config.json) ───────────────────
CREATE TABLE IF NOT EXISTS config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ── drivers: pilotos (jogador + IA) ──────────────────────────────────────────
CREATE TABLE IF NOT EXISTS drivers (
    id                       TEXT PRIMARY KEY,
    nome                     TEXT NOT NULL,
    is_jogador               INTEGER NOT NULL DEFAULT 0,
    idade                    INTEGER NOT NULL,
    nacionalidade            TEXT NOT NULL,
    genero                   TEXT NOT NULL DEFAULT 'M',
    categoria_atual          TEXT,
    status                   TEXT NOT NULL DEFAULT 'Ativo',
    personalidade_primaria   TEXT,
    personalidade_secundaria TEXT,
    ano_inicio_carreira      INTEGER NOT NULL DEFAULT 2024,

    -- 17 atributos — todos 0-100
    skill                    REAL NOT NULL DEFAULT 50.0,
    consistencia             REAL NOT NULL DEFAULT 50.0,
    racecraft                REAL NOT NULL DEFAULT 50.0,
    defesa                   REAL NOT NULL DEFAULT 50.0,
    ritmo_classificacao      REAL NOT NULL DEFAULT 50.0,
    gestao_pneus             REAL NOT NULL DEFAULT 50.0,
    habilidade_largada       REAL NOT NULL DEFAULT 50.0,
    adaptabilidade           REAL NOT NULL DEFAULT 50.0,
    fator_chuva              REAL NOT NULL DEFAULT 50.0,
    fitness                  REAL NOT NULL DEFAULT 50.0,
    experiencia              REAL NOT NULL DEFAULT 50.0,
    desenvolvimento          REAL NOT NULL DEFAULT 50.0,
    aggression               REAL NOT NULL DEFAULT 50.0,
    smoothness               REAL NOT NULL DEFAULT 50.0,
    midia                    REAL NOT NULL DEFAULT 50.0,
    mentalidade              REAL NOT NULL DEFAULT 50.0,
    confianca                REAL NOT NULL DEFAULT 50.0,

    -- Stats da temporada corrente
    temp_pontos              REAL NOT NULL DEFAULT 0.0,
    temp_vitorias            INTEGER NOT NULL DEFAULT 0,
    temp_podios              INTEGER NOT NULL DEFAULT 0,
    temp_poles               INTEGER NOT NULL DEFAULT 0,
    temp_corridas            INTEGER NOT NULL DEFAULT 0,
    temp_dnfs                INTEGER NOT NULL DEFAULT 0,
    temp_posicao_media       REAL NOT NULL DEFAULT 0.0,

    -- Stats de carreira acumuladas
    carreira_pontos_total    REAL NOT NULL DEFAULT 0.0,
    carreira_vitorias        INTEGER NOT NULL DEFAULT 0,
    carreira_podios          INTEGER NOT NULL DEFAULT 0,
    carreira_poles           INTEGER NOT NULL DEFAULT 0,
    carreira_corridas        INTEGER NOT NULL DEFAULT 0,
    carreira_temporadas      INTEGER NOT NULL DEFAULT 0,
    carreira_titulos         INTEGER NOT NULL DEFAULT 0,
    carreira_dnfs            INTEGER NOT NULL DEFAULT 0,

    -- Tracking dinâmico
    motivacao                REAL NOT NULL DEFAULT 75.0,
    historico_circuitos      TEXT NOT NULL DEFAULT '{}',
    ultimos_resultados       TEXT NOT NULL DEFAULT '[]',
    melhor_resultado_temp    INTEGER,
    temporadas_na_categoria  INTEGER NOT NULL DEFAULT 0,
    corridas_na_categoria    INTEGER NOT NULL DEFAULT 0,
    temporadas_motivacao_baixa INTEGER NOT NULL DEFAULT 0
);

-- ── teams: equipes ────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS teams (
    id                   TEXT PRIMARY KEY,
    nome                 TEXT NOT NULL,
    categoria            TEXT NOT NULL,
    is_player_team       INTEGER NOT NULL DEFAULT 0,

    -- Performance (Módulo 16)
    car_performance      REAL NOT NULL DEFAULT 50.0,
    reliability          REAL NOT NULL DEFAULT 50.0,
    budget               REAL NOT NULL DEFAULT 1000000.0,
    prestige             REAL NOT NULL DEFAULT 50.0,

    -- Hierarquia (Módulo 17)
    hierarquia_status    TEXT NOT NULL DEFAULT 'Independente',
    parent_team_id       TEXT,
    aceita_rookies       INTEGER NOT NULL DEFAULT 1,
    meta_posicao         INTEGER NOT NULL DEFAULT 10,

    -- Stats da temporada corrente (Módulo 18)
    temp_pontos          REAL NOT NULL DEFAULT 0.0,
    temp_posicao         INTEGER NOT NULL DEFAULT 0,
    temp_vitorias        INTEGER NOT NULL DEFAULT 0,

    -- Stats de carreira
    carreira_titulos     INTEGER NOT NULL DEFAULT 0,
    carreira_vitorias    INTEGER NOT NULL DEFAULT 0,

    created_at           TEXT NOT NULL DEFAULT ''
);

-- ── contracts: contratos piloto↔equipe ───────────────────────────────────────
CREATE TABLE IF NOT EXISTS contracts (
    id                TEXT PRIMARY KEY,
    piloto_id         TEXT NOT NULL,
    equipe_id         TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'Ativo',
    papel             TEXT NOT NULL DEFAULT 'Titular',
    salario           REAL NOT NULL DEFAULT 0.0,
    temporada_inicio  TEXT NOT NULL,
    temporada_fim     TEXT NOT NULL,
    clausulas         TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (piloto_id) REFERENCES drivers(id),
    FOREIGN KEY (equipe_id) REFERENCES teams(id)
);

-- ── seasons: temporadas ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS seasons (
    id      TEXT PRIMARY KEY,
    numero  INTEGER NOT NULL,
    ano     INTEGER NOT NULL,
    status  TEXT NOT NULL DEFAULT 'Ativa'
);

-- ── calendar: calendário de corridas ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS calendar (
    id           TEXT PRIMARY KEY,
    temporada_id TEXT NOT NULL,
    rodada       INTEGER NOT NULL,
    pista        TEXT NOT NULL,
    categoria    TEXT NOT NULL,
    clima        TEXT NOT NULL DEFAULT 'Seco',
    duracao      INTEGER NOT NULL DEFAULT 60,
    data         TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (temporada_id) REFERENCES seasons(id)
);

-- ── races: corridas disputadas ────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS races (
    id           TEXT PRIMARY KEY,
    temporada_id TEXT NOT NULL,
    calendar_id  TEXT NOT NULL,
    rodada       INTEGER NOT NULL,
    pista        TEXT NOT NULL,
    data         TEXT NOT NULL DEFAULT '',
    clima        TEXT NOT NULL DEFAULT 'Seco',
    status       TEXT NOT NULL DEFAULT 'Pendente',
    FOREIGN KEY (temporada_id) REFERENCES seasons(id),
    FOREIGN KEY (calendar_id)  REFERENCES calendar(id)
);

-- ── race_results: resultados por piloto por corrida ───────────────────────────
CREATE TABLE IF NOT EXISTS race_results (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    race_id             TEXT NOT NULL,
    piloto_id           TEXT NOT NULL,
    equipe_id           TEXT NOT NULL,
    posicao_largada     INTEGER NOT NULL DEFAULT 0,
    posicao_final       INTEGER NOT NULL DEFAULT 0,
    voltas_completadas  INTEGER NOT NULL DEFAULT 0,
    dnf                 INTEGER NOT NULL DEFAULT 0,
    pontos              REAL NOT NULL DEFAULT 0.0,
    tempo_total         REAL NOT NULL DEFAULT 0.0,
    fastest_lap         INTEGER NOT NULL DEFAULT 0,
    dnf_reason          TEXT,
    dnf_segment         TEXT,
    incidents_count     INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (race_id)    REFERENCES races(id),
    FOREIGN KEY (piloto_id)  REFERENCES drivers(id),
    FOREIGN KEY (equipe_id)  REFERENCES teams(id)
);

-- ── standings: classificação por temporada ────────────────────────────────────
CREATE TABLE IF NOT EXISTS standings (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    temporada_id TEXT NOT NULL,
    piloto_id    TEXT NOT NULL,
    equipe_id    TEXT NOT NULL,
    categoria    TEXT NOT NULL,
    posicao      INTEGER NOT NULL DEFAULT 0,
    pontos       REAL NOT NULL DEFAULT 0.0,
    vitorias     INTEGER NOT NULL DEFAULT 0,
    podios       INTEGER NOT NULL DEFAULT 0,
    poles        INTEGER NOT NULL DEFAULT 0,
    corridas     INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (temporada_id) REFERENCES seasons(id),
    FOREIGN KEY (piloto_id)    REFERENCES drivers(id),
    FOREIGN KEY (equipe_id)    REFERENCES teams(id)
);

-- ── licenses: licenças por piloto ────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS licenses (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    piloto_id                TEXT NOT NULL,
    nivel                    TEXT NOT NULL,
    categoria_origem         TEXT NOT NULL,
    data_obtencao            TEXT NOT NULL DEFAULT '',
    temporadas_na_categoria  INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (piloto_id) REFERENCES drivers(id)
);

-- ── injuries: lesões ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS injuries (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    piloto_id         TEXT NOT NULL,
    tipo              TEXT NOT NULL DEFAULT 'Leve',
    corridas_restantes INTEGER NOT NULL DEFAULT 0,
    temporada_id      TEXT NOT NULL,
    descricao         TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (piloto_id)    REFERENCES drivers(id),
    FOREIGN KEY (temporada_id) REFERENCES seasons(id)
);

-- ── market: estado geral do mercado de transferências ────────────────────────
CREATE TABLE IF NOT EXISTS market (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    temporada_id TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'Fechado',
    fase         TEXT NOT NULL DEFAULT 'PreTemporada',
    inicio       TEXT NOT NULL DEFAULT '',
    fim          TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (temporada_id) REFERENCES seasons(id)
);

-- ── market_proposals: propostas equipe→piloto ─────────────────────────────────
CREATE TABLE IF NOT EXISTS market_proposals (
    id              TEXT PRIMARY KEY,
    temporada_id    TEXT NOT NULL,
    equipe_id       TEXT NOT NULL,
    piloto_id       TEXT NOT NULL,
    papel           TEXT NOT NULL DEFAULT 'Titular',
    salario         REAL NOT NULL DEFAULT 0.0,
    status          TEXT NOT NULL DEFAULT 'Pendente',
    motivo_recusa   TEXT,
    criado_em       TEXT NOT NULL DEFAULT '',
    respondido_em   TEXT,
    FOREIGN KEY (temporada_id) REFERENCES seasons(id),
    FOREIGN KEY (equipe_id)    REFERENCES teams(id),
    FOREIGN KEY (piloto_id)    REFERENCES drivers(id)
);

-- ── player_special_offers: convocações especiais recebidas pelo jogador ────────────
CREATE TABLE IF NOT EXISTS player_special_offers (
    id                TEXT PRIMARY KEY,
    season_id         TEXT NOT NULL,
    player_driver_id  TEXT NOT NULL,
    team_id           TEXT NOT NULL,
    team_name         TEXT NOT NULL,
    special_category  TEXT NOT NULL,
    class_name        TEXT NOT NULL,
    papel             TEXT NOT NULL DEFAULT 'Numero2',
    status            TEXT NOT NULL DEFAULT 'Pendente',
    created_at        TEXT NOT NULL DEFAULT '',
    responded_at      TEXT,
    FOREIGN KEY (season_id) REFERENCES seasons(id),
    FOREIGN KEY (player_driver_id) REFERENCES drivers(id),
    FOREIGN KEY (team_id) REFERENCES teams(id)
);

-- ── news: notícias do simulador ───────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS news (
    id           TEXT PRIMARY KEY,
    tipo         TEXT NOT NULL,
    titulo       TEXT NOT NULL,
    texto        TEXT NOT NULL,
    chave_dedup  TEXT UNIQUE,
    temporada_id TEXT NOT NULL,
    rodada       INTEGER NOT NULL DEFAULT 0,
    criado_em    TEXT NOT NULL DEFAULT '',
    lida         INTEGER NOT NULL DEFAULT 0
);

-- ── rivalries: rivalidades entre pilotos ─────────────────────────────────────
CREATE TABLE IF NOT EXISTS rivalries (
    id                 TEXT PRIMARY KEY,
    piloto1_id         TEXT NOT NULL,
    piloto2_id         TEXT NOT NULL,
    intensidade        REAL NOT NULL DEFAULT 0.0,
    tipo               TEXT NOT NULL DEFAULT 'Normal',
    criado_em          TEXT NOT NULL DEFAULT '',
    ultima_atualizacao TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (piloto1_id) REFERENCES drivers(id),
    FOREIGN KEY (piloto2_id) REFERENCES drivers(id)
);

-- ── retired: pilotos aposentados (snapshot histórico) ────────────────────────
CREATE TABLE IF NOT EXISTS retired (
    piloto_id                TEXT PRIMARY KEY,
    nome                     TEXT NOT NULL,
    temporada_aposentadoria  TEXT NOT NULL,
    categoria_final          TEXT NOT NULL,
    estatisticas             TEXT NOT NULL DEFAULT '{}',
    motivo                   TEXT NOT NULL DEFAULT 'Aposentadoria'
);

-- ── history_seasons: resultado final de cada temporada ───────────────────────
CREATE TABLE IF NOT EXISTS history_seasons (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    temporada_id        TEXT NOT NULL,
    ano                 INTEGER NOT NULL,
    categoria           TEXT NOT NULL,
    campeao_piloto_id   TEXT NOT NULL,
    campeao_equipe_id   TEXT NOT NULL,
    classificacao_final TEXT NOT NULL DEFAULT '[]'
);

-- ── history_general: dados históricos genéricos (chave/valor) ────────────────
CREATE TABLE IF NOT EXISTS history_general (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT ''
);

-- ── Índices nas colunas mais consultadas ──────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_drivers_status    ON drivers(status);
CREATE INDEX IF NOT EXISTS idx_drivers_categoria ON drivers(categoria_atual);
CREATE INDEX IF NOT EXISTS idx_teams_categoria   ON teams(categoria);
CREATE INDEX IF NOT EXISTS idx_contracts_piloto  ON contracts(piloto_id);
CREATE INDEX IF NOT EXISTS idx_contracts_equipe  ON contracts(equipe_id);
CREATE INDEX IF NOT EXISTS idx_contracts_status  ON contracts(status);
CREATE INDEX IF NOT EXISTS idx_calendar_temporada ON calendar(temporada_id);
CREATE INDEX IF NOT EXISTS idx_races_temporada   ON races(temporada_id);
CREATE INDEX IF NOT EXISTS idx_race_results_race ON race_results(race_id);
CREATE INDEX IF NOT EXISTS idx_race_results_piloto ON race_results(piloto_id);
CREATE INDEX IF NOT EXISTS idx_standings_temporada ON standings(temporada_id);
CREATE INDEX IF NOT EXISTS idx_standings_piloto  ON standings(piloto_id);
CREATE INDEX IF NOT EXISTS idx_news_temporada    ON news(temporada_id);
CREATE INDEX IF NOT EXISTS idx_news_lida         ON news(lida);
CREATE INDEX IF NOT EXISTS idx_injuries_piloto   ON injuries(piloto_id);
CREATE INDEX IF NOT EXISTS idx_market_proposals_piloto ON market_proposals(piloto_id);
CREATE INDEX IF NOT EXISTS idx_market_proposals_equipe ON market_proposals(equipe_id);
CREATE INDEX IF NOT EXISTS idx_player_special_offers_player ON player_special_offers(player_driver_id);
CREATE INDEX IF NOT EXISTS idx_player_special_offers_team ON player_special_offers(team_id);
CREATE INDEX IF NOT EXISTS idx_rivalries_piloto1      ON rivalries(piloto1_id);
CREATE INDEX IF NOT EXISTS idx_rivalries_piloto2      ON rivalries(piloto2_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_rivalries_pair_unique
    ON rivalries(piloto1_id, piloto2_id);

";

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
        ("current_season", "1"),
        ("current_year", "2024"),
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

    #[test]
    fn test_migrate_v5_preserves_existing_race_results_rows() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY
            );
            CREATE TABLE drivers (
                id TEXT PRIMARY KEY
            );
            CREATE TABLE teams (
                id TEXT PRIMARY KEY
            );
            INSERT INTO calendar (id) VALUES ('R001');
            INSERT INTO drivers (id) VALUES ('P001');
            INSERT INTO teams (id) VALUES ('T001');

            CREATE TABLE race_results (
                id TEXT PRIMARY KEY,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0
            );

            INSERT INTO race_results (id, race_id, piloto_id, equipe_id, posicao_final, pontos)
            VALUES ('legacy', 'R001', 'P001', 'T001', 3, 15.0);
            ",
        )
        .expect("legacy race_results");

        migrate_v5(&conn).expect("migration should preserve rows");

        let row: (String, String, String, i64, f64) = conn
            .query_row(
                "SELECT race_id, piloto_id, equipe_id, posicao_final, pontos FROM race_results",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("migrated row");

        assert_eq!(row.0, "R001");
        assert_eq!(row.1, "P001");
        assert_eq!(row.2, "T001");
        assert_eq!(row.3, 3);
        assert_eq!(row.4, 15.0);
    }

    #[test]
    fn test_migrate_v6_preserves_existing_injuries_rows() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'EmAndamento'
            );
            INSERT INTO seasons (id, numero, ano, status) VALUES ('S001', 1, 2024, 'EmAndamento');

            CREATE TABLE drivers (
                id TEXT PRIMARY KEY
            );
            INSERT INTO drivers (id) VALUES ('P001');

            CREATE TABLE injuries (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                tipo TEXT NOT NULL DEFAULT 'Leve',
                corridas_restantes INTEGER NOT NULL DEFAULT 0,
                temporada_id TEXT NOT NULL,
                descricao TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO injuries (id, piloto_id, tipo, corridas_restantes, temporada_id, descricao)
            VALUES ('I001', 'P001', 'Grave', 2, 'S001', 'legacy injury');
            ",
        )
        .expect("legacy injuries");

        migrate_v6(&conn).expect("migration should preserve rows");

        let row: (String, String, i64, i64, i64) = conn
            .query_row(
                "SELECT pilot_id, type, races_total, races_remaining, season FROM injuries",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("migrated injury");

        assert_eq!(row.0, "P001");
        assert_eq!(row.1, "Grave");
        assert_eq!(row.2, 2);
        assert_eq!(row.3, 2);
        assert_eq!(row.4, 1);
    }

    #[test]
    fn test_migrate_v15_preserves_existing_driver_archive_rows() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE driver_season_results_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL,
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL,
                archived_at TEXT NOT NULL
            );

            INSERT INTO driver_season_results_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json, archived_at
            ) VALUES (
                'P001', 3, 2026, 'Piloto Legado', 'gt3', 2, 180.5, '{\"ok\":true}', '2026-12-01 00:00:00'
            );
            ",
        )
        .expect("legacy archive");

        migrate_v15(&conn).expect("migration should preserve rows");

        let row: (String, i64, String, f64) = conn
            .query_row(
                "SELECT piloto_id, season_number, categoria, pontos FROM driver_season_archive",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("migrated archive row");

        assert_eq!(row.0, "P001");
        assert_eq!(row.1, 3);
        assert_eq!(row.2, "gt3");
        assert_eq!(row.3, 180.5);
    }

    #[test]
    fn test_migrate_v16_allows_partial_schema_without_teams_table() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        migrate_v16(&conn).expect("migration should ignore missing teams table");
    }

    #[test]
    fn test_migrate_v18_rejects_duplicate_active_contracts_for_same_pilot_and_type() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE contracts (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                piloto_nome TEXT NOT NULL DEFAULT '',
                equipe_nome TEXT NOT NULL DEFAULT '',
                temporada_inicio INTEGER NOT NULL DEFAULT 1,
                duracao_anos INTEGER NOT NULL DEFAULT 1,
                temporada_fim INTEGER NOT NULL DEFAULT 1,
                salario REAL NOT NULL DEFAULT 0.0,
                salario_anual REAL NOT NULL DEFAULT 0.0,
                papel TEXT NOT NULL DEFAULT 'Numero2',
                status TEXT NOT NULL DEFAULT 'Ativo',
                tipo TEXT NOT NULL DEFAULT 'Regular',
                categoria TEXT NOT NULL DEFAULT '',
                classe TEXT,
                created_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO contracts (id, piloto_id, equipe_id, status, tipo, categoria)
            VALUES
                ('C001', 'P001', 'T001', 'Ativo', 'Regular', 'gt3'),
                ('C002', 'P001', 'T002', 'Ativo', 'Regular', 'gt4');
            ",
        )
        .expect("duplicate active contracts");

        let err = migrate_v18(&conn).expect_err("duplicate active contracts should fail");
        assert!(
            matches!(err, DbError::Migration(_)),
            "expected migration error, got {err:?}"
        );
    }

    #[test]
    fn test_run_pending_v18_adds_unique_index_for_active_contracts() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '17');

            CREATE TABLE contracts (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                piloto_nome TEXT NOT NULL DEFAULT '',
                equipe_nome TEXT NOT NULL DEFAULT '',
                temporada_inicio INTEGER NOT NULL DEFAULT 1,
                duracao_anos INTEGER NOT NULL DEFAULT 1,
                temporada_fim INTEGER NOT NULL DEFAULT 1,
                salario REAL NOT NULL DEFAULT 0.0,
                salario_anual REAL NOT NULL DEFAULT 0.0,
                papel TEXT NOT NULL DEFAULT 'Numero2',
                status TEXT NOT NULL DEFAULT 'Ativo',
                tipo TEXT NOT NULL DEFAULT 'Regular',
                categoria TEXT NOT NULL DEFAULT '',
                classe TEXT,
                created_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO contracts (id, piloto_id, equipe_id, status, tipo, categoria)
            VALUES ('C001', 'P001', 'T001', 'Ativo', 'Regular', 'gt3');
            ",
        )
        .expect("v17 schema");

        run_pending(&conn).expect("migration to v18 should succeed");

        let duplicate_insert = conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, equipe_id, status, tipo, categoria
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            ("C002", "P001", "T002", "Ativo", "Regular", "gt4"),
        );
        assert!(
            duplicate_insert.is_err(),
            "unique index should reject duplicate active contract for same pilot/type"
        );
    }

    #[test]
    fn test_run_pending_migrates_teams_to_v2_and_preserves_existing_data() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '1');

            CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL,
                is_player_team INTEGER NOT NULL DEFAULT 0,
                car_performance REAL NOT NULL DEFAULT 50.0,
                reliability REAL NOT NULL DEFAULT 50.0,
                budget REAL NOT NULL DEFAULT 1000000.0,
                prestige REAL NOT NULL DEFAULT 50.0,
                hierarquia_status TEXT NOT NULL DEFAULT 'Independente',
                parent_team_id TEXT,
                aceita_rookies INTEGER NOT NULL DEFAULT 1,
                meta_posicao INTEGER NOT NULL DEFAULT 10,
                temp_pontos REAL NOT NULL DEFAULT 0.0,
                temp_posicao INTEGER NOT NULL DEFAULT 0,
                temp_vitorias INTEGER NOT NULL DEFAULT 0,
                carreira_titulos INTEGER NOT NULL DEFAULT 0,
                carreira_vitorias INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO teams (
                id, nome, categoria, temp_pontos, temp_vitorias, carreira_vitorias, created_at
            ) VALUES (
                'T001', 'Equipe Legada', 'gt3', 42.0, 3, 9, '2026-01-01T12:00:00'
            );
            ",
        )
        .expect("legacy schema should be created");

        run_pending(&conn).expect("migration should succeed");

        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );
        assert!(column_exists(&conn, "teams", "nome_curto"));
        assert!(column_exists(&conn, "teams", "stats_vitorias"));
        assert!(column_exists(&conn, "teams", "stats_pontos"));
        assert!(column_exists(&conn, "teams", "historico_vitorias"));
        assert!(column_exists(&conn, "teams", "updated_at"));
        assert!(column_exists(&conn, "teams", "car_build_profile"));
        assert!(column_exists(&conn, "teams", "pit_strategy_risk"));
        assert!(column_exists(&conn, "teams", "pit_crew_quality"));

        let row: (String, i64, i64, i64, String, f64, f64) = conn
            .query_row(
                "SELECT nome_curto, stats_vitorias, stats_pontos, historico_vitorias, car_build_profile,
                        pit_strategy_risk, pit_crew_quality
                 FROM teams WHERE id = 'T001'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("migrated row");

        assert_eq!(row.0, "Equipe Legada");
        assert_eq!(row.4, "balanced");
        assert_eq!(row.5, 50.0);
        assert_eq!(row.6, 50.0);
        assert_eq!(row.1, 3);
        assert_eq!(row.2, 42);
        assert_eq!(row.3, 9);

        let idx_ativa_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_teams_ativa'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .expect("index query");
        assert!(idx_ativa_exists);
    }

    #[test]
    fn test_run_pending_v25_adds_team_finance_columns_with_safe_defaults() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '24');

            CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                nome_curto TEXT NOT NULL DEFAULT '',
                categoria TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO teams (id, nome, nome_curto, categoria, created_at, updated_at)
            VALUES ('T001', 'Equipe Financeira', 'EF', 'gt3', '2026-01-01', '2026-01-01');
            ",
        )
        .expect("legacy v24 schema");

        run_pending(&conn).expect("migration to v25 should succeed");

        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );
        assert!(column_exists(&conn, "teams", "cash_balance"));
        assert!(column_exists(&conn, "teams", "debt_balance"));
        assert!(column_exists(&conn, "teams", "financial_state"));
        assert!(column_exists(&conn, "teams", "season_strategy"));
        assert!(column_exists(&conn, "teams", "last_round_income"));
        assert!(column_exists(&conn, "teams", "last_round_expenses"));
        assert!(column_exists(&conn, "teams", "last_round_net"));
        assert!(column_exists(&conn, "teams", "parachute_payment_remaining"));

        let row: (f64, f64, String, String, f64, f64, f64, f64) = conn
            .query_row(
                "SELECT cash_balance, debt_balance, financial_state, season_strategy,
                        last_round_income, last_round_expenses, last_round_net,
                        parachute_payment_remaining
                 FROM teams WHERE id = 'T001'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .expect("migrated finance row");

        assert_eq!(row.0, 0.0);
        assert_eq!(row.1, 0.0);
        assert_eq!(row.2, "stable");
        assert_eq!(row.3, "balanced");
        assert_eq!(row.4, 0.0);
        assert_eq!(row.5, 0.0);
        assert_eq!(row.6, 0.0);
        assert_eq!(row.7, 0.0);
    }

    #[test]
    fn test_run_pending_migrates_contracts_to_v3_and_backfills_new_fields() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '2');

            CREATE TABLE drivers (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL
            );

            CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL
            );

            CREATE TABLE contracts (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'Ativo',
                papel TEXT NOT NULL DEFAULT 'Titular',
                salario REAL NOT NULL DEFAULT 0.0,
                temporada_inicio TEXT NOT NULL,
                temporada_fim TEXT NOT NULL,
                clausulas TEXT NOT NULL DEFAULT '{}'
            );

            INSERT INTO drivers (id, nome) VALUES ('P001', 'Piloto Legado');
            INSERT INTO teams (id, nome, categoria) VALUES ('T001', 'Equipe Legada', 'gt3');
            INSERT INTO contracts (
                id, piloto_id, equipe_id, status, papel, salario, temporada_inicio, temporada_fim
            ) VALUES (
                'C001', 'P001', 'T001', 'Ativo', 'Titular', 150000.0, '1', '3'
            );
            ",
        )
        .expect("legacy schema should be created");

        run_pending(&conn).expect("migration should succeed");

        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );
        assert!(column_exists(&conn, "contracts", "piloto_nome"));
        assert!(column_exists(&conn, "contracts", "equipe_nome"));
        assert!(column_exists(&conn, "contracts", "duracao_anos"));
        assert!(column_exists(&conn, "contracts", "salario_anual"));
        assert!(column_exists(&conn, "contracts", "categoria"));
        assert!(column_exists(&conn, "contracts", "created_at"));

        let row: (String, String, i64, f64, String, String) = conn
            .query_row(
                "SELECT piloto_nome, equipe_nome, duracao_anos, salario_anual, categoria, papel
                 FROM contracts WHERE id = 'C001'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("migrated row");

        assert_eq!(row.0, "Piloto Legado");
        assert_eq!(row.1, "Equipe Legada");
        assert_eq!(row.2, 3);
        assert_eq!(row.3, 150000.0);
        assert_eq!(row.4, "gt3");
        assert_eq!(row.5, "Numero1");
    }

    #[test]
    fn test_run_pending_migrates_seasons_and_calendar_to_v4() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '3');

            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'Ativa'
            );

            CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL
            );

            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                rodada INTEGER NOT NULL,
                pista TEXT NOT NULL,
                categoria TEXT NOT NULL,
                clima TEXT NOT NULL DEFAULT 'Seco',
                duracao INTEGER NOT NULL DEFAULT 60,
                data TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO seasons (id, numero, ano, status) VALUES ('S001', 1, 2024, 'Ativa');
            INSERT INTO calendar (id, temporada_id, rodada, pista, categoria, clima, duracao)
            VALUES ('R001', 'S001', 1, 'Laguna Seca', 'mazda_rookie', 'Seco', 15);
            ",
        )
        .expect("legacy schema should be created");

        run_pending(&conn).expect("migration should succeed");

        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );
        assert!(column_exists(&conn, "seasons", "rodada_atual"));
        assert!(column_exists(&conn, "seasons", "updated_at"));
        assert!(column_exists(&conn, "calendar", "season_id"));
        assert!(column_exists(&conn, "calendar", "track_name"));
        assert!(column_exists(&conn, "calendar", "duracao_corrida_min"));
        assert!(column_exists(&conn, "calendar", "status"));

        let season_row: (String, i64) = conn
            .query_row(
                "SELECT status, rodada_atual FROM seasons WHERE id = 'S001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("season row");
        assert_eq!(season_row.0, "EmAndamento");
        assert_eq!(season_row.1, 1);

        let calendar_row: (String, String, i64, String) = conn
            .query_row(
                "SELECT season_id, track_name, duracao_corrida_min, clima FROM calendar WHERE id = 'R001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("calendar row");
        assert_eq!(calendar_row.0, "S001");
        assert_eq!(calendar_row.1, "Laguna Seca");
        assert_eq!(calendar_row.2, 15);
        assert_eq!(calendar_row.3, "Dry");
    }

    #[test]
    fn test_run_pending_migrates_to_v14_creates_incident_catalog() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        // Schema mínimo simulando um DB na versão 13
        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '13');

            CREATE TABLE race_results (
                id            TEXT PRIMARY KEY,
                race_id       TEXT NOT NULL,
                piloto_id     TEXT NOT NULL,
                equipe_id     TEXT NOT NULL,
                posicao_final INTEGER NOT NULL DEFAULT 0,
                pontos        REAL NOT NULL DEFAULT 0.0,
                gap_to_winner_ms REAL NOT NULL DEFAULT 0.0,
                final_tire_wear  REAL NOT NULL DEFAULT 1.0
            );

            CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL
            );
            ",
        )
        .expect("legacy v13 schema");

        run_pending(&conn).expect("migration to v14 should succeed");

        // schema_version atualizado para a versão corrente
        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );

        // Tabela incident_catalog criada
        assert!(
            table_exists(&conn, "incident_catalog").expect("table_exists"),
            "incident_catalog table must exist after v14"
        );

        // Mais de 30 entries seed inseridos
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM incident_catalog", [], |row| {
                row.get(0)
            })
            .expect("count incident_catalog");
        assert!(count > 30, "seed should insert >30 entries, got {count}");

        // Colunas adicionadas em race_results
        assert!(
            column_exists(&conn, "race_results", "dnf_catalog_id"),
            "race_results must have dnf_catalog_id"
        );
        assert!(
            column_exists(&conn, "race_results", "damage_origin_segment"),
            "race_results must have damage_origin_segment"
        );

        // Entry SB_S_MEC_01 tem vehicle_class = 'StreetBased'
        let vc: String = conn
            .query_row(
                "SELECT vehicle_class FROM incident_catalog WHERE id = 'SB_S_MEC_01'",
                [],
                |row| row.get(0),
            )
            .expect("SB_S_MEC_01 must exist");
        assert_eq!(vc, "StreetBased");

        // SB_E_PIT_02 tem incident_source = 'Mechanical' (não 'Operational')
        let src: String = conn
            .query_row(
                "SELECT incident_source FROM incident_catalog WHERE id = 'SB_E_PIT_02'",
                [],
                |row| row.get(0),
            )
            .expect("SB_E_PIT_02 must exist");
        assert_eq!(
            src, "Mechanical",
            "SB_E_PIT_02 must use Mechanical source (Resolução 1)"
        );
    }

    #[test]
    fn test_run_pending_v19_normalizes_legacy_active_status_and_adds_unique_index() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '18');

            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                status TEXT NOT NULL,
                rodada_atual INTEGER NOT NULL DEFAULT 1,
                fase TEXT NOT NULL DEFAULT 'BlocoRegular',
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES ('S001', 1, 2024, 'Ativa', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
            ",
        )
        .expect("legacy v18 schema");

        run_pending(&conn).expect("migration to v19 should succeed");

        let status: String = conn
            .query_row("SELECT status FROM seasons WHERE id = 'S001'", [], |row| {
                row.get(0)
            })
            .expect("season status");
        assert_eq!(status, "EmAndamento");

        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_seasons_single_active'",
                [],
                |row| row.get(0),
            )
            .expect("index count");
        assert_eq!(index_count, 1);

        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );
    }

    #[test]
    fn test_run_pending_v19_rejects_duplicate_active_seasons() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '18');

            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                status TEXT NOT NULL,
                rodada_atual INTEGER NOT NULL DEFAULT 1,
                fase TEXT NOT NULL DEFAULT 'BlocoRegular',
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
            VALUES
                ('S001', 1, 2024, 'EmAndamento', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
                ('S002', 2, 2025, 'EmAndamento', 1, 'BlocoRegular', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
            ",
        )
        .expect("legacy v18 schema");

        let err = run_pending(&conn).expect_err("duplicate active seasons should fail");
        assert!(err.to_string().contains("temporadas ativas duplicadas"));
    }

    #[test]
    fn test_run_pending_v20_adds_unique_index_for_race_results() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '19');

            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL
            );

            INSERT INTO race_results (race_id, piloto_id, equipe_id)
            VALUES ('R001', 'P001', 'T001');
            ",
        )
        .expect("legacy v19 schema");

        run_pending(&conn).expect("migration to v20 should succeed");

        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_race_results_unique_race_pilot'",
                [],
                |row| row.get(0),
            )
            .expect("index count");
        assert_eq!(index_count, 1);

        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );
    }

    #[test]
    fn test_run_pending_v20_rejects_duplicate_race_results() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '19');

            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL
            );

            INSERT INTO race_results (race_id, piloto_id, equipe_id)
            VALUES
                ('R001', 'P001', 'T001'),
                ('R001', 'P001', 'T001');
            ",
        )
        .expect("legacy v19 schema with duplicates");

        let err = run_pending(&conn).expect_err("duplicate race results should fail");
        assert!(err
            .to_string()
            .contains("resultados duplicados em race_results"));
    }

    #[test]
    fn test_run_pending_v21_adds_unique_index_for_active_injuries() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '20');

            CREATE TABLE injuries (
                id TEXT PRIMARY KEY,
                pilot_id TEXT NOT NULL,
                type TEXT NOT NULL,
                modifier REAL NOT NULL DEFAULT 1.0,
                races_total INTEGER NOT NULL,
                races_remaining INTEGER NOT NULL,
                skill_penalty REAL NOT NULL DEFAULT 0.0,
                season INTEGER NOT NULL,
                race_occurred TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1
            );

            INSERT INTO injuries (
                id, pilot_id, type, modifier, races_total, races_remaining, skill_penalty, season, race_occurred, active
            ) VALUES (
                'I001', 'P001', 'Leve', 0.95, 2, 2, 0.05, 1, 'R001', 1
            );
            ",
        )
        .expect("legacy v20 schema");

        run_pending(&conn).expect("migration to v21 should succeed");

        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_injuries_single_active_per_pilot'",
                [],
                |row| row.get(0),
            )
            .expect("index count");
        assert_eq!(index_count, 1);

        assert_eq!(
            get_schema_version(&conn).expect("schema version"),
            CURRENT_VERSION
        );
    }

    #[test]
    fn test_run_pending_v21_rejects_duplicate_active_injuries() {
        let conn = Connection::open_in_memory().expect("in-memory db");

        conn.execute_batch(
            "
            CREATE TABLE meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO meta (key, value) VALUES ('schema_version', '20');

            CREATE TABLE injuries (
                id TEXT PRIMARY KEY,
                pilot_id TEXT NOT NULL,
                type TEXT NOT NULL,
                modifier REAL NOT NULL DEFAULT 1.0,
                races_total INTEGER NOT NULL,
                races_remaining INTEGER NOT NULL,
                skill_penalty REAL NOT NULL DEFAULT 0.0,
                season INTEGER NOT NULL,
                race_occurred TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1
            );

            INSERT INTO injuries (
                id, pilot_id, type, modifier, races_total, races_remaining, skill_penalty, season, race_occurred, active
            ) VALUES
                ('I001', 'P001', 'Leve', 0.95, 2, 2, 0.05, 1, 'R001', 1),
                ('I002', 'P001', 'Moderada', 0.85, 4, 4, 0.10, 1, 'R002', 1);
            ",
        )
        .expect("legacy v20 schema with duplicates");

        let err = run_pending(&conn).expect_err("duplicate active injuries should fail");
        assert!(err
            .to_string()
            .contains("lesoes ativas duplicadas em injuries"));
    }

    fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({})", table))
            .expect("pragma table_info");
        let mut rows = stmt.query([]).expect("query pragma");

        while let Some(row) = rows.next().expect("next row") {
            let name: String = row.get("name").expect("column name");
            if name == column {
                return true;
            }
        }

        false
    }
}
