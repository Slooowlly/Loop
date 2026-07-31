use rusqlite::Connection;

use super::campeoes::build_band_champions;
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

// Schema variant with the columns the in-progress-season injection needs: the
// live `teams` stats/`ativa` flag and a `seasons` table to detect an active,
// not-yet-archived season.
fn setup_conn_with_live_season() -> Connection {
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
            cor_secundaria TEXT NOT NULL DEFAULT '#0d1727',
            ativa INTEGER NOT NULL DEFAULT 1,
            stats_pontos INTEGER NOT NULL DEFAULT 0,
            stats_vitorias INTEGER NOT NULL DEFAULT 0,
            stats_melhor_resultado INTEGER NOT NULL DEFAULT 99
        );
        CREATE TABLE seasons (
            id TEXT PRIMARY KEY,
            numero INTEGER NOT NULL,
            ano INTEGER NOT NULL,
            status TEXT NOT NULL
        );
        -- Calendario e resultados sao a fonte real das categorias multiclasse:
        -- sem eles nao da para testar a divergencia com stats_pontos.
        CREATE TABLE calendar (
            id TEXT PRIMARY KEY,
            season_id TEXT,
            temporada_id TEXT,
            categoria TEXT NOT NULL
        );
        CREATE TABLE race_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            race_id TEXT NOT NULL,
            piloto_id TEXT NOT NULL DEFAULT '',
            equipe_id TEXT NOT NULL DEFAULT '',
            posicao_final INTEGER NOT NULL DEFAULT 0,
            dnf INTEGER NOT NULL DEFAULT 0,
            pontos REAL NOT NULL DEFAULT 0.0
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

#[test]
fn in_progress_season_injects_current_division_and_keeps_last_champion_crown() {
    let conn = setup_conn_with_live_season();
    conn.execute_batch(
        "
        -- 2024 (season 4) is FINISHED and archived: T_STAY campeã da amador,
        -- T_DROP em último (10º).
        INSERT INTO seasons (id, numero, ano, status) VALUES
            ('S4', 4, 2024, 'Finalizada'),
            ('S5', 5, 2025, 'Ativa');

        -- Divisão AO VIVO (2025, em andamento): T_STAY seguiu na amador,
        -- T_DROP foi rebaixada pro rookie.
        INSERT INTO teams (id, nome, nome_curto, categoria, classe, ativa) VALUES
            ('T_STAY', 'Stay Racing', 'STY', 'mazda_amador', NULL, 1),
            ('T_DROP', 'Drop Racing', 'DRP', 'mazda_rookie', NULL, 1);

        INSERT INTO team_season_archive (
            team_id, season_number, ano, categoria, classe, posicao_campeonato,
            pontos, vitorias, titulos_construtores
        ) VALUES
            ('T_STAY', 4, 2024, 'mazda_amador', NULL, 1, 200, 8, 1),
            ('T_DROP', 4, 2024, 'mazda_amador', NULL, 10, 0, 0, 0);
        ",
    )
    .expect("seed live-season world");

    let payload = build_global_team_history(&conn, "mazda", 2024, 4).expect("payload");
    assert_eq!(
        payload.max_year, 2025,
        "timeline extends into the active season"
    );
    assert_eq!(payload.current_year, 2025);

    let rookie = payload
        .bands
        .iter()
        .find(|band| band.key == "mazda_rookie")
        .expect("rookie band");
    let drop_rookie = rookie
        .rows
        .iter()
        .find(|row| row.team_id == "T_DROP")
        .expect("T_DROP now shows in the rookie band");
    assert!(
        drop_rookie.points.iter().any(|point| point.year == 2025),
        "T_DROP has a 2025 point in its CURRENT (rookie) division"
    );

    let amador = payload
        .bands
        .iter()
        .find(|band| band.key == "mazda_amador")
        .expect("amador band");
    // T_DROP still has its 2024 amador history, but must NOT appear in the amador
    // 2025 standings — it left that division.
    if let Some(drop_amador) = amador.rows.iter().find(|row| row.team_id == "T_DROP") {
        assert!(
            !drop_amador.points.iter().any(|point| point.year == 2025),
            "T_DROP must not have a 2025 point in the amador division"
        );
    }
    let stay = amador
        .rows
        .iter()
        .find(|row| row.team_id == "T_STAY")
        .expect("T_STAY in amador");
    assert!(
        stay.points.iter().any(|point| point.year == 2025),
        "T_STAY shows its live 2025 amador standing"
    );
    assert!(
        stay.is_reigning_champion,
        "the 2024 champion keeps the crown while 2025 is still running"
    );
    assert!(payload.in_progress, "2025 is running, not archived");
    assert_eq!(
        payload.last_completed_year, 2024,
        "the crown still belongs to the last archived season"
    );
}

