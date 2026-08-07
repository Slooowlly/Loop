use std::path::{Path, PathBuf};

use chrono::Local;

use crate::calendar::full_season::generate_full_season_calendar;
use crate::commands::career_types::{
    CareerDraftState, CreateCareerResult, CreateHistoricalDraftInput, DraftTeamOption,
    FinalizeHistoricalDraftInput, SaveLifecycleStatus, WorldSummary,
};
use crate::config::app_config::AppConfig;
use crate::config::app_config::SaveMeta;
use crate::constants::historical_timeline::is_category_active_in_year;
use crate::db::connection::{Database, DbError};
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::meta as meta_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::pipeline::run_historical_end_of_season;
use crate::finance::planning::{category_finance_scale_for, derive_budget_index_from_money};
use crate::finance::state::{choose_season_strategy, refresh_team_financial_state};
use crate::generators::ids::{next_id, IdType};
use crate::generators::nationality::format_nationality;
use crate::generators::world::generate_historical_world;
use crate::market::pipeline::fill_all_remaining_vacancies;
use crate::models::contract::generate_initial_contract;
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, SeasonPhase, TeamRole};
use crate::models::license::grant_driver_license_for_division_if_needed;
use crate::models::season::Season;
use crate::models::team::Team;
use crate::world::integrity::{audit_historical_world, WorldAuditReport};

const HISTORY_START_YEAR: i32 = 2000;
const HISTORY_END_YEAR: i32 = 2025;
const PLAYABLE_START_YEAR: i32 = 2026;
const STARTING_CATEGORY_IDS: [&str; 2] = ["mazda_rookie", "toyota_rookie"];

pub(crate) fn create_historical_career_draft_in_base_dir(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    let state = create_historical_career_draft_base(base_dir, input)?;
    let career_id = state
        .career_id
        .clone()
        .ok_or_else(|| "Draft sem career_id".to_string())?;
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let db_path = career_dir.join("career.db");
    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco do draft: {e}"))?;

    if let Err(error) = simulate_historical_range(
        &mut db,
        &career_dir,
        HISTORY_START_YEAR,
        HISTORY_END_YEAR,
        PLAYABLE_START_YEAR,
    ) {
        drop(db);
        mark_historical_draft_failed(&career_dir, &error)?;
        return Err(error);
    }

    if let Err(error) = audit_draft_world(&db.conn, PLAYABLE_START_YEAR) {
        drop(db);
        mark_historical_draft_failed(&career_dir, &error)?;
        return Err(error);
    }

    let meta = read_save_meta(&career_dir.join("meta.json"))?;
    build_draft_state(&career_id, &career_dir, &meta)
}

pub(crate) fn get_career_draft_in_base_dir(base_dir: &Path) -> Result<CareerDraftState, String> {
    let config = AppConfig::load_or_default(base_dir);
    let Some((career_id, career_dir, meta)) = find_latest_draft(&config)? else {
        return Ok(empty_draft_state());
    };

    build_draft_state(&career_id, &career_dir, &meta)
}

pub(crate) fn discard_career_draft_in_base_dir(base_dir: &Path) -> Result<(), String> {
    let config = AppConfig::load_or_default(base_dir);
    let Some((_career_id, career_dir, _meta)) = find_latest_draft(&config)? else {
        return Ok(());
    };

    let saves_dir = config
        .saves_dir()
        .canonicalize()
        .map_err(|e| format!("Falha ao resolver diretorio de saves: {e}"))?;
    let target_dir = career_dir
        .canonicalize()
        .map_err(|e| format!("Falha ao resolver diretorio do draft: {e}"))?;
    if !target_dir.starts_with(&saves_dir) {
        return Err("Diretorio do draft fora da pasta de saves.".to_string());
    }

    remove_dir_all_resilient(&target_dir)
        .map_err(|e| format!("Falha ao descartar draft historico: {e}"))
}

pub(crate) fn finalize_career_draft_in_base_dir(
    base_dir: &Path,
    input: FinalizeHistoricalDraftInput,
) -> Result<CreateCareerResult, String> {
    finalize_career_draft(base_dir, input)
}

#[cfg(test)]
pub(crate) fn create_historical_career_draft_base_for_test(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    create_historical_career_draft_base(base_dir, input)
}

