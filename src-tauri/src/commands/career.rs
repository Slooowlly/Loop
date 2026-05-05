use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::calendar::{generate_all_calendars_with_year, CalendarEntry};
use crate::commands::career_detail::build_driver_detail_payload;
use crate::commands::career_types::{
    AcceptedSpecialOfferSummary, BriefingPhraseEntry, BriefingPhraseEntryInput,
    BriefingPhraseHistory, BriefingStorySummary, CareerData, CareerResumeContext, CareerResumeView,
    ContractWarningInfo, CreateCareerResult, DriverDetail, DriverSummary, NextRaceBriefingSummary,
    PrimaryRivalSummary, RaceSummary, SaveInfo, SeasonSummary, TeamHistoryCategoryStep,
    TeamHistoryDossier, TeamHistoryIdentity, TeamHistoryManagement, TeamHistoryRecord,
    TeamHistoryRival, TeamHistorySport, TeamHistoryTimelineItem, TeamHistoryTitleCategory,
    TeamStanding, TeamSummary, TrackHistorySummary, VerifyDatabaseResponse,
};
use crate::commands::race_history::{
    build_driver_histories, empty_previous_champions, ConstructorChampion, DriverRaceHistory,
    PreviousChampions, RoundResult, TrophyInfo,
};
use crate::config::app_config::{AppConfig, SaveMeta};
use crate::constants::historical_timeline::historical_team_foundation_year;
use crate::constants::{categories, scoring};
use crate::db::connection::Database;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::injuries as injury_queries;
use crate::db::queries::market_proposals as market_proposal_queries;
use crate::db::queries::meta as meta_queries;
use crate::db::queries::news as news_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::special_team_entries as special_entry_queries;
use crate::db::queries::standings as standings_queries;
use crate::db::queries::standings::ChampionshipContext;
use crate::db::queries::teams as team_queries;
use crate::event_interest::{
    calculate_expected_event_interest, to_summary, EventInterestContext, EventInterestSummary,
};
use crate::evolution::pipeline::{run_end_of_season, EndOfSeasonResult};
use crate::finance::planning::calculate_financial_plan;
use crate::finance::salary::{calculate_offer_salary_from_money, calculate_salary_ceiling};
use crate::generators::ids::{next_id, IdType};
use crate::generators::nationality::{format_nationality, get_nationality};
use crate::generators::world::generate_world;
use crate::market::pipeline::fill_all_remaining_vacancies;
use crate::market::preseason::{
    advance_week, delete_preseason_plan, load_preseason_plan, save_preseason_plan, PendingAction,
    PlannedEvent, PreSeasonPlan, PreSeasonState, WeekResult,
};
use crate::market::proposals::{MarketProposal, ProposalStatus};
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, SeasonPhase, TeamRole};
use crate::models::license::{
    driver_has_required_license_for_category, ensure_driver_can_join_category,
    grant_driver_license_for_category_if_needed,
};
use crate::models::season::Season;
use crate::models::team::{Team, TeamHierarchyClimate};
use crate::news::{NewsImportance, NewsItem, NewsType};

pub use crate::commands::career_types::CreateCareerInput;

static CAREER_OPEN_REPAIR_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProposalView {
    pub proposal_id: String,
    pub equipe_id: String,
    pub equipe_nome: String,
    pub equipe_cor_primaria: String,
    pub equipe_cor_secundaria: String,
    pub categoria: String,
    pub categoria_nome: String,
    pub categoria_tier: u8,
    pub papel: String,
    pub salario_oferecido: f64,
    pub duracao_anos: i32,
    pub car_performance: f64,
    pub car_performance_rating: u8,
    pub reputacao: f64,
    pub companheiro_nome: Option<String>,
    pub companheiro_skill: Option<u8>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalResponse {
    pub success: bool,
    pub action: String,
    pub message: String,
    pub new_team_name: Option<String>,
    pub remaining_proposals: i32,
    pub news_generated: Vec<String>,
}

pub(crate) fn create_career_in_base_dir(
    base_dir: &Path,
    input: CreateCareerInput,
) -> Result<CreateCareerResult, String> {
    validate_create_career_input(&input)?;

    let normalized_name = input.player_name.trim().to_string();
    let normalized_nationality = input.player_nationality.trim().to_lowercase();
    let normalized_category = input.category.trim().to_lowercase();
    let normalized_difficulty = input.difficulty.trim().to_lowercase();
    let normalized_age = input.player_age.unwrap_or(20).clamp(16, 60);
    let nationality_label = format_nationality(&normalized_nationality, "M", "pt-BR");

    let mut config = AppConfig::load_or_default(base_dir);
    let saves_dir = config.saves_dir();
    let career_id = next_career_id(&saves_dir);
    let career_number = career_number_from_id(&career_id)
        .ok_or_else(|| format!("Falha ao interpretar career_id '{career_id}'"))?;
    let career_dir = saves_dir.join(&career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    std::fs::create_dir_all(&career_dir)
        .map_err(|e| format!("Falha ao criar diretorio da carreira: {e}"))?;

    let creation_result = (|| -> Result<CreateCareerResult, String> {
        let mut db = Database::create_new(&db_path)
            .map_err(|e| format!("Falha ao criar banco da carreira: {e}"))?;

        let world = generate_world(
            &normalized_name,
            &nationality_label,
            normalized_age,
            &normalized_category,
            input.team_index,
            &normalized_difficulty,
        )?;

        let season_id = next_id(&db.conn, IdType::Season)
            .map_err(|e| format!("Falha ao gerar ID da temporada: {e}"))?;
        let season = Season::new(season_id.clone(), 1, 2024);
        let calendars =
            generate_all_calendars_with_year(&season_id, season.ano, &mut rand::thread_rng())?;
        let total_races = count_total_races(&calendars);
        let all_calendar_entries: Vec<CalendarEntry> = calendars
            .values()
            .flat_map(|entries| entries.iter().cloned())
            .collect();

        db.transaction(|tx| {
            for driver in &world.drivers {
                driver_queries::insert_driver(tx, driver)?;
            }

            team_queries::insert_teams(tx, &world.teams)?;
            contract_queries::insert_contracts(tx, &world.contracts)?;
            for contract in &world.contracts {
                grant_driver_license_for_category_if_needed(
                    tx,
                    &contract.piloto_id,
                    &contract.categoria,
                )
                .map_err(crate::db::connection::DbError::Migration)?;
            }
            season_queries::insert_season(tx, &season)?;
            calendar_queries::insert_calendar_entries(tx, &all_calendar_entries)?;
            sync_meta_counters(
                tx,
                world.drivers.len(),
                world.teams.len(),
                world.contracts.len(),
                1,
                total_races,
            )?;
            Ok(())
        })
        .map_err(|e| format!("Falha ao persistir dados da carreira: {e}"))?;

        let player_team = world
            .teams
            .iter()
            .find(|team| team.id == world.player_team_id)
            .ok_or_else(|| "Equipe do jogador nao encontrada apos gerar o mundo".to_string())?;

        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let meta = serde_json::json!({
            "version": 1,
            "career_number": career_number,
            "player_name": normalized_name,
            "current_season": 1,
            "current_year": 2024,
            "created_at": now,
            "last_played": now,
            "team_name": player_team.nome,
            "category": normalized_category,
            "difficulty": normalized_difficulty,
            "total_races": total_races as i32,
        });

        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
        std::fs::write(&meta_path, meta_json)
            .map_err(|e| format!("Falha ao gravar meta.json: {e}"))?;

        config.last_career = Some(career_number);
        config
            .save()
            .map_err(|e| format!("Falha ao salvar config do app: {e}"))?;

        Ok(CreateCareerResult {
            success: true,
            career_id,
            save_path: career_dir.to_string_lossy().to_string(),
            player_id: world.player.id,
            player_team_id: player_team.id.clone(),
            player_team_name: player_team.nome.clone(),
            season_id,
            total_drivers: world.drivers.len(),
            total_teams: world.teams.len(),
            total_races,
            message: "Carreira criada com sucesso".to_string(),
        })
    })();

    if creation_result.is_err() && career_dir.exists() {
        let _ = std::fs::remove_dir_all(&career_dir);
    }

    creation_result
}

pub(crate) fn load_career_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<CareerData, String> {
    let career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let mut config = AppConfig::load_or_default(base_dir);
    let (db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let mut active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let pending_regular_races = calendar_queries::count_pending_races_in_phase(
        &db.conn,
        &active_season.id,
        &SeasonPhase::BlocoRegular,
    )
    .map_err(|e| format!("Falha ao verificar corridas regulares pendentes: {e}"))?;
    if active_season.fase == SeasonPhase::JanelaConvocacao && pending_regular_races > 0 {
        season_queries::update_season_fase(&db.conn, &active_season.id, &SeasonPhase::BlocoRegular)
            .map_err(|e| format!("Falha ao corrigir fase da temporada: {e}"))?;
        active_season.fase = SeasonPhase::BlocoRegular;
    }
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar piloto do jogador: {e}"))?;
    let player_team = find_player_team(&db.conn, &player.id, active_season.fase)?;
    let next_race = if let Some(ref team) = player_team {
        calendar_queries::get_next_race(&db.conn, &active_season.id, &team.categoria)
            .map_err(|e| format!("Falha ao carregar proxima corrida: {e}"))?
    } else {
        None
    };

    let total_drivers = driver_queries::count_drivers(&db.conn)
        .map_err(|e| format!("Falha ao contar pilotos: {e}"))? as usize;
    let total_teams =
        count_rows(&db.conn, "teams").map_err(|e| format!("Falha ao contar equipes: {e}"))?;
    let total_rodadas = if let Some(ref team) = player_team {
        count_calendar_entries(&db.conn, &active_season.id, &team.categoria)
            .map_err(|e| format!("Falha ao contar corridas da temporada: {e}"))?
    } else {
        0
    };

    // Calcular interesse esperado da próxima corrida (fallback silencioso se falhar).
    // Usa race.categoria como fonte semântica do campeonato do evento.
    let event_interest_summary: Option<EventInterestSummary> = next_race.as_ref().map(|race| {
        let champ = standings_queries::get_championship_context(&db.conn, &race.categoria)
            .unwrap_or(ChampionshipContext {
                player_position: 0,
                gap_to_leader: 0,
            });
        let remaining = total_rodadas - race.rodada;
        let is_title_decider =
            remaining <= 2 && champ.gap_to_leader <= 50 && champ.player_position > 0;
        let ctx = EventInterestContext {
            categoria: race.categoria.clone(),
            season_phase: race.season_phase,
            rodada: race.rodada,
            total_rodadas,
            week_of_year: race.week_of_year,
            track_id: race.track_id as i32,
            track_name: race.track_name.clone(),
            is_player_event: true,
            player_championship_position: if champ.player_position > 0 {
                Some(champ.player_position)
            } else {
                None
            },
            player_media: Some(player.atributos.midia as f32),
            championship_gap_to_leader: if champ.gap_to_leader > 0 || champ.player_position == 1 {
                Some(champ.gap_to_leader)
            } else {
                None
            },
            is_title_decider_candidate: is_title_decider,
            thematic_slot: race.thematic_slot,
        };
        let result = calculate_expected_event_interest(&ctx);
        to_summary(&result)
    });

    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    meta.last_played = now.clone();
    write_save_meta(&meta_path, &meta)?;
    config.last_career = Some(career_number);
    config
        .save()
        .map_err(|e| format!("Falha ao atualizar config do app: {e}"))?;

    let team_summary = player_team
        .as_ref()
        .map(|team| {
            build_team_summary(&db.conn, team)
                .map_err(|e| format!("Falha ao montar resumo da equipe: {e}"))
        })
        .transpose()?;
    let accepted_special_offer = build_accepted_special_offer_summary(&db.conn, &player)?;
    let next_race_summary = next_race.as_ref().map(|race| RaceSummary {
        id: race.id.clone(),
        rodada: race.rodada,
        track_name: race.track_name.clone(),
        clima: race.clima.as_str().to_string(),
        duracao_corrida_min: race.duracao_corrida_min,
        status: race.status.as_str().to_string(),
        temperatura: race.temperatura,
        horario: race.horario.clone(),
        week_of_year: race.week_of_year,
        season_phase: race.season_phase.as_str().to_string(),
        display_date: race.display_date.clone(),
        event_interest: event_interest_summary.clone(),
    });
    let next_race_briefing_summary = next_race.as_ref().map(|race| {
        build_next_race_briefing_summary(&db.conn, &player.id, active_season.numero, race)
            .unwrap_or_else(|_error| empty_next_race_briefing_summary())
    });
    let resume_context = read_resume_context(&career_dir)?;

    Ok(CareerData {
        career_id: career_id.to_string(),
        save_path: career_dir.to_string_lossy().to_string(),
        difficulty: meta.difficulty.clone(),
        player: DriverSummary {
            id: player.id.clone(),
            nome: player.nome.clone(),
            nacionalidade: player.nacionalidade.clone(),
            idade: player.idade as i32,
            skill: player.atributos.skill.round().clamp(0.0, 100.0) as u8,
            categoria_especial_ativa: player.categoria_especial_ativa.clone(),
            equipe_id: player_team.as_ref().map(|t| t.id.clone()),
            equipe_nome: player_team.as_ref().map(|t| t.nome.clone()),
            equipe_nome_curto: player_team.as_ref().map(|t| t.nome_curto.clone()),
            equipe_cor: player_team
                .as_ref()
                .map(|t| t.cor_primaria.clone())
                .unwrap_or_default(),
            classe: player_team.as_ref().and_then(|t| t.classe.clone()),
            is_jogador: player.is_jogador,
            is_estreante: player.temporadas_na_categoria == 0,
            is_estreante_da_vida: player.stats_carreira.corridas == 0,
            lesao_ativa_tipo: None,
            pontos: player.stats_temporada.pontos.round() as i32,
            vitorias: player.stats_temporada.vitorias as i32,
            podios: player.stats_temporada.podios as i32,
            posicao_campeonato: 0,
            results: Vec::new(),
        },
        player_team: team_summary,
        season: SeasonSummary {
            id: active_season.id.clone(),
            numero: active_season.numero,
            ano: active_season.ano,
            rodada_atual: active_season.rodada_atual,
            total_rodadas,
            status: active_season.status.as_str().to_string(),
            fase: active_season.fase.as_str().to_string(),
        },
        accepted_special_offer,
        next_race: next_race_summary,
        next_race_briefing: next_race_briefing_summary,
        total_drivers,
        total_teams,
        resume_context,
    })
}

pub(crate) fn delete_career_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<String, String> {
    let career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let mut config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);

    if !career_dir.exists() {
        return Err("Save nao encontrado.".to_string());
    }

    std::fs::remove_dir_all(&career_dir).map_err(|e| format!("Falha ao deletar save: {e}"))?;

    if config.last_career == Some(career_number) {
        config.last_career = None;
        config
            .save()
            .map_err(|e| format!("Falha ao atualizar config do app: {e}"))?;
    }

    Ok(format!("Carreira {career_id} deletada com sucesso."))
}

pub(crate) fn list_saves_in_base_dir(base_dir: &Path) -> Result<Vec<SaveInfo>, String> {
    let config = AppConfig::load_or_default(base_dir);
    Ok(config
        .list_saves()
        .into_iter()
        .map(save_meta_to_info)
        .collect())
}

fn validate_create_career_input(input: &CreateCareerInput) -> Result<(), String> {
    let name = input.player_name.trim();
    let nationality_id = input.player_nationality.trim().to_lowercase();
    let category = input.category.trim().to_lowercase();
    let difficulty = input.difficulty.trim().to_lowercase();
    if name.is_empty() {
        return Err("Informe um nome para o piloto.".to_string());
    }
    if name.chars().count() > 50 {
        return Err("O nome do piloto deve ter no maximo 50 caracteres.".to_string());
    }
    if get_nationality(&nationality_id).is_none() {
        return Err("Selecione uma nacionalidade valida.".to_string());
    }
    if !matches!(category.as_str(), "mazda_rookie" | "toyota_rookie") {
        return Err("A categoria inicial deve ser Mazda Rookie ou Toyota Rookie.".to_string());
    }
    if input.team_index > 5 {
        return Err("A equipe escolhida e invalida para a categoria inicial.".to_string());
    }
    if scoring::get_difficulty_config(&difficulty).is_none() {
        return Err("Selecione uma dificuldade valida.".to_string());
    }
    if let Some(age) = input.player_age {
        if !(16..=60).contains(&age) {
            return Err("A idade do piloto deve ficar entre 16 e 60 anos.".to_string());
        }
    }
    Ok(())
}

fn next_career_id(saves_dir: &Path) -> String {
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

fn count_total_races(calendars: &HashMap<String, Vec<CalendarEntry>>) -> usize {
    calendars.values().map(|entries| entries.len()).sum()
}

fn sync_meta_counters(
    conn: &rusqlite::Connection,
    total_drivers: usize,
    total_teams: usize,
    total_contracts: usize,
    total_seasons: usize,
    total_races: usize,
) -> Result<(), crate::db::connection::DbError> {
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
    Ok(())
}

// Internal diagnostic helper kept out of the production Tauri command surface.
#[allow(dead_code)]
pub(crate) fn verify_database(
    app: AppHandle,
    career_number: u32,
) -> Result<VerifyDatabaseResponse, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.career_db_path(career_number);

    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let table_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("Falha ao contar tabelas: {e}"))?;

    Ok(VerifyDatabaseResponse {
        career_number,
        db_path: db_path.to_string_lossy().to_string(),
        table_count,
        status: "ok".to_string(),
    })
}

// Internal diagnostic helper kept out of the production Tauri command surface.
#[allow(dead_code)]
pub(crate) fn test_create_driver(
    app: AppHandle,
    career_number: u32,
    nome: String,
    nacionalidade: String,
    genero: String,
    category_tier: u32,
    difficulty: String,
) -> Result<Driver, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.career_db_path(career_number);
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let id = next_id(&db.conn, IdType::Driver).map_err(|e| format!("Falha ao gerar ID: {e}"))?;

    let mut rng = rand::thread_rng();
    let category_id = match category_tier {
        0 => "mazda_rookie",
        1 => "mazda_amador",
        2 => "bmw_m2",
        3 => "gt4",
        4 => "gt3",
        _ => "endurance",
    };
    let mut existing_names = HashSet::new();
    let mut generated = Driver::generate_for_category(
        category_id,
        category_tier.min(5) as u8,
        &difficulty,
        1,
        &mut existing_names,
        &mut rng,
    );
    let mut driver = generated
        .pop()
        .ok_or_else(|| "Falha ao gerar piloto de teste".to_string())?;
    driver.id = id;
    if !nome.trim().is_empty() {
        driver.nome = nome;
    }
    if !nacionalidade.trim().is_empty() {
        driver.nacionalidade = nacionalidade;
    }
    if !genero.trim().is_empty() {
        driver.genero = genero;
    }

    driver_queries::insert_driver(&db.conn, &driver)
        .map_err(|e| format!("Falha ao inserir piloto: {e}"))?;

    Ok(driver)
}

// Internal diagnostic helper kept out of the production Tauri command surface.
#[allow(dead_code)]
pub(crate) fn test_list_drivers(app: AppHandle, career_number: u32) -> Result<Vec<Driver>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.career_db_path(career_number);
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    driver_queries::get_all_drivers(&db.conn).map_err(|e| format!("Falha ao listar pilotos: {e}"))
}

pub(crate) fn get_driver_in_base_dir(
    base_dir: &Path,
    career_number: u32,
    driver_id: &str,
) -> Result<Driver, String> {
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.career_db_path(career_number);
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    driver_queries::get_driver(&db.conn, driver_id)
        .map_err(|e| format!("Falha ao buscar piloto: {e}"))
}

pub(crate) fn advance_season_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<EndOfSeasonResult, String> {
    let career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let mut config = AppConfig::load_or_default(base_dir);
    let (mut db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    let pending_races = calendar_queries::get_pending_races(&db.conn, &season.id)
        .map_err(|e| format!("Falha ao verificar corridas pendentes: {e}"))?;
    if !pending_races.is_empty() {
        let mut pending_categories: Vec<String> = pending_races
            .iter()
            .map(|race| race.categoria.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        pending_categories.sort();
        return Err(format!(
            "Ainda existem {} corridas pendentes na temporada {} ({})",
            pending_races.len(),
            season.numero,
            pending_categories.join(", ")
        ));
    }

    // O fechamento anual so acontece depois das corridas especiais e do PosEspecial.
    // Assim o mercado normal nunca atropela a convocacao nem o bloco especial.
    match season.fase {
        SeasonPhase::PosEspecial => {}
        SeasonPhase::BlocoRegular => {
            return Err(
                "A temporada regular terminou, mas a janela de convocacao especial ainda precisa ser aberta."
                    .to_string(),
            );
        }
        SeasonPhase::JanelaConvocacao | SeasonPhase::BlocoEspecial => {
            return Err(format!(
                "Nao e possivel avancar a temporada na fase '{}'. Encerre o bloco especial primeiro.",
                season.fase
            ));
        }
    }

    // Backup canônico de fim de temporada — antes de qualquer mutação da próxima.
    // Falha aqui bloqueia o pipeline: melhor abortar do que avançar sem rede de segurança.
    let db_path = career_dir.join("career.db");
    crate::commands::save::backup_season_internal(
        &db_path,
        &career_dir,
        season.numero as u32,
        &meta_path,
    )
    .map_err(|e| format!("Falha ao criar backup de fim de temporada: {e}"))?;

    let result = run_end_of_season(&mut db.conn, &season, &career_dir)?;
    warn_if_noncritical(
        persist_end_of_season_news(&db.conn, &result, season.numero),
        "Falha ao persistir noticias de fim de temporada",
    );
    let total_races = count_season_calendar_entries(&db.conn, &result.new_season_id)
        .map_err(|e| format!("Falha ao contar corridas da nova temporada: {e}"))?;
    let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    meta.current_season = (season.numero + 1).max(1) as u32;
    meta.current_year = result.new_year.max(0) as u32;
    meta.last_played = now;
    meta.total_races = total_races;
    warn_if_noncritical(
        write_save_meta(&meta_path, &meta),
        "Falha ao atualizar meta.json apos avancar temporada",
    );

    config.last_career = Some(career_number);
    warn_if_noncritical(
        config
            .save()
            .map_err(|e| format!("Falha ao atualizar config do app: {e}")),
        "Falha ao atualizar config do app apos avancar temporada",
    );

    warn_if_noncritical(
        write_resume_context(
            &career_dir,
            &CareerResumeContext {
                active_view: CareerResumeView::EndOfSeason,
                end_of_season_result: Some(result.clone()),
            },
        ),
        "Falha ao persistir resume_context apos avancar temporada",
    );

    Ok(result)
}

/// Simula todas as corridas pendentes da temporada sem participação do jogador,
/// conduzindo a temporada por todas as fases: BlocoRegular → JanelaConvocacao →
/// BlocoEspecial → PosEspecial. Após esta função, advance_season pode ser chamado.
/// Usado quando o jogador está sem equipe e quer pular para a próxima pré-temporada.
pub(crate) fn skip_all_pending_races_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<(), String> {
    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;

    // ── Fase 1: BlocoRegular ─────────────────────────────────────────────────
    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::BlocoRegular {
            let pending = calendar_queries::get_pending_races(&db.conn, &season.id)
                .map_err(|e| format!("Falha ao buscar corridas pendentes: {e}"))?;
            for race in &pending {
                crate::commands::race::simulate_category_race(&mut db, race, false)?;
            }
            crate::convocation::advance_to_convocation_window(&db.conn)
                .map_err(|e| format!("Falha ao avancar para janela de convocacao: {e}"))?;
        }
    }

    // ── Fase 2: JanelaConvocacao ─────────────────────────────────────────────
    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::JanelaConvocacao {
            crate::convocation::run_convocation_window(&db.conn)
                .map_err(|e| format!("Falha ao executar janela de convocacao: {e}"))?;
            crate::convocation::iniciar_bloco_especial(&db.conn)
                .map_err(|e| format!("Falha ao iniciar bloco especial: {e}"))?;
        }
    }

    // ── Fase 3: BlocoEspecial ────────────────────────────────────────────────
    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::BlocoEspecial {
            let player = driver_queries::get_player_driver(&db.conn)
                .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
            if player.categoria_especial_ativa.is_some() {
                return Err(
                    "O jogador participa do bloco especial ativo e deve correr essa fase normalmente."
                        .to_string(),
                );
            }

            for category_id in ["production_challenger", "endurance"] {
                let pending = calendar_queries::get_pending_races_for_category(
                    &db.conn,
                    &season.id,
                    category_id,
                )
                .map_err(|e| {
                    format!("Falha ao buscar corridas pendentes de {}: {e}", category_id)
                })?;
                for race in &pending {
                    crate::commands::race::simulate_category_race(&mut db, race, false)?;
                }
            }

            crate::convocation::encerrar_bloco_especial(&db.conn)
                .map_err(|e| format!("Falha ao encerrar bloco especial: {e}"))?;
            crate::convocation::run_pos_especial(&db.conn)
                .map_err(|e| format!("Falha ao executar pos-especial: {e}"))?;
        }
    }

    Ok(())
}