// Regressao: o status realmente gravado por uma temporada em curso e
// 'EmAndamento' — 'Ativa' e so o alias legado que o enum ainda aceita. A consulta
// que so olhava 'Ativa' nunca achava a temporada viva num save de verdade, e o
// Atlas parava na ultima temporada arquivada.
#[test]
fn temporada_em_andamento_tambem_conta_como_temporada_viva() {
    let conn = setup_conn_with_live_season();
    conn.execute_batch(
        "
        INSERT INTO seasons (id, numero, ano, status) VALUES
            ('S4', 4, 2024, 'Finalizada'),
            ('S5', 5, 2025, 'EmAndamento');

        INSERT INTO teams (id, nome, nome_curto, categoria, classe, ativa) VALUES
            ('T_LIVE', 'Live Racing', 'LIV', 'mazda_rookie', NULL, 1);

        INSERT INTO team_season_archive (
            team_id, season_number, ano, categoria, classe, posicao_campeonato,
            pontos, vitorias, titulos_construtores
        ) VALUES
            ('T_LIVE', 4, 2024, 'mazda_rookie', NULL, 3, 90, 1, 0);
        ",
    )
    .expect("seed em-andamento world");

    let payload = build_global_team_history(&conn, "mazda", 2024, 4).expect("payload");
    assert_eq!(payload.current_year, 2025);
    assert!(payload.in_progress);
    assert_eq!(payload.last_completed_year, 2024);

    let rookie = payload
        .bands
        .iter()
        .find(|band| band.key == "mazda_rookie")
        .expect("rookie band");
    let live = rookie
        .rows
        .iter()
        .find(|row| row.team_id == "T_LIVE")
        .expect("T_LIVE na rookie");
    assert!(
        live.points.iter().any(|point| point.year == 2025),
        "a temporada em andamento entra como coluna viva"
    );
}

// Regressao: categoria multiclasse pontua pelos RESULTADOS de corrida, nao por
// `teams.stats_pontos` — que nem chega a ser alimentado la. O Atlas rankeava por
// stats_pontos e por isso mostrava uma ordem que nao existia na tela de
// construtores: a lider aparecia em quarto.
#[test]
fn multiclasse_ao_vivo_rankeia_pelos_resultados_e_nao_por_stats_pontos() {
    let conn = setup_conn_with_live_season();
    conn.execute_batch(
        "
        INSERT INTO seasons (id, numero, ano, status) VALUES
            ('S4', 4, 2024, 'Finalizada'),
            ('S5', 5, 2025, 'EmAndamento');

        -- stats_pontos diz o contrario do que aconteceu na pista: a lider real
        -- esta zerada ali, e quem nao pontuou tem o numero alto.
        INSERT INTO teams (id, nome, nome_curto, categoria, classe, ativa, stats_pontos, stats_vitorias)
        VALUES
            ('T_LIDER', 'Kestrel', 'KST', 'production_challenger', 'mazda', 1, 0, 0),
            ('T_FUNDO', 'Northgate', 'NGT', 'production_challenger', 'mazda', 1, 999, 9);

        INSERT INTO calendar (id, season_id, temporada_id, categoria) VALUES
            ('R1', 'S5', 'S5', 'production_challenger');

        INSERT INTO race_results (race_id, equipe_id, posicao_final, dnf, pontos) VALUES
            ('R1', 'T_LIDER', 1, 0, 120.0),
            ('R1', 'T_FUNDO', 5, 0, 30.0);
        ",
    )
    .expect("seed multiclasse ao vivo");

    let payload = build_global_team_history(&conn, "mazda", 2024, 4).expect("payload");
    let production = payload
        .bands
        .iter()
        .find(|band| band.key == "production_mazda")
        .expect("faixa production");
    let position_of = |team_id: &str| {
        production
            .rows
            .iter()
            .find(|row| row.team_id == team_id)
            .and_then(|row| row.points.iter().find(|point| point.year == 2025))
            .map(|point| point.position)
    };

    assert_eq!(
        position_of("T_LIDER"),
        Some(1),
        "quem pontuou na pista lidera"
    );
    assert_eq!(
        position_of("T_FUNDO"),
        Some(2),
        "stats_pontos nao manda aqui"
    );
}

