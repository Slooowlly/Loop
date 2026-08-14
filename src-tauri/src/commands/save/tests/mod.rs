use super::{
    backup_season_internal, list_backups_in_career_dir, parse_backup_filename,
    restore_backup_internal, substituir_preservando_anterior, RACE_SCREENS_DIR,
    SABOTAR_ITEM_DA_TROCA_EM_LOTE, SABOTAR_TROCA_DO_RESTORE, SIDECAR_FILES,
};
use crate::commands::career::{
    advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
    finalize_preseason_in_base_dir, get_player_proposals_in_base_dir,
    respond_to_proposal_in_base_dir, CreateCareerInput,
};
use crate::commands::race::simulate_race_weekend_in_base_dir;
use crate::config::app_config::AppConfig;
use crate::constants::historical_timeline::PLAYABLE_START_YEAR;
use crate::db::connection::Database;
use crate::db::migrations::{BASELINE_VERSION, CURRENT_VERSION};
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::market_proposals as market_proposal_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::market::proposals::{MarketProposal, ProposalStatus};
use crate::models::enums::TeamRole;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ano civil da temporada `numero` de uma carreira nova.
///
/// A carreira regular nasce em [`PLAYABLE_START_YEAR`] (`career/lifecycle.rs`), o mesmo início
/// de mundo do draft histórico. Estes testes cravavam 2024 e 2025 no literal, então passaram a
/// falhar quando o início do mundo se moveu — sem nada ter regredido no backup ou no restore,
/// que é o que eles medem. A conta abaixo os prende ao mesmo lugar de onde a produção lê.
fn ano_da_temporada(numero: i32) -> i32 {
    PLAYABLE_START_YEAR + numero - 1
}

fn unique_test_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("iracer_save_cmd_{label}_{nanos}"))
}

fn create_test_career_dir(label: &str) -> PathBuf {
    let base_dir = unique_test_dir(label);
    fs::create_dir_all(&base_dir).expect("base dir");

    let input = CreateCareerInput {
        player_name: "Joao Silva".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };

    create_career_in_base_dir(&base_dir, input).expect("career should be created");
    base_dir
}

fn career_paths(base_dir: &Path) -> (AppConfig, PathBuf, PathBuf, PathBuf) {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join("career_001");
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");
    (config, career_dir, db_path, meta_path)
}

fn mark_all_races_completed(db_path: &Path) {
    let db = Database::open_existing(db_path).expect("db");
    db.conn
        .execute("UPDATE calendar SET status = 'Concluida'", [])
        .expect("mark all races completed");
    db.conn
        .execute(
            "UPDATE seasons SET fase = 'PosEspecial' WHERE status = 'EmAndamento'",
            [],
        )
        .expect("mark season as post-special");
}

fn force_complete_preseason_plan(save_dir: &Path) {
    let mut plan = crate::market::preseason::load_preseason_plan(save_dir)
        .expect("load preseason plan")
        .expect("preseason plan");
    plan.state.is_complete = true;
    plan.state.current_week = plan.state.total_weeks + 1;
    plan.state.phase = crate::market::preseason::PreSeasonPhase::Complete;
    plan.state.player_has_pending_proposals = false;
    crate::market::preseason::save_preseason_plan(save_dir, &plan)
        .expect("save completed preseason plan");
}

fn seed_player_regular_proposal(
    conn: &rusqlite::Connection,
    season_id: &str,
    proposal: &MarketProposal,
) {
    market_proposal_queries::insert_player_proposal(conn, season_id, proposal)
        .expect("insert player proposal");
}