pub(crate) fn advance_market_week_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<WeekResult, String> {
    let _career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;
    let (db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let mut plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao da semana de mercado: {e}"))?;
    let result = advance_week(&tx, &mut plan)?;
    warn_if_noncritical(
        persist_market_week_news(&tx, &plan.state, &result),
        "Falha ao persistir noticias da semana de mercado",
    );
    crate::market::preseason::save_preseason_plan(&career_dir, &plan)?;
    tx.commit()
        .map_err(|e| format!("Falha ao confirmar semana de mercado: {e}"))?;

    meta.last_played = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    warn_if_noncritical(
        write_save_meta(&meta_path, &meta),
        "Falha ao atualizar meta.json apos avancar semana de mercado",
    );
    Ok(result)
}

pub(crate) fn get_preseason_state_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<PreSeasonState, String> {
    let (db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let mut plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada da pre-temporada: {e}"))?
        .ok_or_else(|| format!("Temporada {} nao encontrada", plan.state.season_number))?;
    if season.numero != plan.state.season_number {
        return Err(format!(
            "Plano de pre-temporada desatualizado para a temporada ativa {}.",
            season.numero
        ));
    }
    crate::market::preseason::refresh_preseason_state_display_date(
        &db.conn,
        &season.id,
        &mut plan.state,
    )?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    plan.state.player_has_team =
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .map(|c| c.is_some())
            .unwrap_or(false);
    Ok(plan.state)
}

pub(crate) fn get_player_proposals_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Vec<PlayerProposalView>, String> {
    let (db, _career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let mut proposals =
        market_proposal_queries::get_pending_player_proposals(&db.conn, &season.id, &player.id)
            .map_err(|e| format!("Falha ao buscar propostas pendentes: {e}"))?
            .into_iter()
            .map(|proposal| build_player_proposal_view(&db.conn, &proposal))
            .collect::<Result<Vec<_>, _>>()?;
    proposals.sort_by(|a, b| b.car_performance.total_cmp(&a.car_performance));
    Ok(proposals)
}

pub(crate) fn respond_to_proposal_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    proposal_id: &str,
    accept: bool,
) -> Result<ProposalResponse, String> {
    let (mut db, career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let proposal =
        market_proposal_queries::get_market_proposal_by_id(&db.conn, &season.id, proposal_id)
            .map_err(|e| format!("Falha ao carregar proposta: {e}"))?
            .ok_or_else(|| "Proposta nao encontrada.".to_string())?;
    if proposal.piloto_id != player.id {
        return Err("A proposta nao pertence ao jogador.".to_string());
    }
    if proposal.status != ProposalStatus::Pendente {
        return Err("A proposta nao esta mais pendente.".to_string());
    }

    let mut news_items = Vec::new();
    let mut new_team_name = None;
    let action = if accept { "accepted" } else { "rejected" }.to_string();

    if accept {
        let tx = db
            .conn
            .transaction()
            .map_err(|e| format!("Falha ao iniciar transacao de aceite: {e}"))?;
        accept_player_proposal_tx(&tx, &player, &season, &proposal)?;
        tx.commit()
            .map_err(|e| format!("Falha ao confirmar aceite da proposta: {e}"))?;

        warn_if_noncritical(
            reconcile_plan_after_player_accept(&career_dir, &db.conn, &proposal),
            "Falha ao reconciliar plano apos aceite da proposta",
        );
        new_team_name = Some(proposal.equipe_nome.clone());
    } else {
        let tx = db
            .conn
            .transaction()
            .map_err(|e| format!("Falha ao iniciar transacao de recusa: {e}"))?;
        market_proposal_queries::update_proposal_status(
            &tx,
            &proposal.id,
            "Recusada",
            Some("Jogador recusou a proposta"),
        )
        .map_err(|e| format!("Falha ao recusar proposta: {e}"))?;
        tx.commit()
            .map_err(|e| format!("Falha ao confirmar recusa da proposta: {e}"))?;
    }

    let mut remaining =
        market_proposal_queries::count_pending_player_proposals(&db.conn, &season.id, &player.id)
            .map_err(|e| format!("Falha ao contar propostas pendentes: {e}"))?;

    if !accept && remaining == 0 {
        if contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .map_err(|e| format!("Falha ao verificar equipe regular do jogador: {e}"))?
            .is_none()
        {
            let emergency = generate_emergency_player_proposals(&db.conn, &player, &season)?;
            if emergency.is_empty() {
                if let Some(team_name) =
                    force_place_player(&db.conn, &player, &season, &mut news_items)?
                {
                    new_team_name = Some(team_name);
                }
            } else {
                remaining = emergency.len() as i32;
            }
        }
    }

    warn_if_noncritical(
        sync_preseason_pending_flag(&career_dir, remaining > 0),
        "Falha ao sincronizar indicador de propostas pendentes",
    );
    let headlines = news_items
        .iter()
        .map(|item| item.titulo.clone())
        .collect::<Vec<_>>();

    let message = if accept {
        format!(
            "Voce assinou com {} como {}!",
            proposal.equipe_nome,
            if proposal.papel == TeamRole::Numero1 {
                "N1"
            } else {
                "N2"
            }
        )
    } else if let Some(team_name) = &new_team_name {
        format!(
            "Voce recusou a proposta de {}. O mercado o alocou em {} para evitar que fique sem equipe.",
            proposal.equipe_nome, team_name
        )
    } else if remaining > 0 {
        format!(
            "Voce recusou a proposta de {}. Novas opcoes emergenciais foram geradas.",
            proposal.equipe_nome
        )
    } else {
        format!("Voce recusou a proposta de {}.", proposal.equipe_nome)
    };

    Ok(ProposalResponse {
        success: true,
        action,
        message,
        new_team_name,
        remaining_proposals: remaining,
        news_generated: headlines,
    })
}

pub(crate) fn get_news_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    season: Option<i32>,
    tipo: Option<&str>,
    limit: Option<i32>,
) -> Result<Vec<NewsItem>, String> {
    let (db, _career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    let max_items = limit.unwrap_or(50).clamp(1, 400);
    let query_limit = if tipo.is_some() { 400 } else { max_items };
    let mut items = match season {
        Some(season_number) => {
            news_queries::get_news_by_season(&db.conn, season_number, query_limit)
                .map_err(|e| format!("Falha ao buscar noticias por temporada: {e}"))?
        }
        None => news_queries::get_recent_news(&db.conn, query_limit)
            .map_err(|e| format!("Falha ao buscar noticias recentes: {e}"))?,
    };

    if let Some(tipo) = tipo {
        let tipo_normalizado = NewsType::from_str_strict(tipo)
            .map_err(|e| format!("Filtro de noticia invalido: {e}"))?;
        items.retain(|item| item.tipo == tipo_normalizado);
    }

    items.truncate(max_items as usize);
    Ok(items)
}

pub(crate) fn finalize_preseason_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<(), String> {
    let (db, career_dir, mut meta) = open_career_resources(base_dir, career_id)?;
    let meta_path = career_dir.join("meta.json");
    let plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    if !plan.state.is_complete {
        return Err("Pre-temporada ainda nao foi concluida.".to_string());
    }

    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let pending =
        market_proposal_queries::count_pending_player_proposals(&db.conn, &season.id, &player.id)
            .map_err(|e| format!("Falha ao contar propostas pendentes: {e}"))?;
    if pending > 0 {
        return Err(format!(
            "Voce tem {} proposta(s) pendente(s). Resolva antes de iniciar a temporada.",
            pending
        ));
    }

    let mut rng = rand::thread_rng();

    // 1. Invariante: Garantir que todas as equipes regulares tenham lineup completo antes de iniciar
    fill_all_remaining_vacancies(&db.conn, season.numero, &mut rng)
        .map_err(|e| format!("Falha ao preencher vagas remanescentes: {e}"))?;

    // 1b. Invariante: Garantir que N1/N2 de toda equipe regular está alinhado com o lineup final.
    // Normaliza equipes preenchidas por fallback que não passaram pelo UpdateHierarchy do mercado.
    crate::hierarchy::transition::validate_and_normalize_team_hierarchies(&db.conn)?;

    // 2. Limpar artefatos da corrida anterior (cache do dashboard)
    let results_path = career_dir.join("race_results.json");
    if results_path.exists() {
        let _ = std::fs::remove_file(&results_path);
    }

    delete_preseason_plan(&career_dir)?;
    delete_resume_context(&career_dir)?;
    meta.last_played = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    write_save_meta(&meta_path, &meta)?;

    Ok(())
}

pub(crate) fn get_preseason_free_agents_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Vec<crate::commands::career_types::FreeAgentPreview>, String> {
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let raw = contract_queries::get_free_agents_for_preseason(&db.conn)
        .map_err(|e| format!("Falha ao buscar agentes livres: {e}"))?;

    let result = raw
        .into_iter()
        .map(|r| {
            let abbr = r
                .previous_team_name
                .as_deref()
                .map(|name| name.chars().take(3).collect::<String>().to_uppercase());
            let (license_nivel, license_sigla) = match r.max_license_level {
                Some(0) => ("Rookie", "R"),
                Some(1) => ("Amador", "A"),
                Some(2) => ("Pro", "P"),
                Some(3) => ("Super Pro", "SP"),
                Some(4) => ("Elite", "E"),
                Some(_) => ("Super Elite", "SE"),
                None => ("Rookie", "R"),
            };
            crate::commands::career_types::FreeAgentPreview {
                driver_id: r.driver_id,
                driver_name: r.driver_name,
                categoria: r.categoria,
                is_rookie: r.is_rookie,
                previous_team_name: r.previous_team_name,
                previous_team_color: r.previous_team_color,
                previous_team_abbr: abbr,
                seasons_at_last_team: r.seasons_at_last_team,
                total_career_seasons: r.total_career_seasons,
                license_nivel: license_nivel.to_string(),
                license_sigla: license_sigla.to_string(),
                last_championship_position: r.last_championship_position,
                last_championship_total_drivers: r.last_championship_total_drivers,
            }
        })
        .collect();

    Ok(result)
}

pub(crate) fn get_driver_detail_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    driver_id: &str,
) -> Result<DriverDetail, String> {
    let (db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let driver = driver_queries::get_driver(&db.conn, driver_id)
        .map_err(|e| format!("Falha ao buscar piloto: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let contract = preferred_active_contract_for_phase(&db.conn, driver_id, season.fase)?;
    let team = resolve_driver_team(&db.conn, driver_id, contract.as_ref())?;
    let role = resolve_driver_role(driver_id, contract.as_ref(), team.as_ref());

    build_driver_detail_payload(
        &db.conn,
        &career_dir,
        &season,
        &driver,
        contract.as_ref(),
        team.as_ref(),
        role,
    )
}

fn read_save_meta(path: &Path) -> Result<SaveMeta, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Falha ao ler meta.json: {e}"))?;
    serde_json::from_str::<SaveMeta>(&content)
        .map_err(|e| format!("Falha ao parsear meta.json: {e}"))
}

fn resume_context_path(career_dir: &Path) -> PathBuf {
    career_dir.join("resume_context.json")
}

fn read_resume_context(career_dir: &Path) -> Result<Option<CareerResumeContext>, String> {
    let path = resume_context_path(career_dir);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Falha ao ler resume_context.json: {e}"))?;
    let context = serde_json::from_str::<CareerResumeContext>(&content)
        .map_err(|e| format!("Falha ao parsear resume_context.json: {e}"))?;
    normalize_resume_context(career_dir, context)
}

fn normalize_resume_context(
    career_dir: &Path,
    context: CareerResumeContext,
) -> Result<Option<CareerResumeContext>, String> {
    match context.active_view {
        CareerResumeView::Dashboard => Ok(None),
        CareerResumeView::EndOfSeason => {
            if context.end_of_season_result.is_some() {
                Ok(Some(context))
            } else if load_preseason_plan(career_dir)?.is_some() {
                Ok(Some(CareerResumeContext {
                    active_view: CareerResumeView::Preseason,
                    end_of_season_result: None,
                }))
            } else {
                Ok(None)
            }
        }
        CareerResumeView::Preseason => {
            if load_preseason_plan(career_dir)?.is_some() {
                Ok(Some(CareerResumeContext {
                    active_view: CareerResumeView::Preseason,
                    end_of_season_result: None,
                }))
            } else {
                Ok(None)
            }
        }
    }
}

fn write_resume_context(career_dir: &Path, context: &CareerResumeContext) -> Result<(), String> {
    let path = resume_context_path(career_dir);
    let payload = serde_json::to_string_pretty(context)
        .map_err(|e| format!("Falha ao serializar resume_context.json: {e}"))?;
    std::fs::write(&path, payload).map_err(|e| format!("Falha ao gravar resume_context.json: {e}"))
}

fn delete_resume_context(career_dir: &Path) -> Result<(), String> {
    let path = resume_context_path(career_dir);
    if !path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&path).map_err(|e| format!("Falha ao remover resume_context.json: {e}"))
}

pub(crate) fn persist_resume_context_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    active_view: CareerResumeView,
    end_of_season_result: Option<EndOfSeasonResult>,
) -> Result<(), String> {
    let (_db, career_dir, _) = open_career_resources(base_dir, career_id)?;

    match active_view {
        CareerResumeView::Dashboard => delete_resume_context(&career_dir),
        CareerResumeView::EndOfSeason => {
            let Some(result) = end_of_season_result else {
                return Err(
                    "Estado de fim de temporada requer payload para restauracao.".to_string(),
                );
            };

            write_resume_context(
                &career_dir,
                &CareerResumeContext {
                    active_view,
                    end_of_season_result: Some(result),
                },
            )
        }
        CareerResumeView::Preseason => write_resume_context(
            &career_dir,
            &CareerResumeContext {
                active_view,
                end_of_season_result: None,
            },
        ),
    }
}

pub(crate) fn get_briefing_phrase_history_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<BriefingPhraseHistory, String> {
    let (_db, career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    read_briefing_phrase_history(&briefing_phrase_history_path(&career_dir))
}

pub(crate) fn save_briefing_phrase_history_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    season_number: i32,
    entries: Vec<BriefingPhraseEntryInput>,
) -> Result<BriefingPhraseHistory, String> {
    let (_db, career_dir, _meta) = open_career_resources(base_dir, career_id)?;
    let history_path = briefing_phrase_history_path(&career_dir);
    let current = read_briefing_phrase_history(&history_path)?;
    let updated = merge_briefing_phrase_history(current, season_number, entries);
    write_briefing_phrase_history(&history_path, &updated)?;
    Ok(updated)
}

fn briefing_phrase_history_path(career_dir: &Path) -> PathBuf {
    career_dir.join("briefing_phrase_history.json")
}

fn read_briefing_phrase_history(path: &Path) -> Result<BriefingPhraseHistory, String> {
    if !path.exists() {
        return Ok(BriefingPhraseHistory::default());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Falha ao ler briefing_phrase_history.json: {e}"))?;
    serde_json::from_str::<BriefingPhraseHistory>(&content)
        .map_err(|e| format!("Falha ao parsear briefing_phrase_history.json: {e}"))
}

fn write_briefing_phrase_history(
    path: &Path,
    history: &BriefingPhraseHistory,
) -> Result<(), String> {
    let payload = serde_json::to_string_pretty(history)
        .map_err(|e| format!("Falha ao serializar briefing_phrase_history.json: {e}"))?;
    std::fs::write(path, payload)
        .map_err(|e| format!("Falha ao gravar briefing_phrase_history.json: {e}"))
}

fn merge_briefing_phrase_history(
    current: BriefingPhraseHistory,
    season_number: i32,
    entries: Vec<BriefingPhraseEntryInput>,
) -> BriefingPhraseHistory {
    let mut merged_entries = if current.season_number == season_number {
        current.entries
    } else {
        Vec::new()
    };

    for entry in entries {
        merged_entries.retain(|existing| {
            !(existing.round_number == entry.round_number
                && existing.driver_id == entry.driver_id
                && existing.bucket_key == entry.bucket_key)
        });

        merged_entries.push(BriefingPhraseEntry {
            season_number,
            round_number: entry.round_number,
            driver_id: entry.driver_id,
            bucket_key: entry.bucket_key,
            phrase_id: entry.phrase_id,
        });
    }

    merged_entries.sort_by(|left, right| {
        right
            .round_number
            .cmp(&left.round_number)
            .then_with(|| left.driver_id.cmp(&right.driver_id))
            .then_with(|| left.bucket_key.cmp(&right.bucket_key))
    });

    let mut per_bucket_counts: HashMap<(String, String), usize> = HashMap::new();
    merged_entries.retain(|entry| {
        let key = (entry.driver_id.clone(), entry.bucket_key.clone());
        let count = per_bucket_counts.entry(key).or_insert(0);
        if *count >= 5 {
            return false;
        }
        *count += 1;
        true
    });

    BriefingPhraseHistory {
        season_number,
        entries: merged_entries,
    }
}

fn persist_end_of_season_news(
    _conn: &rusqlite::Connection,
    _result: &EndOfSeasonResult,
    _season_number: i32,
) -> Result<(), String> {
    Ok(())
}

fn persist_market_week_news(
    _conn: &rusqlite::Connection,
    _state: &PreSeasonState,
    _week_result: &WeekResult,
) -> Result<(), String> {
    Ok(())
}

fn build_player_proposal_view(
    conn: &rusqlite::Connection,
    proposal: &MarketProposal,
) -> Result<PlayerProposalView, String> {
    let team = team_queries::get_team_by_id(conn, &proposal.equipe_id)
        .map_err(|e| format!("Falha ao carregar equipe da proposta: {e}"))?
        .ok_or_else(|| "Equipe da proposta nao encontrada.".to_string())?;
    let category = categories::get_category_config(&team.categoria)
        .ok_or_else(|| format!("Categoria '{}' nao encontrada", team.categoria))?;
    let companion_id = match proposal.papel {
        TeamRole::Numero1 => team
            .piloto_2_id
            .clone()
            .or_else(|| team.piloto_1_id.clone()),
        TeamRole::Numero2 => team
            .piloto_1_id
            .clone()
            .or_else(|| team.piloto_2_id.clone()),
    };
    let companion = companion_id
        .as_deref()
        .map(|id| driver_queries::get_driver(conn, id))
        .transpose()
        .map_err(|e| format!("Falha ao carregar companheiro de equipe: {e}"))?;
    Ok(PlayerProposalView {
        proposal_id: proposal.id.clone(),
        equipe_id: team.id.clone(),
        equipe_nome: team.nome.clone(),
        equipe_cor_primaria: team.cor_primaria.clone(),
        equipe_cor_secundaria: team.cor_secundaria.clone(),
        categoria: team.categoria.clone(),
        categoria_nome: category.nome_curto.to_string(),
        categoria_tier: category.tier,
        papel: if proposal.papel == TeamRole::Numero1 {
            "N1".to_string()
        } else {
            "N2".to_string()
        },
        salario_oferecido: proposal.salario_oferecido,
        duracao_anos: proposal.duracao_anos,
        car_performance: team.car_performance,
        car_performance_rating: normalize_car_performance(team.car_performance),
        reputacao: team.reputacao,
        companheiro_nome: companion.as_ref().map(|driver| driver.nome.clone()),
        companheiro_skill: companion
            .as_ref()
            .map(|driver| driver.atributos.skill.round().clamp(0.0, 100.0) as u8),
        status: proposal.status.as_str().to_string(),
    })
}

fn accept_player_proposal_tx(
    tx: &rusqlite::Transaction<'_>,
    player: &Driver,
    season: &Season,
    proposal: &MarketProposal,
) -> Result<(), String> {
    let previous_contract = contract_queries::get_active_regular_contract_for_pilot(tx, &player.id)
        .map_err(|e| format!("Falha ao buscar contrato regular atual do jogador: {e}"))?;
    let previous_team_id = previous_contract
        .as_ref()
        .map(|contract| contract.equipe_id.clone());

    if let Some(contract) = previous_contract {
        contract_queries::update_contract_status(tx, &contract.id, &ContractStatus::Rescindido)
            .map_err(|e| format!("Falha ao rescindir contrato atual: {e}"))?;
        team_queries::remove_pilot_from_team(tx, &player.id, &contract.equipe_id)
            .map_err(|e| format!("Falha ao remover jogador da equipe antiga: {e}"))?;
        refresh_team_hierarchy_now(tx, &contract.equipe_id)?;
    }

    let team = team_queries::get_team_by_id(tx, &proposal.equipe_id)
        .map_err(|e| format!("Falha ao carregar equipe da proposta: {e}"))?
        .ok_or_else(|| "Equipe da proposta nao encontrada.".to_string())?;
    ensure_driver_can_join_category(tx, &player.id, &player.nome, &team.categoria)?;
    let contract = crate::models::contract::Contract::new(
        next_id(tx, IdType::Contract).map_err(|e| format!("Falha ao gerar ID de contrato: {e}"))?,
        player.id.clone(),
        player.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        season.numero,
        proposal.duracao_anos,
        proposal.salario_oferecido,
        proposal.papel.clone(),
        team.categoria.clone(),
    );
    contract_queries::insert_contract(tx, &contract)
        .map_err(|e| format!("Falha ao criar novo contrato do jogador: {e}"))?;
    normalize_regular_contracts_for_team(tx, &team.id)?;
    refresh_team_hierarchy_now(tx, &team.id)?;

    let mut updated_player = player.clone();
    updated_player.categoria_atual = Some(team.categoria.clone());
    updated_player.status = crate::models::enums::DriverStatus::Ativo;
    driver_queries::update_driver(tx, &updated_player)
        .map_err(|e| format!("Falha ao atualizar categoria do jogador: {e}"))?;

    market_proposal_queries::update_proposal_status(tx, &proposal.id, "Aceita", None)
        .map_err(|e| format!("Falha ao marcar proposta como aceita: {e}"))?;
    market_proposal_queries::expire_remaining_proposals(tx, &season.id, &player.id, &proposal.id)
        .map_err(|e| format!("Falha ao expirar demais propostas: {e}"))?;

    if let Some(previous_team_id) = previous_team_id.filter(|old_team| old_team != &team.id) {
        backfill_team_vacancy(tx, &previous_team_id, season.numero)?;
        refresh_team_hierarchy_now(tx, &previous_team_id)?;
    }

    Ok(())
}

fn normalize_regular_contracts_for_team(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<bool, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para normalizar contratos: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para normalizar contratos.".to_string())?;
    let mut active_regular_contracts =
        contract_queries::get_active_contracts_for_team(conn, team_id)
            .map_err(|e| format!("Falha ao carregar contratos ativos da equipe: {e}"))?
            .into_iter()
            .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
            .collect::<Vec<_>>();
    active_regular_contracts.sort_by(|a, b| {
        b.temporada_inicio
            .cmp(&a.temporada_inicio)
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| b.id.cmp(&a.id))
    });

    let mut keep_n1 = None;
    let mut keep_n2 = None;
    let mut displaced_driver_ids = HashSet::new();
    let mut contract_ids_in_slots = HashSet::new();
    let mut role_fixed = false;

    if let Some(expected_n1) = team.piloto_1_id.as_deref() {
        if let Some(contract) = active_regular_contracts
            .iter()
            .find(|contract| contract.piloto_id == expected_n1)
            .cloned()
        {
            contract_ids_in_slots.insert(contract.id.clone());
            keep_n1 = Some(contract);
        }
    }

    if let Some(expected_n2) = team.piloto_2_id.as_deref() {
        if let Some(contract) = active_regular_contracts
            .iter()
            .find(|contract| {
                contract.piloto_id == expected_n2 && !contract_ids_in_slots.contains(&contract.id)
            })
            .cloned()
        {
            contract_ids_in_slots.insert(contract.id.clone());
            keep_n2 = Some(contract);
        }
    }

    for contract in active_regular_contracts {
        if contract_ids_in_slots.contains(&contract.id) {
            continue;
        }
        let slot = match contract.papel {
            TeamRole::Numero1 => &mut keep_n1,
            TeamRole::Numero2 => &mut keep_n2,
        };
        if slot.is_none() {
            *slot = Some(contract);
            continue;
        }

        contract_queries::update_contract_status(conn, &contract.id, &ContractStatus::Rescindido)
            .map_err(|e| {
            format!(
                "Falha ao rescindir contrato regular excedente '{}': {e}",
                contract.id
            )
        })?;
        displaced_driver_ids.insert(contract.piloto_id);
    }

    if let Some(contract) = &keep_n1 {
        if contract.papel != TeamRole::Numero1 {
            conn.execute(
                "UPDATE contracts SET papel = 'Numero1' WHERE id = ?1",
                rusqlite::params![&contract.id],
            )
            .map_err(|e| {
                format!(
                    "Falha ao alinhar papel Numero1 do contrato '{}': {e}",
                    contract.id
                )
            })?;
            role_fixed = true;
        }
    }

    if let Some(contract) = &keep_n2 {
        if contract.papel != TeamRole::Numero2 {
            conn.execute(
                "UPDATE contracts SET papel = 'Numero2' WHERE id = ?1",
                rusqlite::params![&contract.id],
            )
            .map_err(|e| {
                format!(
                    "Falha ao alinhar papel Numero2 do contrato '{}': {e}",
                    contract.id
                )
            })?;
            role_fixed = true;
        }
    }

    let piloto_1 = keep_n1.as_ref().map(|contract| contract.piloto_id.as_str());
    let piloto_2 = keep_n2.as_ref().map(|contract| contract.piloto_id.as_str());
    let changed = team.piloto_1_id.as_deref() != piloto_1
        || team.piloto_2_id.as_deref() != piloto_2
        || !displaced_driver_ids.is_empty()
        || role_fixed;

    if team.piloto_1_id.as_deref() != piloto_1 || team.piloto_2_id.as_deref() != piloto_2 {
        team_queries::update_team_pilots(conn, team_id, piloto_1, piloto_2)
            .map_err(|e| format!("Falha ao atualizar lineup da equipe '{}': {e}", team.nome))?;
    }

    for driver_id in displaced_driver_ids {
        if contract_queries::get_active_contract_for_pilot(conn, &driver_id)
            .map_err(|e| {
                format!(
                    "Falha ao verificar contrato remanescente de '{}': {e}",
                    driver_id
                )
            })?
            .is_some()
        {
            continue;
        }
        let mut driver = driver_queries::get_driver(conn, &driver_id)
            .map_err(|e| format!("Falha ao carregar piloto deslocado '{}': {e}", driver_id))?;
        if driver.categoria_atual.is_none() {
            continue;
        }
        driver.categoria_atual = None;
        driver_queries::update_driver(conn, &driver).map_err(|e| {
            format!(
                "Falha ao limpar categoria do piloto deslocado '{}': {e}",
                driver_id
            )
        })?;
    }

    Ok(changed)
}

fn place_driver_in_team(
    conn: &rusqlite::Connection,
    team_id: &str,
    driver_id: &str,
    role: TeamRole,
) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para encaixar jogador: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para encaixe do jogador.".to_string())?;
    let existing = [team.piloto_1_id.clone(), team.piloto_2_id.clone()]
        .into_iter()
        .flatten()
        .filter(|id| id != driver_id)
        .collect::<Vec<_>>();
    let (piloto_1, piloto_2) = match role {
        TeamRole::Numero1 => (Some(driver_id.to_string()), existing.first().cloned()),
        TeamRole::Numero2 => (existing.first().cloned(), Some(driver_id.to_string())),
    };
    team_queries::update_team_pilots(conn, team_id, piloto_1.as_deref(), piloto_2.as_deref())
        .map_err(|e| format!("Falha ao atualizar pilotos da nova equipe: {e}"))?;
    Ok(())
}

fn refresh_team_hierarchy_now(conn: &rusqlite::Connection, team_id: &str) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para hierarquia: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para hierarquia.".to_string())?;
    let mut candidates = [team.piloto_1_id.clone(), team.piloto_2_id.clone()]
        .into_iter()
        .flatten()
        .filter_map(|id| driver_queries::get_driver(conn, &id).ok())
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.atributos.skill.total_cmp(&a.atributos.skill));
    let n1_id = candidates.first().map(|driver| driver.id.as_str());
    let n2_id = candidates.get(1).map(|driver| driver.id.as_str());
    team_queries::update_team_hierarchy(
        conn,
        team_id,
        n1_id,
        n2_id,
        TeamHierarchyClimate::Estavel.as_str(),
        0.0,
    )
    .map_err(|e| format!("Falha ao atualizar hierarquia da equipe: {e}"))?;
    Ok(())
}

#[derive(Clone)]
struct TeamVacancy {
    team: Team,
    role: TeamRole,
}

fn list_team_vacancies(conn: &rusqlite::Connection) -> Result<Vec<TeamVacancy>, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao listar equipes: {e}"))?;
    let mut vacancies = Vec::new();
    for team in teams {
        if team.piloto_1_id.is_none() {
            vacancies.push(TeamVacancy {
                team: team.clone(),
                role: TeamRole::Numero1,
            });
        }
        if team.piloto_2_id.is_none() {
            vacancies.push(TeamVacancy {
                team,
                role: TeamRole::Numero2,
            });
        }
    }
    Ok(vacancies)
}

fn generate_emergency_player_proposals(
    conn: &rusqlite::Connection,
    player: &Driver,
    season: &Season,
) -> Result<Vec<MarketProposal>, String> {
    let player_tier = player
        .categoria_atual
        .as_deref()
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
        .unwrap_or(0);
    let mut vacancies = Vec::new();
    for vacancy in list_team_vacancies(conn)? {
        let tier = categories::get_category_config(&vacancy.team.categoria)
            .map(|config| config.tier)
            .unwrap_or(0);
        let tier_ok = tier >= player_tier && tier <= player_tier + 1;
        if tier_ok
            && driver_has_required_license_for_category(conn, &player.id, &vacancy.team.categoria)?
        {
            vacancies.push(vacancy);
        }
    }
    if vacancies.is_empty() {
        for vacancy in list_team_vacancies(conn)? {
            if driver_has_required_license_for_category(conn, &player.id, &vacancy.team.categoria)?
            {
                vacancies.push(vacancy);
            }
        }
    }
    vacancies.sort_by(|a, b| b.team.car_performance.total_cmp(&a.team.car_performance));

    let mut created = Vec::new();
    for (index, vacancy) in vacancies.into_iter().take(2).enumerate() {
        let proposal = MarketProposal {
            id: format!(
                "MP-{}-{}-{}-EM-{}",
                season.numero, vacancy.team.id, player.id, index
            ),
            equipe_id: vacancy.team.id.clone(),
            equipe_nome: vacancy.team.nome.clone(),
            piloto_id: player.id.clone(),
            piloto_nome: player.nome.clone(),
            categoria: vacancy.team.categoria.clone(),
            papel: vacancy.role.clone(),
            salario_oferecido: calculate_offer_salary_for_team(&vacancy.team, player),
            duracao_anos: if categories::get_category_config(&vacancy.team.categoria)
                .map(|config| config.tier >= 3)
                .unwrap_or(false)
            {
                2
            } else {
                1
            },
            status: ProposalStatus::Pendente,
            motivo_recusa: None,
        };
        market_proposal_queries::insert_player_proposal(conn, &season.id, &proposal)
            .map_err(|e| format!("Falha ao persistir proposta emergencial: {e}"))?;
        created.push(proposal);
    }

    Ok(created)
}

fn force_place_player(
    conn: &rusqlite::Connection,
    player: &Driver,
    season: &Season,
    _news_items: &mut Vec<NewsItem>,
) -> Result<Option<String>, String> {
    let player_tier = player
        .categoria_atual
        .as_deref()
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
        .unwrap_or(0);
    let mut vacancies = Vec::new();
    for vacancy in list_team_vacancies(conn)? {
        let tier_ok = categories::get_category_config(&vacancy.team.categoria)
            .map(|config| config.tier == player_tier)
            .unwrap_or(false);
        if tier_ok
            && driver_has_required_license_for_category(conn, &player.id, &vacancy.team.categoria)?
        {
            vacancies.push(vacancy);
        }
    }
    if vacancies.is_empty() {
        for vacancy in list_team_vacancies(conn)? {
            if driver_has_required_license_for_category(conn, &player.id, &vacancy.team.categoria)?
            {
                vacancies.push(vacancy);
            }
        }
    }
    vacancies.sort_by(|a, b| a.team.car_performance.total_cmp(&b.team.car_performance));
    let Some(vacancy) = vacancies.into_iter().next() else {
        return Ok(None);
    };
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao de alocacao forcada: {e}"))?;
    ensure_driver_can_join_category(&tx, &player.id, &player.nome, &vacancy.team.categoria)?;

    let contract = crate::models::contract::Contract::new(
        next_id(&tx, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar contrato forçado: {e}"))?,
        player.id.clone(),
        player.nome.clone(),
        vacancy.team.id.clone(),
        vacancy.team.nome.clone(),
        season.numero,
        1,
        calculate_offer_salary_for_team(&vacancy.team, player).max(5_000.0),
        vacancy.role.clone(),
        vacancy.team.categoria.clone(),
    );
    contract_queries::insert_contract(&tx, &contract)
        .map_err(|e| format!("Falha ao inserir contrato forçado: {e}"))?;
    place_driver_in_team(&tx, &vacancy.team.id, &player.id, vacancy.role.clone())?;
    refresh_team_hierarchy_now(&tx, &vacancy.team.id)?;
    let mut updated_player = player.clone();
    updated_player.categoria_atual = Some(vacancy.team.categoria.clone());
    updated_player.status = crate::models::enums::DriverStatus::Ativo;
    driver_queries::update_driver(&tx, &updated_player)
        .map_err(|e| format!("Falha ao atualizar jogador apos alocacao forcada: {e}"))?;
    tx.commit()
        .map_err(|e| format!("Falha ao confirmar alocacao forcada: {e}"))?;
    Ok(Some(vacancy.team.nome))
}

fn backfill_team_vacancy(
    conn: &rusqlite::Connection,
    team_id: &str,
    season_number: i32,
) -> Result<(), String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para reposicao: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para reposicao.".to_string())?;
    let role = if team.piloto_1_id.is_none() {
        TeamRole::Numero1
    } else if team.piloto_2_id.is_none() {
        TeamRole::Numero2
    } else {
        return Ok(());
    };

    let free_driver = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para reposicao: {e}"))?
        .into_iter()
        .filter(|driver| driver.status == crate::models::enums::DriverStatus::Ativo)
        .filter(|driver| {
            contract_queries::get_active_regular_contract_for_pilot(conn, &driver.id)
                .ok()
                .flatten()
                .is_none()
        })
        .filter(|driver| {
            driver_has_required_license_for_category(conn, &driver.id, &team.categoria)
                .unwrap_or(false)
        })
        .max_by(|a, b| a.atributos.skill.total_cmp(&b.atributos.skill));

    let replacement = if let Some(driver) = free_driver {
        driver
    } else {
        let mut existing_names = driver_queries::get_all_drivers(conn)
            .map_err(|e| format!("Falha ao carregar nomes existentes: {e}"))?
            .into_iter()
            .map(|driver| driver.nome)
            .collect::<HashSet<_>>();
        let mut rng = rand::thread_rng();
        let mut rookie =
            crate::evolution::rookies::generate_rookies(1, &mut existing_names, &mut rng)
                .into_iter()
                .next()
                .ok_or_else(|| "Falha ao gerar rookie emergencial.".to_string())?;
        rookie.id = format!(
            "P-EM-{}",
            next_id(conn, IdType::Driver)
                .map_err(|e| format!("Falha ao gerar ID emergencial: {e}"))?
        );
        driver_queries::insert_driver(conn, &rookie)
            .map_err(|e| format!("Falha ao inserir rookie emergencial: {e}"))?;
        grant_driver_license_for_category_if_needed(conn, &rookie.id, &team.categoria)?;
        rookie
    };
    ensure_driver_can_join_category(conn, &replacement.id, &replacement.nome, &team.categoria)?;

    let contract = crate::models::contract::Contract::new(
        next_id(conn, IdType::Contract)
            .map_err(|e| format!("Falha ao gerar contrato de reposicao: {e}"))?,
        replacement.id.clone(),
        replacement.nome.clone(),
        team.id.clone(),
        team.nome.clone(),
        season_number,
        1,
        calculate_offer_salary_for_team(&team, &replacement).max(5_000.0),
        role.clone(),
        team.categoria.clone(),
    );
    contract_queries::insert_contract(conn, &contract)
        .map_err(|e| format!("Falha ao inserir contrato de reposicao: {e}"))?;
    place_driver_in_team(conn, &team.id, &replacement.id, role)?;
    let mut updated_driver = replacement.clone();
    updated_driver.categoria_atual = Some(team.categoria.clone());
    driver_queries::update_driver(conn, &updated_driver)
        .map_err(|e| format!("Falha ao atualizar piloto de reposicao: {e}"))?;
    Ok(())
}