// Comeco de temporada: ninguem pontuou ainda. A ordem que vale e a do ano passado —
// um empate de zeros resolvido pelo alfabeto se leria como classificacao real.
#[test]
fn temporada_recem_comecada_herda_a_ordem_do_ano_passado() {
    let conn = setup_conn_with_live_season();
    conn.execute_batch(
        "
        INSERT INTO seasons (id, numero, ano, status) VALUES
            ('S4', 4, 2024, 'Finalizada'),
            ('S5', 5, 2025, 'EmAndamento');

        -- Ordem alfabetica poria ZEBRA na frente; a de 2024 poe ALFA em segundo.
        INSERT INTO teams (id, nome, nome_curto, categoria, classe, ativa) VALUES
            ('T_ALFA', 'Alfa Racing', 'ALF', 'mazda_rookie', NULL, 1),
            ('T_ZEBRA', 'Zebra Racing', 'ZBR', 'mazda_rookie', NULL, 1);

        INSERT INTO team_season_archive (
            team_id, season_number, ano, categoria, classe, posicao_campeonato,
            pontos, vitorias, titulos_construtores
        ) VALUES
            ('T_ZEBRA', 4, 2024, 'mazda_rookie', NULL, 1, 150, 6, 1),
            ('T_ALFA', 4, 2024, 'mazda_rookie', NULL, 2, 120, 3, 0);
        ",
    )
    .expect("seed temporada recem comecada");

    let payload = build_global_team_history(&conn, "mazda", 2024, 4).expect("payload");
    let rookie = payload
        .bands
        .iter()
        .find(|band| band.key == "mazda_rookie")
        .expect("faixa rookie");
    let position_of = |team_id: &str| {
        rookie
            .rows
            .iter()
            .find(|row| row.team_id == team_id)
            .and_then(|row| row.points.iter().find(|point| point.year == 2025))
            .map(|point| point.position)
    };

    assert_eq!(position_of("T_ZEBRA"), Some(1));
    assert_eq!(position_of("T_ALFA"), Some(2));
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
    // O SQL do Atlas mora nos submodulos; a fachada so orquestra. Varre TODAS as
    // fontes do modulo para o guard continuar honesto apos a quebra em submodulos.
    let source = concat!(
        include_str!("../../global_team_history.rs"),
        include_str!("../familias.rs"),
        include_str!("../dados.rs"),
        include_str!("../bandas.rs"),
    );
    let legacy_table = concat!("special", "_team", "_entries");

    assert!(
        !source.contains(legacy_table),
        "Atlas global history must not read legacy special entries"
    );
}

#[test]
fn titles_counted_across_all_family_bands_independent_of_window() {
    let conn = setup_conn();
    // T_SUNDAY: 1× mazda_rookie (2020), 1× production_mazda (2023)
    seed_team_history(&conn);

    // Window A: 2020–2023 — includes the rookie title year
    let payload_a = build_global_team_history(&conn, "mazda", 2020, 4).expect("payload A");
    // T_SUNDAY appears in production_mazda band (its highest level in window)
    let prod_a = payload_a
        .bands
        .iter()
        .find(|b| b.key == "production_mazda")
        .expect("prod band A");
    let sunday_a = prod_a
        .rows
        .iter()
        .find(|r| r.team_id == "T_SUNDAY")
        .expect("sunday A");
    // Should see titles from BOTH bands, not just the displayed one
    assert!(
        sunday_a
            .titles
            .iter()
            .any(|t| t.band_key == "mazda_rookie" && t.count == 1),
        "expected mazda_rookie x1"
    );
    assert!(
        sunday_a
            .titles
            .iter()
            .any(|t| t.band_key == "production_mazda" && t.count == 1),
        "expected production_mazda x1"
    );

    // Window B: 2022–2025 — does NOT include the rookie title year
    let payload_b = build_global_team_history(&conn, "mazda", 2022, 4).expect("payload B");
    let prod_b = payload_b
        .bands
        .iter()
        .find(|b| b.key == "production_mazda")
        .expect("prod band B");
    let sunday_b = prod_b
        .rows
        .iter()
        .find(|r| r.team_id == "T_SUNDAY")
        .expect("sunday B");
    // Counts must be identical regardless of window
    assert_eq!(sunday_a.titles.len(), sunday_b.titles.len());
    for tc in &sunday_b.titles {
        let matching = sunday_a.titles.iter().find(|t| t.band_key == tc.band_key);
        assert!(
            matching.is_some_and(|t| t.count == tc.count),
            "title count for {} changed between windows",
            tc.band_key
        );
    }
}