#[test]
fn backup_restore_round_trip_restores_sidecar_snapshot() {
    let base_dir = create_test_career_dir("restore_sidecars");
    let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
    let race_results_path = career_dir.join("race_results.json");
    let resume_context_path = career_dir.join("resume_context.json");
    let briefing_path = career_dir.join("briefing_phrase_history.json");
    let preseason_path = career_dir.join("preseason_plan.json");

    fs::write(&race_results_path, "{\"version\":1}").expect("seed race results");
    fs::write(&resume_context_path, "{\"active_view\":\"preseason\"}")
        .expect("seed resume context");
    fs::write(&briefing_path, "{\"season_number\":1,\"entries\":[]}").expect("seed briefing");
    fs::write(&preseason_path, "{\"state\":{\"current_week\":1}}").expect("seed preseason");

    let original_meta = fs::read_to_string(&meta_path).expect("read original meta");
    backup_season_internal(&db_path, &career_dir, 1, &meta_path).expect("backup should work");

    fs::write(&race_results_path, "{\"version\":2}").expect("mutate race results");
    fs::remove_file(&resume_context_path).expect("remove resume context");
    fs::write(
        &briefing_path,
        "{\"season_number\":99,\"entries\":[{\"id\":\"changed\"}]}",
    )
    .expect("mutate briefing");
    fs::remove_file(&preseason_path).expect("remove preseason");
    fs::write(
        &meta_path,
        original_meta.replace("\"current_season\": 1", "\"current_season\": 99"),
    )
    .expect("mutate meta");

    restore_backup_internal(&db_path, &career_dir, 1).expect("restore should work");

    assert_eq!(
        fs::read_to_string(&race_results_path).expect("restored race results"),
        "{\"version\":1}"
    );
    assert_eq!(
        fs::read_to_string(&resume_context_path).expect("restored resume context"),
        "{\"active_view\":\"preseason\"}"
    );
    assert_eq!(
        fs::read_to_string(&briefing_path).expect("restored briefing"),
        "{\"season_number\":1,\"entries\":[]}"
    );
    assert_eq!(
        fs::read_to_string(&preseason_path).expect("restored preseason"),
        "{\"state\":{\"current_week\":1}}"
    );

    let restored_meta = fs::read_to_string(&meta_path).expect("restored meta");
    assert!(restored_meta.contains("\"current_season\": 1"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn full_career_flow_backup_restore_round_trip() {
    let base_dir = create_test_career_dir("full_flow_backup_restore");
    let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);

    let db = Database::open_existing(&db_path).expect("db");
    let active_season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let next_race = calendar_queries::get_next_race(&db.conn, &active_season.id, "mazda_rookie")
        .expect("next race query")
        .expect("pending race");
    drop(db);

    let race_result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race.id)
        .expect("simulate opening race");
    assert!(
        !race_result.player_race.race_results.is_empty(),
        "player race should persist race results",
    );

    let race_results_path = career_dir.join("race_results.json");
    assert!(
        race_results_path.exists(),
        "simulating a real race should create race_results.json",
    );

    mark_all_races_completed(&db_path);

    let season_result =
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    assert_eq!(season_result.new_year, ano_da_temporada(2));
    assert!(season_result.preseason_initialized);
    assert!(
        season_result.promotion_result.errors.is_empty(),
        "promotion/relegation should finish without errors: {:?}",
        season_result.promotion_result.errors
    );

    let advanced_week = advance_market_week_in_base_dir(&base_dir, "career_001", None)
        .expect("advance market week");
    assert_eq!(advanced_week.week_number, 1);

    let preseason_path = career_dir.join("preseason_plan.json");
    assert!(
        preseason_path.exists(),
        "preseason plan should exist after season advance"
    );

    backup_season_internal(&db_path, &career_dir, 2, &meta_path)
        .expect("season 2 backup should work");

    let expected_meta = fs::read_to_string(&meta_path).expect("read backed-up meta");
    let expected_race_results =
        fs::read_to_string(&race_results_path).expect("read backed-up race results");
    let expected_preseason = fs::read_to_string(&preseason_path).expect("read backed-up preseason");

    let mutated_meta = expected_meta
        .replace("\"current_season\": 2", "\"current_season\": 99")
        .replace(
            &format!("\"current_year\": {}", ano_da_temporada(2)),
            "\"current_year\": 2099",
        );
    fs::write(&meta_path, mutated_meta).expect("mutate meta");
    fs::write(&race_results_path, "{\"version\":999}").expect("mutate race results");
    fs::write(&preseason_path, "{\"state\":{\"current_week\":99}}").expect("mutate preseason");

    let db = Database::open_existing(&db_path).expect("db before restore");
    db.conn
        .execute(
            "UPDATE seasons SET numero = 99, ano = 2099 WHERE status = 'EmAndamento'",
            [],
        )
        .expect("mutate active season");
    // A troca do banco vivo e por rename, e no Windows renomear arquivo com conexao
    // SQLite aberta falha (os error 32). Em producao nenhum comando segura conexao
    // durante o restore; aqui a conexao de mutacao precisa sair de cena antes.
    drop(db);

    restore_backup_internal(&db_path, &career_dir, 2).expect("restore season 2 backup");

    let restored_db = Database::open_existing(&db_path).expect("restored db");
    let restored_active_season = season_queries::get_active_season(&restored_db.conn)
        .expect("restored season query")
        .expect("restored active season");
    assert_eq!(restored_active_season.numero, 2);
    assert_eq!(restored_active_season.ano, ano_da_temporada(2));

    assert_eq!(
        fs::read_to_string(&meta_path).expect("restored meta"),
        expected_meta
    );
    assert_eq!(
        fs::read_to_string(&race_results_path).expect("restored race results"),
        expected_race_results
    );
    assert_eq!(
        fs::read_to_string(&preseason_path).expect("restored preseason"),
        expected_preseason
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn full_preseason_player_proposal_flow_reaches_season_start() {
    let base_dir = create_test_career_dir("full_preseason_player_proposal_flow");
    let (config, career_dir, db_path, _meta_path) = career_paths(&base_dir);

    mark_all_races_completed(&db_path);
    let season_result =
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
    assert!(season_result.preseason_initialized);

    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    let active_regular =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("active regular contract query");
    let current_regular_team_id = active_regular
        .as_ref()
        .map(|contract| contract.equipe_id.clone());
    let player_regular_category = active_regular
        .map(|contract| contract.categoria)
        .or_else(|| player.categoria_atual.clone())
        .unwrap_or_else(|| "mazda_rookie".to_string());
    let target_team = team_queries::get_teams_by_category(&db.conn, &player_regular_category)
        .expect("teams by category")
        .into_iter()
        .find(|team| current_regular_team_id.as_ref() != Some(&team.id))
        .unwrap_or_else(|| {
            team_queries::get_teams_by_category(&db.conn, &player_regular_category)
                .expect("fallback teams by category")
                .into_iter()
                .next()
                .expect("at least one team in player regular category")
        });
    let proposal = MarketProposal {
        id: format!("MP-{}-{}", target_team.id, player.id),
        equipe_id: target_team.id.clone(),
        equipe_nome: target_team.nome.clone(),
        piloto_id: player.id.clone(),
        piloto_nome: player.nome.clone(),
        categoria: target_team.categoria.clone(),
        papel: TeamRole::Numero1,
        salario_oferecido: 125_000.0,
        duracao_anos: 2,
        status: ProposalStatus::Pendente,
        motivo_recusa: None,
    };
    seed_player_regular_proposal(&db.conn, &season.id, &proposal);
    drop(db);

    let proposals =
        get_player_proposals_in_base_dir(&base_dir, "career_001").expect("player proposals");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].proposal_id, proposal.id);

    let response = respond_to_proposal_in_base_dir(&base_dir, "career_001", &proposal.id, true)
        .expect("accept proposal");
    assert!(response.success);
    assert_eq!(response.action, "accepted");
    assert_eq!(response.remaining_proposals, 0);
    assert_eq!(
        response.new_team_name.as_deref(),
        Some(target_team.nome.as_str())
    );

    force_complete_preseason_plan(&career_dir);
    finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

    let finalized_db = Database::open_existing(&db_path).expect("db after finalize");
    let finalized_contract =
        contract_queries::get_active_regular_contract_for_pilot(&finalized_db.conn, &player.id)
            .expect("active contract after finalize")
            .expect("player should keep active regular contract");
    let finalized_season = season_queries::get_active_season(&finalized_db.conn)
        .expect("active season after finalize")
        .expect("active season");

    assert_eq!(finalized_contract.equipe_id, target_team.id);
    assert_eq!(finalized_season.numero, 2);
    assert_eq!(finalized_season.ano, ano_da_temporada(2));
    assert!(
        !config
            .saves_dir()
            .join("career_001")
            .join("preseason_plan.json")
            .exists(),
        "finalizar a pre-temporada deve remover o plano salvo"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn backup_season_internal_does_not_leave_backup_when_meta_update_fails() {
    let base_dir = create_test_career_dir("meta_fail");
    let (_config, career_dir, db_path, _meta_path) = career_paths(&base_dir);
    let invalid_meta_path = career_dir.join("missing").join("meta.json");
    let backups_dir = career_dir.join("backups");
    let backup_file = backups_dir.join("temporada_001.db");
    let sidecars_dir = backups_dir.join("temporada_001.files");

    let err = backup_season_internal(&db_path, &career_dir, 1, &invalid_meta_path)
        .expect_err("invalid meta path should fail");

    assert!(err.contains("meta.json"));
    assert!(
        !backup_file.exists(),
        "failed backup should not leave final db"
    );
    assert!(
        !sidecars_dir.exists(),
        "failed backup should not leave final sidecar snapshot"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn list_backups_in_career_dir_propagates_filesystem_errors() {
    let base_dir = unique_test_dir("list_backups_error");
    fs::create_dir_all(&base_dir).expect("base dir");
    let fake_career_dir = base_dir.join("career_001");
    let backups_file = fake_career_dir.join("backups");
    fs::create_dir_all(&fake_career_dir).expect("career dir");
    fs::write(&backups_file, "not a directory").expect("seed backups file");

    let err = list_backups_in_career_dir(&fake_career_dir)
        .expect_err("read_dir failure should propagate");

    assert!(err.contains("backups"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn restore_legacy_backup_rebuilds_meta_and_clears_stale_sidecars() {
    let base_dir = create_test_career_dir("legacy_restore");
    let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
    let backups_dir = career_dir.join("backups");
    fs::create_dir_all(&backups_dir).expect("backups dir");

    let legacy_backup = backups_dir.join("temporada_001.db");
    let db = Database::open_existing(&db_path).expect("db");
    db.backup(&legacy_backup).expect("legacy db-only backup");
    drop(db);

    let race_results_path = career_dir.join("race_results.json");
    let resume_context_path = career_dir.join("resume_context.json");
    let briefing_path = career_dir.join("briefing_phrase_history.json");
    let preseason_path = career_dir.join("preseason_plan.json");
    fs::write(&race_results_path, "{\"version\":2}").expect("seed race results");
    fs::write(&resume_context_path, "{\"active_view\":\"market\"}").expect("seed resume");
    fs::write(&briefing_path, "{\"season_number\":99,\"entries\":[]}").expect("seed briefing");
    fs::write(&preseason_path, "{\"state\":{\"current_week\":7}}").expect("seed preseason");

    let db = Database::open_existing(&db_path).expect("db");
    db.conn
        .execute(
            "UPDATE seasons SET numero = 99, ano = 2099 WHERE status = 'EmAndamento'",
            [],
        )
        .expect("mutate season");
    // Ver o `drop` equivalente em `full_career_flow_backup_restore_round_trip`: a troca
    // por rename nao acontece com conexao aberta no banco vivo.
    drop(db);
    fs::write(
        &meta_path,
        fs::read_to_string(&meta_path)
            .expect("read meta")
            .replace("\"current_season\": 1", "\"current_season\": 99")
            .replace(
                &format!("\"current_year\": {}", ano_da_temporada(1)),
                "\"current_year\": 2099",
            ),
    )
    .expect("mutate meta");

    restore_backup_internal(&db_path, &career_dir, 1).expect("legacy restore should work");

    let restored_db = Database::open_existing(&db_path).expect("restored db");
    let active_season = season_queries::get_active_season(&restored_db.conn)
        .expect("season query")
        .expect("active season");
    assert_eq!(active_season.numero, 1);
    assert_eq!(active_season.ano, ano_da_temporada(1));

    let restored_meta = fs::read_to_string(&meta_path).expect("restored meta");
    assert!(restored_meta.contains("\"current_season\": 1"));
    assert!(restored_meta.contains(&format!("\"current_year\": {}", ano_da_temporada(1))));
    assert!(!race_results_path.exists());
    assert!(!resume_context_path.exists());
    assert!(!briefing_path.exists());
    assert!(!preseason_path.exists());

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn parse_backup_filename_accepts_current_and_legacy_names() {
    assert_eq!(parse_backup_filename("temporada_007.db"), Some(7));
    assert_eq!(parse_backup_filename("season_042.db"), Some(42));
}

/// Falha de substituicao com o arquivo bom no lugar. A origem staged nao existe, entao
/// o rename final falha depois do destino ja ter saido para o `.old`: o rollback tem
/// que devolver o original intacto.
#[test]
fn substituir_preservando_anterior_mantem_o_arquivo_quando_a_troca_falha() {
    let base_dir = unique_test_dir("swap_arquivo_falha");
    fs::create_dir_all(&base_dir).expect("base dir");

    let destino = base_dir.join("temporada_001.db");
    fs::write(&destino, "backup bom").expect("seed destino");
    let staged = base_dir.join("temporada_001.db.tmp");

    let err = substituir_preservando_anterior(&staged, &destino, "backup")
        .expect_err("rename sem origem deve falhar");

    assert!(err.contains("backup"), "erro deve nomear o rotulo: {err}");
    assert_eq!(
        fs::read_to_string(&destino).expect("o backup anterior tem que sobreviver"),
        "backup bom"
    );
    assert!(
        !base_dir.join("temporada_001.db.old").exists(),
        "o rollback tem que devolver o .old ao caminho original"
    );

    let _ = fs::remove_dir_all(base_dir);
}

/// Mesma falha, agora no diretorio de sidecars: o snapshot anterior nao pode evaporar.
#[test]
fn substituir_preservando_anterior_mantem_o_diretorio_quando_a_troca_falha() {
    let base_dir = unique_test_dir("swap_dir_falha");
    fs::create_dir_all(&base_dir).expect("base dir");

    let destino = base_dir.join("temporada_001.files");
    fs::create_dir_all(&destino).expect("seed destino");
    fs::write(destino.join("meta.json"), "{\"current_season\":1}").expect("seed sidecar");
    let staged = base_dir.join("temporada_001.files.tmp");

    let err = substituir_preservando_anterior(&staged, &destino, "snapshot auxiliar")
        .expect_err("rename sem origem deve falhar");

    assert!(err.contains("snapshot auxiliar"), "erro sem rotulo: {err}");
    assert_eq!(
        fs::read_to_string(destino.join("meta.json")).expect("snapshot anterior preservado"),
        "{\"current_season\":1}"
    );
    assert!(!base_dir.join("temporada_001.files.old").exists());

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn substituir_preservando_anterior_troca_e_descarta_o_old() {
    let base_dir = unique_test_dir("swap_ok");
    fs::create_dir_all(&base_dir).expect("base dir");

    let destino = base_dir.join("temporada_001.db");
    let staged = base_dir.join("temporada_001.db.tmp");
    fs::write(&destino, "antigo").expect("seed destino");
    fs::write(&staged, "novo").expect("seed staged");

    substituir_preservando_anterior(&staged, &destino, "backup").expect("troca deve concluir");

    assert_eq!(fs::read_to_string(&destino).expect("destino"), "novo");
    assert!(!staged.exists(), "o staged sai do lugar pelo rename");
    assert!(
        !base_dir.join("temporada_001.db.old").exists(),
        "o .old so vive durante a troca"
    );

    let _ = fs::remove_dir_all(base_dir);
}

/// Um backup bom ja em disco e uma segunda tentativa que falha. O arquivo anterior
/// continua de pe e o meta.json nao pode carimbar um sucesso que nao houve.
#[test]
fn backup_falho_preserva_backup_anterior_e_nao_carimba_meta() {
    let base_dir = create_test_career_dir("backup_falho_preserva");
    let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
    let backup_file = career_dir.join("backups").join("temporada_001.db");
    let sidecars_dir = career_dir.join("backups").join("temporada_001.files");

    backup_season_internal(&db_path, &career_dir, 1, &meta_path).expect("primeiro backup");

    let bytes_do_bom = fs::read(&backup_file).expect("backup bom");
    let meta_do_bom = fs::read_to_string(&meta_path).expect("meta apos backup bom");
    assert!(
        meta_do_bom.contains("\"last_backup\""),
        "o backup que deu certo tem que carimbar last_backup"
    );

    let meta_invalido = career_dir.join("missing").join("meta.json");
    let err = backup_season_internal(&db_path, &career_dir, 1, &meta_invalido)
        .expect_err("meta inacessivel deve falhar o backup");
    assert!(err.contains("meta.json"), "erro inesperado: {err}");

    assert_eq!(
        fs::read(&backup_file).expect("backup anterior tem que continuar la"),
        bytes_do_bom,
        "a tentativa falha nao pode destruir o ultimo backup bom"
    );
    assert!(
        sidecars_dir.exists(),
        "o snapshot anterior tem que sobreviver"
    );
    assert_eq!(
        fs::read_to_string(&meta_path).expect("meta"),
        meta_do_bom,
        "backup que falhou nao pode mexer nos carimbos do meta.json"
    );

    let _ = fs::remove_dir_all(base_dir);
}

// ── Restore: inspecao do backup e troca com rollback ─────────────────────────────
//
// O caminho feliz (banco + sidecars restaurados) esta em
// `backup_restore_round_trip_restores_sidecar_snapshot`, la em cima. Os tres testes
// abaixo cobrem as saidas em que o banco vivo NAO pode ser tocado.

/// Marca o banco vivo, para distinguir "continua o mesmo" de "foi trocado pelo backup".
fn marca_o_banco_vivo(db_path: &Path, valor: &str) {
    let db = Database::open_existing(db_path).expect("abrir banco vivo");
    db.conn
        .execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('canario_do_teste', ?1)",
            [valor],
        )
        .expect("gravar canario");
}

fn canario_do_banco(db_path: &Path) -> Option<String> {
    let db = Database::open_existing(db_path).expect("abrir banco para ler o canario");
    db.conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'canario_do_teste'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
}

/// Backup sintetico com um carimbo de schema escolhido: e tudo que a inspecao precisa
/// ler para recusar antes de encostar no banco vivo.
fn escreve_backup_com_schema(career_dir: &Path, season_number: u32, versao: u32) {
    let backups_dir = career_dir.join("backups");
    fs::create_dir_all(&backups_dir).expect("dir de backups");
    let path = backups_dir.join(format!("temporada_{season_number:03}.db"));

    let conn = rusqlite::Connection::open(&path).expect("criar backup sintetico");
    conn.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
        .expect("meta do backup sintetico");
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)",
        [versao.to_string()],
    )
    .expect("carimbo do backup sintetico");
    drop(conn);
}

/// Falha no meio da substituicao. O `career.db` original tem que voltar inteiro, e sem
/// deixar `.old` nem `.novo` para tras.
#[test]
fn restore_falho_na_troca_preserva_o_banco_vivo() {
    let base_dir = create_test_career_dir("restore_troca_falha");
    let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);

    backup_season_internal(&db_path, &career_dir, 1, &meta_path).expect("backup de referencia");
    marca_o_banco_vivo(&db_path, "vivo depois do backup");

    SABOTAR_TROCA_DO_RESTORE.with(|interruptor| interruptor.set(true));
    let err = restore_backup_internal(&db_path, &career_dir, 1)
        .expect_err("a troca sabotada tem que falhar");
    SABOTAR_TROCA_DO_RESTORE.with(|interruptor| interruptor.set(false));

    assert!(
        err.contains("banco da carreira"),
        "o erro tem que nomear o que falhou: {err}"
    );
    assert!(db_path.exists(), "o banco vivo sumiu depois da troca falha");
    assert_eq!(
        canario_do_banco(&db_path).as_deref(),
        Some("vivo depois do backup"),
        "o banco vivo foi trocado (ou corrompido) por uma restauracao que falhou"
    );
    assert!(
        !career_dir.join("career.db.old").exists(),
        "o rollback tem que devolver o `.old` ao caminho original"
    );
    assert!(
        !career_dir.join("career.db.novo").exists(),
        "o staging tem que ser descartado quando a troca falha"
    );

    let _ = fs::remove_dir_all(base_dir);
}

/// Backup antigo demais: carimbado abaixo da baseline, nao tem caminho de atualizacao.
/// A recusa acontece antes de qualquer escrita — a ausencia do `career.db.bak` e a
/// prova de que o banco vivo nem chegou a ser aberto para a troca.
#[test]
fn restore_recusa_backup_antigo_demais_antes_de_tocar_no_banco_vivo() {
    let base_dir = create_test_career_dir("restore_backup_antigo");
    let (_config, career_dir, db_path, _meta_path) = career_paths(&base_dir);

    marca_o_banco_vivo(&db_path, "intocado");
    escreve_backup_com_schema(&career_dir, 7, BASELINE_VERSION - 1);

    let err = restore_backup_internal(&db_path, &career_dir, 7)
        .expect_err("backup anterior a baseline tem que ser recusado");

    assert!(
        err.contains("incompativel") && err.contains("baseline"),
        "a recusa tem que explicar o motivo: {err}"
    );
    assert!(
        !career_dir.join("career.db.bak").exists(),
        "a recusa tem que acontecer antes de encostar no banco vivo"
    );
    assert!(!career_dir.join("career.db.novo").exists());
    assert_eq!(canario_do_banco(&db_path).as_deref(), Some("intocado"));

    let _ = fs::remove_dir_all(base_dir);
}

/// Backup novo demais: veio de uma versao do Loop que grava um schema que este binario
/// nao conhece. As migracoes so sabem subir, entao nao ha como rebaixa-lo.
#[test]
fn restore_recusa_backup_mais_novo_que_o_binario() {
    let base_dir = create_test_career_dir("restore_backup_futuro");
    let (_config, career_dir, db_path, _meta_path) = career_paths(&base_dir);

    marca_o_banco_vivo(&db_path, "intocado");
    escreve_backup_com_schema(&career_dir, 9, CURRENT_VERSION + 1);

    let err = restore_backup_internal(&db_path, &career_dir, 9)
        .expect_err("backup mais novo que o binario tem que ser recusado");

    assert!(
        err.contains("incompativel") && err.contains("MAIS NOVA"),
        "a recusa tem que dizer que o backup veio de versao mais nova: {err}"
    );
    assert!(
        !career_dir.join("career.db.bak").exists(),
        "a recusa tem que acontecer antes de encostar no banco vivo"
    );
    assert!(!career_dir.join("career.db.novo").exists());
    assert_eq!(canario_do_banco(&db_path).as_deref(), Some("intocado"));

    let _ = fs::remove_dir_all(base_dir);
}

/// B77 — AS TELAS PÓS-CORRIDA SÃO ESTADO DA CARREIRA. Ficavam fora do snapshot, então o
/// restore devolvia o banco de uma temporada e deixava em pé as telas da linha temporal
/// abandonada. Como os IDs de corrida são reaproveitados pela linha nova, a tela de
/// `C002` seria reaberta como se fosse da corrida recém-disputada.
#[test]
fn restore_devolve_as_telas_pos_corrida_e_apaga_a_de_id_reaproveitado() {
    let base_dir = create_test_career_dir("telas_pos_corrida");
    let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);
    let telas = career_dir.join(RACE_SCREENS_DIR);
    fs::create_dir_all(&telas).expect("dir das telas");
    fs::write(telas.join("C001.json"), "{\"tela\":\"do backup\"}").expect("tela do backup");

    backup_season_internal(&db_path, &career_dir, 1, &meta_path).expect("backup");

    // A linha temporal que vai ser abandonada: a mesma corrida reescrita, e uma corrida
    // seguinte cujo ID o futuro restaurado vai reaproveitar.
    fs::write(telas.join("C001.json"), "{\"tela\":\"abandonada\"}").expect("tela mutada");
    fs::write(telas.join("C002.json"), "{\"tela\":\"abandonada\"}").expect("tela orfa");

    restore_backup_internal(&db_path, &career_dir, 1).expect("restore");

    assert_eq!(
        fs::read_to_string(telas.join("C001.json")).expect("tela restaurada"),
        "{\"tela\":\"do backup\"}",
        "a tela do snapshot devia voltar por cima da abandonada"
    );
    assert!(
        !telas.join("C002.json").exists(),
        "tela de ID reaproveitado nao pode sobreviver ao restore"
    );

    let _ = fs::remove_dir_all(base_dir);
}

/// B76 — A RESTAURAÇÃO DOS AUXILIARES É UMA OPERAÇÃO SÓ. Cada arquivo era copiado por
/// cima do vivo, um de cada vez: a falha do meio deixava metade da linha temporal antiga
/// e metade da nova, um estado que o jogo abre sem reclamar e ninguém desfaz depois.
/// Com a troca em lote, a falha devolve TODOS ao estado anterior.
#[test]
fn falha_no_meio_da_restauracao_preserva_a_linha_temporal_inteira() {
    let base_dir = create_test_career_dir("restore_em_lote");
    let (_config, career_dir, db_path, meta_path) = career_paths(&base_dir);

    for file_name in SIDECAR_FILES {
        fs::write(career_dir.join(file_name), "{\"origem\":\"do backup\"}").expect("auxiliar");
    }
    let telas = career_dir.join(RACE_SCREENS_DIR);
    fs::create_dir_all(&telas).expect("dir das telas");
    fs::write(telas.join("C001.json"), "{\"origem\":\"do backup\"}").expect("tela");

    backup_season_internal(&db_path, &career_dir, 1, &meta_path).expect("backup");

    for file_name in SIDECAR_FILES {
        fs::write(career_dir.join(file_name), "{\"origem\":\"viva\"}").expect("auxiliar mutado");
    }
    fs::write(telas.join("C001.json"), "{\"origem\":\"viva\"}").expect("tela mutada");
    let meta_vivo = fs::read_to_string(&meta_path).expect("meta vivo");

    // Morre no terceiro auxiliar, com os dois primeiros ja publicados.
    SABOTAR_ITEM_DA_TROCA_EM_LOTE.with(|interruptor| interruptor.set(Some(2)));
    let falha = restore_backup_internal(&db_path, &career_dir, 1);
    SABOTAR_ITEM_DA_TROCA_EM_LOTE.with(|interruptor| interruptor.set(None));

    assert!(falha.is_err(), "a falha injetada devia derrubar o restore");
    for file_name in SIDECAR_FILES {
        assert_eq!(
            fs::read_to_string(career_dir.join(file_name)).expect("auxiliar apos falha"),
            "{\"origem\":\"viva\"}",
            "'{file_name}' devia ter voltado ao estado anterior ao restore"
        );
        assert!(
            !career_dir.join(format!("{file_name}.novo")).exists(),
            "staging de '{file_name}' nao pode sobreviver a falha"
        );
    }
    assert_eq!(
        fs::read_to_string(telas.join("C001.json")).expect("tela apos falha"),
        "{\"origem\":\"viva\"}"
    );
    assert_eq!(
        fs::read_to_string(&meta_path).expect("meta apos falha"),
        meta_vivo
    );

    // Sem sabotagem, a mesma restauracao leva todos juntos para o lado do snapshot.
    restore_backup_internal(&db_path, &career_dir, 1).expect("restore");
    for file_name in SIDECAR_FILES {
        assert_eq!(
            fs::read_to_string(career_dir.join(file_name)).expect("auxiliar restaurado"),
            "{\"origem\":\"do backup\"}"
        );
    }
    assert_eq!(
        fs::read_to_string(telas.join("C001.json")).expect("tela restaurada"),
        "{\"origem\":\"do backup\"}"
    );

    let _ = fs::remove_dir_all(base_dir);
}