fn calculate_offer_salary_for_team(team: &Team, player: &Driver) -> f64 {
    calculate_offer_salary_from_money(team, player.atributos.skill)
}

fn normalize_car_performance(car_performance: f64) -> u8 {
    (((car_performance + 5.0) / 21.0) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8
}

fn pending_player_event_team_ids(event: &PendingAction, player_id: &str) -> Option<Vec<String>> {
    match event {
        PendingAction::ExpireContract {
            driver_id, team_id, ..
        } if driver_id == player_id => Some(vec![team_id.clone()]),
        PendingAction::RenewContract {
            driver_id, team_id, ..
        } if driver_id == player_id => Some(vec![team_id.clone()]),
        PendingAction::Transfer {
            driver_id,
            from_team_id,
            to_team_id,
            ..
        } if driver_id == player_id => {
            let mut team_ids = Vec::new();
            if let Some(from_team_id) = from_team_id {
                team_ids.push(from_team_id.clone());
            }
            team_ids.push(to_team_id.clone());
            Some(team_ids)
        }
        PendingAction::PlayerProposal { proposal } if proposal.piloto_id == player_id => {
            Some(vec![proposal.equipe_id.clone()])
        }
        PendingAction::PlaceRookie {
            driver, team_id, ..
        } if driver.id == player_id => Some(vec![team_id.clone()]),
        _ => None,
    }
}

fn is_team_role_vacant(
    conn: &rusqlite::Connection,
    team_id: &str,
    role: &str,
) -> Result<bool, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para validar vaga: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para validar vaga.".to_string())?;
    let is_vacant = match TeamRole::from_str_strict(role)
        .map_err(|e| format!("Papel de equipe invalido ao validar vaga: {e}"))?
    {
        TeamRole::Numero1 => team.piloto_1_id.is_none(),
        TeamRole::Numero2 => team.piloto_2_id.is_none(),
    };
    Ok(is_vacant)
}