fn create_historical_career_draft_base(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
) -> Result<CareerDraftState, String> {
    let normalized_name = input.player_name.trim().to_string();
    if normalized_name.is_empty() {
        return Err("Informe um nome para o piloto.".to_string());
    }

    let normalized_nationality = input.player_nationality.trim().to_lowercase();
    let normalized_difficulty = input.difficulty.trim().to_lowercase();
    let normalized_age = input.player_age.unwrap_or(20).clamp(16, 60);

    let config = AppConfig::load_or_default(base_dir);
    let saves_dir = config.saves_dir();
    let career_id = next_draft_career_id(&saves_dir);
    let career_number = career_number_from_id(&career_id)
        .ok_or_else(|| format!("Falha ao interpretar career_id '{career_id}'"))?;
    let career_dir = saves_dir.join(&career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    std::fs::create_dir_all(&career_dir)
        .map_err(|e| format!("Falha ao criar diretorio do draft: {e}"))?;

    let creation_result = (|| -> Result<CareerDraftState, String> {
        let mut db = Database::create_new(&db_path)
            .map_err(|e| format!("Falha ao criar banco do draft: {e}"))?;
        let world = generate_historical_world(&normalized_difficulty, HISTORY_START_YEAR)?;

        let season_id = "S001".to_string();
        let mut season = Season::new(season_id.clone(), 1, HISTORY_START_YEAR);
        season.fase = SeasonPhase::Temporada;
        let calendar_seed: u64 = rand::random();

        let total_races = db
            .transaction(|tx| {
                for driver in &world.drivers {
                    driver_queries::insert_driver(tx, driver)?;
                }
                team_queries::insert_teams(tx, &world.teams)?;
                // Sistema de Nível do Carro: o draft histórico semeia o carro igual à
                // carreira clássica. Sem isto o carro só nascia no 1º pit da 1ª corrida,
                // pelo fallback neutro de `maintain_team_car_pits` (qualidade 0,5 pra TODO
                // mundo) — as 26 temporadas de backstory largavam com o grid inteiro no
                // mesmo carro, e a hierarquia do seed era descartada.
                crate::market::car_maintenance::seed_and_persist_team_cars(tx, &world.teams)?;
                contract_queries::insert_contracts(tx, &world.contracts)?;
                for contract in &world.contracts {
                    grant_driver_license_for_division_if_needed(
                        tx,
                        &contract.piloto_id,
                        &contract.categoria,
                        contract.classe.as_deref(),
                    )
                    .map_err(DbError::Migration)?;
                }
                season_queries::insert_season(tx, &season)?;
                let n = generate_full_season_calendar(tx, &season_id, season.ano, calendar_seed)?;
                sync_draft_meta_counters(
                    tx,
                    world.drivers.len(),
                    world.teams.len(),
                    world.contracts.len(),
                    1,
                    n,
                    HISTORY_START_YEAR,
                    PLAYABLE_START_YEAR,
                )?;
                Ok(n)
            })
            .map_err(|e| format!("Falha ao persistir dados do draft: {e}"))?;

        write_draft_meta(
            &meta_path,
            career_number,
            &normalized_name,
            &normalized_nationality,
            normalized_age,
            &normalized_difficulty,
            total_races as i32,
        )?;

        Ok(CareerDraftState {
            exists: true,
            career_id: Some(career_id),
            lifecycle_status: SaveLifecycleStatus::Draft,
            progress_year: Some(HISTORY_START_YEAR as u32),
            error: None,
            categories: Vec::new(),
            teams: Vec::new(),
            world_summary: None,
        })
    })();

    if creation_result.is_err() && career_dir.exists() {
        let _ = std::fs::remove_dir_all(&career_dir);
    }

    creation_result
}

fn sync_draft_meta_counters(
    conn: &rusqlite::Connection,
    total_drivers: usize,
    total_teams: usize,
    total_contracts: usize,
    total_seasons: usize,
    total_races: usize,
    current_year: i32,
    career_start_year: i32,
) -> Result<(), DbError> {
    meta_queries::set_meta_value(
        conn,
        "next_driver_id",
        &(total_drivers as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(conn, "next_team_id", &(total_teams as u32 + 1).to_string())?;
    meta_queries::set_meta_value(
        conn,
        "next_contract_id",
        &(total_contracts as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(
        conn,
        "next_season_id",
        &(total_seasons as u32 + 1).to_string(),
    )?;
    meta_queries::set_meta_value(conn, "next_race_id", &(total_races as u32 + 1).to_string())?;
    meta_queries::set_meta_value(conn, "current_season", &total_seasons.to_string())?;
    meta_queries::set_meta_value(conn, "current_year", &current_year.to_string())?;
    // Marca o ano-base da carreira: o 1º ano JOGÁVEL, não o início do backstory. Diferente
    // de "current_year" (que avança a cada temporada), este valor é escrito UMA ÚNICA VEZ na
    // criação e nunca atualizado.
    //
    // Recebia `current_year` (= HISTORY_START_YEAR, 2000), o que fazia toda a carreira
    // nascer com 26 "anos de carreira" já rodados — qualquer regra ancorada aqui já
    // começava esmaecida.
    meta_queries::set_meta_value(conn, "career_start_year", &career_start_year.to_string())?;
    Ok(())
}

fn write_draft_meta(
    meta_path: &Path,
    career_number: u32,
    player_name: &str,
    player_nationality: &str,
    player_age: i32,
    difficulty: &str,
    total_races: i32,
) -> Result<(), String> {
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let meta = serde_json::json!({
        "version": 1,
        "career_number": career_number,
        "player_name": player_name,
        "current_season": 1,
        "current_year": HISTORY_START_YEAR,
        "created_at": now,
        "last_played": now,
        "team_name": null,
        "category": "",
        "difficulty": difficulty,
        "total_races": total_races,
        "lifecycle_status": "draft",
        "history_start_year": HISTORY_START_YEAR,
        "history_end_year": HISTORY_END_YEAR,
        "playable_start_year": PLAYABLE_START_YEAR,
        "draft_progress_year": HISTORY_START_YEAR,
        "draft_error": null,
        "pending_player_nationality": player_nationality,
        "pending_player_age": player_age,
    });
    let payload = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta do draft: {e}"))?;
    std::fs::write(meta_path, payload).map_err(|e| format!("Falha ao gravar meta do draft: {e}"))
}

fn next_draft_career_id(saves_dir: &Path) -> String {
    if !saves_dir.exists() {
        return "career_001".to_string();
    }

    let next_number = std::fs::read_dir(saves_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.strip_prefix("career_")?.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
        + 1;

    format!("career_{next_number:03}")
}

fn career_number_from_id(career_id: &str) -> Option<u32> {
    career_id.strip_prefix("career_")?.parse::<u32>().ok()
}

fn finalize_career_draft(
    base_dir: &Path,
    input: FinalizeHistoricalDraftInput,
) -> Result<CreateCareerResult, String> {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(&input.career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");
    if !career_dir.exists() {
        return Err("Draft nao encontrado.".to_string());
    }

    let meta_content = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Falha ao ler meta do draft: {e}"))?;
    let mut meta: crate::config::app_config::SaveMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Falha ao parsear meta do draft: {e}"))?;
    if meta.lifecycle_status != SaveLifecycleStatus::Draft {
        return Err("Somente drafts podem ser finalizados.".to_string());
    }

    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco do draft: {e}"))?;
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa do draft: {e}"))?
        .ok_or_else(|| "Temporada ativa do draft nao encontrada.".to_string())?;
    if let Err(error) = audit_draft_world(&db.conn, active_season.ano) {
        drop(db);
        mark_historical_draft_failed(&career_dir, &error)?;
        return Err(error);
    }
    let mut selected_team = team_queries::get_team_by_id(&db.conn, &input.team_id)
        .map_err(|e| format!("Falha ao buscar equipe selecionada: {e}"))?
        .ok_or_else(|| "Equipe selecionada nao encontrada.".to_string())?;
    if selected_team.categoria != input.category {
        return Err("Equipe selecionada nao pertence a categoria escolhida.".to_string());
    }
    let displaced_n2 = selected_team
        .piloto_2_id
        .clone()
        .ok_or_else(|| "Equipe selecionada nao possui N2 para substituir.".to_string())?;

    let pending_nationality = meta
        .pending_player_nationality
        .clone()
        .unwrap_or_else(|| "br".to_string());
    let player_age = meta.pending_player_age.unwrap_or(20).clamp(16, 60);
    let player_nationality = format_nationality(&pending_nationality, "M", "pt-BR");
    let player_name = meta.player_name.clone();

    let (player_id, player_team_id, player_team_name, total_drivers, total_teams, total_races) = db
        .transaction(|tx| {
            let player_id = next_id(tx, IdType::Driver)?;
            let contract_id = next_id(tx, IdType::Contract)?;
            let mut player = Driver::new_player(
                player_id.clone(),
                player_name.clone(),
                player_nationality,
                player_age as u32,
                active_season.ano.max(0) as u32,
            );
            player.categoria_atual = Some(input.category.clone());
            driver_queries::insert_driver(tx, &player)?;
            grant_driver_license_for_division_if_needed(
                tx,
                &player.id,
                &input.category,
                selected_team.classe.as_deref(),
            )
            .map_err(DbError::Migration)?;

            if let Some(displaced_contract) =
                contract_queries::get_active_regular_contract_for_pilot(tx, &displaced_n2)?
            {
                contract_queries::update_contract_status(
                    tx,
                    &displaced_contract.id,
                    &ContractStatus::Rescindido,
                )?;
            }

            selected_team.piloto_2_id = Some(player.id.clone());
            selected_team.hierarquia_n2_id = Some(player.id.clone());
            selected_team.is_player_team = true;
            team_queries::update_team(tx, &selected_team)?;

            let mut player_contract = generate_initial_contract(
                contract_id,
                &player.id,
                &player.nome,
                &selected_team.id,
                &selected_team.nome,
                TeamRole::Numero2,
                &input.category,
                active_season.numero,
            );
            player_contract.classe = selected_team.classe.clone();
            contract_queries::insert_contract(tx, &player_contract)?;

            let total_drivers = driver_queries::count_drivers(tx)? as usize;
            let total_teams = count_rows(tx, "teams")?;
            let total_races = count_rows(tx, "calendar")?;

            Ok((
                player.id.clone(),
                selected_team.id.clone(),
                selected_team.nome.clone(),
                total_drivers,
                total_teams,
                total_races,
            ))
        })
        .map_err(|e| format!("Falha ao finalizar draft: {e}"))?;

    meta.lifecycle_status = SaveLifecycleStatus::Active;
    meta.current_season = active_season.numero.max(1) as u32;
    meta.current_year = active_season.ano.max(0) as u32;
    meta.team_name = Some(player_team_name.clone());
    meta.category = input.category;
    meta.total_races = total_races as i32;
    meta.draft_progress_year = None;
    meta.draft_error = None;
    let payload = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar meta finalizado: {e}"))?;
    std::fs::write(&meta_path, payload)
        .map_err(|e| format!("Falha ao gravar meta finalizado: {e}"))?;

    Ok(CreateCareerResult {
        success: true,
        career_id: input.career_id,
        save_path: career_dir.to_string_lossy().to_string(),
        player_id,
        player_team_id,
        player_team_name,
        season_id: active_season.id,
        total_drivers,
        total_teams,
        total_races,
        message: "Carreira historica criada com sucesso".to_string(),
    })
}

fn count_rows(conn: &rusqlite::Connection, table: &str) -> Result<usize, DbError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(count as usize)
}

fn empty_draft_state() -> CareerDraftState {
    CareerDraftState {
        exists: false,
        career_id: None,
        lifecycle_status: SaveLifecycleStatus::Active,
        progress_year: None,
        error: None,
        categories: Vec::new(),
        teams: Vec::new(),
        world_summary: None,
    }
}

fn find_latest_draft(config: &AppConfig) -> Result<Option<(String, PathBuf, SaveMeta)>, String> {
    let saves_dir = config.saves_dir();
    if !saves_dir.exists() {
        return Ok(None);
    }

    let entries = std::fs::read_dir(&saves_dir)
        .map_err(|e| format!("Falha ao listar saves para buscar draft: {e}"))?;
    let mut candidates = Vec::new();
    for entry in entries.filter_map(|entry| entry.ok()) {
        let career_dir = entry.path();
        let career_id = entry.file_name().to_string_lossy().to_string();
        if !career_id.starts_with("career_") {
            continue;
        }
        let meta_path = career_dir.join("meta.json");
        let Ok(content) = std::fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<SaveMeta>(&content) else {
            continue;
        };
        if matches!(
            meta.lifecycle_status,
            SaveLifecycleStatus::Draft | SaveLifecycleStatus::Failed
        ) {
            candidates.push((career_id, career_dir, meta));
        }
    }

    candidates.sort_by(|a, b| b.2.last_played.cmp(&a.2.last_played));
    Ok(candidates.into_iter().next())
}

fn build_draft_state(
    career_id: &str,
    career_dir: &Path,
    meta: &SaveMeta,
) -> Result<CareerDraftState, String> {
    let mut state = CareerDraftState {
        exists: true,
        career_id: Some(career_id.to_string()),
        lifecycle_status: meta.lifecycle_status,
        progress_year: meta.draft_progress_year,
        error: meta.draft_error.clone(),
        categories: Vec::new(),
        teams: Vec::new(),
        world_summary: None,
    };

    if meta.lifecycle_status == SaveLifecycleStatus::Failed {
        return Ok(state);
    }

    let db_path = career_dir.join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco do draft: {e}"))?;

    // Um draft só é finalizável se o mundo passa na auditoria do ano jogável.
    // Uma geração interrompida (app fechado no meio) ou inválida deixa o save num
    // estado que reprovaria na finalização (ex.: sem calendário jogável, contadores
    // de meta defasados). Em vez de apresentá-lo como pronto — equipes selecionáveis,
    // botão "Finalizar" — e só quebrar no fim, detectamos aqui: devolvemos o estado
    // com o erro real e sem equipes, o que faz o fluxo obrigar a regerar em vez de
    // finalizar um mundo quebrado.
    // A auditoria roda contra o ano da temporada ATIVA no banco — a mesma fonte de
    // verdade que a finalização usa (`finalize_career_draft`). Assim garantimos que,
    // se o mundo reprovaria em "Finalizar", ele também não é apresentado como pronto
    // aqui. Sem temporada ativa (ou reprovando na auditoria) devolvemos o estado com
    // o erro e sem equipes, o que força a regeração.
    match season_queries::get_active_season(&db.conn) {
        Ok(Some(active_season)) => {
            if let Err(error) = audit_draft_world(&db.conn, active_season.ano) {
                state.error = Some(error);
                return Ok(state);
            }
        }
        Ok(None) => {
            state.error = Some("Temporada ativa do draft nao encontrada.".to_string());
            return Ok(state);
        }
        Err(e) => {
            return Err(format!("Falha ao buscar temporada ativa do draft: {e}"));
        }
    }

    let teams = team_queries::get_all_teams(&db.conn)
        .map_err(|e| format!("Falha ao listar equipes do draft: {e}"))?;

    for category_id in STARTING_CATEGORY_IDS {
        let mut category_has_team = false;
        for team in teams
            .iter()
            .filter(|team| team.ativa && team.categoria == category_id)
        {
            category_has_team = true;
            state.teams.push(DraftTeamOption {
                id: team.id.clone(),
                nome: team.nome.clone(),
                nome_curto: team.nome_curto.clone(),
                categoria: team.categoria.clone(),
                cor_primaria: team.cor_primaria.clone(),
                cor_secundaria: team.cor_secundaria.clone(),
                car_performance: team.effective_car_performance(),
                reputacao: team.reputacao,
                n1_nome: optional_driver_name(&db.conn, team.piloto_1_id.as_deref()),
                n2_nome: optional_driver_name(&db.conn, team.piloto_2_id.as_deref()),
            });
        }
        if category_has_team {
            state.categories.push(category_id.to_string());
        }
    }

    state.world_summary = build_world_summary(&db.conn);

    Ok(state)
}

/// Conta o mundo simulado a partir dos arquivos de temporada persistidos. Cada query
/// degrada pra 0 se a tabela estiver vazia. `corridas` = soma das vitórias de equipe
/// (cada corrida tem exatamente um vencedor). Retorna None se não houver histórico.
fn build_world_summary(conn: &rusqlite::Connection) -> Option<WorldSummary> {
    let count = |sql: &str| -> i64 {
        conn.query_row(sql, [], |row| row.get::<_, i64>(0))
            .unwrap_or(0)
    };

    let summary = WorldSummary {
        temporadas: count("SELECT COUNT(DISTINCT season_number) FROM team_season_archive"),
        pilotos: count("SELECT COUNT(DISTINCT piloto_id) FROM driver_season_archive"),
        corridas: count("SELECT COALESCE(SUM(vitorias), 0) FROM team_season_archive"),
        // Campeões = pilotos distintos que venceram ao menos um campeonato.
        campeoes: count(
            "SELECT COUNT(DISTINCT piloto_id) FROM driver_season_archive \
             WHERE posicao_campeonato = 1",
        ),
        // Tricampeões = pilotos com 3+ títulos.
        tricampeoes: count(
            "SELECT COUNT(*) FROM ( \
                SELECT piloto_id FROM driver_season_archive \
                WHERE posicao_campeonato = 1 \
                GROUP BY piloto_id HAVING COUNT(*) >= 3 \
             )",
        ),
    };

    // Sem nenhuma temporada arquivada não há mundo a resumir.
    if summary.temporadas == 0 {
        return None;
    }
    Some(summary)
}

fn audit_draft_world(conn: &rusqlite::Connection, playable_year: i32) -> Result<(), String> {
    let report = audit_historical_world(conn, playable_year)?;
    if report.is_valid() {
        return Ok(());
    }
    Err(summarize_audit_errors(&report))
}

fn summarize_audit_errors(report: &WorldAuditReport) -> String {
    let summary = report
        .errors
        .iter()
        .take(3)
        .map(|issue| format!("{}: {}", issue.code, issue.message))
        .collect::<Vec<_>>()
        .join("; ");
    if report.errors.len() > 3 {
        format!(
            "Mundo historico invalido: {summary}; +{} erro(s).",
            report.errors.len() - 3
        )
    } else {
        format!("Mundo historico invalido: {summary}.")
    }
}

fn mark_historical_draft_failed(career_dir: &Path, message: &str) -> Result<(), String> {
    let meta_path = career_dir.join("meta.json");
    if meta_path.exists() {
        let content = std::fs::read_to_string(&meta_path)
            .map_err(|e| format!("Falha ao ler meta do draft falho: {e}"))?;
        let mut meta: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Falha ao parsear meta do draft falho: {e}"))?;
        meta["lifecycle_status"] = serde_json::json!("failed");
        meta["draft_error"] = serde_json::json!(message);
        meta["draft_progress_year"] = serde_json::Value::Null;
        let payload = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("Falha ao serializar meta do draft falho: {e}"))?;
        std::fs::write(&meta_path, payload)
            .map_err(|e| format!("Falha ao gravar meta do draft falho: {e}"))?;
    }

    // A limpeza dos artefatos é BEST-EFFORT de propósito. O meta.json acima já
    // registrou a causa REAL da falha; se a remoção de um arquivo falhar (no
    // Windows é comum um `os error 32` transitório — antivírus/indexador segurando
    // o handle logo após fechar a conexão SQLite), não podemos deixar essa falha
    // secundária mascarar o erro de verdade que o chamador vai propagar. Tentamos
    // com retry e, se ainda assim não der, apenas registramos e seguimos.
    for path in [
        career_dir.join("career.db"),
        career_dir.join("career.db-shm"),
        career_dir.join("career.db-wal"),
        career_dir.join("preseason_plan.json"),
    ] {
        if let Err(e) = remove_file_resilient(&path) {
            eprintln!(
                "Aviso: falha ao remover artefato de draft falho '{}': {e}",
                path.display()
            );
        }
    }

    let backups_dir = career_dir.join("backups");
    if let Err(e) = remove_dir_all_resilient(&backups_dir) {
        eprintln!("Aviso: falha ao limpar backups do draft falho: {e}");
    }

    Ok(())
}

/// Remove um arquivo tolerando o bloqueio transitório do Windows. Logo após fechar
/// uma conexão SQLite, o antivírus ou o indexador pode segurar o handle por alguns
/// milissegundos, fazendo `remove_file` retornar `os error 32` (sharing violation).
/// Alguns retries curtos resolvem. Arquivo inexistente conta como sucesso.
fn remove_file_resilient(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    remove_with_retry(|| std::fs::remove_file(path))
}

/// Versão para diretórios (mesma lógica de retry contra locks transitórios do Windows).
fn remove_dir_all_resilient(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    remove_with_retry(|| std::fs::remove_dir_all(path))
}

fn remove_with_retry<F>(op: F) -> std::io::Result<()>
where
    F: Fn() -> std::io::Result<()>,
{
    const ATTEMPTS: usize = 12;
    let mut last_err = None;
    for attempt in 0..ATTEMPTS {
        match op() {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < ATTEMPTS {
                    // Backoff curto e crescente: ~50ms, 100ms, ... até dar tempo do
                    // scanner/indexador liberar o handle sem travar a UI por muito tempo.
                    std::thread::sleep(std::time::Duration::from_millis(50 * (attempt as u64 + 1)));
                }
            }
        }
    }
    Err(last_err.expect("retry loop sempre registra o último erro antes de sair"))
}

fn read_save_meta(meta_path: &Path) -> Result<SaveMeta, String> {
    let content = std::fs::read_to_string(meta_path)
        .map_err(|e| format!("Falha ao ler meta do draft: {e}"))?;
    serde_json::from_str::<SaveMeta>(&content)
        .map_err(|e| format!("Falha ao parsear meta do draft: {e}"))
}

fn optional_driver_name(conn: &rusqlite::Connection, driver_id: Option<&str>) -> Option<String> {
    driver_id.and_then(|id| {
        driver_queries::get_driver(conn, id)
            .ok()
            .map(|driver| driver.nome)
    })
}

#[cfg(test)]
pub(crate) fn create_historical_career_draft_for_range_for_test(
    base_dir: &Path,
    input: CreateHistoricalDraftInput,
    start_year: i32,
    end_year: i32,
    playable_year: i32,
) -> Result<CareerDraftState, String> {
    let state = create_historical_career_draft_base(base_dir, input)?;
    let career_id = state
        .career_id
        .clone()
        .ok_or_else(|| "Draft sem career_id".to_string())?;
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let db_path = career_dir.join("career.db");
    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco do draft: {e}"))?;

    simulate_historical_range(&mut db, &career_dir, start_year, end_year, playable_year)?;

    let meta = read_save_meta(&career_dir.join("meta.json"))?;
    build_draft_state(&career_id, &career_dir, &meta)
}

fn simulate_historical_range(
    db: &mut Database,
    career_dir: &Path,
    start_year: i32,
    end_year: i32,
    playable_year: i32,
) -> Result<(), String> {
    for _year in start_year..=end_year {
        simulate_current_historical_season(db)?;
        let current_season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada historica ativa: {e}"))?
            .ok_or_else(|| "Temporada historica ativa nao encontrada.".to_string())?;
        if current_season.fase.is_legacy() {
            simulate_current_historical_special_block(db)?;
        }
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada historica ativa: {e}"))?
            .ok_or_else(|| "Temporada historica ativa nao encontrada.".to_string())?;
        run_historical_end_of_season(&mut db.conn, &season, career_dir)?;
        let next_season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar proxima temporada historica: {e}"))?
            .ok_or_else(|| "Proxima temporada historica nao encontrada.".to_string())?;
        fill_all_remaining_vacancies(&db.conn, next_season.numero, &mut rand::thread_rng())?;
        clear_historical_news(&db.conn)?;
        update_draft_progress(career_dir, (season.ano + 1) as u32)?;
    }

    purge_never_raced_backstory_orphans(&db.conn)?;
    reset_historical_finance_for_playable_start(&db.conn)?;
    sync_meta_counters_from_observed(&db.conn)?;

    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada jogavel ativa: {e}"))?
        .ok_or_else(|| "Temporada jogavel ativa nao encontrada.".to_string())?;
    if active_season.ano != playable_year {
        return Err(format!(
            "Ano jogavel esperado {playable_year}, encontrado {}.",
            active_season.ano
        ));
    }
    Ok(())
}

fn simulate_current_historical_special_block(db: &mut Database) -> Result<(), String> {
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada historica ativa: {e}"))?
        .ok_or_else(|| "Temporada historica ativa nao encontrada.".to_string())?;

    crate::convocation::advance_to_convocation_window(&db.conn)
        .map_err(|e| format!("Falha ao abrir janela especial historica: {e}"))?;
    crate::convocation::run_convocation_window(&db.conn)
        .map_err(|e| format!("Falha ao gerar convocacoes especiais historicas: {e}"))?;
    crate::convocation::iniciar_bloco_especial(&db.conn)
        .map_err(|e| format!("Falha ao iniciar bloco especial historico: {e}"))?;

    for category_id in ["production_challenger", "endurance"] {
        if !is_category_active_in_year(category_id, season.ano) {
            continue;
        }

        let pending =
            calendar_queries::get_pending_races_for_category(&db.conn, &season.id, category_id)
                .map_err(|e| {
                    format!("Falha ao buscar corridas especiais historicas de {category_id}: {e}")
                })?;

        for race in &pending {
            crate::commands::race::simulate_historical_category_race(db, race)?;
        }
    }

    crate::convocation::encerrar_bloco_especial(&db.conn)
        .map_err(|e| format!("Falha ao encerrar bloco especial historico: {e}"))?;
    crate::convocation::run_pos_especial(&db.conn)
        .map_err(|e| format!("Falha ao limpar pos-especial historico: {e}"))?;

    Ok(())
}

/// Remove órfãos do backstory: pilotos da IA que NUNCA competiram e não têm contrato
/// regular ativo. São artefatos da geração histórica — as categorias de estreia
/// (mazda/toyota_rookie) só passam a existir em 2020, mas seus times eram preenchidos
/// nos anos anteriores com rookies que nunca chegaram a correr (categoria não simulada).
/// Roda uma vez ao fim do draft, antes do ano jogável, para o mundo começar limpo.
/// O critério "nunca correu + sem contrato regular ativo" preserva os rookies recém
/// colocados para o ano jogável (esses têm contrato ativo).
fn purge_never_raced_backstory_orphans(conn: &rusqlite::Connection) -> Result<usize, String> {
    const ORPHAN_FILTER: &str = "status = 'Ativo' AND is_jogador = 0 AND carreira_corridas = 0
         AND id NOT IN (
             SELECT piloto_id FROM contracts WHERE status = 'Ativo' AND tipo = 'Regular'
         )";
    let select_ids = format!("SELECT id FROM drivers WHERE {ORPHAN_FILTER}");

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar poda de orfaos do backstory: {e}"))?;
    for table in ["special_window_candidate_pool", "licenses", "contracts"] {
        let column = if table == "special_window_candidate_pool" {
            "driver_id"
        } else {
            "piloto_id"
        };
        tx.execute(
            &format!("DELETE FROM {table} WHERE {column} IN ({select_ids})"),
            [],
        )
        .map_err(|e| format!("Falha ao limpar '{table}' de orfaos do backstory: {e}"))?;
    }
    let removed = tx
        .execute(&format!("DELETE FROM drivers WHERE {ORPHAN_FILTER}"), [])
        .map_err(|e| format!("Falha ao remover orfaos do backstory: {e}"))?;
    tx.commit()
        .map_err(|e| format!("Falha ao concluir poda de orfaos do backstory: {e}"))?;
    Ok(removed)
}

fn reset_historical_finance_for_playable_start(conn: &rusqlite::Connection) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao carregar equipes para limpar financeiro historico: {e}"))?;

    for team in teams {
        let mut updated_team = team;
        updated_team.cash_balance = playable_start_cash_balance(&updated_team);
        updated_team.debt_balance = 0.0;
        updated_team.last_round_income = 0.0;
        updated_team.last_round_expenses = 0.0;
        updated_team.last_round_net = 0.0;
        updated_team.parachute_payment_remaining = 0.0;
        refresh_team_financial_state(&mut updated_team);
        updated_team.season_strategy = choose_season_strategy(&updated_team).to_string();
        updated_team.budget = derive_budget_index_from_money(&updated_team);
        team_queries::update_team(conn, &updated_team).map_err(|e| {
            format!(
                "Falha ao limpar financeiro historico da equipe {}: {e}",
                updated_team.nome
            )
        })?;
    }

    Ok(())
}

fn playable_start_cash_balance(team: &Team) -> f64 {
    let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
    let category_window = (scale.cash_max - scale.cash_min).max(1.0);
    let reputation_weight = (team.reputacao / 100.0).clamp(0.0, 1.0);
    let performance_weight = ((team.car_performance + 5.0) / 21.0).clamp(0.0, 1.0);
    let structure_weight = ((team.facilities + team.engineering) / 200.0).clamp(0.0, 1.0);
    let position =
        (0.20 + reputation_weight * 0.35 + performance_weight * 0.20 + structure_weight * 0.25)
            .clamp(0.20, 0.90);

    scale.cash_min + category_window * position
}

fn update_draft_progress(career_dir: &Path, progress_year: u32) -> Result<(), String> {
    let meta_path = career_dir.join("meta.json");
    let content = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Falha ao ler meta do draft: {e}"))?;
    let mut meta: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Falha ao parsear meta do draft: {e}"))?;
    meta["draft_progress_year"] = serde_json::json!(progress_year);
    meta["current_year"] = serde_json::json!(progress_year);
    let payload = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Falha ao serializar progresso do draft: {e}"))?;
    std::fs::write(&meta_path, payload)
        .map_err(|e| format!("Falha ao gravar progresso do draft: {e}"))
}

fn clear_historical_news(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute("DELETE FROM news", [])
        .map_err(|e| format!("Falha ao limpar noticias historicas: {e}"))?;
    Ok(())
}

fn sync_meta_counters_from_observed(conn: &rusqlite::Connection) -> Result<(), String> {
    for (key, prefix, tables) in [
        ("next_driver_id", "P", &["drivers"][..]),
        ("next_team_id", "T", &["teams"][..]),
        ("next_season_id", "S", &["seasons"][..]),
        ("next_race_id", "R", &["calendar", "races"][..]),
        ("next_contract_id", "C", &["contracts"][..]),
    ] {
        let observed = observed_next_counter(conn, prefix, tables)?;
        meta_queries::set_meta_value(conn, key, &observed.to_string())
            .map_err(|e| format!("Falha ao sincronizar {key}: {e}"))?;
    }
    Ok(())
}

fn observed_next_counter(
    conn: &rusqlite::Connection,
    prefix: &str,
    tables: &[&str],
) -> Result<i64, String> {
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

fn simulate_current_historical_season(db: &mut Database) -> Result<(), String> {
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada historica ativa: {e}"))?
        .ok_or_else(|| "Temporada historica ativa nao encontrada.".to_string())?;
    let pending_races = calendar_queries::get_pending_races(&db.conn, &season.id)
        .map_err(|e| format!("Falha ao buscar corridas historicas pendentes: {e}"))?;

    for race in &pending_races {
        if !is_category_active_in_year(&race.categoria, season.ano) {
            calendar_queries::mark_race_completed(&db.conn, &race.id).map_err(|e| {
                format!(
                    "Falha ao fechar corrida historica inativa '{}' de {}: {e}",
                    race.id, race.categoria
                )
            })?;
            continue;
        }
        crate::commands::race::simulate_historical_category_race(db, race)?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "historical_draft/tests/mod.rs"]
mod tests;