#[test]
fn titles_empty_for_team_with_no_championships() {
    let conn = setup_conn();
    seed_team_history(&conn);
    // T_SUNDAY row 2021: mazda_amador, position 2 → no title that year
    // T_DUAL has titles in mazda_amador, but let's check a band where nobody
    // has won anything: add a team that only ever finished 2nd.
    conn.execute_batch(
        "
        INSERT INTO teams (id, nome, nome_curto, categoria, classe, cor_primaria, cor_secundaria)
        VALUES ('T_NEVER', 'Never Won', 'NVR', 'mazda_rookie', NULL, '#aaa', '#111');
        INSERT INTO team_season_archive (
            team_id, season_number, ano, categoria, classe, posicao_campeonato,
            pontos, vitorias, podios, poles, corridas, titulos_construtores
        ) VALUES ('T_NEVER', 10, 2020, 'mazda_rookie', NULL, 2, 80, 0, 2, 0, 8, 0);
        ",
    )
    .expect("seed never-won");

    let payload = build_global_team_history(&conn, "mazda", 2020, 4).expect("payload");
    let rookie = payload
        .bands
        .iter()
        .find(|b| b.key == "mazda_rookie")
        .expect("rookie band");
    let never_row = rookie
        .rows
        .iter()
        .find(|r| r.team_id == "T_NEVER")
        .expect("never-won row");
    assert!(never_row.titles.is_empty(), "expected empty titles vec");
    assert!(!never_row.is_reigning_champion);
}

#[test]
fn is_reigning_champion_set_only_for_champion_in_window_end_year() {
    let conn = setup_conn();
    seed_team_history(&conn);

    // Window end = 2023: T_SUNDAY won production_mazda in 2023 (season_number 4)
    let payload = build_global_team_history(&conn, "mazda", 2020, 4).expect("payload");
    assert_eq!(payload.window_end, 2023);

    let prod = payload
        .bands
        .iter()
        .find(|b| b.key == "production_mazda")
        .expect("prod band");
    let sunday = prod
        .rows
        .iter()
        .find(|r| r.team_id == "T_SUNDAY")
        .expect("sunday");
    assert!(
        sunday.is_reigning_champion,
        "T_SUNDAY won production_mazda in 2023"
    );

    // Shift window so 2023 is no longer the end year
    let payload2 = build_global_team_history(&conn, "mazda", 2021, 4).expect("payload2");
    assert_eq!(payload2.window_end, 2024);
    let prod2 = payload2
        .bands
        .iter()
        .find(|b| b.key == "production_mazda")
        .expect("prod band2");
    let sunday2 = prod2
        .rows
        .iter()
        .find(|r| r.team_id == "T_SUNDAY")
        .expect("sunday2");
    // 2024 has no production_mazda data for T_SUNDAY → last.year = 2023 ≠ window_end 2024
    assert!(
        !sunday2.is_reigning_champion,
        "no production_mazda data in 2024 for T_SUNDAY"
    );
}

#[test]
fn titles_ordered_lowest_band_first() {
    let conn = setup_conn();
    seed_team_history(&conn);
    // T_SUNDAY: rookiex1, productionx1 — rookie is band index 2 in MAZDA_BANDS,
    // production_mazda is band index 0 → production comes first
    let payload = build_global_team_history(&conn, "mazda", 2020, 4).expect("payload");
    let prod = payload
        .bands
        .iter()
        .find(|b| b.key == "production_mazda")
        .expect("prod band");
    let sunday = prod
        .rows
        .iter()
        .find(|r| r.team_id == "T_SUNDAY")
        .expect("sunday");
    // Titles should be in band-index order (production_mazda=0, mazda_amador=1, mazda_rookie=2)
    let keys: Vec<&str> = sunday.titles.iter().map(|t| t.band_key.as_str()).collect();
    for window in keys.windows(2) {
        let idx_a = MAZDA_BANDS
            .iter()
            .position(|b| b.key == window[0])
            .unwrap_or(99);
        let idx_b = MAZDA_BANDS
            .iter()
            .position(|b| b.key == window[1])
            .unwrap_or(99);
        assert!(idx_a <= idx_b, "titles not in band-index order: {:?}", keys);
    }
}