fn reconcile_plan_after_player_accept(
    career_dir: &Path,
    conn: &rusqlite::Connection,
    proposal: &MarketProposal,
) -> Result<(), String> {
    let Some(mut plan) = load_preseason_plan(career_dir)? else {
        return Ok(());
    };
    let mut affected_team_ids = HashSet::from([proposal.equipe_id.clone()]);
    plan.planned_events.retain(|event| {
        if event.executed {
            return true;
        }
        if let Some(team_ids) = pending_player_event_team_ids(&event.event, &proposal.piloto_id) {
            affected_team_ids.extend(team_ids);
            return false;
        }
        true
    });

    let stale_rookie_indices = plan
        .planned_events
        .iter()
        .enumerate()
        .filter(|(_, event)| !event.executed)
        .filter_map(|(index, event)| match &event.event {
            PendingAction::PlaceRookie { team_id, role, .. }
                if affected_team_ids.contains(team_id) =>
            {
                Some(
                    is_team_role_vacant(conn, team_id, role)
                        .map(|is_vacant| (!is_vacant).then_some(index)),
                )
            }
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    for index in stale_rookie_indices.into_iter().rev() {
        plan.planned_events.remove(index);
    }
    for team_id in affected_team_ids {
        refresh_planned_hierarchy_for_team(&mut plan, conn, &team_id)?;
    }
    plan.state.player_has_pending_proposals = false;
    save_preseason_plan(career_dir, &plan)
}

fn sync_preseason_pending_flag(career_dir: &Path, has_pending: bool) -> Result<(), String> {
    let Some(mut plan) = load_preseason_plan(career_dir)? else {
        return Ok(());
    };
    plan.state.player_has_pending_proposals = has_pending;
    save_preseason_plan(career_dir, &plan)
}

fn refresh_planned_hierarchy_for_team(
    plan: &mut PreSeasonPlan,
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<(), String> {
    let hierarchy_week = plan
        .planned_events
        .iter()
        .filter_map(|event| match &event.event {
            PendingAction::UpdateHierarchy {
                team_id: current, ..
            } if current == team_id => Some(event.week),
            PendingAction::UpdateHierarchy { .. } => Some(event.week),
            _ => None,
        })
        .max()
        .unwrap_or(plan.state.total_weeks);
    plan.planned_events.retain(|event| {
        !(!event.executed
            && matches!(&event.event, PendingAction::UpdateHierarchy { team_id: current, .. } if current == team_id))
    });

    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar equipe para atualizar plano: {e}"))?
        .ok_or_else(|| "Equipe nao encontrada para atualizar plano.".to_string())?;
    let mut candidates = Vec::new();
    for driver_id in [team.piloto_1_id.clone(), team.piloto_2_id.clone()]
        .into_iter()
        .flatten()
    {
        let driver = driver_queries::get_driver(conn, &driver_id)
            .map_err(|e| format!("Falha ao carregar piloto da equipe para plano: {e}"))?;
        candidates.push((driver.id, driver.nome, driver.atributos.skill));
    }
    for event in plan.planned_events.iter() {
        if event.executed {
            continue;
        }
        if let PendingAction::PlaceRookie {
            driver,
            team_id: current,
            ..
        } = &event.event
        {
            if current == team_id {
                candidates.push((
                    driver.id.clone(),
                    driver.nome.clone(),
                    driver.atributos.skill,
                ));
            }
        }
    }
    candidates.sort_by(|a, b| b.2.total_cmp(&a.2));
    candidates.dedup_by(|a, b| a.0 == b.0);
    let n1 = candidates.first().cloned();
    let n2 = candidates.get(1).cloned();
    plan.planned_events.push(PlannedEvent {
        week: hierarchy_week,
        executed: false,
        event: PendingAction::UpdateHierarchy {
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            n1_id: n1.as_ref().map(|candidate| candidate.0.clone()),
            n1_name: n1
                .as_ref()
                .map(|candidate| candidate.1.clone())
                .unwrap_or_else(|| "Sem piloto".to_string()),
            n2_id: n2.as_ref().map(|candidate| candidate.0.clone()),
            n2_name: n2
                .as_ref()
                .map(|candidate| candidate.1.clone())
                .unwrap_or_else(|| "Sem piloto".to_string()),
            prev_n1_id: team.hierarquia_n1_id.clone(),
            prev_n2_id: team.hierarquia_n2_id.clone(),
            prev_tensao: team.hierarquia_tensao,
            prev_status: team.hierarquia_status.clone(),
            prev_categoria: team.categoria.clone(),
        },
    });
    Ok(())
}

fn open_career_resources(
    base_dir: &Path,
    career_id: &str,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    open_career_resources_with_repair(base_dir, career_id, true)
}

fn open_career_resources_read_only(
    base_dir: &Path,
    career_id: &str,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    open_career_resources_with_repair(base_dir, career_id, false)
}

fn open_career_resources_with_repair(
    base_dir: &Path,
    career_id: &str,
    repair_contracts: bool,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    let _career_number =
        career_number_from_id(career_id).ok_or_else(|| "ID de carreira invalido.".to_string())?;

    let config = AppConfig::load_or_default(base_dir);
    let career_dir = config.saves_dir().join(career_id);
    let db_path = career_dir.join("career.db");
    let meta_path = career_dir.join("meta.json");

    if !career_dir.exists() {
        return Err("Save nao encontrado.".to_string());
    }
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }

    let preseason_active = load_preseason_plan(&career_dir)?.is_some();
    let db = if repair_contracts {
        let _repair_guard = match CAREER_OPEN_REPAIR_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
        {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let db = Database::open_existing(&db_path)
            .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
        repair_regular_contract_consistency(&db.conn, !preseason_active)?;
        db
    } else {
        Database::open_existing(&db_path)
            .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?
    };
    let meta = read_save_meta(&meta_path)?;

    Ok((db, career_dir, meta))
}

fn repair_regular_contract_consistency(
    conn: &rusqlite::Connection,
    allow_regular_vacancy_fill: bool,
) -> Result<(), String> {
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|e| format!("Falha ao iniciar reparo de contratos: {e}"))?;
    let mut affected_team_ids = HashSet::new();
    let active_regular_contracts = contract_queries::get_all_active_regular_contracts(&tx)
        .map_err(|e| format!("Falha ao carregar contratos regulares ativos: {e}"))?;
    let mut contracts_by_pilot = HashMap::<String, Vec<_>>::new();

    for contract in active_regular_contracts {
        contracts_by_pilot
            .entry(contract.piloto_id.clone())
            .or_default()
            .push(contract);
    }

    for contracts in contracts_by_pilot.values_mut() {
        if contracts.len() <= 1 {
            continue;
        }

        contracts.sort_by(|a, b| {
            b.temporada_inicio
                .cmp(&a.temporada_inicio)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| b.id.cmp(&a.id))
        });

        for duplicate in contracts.iter().skip(1) {
            contract_queries::update_contract_status(
                &tx,
                &duplicate.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular duplicado '{}': {e}",
                    duplicate.id
                )
            })?;
            affected_team_ids.insert(duplicate.equipe_id.clone());
        }

        if let Some(kept) = contracts.first() {
            affected_team_ids.insert(kept.equipe_id.clone());
        }
    }

    let teams =
        team_queries::get_all_teams(&tx).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let teams_by_id = teams
        .iter()
        .map(|team| (team.id.clone(), team.clone()))
        .collect::<HashMap<_, _>>();
    let drivers = driver_queries::get_all_drivers(&tx)
        .map_err(|e| format!("Falha ao carregar pilotos para reparo: {e}"))?;
    let drivers_by_id = drivers
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect::<HashMap<_, _>>();
    let active_regular_contracts = contract_queries::get_all_active_regular_contracts(&tx)
        .map_err(|e| format!("Falha ao recarregar contratos regulares ativos: {e}"))?;
    for contract in active_regular_contracts {
        if categories::is_especial(&contract.categoria) {
            contract_queries::update_contract_status(
                &tx,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular especial '{}': {e}",
                    contract.id
                )
            })?;
            affected_team_ids.insert(contract.equipe_id.clone());
            continue;
        }

        let Some(team) = teams_by_id.get(&contract.equipe_id) else {
            continue;
        };
        if categories::is_especial(&team.categoria) {
            contract_queries::update_contract_status(
                &tx,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular em equipe especial '{}': {e}",
                    contract.id
                )
            })?;
            affected_team_ids.insert(contract.equipe_id.clone());
            continue;
        }

        let Some(driver) = drivers_by_id.get(&contract.piloto_id) else {
            continue;
        };
        if driver.status == DriverStatus::Aposentado {
            contract_queries::update_contract_status(
                &tx,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular invalido '{}': {e}",
                    contract.id
                )
            })?;
            affected_team_ids.insert(contract.equipe_id.clone());
            continue;
        }

        if driver.categoria_atual.as_deref() != Some(team.categoria.as_str()) {
            let mut updated_driver = driver.clone();
            updated_driver.categoria_atual = Some(team.categoria.clone());
            driver_queries::update_driver(&tx, &updated_driver).map_err(|e| {
                format!("Falha ao corrigir categoria do piloto '{}': {e}", driver.id)
            })?;
        }
    }

    for team in teams
        .iter()
        .filter(|team| !categories::is_especial(&team.categoria))
    {
        if normalize_regular_contracts_for_team(&tx, &team.id)? {
            affected_team_ids.insert(team.id.clone());
        }
    }

    for team_id in affected_team_ids {
        refresh_team_hierarchy_now(&tx, &team_id)?;
    }

    tx.execute(
        "UPDATE drivers SET categoria_atual = NULL
         WHERE categoria_atual IS NOT NULL
           AND id NOT IN (SELECT piloto_id FROM contracts WHERE status = 'Ativo')",
        [],
    )
    .map_err(|e| format!("Falha ao limpar categoria_atual de pilotos sem contrato: {e}"))?;

    tx.commit()
        .map_err(|e| format!("Falha ao concluir reparo de contratos: {e}"))?;
    if allow_regular_vacancy_fill {
        if let Some(active_season) = season_queries::get_active_season(conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa para reparo de vagas: {e}"))?
        {
            let pending_regular_races = calendar_queries::count_pending_races_in_phase(
                conn,
                &active_season.id,
                &SeasonPhase::BlocoRegular,
            )
            .map_err(|e| format!("Falha ao contar corridas regulares pendentes: {e}"))?;
            if active_season.fase == SeasonPhase::BlocoRegular && pending_regular_races > 0 {
                let mut rng = rand::thread_rng();
                fill_all_remaining_vacancies(conn, active_season.numero, &mut rng)
                    .map_err(|e| format!("Falha ao preencher vagas regulares pendentes: {e}"))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn get_drivers_by_category_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<DriverSummary>, String> {
    let category = category.trim().to_lowercase();
    let (db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let total_rounds = count_calendar_entries(&db.conn, &season.id, &category)
        .map_err(|e| format!("Falha ao contar corridas da categoria: {e}"))?
        as usize;

    if categories::is_especial(&category) {
        let special_standings = get_special_driver_standings_from_results(
            &db,
            &career_dir,
            &season,
            &category,
            total_rounds,
        )?;
        if !special_standings.is_empty() {
            return Ok(special_standings);
        }
    }

    let mut drivers = driver_queries::get_drivers_by_category(&db.conn, &category)
        .map_err(|e| format!("Falha ao buscar pilotos da categoria: {e}"))?;
    let participant_ids = get_regular_standings_participant_ids(&db.conn, &season.id, &category)?;
    if !participant_ids.is_empty() {
        drivers.retain(|driver| participant_ids.contains(&driver.id));
    }
    let driver_ids: Vec<String> = drivers.iter().map(|driver| driver.id.clone()).collect();
    let active_injuries_by_driver =
        injury_queries::get_active_injury_types_by_pilot(&db.conn, &driver_ids)
            .map_err(|e| format!("Falha ao buscar lesoes ativas dos pilotos: {e}"))?;
    let history_map: HashMap<String, Vec<Option<RoundResult>>> =
        build_driver_histories(&career_dir, &category, total_rounds, &driver_ids)?
            .into_iter()
            .map(|history| (history.driver_id, history.results))
            .collect();

    let mut standings: Vec<DriverSummary> = drivers
        .into_iter()
        .map(|driver| {
            let driver_id = driver.id.clone();
            let team = find_player_team(&db.conn, &driver.id, season.fase)
                .ok()
                .flatten();
            DriverSummary {
                id: driver_id.clone(),
                nome: driver.nome,
                nacionalidade: driver.nacionalidade,
                idade: driver.idade as i32,
                skill: driver.atributos.skill.round().clamp(0.0, 100.0) as u8,
                categoria_especial_ativa: driver.categoria_especial_ativa.clone(),
                equipe_id: team.as_ref().map(|value| value.id.clone()),
                equipe_nome: team.as_ref().map(|value| value.nome.clone()),
                equipe_nome_curto: team.as_ref().map(|value| value.nome_curto.clone()),
                equipe_cor: team
                    .as_ref()
                    .map(|value| value.cor_primaria.clone())
                    .unwrap_or_else(|| "#7d8590".to_string()),
                classe: team.as_ref().and_then(|value| value.classe.clone()),
                is_jogador: driver.is_jogador,
                is_estreante: driver.temporadas_na_categoria == 0,
                is_estreante_da_vida: driver.stats_carreira.corridas == 0,
                lesao_ativa_tipo: active_injuries_by_driver.get(&driver_id).cloned(),
                pontos: driver.stats_temporada.pontos.round() as i32,
                vitorias: driver.stats_temporada.vitorias as i32,
                podios: driver.stats_temporada.podios as i32,
                posicao_campeonato: 0,
                results: merge_recent_results_fallback(
                    history_map.get(&driver_id).cloned().unwrap_or_default(),
                    &driver.ultimos_resultados,
                    total_rounds,
                    driver.stats_temporada.corridas as usize,
                ),
            }
        })
        .collect();

    standings.sort_by(|a, b| {
        b.pontos
            .cmp(&a.pontos)
            .then_with(|| b.vitorias.cmp(&a.vitorias))
            .then_with(|| b.podios.cmp(&a.podios))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    for (index, driver) in standings.iter_mut().enumerate() {
        driver.posicao_campeonato = index as i32 + 1;
    }

    Ok(standings)
}

fn get_regular_standings_participant_ids(
    conn: &rusqlite::Connection,
    season_id: &str,
    category: &str,
) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT rr.piloto_id
             FROM race_results rr
             INNER JOIN calendar c ON c.id = rr.race_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
               AND c.categoria = ?2",
        )
        .map_err(|e| format!("Falha ao preparar participantes da classificacao: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![season_id, category], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| format!("Falha ao buscar participantes da classificacao: {e}"))?;

    let mut participant_ids = HashSet::new();
    for row in rows {
        participant_ids
            .insert(row.map_err(|e| format!("Falha ao ler participante da classificacao: {e}"))?);
    }

    Ok(participant_ids)
}

struct HistoricalSpecialStanding {
    driver_id: String,
    points: f64,
    wins: i32,
    podiums: i32,
    latest_team_id: Option<String>,
    latest_class_name: Option<String>,
}

struct HistoricalSpecialTeamStanding {
    team_id: String,
    points: f64,
    wins: i32,
    class_name: Option<String>,
}

fn get_special_driver_standings_from_results(
    db: &Database,
    career_dir: &Path,
    season: &Season,
    category: &str,
    total_rounds: usize,
) -> Result<Vec<DriverSummary>, String> {
    let rows = query_special_driver_standing_rows(&db.conn, &season.id, category)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let driver_ids: Vec<String> = rows.iter().map(|row| row.driver_id.clone()).collect();
    let active_injuries_by_driver =
        injury_queries::get_active_injury_types_by_pilot(&db.conn, &driver_ids)
            .map_err(|e| format!("Falha ao buscar lesoes ativas dos pilotos especiais: {e}"))?;
    let history_map: HashMap<String, Vec<Option<RoundResult>>> =
        build_driver_histories(career_dir, category, total_rounds, &driver_ids)?
            .into_iter()
            .map(|history| (history.driver_id, history.results))
            .collect();

    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let driver = driver_queries::get_driver(&db.conn, &row.driver_id).map_err(|e| {
                format!("Falha ao carregar piloto especial '{}': {e}", row.driver_id)
            })?;
            let team = row
                .latest_team_id
                .as_deref()
                .map(|team_id| {
                    team_queries::get_team_by_id(&db.conn, team_id).map_err(|e| {
                        format!("Falha ao carregar equipe especial '{}': {e}", team_id)
                    })
                })
                .transpose()?
                .flatten();

            Ok(DriverSummary {
                id: driver.id.clone(),
                nome: driver.nome,
                nacionalidade: driver.nacionalidade,
                idade: driver.idade as i32,
                skill: driver.atributos.skill.round().clamp(0.0, 100.0) as u8,
                categoria_especial_ativa: driver.categoria_especial_ativa.clone(),
                equipe_id: team.as_ref().map(|value| value.id.clone()),
                equipe_nome: team.as_ref().map(|value| value.nome.clone()),
                equipe_nome_curto: team.as_ref().map(|value| value.nome_curto.clone()),
                equipe_cor: team
                    .as_ref()
                    .map(|value| value.cor_primaria.clone())
                    .unwrap_or_else(|| "#7d8590".to_string()),
                classe: row
                    .latest_class_name
                    .clone()
                    .or_else(|| team.as_ref().and_then(|value| value.classe.clone())),
                is_jogador: driver.is_jogador,
                is_estreante: driver.temporadas_na_categoria == 0,
                is_estreante_da_vida: driver.stats_carreira.corridas == 0,
                lesao_ativa_tipo: active_injuries_by_driver.get(&driver.id).cloned(),
                pontos: row.points.round() as i32,
                vitorias: row.wins,
                podios: row.podiums,
                posicao_campeonato: index as i32 + 1,
                results: history_map.get(&driver.id).cloned().unwrap_or_default(),
            })
        })
        .collect()
}

fn query_special_driver_standing_rows(
    conn: &rusqlite::Connection,
    season_id: &str,
    category: &str,
) -> Result<Vec<HistoricalSpecialStanding>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                r.piloto_id,
                COALESCE(SUM(r.pontos), 0.0) AS total_points,
                SUM(CASE WHEN r.posicao_final = 1 AND r.dnf = 0 THEN 1 ELSE 0 END) AS total_wins,
                SUM(CASE WHEN r.posicao_final <= 3 AND r.dnf = 0 THEN 1 ELSE 0 END) AS total_podiums,
                (
                    SELECT rr.equipe_id
                    FROM race_results rr
                    INNER JOIN calendar cc ON cc.id = rr.race_id
                    WHERE rr.piloto_id = r.piloto_id
                      AND COALESCE(cc.season_id, cc.temporada_id) = ?1
                      AND cc.categoria = ?2
                    ORDER BY cc.rodada DESC, rr.id DESC
                    LIMIT 1
                ) AS latest_team_id,
                MAX(e.class_name) AS latest_class_name
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             INNER JOIN drivers d ON d.id = r.piloto_id
             LEFT JOIN special_team_entries e
               ON e.season_id = COALESCE(c.season_id, c.temporada_id)
              AND e.special_category = c.categoria
              AND e.team_id = r.equipe_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
               AND c.categoria = ?2
             GROUP BY r.piloto_id
             ORDER BY total_points DESC, total_wins DESC, total_podiums DESC, d.nome ASC",
        )
        .map_err(|e| format!("Falha ao preparar standings especiais: {e}"))?;

    let mapped = stmt
        .query_map(rusqlite::params![season_id, category], |row| {
            Ok(HistoricalSpecialStanding {
                driver_id: row.get(0)?,
                points: row.get(1)?,
                wins: row.get(2)?,
                podiums: row.get(3)?,
                latest_team_id: row.get(4)?,
                latest_class_name: row.get(5)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar standings especiais: {e}"))?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row.map_err(|e| format!("Falha ao ler standings especiais: {e}"))?);
    }
    Ok(rows)
}

fn merge_recent_results_fallback(
    history: Vec<Option<RoundResult>>,
    recent_results: &serde_json::Value,
    total_rounds: usize,
    raced_rounds: usize,
) -> Vec<Option<RoundResult>> {
    if history.iter().any(Option::is_some) {
        return history;
    }

    let fallback_results = parse_recent_results_json(recent_results);
    if fallback_results.is_empty() {
        return history;
    }

    let normalized_len = total_rounds.max(fallback_results.len());
    let mut merged = vec![None; normalized_len];
    let end_index = raced_rounds.min(normalized_len).max(fallback_results.len());
    let start_index = end_index.saturating_sub(fallback_results.len());

    for (offset, result) in fallback_results.into_iter().enumerate() {
        merged[start_index + offset] = Some(result);
    }

    merged
}

fn parse_recent_results_json(value: &serde_json::Value) -> Vec<RoundResult> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_recent_result_entry)
        .collect()
}

fn parse_recent_result_entry(value: &serde_json::Value) -> Option<RoundResult> {
    let object = value.as_object()?;
    let position = object
        .get("position")
        .and_then(|entry| entry.as_i64())
        .unwrap_or_default() as i32;
    let is_dnf = object
        .get("is_dnf")
        .and_then(|entry| entry.as_bool())
        .unwrap_or(false);

    if position <= 0 && !is_dnf {
        return None;
    }

    Some(RoundResult {
        position,
        is_dnf,
        has_fastest_lap: object
            .get("has_fastest_lap")
            .and_then(|entry| entry.as_bool())
            .unwrap_or(false),
        grid_position: object
            .get("grid_position")
            .and_then(|entry| entry.as_i64())
            .unwrap_or_default() as i32,
        positions_gained: object
            .get("positions_gained")
            .and_then(|entry| entry.as_i64())
            .unwrap_or_default() as i32,
    })
}

fn get_driver_slot_info(
    db: &Database,
    driver_id: Option<&String>,
    team_id: &str,
    active_season_number: i32,
) -> (Option<String>, Option<i32>) {
    let Some(driver_id) = driver_id else {
        return (None, None);
    };

    let driver_name = driver_queries::get_driver(&db.conn, driver_id)
        .ok()
        .map(|driver| driver.nome);
    let tenure_seasons =
        calculate_consecutive_team_tenure(&db.conn, driver_id, team_id, active_season_number);
    (driver_name, tenure_seasons)
}

fn calculate_consecutive_team_tenure(
    conn: &rusqlite::Connection,
    driver_id: &str,
    team_id: &str,
    active_season_number: i32,
) -> Option<i32> {
    let contracts = contract_queries::get_contracts_for_pilot(conn, driver_id).ok()?;
    consecutive_team_seasons_up_to(&contracts, team_id, active_season_number)
}

fn consecutive_team_seasons_up_to(
    contracts: &[crate::models::contract::Contract],
    team_id: &str,
    active_season_number: i32,
) -> Option<i32> {
    let mut intervals: Vec<(i32, i32)> = contracts
        .iter()
        .filter(|contract| {
            contract.tipo == crate::models::enums::ContractType::Regular
                && contract.equipe_id == team_id
                && contract.status != crate::models::enums::ContractStatus::Pendente
        })
        .map(|contract| {
            (
                contract.temporada_inicio,
                contract.temporada_fim.min(active_season_number),
            )
        })
        .filter(|(start, end)| *start <= *end)
        .collect();

    intervals.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

    let mut covered_until = active_season_number;
    let mut earliest_start = None;

    for (start, end) in intervals {
        if end < covered_until {
            if end + 1 != covered_until {
                continue;
            }
        } else if start > covered_until || end < covered_until {
            continue;
        }

        earliest_start = Some(start);
        covered_until = start - 1;
    }

    earliest_start.map(|start| active_season_number - start + 1)
}

pub(crate) fn get_teams_standings_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<TeamStanding>, String> {
    let category = category.trim().to_lowercase();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let previous_champions = get_previous_champions_in_base_dir(base_dir, career_id, &category)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let active_season_number = season.numero;

    if categories::is_especial(&category) {
        let special_standings = get_special_team_standings_from_results(
            &db.conn,
            &season,
            &category,
            &previous_champions,
        )?;
        if !special_standings.is_empty() {
            return Ok(special_standings);
        }
    }
    let teams = if categories::is_especial(&category) {
        let entry_teams =
            special_entry_queries::get_entry_teams_for_category(&db.conn, &season.id, &category)
                .map_err(|e| format!("Falha ao buscar equipes da categoria: {e}"))?;
        if entry_teams.is_empty() {
            team_queries::get_teams_by_category(&db.conn, &category)
        } else {
            Ok(entry_teams)
        }
    } else {
        team_queries::get_teams_by_category(&db.conn, &category)
    }
    .map_err(|e| format!("Falha ao buscar equipes da categoria: {e}"))?;

    let mut standings: Vec<TeamStanding> = teams
        .into_iter()
        .map(|team| {
            let team_id = team.id.clone();
            let (piloto_1_nome, piloto_1_tenure_seasons) = get_driver_slot_info(
                &db,
                team.piloto_1_id.as_ref(),
                &team_id,
                active_season_number,
            );
            let (piloto_2_nome, piloto_2_tenure_seasons) = get_driver_slot_info(
                &db,
                team.piloto_2_id.as_ref(),
                &team_id,
                active_season_number,
            );
            let founded_year = team_founded_year_for_payload(&team);

            TeamStanding {
                posicao: 0,
                id: team_id.clone(),
                nome: team.nome,
                nome_curto: team.nome_curto,
                cor_primaria: team.cor_primaria,
                cash_balance: team.cash_balance,
                car_performance: team.car_performance,
                car_build_profile: team.car_build_profile.as_str().to_string(),
                founded_year,
                pontos: team.stats_pontos,
                vitorias: team.stats_vitorias,
                piloto_1_nome,
                piloto_1_tenure_seasons,
                piloto_2_nome,
                piloto_2_tenure_seasons,
                trofeus: previous_champions
                    .constructor_champions
                    .iter()
                    .find(|champion| champion.team_id == team_id)
                    .map(|champion| {
                        vec![TrophyInfo {
                            tipo: "ouro".to_string(),
                            temporada: 0,
                            is_defending: champion.is_defending,
                        }]
                    })
                    .unwrap_or_default(),
                classe: team.classe.clone(),
                temp_posicao: team.temp_posicao,
                categoria_anterior: team.categoria_anterior.clone(),
            }
        })
        .collect();

    let use_previous_season_order = standings
        .iter()
        .all(|team| team.pontos == 0 && team.vitorias == 0);
    let previous_team_positions = if use_previous_season_order {
        previous_team_positions_by_team(&db.conn, active_season_number, &category)?
    } else {
        HashMap::new()
    };

    standings.sort_by(|a, b| {
        if use_previous_season_order {
            let a_previous = previous_team_positions
                .get(&a.id)
                .copied()
                .unwrap_or(i32::MAX);
            let b_previous = previous_team_positions
                .get(&b.id)
                .copied()
                .unwrap_or(i32::MAX);

            return a_previous
                .cmp(&b_previous)
                .then_with(|| a.nome.cmp(&b.nome));
        }

        b.pontos
            .cmp(&a.pontos)
            .then_with(|| b.vitorias.cmp(&a.vitorias))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    for (index, team) in standings.iter_mut().enumerate() {
        team.posicao = index as i32 + 1;
    }

    Ok(standings)
}

fn previous_team_positions_by_team(
    conn: &rusqlite::Connection,
    active_season_number: i32,
    category: &str,
) -> Result<HashMap<String, i32>, String> {
    let previous_season_number = active_season_number - 1;
    if previous_season_number < 1 {
        return Ok(HashMap::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT st.equipe_id,
                    COALESCE(SUM(st.pontos), 0.0) AS total_pontos,
                    COALESCE(SUM(st.vitorias), 0) AS total_vitorias,
                    COALESCE(MIN(NULLIF(st.posicao, 0)), 999999) AS melhor_posicao
             FROM standings st
             INNER JOIN seasons s ON s.id = st.temporada_id
             WHERE s.numero = ?1
               AND LOWER(TRIM(st.categoria)) = ?2
               AND st.equipe_id IS NOT NULL
               AND TRIM(st.equipe_id) <> ''
             GROUP BY st.equipe_id
             ORDER BY total_pontos DESC,
                      total_vitorias DESC,
                      melhor_posicao ASC,
                      st.equipe_id ASC",
        )
        .map_err(|e| format!("Falha ao preparar ranking anterior de equipes: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![previous_season_number, category], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| format!("Falha ao buscar ranking anterior de equipes: {e}"))?;

    let mut positions = HashMap::new();
    for (index, row) in rows.enumerate() {
        let team_id = row.map_err(|e| format!("Falha ao ler ranking anterior de equipes: {e}"))?;
        positions.insert(team_id, index as i32 + 1);
    }

    Ok(positions)
}

fn team_founded_year_for_payload(team: &Team) -> i32 {
    if team.ano_fundacao > 1800 {
        return team.ano_fundacao;
    }

    let rank_index = team.meta_posicao.saturating_sub(1).max(0) as usize;
    historical_team_foundation_year(&team.nome, &team.categoria, rank_index, 10)
}

#[derive(Debug, Clone)]
struct TeamRaceFact {
    team_id: String,
    season_number: i32,
    season_year: i32,
    category: String,
    round: i32,
    points: f64,
    win: bool,
    podium: bool,
}

#[derive(Debug, Clone, Default)]
struct TeamHistoryAggregate {
    races: i32,
    wins: i32,
    podiums: i32,
    points: f64,
}

#[derive(Debug, Clone, Default)]
struct DriverSymbolAggregate {
    name: String,
    races: i32,
    wins: i32,
    podiums: i32,
}

pub(crate) fn get_team_history_dossier_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    team_id: &str,
    category: &str,
) -> Result<TeamHistoryDossier, String> {
    let category = category.trim().to_lowercase();
    let team_id = team_id.trim();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let category_ids = team_history_group_categories(&category);
    let record_scope = team_history_group_label(&category);
    let all_facts = load_team_race_facts(&db.conn, &category_ids)?;
    let selected_facts: Vec<TeamRaceFact> = all_facts
        .iter()
        .filter(|fact| fact.team_id == team_id)
        .cloned()
        .collect();
    let aggregates = aggregate_team_history(&all_facts);
    let selected = aggregates.get(team_id).cloned().unwrap_or_default();
    let titles_by_team = load_constructor_titles_by_team(&db.conn, &category_ids)?;
    let selected_titles = titles_by_team.get(team_id).cloned().unwrap_or_default();
    let title_count = selected_titles.len() as i32;

    let races = selected.races.max(0);
    let wins = selected.wins.max(0);
    let podiums = selected.podiums.max(0);
    let win_rate = percentage(wins, races);
    let podium_rate = percentage(podiums, races);
    let seasons = distinct_seasons(&selected_facts);
    let has_history = races > 0;
    let sport = TeamHistorySport {
        seasons: season_count_label(seasons.len() as i32),
        current_streak: current_season_streak_label(&seasons, &record_scope),
        best_streak: best_real_streak_label(&selected_facts),
        podium_rate: format!("{podium_rate}%"),
        win_rate: format!("{win_rate}%"),
        races,
        wins,
        podiums,
    };

    Ok(TeamHistoryDossier {
        team_id: team_id.to_string(),
        category: category.clone(),
        record_scope: record_scope.clone(),
        has_history,
        records: vec![
            TeamHistoryRecord {
                label: "Títulos".to_string(),
                rank: rank_for_i32(&titles_by_team, team_id),
                value: title_count.to_string(),
            },
            TeamHistoryRecord {
                label: "Vitórias".to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| entry.wins as f64),
                value: wins.to_string(),
            },
            TeamHistoryRecord {
                label: "Pódios".to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| entry.podiums as f64),
                value: podiums.to_string(),
            },
            TeamHistoryRecord {
                label: "Taxa de pódio".to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| {
                    if entry.races > 0 {
                        entry.podiums as f64 / entry.races as f64
                    } else {
                        0.0
                    }
                }),
                value: format!("{podium_rate}%"),
            },
            TeamHistoryRecord {
                label: "Taxa de vitória".to_string(),
                rank: rank_for_aggregate(&aggregates, team_id, |entry| {
                    if entry.races > 0 {
                        entry.wins as f64 / entry.races as f64
                    } else {
                        0.0
                    }
                }),
                value: format!("{win_rate}%"),
            },
        ],
        sport,
        identity: build_real_team_identity(
            &db.conn,
            team_id,
            &category,
            &record_scope,
            &selected_facts,
            &aggregates,
            title_count,
        )?,
        management: build_real_team_management(&db.conn, team_id, &selected_facts)?,
        timeline: build_real_team_timeline(&selected_facts),
        title_categories: selected_titles
            .into_iter()
            .enumerate()
            .map(|(index, title)| TeamHistoryTitleCategory {
                category: team_history_category_label(&title.category),
                year: title.season_year.to_string(),
                color: history_palette(index),
            })
            .collect(),
        category_path: build_real_category_path(&selected_facts),
    })
}

fn build_real_team_management(
    conn: &rusqlite::Connection,
    team_id: &str,
    facts: &[TeamRaceFact],
) -> Result<TeamHistoryManagement, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar gestão histórica da equipe: {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' não encontrada para gestão histórica"))?;
    let cash = team.cash_balance.max(0.0);
    let debt = team.debt_balance.max(0.0);
    let points: f64 = facts.iter().map(|fact| fact.points).sum();
    let seasons = distinct_seasons(facts);
    let seasons_count = seasons.len().max(1) as f64;
    let points_per_season = points / seasons_count;
    let healthy_years = if debt <= 0.0 && team.financial_state == "healthy" {
        seasons.len() as i32
    } else {
        0
    };
    let state_label = financial_state_label_for_dossier(&team.financial_state);
    let technical_level = team.car_performance.round().clamp(0.0, 16.0) as i32;

    Ok(TeamHistoryManagement {
        operation_health: state_label.to_string(),
        peak_cash: format_brl(cash),
        worst_crisis: if debt > 0.0 {
            format!("{} de dívida", format_brl(debt))
        } else {
            "Sem dívida real registrada".to_string()
        },
        healthy_years: format!("{healthy_years} Temporadas"),
        efficiency: format!("{} pts/temporada", format_decimal_pt(points_per_season, 1)),
        biggest_investment: format!("Nível {technical_level} - pacote técnico atual"),
        summary: format!(
            "{state_label}: operação com {} em caixa, {} em dívida e {} pontos reais no recorte.",
            format_brl(cash),
            format_brl(debt),
            points.round() as i32
        ),
        peak_cash_detail: "Maior folga financeira registrada no retrato atual da equipe."
            .to_string(),
        worst_crisis_detail: if debt > 0.0 {
            "Passivo real ainda pesa sobre a operação no ciclo atual.".to_string()
        } else {
            "Sem crise de dívida registrada no recorte real disponível.".to_string()
        },
        healthy_years_detail: "Temporadas reais em que a equipe operou sem dívida relevante."
            .to_string(),
        efficiency_detail: format!(
            "{} pontos reais no recorte; média esportiva de {} por temporada.",
            points.round() as i32,
            format_decimal_pt(points_per_season, 1)
        ),
        investment_detail: "Leitura do pacote técnico atual a partir da performance do carro."
            .to_string(),
    })
}

fn financial_state_label_for_dossier(state: &str) -> &'static str {
    match state {
        "dominant" | "healthy" => "Saudável",
        "stable" => "Estável",
        "pressured" => "Pressionada",
        "critical" => "Crítica",
        _ => "Monitorada",
    }
}

fn format_brl(value: f64) -> String {
    let rounded = value.round().max(0.0) as i64;
    let raw = rounded.to_string();
    let mut formatted = String::new();
    for (index, ch) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push('.');
        }
        formatted.push(ch);
    }
    let grouped: String = formatted.chars().rev().collect();
    format!("R$ {grouped}")
}

fn format_decimal_pt(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}").replace('.', ",")
}

#[derive(Debug, Clone)]
struct TeamTitleFact {
    season_year: i32,
    category: String,
}

fn load_team_race_facts(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<Vec<TeamRaceFact>, String> {
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.equipe_id,
            s.numero,
            s.ano,
            c.categoria,
            c.rodada,
            r.race_id,
            SUM(r.pontos) AS team_points,
            MAX(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS has_win,
            MAX(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS has_podium
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN seasons s ON s.id = c.temporada_id
         WHERE c.categoria IN ({placeholders})
         GROUP BY r.equipe_id, s.numero, c.categoria, c.rodada, r.race_id
         ORDER BY s.numero ASC, c.rodada ASC, r.race_id ASC, r.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar histórico real da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok(TeamRaceFact {
                team_id: row.get(0)?,
                season_number: row.get(1)?,
                season_year: row.get(2)?,
                category: row.get(3)?,
                round: row.get(4)?,
                points: row.get(6)?,
                win: row.get::<_, i32>(7)? > 0,
                podium: row.get::<_, i32>(8)? > 0,
            })
        })
        .map_err(|e| format!("Falha ao consultar histórico real da equipe: {e}"))?;

    let mut facts = Vec::new();
    for row in rows {
        facts.push(row.map_err(|e| format!("Falha ao ler histórico real da equipe: {e}"))?);
    }
    Ok(facts)
}

fn aggregate_team_history(facts: &[TeamRaceFact]) -> HashMap<String, TeamHistoryAggregate> {
    let mut aggregates: HashMap<String, TeamHistoryAggregate> = HashMap::new();
    for fact in facts {
        let entry = aggregates.entry(fact.team_id.clone()).or_default();
        entry.races += 1;
        entry.points += fact.points;
        if fact.win {
            entry.wins += 1;
        }
        if fact.podium {
            entry.podiums += 1;
        }
    }
    aggregates
}

fn load_constructor_titles_by_team(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<HashMap<String, Vec<TeamTitleFact>>, String> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            st.temporada_id,
            s.numero,
            s.ano,
            st.equipe_id,
            st.categoria,
            SUM(st.pontos) AS team_points,
            SUM(st.vitorias) AS team_wins
         FROM standings st
         JOIN seasons s ON s.id = st.temporada_id
         WHERE st.equipe_id IS NOT NULL
           AND st.categoria IN ({placeholders})
         GROUP BY st.temporada_id, s.numero, s.ano, st.equipe_id, st.categoria
         ORDER BY s.numero ASC, team_points DESC, team_wins DESC, st.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar títulos reais de equipes: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i32>(6)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar títulos reais de equipes: {e}"))?;

    let mut best_by_season_category: BTreeMap<String, (i32, i32, String, String, f64, i32)> =
        BTreeMap::new();
    for row in rows {
        let (season_id, season_number, season_year, team_id, category, points, wins) =
            row.map_err(|e| format!("Falha ao ler títulos reais de equipes: {e}"))?;
        let key = format!("{season_id}:{category}");
        let replace = best_by_season_category
            .get(&key)
            .map(|(_, _, current_team, _, current_points, current_wins)| {
                points > *current_points
                    || ((points - *current_points).abs() < f64::EPSILON
                        && (wins > *current_wins
                            || (wins == *current_wins && team_id < *current_team)))
            })
            .unwrap_or(true);
        if replace {
            best_by_season_category.insert(
                key,
                (season_number, season_year, team_id, category, points, wins),
            );
        }
    }

    let mut titles: HashMap<String, Vec<TeamTitleFact>> = HashMap::new();
    for (_, (_season_number, season_year, team_id, category, _, _)) in best_by_season_category {
        titles.entry(team_id).or_default().push(TeamTitleFact {
            season_year,
            category,
        });
    }
    Ok(titles)
}

fn distinct_seasons(facts: &[TeamRaceFact]) -> Vec<i32> {
    facts
        .iter()
        .map(|fact| fact.season_number)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn season_count_label(total: i32) -> String {
    match total {
        0 => "Sem Temporadas registradas".to_string(),
        1 => "1 Temporada".to_string(),
        value => format!("{value} Temporadas"),
    }
}

fn current_season_streak_label(seasons: &[i32], record_scope: &str) -> String {
    if seasons.is_empty() {
        return "Sem sequência registrada".to_string();
    }
    let mut streak = 1;
    for window in seasons.windows(2).rev() {
        if window[1] - window[0] == 1 {
            streak += 1;
        } else {
            break;
        }
    }
    if streak == 1 {
        format!("1 Temporada seguida no {record_scope}")
    } else {
        format!("{streak} Temporadas seguidas no {record_scope}")
    }
}

fn best_real_streak_label(facts: &[TeamRaceFact]) -> String {
    if facts.is_empty() {
        return "Sem sequência registrada".to_string();
    }
    let mut best_podium = 0;
    let mut current_podium = 0;
    let mut best_points = 0;
    let mut current_points = 0;
    for fact in facts {
        if fact.podium {
            current_podium += 1;
            best_podium = best_podium.max(current_podium);
        } else {
            current_podium = 0;
        }
        if fact.points > 0.0 {
            current_points += 1;
            best_points = best_points.max(current_points);
        } else {
            current_points = 0;
        }
    }
    if best_podium > 0 {
        if best_podium == 1 {
            "1 Pódio registrado".to_string()
        } else {
            format!("{best_podium} Pódios consecutivos")
        }
    } else if best_points > 0 {
        if best_points == 1 {
            "1 Corrida pontuando".to_string()
        } else {
            format!("{best_points} Corridas pontuando")
        }
    } else {
        "Sem sequência registrada".to_string()
    }
}

fn build_real_team_timeline(facts: &[TeamRaceFact]) -> Vec<TeamHistoryTimelineItem> {
    let Some(first) = facts.first() else {
        return vec![TeamHistoryTimelineItem {
            year: "-".to_string(),
            text: "Sem corridas registradas neste recorte.".to_string(),
        }];
    };
    let mut items = vec![TeamHistoryTimelineItem {
        year: first.season_year.to_string(),
        text: format!(
            "Primeira corrida registrada em {}, rodada {}.",
            team_history_category_label(&first.category),
            first.round
        ),
    }];

    if let Some(first_win) = facts.iter().find(|fact| fact.win) {
        items.push(TeamHistoryTimelineItem {
            year: first_win.season_year.to_string(),
            text: format!(
                "Primeira vitória real em {}, rodada {}.",
                team_history_category_label(&first_win.category),
                first_win.round
            ),
        });
    }

    if let Some((season, points)) = best_real_season_points(facts) {
        items.push(TeamHistoryTimelineItem {
            year: season.to_string(),
            text: format!(
                "Melhor temporada registrada: {} pts.",
                points.round() as i32
            ),
        });
    }

    if let Some(latest) = facts.last() {
        items.push(TeamHistoryTimelineItem {
            year: latest.season_year.to_string(),
            text: format!(
                "Último registro em {}, rodada {}.",
                team_history_category_label(&latest.category),
                latest.round
            ),
        });
    }

    items
}

fn build_real_team_identity(
    conn: &rusqlite::Connection,
    team_id: &str,
    category: &str,
    record_scope: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    titles: i32,
) -> Result<TeamHistoryIdentity, String> {
    let origin_category = selected_facts
        .first()
        .map(|fact| fact.category.as_str())
        .unwrap_or(category);
    let current = current_team_category_label(conn, team_id)
        .unwrap_or_else(|| team_history_category_label(category));
    let profile = real_team_profile(
        selected_facts.len() as i32,
        selected_facts.iter().filter(|fact| fact.win).count() as i32,
        selected_facts.iter().filter(|fact| fact.podium).count() as i32,
        titles,
    );
    let team_name = conn
        .query_row(
            "SELECT nome FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar equipe para identidade real: {e}"))?
        .unwrap_or_else(|| "A equipe".to_string());
    let rival = real_team_rival(conn, team_id, selected_facts, aggregates, record_scope)?;
    let (symbol_driver, symbol_driver_detail) = real_symbol_driver(conn, team_id, selected_facts)?;

    Ok(TeamHistoryIdentity {
        origin: team_history_category_label(origin_category),
        current,
        profile: profile.clone(),
        summary: real_identity_summary(&team_name, &profile, selected_facts.len() as i32, titles),
        rival,
        symbol_driver,
        symbol_driver_detail,
    })
}

fn current_team_category_label(conn: &rusqlite::Connection, team_id: &str) -> Option<String> {
    let category = conn
        .query_row(
            "SELECT categoria FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()?;
    Some(team_history_category_label(&category))
}

fn real_team_profile(races: i32, wins: i32, podiums: i32, titles: i32) -> String {
    let win_rate = if races > 0 {
        wins as f64 / races as f64
    } else {
        0.0
    };
    let podium_rate = if races > 0 {
        podiums as f64 / races as f64
    } else {
        0.0
    };
    if titles > 0 || win_rate >= 0.25 || podium_rate >= 0.60 {
        "Dominante".to_string()
    } else if podium_rate >= 0.35 {
        "Competitiva".to_string()
    } else if races >= 10 && wins == 0 {
        "Sobrevivente Competitiva".to_string()
    } else {
        "Equipe de Meio de Grid".to_string()
    }
}

fn real_identity_summary(team_name: &str, profile: &str, races: i32, titles: i32) -> String {
    match profile {
        "Dominante" => format!(
            "{team_name} construiu uma identidade vencedora com resultados reais no histórico recente."
        ),
        "Competitiva" => format!(
            "{team_name} aparece como força constante, sustentada por pódios e presença regular no pelotão da frente."
        ),
        "Sobrevivente Competitiva" => format!(
            "{team_name} acumulou {races} corridas reais no recorte, resistindo mesmo sem transformar presença em vitórias."
        ),
        _ => {
            if titles > 0 {
                format!("{team_name} tem título registrado, mas ainda busca transformar o histórico em domínio contínuo.")
            } else {
                format!("{team_name} tem identidade em construção, baseada nos resultados reais já registrados.")
            }
        }
    }
}

fn real_team_rival(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    record_scope: &str,
) -> Result<TeamHistoryRival, String> {
    let selected_races: HashSet<(i32, i32, String)> = selected_facts
        .iter()
        .map(|fact| (fact.season_number, fact.round, fact.category.clone()))
        .collect();
    let mut shared_races: HashMap<String, i32> = HashMap::new();
    for fact in load_team_race_facts(
        conn,
        &selected_facts
            .iter()
            .map(|fact| fact.category.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    )? {
        if fact.team_id == team_id {
            continue;
        }
        if selected_races.contains(&(fact.season_number, fact.round, fact.category.clone())) {
            *shared_races.entry(fact.team_id).or_default() += 1;
        }
    }

    let selected_points = aggregates
        .get(team_id)
        .map(|entry| entry.points)
        .unwrap_or(0.0);
    let rival_id = shared_races
        .iter()
        .max_by(|(left_id, left_shared), (right_id, right_shared)| {
            left_shared.cmp(right_shared).then_with(|| {
                let left_gap = aggregates
                    .get(left_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                let right_gap = aggregates
                    .get(right_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                right_gap.total_cmp(&left_gap)
            })
        })
        .map(|(id, _)| id.clone());

    let Some(rival_id) = rival_id else {
        return Ok(TeamHistoryRival {
            name: "Sem rival consolidado".to_string(),
            current_category: record_scope.to_string(),
            note: "Histórico real ainda sem confronto repetido o bastante para formar rivalidade."
                .to_string(),
        });
    };

    let (name, category): (String, String) = conn
        .query_row(
            "SELECT nome, categoria FROM teams WHERE id = ?1",
            rusqlite::params![&rival_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Falha ao buscar rival histórico real: {e}"))?;
    let shared = shared_races.get(&rival_id).copied().unwrap_or(0);
    Ok(TeamHistoryRival {
        name,
        current_category: team_history_category_label(&category),
        note: format!("{shared} disputas diretas reais no {record_scope}."),
    })
}

fn real_symbol_driver(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
) -> Result<(String, String), String> {
    if selected_facts.is_empty() {
        return Ok((
            "Sem piloto símbolo".to_string(),
            "A equipe ainda não tem corridas registradas suficientes nesse recorte.".to_string(),
        ));
    }
    let category_ids = selected_facts
        .iter()
        .map(|fact| fact.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.piloto_id,
            d.nome,
            COUNT(DISTINCT r.race_id) AS races,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins,
            SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS podiums
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN drivers d ON d.id = r.piloto_id
         WHERE r.equipe_id = ?1
           AND c.categoria IN ({placeholders})
         GROUP BY r.piloto_id, d.nome
         ORDER BY wins DESC, podiums DESC, races DESC, d.nome ASC
         LIMIT 1"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&team_id];
    for category in &category_ids {
        params.push(category);
    }
    let symbol = conn
        .query_row(&sql, params.as_slice(), |row| {
            Ok(DriverSymbolAggregate {
                name: row.get(1)?,
                races: row.get(2)?,
                wins: row.get(3)?,
                podiums: row.get(4)?,
            })
        })
        .optional()
        .map_err(|e| format!("Falha ao buscar piloto símbolo real: {e}"))?;

    let Some(symbol) = symbol else {
        return Ok((
            "Sem piloto símbolo".to_string(),
            "A equipe ainda não tem piloto com resultados registrados nesse recorte.".to_string(),
        ));
    };

    Ok((
        symbol.name,
        format!(
            "{}, {}, {} pela equipe.",
            count_label(symbol.races, "corrida", "corridas"),
            count_label(symbol.wins, "vitória", "vitórias"),
            count_label(symbol.podiums, "pódio", "pódios")
        ),
    ))
}

fn count_label(count: i32, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn best_real_season_points(facts: &[TeamRaceFact]) -> Option<(i32, f64)> {
    let mut points_by_season: BTreeMap<i32, f64> = BTreeMap::new();
    for fact in facts {
        *points_by_season.entry(fact.season_year).or_default() += fact.points;
    }
    points_by_season
        .into_iter()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
}

fn build_real_category_path(facts: &[TeamRaceFact]) -> Vec<TeamHistoryCategoryStep> {
    let mut by_category: BTreeMap<String, (i32, i32)> = BTreeMap::new();
    for fact in facts {
        by_category
            .entry(fact.category.clone())
            .and_modify(|(start, end)| {
                *start = (*start).min(fact.season_number);
                *end = (*end).max(fact.season_number);
            })
            .or_insert((fact.season_number, fact.season_number));
    }
    by_category
        .into_iter()
        .enumerate()
        .map(
            |(index, (category, (start, end)))| TeamHistoryCategoryStep {
                category: team_history_category_label(&category),
                years: if start == end {
                    start.to_string()
                } else {
                    format!("{start}-{end}")
                },
                detail: "Resultados reais registrados nesse recorte.".to_string(),
                color: history_palette(index),
            },
        )
        .collect()
}

fn percentage(numerator: i32, denominator: i32) -> i32 {
    if denominator <= 0 {
        0
    } else {
        ((numerator as f64 / denominator as f64) * 100.0).round() as i32
    }
}

fn rank_for_aggregate<F>(
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    selected_team_id: &str,
    metric: F,
) -> String
where
    F: Fn(&TeamHistoryAggregate) -> f64,
{
    let mut ordered: Vec<(&String, f64)> = aggregates
        .iter()
        .map(|(team_id, aggregate)| (team_id, metric(aggregate)))
        .collect();
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right.total_cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let rank = ordered
        .iter()
        .position(|(team_id, _)| team_id.as_str() == selected_team_id)
        .map(|index| index + 1)
        .unwrap_or(1);
    format_ordinal_i32(rank as i32)
}

fn rank_for_i32(values: &HashMap<String, Vec<TeamTitleFact>>, selected_team_id: &str) -> String {
    let mut ordered: Vec<(String, i32)> = values
        .iter()
        .map(|(team_id, titles)| (team_id.clone(), titles.len() as i32))
        .collect();
    if !values.contains_key(selected_team_id) {
        ordered.push((selected_team_id.to_string(), 0));
    }
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right.cmp(left).then_with(|| left_id.cmp(right_id))
    });
    let rank = ordered
        .iter()
        .position(|(team_id, _)| team_id.as_str() == selected_team_id)
        .map(|index| index + 1)
        .unwrap_or(1);
    format_ordinal_i32(rank as i32)
}

fn format_ordinal_i32(value: i32) -> String {
    format!("{value}º")
}

fn team_history_group_categories(category: &str) -> Vec<String> {
    match category {
        "mazda_rookie" | "mazda_amador" => {
            vec!["mazda_rookie".to_string(), "mazda_amador".to_string()]
        }
        "toyota_rookie" | "toyota_amador" => {
            vec!["toyota_rookie".to_string(), "toyota_amador".to_string()]
        }
        "bmw_m2" => vec!["bmw_m2".to_string()],
        "production_challenger" => vec![
            "mazda_rookie".to_string(),
            "mazda_amador".to_string(),
            "toyota_rookie".to_string(),
            "toyota_amador".to_string(),
            "bmw_m2".to_string(),
            "production_challenger".to_string(),
        ],
        "gt4" => vec!["gt4".to_string()],
        "gt3" => vec!["gt3".to_string()],
        "endurance" => vec!["endurance".to_string()],
        other => vec![other.to_string()],
    }
}

fn team_history_group_label(category: &str) -> String {
    match category {
        "mazda_rookie" | "mazda_amador" => "Grupo Mazda",
        "toyota_rookie" | "toyota_amador" => "Grupo Toyota",
        "bmw_m2" => "Grupo BMW",
        "production_challenger" => "Grupo Production",
        "gt4" => "Grupo GT4",
        "gt3" => "Grupo GT3",
        "endurance" => "Grupo Endurance",
        _ => "Grupo da categoria",
    }
    .to_string()
}

fn team_history_category_label(category: &str) -> String {
    categories::get_category_config(category)
        .map(|config| config.nome_curto.to_string())
        .unwrap_or_else(|| category.to_string())
}

fn history_palette(index: usize) -> String {
    ["#58a6ff", "#f2c46d", "#5ee7a8", "#ff6b6b"][index % 4].to_string()
}

fn get_special_team_standings_from_results(
    conn: &rusqlite::Connection,
    season: &Season,
    category: &str,
    previous_champions: &PreviousChampions,
) -> Result<Vec<TeamStanding>, String> {
    let rows = query_special_team_standing_rows(conn, &season.id, category)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let team = team_queries::get_team_by_id(conn, &row.team_id)
                .map_err(|e| format!("Falha ao carregar equipe especial '{}': {e}", row.team_id))?
                .ok_or_else(|| format!("Equipe especial '{}' nao encontrada", row.team_id))?;
            let driver_names =
                query_special_team_driver_names(conn, &season.id, category, &row.team_id)?;
            let team_id = team.id.clone();
            let founded_year = team_founded_year_for_payload(&team);

            Ok(TeamStanding {
                posicao: index as i32 + 1,
                id: team_id.clone(),
                nome: team.nome,
                nome_curto: team.nome_curto,
                cor_primaria: team.cor_primaria,
                cash_balance: team.cash_balance,
                car_performance: team.car_performance,
                car_build_profile: team.car_build_profile.as_str().to_string(),
                founded_year,
                pontos: row.points.round() as i32,
                vitorias: row.wins,
                piloto_1_nome: driver_names.get(0).cloned(),
                piloto_1_tenure_seasons: None,
                piloto_2_nome: driver_names.get(1).cloned(),
                piloto_2_tenure_seasons: None,
                trofeus: previous_champions
                    .constructor_champions
                    .iter()
                    .find(|champion| champion.team_id == team_id)
                    .map(|champion| {
                        vec![TrophyInfo {
                            tipo: "ouro".to_string(),
                            temporada: 0,
                            is_defending: champion.is_defending,
                        }]
                    })
                    .unwrap_or_default(),
                classe: row.class_name.clone().or_else(|| team.classe.clone()),
                temp_posicao: team.temp_posicao,
                categoria_anterior: team.categoria_anterior.clone(),
            })
        })
        .collect()
}

fn query_special_team_standing_rows(
    conn: &rusqlite::Connection,
    season_id: &str,
    category: &str,
) -> Result<Vec<HistoricalSpecialTeamStanding>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                r.equipe_id,
                COALESCE(SUM(r.pontos), 0.0) AS total_points,
                SUM(CASE WHEN r.posicao_final = 1 AND r.dnf = 0 THEN 1 ELSE 0 END) AS total_wins,
                MAX(e.class_name) AS class_name
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             INNER JOIN teams t ON t.id = r.equipe_id
             LEFT JOIN special_team_entries e
               ON e.season_id = COALESCE(c.season_id, c.temporada_id)
              AND e.special_category = c.categoria
              AND e.team_id = r.equipe_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
               AND c.categoria = ?2
               AND r.equipe_id <> ''
             GROUP BY r.equipe_id
             ORDER BY total_points DESC, total_wins DESC, t.nome ASC",
        )
        .map_err(|e| format!("Falha ao preparar standings especiais de equipes: {e}"))?;

    let mapped = stmt
        .query_map(rusqlite::params![season_id, category], |row| {
            Ok(HistoricalSpecialTeamStanding {
                team_id: row.get(0)?,
                points: row.get(1)?,
                wins: row.get(2)?,
                class_name: row.get(3)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar standings especiais de equipes: {e}"))?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row.map_err(|e| format!("Falha ao ler standings especiais de equipes: {e}"))?);
    }
    Ok(rows)
}

fn query_special_team_driver_names(
    conn: &rusqlite::Connection,
    season_id: &str,
    category: &str,
    team_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT d.nome
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             INNER JOIN drivers d ON d.id = r.piloto_id
             WHERE COALESCE(c.season_id, c.temporada_id) = ?1
               AND c.categoria = ?2
               AND r.equipe_id = ?3
             GROUP BY r.piloto_id
             ORDER BY COUNT(*) DESC, MIN(c.rodada) ASC, MIN(r.posicao_final) ASC, d.nome ASC
             LIMIT 2",
        )
        .map_err(|e| format!("Falha ao preparar pilotos da equipe especial: {e}"))?;

    let mapped = stmt
        .query_map(rusqlite::params![season_id, category, team_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| format!("Falha ao consultar pilotos da equipe especial: {e}"))?;

    let mut names = Vec::new();
    for row in mapped {
        names.push(row.map_err(|e| format!("Falha ao ler piloto da equipe especial: {e}"))?);
    }
    Ok(names)
}

pub(crate) fn get_race_results_by_category_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<DriverRaceHistory>, String> {
    let category = category.trim().to_lowercase();
    let (db, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let drivers = driver_queries::get_drivers_by_category(&db.conn, &category)
        .map_err(|e| format!("Falha ao buscar pilotos da categoria: {e}"))?;
    let total_rounds = count_calendar_entries(&db.conn, &season.id, &category)
        .map_err(|e| format!("Falha ao contar corridas da categoria: {e}"))?
        as usize;
    let driver_ids: Vec<String> = drivers.into_iter().map(|driver| driver.id).collect();

    build_driver_histories(&career_dir, &category, total_rounds, &driver_ids)
}

pub(crate) fn get_previous_champions_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    _category: &str,
) -> Result<PreviousChampions, String> {
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    if season.numero <= 1 {
        return Ok(empty_previous_champions());
    }

    Ok(PreviousChampions {
        driver_champion_id: None,
        constructor_champions: Vec::<ConstructorChampion>::new(),
    })
}

pub(crate) fn get_calendar_for_category_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<RaceSummary>, String> {
    let category = category.trim().to_lowercase();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    calendar_queries::normalize_calendar_display_dates_for_weekday_policy(
        &db.conn, &season.id, season.ano,
    )
    .map_err(|e| format!("Falha ao normalizar datas do calendario: {e}"))?;
    let entries = calendar_queries::get_calendar(&db.conn, &season.id, &category)
        .map_err(|e| format!("Falha ao buscar calendario da categoria: {e}"))?;

    Ok(entries
        .into_iter()
        .map(|race| RaceSummary {
            id: race.id,
            rodada: race.rodada,
            track_name: race.track_name,
            clima: race.clima.as_str().to_string(),
            duracao_corrida_min: race.duracao_corrida_min,
            status: race.status.as_str().to_string(),
            temperatura: race.temperatura,
            horario: race.horario.clone(),
            week_of_year: race.week_of_year,
            season_phase: race.season_phase.as_str().to_string(),
            display_date: race.display_date.clone(),
            event_interest: None,
        })
        .collect())
}

fn write_save_meta(path: &Path, meta: &SaveMeta) -> Result<(), String> {
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Falha ao serializar meta.json: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Falha ao gravar meta.json: {e}"))
}

fn save_meta_to_info(meta: SaveMeta) -> SaveInfo {
    SaveInfo {
        career_id: format!("career_{:03}", meta.career_number),
        player_name: meta.player_name,
        category_name: categories::get_category_config(&meta.category)
            .map(|category| category.nome.to_string())
            .unwrap_or_else(|| meta.category.clone()),
        category: meta.category,
        season: meta.current_season as i32,
        year: meta.current_year as i32,
        difficulty: meta.difficulty,
        created: meta.created_at,
        last_played: meta.last_played,
        total_races: meta.total_races,
    }
}

fn preferred_active_contract_for_phase(
    conn: &rusqlite::Connection,
    driver_id: &str,
    season_phase: SeasonPhase,
) -> Result<Option<crate::models::contract::Contract>, String> {
    if season_phase == SeasonPhase::BlocoEspecial {
        let special_contract =
            contract_queries::get_active_especial_contract_for_pilot(conn, driver_id)
                .map_err(|e| format!("Falha ao buscar contrato especial ativo: {e}"))?;
        if special_contract.is_some() {
            return Ok(special_contract);
        }
    }

    contract_queries::get_active_regular_contract_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao buscar contrato regular ativo: {e}"))
}

fn find_player_team(
    conn: &rusqlite::Connection,
    player_id: &str,
    season_phase: SeasonPhase,
) -> Result<Option<Team>, String> {
    let contract = preferred_active_contract_for_phase(conn, player_id, season_phase)?;
    resolve_driver_team(conn, player_id, contract.as_ref())
}

fn resolve_driver_team(
    conn: &rusqlite::Connection,
    driver_id: &str,
    contract: Option<&crate::models::contract::Contract>,
) -> Result<Option<Team>, String> {
    if let Some(contract) = contract {
        if let Some(mut team) = team_queries::get_team_by_id(conn, &contract.equipe_id)
            .map_err(|e| format!("Falha ao buscar equipe do contrato: {e}"))?
        {
            if contract.tipo.as_str() == "Especial" {
                team.categoria = contract.categoria.clone();
                team.classe = contract.classe.clone();
                let special_contracts =
                    contract_queries::get_active_especial_contracts_by_category(
                        conn,
                        &contract.categoria,
                    )
                    .map_err(|e| format!("Falha ao buscar contratos especiais ativos: {e}"))?;
                team.piloto_1_id = special_contracts
                    .iter()
                    .find(|value| {
                        value.equipe_id == contract.equipe_id && value.papel.as_str() == "Numero1"
                    })
                    .map(|value| value.piloto_id.clone());
                team.piloto_2_id = special_contracts
                    .iter()
                    .find(|value| {
                        value.equipe_id == contract.equipe_id && value.papel.as_str() == "Numero2"
                    })
                    .map(|value| value.piloto_id.clone());
            }
            return Ok(Some(team));
        }
    }

    let mut stmt = conn
        .prepare("SELECT id FROM teams WHERE piloto_1_id = ?1 OR piloto_2_id = ?1 LIMIT 1")
        .map_err(|e| format!("Falha ao procurar equipe do piloto: {e}"))?;
    let team_id: Option<String> = stmt
        .query_row(rusqlite::params![driver_id], |row| row.get(0))
        .optional()
        .map_err(|e| format!("Falha ao procurar equipe do piloto: {e}"))?;

    match team_id {
        Some(id) => team_queries::get_team_by_id(conn, &id)
            .map_err(|e| format!("Falha ao carregar equipe do piloto: {e}")),
        None => Ok(None),
    }
}

fn resolve_driver_role(
    driver_id: &str,
    contract: Option<&crate::models::contract::Contract>,
    team: Option<&Team>,
) -> Option<String> {
    if let Some(contract) = contract {
        return Some(contract.papel.as_str().to_string());
    }

    team.and_then(|value| {
        if value.piloto_1_id.as_deref() == Some(driver_id) {
            Some("Numero1".to_string())
        } else if value.piloto_2_id.as_deref() == Some(driver_id) {
            Some("Numero2".to_string())
        } else {
            None
        }
    })
}

fn build_team_summary(conn: &rusqlite::Connection, team: &Team) -> Result<TeamSummary, String> {
    let piloto_1_nome = match &team.piloto_1_id {
        Some(id) => Some(
            driver_queries::get_driver(conn, id)
                .map_err(|e| format!("Falha ao carregar piloto 1 da equipe: {e}"))?
                .nome,
        ),
        None => None,
    };

    let piloto_2_nome = match &team.piloto_2_id {
        Some(id) => Some(
            driver_queries::get_driver(conn, id)
                .map_err(|e| format!("Falha ao carregar piloto 2 da equipe: {e}"))?
                .nome,
        ),
        None => None,
    };

    let financial_plan = calculate_financial_plan(team);
    let salary_ceiling = calculate_salary_ceiling(team);
    let active_contracts = contract_queries::get_active_contracts_for_team(conn, &team.id)
        .map_err(|e| format!("Falha ao carregar contratos ativos da equipe: {e}"))?;
    let piloto_1_salario_anual = salary_for_driver(&active_contracts, team.piloto_1_id.as_deref());
    let piloto_2_salario_anual = salary_for_driver(&active_contracts, team.piloto_2_id.as_deref());

    Ok(TeamSummary {
        id: team.id.clone(),
        nome: team.nome.clone(),
        nome_curto: team.nome_curto.clone(),
        cor_primaria: team.cor_primaria.clone(),
        cor_secundaria: team.cor_secundaria.clone(),
        categoria: team.categoria.clone(),
        classe: team.classe.clone(),
        car_performance: team.car_performance,
        car_build_profile: team.car_build_profile.as_str().to_string(),
        confiabilidade: team.confiabilidade,
        pit_strategy_risk: team.pit_strategy_risk,
        pit_crew_quality: team.pit_crew_quality,
        budget: team.budget,
        spending_power: financial_plan.spending_power,
        salary_ceiling,
        budget_index: financial_plan.budget_index,
        cash_balance: team.cash_balance,
        debt_balance: team.debt_balance,
        financial_state: team.financial_state.clone(),
        season_strategy: team.season_strategy.clone(),
        last_round_income: team.last_round_income,
        last_round_expenses: team.last_round_expenses,
        last_round_net: team.last_round_net,
        parachute_payment_remaining: team.parachute_payment_remaining,
        piloto_1_id: team.piloto_1_id.clone(),
        piloto_1_nome,
        piloto_1_salario_anual,
        piloto_2_id: team.piloto_2_id.clone(),
        piloto_2_nome,
        piloto_2_salario_anual,
    })
}

fn salary_for_driver(
    contracts: &[crate::models::contract::Contract],
    driver_id: Option<&str>,
) -> Option<f64> {
    let driver_id = driver_id?;
    contracts
        .iter()
        .find(|contract| contract.piloto_id == driver_id)
        .map(|contract| contract.salario_anual)
}

fn build_accepted_special_offer_summary(
    conn: &rusqlite::Connection,
    player: &crate::models::driver::Driver,
) -> Result<Option<AcceptedSpecialOfferSummary>, String> {
    if player.categoria_especial_ativa.is_none() {
        return Ok(None);
    }

    let Some(contract) = contract_queries::get_active_especial_contract_for_pilot(conn, &player.id)
        .map_err(|e| format!("Falha ao buscar contrato especial ativo: {e}"))?
    else {
        return Ok(None);
    };

    Ok(Some(AcceptedSpecialOfferSummary {
        id: contract.id,
        team_id: contract.equipe_id,
        team_name: contract.equipe_nome,
        special_category: contract.categoria,
        class_name: contract.classe.unwrap_or_default(),
        papel: contract.papel.as_str().to_string(),
    }))
}

fn empty_track_history_summary() -> TrackHistorySummary {
    TrackHistorySummary {
        has_data: false,
        starts: 0,
        best_finish: None,
        last_finish: None,
        dnfs: 0,
        last_visit_season: None,
        last_visit_round: None,
    }
}

fn empty_next_race_briefing_summary() -> NextRaceBriefingSummary {
    NextRaceBriefingSummary {
        track_history: Some(empty_track_history_summary()),
        primary_rival: None,
        weekend_stories: Vec::new(),
        contract_warning: None,
    }
}

fn build_next_race_briefing_summary(
    conn: &rusqlite::Connection,
    player_id: &str,
    season_number: i32,
    race: &CalendarEntry,
) -> Result<NextRaceBriefingSummary, String> {
    let contract_warning = contract_queries::get_active_regular_contract_for_pilot(conn, player_id)
        .map_err(|e| format!("Falha ao buscar contrato regular do jogador: {e}"))?
        .and_then(|c| {
            if c.is_ultimo_ano(season_number) {
                Some(ContractWarningInfo {
                    temporada_fim: c.temporada_fim,
                    equipe_nome: c.equipe_nome,
                })
            } else {
                None
            }
        });

    Ok(NextRaceBriefingSummary {
        track_history: Some(build_track_history_summary(
            conn,
            player_id,
            &race.track_name,
        )?),
        primary_rival: build_primary_rival_summary(conn, player_id, &race.categoria)?,
        weekend_stories: build_weekend_story_summaries(
            conn,
            season_number,
            &race.categoria,
            race.rodada,
        )?,
        contract_warning,
    })
}

fn build_track_history_summary(
    conn: &rusqlite::Connection,
    player_id: &str,
    track_name: &str,
) -> Result<TrackHistorySummary, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.numero, c.rodada, r.posicao_final, r.dnf
             FROM race_results r
             JOIN calendar c ON r.race_id = c.id
             JOIN seasons s ON COALESCE(c.season_id, c.temporada_id) = s.id
             WHERE r.piloto_id = ?1
               AND c.track_name = ?2
             ORDER BY s.numero DESC, c.rodada DESC",
        )
        .map_err(|e| format!("Falha ao preparar historico de pista: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![player_id, track_name], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)? != 0,
            ))
        })
        .map_err(|e| format!("Falha ao buscar historico de pista: {e}"))?;

    let mut visits = Vec::new();
    for row in rows {
        visits.push(row.map_err(|e| format!("Falha ao ler historico de pista: {e}"))?);
    }

    if visits.is_empty() {
        return Ok(empty_track_history_summary());
    }

    let last_visit = visits[0];
    let best_finish = visits
        .iter()
        .filter(|(_, _, position, is_dnf)| !*is_dnf && *position > 0)
        .map(|(_, _, position, _)| *position)
        .min();
    let dnfs = visits.iter().filter(|(_, _, _, is_dnf)| *is_dnf).count() as i32;

    Ok(TrackHistorySummary {
        has_data: true,
        starts: visits.len() as i32,
        best_finish,
        last_finish: Some(last_visit.2),
        dnfs,
        last_visit_season: Some(last_visit.0),
        last_visit_round: Some(last_visit.1),
    })
}

fn build_primary_rival_summary(
    conn: &rusqlite::Connection,
    player_id: &str,
    categoria: &str,
) -> Result<Option<PrimaryRivalSummary>, String> {
    let mut drivers = driver_queries::get_drivers_by_category(conn, categoria)
        .map_err(|e| format!("Falha ao buscar pilotos da categoria para rival principal: {e}"))?;

    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
            .then_with(|| b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    let Some(player_index) = drivers.iter().position(|driver| driver.id == player_id) else {
        return Ok(None);
    };

    let player = &drivers[player_index];
    let rival_index = if player_index == 0 {
        if drivers.len() > 1 {
            1
        } else {
            return Ok(None);
        }
    } else {
        player_index - 1
    };
    let rival = &drivers[rival_index];
    let is_ahead = rival_index < player_index;
    let gap_points = if is_ahead {
        (rival.stats_temporada.pontos - player.stats_temporada.pontos)
            .max(0.0)
            .round() as i32
    } else {
        (player.stats_temporada.pontos - rival.stats_temporada.pontos)
            .max(0.0)
            .round() as i32
    };

    Ok(Some(PrimaryRivalSummary {
        driver_id: rival.id.clone(),
        driver_name: rival.nome.clone(),
        championship_position: rival_index as i32 + 1,
        gap_points,
        is_ahead,
        rivalry_label: None,
    }))
}

fn build_weekend_story_summaries(
    conn: &rusqlite::Connection,
    season_number: i32,
    categoria: &str,
    round_number: i32,
) -> Result<Vec<BriefingStorySummary>, String> {
    let mut stories = news_queries::get_news_by_season(conn, season_number, 200)
        .map_err(|e| format!("Falha ao buscar noticias da temporada para a previa: {e}"))?
        .into_iter()
        .filter(|item| {
            item.categoria_id.as_deref() == Some(categoria) && item.rodada == Some(round_number)
        })
        .collect::<Vec<_>>();

    stories.sort_by(|left, right| {
        briefing_importance_rank(&right.importancia)
            .cmp(&briefing_importance_rank(&left.importancia))
            .then_with(|| briefing_type_rank(&right.tipo).cmp(&briefing_type_rank(&left.tipo)))
            .then_with(|| right.timestamp.cmp(&left.timestamp))
    });

    Ok(stories
        .into_iter()
        .take(3)
        .map(|item| BriefingStorySummary {
            id: item.id,
            icon: item.icone,
            title: item.titulo,
            summary: build_briefing_story_summary_text(&item.texto),
            importance: item.importancia.as_str().to_string(),
        })
        .collect())
}

fn briefing_importance_rank(value: &NewsImportance) -> i32 {
    match value {
        NewsImportance::Destaque => 4,
        NewsImportance::Alta => 3,
        NewsImportance::Media => 2,
        NewsImportance::Baixa => 1,
    }
}

fn briefing_type_rank(value: &NewsType) -> i32 {
    match value {
        NewsType::Rivalidade => 5,
        NewsType::Hierarquia => 4,
        NewsType::Corrida => 3,
        NewsType::Incidente => 2,
        NewsType::FramingSazonal => 1,
        _ => 0,
    }
}

fn build_briefing_story_summary_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "O paddock segue produzindo contexto para a proxima largada.".to_string();
    }

    if let Some((first_sentence, _)) = trimmed.split_once('.') {
        let sentence = first_sentence.trim();
        if !sentence.is_empty() {
            return format!("{sentence}.");
        }
    }

    trimmed.chars().take(140).collect()
}

fn warn_if_noncritical<T>(result: Result<T, String>, context: &str) {
    if let Err(error) = result {
        eprintln!("Aviso: {context}: {error}");
    }
}

fn count_rows(conn: &rusqlite::Connection, table: &str) -> Result<usize, rusqlite::Error> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
    Ok(count as usize)
}

pub(crate) fn count_calendar_entries(
    conn: &rusqlite::Connection,
    season_id: &str,
    categoria: &str,
) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1
           AND categoria = ?2",
        rusqlite::params![season_id, categoria],
        |row| row.get(0),
    )
}