#[test]
fn current_year_falls_back_to_max_year_when_no_active_season() {
    let conn = setup_conn();
    seed_team_history(&conn);
    // No seasons table → active_year = None → current_year = max_year = DEFAULT_MAX_YEAR
    let payload = build_global_team_history(&conn, "mazda", 2020, 4).expect("payload");
    assert_eq!(payload.current_year, DEFAULT_MAX_YEAR);
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

// ---------------------------------------------------------------------------
// Salao dos campeoes
// ---------------------------------------------------------------------------

fn setup_champions_conn() -> Connection {
    let conn = setup_conn();
    conn.execute_batch(
        "
        CREATE TABLE drivers (
            id TEXT PRIMARY KEY,
            nome TEXT NOT NULL
        );
        CREATE TABLE driver_season_archive (
            piloto_id TEXT NOT NULL,
            season_number INTEGER NOT NULL,
            ano INTEGER NOT NULL,
            nome TEXT NOT NULL,
            categoria TEXT NOT NULL DEFAULT '',
            posicao_campeonato INTEGER,
            -- A classe do piloto so existe dentro do snapshot: a tabela nao tem coluna
            -- propria para ela, e e dai que o salao dos campeoes tem que le-la.
            snapshot_json TEXT
        );
        ",
    )
    .expect("schema de campeoes");
    conn
}

fn seed_champions(conn: &Connection) {
    conn.execute_batch(
        "
        INSERT INTO teams (id, nome, nome_curto, categoria, classe, cor_primaria, cor_secundaria)
        VALUES
            ('T_KESTREL', 'Kestrel', 'KES', 'production_challenger', 'mazda', '#ff4d4d', '#3d0d0d'),
            ('T_APERTURE', 'Aperture', 'APE', 'production_challenger', 'mazda', '#38bdf8', '#08304a');

        INSERT INTO drivers (id, nome) VALUES
            ('D_MOREAU', 'Lucien Moreau'),
            ('D_OKAFOR', 'Rui Okafor');

        -- Fischer se aposentou: nao esta mais em `drivers`, so no arquivo da temporada.
        INSERT INTO driver_season_archive (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, snapshot_json)
        VALUES
            -- Campea da classe toyota de 2025, arquivada ANTES do campeao da mazda: e ela
            -- que um LIMIT 1 sem filtro de classe pegaria.
            ('D_SATO', 6, 2025, 'Aiko Sato', 'production_challenger', 1, '{\"classe\":\"toyota\"}'),
            ('D_MOREAU', 6, 2025, 'Lucien Moreau', 'production_challenger', 1, '{\"classe\":\"mazda\"}'),
            ('D_FISCHER', 5, 2024, 'Tomas Fischer', 'production_challenger', 4, '{\"classe\":\"mazda\"}'),
            ('D_ORTEGA', 5, 2024, 'Ines Ortega', 'production_challenger', 1, '{\"classe\":\"mazda\"}');

        INSERT INTO team_season_archive (
            team_id, season_number, ano, categoria, classe, posicao_campeonato,
            pontos, vitorias, podios, poles, corridas, titulos_construtores,
            piloto_1_id, piloto_2_id
        ) VALUES
            ('T_KESTREL', 6, 2025, 'production_challenger', 'mazda', 1, 210, 9, 14, 5, 18, 1, 'D_MOREAU', 'D_OKAFOR'),
            ('T_APERTURE', 5, 2024, 'production_challenger', 'mazda', 1, 190, 6, 12, 4, 18, 1, 'D_FISCHER', NULL),
            ('T_KESTREL', 4, 2023, 'production_challenger', 'mazda', 1, 205, 7, 13, 6, 18, 1, 'D_MOREAU', NULL),
            -- Vice-campea: nao pode aparecer no salao.
            ('T_APERTURE', 4, 2023, 'production_challenger', 'mazda', 2, 180, 5, 10, 3, 18, 0, 'D_FISCHER', NULL),
            -- Mesma categoria, OUTRA classe: pertence a outra faixa.
            ('T_KESTREL', 4, 2023, 'production_challenger', 'toyota', 1, 150, 4, 9, 2, 18, 1, 'D_OKAFOR', NULL);
        ",
    )
    .expect("seed de campeoes");
}

#[test]
fn campeoes_listam_so_os_titulos_da_propria_faixa() {
    let conn = setup_champions_conn();
    seed_champions(&conn);

    let payload = build_band_champions(&conn, "production_mazda").expect("payload");

    // A classe separa as faixas: o titulo de 2023 da classe toyota nao entra aqui,
    // ainda que a categoria seja a mesma.
    let years: Vec<i32> = payload.seasons.iter().map(|season| season.year).collect();
    assert_eq!(years, vec![2025, 2024, 2023]);
    assert_eq!(payload.band_label, "Mazda Production");
}

#[test]
fn campeoes_trazem_a_dupla_do_ano_e_marcam_o_campeao_de_pilotos() {
    let conn = setup_champions_conn();
    seed_champions(&conn);

    let payload = build_band_champions(&conn, "production_mazda").expect("payload");
    let dois_mil_e_vinte_cinco = &payload.seasons[0];

    // A dupla inteira aparece — e o titulo de pilotos e de UM deles.
    let nomes: Vec<&str> = dois_mil_e_vinte_cinco
        .drivers
        .iter()
        .map(|driver| driver.nome.as_str())
        .collect();
    assert_eq!(nomes, vec!["Lucien Moreau", "Rui Okafor"]);
    assert!(dois_mil_e_vinte_cinco.drivers[0].is_season_champion);
    assert!(!dois_mil_e_vinte_cinco.drivers[1].is_season_champion);

    // 2024: a equipe foi campea de construtores, mas o campeao de pilotos correu por
    // outra. Ninguem da dupla leva a marca, e o nome vem do arquivo porque o piloto
    // ja nao existe mais em `drivers`.
    let dois_mil_e_vinte_quatro = &payload.seasons[1];
    assert_eq!(dois_mil_e_vinte_quatro.drivers.len(), 1);
    assert_eq!(dois_mil_e_vinte_quatro.drivers[0].nome, "Tomas Fischer");
    assert!(!dois_mil_e_vinte_quatro.drivers[0].is_season_champion);
}

/// Regressao: `production_challenger` e `endurance` tem tres classes na mesma categoria,
/// entao ha tres campeoes de pilotos por temporada. Sem filtrar pela classe, o `LIMIT 1`
/// escolhia um deles a esmo — e a faixa inteira aparecia sem campeao marcado, mesmo com o
/// campeao sentado na equipe campea de construtores.
#[test]
fn campeao_de_pilotos_de_classe_irma_nao_rouba_a_marca_da_faixa() {
    let conn = setup_champions_conn();
    seed_champions(&conn);

    let mazda = build_band_champions(&conn, "production_mazda").expect("payload");
    let campeao_2025 = mazda.seasons[0]
        .drivers
        .iter()
        .find(|driver| driver.is_season_champion)
        .expect("2025 tem campeao de pilotos na mazda");
    assert_eq!(campeao_2025.nome, "Lucien Moreau");

    // Aiko Sato foi campea da toyota no mesmo ano e nao pode contaminar a faixa da mazda.
    assert!(!mazda.seasons[0]
        .drivers
        .iter()
        .any(|driver| driver.nome == "Aiko Sato"));
}

#[test]
fn dinastias_ordenam_por_titulos_e_desempatam_pelo_mais_recente() {
    let conn = setup_champions_conn();
    seed_champions(&conn);

    let payload = build_band_champions(&conn, "production_mazda").expect("payload");

    assert_eq!(payload.dynasties.len(), 2);
    assert_eq!(payload.dynasties[0].team_id, "T_KESTREL");
    assert_eq!(payload.dynasties[0].titles, 2);
    assert_eq!(payload.dynasties[0].last_year, 2025);
    assert_eq!(payload.dynasties[1].titles, 1);
}

#[test]
fn faixa_desconhecida_e_erro_e_faixa_sem_titulo_e_payload_vazio() {
    let conn = setup_champions_conn();
    seed_champions(&conn);

    assert!(build_band_champions(&conn, "faixa_que_nao_existe").is_err());

    let vazio = build_band_champions(&conn, "mazda_rookie").expect("payload");
    assert!(vazio.seasons.is_empty());
    assert!(vazio.dynasties.is_empty());
}