fn count_season_calendar_entries(
    conn: &rusqlite::Connection,
    season_id: &str,
) -> Result<i32, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*) FROM calendar
         WHERE COALESCE(season_id, temporada_id) = ?1",
        rusqlite::params![season_id],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, NaiveDate};
    use std::fs;

    use super::*;

    #[test]
    fn test_validate_input_valid() {
        let input = CreateCareerInput {
            player_name: "Joao Silva".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 2,
            difficulty: "medio".to_string(),
        };
        assert!(validate_create_career_input(&input).is_ok());
    }

    #[test]
    fn test_validate_input_empty_name() {
        let input = CreateCareerInput {
            player_name: "   ".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 2,
            difficulty: "medio".to_string(),
        };
        assert!(validate_create_career_input(&input).is_err());
    }

    #[test]
    fn test_validate_input_invalid_category() {
        let input = CreateCareerInput {
            player_name: "Joao".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "gt4".to_string(),
            team_index: 2,
            difficulty: "medio".to_string(),
        };
        assert!(validate_create_career_input(&input).is_err());
    }

    #[test]
    fn offer_salary_uses_real_money_instead_of_legacy_budget() {
        let mut team = crate::models::team::placeholder_team_from_db(
            "TGT4".to_string(),
            "GT4 Rich".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = 6_000_000.0;
        team.debt_balance = 0.0;
        team.financial_state = "healthy".to_string();
        team.budget = 1.0;

        let mut driver = Driver::new(
            "P001".to_string(),
            "Piloto Forte".to_string(),
            "br".to_string(),
            "M".to_string(),
            24,
            2026,
        );
        driver.atributos.skill = 80.0;

        let offer = calculate_offer_salary_for_team(&team, &driver);

        assert!(offer > 100_000.0);
    }

    #[test]
    fn test_validate_input_invalid_team_index() {
        let input = CreateCareerInput {
            player_name: "Joao".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "toyota_rookie".to_string(),
            team_index: 9,
            difficulty: "medio".to_string(),
        };
        assert!(validate_create_career_input(&input).is_err());
    }

    #[test]
    fn test_validate_input_invalid_difficulty() {
        let input = CreateCareerInput {
            player_name: "Joao".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "toyota_rookie".to_string(),
            team_index: 2,
            difficulty: "insano".to_string(),
        };
        assert!(validate_create_career_input(&input).is_err());
    }

    #[test]
    fn test_next_career_id_empty_dir() {
        let base = unique_test_dir("empty");
        let saves_dir = base.join("saves");
        let next = next_career_id(&saves_dir);
        assert_eq!(next, "career_001");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn test_next_career_id_with_existing() {
        let base = unique_test_dir("existing");
        let saves_dir = base.join("saves");
        fs::create_dir_all(saves_dir.join("career_001")).expect("career 001");
        fs::create_dir_all(saves_dir.join("career_003")).expect("career 003");
        let next = next_career_id(&saves_dir);
        assert_eq!(next, "career_004");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn test_create_career_full_flow() {
        let base_dir = unique_test_dir("full_flow");
        fs::create_dir_all(&base_dir).expect("base dir");

        let input = CreateCareerInput {
            player_name: "Joao Silva".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 2,
            difficulty: "medio".to_string(),
        };

        let result = create_career_in_base_dir(&base_dir, input).expect("career should be created");
        assert!(result.success);
        assert_eq!(result.total_drivers, 196);
        assert_eq!(result.total_teams, 71);
        // Categorias especiais (production_challenger=10, endurance=6) não geram calendário
        // no BlocoRegular — calendário delas é criado na JanelaConvocação (Passos 6+).
        assert_eq!(result.total_races, 58);

        let db_path = std::path::PathBuf::from(&result.save_path).join("career.db");
        assert!(db_path.exists());
        let meta_path = std::path::PathBuf::from(&result.save_path).join("meta.json");
        assert!(meta_path.exists());

        let db = Database::open_existing(&db_path).expect("db should open");
        let drivers_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM drivers", [], |row| row.get(0))
            .expect("drivers count");
        let teams_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM teams", [], |row| row.get(0))
            .expect("teams count");
        let contracts_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM contracts", [], |row| row.get(0))
            .expect("contracts count");
        let seasons_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
            .expect("seasons count");
        let calendar_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM calendar", [], |row| row.get(0))
            .expect("calendar count");

        assert_eq!(drivers_count, 196);
        assert_eq!(teams_count, 71);
        // 132 contratos: categorias especiais (production_challenger, endurance) não geram contratos
        assert_eq!(contracts_count, 132);
        assert_eq!(seasons_count, 1);
        // 58 corridas: sem as 16 das categorias especiais (10+6), geradas na JanelaConvocação
        assert_eq!(calendar_count, 58);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_create_career_seeds_initial_licenses_for_active_grid() {
        let base_dir = unique_test_dir("seed_initial_licenses");
        fs::create_dir_all(&base_dir).expect("base dir");

        let input = CreateCareerInput {
            player_name: "Joao Silva".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 2,
            difficulty: "medio".to_string(),
        };

        let result = create_career_in_base_dir(&base_dir, input).expect("career should be created");
        let db_path = std::path::PathBuf::from(&result.save_path).join("career.db");
        let db = Database::open_existing(&db_path).expect("db should open");

        let seeded_licenses: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM licenses", [], |row| row.get(0))
            .expect("licenses count");
        let gt3_without_license: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM contracts c
                 JOIN teams t ON t.id = c.equipe_id
                 LEFT JOIN licenses l
                   ON l.piloto_id = c.piloto_id
                  AND CAST(l.nivel AS INTEGER) >= 3
                 WHERE c.status = 'Ativo'
                   AND c.tipo = 'Regular'
                   AND t.categoria = 'gt3'
                   AND l.piloto_id IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("gt3 license coverage");
        let gt4_without_license: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM contracts c
                 JOIN teams t ON t.id = c.equipe_id
                 LEFT JOIN licenses l
                   ON l.piloto_id = c.piloto_id
                  AND CAST(l.nivel AS INTEGER) >= 2
                 WHERE c.status = 'Ativo'
                   AND c.tipo = 'Regular'
                   AND t.categoria = 'gt4'
                   AND l.piloto_id IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("gt4 license coverage");

        assert_eq!(seeded_licenses, 108);
        assert_eq!(gt3_without_license, 0);
        assert_eq!(gt4_without_license, 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_returns_player() {
        let base_dir = create_test_career_dir("load_player");
        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");

        assert!(career.player.is_jogador);
        assert_eq!(career.player.nome, "Joao Silva");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_returns_team() {
        let base_dir = create_test_career_dir("load_team");
        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let player_team = career.player_team.as_ref().expect("player team");

        assert!(!player_team.id.is_empty());
        assert!(player_team.piloto_1_id.is_some());
        assert!(player_team.piloto_2_id.is_some());
        assert!((0.0..=100.0).contains(&player_team.pit_strategy_risk));
        assert!((0.0..=100.0).contains(&player_team.pit_crew_quality));
        assert!(player_team.cash_balance >= 0.0);
        assert!(player_team.debt_balance >= 0.0);
        assert!(!player_team.financial_state.is_empty());
        assert!(!player_team.season_strategy.is_empty());
        assert!(player_team.spending_power.is_finite());
        assert!(player_team.salary_ceiling > 0.0);
        assert!((0.0..=100.0).contains(&player_team.budget_index));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_returns_season() {
        let base_dir = create_test_career_dir("load_season");
        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");

        assert_eq!(career.season.numero, 1);
        assert_eq!(career.season.ano, 2024);
        assert!(career.season.total_rodadas > 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_includes_next_race_briefing() {
        let base_dir = create_test_career_dir("load_briefing_contract");
        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let career_json = serde_json::to_value(&career).expect("career json");

        assert!(
            career_json.get("next_race_briefing").is_some(),
            "expected load_career payload to expose next_race_briefing",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_restores_resume_context_snapshot() {
        let base_dir = create_test_career_dir("load_resume_context");
        mark_all_races_completed(&base_dir, "career_001");

        let result = advance_season_in_base_dir(&base_dir, "career_001")
            .expect("advance season should work");
        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let resume_context = career.resume_context.expect("resume context");

        assert_eq!(resume_context.active_view, CareerResumeView::EndOfSeason);
        assert_eq!(
            resume_context
                .end_of_season_result
                .as_ref()
                .map(|snapshot| snapshot.new_year),
            Some(result.new_year)
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_repairs_early_convocation_with_regular_races_pending() {
        let base_dir = create_test_career_dir("load_repair_early_convocation");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::JanelaConvocacao)
            .expect("force early convocation");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");

        assert_eq!(career.season.fase, "BlocoRegular");
        let refreshed_season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        assert_eq!(refreshed_season.fase, SeasonPhase::BlocoRegular);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_prefers_active_special_contract_team() {
        let base_dir = create_test_career_dir("load_active_special_team");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.categoria_atual = Some("gt4".to_string());
        player.atributos.skill = 98.0;
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        let offers = crate::commands::convocation::get_player_special_offers_in_base_dir(
            &base_dir,
            "career_001",
        )
        .expect("special offers");
        crate::commands::convocation::respond_player_special_offer_in_base_dir(
            &base_dir,
            "career_001",
            &offers[0].id,
            true,
        )
        .expect("accept offer");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let player_team = career.player_team.as_ref().expect("player team");

        assert_eq!(player_team.categoria, "endurance");
        assert_eq!(
            career
                .next_race
                .as_ref()
                .map(|race| race.season_phase.as_str()),
            Some("BlocoEspecial")
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_serializes_convocation_state_fields() {
        let base_dir = create_test_career_dir("load_convocation_contract_payload");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.categoria_atual = Some("gt4".to_string());
        player.atributos.skill = 98.0;
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        mark_regular_races_completed(&db);
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        let offers = crate::commands::convocation::get_player_special_offers_in_base_dir(
            &base_dir,
            "career_001",
        )
        .expect("special offers");
        crate::commands::convocation::respond_player_special_offer_in_base_dir(
            &base_dir,
            "career_001",
            &offers[0].id,
            true,
        )
        .expect("accept offer");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let payload = serde_json::to_value(&career).expect("serialize payload");

        assert_eq!(payload["season"]["fase"], "JanelaConvocacao");
        assert_eq!(payload["player"]["categoria_especial_ativa"], "endurance");
        assert!(
            payload["player_team"].get("classe").is_some(),
            "player_team.classe deveria ser serializado para a UI"
        );
        assert_eq!(
            payload["accepted_special_offer"]["special_category"],
            "endurance"
        );
        assert_eq!(
            payload["accepted_special_offer"]["team_name"],
            offers[0].team_name
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_prefers_regular_team_outside_special_phase() {
        let base_dir = create_test_career_dir("load_regular_team_outside_special_phase");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let regular_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("regular contract")
                .expect("player regular contract");
        let special_team = team_queries::get_teams_by_category(&db.conn, "endurance")
            .expect("special teams")
            .into_iter()
            .next()
            .expect("endurance team");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");

        let special_contract = contract_queries::generate_especial_contract(
            next_id(&db.conn, IdType::Contract).expect("special contract id"),
            &player.id,
            &player.nome,
            &special_team.id,
            &special_team.nome,
            TeamRole::Numero1,
            "endurance",
            special_team.classe.as_deref().unwrap_or("gt4"),
            season.numero,
        );
        contract_queries::insert_contract(&db.conn, &special_contract).expect("insert special");
        driver_queries::update_driver_especial_category(&db.conn, &player.id, Some("endurance"))
            .expect("set special category");
        team_queries::update_team_pilots(&db.conn, &special_team.id, Some(&player.id), None)
            .expect("set special lineup");
        season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::BlocoRegular)
            .expect("keep regular phase");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let player_team = career.player_team.as_ref().expect("player team");

        assert_eq!(player_team.id, regular_contract.equipe_id);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_repairs_duplicate_regular_contract_state() {
        let base_dir = create_test_career_dir("repair_duplicate_regular_contract_state");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.atributos.skill = 99.0;
        player.categoria_atual = Some("gt4".to_string());
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        let original_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("original contract")
                .expect("player regular contract");
        let replacement_team = team_queries::get_teams_by_category(&db.conn, "mazda_rookie")
            .expect("rookie teams")
            .into_iter()
            .find(|team| team.id != original_contract.equipe_id)
            .expect("replacement team");
        let displaced_contract =
            contract_queries::get_active_contracts_for_team(&db.conn, &replacement_team.id)
                .expect("replacement contracts")
                .into_iter()
                .find(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
                .expect("regular driver to displace");
        contract_queries::update_contract_status(
            &db.conn,
            &displaced_contract.id,
            &ContractStatus::Rescindido,
        )
        .expect("rescind replacement seat");
        db.conn
            .execute_batch("DROP INDEX IF EXISTS idx_contracts_active_pilot_tipo;")
            .expect("drop active-contract uniqueness guard for corruption scenario");

        let mut replacement_contract = crate::models::contract::Contract::new(
            next_id(&db.conn, IdType::Contract).expect("replacement contract id"),
            player.id.clone(),
            player.nome.clone(),
            replacement_team.id.clone(),
            replacement_team.nome.clone(),
            original_contract.temporada_inicio,
            2,
            250_000.0,
            TeamRole::Numero1,
            replacement_team.categoria.clone(),
        );
        replacement_contract.created_at = "9999-12-31T23:59:59".to_string();
        contract_queries::insert_contract(&db.conn, &replacement_contract)
            .expect("insert replacement contract");

        let gt4_team = team_queries::get_teams_by_category(&db.conn, "gt4")
            .expect("gt4 teams")
            .into_iter()
            .next()
            .expect("gt4 team");
        team_queries::update_team_pilots(
            &db.conn,
            &gt4_team.id,
            Some(&player.id),
            gt4_team.piloto_2_id.as_deref(),
        )
        .expect("corrupt gt4 lineup");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
        let refreshed_player =
            driver_queries::get_player_driver(&refreshed_db.conn).expect("player");
        let active_regular_contracts =
            contract_queries::get_contracts_for_pilot(&refreshed_db.conn, &player.id)
                .expect("player contracts")
                .into_iter()
                .filter(|contract| {
                    contract.status == ContractStatus::Ativo
                        && contract.tipo == crate::models::enums::ContractType::Regular
                })
                .collect::<Vec<_>>();
        let original_contract_after =
            contract_queries::get_contract_by_id(&refreshed_db.conn, &original_contract.id)
                .expect("query original contract")
                .expect("original contract exists");
        let refreshed_replacement_team =
            team_queries::get_team_by_id(&refreshed_db.conn, &replacement_team.id)
                .expect("query replacement team")
                .expect("replacement team");
        let refreshed_gt4_team = team_queries::get_team_by_id(&refreshed_db.conn, &gt4_team.id)
            .expect("query gt4 team")
            .expect("gt4 team");
        let player_team = career.player_team.as_ref().expect("player team");

        assert_eq!(player_team.id, replacement_team.id);
        assert_eq!(active_regular_contracts.len(), 1);
        assert_eq!(active_regular_contracts[0].id, replacement_contract.id);
        assert_eq!(original_contract_after.status, ContractStatus::Rescindido);
        assert_eq!(
            refreshed_player.categoria_atual.as_deref(),
            Some(replacement_team.categoria.as_str())
        );
        assert!(
            refreshed_gt4_team.piloto_1_id.as_deref() != Some(player.id.as_str())
                && refreshed_gt4_team.piloto_2_id.as_deref() != Some(player.id.as_str())
        );
        assert!(
            refreshed_replacement_team.piloto_1_id.as_deref() == Some(player.id.as_str())
                || refreshed_replacement_team.piloto_2_id.as_deref() == Some(player.id.as_str())
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_next_race_briefing_summarizes_track_history() {
        let base_dir = create_test_career_dir("load_briefing_track_history");
        let career_id = "career_001";
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let calendar =
            calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie").expect("calendar");
        let race_one = calendar.first().expect("race one");
        let race_two = calendar.get(1).expect("race two");

        db.conn
            .execute(
                "UPDATE calendar SET track_name = ?1 WHERE id IN (?2, ?3)",
                rusqlite::params!["Pista Espelho", race_one.id, race_two.id],
            )
            .expect("update track names");

        let race_result = crate::commands::race::simulate_race_weekend_in_base_dir(
            &base_dir,
            career_id,
            &race_one.id,
        )
        .expect("simulate race");
        let player_finish = race_result
            .player_race
            .race_results
            .iter()
            .find(|entry| entry.is_jogador)
            .map(|entry| entry.finish_position)
            .expect("player finish");
        let player_dnf = race_result
            .player_race
            .race_results
            .iter()
            .find(|entry| entry.is_jogador)
            .map(|entry| entry.is_dnf)
            .expect("player dnf flag");

        let career = load_career_in_base_dir(&base_dir, career_id).expect("load career");
        let track_history = career
            .next_race_briefing
            .as_ref()
            .and_then(|briefing| briefing.track_history.as_ref())
            .expect("track history");

        assert!(track_history.has_data);
        assert_eq!(track_history.starts, 1);
        assert_eq!(track_history.best_finish, Some(player_finish));
        assert_eq!(track_history.last_finish, Some(player_finish));
        assert_eq!(track_history.dnfs, if player_dnf { 1 } else { 0 });
        assert_eq!(track_history.last_visit_season, Some(1));
        assert_eq!(track_history.last_visit_round, Some(1));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_next_race_briefing_exposes_primary_rival() {
        let base_dir = create_test_career_dir("load_briefing_primary_rival");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let rival_driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
            .expect("category drivers")
            .into_iter()
            .find(|driver| !driver.is_jogador)
            .expect("ai rival");

        db.conn
            .execute(
                "UPDATE drivers SET temp_pontos = 90.0, temp_vitorias = 3, temp_podios = 4 WHERE id = ?1",
                rusqlite::params![player.id],
            )
            .expect("update player");
        db.conn
            .execute(
                "UPDATE drivers SET temp_pontos = 96.0, temp_vitorias = 4, temp_podios = 5 WHERE id = ?1",
                rusqlite::params![rival_driver.id],
            )
            .expect("update rival");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let rival = career
            .next_race_briefing
            .as_ref()
            .and_then(|briefing| briefing.primary_rival.as_ref())
            .expect("primary rival");

        assert_eq!(rival.driver_id, rival_driver.id);
        assert_eq!(rival.driver_name, rival_driver.nome);
        assert_eq!(rival.championship_position, 1);
        assert_eq!(rival.gap_points, 6);
        assert!(rival.is_ahead);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_next_race_briefing_filters_weekend_stories() {
        let base_dir = create_test_career_dir("load_briefing_weekend_stories");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");

        news_queries::insert_news_batch(
            &db.conn,
            &vec![
                NewsItem {
                    id: "BRF001".to_string(),
                    tipo: NewsType::Rivalidade,
                    icone: "R".to_string(),
                    titulo: "Duelo esquenta a abertura".to_string(),
                    texto: "A tensao entre os protagonistas cresce antes da etapa de abertura."
                        .to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Destaque,
                    timestamp: 300,
                    driver_id: Some("P001".to_string()),
                    driver_id_secondary: Some("P002".to_string()),
                    team_id: None,
                },
                NewsItem {
                    id: "BRF002".to_string(),
                    tipo: NewsType::Hierarquia,
                    icone: "H".to_string(),
                    titulo: "Equipe reavalia ordem interna".to_string(),
                    texto: "O box chega atento ao equilibrio interno antes da largada.".to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 250,
                    driver_id: Some("P001".to_string()),
                    driver_id_secondary: None,
                    team_id: None,
                },
                NewsItem {
                    id: "BRF003".to_string(),
                    tipo: NewsType::Corrida,
                    icone: "C".to_string(),
                    titulo: "Abertura promete grid apertado".to_string(),
                    texto:
                        "A etapa de abertura deve embaralhar o pelotao logo nas primeiras voltas."
                            .to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("mazda_rookie".to_string()),
                    categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                    importancia: NewsImportance::Alta,
                    timestamp: 200,
                    driver_id: Some("P001".to_string()),
                    driver_id_secondary: None,
                    team_id: None,
                },
                NewsItem {
                    id: "BRF004".to_string(),
                    tipo: NewsType::Corrida,
                    icone: "X".to_string(),
                    titulo: "Outra categoria movimenta a semana".to_string(),
                    texto: "Essa noticia nao deve entrar na previa da etapa do jogador."
                        .to_string(),
                    rodada: Some(1),
                    semana_pretemporada: None,
                    temporada: season.numero,
                    categoria_id: Some("gt4".to_string()),
                    categoria_nome: Some("GT4".to_string()),
                    importancia: NewsImportance::Destaque,
                    timestamp: 400,
                    driver_id: None,
                    driver_id_secondary: None,
                    team_id: None,
                },
            ],
        )
        .expect("seed news");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let stories = &career
            .next_race_briefing
            .as_ref()
            .expect("briefing")
            .weekend_stories;

        assert_eq!(stories.len(), 3);
        assert_eq!(stories[0].title, "Duelo esquenta a abertura");
        assert!(stories
            .iter()
            .all(|story| !story.title.contains("Outra categoria")));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_load_career_invalid_id() {
        let base_dir = unique_test_dir("load_invalid");
        fs::create_dir_all(&base_dir).expect("base dir");

        let error = load_career_in_base_dir(&base_dir, "career_999").expect_err("should fail");
        assert!(error.contains("Save nao encontrado"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_list_saves_format() {
        let base_dir = create_test_career_dir("list_saves");
        let saves = list_saves_in_base_dir(&base_dir).expect("list saves");

        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].career_id, "career_001");
        assert_eq!(saves[0].player_name, "Joao Silva");
        assert_eq!(saves[0].category, "mazda_rookie");
        assert_eq!(saves[0].season, 1);
        assert!(saves[0].total_races > 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_drivers_by_category_returns_ordered_standings() {
        let base_dir = create_test_career_dir("drivers_by_category");
        let standings =
            get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
                .expect("driver standings");

        assert_eq!(standings.len(), 12);
        assert_eq!(standings[0].posicao_campeonato, 1);
        assert!(standings
            .windows(2)
            .all(|window| window[0].pontos >= window[1].pontos));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_drivers_by_category_marks_rookies() {
        let base_dir = create_test_career_dir("drivers_rookie_marker");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");

        let mut rookie = driver_queries::get_player_driver(&db.conn).expect("player");
        rookie.stats_carreira.corridas = 0;
        rookie.stats_carreira.temporadas = 0;
        rookie.temporadas_na_categoria = 0;
        driver_queries::update_driver(&db.conn, &rookie).expect("update rookie");

        let mut veteran = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
            .expect("drivers")
            .into_iter()
            .find(|driver| !driver.is_jogador)
            .expect("non-player driver");
        veteran.stats_carreira.corridas = 12;
        veteran.stats_carreira.temporadas = 1;
        veteran.temporadas_na_categoria = 0;
        driver_queries::update_driver(&db.conn, &veteran).expect("update veteran");

        let tx = db.conn.unchecked_transaction().expect("injury tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &crate::models::injury::Injury {
                id: "I_TEST_LIGHT".to_string(),
                pilot_id: rookie.id.clone(),
                injury_type: crate::models::enums::InjuryType::Moderada,
                injury_name: "Braço machucado".to_string(),
                modifier: 0.85,
                races_total: 2,
                races_remaining: 2,
                skill_penalty: 0.1,
                season: 1,
                race_occurred: "R001".to_string(),
                active: true,
            },
        )
        .expect("insert rookie injury");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &crate::models::injury::Injury {
                id: "I_TEST_GRAVE".to_string(),
                pilot_id: veteran.id.clone(),
                injury_type: crate::models::enums::InjuryType::Grave,
                injury_name: "Braço fraturado".to_string(),
                modifier: 0.65,
                races_total: 4,
                races_remaining: 4,
                skill_penalty: 0.25,
                season: 1,
                race_occurred: "R002".to_string(),
                active: true,
            },
        )
        .expect("insert veteran injury");
        tx.commit().expect("commit injuries");
        drop(db);

        let standings =
            get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
                .expect("driver standings");
        let rookie_entry = standings
            .iter()
            .find(|entry| entry.id == rookie.id)
            .expect("rookie entry");
        let veteran_entry = standings
            .iter()
            .find(|entry| entry.id == veteran.id)
            .expect("veteran entry");

        assert!(rookie_entry.is_estreante);
        assert!(rookie_entry.is_estreante_da_vida);
        assert!(veteran_entry.is_estreante);
        assert!(!veteran_entry.is_estreante_da_vida);
        assert_eq!(rookie_entry.lesao_ativa_tipo.as_deref(), Some("Moderada"));
        assert_eq!(veteran_entry.lesao_ativa_tipo.as_deref(), Some("Grave"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_drivers_by_category_excludes_non_participants_once_category_has_results() {
        let base_dir = create_test_career_dir("drivers_exclude_non_participants");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");

        let mut outsider = crate::models::driver::Driver::new(
            "P_OUTSIDER".to_string(),
            "Miguel Garcia".to_string(),
            "br".to_string(),
            "M".to_string(),
            19,
            2025,
        );
        outsider.categoria_atual = Some("mazda_rookie".to_string());
        driver_queries::insert_driver(&db.conn, &outsider).expect("insert outsider");

        let participant = driver_queries::get_player_driver(&db.conn).expect("player");
        let participant_team = find_player_team(
            &db.conn,
            &participant.id,
            crate::models::enums::SeasonPhase::BlocoRegular,
        )
        .expect("player team")
        .expect("active player team");
        let race_id: String = db
            .conn
            .query_row(
                "SELECT id FROM calendar WHERE categoria = 'mazda_rookie' ORDER BY rodada ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("mazda rookie calendar race");
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, 1, 25.0)",
                rusqlite::params![race_id, participant.id, participant_team.id],
            )
            .expect("seed participant race result");
        drop(db);

        let standings =
            get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
                .expect("driver standings");

        assert!(
            standings.iter().all(|entry| entry.id != "P_OUTSIDER"),
            "driver without season participation should not appear once the category already has race results"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_open_career_repairs_regular_category_vacancies() {
        let base_dir = create_test_career_dir("repair_regular_vacancies");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
            .expect("toyota teams")
            .into_iter()
            .next()
            .expect("toyota team");
        let removed_driver = team
            .piloto_2_id
            .clone()
            .expect("test team should have second driver");
        let removed_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &removed_driver)
                .expect("contract query")
                .expect("active contract");

        contract_queries::update_contract_status(
            &db.conn,
            &removed_contract.id,
            &ContractStatus::Rescindido,
        )
        .expect("rescind contract");
        team_queries::update_team_pilots(&db.conn, &team.id, team.piloto_1_id.as_deref(), None)
            .expect("clear team slot");
        drop(db);

        load_career_in_base_dir(&base_dir, "career_001").expect("load career should repair");
        let standings =
            get_drivers_by_category_in_base_dir(&base_dir, "career_001", "toyota_rookie")
                .expect("driver standings after repair");
        let db = Database::open_existing(&db_path).expect("db after repair");
        let empty_slots: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE categoria = 'toyota_rookie'
                   AND ativa = 1
                   AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
                [],
                |row| row.get(0),
            )
            .expect("empty slots");
        let active_contracts: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT piloto_id)
                 FROM contracts
                 WHERE categoria = 'toyota_rookie'
                   AND tipo = 'Regular'
                   AND status = 'Ativo'",
                [],
                |row| row.get(0),
            )
            .expect("active contracts");

        assert_eq!(standings.len(), 12);
        assert_eq!(empty_slots, 0);
        assert_eq!(active_contracts, 12);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_open_career_repairs_regular_contracts_in_special_categories() {
        let base_dir = create_test_career_dir("repair_regular_special_contract");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let special_team = team_queries::get_teams_by_category(&db.conn, "endurance")
            .expect("endurance teams")
            .into_iter()
            .next()
            .expect("endurance team");
        let mut driver = crate::models::driver::Driver::new(
            "P_BAD_SPECIAL".to_string(),
            "Regular Especial".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2024,
        );
        driver.categoria_atual = Some("endurance".to_string());
        driver_queries::insert_driver(&db.conn, &driver).expect("insert driver");
        let bad_contract = crate::models::contract::Contract::new(
            "C_BAD_SPECIAL".to_string(),
            driver.id.clone(),
            driver.nome.clone(),
            special_team.id.clone(),
            special_team.nome.clone(),
            1,
            3,
            100_000.0,
            TeamRole::Numero1,
            "endurance".to_string(),
        );
        contract_queries::insert_contract(&db.conn, &bad_contract).expect("insert bad contract");
        team_queries::update_team_pilots(&db.conn, &special_team.id, Some(&driver.id), None)
            .expect("assign special team");
        mark_regular_races_completed(&db);
        drop(db);

        load_career_in_base_dir(&base_dir, "career_001").expect("load career should repair");

        let repaired_db = Database::open_existing(&db_path).expect("db after repair");
        let repaired_contract =
            contract_queries::get_contract_by_id(&repaired_db.conn, "C_BAD_SPECIAL")
                .expect("contract query")
                .expect("contract");
        let repaired_driver =
            driver_queries::get_driver(&repaired_db.conn, "P_BAD_SPECIAL").expect("driver");

        assert_eq!(repaired_contract.status, ContractStatus::Rescindido);
        assert_ne!(
            repaired_driver.categoria_atual.as_deref(),
            Some("endurance")
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_open_career_does_not_fill_regular_vacancies_after_regular_block_ends() {
        let base_dir = create_test_career_dir("skip_regular_repair_after_block_end");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
            .expect("toyota teams")
            .into_iter()
            .find(|candidate| candidate.piloto_2_id.is_some())
            .expect("toyota team with second driver");
        let removed_driver = team
            .piloto_2_id
            .clone()
            .expect("test team should have second driver");
        let removed_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &removed_driver)
                .expect("contract query")
                .expect("active contract");

        contract_queries::update_contract_status(
            &db.conn,
            &removed_contract.id,
            &ContractStatus::Rescindido,
        )
        .expect("rescind contract");
        team_queries::update_team_pilots(&db.conn, &team.id, team.piloto_1_id.as_deref(), None)
            .expect("clear team slot");
        mark_regular_races_completed(&db);
        drop(db);

        load_career_in_base_dir(&base_dir, "career_001")
            .expect("load career should repair state without hiring replacement");
        let db = Database::open_existing(&db_path).expect("db after load");
        let empty_slots: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE categoria = 'toyota_rookie'
                   AND ativa = 1
                   AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
                [],
                |row| row.get(0),
            )
            .expect("empty slots");
        let active_contracts: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT piloto_id)
                 FROM contracts
                 WHERE categoria = 'toyota_rookie'
                   AND tipo = 'Regular'
                   AND status = 'Ativo'",
                [],
                |row| row.get(0),
            )
            .expect("active contracts");
        let removed_driver_after =
            driver_queries::get_driver(&db.conn, &removed_driver).expect("removed driver");

        assert_eq!(empty_slots, 1);
        assert_eq!(active_contracts, 11);
        assert_eq!(removed_driver_after.categoria_atual, None);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_open_career_does_not_fill_regular_vacancies_during_preseason() {
        let base_dir = create_test_career_dir("skip_regular_repair_during_preseason");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let baseline_empty_slots: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE categoria = 'toyota_rookie'
                   AND ativa = 1
                   AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
                [],
                |row| row.get(0),
            )
            .expect("baseline empty slots");
        let baseline_active_contracts: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT piloto_id)
                 FROM contracts
                 WHERE categoria = 'toyota_rookie'
                   AND tipo = 'Regular'
                   AND status = 'Ativo'",
                [],
                |row| row.get(0),
            )
            .expect("baseline active contracts");
        let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
            .expect("toyota teams")
            .into_iter()
            .find(|candidate| candidate.piloto_2_id.is_some())
            .expect("toyota team with second driver");
        let removed_driver = team
            .piloto_2_id
            .clone()
            .expect("test team should have second driver");
        let removed_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &removed_driver)
                .expect("contract query")
                .expect("active contract");

        contract_queries::update_contract_status(
            &db.conn,
            &removed_contract.id,
            &ContractStatus::Rescindido,
        )
        .expect("rescind contract");
        team_queries::update_team_pilots(&db.conn, &team.id, team.piloto_1_id.as_deref(), None)
            .expect("clear team slot");
        let expected_empty_slots = baseline_empty_slots + 1;
        let expected_active_contracts = baseline_active_contracts - 1;
        drop(db);

        get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");

        let db = Database::open_existing(&db_path).expect("db after load");
        let empty_slots: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                 FROM teams
                 WHERE categoria = 'toyota_rookie'
                   AND ativa = 1
                   AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
                [],
                |row| row.get(0),
            )
            .expect("empty slots");
        let active_contracts: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT piloto_id)
                 FROM contracts
                 WHERE categoria = 'toyota_rookie'
                   AND tipo = 'Regular'
                   AND status = 'Ativo'",
                [],
                |row| row.get(0),
            )
            .expect("active contracts");

        assert_eq!(empty_slots, expected_empty_slots);
        assert_eq!(active_contracts, expected_active_contracts);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_open_career_preserves_player_team_when_regular_roles_are_duplicated() {
        let base_dir = create_test_career_dir("preserve_player_team_on_duplicate_roles");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let player_team = find_player_team(&db.conn, &player.id, season.fase)
            .expect("player team lookup")
            .expect("player team");
        let teammate_id = player_team
            .piloto_1_id
            .clone()
            .filter(|id| id != &player.id)
            .or_else(|| {
                player_team
                    .piloto_2_id
                    .clone()
                    .filter(|id| id != &player.id)
            })
            .expect("teammate id");
        let teammate_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &teammate_id)
                .expect("teammate contract query")
                .expect("teammate active contract");
        let player_contract =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("player contract query")
                .expect("player active contract");

        db.conn
            .execute(
                "UPDATE contracts SET papel = 'Numero2', created_at = '9999-12-31T23:59:59' WHERE id = ?1",
                rusqlite::params![&teammate_contract.id],
            )
            .expect("force duplicated role on teammate contract");
        db.conn
            .execute(
                "UPDATE contracts SET papel = 'Numero2', created_at = '2020-01-01T00:00:00' WHERE id = ?1",
                rusqlite::params![&player_contract.id],
            )
            .expect("force duplicated role on player contract");
        drop(db);

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let repaired_db = Database::open_existing(&db_path).expect("db after repair");
        let repaired_player_contract =
            contract_queries::get_active_regular_contract_for_pilot(&repaired_db.conn, &player.id)
                .expect("player contract query after repair")
                .expect("player contract should remain active");
        let repaired_team = team_queries::get_team_by_id(&repaired_db.conn, &player_team.id)
            .expect("team query after repair")
            .expect("player team after repair");
        let repaired_teammate_contract = contract_queries::get_active_regular_contract_for_pilot(
            &repaired_db.conn,
            &teammate_id,
        )
        .expect("teammate contract query after repair")
        .expect("teammate contract should remain active");

        assert_eq!(career.player.id, player.id);
        assert_eq!(repaired_player_contract.equipe_id, player_team.id);
        assert_eq!(repaired_player_contract.papel, TeamRole::Numero2);
        assert_eq!(repaired_teammate_contract.equipe_id, player_team.id);
        assert_eq!(repaired_teammate_contract.papel, TeamRole::Numero1);
        assert_eq!(
            repaired_team.piloto_2_id.as_deref(),
            Some(player.id.as_str())
        );
        assert_eq!(
            repaired_team.piloto_1_id.as_deref(),
            Some(teammate_id.as_str())
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_concurrent_career_loads_serialize_regular_contract_repair() {
        let base_dir = create_test_career_dir("serialize_regular_repair");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let team = team_queries::get_teams_by_category(&db.conn, "toyota_rookie")
            .expect("toyota teams")
            .into_iter()
            .next()
            .expect("toyota team");
        let free_driver = driver_queries::get_all_drivers(&db.conn)
            .expect("drivers")
            .into_iter()
            .find(|driver| driver.categoria_atual.is_none())
            .expect("free driver");
        let mut surplus_contract = crate::models::contract::Contract::new(
            next_id(&db.conn, IdType::Contract).expect("contract id"),
            free_driver.id.clone(),
            free_driver.nome.clone(),
            team.id.clone(),
            team.nome.clone(),
            1,
            1,
            50_000.0,
            TeamRole::Numero2,
            team.categoria.clone(),
        );
        surplus_contract.created_at = "0000-01-01T00:00:00".to_string();
        contract_queries::insert_contract(&db.conn, &surplus_contract)
            .expect("insert surplus contract");
        drop(db);

        let handles = (0..4)
            .map(|_| {
                let base_dir = base_dir.clone();
                std::thread::spawn(move || load_career_in_base_dir(&base_dir, "career_001"))
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("thread should finish").expect("load");
        }

        let db = Database::open_existing(&db_path).expect("db after repair");
        let repaired_contract =
            contract_queries::get_contract_by_id(&db.conn, &surplus_contract.id)
                .expect("surplus query")
                .expect("surplus contract");
        assert_eq!(repaired_contract.status, ContractStatus::Rescindido);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_drivers_by_category_uses_recent_results_fallback_from_driver_record() {
        let base_dir = create_test_career_dir("drivers_recent_fallback");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");

        let mut driver = driver_queries::get_player_driver(&db.conn).expect("player");
        driver.stats_temporada.corridas = 3;
        driver.ultimos_resultados = serde_json::json!([
            { "position": 8, "is_dnf": false },
            { "position": 6, "is_dnf": false },
            { "position": 4, "is_dnf": false }
        ]);
        driver_queries::update_driver(&db.conn, &driver).expect("update driver");

        let results_path = config
            .saves_dir()
            .join("career_001")
            .join("race_results.json");
        if results_path.exists() {
            fs::remove_file(&results_path).expect("remove history file");
        }

        let standings =
            get_drivers_by_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
                .expect("driver standings");
        let player = standings
            .into_iter()
            .find(|entry| entry.is_jogador)
            .expect("player standing");

        let fallback_tail: Vec<i32> = player
            .results
            .iter()
            .flatten()
            .map(|result| result.position)
            .collect();

        assert_eq!(fallback_tail, vec![8, 6, 4]);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_drivers_by_category_keeps_special_standings_after_skip_cleanup() {
        let base_dir = create_test_career_dir("special_standings_after_skip");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        db.conn
            .execute(
                "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
                [],
            )
            .expect("complete regular block");
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
        drop(db);

        crate::commands::race::simulate_special_block_in_base_dir(&base_dir, "career_001")
            .expect("simulate special block");
        let db = Database::open_existing(&db_path).expect("db after special sim");
        crate::convocation::encerrar_bloco_especial(&db.conn).expect("end special block");
        crate::convocation::run_pos_especial(&db.conn).expect("run pos especial");
        drop(db);

        let standings =
            get_drivers_by_category_in_base_dir(&base_dir, "career_001", "production_challenger")
                .expect("production special standings");

        assert!(
            !standings.is_empty(),
            "standings especiais devem continuar visiveis apos o cleanup"
        );
        assert!(
            standings.iter().any(|driver| driver.pontos > 0),
            "standings especiais devem refletir pontos simulados"
        );
        assert!(
            standings
                .iter()
                .any(|driver| driver.results.iter().any(Option::is_some)),
            "standings especiais devem manter resultados por rodada"
        );
        assert!(
            standings
                .iter()
                .any(|driver| driver.classe.as_deref() == Some("bmw")),
            "standings especiais devem carregar a classe/carro do piloto"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_teams_standings_keeps_special_lineup_after_skip_cleanup() {
        let base_dir = create_test_career_dir("special_team_standings_after_skip");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        db.conn
            .execute(
                "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
                [],
            )
            .expect("complete regular block");
        crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
        crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
        drop(db);

        crate::commands::race::simulate_special_block_in_base_dir(&base_dir, "career_001")
            .expect("simulate special block");
        let db = Database::open_existing(&db_path).expect("db after special sim");
        crate::convocation::encerrar_bloco_especial(&db.conn).expect("end special block");
        crate::convocation::run_pos_especial(&db.conn).expect("run pos especial");
        drop(db);

        let standings =
            get_teams_standings_in_base_dir(&base_dir, "career_001", "production_challenger")
                .expect("production team standings");

        assert!(
            !standings.is_empty(),
            "standings de equipes especiais devem continuar visiveis apos o cleanup"
        );
        assert!(
            standings.iter().any(|team| team.pontos > 0),
            "standings de equipes especiais devem refletir pontos simulados"
        );
        assert!(
            standings
                .iter()
                .any(|team| { team.piloto_1_nome.is_some() || team.piloto_2_nome.is_some() }),
            "standings de equipes especiais devem preservar os pilotos pelo historico de corrida"
        );
        assert!(
            standings
                .iter()
                .any(|team| team.classe.as_deref() == Some("bmw")),
            "standings de equipes especiais devem carregar a classe/carro da equipe"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_teams_standings_returns_category_grid() {
        let base_dir = create_test_career_dir("teams_standings");
        let standings = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
            .expect("team standings");

        assert_eq!(standings.len(), 6);
        assert_eq!(standings[0].posicao, 1);
        assert!(standings[0].founded_year > 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_teams_standings_uses_previous_season_order_before_first_race() {
        let base_dir = create_test_career_dir("teams_standings_previous_order");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let teams = team_queries::get_teams_by_category(&db.conn, "mazda_rookie").expect("teams");
        let first_team = teams.first().expect("first team");
        let second_team = teams.get(1).expect("second team");

        db.conn
            .execute(
                "UPDATE seasons SET numero = 2, ano = 2026 WHERE status = 'EmAndamento'",
                [],
            )
            .expect("move active season");
        db.conn
            .execute(
                "INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
                 VALUES ('S_PREV_TEAM_ORDER', 1, 2025, 'Finalizada', 8, 'PosEspecial', '', '')",
                [],
            )
            .expect("insert previous season");
        db.conn
            .execute(
                "INSERT INTO drivers (id, nome, idade, nacionalidade, genero)
                 VALUES
                    ('P_PREV_LOW', 'Piloto Anterior Baixo', 24, 'Brasil', 'M'),
                    ('P_PREV_HIGH', 'Piloto Anterior Alto', 26, 'Brasil', 'M')",
                [],
            )
            .expect("insert previous drivers");
        db.conn
            .execute(
                "INSERT INTO standings (
                    temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
                 ) VALUES
                    ('S_PREV_TEAM_ORDER', 'P_PREV_LOW', ?1, 'mazda_rookie', 2, 12, 0, 0, 0, 8),
                    ('S_PREV_TEAM_ORDER', 'P_PREV_HIGH', ?2, 'mazda_rookie', 1, 88, 4, 6, 0, 8)",
                rusqlite::params![&first_team.id, &second_team.id],
            )
            .expect("insert previous standings");

        let standings = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
            .expect("team standings");

        assert_eq!(standings[0].id, second_team.id);
        assert_eq!(standings[0].posicao, 1);
        assert_eq!(standings[1].id, first_team.id);
        assert_eq!(standings[1].posicao, 2);
        assert_eq!(
            standings[0].pontos, 0,
            "temporada atual ainda deve estar zerada"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_team_history_dossier_uses_real_race_results_for_any_team() {
        let base_dir = create_test_career_dir("team_history_dossier_real_results");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");

        let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
            .expect("team standings");
        let selected = teams.first().expect("selected team");
        let rival = teams.get(1).expect("rival team");
        let (selected_driver_1, selected_driver_2) =
            team_driver_ids(&db.conn, &selected.id).expect("selected drivers");
        let (rival_driver_1, _) = team_driver_ids(&db.conn, &rival.id).expect("rival drivers");
        let race_ids: Vec<String> = db
            .conn
            .prepare(
                "SELECT id FROM calendar
                 WHERE categoria = 'mazda_rookie'
                 ORDER BY rodada ASC
                 LIMIT 4",
            )
            .expect("prepare races")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query races")
            .collect::<Result<Vec<_>, _>>()
            .expect("race ids");

        db.conn
            .execute("DELETE FROM race_results", [])
            .expect("clear race results");
        for (race_id, driver_id, team_id, finish, points) in [
            (&race_ids[0], &selected_driver_1, &selected.id, 1, 25.0),
            (&race_ids[0], &selected_driver_2, &selected.id, 4, 12.0),
            (&race_ids[0], &rival_driver_1, &rival.id, 2, 18.0),
            (&race_ids[1], &selected_driver_1, &selected.id, 2, 18.0),
            (&race_ids[1], &selected_driver_2, &selected.id, 5, 10.0),
            (&race_ids[1], &rival_driver_1, &rival.id, 1, 25.0),
            (&race_ids[2], &selected_driver_1, &selected.id, 8, 4.0),
            (&race_ids[2], &selected_driver_2, &selected.id, 9, 2.0),
            (&race_ids[2], &rival_driver_1, &rival.id, 1, 25.0),
            (&race_ids[3], &selected_driver_1, &selected.id, 3, 15.0),
            (&race_ids[3], &selected_driver_2, &selected.id, 6, 8.0),
            (&race_ids[3], &rival_driver_1, &rival.id, 1, 25.0),
        ] {
            db.conn
                .execute(
                    "INSERT INTO race_results (
                        race_id, piloto_id, equipe_id, posicao_final, pontos
                    ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![race_id, driver_id, team_id, finish, points],
                )
                .expect("insert race result");
        }
        db.conn
            .execute(
                "UPDATE teams
                 SET cash_balance = ?1,
                     debt_balance = ?2,
                     financial_state = ?3,
                     last_round_income = ?4,
                     last_round_expenses = ?5,
                     last_round_net = ?6,
                     car_performance = ?7,
                     engineering = ?8,
                     facilities = ?9
                 WHERE id = ?10",
                rusqlite::params![
                    4_200_000.0,
                    1_250_000.0,
                    "pressured",
                    380_000.0,
                    510_000.0,
                    -130_000.0,
                    7.4,
                    63.0,
                    58.0,
                    &selected.id,
                ],
            )
            .expect("update real finance snapshot");
        drop(db);

        let dossier = get_team_history_dossier_in_base_dir(
            &base_dir,
            "career_001",
            &selected.id,
            "mazda_rookie",
        )
        .expect("team dossier");

        assert!(dossier.has_history);
        assert_eq!(dossier.record_scope, "Grupo Mazda");
        assert_eq!(dossier.sport.races, 4);
        assert_eq!(dossier.sport.wins, 1);
        assert_eq!(dossier.sport.podiums, 3);
        assert_eq!(dossier.sport.win_rate, "25%");
        assert_eq!(dossier.sport.podium_rate, "75%");
        assert_eq!(dossier.sport.seasons, "1 Temporada");
        assert_eq!(
            dossier.sport.current_streak,
            "1 Temporada seguida no Grupo Mazda"
        );
        assert_eq!(dossier.sport.best_streak, "2 Pódios consecutivos");
        assert!(dossier
            .timeline
            .iter()
            .any(|item| item.text.contains("vitória real")));
        assert_eq!(
            dossier
                .records
                .iter()
                .find(|record| record.label == "Vitórias")
                .map(|record| (record.rank.as_str(), record.value.as_str())),
            Some(("2º", "1"))
        );
        assert_eq!(dossier.identity.origin, "Mazda Rookie");
        assert_eq!(dossier.identity.current, "Mazda Rookie");
        assert_eq!(dossier.identity.profile, "Dominante");
        assert_eq!(dossier.identity.rival.name, rival.nome);
        assert_eq!(dossier.identity.rival.current_category, "Mazda Rookie");
        assert!(dossier
            .identity
            .rival
            .note
            .contains("4 disputas diretas reais"));
        assert_eq!(
            dossier.identity.symbol_driver,
            driver_name(&db_path, &selected_driver_1)
        );
        assert!(dossier
            .identity
            .symbol_driver_detail
            .contains("4 corridas, 1 vitória, 3 pódios"));
        assert_eq!(dossier.management.peak_cash, "R$ 4.200.000");
        assert_eq!(dossier.management.worst_crisis, "R$ 1.250.000 de dívida");
        assert_eq!(dossier.management.healthy_years, "0 Temporadas");
        assert_eq!(dossier.management.operation_health, "Pressionada");
        assert!(dossier.management.efficiency.contains("pts/temporada"));
        assert!(dossier
            .management
            .efficiency_detail
            .contains("média esportiva"));
        assert_eq!(
            dossier.management.biggest_investment,
            "Nível 7 - pacote técnico atual"
        );
        assert!(dossier.management.summary.contains("Pressionada"));

        let _ = fs::remove_dir_all(base_dir);
    }

    fn driver_name(db_path: &Path, driver_id: &str) -> String {
        let db = Database::open_existing(db_path).expect("db");
        db.conn
            .query_row(
                "SELECT nome FROM drivers WHERE id = ?1",
                rusqlite::params![driver_id],
                |row| row.get::<_, String>(0),
            )
            .expect("driver name")
    }

    fn team_driver_ids(
        conn: &rusqlite::Connection,
        team_id: &str,
    ) -> Result<(String, String), rusqlite::Error> {
        conn.query_row(
            "SELECT piloto_1_id, piloto_2_id FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
    }

    #[test]
    fn test_consecutive_team_seasons_up_to_counts_only_current_streak() {
        let mut season_one = crate::models::contract::Contract::new(
            "C001".to_string(),
            "P001".to_string(),
            "Piloto".to_string(),
            "T001".to_string(),
            "Equipe 1".to_string(),
            1,
            1,
            100_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        season_one.status = ContractStatus::Expirado;
        let mut season_two = crate::models::contract::Contract::new(
            "C002".to_string(),
            "P001".to_string(),
            "Piloto".to_string(),
            "T001".to_string(),
            "Equipe 1".to_string(),
            2,
            1,
            110_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        season_two.status = ContractStatus::Expirado;
        let season_three = crate::models::contract::Contract::new(
            "C003".to_string(),
            "P001".to_string(),
            "Piloto".to_string(),
            "T001".to_string(),
            "Equipe 1".to_string(),
            3,
            2,
            120_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        let mut different_team = crate::models::contract::Contract::new(
            "C004".to_string(),
            "P002".to_string(),
            "Piloto 2".to_string(),
            "T001".to_string(),
            "Equipe 1".to_string(),
            1,
            1,
            95_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );
        different_team.status = ContractStatus::Expirado;
        let current_other_team = crate::models::contract::Contract::new(
            "C005".to_string(),
            "P002".to_string(),
            "Piloto 2".to_string(),
            "T002".to_string(),
            "Equipe 2".to_string(),
            3,
            1,
            105_000.0,
            TeamRole::Numero1,
            "gt4".to_string(),
        );

        let veteran_streak =
            consecutive_team_seasons_up_to(&[season_one, season_two, season_three], "T001", 3);
        let newcomer_streak =
            consecutive_team_seasons_up_to(&[different_team, current_other_team], "T002", 3);

        assert_eq!(veteran_streak, Some(3));
        assert_eq!(newcomer_streak, Some(1));
    }

    #[test]
    fn test_get_calendar_for_category_returns_races() {
        let base_dir = create_test_career_dir("calendar_category");
        let races = get_calendar_for_category_in_base_dir(&base_dir, "career_001", "mazda_rookie")
            .expect("calendar");

        assert_eq!(races.len(), 5);
        assert_eq!(races[0].rodada, 1);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_race_results_by_category_returns_round_history_after_simulation() {
        let base_dir = create_test_career_dir("race_history");
        let career_id = "career_001";
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = calendar_queries::get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");

        crate::commands::race::simulate_race_weekend_in_base_dir(
            &base_dir,
            career_id,
            &next_race.id,
        )
        .expect("simulate race");

        let histories =
            get_race_results_by_category_in_base_dir(&base_dir, career_id, "mazda_rookie")
                .expect("race history");

        assert_eq!(histories.len(), 12);
        assert!(histories.iter().all(|history| history.results.len() == 5));
        assert!(histories.iter().any(|history| history.results[0].is_some()));
        assert!(
            histories.iter().any(|history| {
                history
                    .results
                    .iter()
                    .flatten()
                    .any(|result| result.has_fastest_lap)
            }),
            "expected persisted race history to retain the fastest-lap marker",
        );
        assert!(
            histories
                .iter()
                .flat_map(|history| history.results.iter().flatten())
                .all(|result| result.grid_position > 0),
            "expected persisted race history to retain grid positions",
        );
        assert!(
            histories
                .iter()
                .flat_map(|history| history.results.iter().flatten())
                .all(|result| result.positions_gained == result.grid_position - result.position),
            "expected persisted race history to retain positions gained",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_driver_detail_counts_fastest_laps_from_persisted_history() {
        let base_dir = create_test_career_dir("driver_detail_fastest_lap");
        let career_id = "career_001";
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let next_race = calendar_queries::get_next_race(&db.conn, &season.id, "mazda_rookie")
            .expect("next race")
            .expect("pending race");

        let race_result = crate::commands::race::simulate_race_weekend_in_base_dir(
            &base_dir,
            career_id,
            &next_race.id,
        )
        .expect("simulate race");
        let fastest_lap_driver_id = race_result
            .player_race
            .race_results
            .iter()
            .find(|entry| entry.has_fastest_lap)
            .map(|entry| entry.pilot_id.clone())
            .expect("fastest lap driver");

        let detail = get_driver_detail_in_base_dir(&base_dir, career_id, &fastest_lap_driver_id)
            .expect("driver detail");

        assert_eq!(detail.performance.temporada.voltas_rapidas, Some(1));
        assert_eq!(detail.performance.carreira.voltas_rapidas, Some(1));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_previous_champions_returns_empty_for_first_season() {
        let base_dir = create_test_career_dir("previous_champions");
        let champions = get_previous_champions_in_base_dir(&base_dir, "career_001", "mazda_rookie")
            .expect("previous champions");

        assert!(champions.driver_champion_id.is_none());
        assert!(champions.constructor_champions.is_empty());

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_driver_detail_returns_contracted_ai_payload() {
        let base_dir = create_test_career_dir("driver_detail_contracted");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let mut driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
            .expect("drivers")
            .into_iter()
            .find(|candidate| !candidate.is_jogador)
            .expect("ai driver");

        driver.atributos.skill = 97.0;
        driver.atributos.gestao_pneus = 20.0;
        driver.motivacao = 82.0;
        driver.melhor_resultado_temp = Some(2);
        driver.stats_temporada.corridas = 3;
        driver.stats_temporada.pontos = 28.0;
        driver.stats_temporada.vitorias = 1;
        driver.stats_temporada.podios = 2;
        driver.stats_temporada.poles = 0;
        driver.stats_temporada.dnfs = 0;
        driver.stats_carreira.corridas = 9;
        driver.stats_carreira.pontos_total = 84.0;
        driver.stats_carreira.vitorias = 2;
        driver.stats_carreira.podios = 4;
        driver.stats_carreira.poles = 1;
        driver.stats_carreira.dnfs = 1;
        driver.stats_carreira.titulos = 2;
        driver_queries::update_driver(&db.conn, &driver).expect("update driver");

        let contract = contract_queries::get_active_contract_for_pilot(&db.conn, &driver.id)
            .expect("active contract")
            .expect("contract");
        let team = team_queries::get_team_by_id(&db.conn, &contract.equipe_id)
            .expect("team query")
            .expect("team");

        let detail = get_driver_detail_in_base_dir(&base_dir, "career_001", &driver.id)
            .expect("driver detail");
        let detail_json = serde_json::to_value(&detail).expect("serialize driver detail");

        assert_eq!(detail.id, driver.id);
        assert_eq!(detail.nome, driver.nome);
        assert_eq!(detail.status, "ativo");
        assert_eq!(
            detail.equipe_id.as_deref(),
            Some(contract.equipe_id.as_str())
        );
        assert_eq!(detail.equipe_nome.as_deref(), Some(team.nome.as_str()));
        assert_eq!(
            detail.equipe_cor_primaria.as_deref(),
            Some(team.cor_primaria.as_str())
        );
        assert_eq!(
            detail.equipe_cor_secundaria.as_deref(),
            Some(team.cor_secundaria.as_str())
        );
        assert_eq!(detail.papel.as_deref(), Some(contract.papel.as_str()));
        assert!(detail.personalidade_primaria.is_some());
        assert!(detail.personalidade_secundaria.is_some());
        assert_eq!(detail.motivacao, 82);
        assert_eq!(detail.stats_temporada.corridas, 3);
        assert_eq!(detail.stats_temporada.pontos, 28);
        assert_eq!(detail.stats_temporada.melhor_resultado, 2);
        assert_eq!(detail.stats_carreira.corridas, 9);
        assert_eq!(detail.stats_carreira.pontos, 84);
        assert_eq!(
            detail.contrato.as_ref().map(|value| value.anos_restantes),
            Some(contract.anos_restantes(season.numero))
        );
        assert!(detail.tags.iter().any(|tag| {
            tag.attribute_name == "skill"
                && tag.tag_text == "Alien"
                && tag.level == "elite"
                && tag.color == "#bc8cff"
        }));
        assert!(detail.tags.iter().any(|tag| {
            tag.attribute_name == "gestao_pneus" && tag.level == "defeito" && tag.color == "#db6d28"
        }));
        assert!(
            detail_json.get("perfil").is_some(),
            "expected modular profile block"
        );
        assert!(
            detail_json.get("competitivo").is_some(),
            "expected modular competitive block",
        );
        assert!(
            detail_json.get("performance").is_some(),
            "expected modular performance block",
        );
        assert!(
            detail_json.get("leitura_tecnica").is_some(),
            "expected backend technical-reading block",
        );
        assert_eq!(detail.leitura_tecnica.itens.len(), 4);
        assert!(detail
            .leitura_tecnica
            .itens
            .iter()
            .any(|item| item.chave == "velocidade" && item.nivel == "Elite"));
        assert!(
            detail_json.get("forma").is_some(),
            "expected current-form block"
        );
        assert!(
            detail_json.get("trajetoria").is_some(),
            "expected basic career-path block",
        );
        assert_eq!(detail.trajetoria.titulos, 2);
        assert!(detail.trajetoria.foi_campeao);
        assert!(
            detail_json.get("contrato_mercado").is_some(),
            "expected contract-and-market block",
        );
        assert!(
            detail.contrato_mercado.mercado.is_some(),
            "expected market block to be connected for active drivers",
        );
        assert_eq!(
            detail_json.pointer("/performance/temporada/pontos"),
            None,
            "expected points to stop being a primary dossier metric",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_driver_detail_marks_active_driver_without_contract_as_livre() {
        let base_dir = create_test_career_dir("driver_detail_free");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let free_driver = Driver::new(
            "P-LIVRE-001".to_string(),
            "Piloto Livre".to_string(),
            "🇧🇷 Brasileiro".to_string(),
            "M".to_string(),
            27,
            2020,
        );
        driver_queries::insert_driver(&db.conn, &free_driver).expect("insert free driver");

        let detail = get_driver_detail_in_base_dir(&base_dir, "career_001", &free_driver.id)
            .expect("driver detail");
        let detail_json = serde_json::to_value(&detail).expect("serialize driver detail");

        assert_eq!(detail.id, free_driver.id);
        assert_eq!(detail.status, "livre");
        assert!(detail.equipe_id.is_none());
        assert!(detail.equipe_nome.is_none());
        assert!(detail.papel.is_none());
        assert!(detail.contrato.is_none());
        assert_eq!(detail.stats_temporada.melhor_resultado, 0);
        assert_eq!(detail.stats_carreira.melhor_resultado, 0);
        assert_eq!(detail.resumo_atual.veredito, "Estreante");
        assert_eq!(detail.resumo_atual.tom, "info");
        assert!(
            detail_json.get("contrato_mercado").is_some(),
            "expected contract/market block to exist structurally",
        );
        assert!(
            detail_json.pointer("/contrato_mercado/mercado").is_some(),
            "expected market data to be connected even for free active drivers",
        );
        assert!(
            detail_json.get("relacionamentos").is_none()
                || detail_json
                    .get("relacionamentos")
                    .is_some_and(|value| value.is_null()),
            "expected relationships block to stay empty when there is no real data",
        );
        assert!(
            detail_json.get("reputacao").is_none()
                || detail_json
                    .get("reputacao")
                    .is_some_and(|value| value.is_null()),
            "expected reputation block to stay empty when there is no real data",
        );
        assert!(
            detail_json.get("saude").is_none()
                || detail_json
                    .get("saude")
                    .is_some_and(|value| value.is_null()),
            "expected health block to stay empty when there is no real data",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_driver_detail_includes_active_injury_context() {
        let base_dir = create_test_career_dir("driver_detail_active_injury");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season")
            .expect("active season");
        let mut driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
            .expect("drivers")
            .into_iter()
            .find(|candidate| !candidate.is_jogador)
            .expect("ai driver");
        driver.status = crate::models::enums::DriverStatus::Lesionado;
        driver_queries::update_driver(&db.conn, &driver).expect("update injured driver");
        let race = calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie")
            .expect("calendar")
            .into_iter()
            .next()
            .expect("race");

        let tx = db.conn.unchecked_transaction().expect("injury tx");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &crate::models::injury::Injury {
                id: "I-DETAIL-001".to_string(),
                pilot_id: driver.id.clone(),
                injury_type: crate::models::enums::InjuryType::Moderada,
                injury_name: "".to_string(),
                modifier: 0.88,
                races_total: 4,
                races_remaining: 3,
                skill_penalty: 0.10,
                season: season.numero,
                race_occurred: race.id.clone(),
                active: true,
            },
        )
        .expect("insert active injury");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &crate::models::injury::Injury {
                id: "I-DETAIL-002".to_string(),
                pilot_id: driver.id.clone(),
                injury_type: crate::models::enums::InjuryType::Leve,
                injury_name: "Dor no braço".to_string(),
                modifier: 0.95,
                races_total: 2,
                races_remaining: 0,
                skill_penalty: 0.05,
                season: season.numero,
                race_occurred: race.id.clone(),
                active: false,
            },
        )
        .expect("insert light injury history");
        crate::db::queries::injuries::insert_injury(
            &tx,
            &crate::models::injury::Injury {
                id: "I-DETAIL-003".to_string(),
                pilot_id: driver.id.clone(),
                injury_type: crate::models::enums::InjuryType::Grave,
                injury_name: "Braço fraturado".to_string(),
                modifier: 0.75,
                races_total: 8,
                races_remaining: 0,
                skill_penalty: 0.15,
                season: season.numero,
                race_occurred: race.id.clone(),
                active: false,
            },
        )
        .expect("insert grave injury history");
        tx.commit().expect("commit injury");

        let detail = get_driver_detail_in_base_dir(&base_dir, "career_001", &driver.id)
            .expect("driver detail");
        let active_injury = detail
            .saude
            .as_ref()
            .and_then(|health| health.lesao_ativa.as_ref())
            .expect("active injury context");

        assert_eq!(active_injury.tipo, "Moderada");
        assert_eq!(active_injury.nome.as_deref(), Some("Dor forte nas costas"));
        assert_eq!(active_injury.corridas_total, 4);
        assert_eq!(active_injury.corridas_restantes, 3);
        assert_eq!(active_injury.corrida_ocorrida_id, race.id);
        assert_eq!(active_injury.corrida_ocorrida_rodada, Some(race.rodada));
        assert_eq!(
            active_injury.corrida_ocorrida_pista.as_deref(),
            Some(race.track_name.as_str())
        );
        assert_eq!(detail.trajetoria.historico.lesoes.leves, 1);
        assert_eq!(detail.trajetoria.historico.lesoes.moderadas, 1);
        assert_eq!(detail.trajetoria.historico.lesoes.graves, 1);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_advance_season_rejects_pending_races() {
        let base_dir = create_test_career_dir("advance_pending");

        let error =
            advance_season_in_base_dir(&base_dir, "career_001").expect_err("should reject advance");

        assert!(error.contains("corridas pendentes"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_advance_season_rejects_completed_regular_before_special_flow() {
        let base_dir = create_test_career_dir("advance_regular_before_special");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        db.conn
            .execute("UPDATE calendar SET status = 'Concluida'", [])
            .expect("mark calendar completed");

        let error =
            advance_season_in_base_dir(&base_dir, "career_001").expect_err("should reject advance");

        assert!(error.contains("convocacao"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_advance_season_updates_meta_and_creates_next_season() {
        let base_dir = create_test_career_dir("advance_success");
        mark_all_races_completed(&base_dir, "career_001");

        let result = advance_season_in_base_dir(&base_dir, "career_001")
            .expect("advance season should work");

        assert_eq!(result.new_year, 2025);
        assert!(result.preseason_initialized);
        assert!(result.preseason_total_weeks >= 3);

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let active_season = season_queries::get_active_season(&db.conn)
            .expect("active season query")
            .expect("active season");
        let meta = read_save_meta(&config.saves_dir().join("career_001").join("meta.json"))
            .expect("read meta");
        let total_races =
            count_season_calendar_entries(&db.conn, &active_season.id).expect("season race count");
        let distinct_race_ids: i32 = db
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT id) FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1",
                rusqlite::params![&active_season.id],
                |row| row.get(0),
            )
            .expect("distinct race ids");

        assert_eq!(active_season.id, result.new_season_id);
        assert_eq!(active_season.numero, 2);
        assert_eq!(active_season.ano, 2025);
        assert_eq!(meta.current_season, 2);
        assert_eq!(meta.current_year, 2025);
        assert_eq!(meta.total_races, total_races);
        assert!(total_races > 0);
        assert_eq!(distinct_race_ids, total_races);
        assert!(config
            .saves_dir()
            .join("career_001")
            .join("preseason_plan.json")
            .exists());
        let resume_context = read_resume_context(&config.saves_dir().join("career_001"))
            .expect("read resume context")
            .expect("resume context");
        assert_eq!(resume_context.active_view, CareerResumeView::EndOfSeason);
        assert_eq!(
            resume_context
                .end_of_season_result
                .as_ref()
                .map(|snapshot| snapshot.new_year),
            Some(result.new_year)
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_advance_season_succeeds_even_if_resume_context_write_fails() {
        let base_dir = create_test_career_dir("advance_resume_context_failure");
        mark_all_races_completed(&base_dir, "career_001");

        let config = AppConfig::load_or_default(&base_dir);
        let save_dir = config.saves_dir().join("career_001");
        fs::create_dir_all(save_dir.join("resume_context.json"))
            .expect("block resume context path");

        let result = advance_season_in_base_dir(&base_dir, "career_001")
            .expect("advance season should still succeed");

        let db_path = save_dir.join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let active_season = season_queries::get_active_season(&db.conn)
            .expect("active season query")
            .expect("active season");

        assert_eq!(result.new_year, 2025);
        assert_eq!(active_season.numero, 2);
        assert_eq!(active_season.ano, 2025);
        assert!(save_dir.join("resume_context.json").is_dir());

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_preseason_state_returns_initialized_state() {
        let base_dir = create_test_career_dir("preseason_state");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let state =
            get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");

        assert_eq!(state.current_week, 1);
        assert!(!state.is_complete);
        assert!(state.total_weeks >= 3);
        assert!(
            state.current_display_date.is_some(),
            "preseason state should expose a simulation date",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_advance_market_week_updates_plan_state() {
        let base_dir = create_test_career_dir("advance_market_week");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let initial_state =
            get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
        let initial_date = initial_state
            .current_display_date
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .expect("valid initial preseason date");

        let week =
            advance_market_week_in_base_dir(&base_dir, "career_001").expect("advance market week");
        let state =
            get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
        let advanced_date = state
            .current_display_date
            .as_deref()
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .expect("valid advanced preseason date");

        assert_eq!(week.week_number, 1);
        if week.events.iter().any(|event| {
            event.driver_name.is_some()
                && matches!(
                    event.event_type,
                    crate::market::preseason::MarketEventType::TransferCompleted
                        | crate::market::preseason::MarketEventType::ContractRenewed
                        | crate::market::preseason::MarketEventType::RookieSigned
                )
        }) {
            assert!(
                week.events
                    .iter()
                    .any(|event| event.championship_position.is_some()),
                "ao menos uma movimentacao semanal ranqueavel deve carregar posicao para o fechamento visual"
            );
        }
        assert!(state.current_week >= 2 || state.is_complete);
        assert_eq!(
            advanced_date.signed_duration_since(initial_date).num_days(),
            7,
            "advancing the preseason should move the simulated date by one week",
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_preseason_dates_stay_inside_december_to_february_window() {
        let base_dir = create_test_career_dir("preseason_market_window");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let mut state =
            get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");

        loop {
            let current_date = state
                .current_display_date
                .as_deref()
                .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
                .expect("valid preseason date");
            assert!(
                matches!(current_date.month(), 12 | 1 | 2),
                "preseason date {} should stay inside the december-february market window",
                current_date
            );

            if state.is_complete {
                break;
            }

            advance_market_week_in_base_dir(&base_dir, "career_001").expect("advance market week");
            state =
                get_preseason_state_in_base_dir(&base_dir, "career_001").expect("preseason state");
        }

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_preseason_free_agents_payload_keeps_regular_history_when_special_exists() {
        let base_dir = create_test_career_dir("preseason_free_agents_regular_history");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");

        let regular_team = team_queries::get_teams_by_category(&db.conn, "mazda_amador")
            .expect("regular teams")
            .into_iter()
            .next()
            .expect("regular team");
        let special_team = team_queries::get_teams_by_category(&db.conn, "mazda_amador")
            .expect("special entry teams")
            .into_iter()
            .next()
            .expect("special entry team");

        let mut driver = Driver::new(
            "P-PRESEASON-SPECIAL-001".to_string(),
            "Piloto Historico".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            26,
            2021,
        );
        driver.status = DriverStatus::Ativo;
        driver.categoria_atual = Some("mazda_amador".to_string());
        driver_queries::insert_driver(&db.conn, &driver).expect("insert driver");

        let mut regular_contract = crate::models::contract::Contract::new(
            next_id(&db.conn, IdType::Contract).expect("regular contract id"),
            driver.id.clone(),
            driver.nome.clone(),
            regular_team.id.clone(),
            regular_team.nome.clone(),
            2,
            3,
            80_000.0,
            TeamRole::Numero1,
            "mazda_amador".to_string(),
        );
        regular_contract.status = ContractStatus::Expirado;
        regular_contract.created_at = "2026-01-01T08:00:00".to_string();
        contract_queries::insert_contract(&db.conn, &regular_contract).expect("insert regular");

        let mut special_contract = contract_queries::generate_especial_contract(
            next_id(&db.conn, IdType::Contract).expect("special contract id"),
            &driver.id,
            &driver.nome,
            &special_team.id,
            &special_team.nome,
            TeamRole::Numero2,
            "production_challenger",
            "mazda",
            4,
        );
        special_contract.status = ContractStatus::Expirado;
        special_contract.created_at = "2026-06-01T08:00:00".to_string();
        contract_queries::insert_contract(&db.conn, &special_contract).expect("insert special");
        db.conn
            .execute(
                "INSERT OR REPLACE INTO driver_season_archive
                 (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    &driver.id,
                    1,
                    2025,
                    &driver.nome,
                    "mazda_amador",
                    12,
                    95.0,
                    serde_json::json!({
                        "total_pilotos": 20
                    })
                    .to_string()
                ],
            )
            .expect("insert archive");

        let free_agents =
            get_preseason_free_agents_in_base_dir(&base_dir, "career_001").expect("free agents");
        let preview = free_agents
            .into_iter()
            .find(|item| item.driver_id == driver.id)
            .expect("driver preview");

        assert_eq!(preview.categoria, "mazda_amador");
        assert_eq!(
            preview.previous_team_name.as_deref(),
            Some(regular_team.nome.as_str())
        );
        assert_eq!(
            preview.previous_team_color.as_deref(),
            Some(regular_team.cor_primaria.as_str())
        );
        assert_eq!(preview.seasons_at_last_team, 3);
        assert_eq!(preview.total_career_seasons, 3);
        assert_eq!(preview.last_championship_position, Some(12));
        assert_eq!(preview.last_championship_total_drivers, Some(20));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_advance_season_clears_current_standings_results_and_archives_previous_season() {
        let base_dir = create_test_career_dir("advance_archives_recent_results");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");

        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.stats_temporada.corridas = 3;
        player.stats_temporada.pontos = 41.0;
        player.stats_temporada.vitorias = 1;
        player.stats_temporada.podios = 2;
        player.ultimos_resultados = serde_json::json!([
            { "position": 9, "is_dnf": false },
            { "position": 5, "is_dnf": false },
            { "position": 1, "is_dnf": false }
        ]);
        driver_queries::update_driver(&db.conn, &player).expect("update player");

        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");

        let refreshed_player_record = driver_queries::get_player_driver(&db.conn).expect("player");
        let detail_after_advance =
            get_driver_detail_in_base_dir(&base_dir, "career_001", &player.id)
                .expect("driver detail after advance");
        let snapshot_json: String = db
            .conn
            .query_row(
                "SELECT snapshot_json
                 FROM driver_season_archive
                 WHERE piloto_id = ?1 AND season_number = 1",
                rusqlite::params![&player.id],
                |row| row.get(0),
            )
            .expect("archived season snapshot");
        let snapshot: serde_json::Value =
            serde_json::from_str(&snapshot_json).expect("valid snapshot json");

        assert!(
            refreshed_player_record.ultimos_resultados == serde_json::json!([]),
            "new season player record should not keep previous season recent results"
        );
        assert_eq!(
            detail_after_advance.forma.ultimas_5.len(),
            3,
            "driver detail should keep reading recent form from the previous season archive"
        );
        assert_eq!(
            detail_after_advance.forma.ultimas_10.len(),
            3,
            "driver detail should expose archived recent form in the 10-race chart payload"
        );
        assert_eq!(detail_after_advance.forma.ultimas_5[0].chegada, Some(9));
        assert_eq!(detail_after_advance.forma.ultimas_5[1].chegada, Some(5));
        assert_eq!(detail_after_advance.forma.ultimas_5[2].chegada, Some(1));
        assert_eq!(detail_after_advance.forma.media_chegada, Some(5.0));
        assert_eq!(
            refreshed_player_record.stats_temporada.corridas, 0,
            "new season player record should reset season race count"
        );
        assert_eq!(
            snapshot["ultimos_resultados"],
            serde_json::json!([
                { "position": 9, "is_dnf": false },
                { "position": 5, "is_dnf": false },
                { "position": 1, "is_dnf": false }
            ]),
            "snapshot should preserve ultimos_resultados from the archived season"
        );
        assert_eq!(snapshot["corridas"], 3, "snapshot should preserve corridas");
        assert!(
            snapshot["atributos"]["skill"].is_number(),
            "snapshot should include atributos"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_news_filters_by_season_and_type() {
        let base_dir = create_test_career_dir("get_news_filters");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        advance_market_week_in_base_dir(&base_dir, "career_001").expect("advance market week");

        // news generation is now stubbed; just check the query runs without error
        let _ =
            get_news_in_base_dir(&base_dir, "career_001", Some(1), None, Some(50)).expect("news");
        let _ = get_news_in_base_dir(&base_dir, "career_001", Some(2), Some("Mercado"), Some(50))
            .expect("market news");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_news_rejects_invalid_type_filter() {
        let base_dir = create_test_career_dir("get_news_invalid_type");
        let error = get_news_in_base_dir(
            &base_dir,
            "career_001",
            Some(1),
            Some("TipoInvalido"),
            Some(50),
        )
        .expect_err("invalid news type should fail");

        assert!(error.contains("NewsType"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_is_team_role_vacant_rejects_invalid_role() {
        let base_dir = create_test_career_dir("invalid_team_role_vacancy");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");

        let error = is_team_role_vacant(&db.conn, "T001", "PapelInvalido")
            .expect_err("invalid role should fail");

        assert!(error.contains("TeamRole"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_finalize_preseason_rejects_incomplete_plan() {
        let base_dir = create_test_career_dir("finalize_preseason_incomplete");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let error = finalize_preseason_in_base_dir(&base_dir, "career_001")
            .expect_err("should reject incomplete preseason");

        assert!(error.contains("nao foi concluida"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_player_proposals_returns_pending_only() {
        let base_dir = create_test_career_dir("player_proposals_pending_only");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T002", "Recusada");

        let proposals =
            get_player_proposals_in_base_dir(&base_dir, "career_001").expect("player proposals");

        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].status, "Pendente");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_get_player_proposals_enriched_with_team_data() {
        let base_dir = create_test_career_dir("player_proposals_enriched");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

        let proposals =
            get_player_proposals_in_base_dir(&base_dir, "career_001").expect("player proposals");

        assert!(!proposals.is_empty());
        assert!(!proposals[0].equipe_nome.is_empty());
        assert!(!proposals[0].categoria_nome.is_empty());
        assert!(proposals[0].car_performance_rating <= 100);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_accept_proposal_creates_contract_and_expires_other_proposals() {
        let base_dir = create_test_career_dir("accept_proposal");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T002", "Pendente");

        let response =
            respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", true)
                .expect("accept proposal");

        assert!(response.success);
        assert_eq!(response.action, "accepted");

        let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
        let active_contract =
            contract_queries::get_active_contract_for_pilot(&refreshed_db.conn, &player.id)
                .expect("active contract")
                .expect("contract");
        assert_eq!(active_contract.equipe_id, "T001");
        let expired = crate::db::queries::market_proposals::get_market_proposal_by_id(
            &refreshed_db.conn,
            &season.id,
            "MP-T002-P001",
        )
        .expect("proposal query")
        .expect("proposal");
        assert_eq!(expired.status.as_str(), "Expirada");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_accept_proposal_replaces_only_regular_contract_when_special_exists() {
        let base_dir = create_test_career_dir("accept_proposal_with_special_residue");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let special_team = team_queries::get_teams_by_category(&db.conn, "endurance")
            .expect("special teams")
            .into_iter()
            .next()
            .expect("endurance team");

        let special_contract = contract_queries::generate_especial_contract(
            next_id(&db.conn, IdType::Contract).expect("special contract id"),
            &player.id,
            &player.nome,
            &special_team.id,
            &special_team.nome,
            TeamRole::Numero1,
            "endurance",
            special_team.classe.as_deref().unwrap_or("gt4"),
            season.numero,
        );
        contract_queries::insert_contract(&db.conn, &special_contract).expect("insert special");
        driver_queries::update_driver_especial_category(&db.conn, &player.id, Some("endurance"))
            .expect("set special category");
        team_queries::update_team_pilots(&db.conn, &special_team.id, Some(&player.id), None)
            .expect("set special lineup");

        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

        let response =
            respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", true)
                .expect("accept proposal");

        assert!(response.success);

        let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
        let active_regular =
            contract_queries::get_active_regular_contract_for_pilot(&refreshed_db.conn, &player.id)
                .expect("regular contract query")
                .expect("active regular contract");
        let active_regular_count: i64 = refreshed_db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM contracts
                 WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular'",
                rusqlite::params![&player.id],
                |row| row.get(0),
            )
            .expect("count active regular contracts");

        assert_eq!(active_regular.equipe_id, "T001");
        assert_eq!(active_regular_count, 1);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_accept_proposal_to_full_team_replaces_incumbent_instead_of_creating_third_driver() {
        let base_dir = create_test_career_dir("accept_proposal_replaces_full_team_incumbent");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let mut player = driver_queries::get_player_driver(&db.conn).expect("player");
        player.atributos.skill = 1.0;
        driver_queries::update_driver(&db.conn, &player).expect("downgrade player skill");

        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let current_contract = latest_regular_contract_for_driver(&db.conn, &player.id);
        let target_team =
            team_queries::get_teams_by_category(&db.conn, &current_contract.categoria)
                .expect("teams by category")
                .into_iter()
                .find(|team| {
                    if team.id == current_contract.equipe_id
                        || team.piloto_1_id.is_none()
                        || team.piloto_2_id.is_none()
                    {
                        return false;
                    }

                    contract_queries::get_active_contracts_for_team(&db.conn, &team.id)
                        .map(|contracts| {
                            contracts
                                .into_iter()
                                .filter(|contract| {
                                    contract.tipo == crate::models::enums::ContractType::Regular
                                })
                                .count()
                                == 2
                        })
                        .unwrap_or(false)
                })
                .expect("full target team");
        let displaced_driver_id = target_team
            .piloto_1_id
            .clone()
            .expect("full target team should have n1 incumbent");

        seed_player_proposal(
            &db.conn,
            &season.id,
            &player.id,
            &target_team.id,
            "Pendente",
        );

        respond_to_proposal_in_base_dir(
            &base_dir,
            "career_001",
            &format!("MP-{}-{}", target_team.id, player.id),
            true,
        )
        .expect("accept proposal");

        let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
        let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
        let refreshed_target_team =
            team_queries::get_team_by_id(&refreshed_db.conn, &target_team.id)
                .expect("query target team")
                .expect("target team");
        let target_contracts =
            contract_queries::get_active_contracts_for_team(&refreshed_db.conn, &target_team.id)
                .expect("target team contracts")
                .into_iter()
                .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
                .collect::<Vec<_>>();
        let displaced_contract = contract_queries::get_active_regular_contract_for_pilot(
            &refreshed_db.conn,
            &displaced_driver_id,
        )
        .expect("displaced contract query");
        let player_team = career.player_team.as_ref().expect("player team");

        assert_eq!(player_team.id, target_team.id);
        assert_eq!(target_contracts.len(), 2);
        assert!(
            refreshed_target_team.piloto_1_id.as_deref() == Some(player.id.as_str())
                || refreshed_target_team.piloto_2_id.as_deref() == Some(player.id.as_str()),
            "accepted player should remain in the target lineup after consistency repair"
        );
        assert!(
            displaced_contract
                .as_ref()
                .is_none_or(|contract| contract.equipe_id != target_team.id),
            "incumbent displaced from the accepted role should no longer hold an active regular contract for the target team"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_accept_proposal_rejects_team_without_required_license() {
        let base_dir = create_test_career_dir("accept_proposal_without_required_license");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let invalid_team = team_queries::get_teams_by_category(&db.conn, "gt4")
            .expect("gt4 teams")
            .into_iter()
            .next()
            .expect("gt4 team");

        seed_player_proposal(
            &db.conn,
            &season.id,
            &player.id,
            &invalid_team.id,
            "Pendente",
        );

        let error = respond_to_proposal_in_base_dir(
            &base_dir,
            "career_001",
            &format!("MP-{}-{}", invalid_team.id, player.id),
            true,
        )
        .expect_err("accept proposal should fail without required license");

        assert!(error.to_lowercase().contains("licenc"));

        let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
        let active_regular =
            contract_queries::get_active_regular_contract_for_pilot(&refreshed_db.conn, &player.id);
        let active_regular = active_regular.expect("regular contract query");
        assert!(active_regular
            .as_ref()
            .is_none_or(|contract| contract.equipe_id != invalid_team.id));

        let invalid_team_contracts: i64 = refreshed_db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM contracts
                 WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular' AND equipe_id = ?2",
                rusqlite::params![&player.id, &invalid_team.id],
                |row| row.get(0),
            )
            .expect("count invalid team contracts");
        assert_eq!(invalid_team_contracts, 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_accept_proposal_removes_pending_player_events_from_preseason_plan() {
        let base_dir = create_test_career_dir("accept_proposal_clears_pending_player_events");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let save_dir = config.saves_dir().join("career_001");
        let db_path = save_dir.join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let current_contract = latest_regular_contract_for_driver(&db.conn, &player.id);
        let gt4_team = team_queries::get_teams_by_category(&db.conn, "gt4")
            .expect("gt4 teams")
            .into_iter()
            .find(|team| team.id != current_contract.equipe_id)
            .expect("gt4 team");

        seed_player_proposal(
            &db.conn,
            &season.id,
            &player.id,
            &current_contract.equipe_id,
            "Pendente",
        );

        let mut plan = crate::market::preseason::load_preseason_plan(&save_dir)
            .expect("load plan")
            .expect("preseason plan");
        plan.planned_events.push(PlannedEvent {
            week: 2,
            executed: false,
            event: PendingAction::ExpireContract {
                contract_id: current_contract.id.clone(),
                driver_id: player.id.clone(),
                driver_name: player.nome.clone(),
                team_id: current_contract.equipe_id.clone(),
                team_name: current_contract.equipe_nome.clone(),
            },
        });
        plan.planned_events.push(PlannedEvent {
            week: 3,
            executed: false,
            event: PendingAction::Transfer {
                driver_id: player.id.clone(),
                driver_name: player.nome.clone(),
                from_team_id: Some(current_contract.equipe_id.clone()),
                from_team_name: Some(current_contract.equipe_nome.clone()),
                from_categoria: Some(current_contract.categoria.clone()),
                to_team_id: gt4_team.id.clone(),
                to_team_name: gt4_team.nome.clone(),
                salary: 120_000.0,
                duration: 1,
                role: TeamRole::Numero2.as_str().to_string(),
            },
        });
        save_preseason_plan(&save_dir, &plan).expect("save mutated plan");

        let response = respond_to_proposal_in_base_dir(
            &base_dir,
            "career_001",
            &format!("MP-{}-{}", current_contract.equipe_id, player.id),
            true,
        )
        .expect("accept proposal");

        assert!(response.success);

        let plan = crate::market::preseason::load_preseason_plan(&save_dir)
            .expect("reload plan")
            .expect("preseason plan");
        assert!(
            !plan.planned_events.iter().any(|event| {
                !event.executed
                    && matches!(
                        &event.event,
                        PendingAction::ExpireContract { driver_id, .. }
                            | PendingAction::RenewContract { driver_id, .. }
                            | PendingAction::Transfer { driver_id, .. }
                            if driver_id == &player.id
                    )
            }),
            "nenhum evento pendente do jogador deve sobreviver apos aceitar proposta"
        );
        assert!(
            !plan.planned_events.iter().any(|event| {
                !event.executed
                    && matches!(
                        &event.event,
                        PendingAction::PlayerProposal { proposal } if proposal.piloto_id == player.id
                    )
            }),
            "nenhuma proposta futura do jogador deve continuar pendente no plano"
        );

        let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
        let active_regular =
            contract_queries::get_active_regular_contract_for_pilot(&refreshed_db.conn, &player.id)
                .expect("regular contract query")
                .expect("active regular contract");

        assert_eq!(active_regular.equipe_id, current_contract.equipe_id);
        assert_eq!(active_regular.categoria, current_contract.categoria);

        let gt4_contracts: i64 = refreshed_db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM contracts
                 WHERE piloto_id = ?1 AND status = 'Ativo' AND tipo = 'Regular' AND equipe_id = ?2",
                rusqlite::params![&player.id, &gt4_team.id],
                |row| row.get(0),
            )
            .expect("count gt4 contracts");
        assert_eq!(gt4_contracts, 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_accept_proposal_removes_stale_place_rookie_for_accepted_team_role() {
        let base_dir = create_test_career_dir("accept_proposal_clears_backfilled_rookie");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let save_dir = config.saves_dir().join("career_001");
        let db_path = save_dir.join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");

        if contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("active regular contract query")
            .is_none()
        {
            let mut news_items = Vec::new();
            force_place_player(&db.conn, &player, &season, &mut news_items)
                .expect("force place player");
        }

        let current_contract = latest_regular_contract_for_driver(&db.conn, &player.id);
        let target_team =
            team_queries::get_teams_by_category(&db.conn, &current_contract.categoria)
                .expect("teams by category")
                .into_iter()
                .find(|team| team.id != current_contract.equipe_id)
                .expect("target team");
        seed_player_proposal(
            &db.conn,
            &season.id,
            &player.id,
            &target_team.id,
            "Pendente",
        );

        let mut plan = crate::market::preseason::load_preseason_plan(&save_dir)
            .expect("load plan")
            .expect("preseason plan");
        plan.planned_events.push(PlannedEvent {
            week: 4,
            executed: false,
            event: PendingAction::PlaceRookie {
                driver: Driver::new(
                    "P-PLAN-ROOKIE".to_string(),
                    "Rookie de Plano".to_string(),
                    "🇧🇷 Brasileiro".to_string(),
                    "M".to_string(),
                    18,
                    2025,
                ),
                team_id: target_team.id.clone(),
                team_name: target_team.nome.clone(),
                salary: 22_000.0,
                duration: 1,
                role: TeamRole::Numero1.as_str().to_string(),
            },
        });
        save_preseason_plan(&save_dir, &plan).expect("save mutated plan");

        let response = respond_to_proposal_in_base_dir(
            &base_dir,
            "career_001",
            &format!("MP-{}-{}", target_team.id, player.id),
            true,
        )
        .expect("accept proposal");

        assert!(response.success);

        let plan = crate::market::preseason::load_preseason_plan(&save_dir)
            .expect("reload plan")
            .expect("preseason plan");
        assert!(
            !plan.planned_events.iter().any(|event| {
                !event.executed
                    && matches!(
                        &event.event,
                        PendingAction::PlaceRookie { team_id, role, .. }
                            if team_id == &target_team.id
                                && role == TeamRole::Numero1.as_str()
                    )
            }),
            "a vaga preenchida pelo aceite nao deve manter PlaceRookie pendente"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_reject_proposal_marks_recusada_and_generates_news() {
        let base_dir = create_test_career_dir("reject_proposal");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

        let response =
            respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", false)
                .expect("reject proposal");

        assert!(response.success);
        assert_eq!(response.action, "rejected");

        let refreshed_db = Database::open_existing(&db_path).expect("db reopen");
        let proposal = crate::db::queries::market_proposals::get_market_proposal_by_id(
            &refreshed_db.conn,
            &season.id,
            "MP-T001-P001",
        )
        .expect("proposal query")
        .expect("proposal");
        assert_eq!(proposal.status.as_str(), "Recusada");

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_finalize_blocks_with_pending_proposals() {
        let base_dir = create_test_career_dir("finalize_pending_proposals");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");
        force_complete_preseason_plan(&config.saves_dir().join("career_001"));

        let error = finalize_preseason_in_base_dir(&base_dir, "career_001")
            .expect_err("should block pending proposals");

        assert!(error.contains("pendente"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_finalize_allows_player_without_team_when_plan_is_resolved() {
        let base_dir = create_test_career_dir("finalize_without_team");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        if let Some(contract) =
            contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract")
        {
            contract_queries::update_contract_status(
                &db.conn,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )
            .expect("rescind old contract");
            team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
                .expect("remove from team");
        }
        force_complete_preseason_plan(&config.saves_dir().join("career_001"));

        finalize_preseason_in_base_dir(&base_dir, "career_001")
            .expect("should allow advancing even without an active player team");

        let save_dir = config.saves_dir().join("career_001");
        assert!(
            !save_dir.join("preseason_plan.json").exists(),
            "finalizacao deve limpar o plano da pre-temporada"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_finalize_succeeds_when_all_resolved() {
        let base_dir = create_test_career_dir("finalize_success");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let save_dir = config.saves_dir().join("career_001");
        force_complete_preseason_plan(&save_dir);
        persist_resume_context_in_base_dir(
            &base_dir,
            "career_001",
            CareerResumeView::Preseason,
            None,
        )
        .expect("persist preseason resume context");

        finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

        assert!(!save_dir.join("preseason_plan.json").exists());
        assert!(read_resume_context(&save_dir)
            .expect("read resume context")
            .is_none());

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_can_advance_from_second_season_after_finalizing_preseason() {
        let base_dir = create_test_career_dir("advance_second_season");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance to season 2");
        let config = AppConfig::load_or_default(&base_dir);
        let save_dir = config.saves_dir().join("career_001");
        let db_path = save_dir.join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("active season query")
            .expect("active season");
        if contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
            .expect("active regular contract query")
            .is_none()
        {
            let mut news_items = Vec::new();
            force_place_player(&db.conn, &player, &season, &mut news_items)
                .expect("force place player for season 2");
        }

        force_complete_preseason_plan(&save_dir);
        finalize_preseason_in_base_dir(&base_dir, "career_001").expect("finalize preseason");

        mark_all_races_completed(&base_dir, "career_001");
        let result = advance_season_in_base_dir(&base_dir, "career_001")
            .expect("advance to season 3 should work");

        let refreshed_db = Database::open_existing(&db_path).expect("db");
        let active_season = season_queries::get_active_season(&refreshed_db.conn)
            .expect("active season query")
            .expect("active season");

        assert_eq!(result.new_year, 2026);
        assert_eq!(active_season.numero, 3);
        assert_eq!(active_season.ano, 2026);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_skip_all_pending_races_allows_teamless_player_to_reach_next_preseason() {
        let base_dir = create_test_career_dir("skip_teamless_second_season");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance to season 2");
        let config = AppConfig::load_or_default(&base_dir);
        let save_dir = config.saves_dir().join("career_001");
        let db_path = save_dir.join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");

        if let Some(contract) =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active regular contract")
        {
            contract_queries::update_contract_status(
                &db.conn,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )
            .expect("rescind old contract");
            team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
                .expect("remove from team");
        }

        force_complete_preseason_plan(&save_dir);
        finalize_preseason_in_base_dir(&base_dir, "career_001")
            .expect("finalize preseason without team");

        skip_all_pending_races_in_base_dir(&base_dir, "career_001")
            .expect("teamless player should be able to skip season");
        let result = advance_season_in_base_dir(&base_dir, "career_001")
            .expect("advance to season 3 should work after skipping teamless season");

        let refreshed_db = Database::open_existing(&db_path).expect("db");
        let active_season = season_queries::get_active_season(&refreshed_db.conn)
            .expect("active season query")
            .expect("active season");

        assert_eq!(result.new_year, 2026);
        assert_eq!(active_season.numero, 3);
        assert_eq!(active_season.ano, 2026);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_teamless_player_skip_path_keeps_special_grids_assignable() {
        let base_dir = create_test_career_dir("skip_teamless_special_grid");
        mark_all_races_completed(&base_dir, "career_001");

        advance_season_in_base_dir(&base_dir, "career_001").expect("advance to season 2");
        let config = AppConfig::load_or_default(&base_dir);
        let save_dir = config.saves_dir().join("career_001");
        let db_path = save_dir.join("career.db");
        let mut db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");

        if let Some(contract) =
            contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
                .expect("active regular contract")
        {
            contract_queries::update_contract_status(
                &db.conn,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )
            .expect("rescind old contract");
            team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
                .expect("remove from team");
        }

        force_complete_preseason_plan(&save_dir);
        finalize_preseason_in_base_dir(&base_dir, "career_001")
            .expect("finalize preseason without team");

        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        let pending_regular =
            calendar_queries::get_pending_races(&db.conn, &season.id).expect("pending races");
        for race in &pending_regular {
            crate::commands::race::simulate_category_race(&mut db, race, false)
                .expect("simulate regular race while skipping");
        }

        crate::convocation::advance_to_convocation_window(&db.conn)
            .expect("advance to convocation");
        let convocation =
            crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
        assert!(
            convocation.errors.is_empty(),
            "convocation should not report structural errors: {:?}",
            convocation.errors
        );
        crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");

        for category_id in ["production_challenger", "endurance"] {
            let active_drivers =
                driver_queries::get_drivers_by_active_category(&db.conn, category_id)
                    .expect("active special drivers");
            let contracts =
                contract_queries::get_active_especial_contracts_by_category(&db.conn, category_id)
                    .expect("active special contracts");
            let assigned_ids: std::collections::HashSet<String> = contracts
                .iter()
                .map(|contract| contract.piloto_id.clone())
                .collect();
            let orphaned: Vec<String> = active_drivers
                .iter()
                .filter(|driver| !assigned_ids.contains(&driver.id))
                .map(|driver| format!("{} ({})", driver.nome, driver.id))
                .collect();

            assert!(
                orphaned.is_empty(),
                "special category '{}' should not contain drivers without lineup: {}",
                category_id,
                orphaned.join(", ")
            );
        }

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_cannot_accept_already_resolved_proposal() {
        let base_dir = create_test_career_dir("accept_resolved_proposal");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Recusada");

        let error = respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", true)
            .expect_err("should reject resolved proposal");

        assert!(error.contains("nao esta mais pendente"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_player_rejects_all_gets_emergency_proposals() {
        let base_dir = create_test_career_dir("reject_all_emergency");
        mark_all_races_completed(&base_dir, "career_001");
        advance_season_in_base_dir(&base_dir, "career_001").expect("advance season");
        let config = AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
        let player = driver_queries::get_player_driver(&db.conn).expect("player");
        let season = season_queries::get_active_season(&db.conn)
            .expect("season query")
            .expect("active season");
        if let Some(contract) =
            contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
                .expect("active contract")
        {
            contract_queries::update_contract_status(
                &db.conn,
                &contract.id,
                &crate::models::enums::ContractStatus::Rescindido,
            )
            .expect("rescind old contract");
            team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
                .expect("remove from team");
        }
        seed_player_proposal(&db.conn, &season.id, &player.id, "T001", "Pendente");

        let response =
            respond_to_proposal_in_base_dir(&base_dir, "career_001", "MP-T001-P001", false)
                .expect("reject proposal");

        assert_eq!(response.action, "rejected");
        assert!(response.remaining_proposals > 0);

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_briefing_phrase_history_persists_and_keeps_only_last_five_rounds_per_driver_bucket() {
        let base_dir = create_test_career_dir("briefing_phrase_history");
        let career_id = "career_001";

        for round_number in 1..=7 {
            save_briefing_phrase_history_in_base_dir(
                &base_dir,
                career_id,
                1,
                vec![BriefingPhraseEntryInput {
                    round_number,
                    driver_id: "drv-player".to_string(),
                    bucket_key: "p1".to_string(),
                    phrase_id: format!("p1-baseline-{round_number}"),
                }],
            )
            .expect("save phrase history");
        }

        let history =
            get_briefing_phrase_history_in_base_dir(&base_dir, career_id).expect("phrase history");

        assert_eq!(history.season_number, 1);
        assert_eq!(history.entries.len(), 5);
        assert_eq!(
            history
                .entries
                .iter()
                .map(|entry| entry.round_number)
                .collect::<Vec<_>>(),
            vec![7, 6, 5, 4, 3]
        );
        assert!(history
            .entries
            .iter()
            .all(|entry| entry.driver_id == "drv-player" && entry.bucket_key == "p1"));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn test_briefing_phrase_history_resets_when_season_changes() {
        let base_dir = create_test_career_dir("briefing_phrase_history_reset");
        let career_id = "career_001";

        save_briefing_phrase_history_in_base_dir(
            &base_dir,
            career_id,
            1,
            vec![BriefingPhraseEntryInput {
                round_number: 5,
                driver_id: "drv-player".to_string(),
                bucket_key: "p2".to_string(),
                phrase_id: "p2-stable-1".to_string(),
            }],
        )
        .expect("save season one");

        let history = save_briefing_phrase_history_in_base_dir(
            &base_dir,
            career_id,
            2,
            vec![BriefingPhraseEntryInput {
                round_number: 1,
                driver_id: "drv-player".to_string(),
                bucket_key: "p2".to_string(),
                phrase_id: "p2-stable-2".to_string(),
            }],
        )
        .expect("save season two");

        assert_eq!(history.season_number, 2);
        assert_eq!(history.entries.len(), 1);
        assert_eq!(history.entries[0].round_number, 1);
        assert_eq!(history.entries[0].phrase_id, "p2-stable-2");

        let _ = fs::remove_dir_all(base_dir);
    }

    fn create_test_career_dir(label: &str) -> std::path::PathBuf {
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

        let _ = create_career_in_base_dir(&base_dir, input).expect("career should be created");
        base_dir
    }

    fn mark_all_races_completed(base_dir: &Path, career_id: &str) {
        let config = AppConfig::load_or_default(base_dir);
        let db_path = config.saves_dir().join(career_id).join("career.db");
        let db = Database::open_existing(&db_path).expect("db");
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

    fn mark_regular_races_completed(db: &Database) {
        db.conn
            .execute(
                "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
                [],
            )
            .expect("complete regular block");
    }

    fn unique_test_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracerapp_{label}_{nanos}"))
    }

    fn seed_player_proposal(
        conn: &rusqlite::Connection,
        season_id: &str,
        player_id: &str,
        team_id: &str,
        status: &str,
    ) {
        let team = team_queries::get_team_by_id(conn, team_id)
            .expect("team query")
            .expect("team");
        let player = driver_queries::get_driver(conn, player_id).expect("player");
        crate::db::queries::market_proposals::insert_player_proposal(
            conn,
            season_id,
            &crate::market::proposals::MarketProposal {
                id: format!("MP-{team_id}-{player_id}"),
                equipe_id: team.id.clone(),
                equipe_nome: team.nome.clone(),
                piloto_id: player.id.clone(),
                piloto_nome: player.nome.clone(),
                categoria: team.categoria.clone(),
                papel: crate::models::enums::TeamRole::Numero1,
                salario_oferecido: 95_000.0,
                duracao_anos: 2,
                status: match status {
                    "Aceita" => crate::market::proposals::ProposalStatus::Aceita,
                    "Recusada" => crate::market::proposals::ProposalStatus::Recusada,
                    "Expirada" => crate::market::proposals::ProposalStatus::Expirada,
                    _ => crate::market::proposals::ProposalStatus::Pendente,
                },
                motivo_recusa: None,
            },
        )
        .expect("insert player proposal");
    }

    fn force_complete_preseason_plan(save_dir: &Path) {
        let mut plan = crate::market::preseason::load_preseason_plan(save_dir)
            .expect("load plan")
            .expect("plan");
        plan.state.is_complete = true;
        plan.state.current_week = plan.state.total_weeks + 1;
        plan.state.phase = crate::market::preseason::PreSeasonPhase::Complete;
        plan.state.player_has_pending_proposals = false;
        crate::market::preseason::save_preseason_plan(save_dir, &plan).expect("save plan");
    }

    fn latest_regular_contract_for_driver(
        conn: &rusqlite::Connection,
        driver_id: &str,
    ) -> crate::models::contract::Contract {
        contract_queries::get_contracts_for_pilot(conn, driver_id)
            .expect("driver contracts query")
            .into_iter()
            .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
            .max_by(|a, b| {
                a.temporada_inicio
                    .cmp(&b.temporada_inicio)
                    .then_with(|| a.created_at.cmp(&b.created_at))
            })
            .expect("latest regular contract")
    }
}
