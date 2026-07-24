use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::calendar::{full_season::generate_full_season_calendar, CalendarEntry};
use crate::commands::career_detail::build_driver_detail_payload;
use crate::commands::career_types::{
    AcceptedSpecialOfferSummary, BriefingPhraseEntry, BriefingPhraseEntryInput,
    BriefingPhraseHistory, BriefingStorySummary, CareerData, CareerResumeContext, CareerResumeView,
    ContractWarningInfo, CreateCareerResult, DriverDetail, DriverSummary, NextRaceBriefingSummary,
    PrimaryRivalSummary, RaceSummary, SaveInfo, SeasonSummary, TeamStanding, TeamSummary,
    TrackHistorySummary, VerifyDatabaseResponse,
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
use crate::generators::world::{align_world_career_start_years, generate_world};
use crate::market::pipeline::fill_all_remaining_vacancies;
use crate::market::preseason::{
    advance_week, delete_preseason_plan, load_preseason_plan, save_preseason_plan, PendingAction,
    PlannedEvent, PreSeasonPlan, PreSeasonState, WeekResult,
};
use crate::market::proposals::{MarketProposal, ProposalStatus};
use crate::models::driver::Driver;
use crate::models::enums::{ContractStatus, DriverStatus, SeasonPhase, TeamRole};
use crate::models::license::{
    driver_has_required_license_for_division, ensure_driver_can_join_division,
    grant_driver_license_for_division_if_needed,
};
use crate::models::season::Season;
use crate::models::team::{Team, TeamHierarchyClimate};
use crate::news::{NewsImportance, NewsItem, NewsType};

pub use crate::commands::career_types::CreateCareerInput;

#[path = "career/save_state.rs"]
mod save_state;

pub(crate) use save_state::{
    delete_resume_context, get_briefing_phrase_history_in_base_dir,
    persist_resume_context_in_base_dir, read_resume_context, read_save_meta,
    save_briefing_phrase_history_in_base_dir, save_meta_to_info, write_resume_context,
    write_save_meta,
};

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
    /// Semanas até a proposta expirar (Fase B). `None` = sem prazo (proposta de rollover).
    pub semanas_restantes: Option<i32>,
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

        let mut world = generate_world(
            &normalized_name,
            &nationality_label,
            normalized_age,
            &normalized_category,
            input.team_index,
            &normalized_difficulty,
        )?;

        let season_id = next_id(&db.conn, IdType::Season)
            .map_err(|e| format!("Falha ao gerar ID da temporada: {e}"))?;
        let mut season = Season::new(season_id.clone(), 1, 2024);
        season.fase = SeasonPhase::Temporada;
        align_world_career_start_years(&mut world, season.ano as u32);
        let calendar_seed: u64 = rand::random();

        let total_races = db
            .transaction(|tx| {
                for driver in &world.drivers {
                    driver_queries::insert_driver(tx, driver)?;
                }

                team_queries::insert_teams(tx, &world.teams)?;
                // Semeia o carro inicial de cada time (Sistema de Nível do Carro):
                // correlacionado com a qualidade na categoria; rookie = spec.
                crate::market::car_maintenance::seed_and_persist_team_cars(tx, &world.teams)?;
                contract_queries::insert_contracts(tx, &world.contracts)?;
                for contract in &world.contracts {
                    grant_driver_license_for_division_if_needed(
                        tx,
                        &contract.piloto_id,
                        &contract.categoria,
                        contract.classe.as_deref(),
                    )
                    .map_err(crate::db::connection::DbError::Migration)?;
                }
                season_queries::insert_season(tx, &season)?;
                let n = generate_full_season_calendar(tx, &season_id, season.ano, calendar_seed)?;
                sync_meta_counters(
                    tx,
                    world.drivers.len(),
                    world.teams.len(),
                    world.contracts.len(),
                    1,
                    n,
                )?;
                Ok(n)
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
            message: rust_i18n::t!("career.message.created").to_string(),
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

    // Telemetria: onde esta carreira está no mundo (ano/categoria/dificuldade/progresso).
    // Fica aqui porque `load_career` é o ponto por onde TODA carreira aberta passa —
    // inclusive depois de virar a temporada, quando a UI recarrega. Só grava num estático
    // em memória; quem envia é a borda de corrida, e só se o jogador tiver consentido.
    // Sem equipe (agente livre) a categoria vem do último campeonato do piloto.
    //
    // A dificuldade viaja em TODO evento (não só no fim de corrida) porque é o eixo pelo
    // qual o desfecho é lido: posição e ritmo só calibram a curva se você souber em que
    // nível aquela corrida foi disputada.
    crate::telemetry::set_career_context(
        active_season.ano,
        player_team
            .as_ref()
            .map(|t| t.categoria.clone())
            .or_else(|| player.categoria_atual.clone())
            .unwrap_or_else(|| "sem_equipe".to_string()),
        meta.difficulty.clone(),
        player.stats_carreira.temporadas as i32,
        player.stats_carreira.corridas as i32,
    );

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
    // Cota de público do jogador (Fase 3 do Estrelato): fama do lineup da equipe do
    // jogador vs o grid da próxima corrida → fração do portão que a equipe captura
    // (piso + prêmio de estrela, mesma conta da bilheteria). `None` sem equipe.
    let public_fame_share: Option<f64> = next_race.as_ref().and_then(|race| {
        let team = player_team.as_ref()?;
        let category_teams = team_queries::get_teams_by_category(&db.conn, &race.categoria).ok()?;
        let grid_total: f64 = category_teams
            .iter()
            .map(|t| {
                let medias =
                    team_queries::get_team_lineup_medias(&db.conn, &t.id).unwrap_or_default();
                crate::public_presence::team::derive_team_public_presence(&medias).raw_score
            })
            .sum();
        let team_medias =
            team_queries::get_team_lineup_medias(&db.conn, &team.id).unwrap_or_default();
        let team_presence =
            crate::public_presence::team::derive_team_public_presence(&team_medias).raw_score;
        let n = category_teams.len().max(1) as f64;
        Some(crate::finance::cashflow::team_gate_share(
            team_presence,
            grid_total,
            n,
        ))
    });
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
        thematic_slot: race.thematic_slot.as_str().to_string(),
        event_interest: event_interest_summary.clone(),
        public_fame_share,
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
            midia: player.atributos.midia.round().clamp(0.0, 100.0) as u8,
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
            is_aposentado: false,
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

    Ok(rust_i18n::t!("career.message.deleted", id = career_id).to_string())
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
        5 => "endurance",
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

/// Dossiê de habilidade do JOGADOR: atributos inferidos do desempenho REAL na
/// pista (só visual — o mercado NÃO consulta). Reconstrói o grid de cada corrida
/// e roda o estimador puro (ver `crate::player_skill` e o spec de 2026-07-12).
pub(crate) fn get_player_dossier_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<crate::player_skill::PlayerDossier, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let samples = crate::db::queries::race_history::get_player_race_samples(&db.conn, &player.id)
        .map_err(|e| format!("Falha ao reconstruir histórico do jogador: {e}"))?;

    Ok(crate::player_skill::build_dossier(
        &samples,
        player.atributos.midia,
    ))
}

pub(crate) fn get_driver_in_base_dir(
    base_dir: &Path,
    career_number: u32,
    driver_id: &str,
) -> Result<Driver, String> {
    let config = AppConfig::load_or_default(base_dir);
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
    let mut season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    let pending_races = calendar_queries::get_pending_races(&db.conn, &season.id)
        .map_err(|e| format!("Falha ao verificar corridas pendentes: {e}"))?;
    let pending_error = || {
        let mut pending_categories: Vec<String> = pending_races
            .iter()
            .map(|race| race.categoria.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        pending_categories.sort();
        format!(
            "Ainda existem {} corridas pendentes na temporada {} ({})",
            pending_races.len(),
            season.numero,
            pending_categories.join(", ")
        )
    };

    // O fechamento anual so acontece depois das corridas especiais e do PosEspecial.
    // Assim o mercado normal nunca atropela a convocacao nem o bloco especial.
    match season.fase {
        SeasonPhase::PreTemporada => {
            return Err("A temporada ainda nao comecou.".to_string());
        }
        SeasonPhase::Temporada => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::Encerramento)
                .map_err(|e| format!("Falha ao encerrar temporada concluida: {e}"))?;
            season.fase = SeasonPhase::Encerramento;
        }
        SeasonPhase::Encerramento => {}
        SeasonPhase::PosEspecial => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            cleanup_legacy_special_state_for_9d_transition(&db.conn, season.numero)?;
        }
        SeasonPhase::BlocoRegular => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            return Err(
                "A temporada regular terminou, mas a janela de convocacao especial ainda precisa ser aberta."
                    .to_string(),
            );
        }
        SeasonPhase::JanelaConvocacao | SeasonPhase::BlocoEspecial => {
            if !pending_races.is_empty() {
                return Err(pending_error());
            }
            return Err(format!(
                "Nao e possivel avancar a temporada na fase '{}'. Encerre o bloco especial primeiro.",
                season.fase
            ));
        } // LEGADO 9D: fases do modelo novo nunca chegam aqui em saves pré-v33
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

    {
        let season = season_queries::get_active_season(&db.conn)
            .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
            .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

        if season.fase == SeasonPhase::Temporada {
            let pending = calendar_queries::get_pending_races(&db.conn, &season.id)
                .map_err(|e| format!("Falha ao buscar corridas pendentes: {e}"))?;
            for race in &pending {
                crate::commands::race::simulate_category_race(&mut db, race, false)?;
            }
            season_queries::move_to_encerramento_if_completed(&db.conn, &season)
                .map_err(|e| format!("Falha ao encerrar temporada 9D: {e}"))?;
            return Ok(());
        }
    }

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
    accepted_seat_id: Option<&str>,
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
    let result = advance_week(&tx, &mut plan, accepted_seat_id)?;
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

/// Quebra de contrato do jogador (Fase 2b.3): a oferta guardada no plano da janela,
/// ou None. Enriquecida no setup (`compute_player_poach_offer`); só leitura aqui.
pub(crate) fn get_player_poach_offer_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Option<crate::market::pipeline::PlayerPoachOffer>, String> {
    let (_, career_dir, _) = open_career_resources_read_only(base_dir, career_id)?;
    let plan = load_preseason_plan(&career_dir)?
        .ok_or_else(|| "Plano da pre-temporada nao encontrado.".to_string())?;
    Ok(plan.player_poach_offer)
}

/// Resolve a decisão do jogador na quebra de contrato: `accept = true` sai pro
/// pretendente, `false` fica no time atual. Recebe a oferta que o jogador VIU (a UI a
/// tem em mãos), aplica no banco e — se existir um plano de janela — limpa a oferta
/// dele. Independente do plano, pra o debug funcionar de qualquer tela.
pub(crate) fn resolve_player_poach_offer_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    offer: &crate::market::pipeline::PlayerPoachOffer,
    accept: bool,
) -> Result<crate::market::pipeline::PlayerPoachOutcome, String> {
    let (db, career_dir, _) = open_career_resources(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;

    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao da quebra de contrato: {e}"))?;
    let outcome = crate::market::pipeline::resolve_player_poach(&tx, offer, accept, season.numero)?;
    tx.commit()
        .map_err(|e| format!("Falha ao confirmar quebra de contrato: {e}"))?;

    // Consome a oferta do plano da janela, se houver (uma decisão por janela).
    if let Ok(Some(mut plan)) = load_preseason_plan(&career_dir) {
        if plan.player_poach_offer.is_some() {
            plan.player_poach_offer = None;
            plan.state.player_has_team = true;
            crate::market::preseason::save_preseason_plan(&career_dir, &plan)?;
        }
    }
    Ok(outcome)
}

/// DEBUG: força uma proposta de quebra de contrato pro jogador (Fase 2b.3), pra testar
/// a tela do leilão mesmo num save sem o cenário raro. Relaxa os portões e escolhe o
/// pretendente mais rico da categoria. NÃO exige a janela de mercado — se houver um
/// plano, guarda a oferta nele; senão, só devolve pra UI mostrar. Exige o jogador sob
/// contrato regular (agente livre não é "arrancado").
pub(crate) fn debug_force_player_poach_offer_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Option<crate::market::pipeline::PlayerPoachOffer>, String> {
    let (db, career_dir, _) = open_career_resources(base_dir, career_id)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;
    let offer = crate::market::pipeline::debug_build_player_poach_offer(&db.conn, season.numero)?;
    // Se estamos numa janela de mercado, persiste no plano (fluxo real); senão só devolve.
    if let Ok(Some(mut plan)) = load_preseason_plan(&career_dir) {
        plan.player_poach_offer = offer.clone();
        crate::market::preseason::save_preseason_plan(&career_dir, &plan)?;
    }
    Ok(offer)
}

/// DEBUG: prepara o mercado num cenário específico. Simula as corridas restantes (encerra
/// a temporada), torna o jogador AGENTE LIVRE (pra as propostas formais aparecerem) e força
/// a posição final no campeonato (visibilidade → mérito). Cenários: "no_team" (livre, sem
/// forçar posição), "first" (campeão), "fifth" (meio do pelotão). O chamador avança a
/// temporada depois. Só usado pelos controles de debug da UI.
pub(crate) fn debug_prepare_market_scenario_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    scenario: &str,
) -> Result<(), String> {
    skip_all_pending_races_in_base_dir(base_dir, career_id)?;

    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;

    // Todos os cenários entram no mercado como agente livre (as propostas formais só são
    // geradas pra quem está sem contrato nesta fase).
    if let Some(contract) = contract_queries::get_active_contract_for_pilot(&db.conn, &player.id)
        .map_err(|e| format!("Falha ao buscar contrato do jogador: {e}"))?
    {
        contract_queries::update_contract_status(
            &db.conn,
            &contract.id,
            &ContractStatus::Rescindido,
        )
        .map_err(|e| format!("Falha ao rescindir contrato (debug): {e}"))?;
        team_queries::remove_pilot_from_team(&db.conn, &player.id, &contract.equipe_id)
            .map_err(|e| format!("Falha ao remover jogador da equipe (debug): {e}"))?;
    }

    // Força a posição final no campeonato (posicao, pontos, vitorias, podios).
    let forced: Option<(i32, f64, i32)> = match scenario {
        "first" => Some((1, 420.0, 10)),
        "fifth" => Some((5, 180.0, 2)),
        _ => None, // "no_team": não força posição
    };
    if let Some((pos, pts, wins)) = forced {
        // Categoria do "campeão/mediano": usa a atual; se o jogador nunca teve categoria
        // (ex.: save iniciado via "sem time"), assume rookie — assim a promoção mira a Cup.
        let categoria = player
            .categoria_atual
            .clone()
            .unwrap_or_else(|| "mazda_rookie".to_string());

        // Garante categoria_atual no jogador (o tier de mercado deriva dela).
        if player.categoria_atual.is_none() {
            db.conn
                .execute(
                    "UPDATE drivers SET categoria_atual = ?1 WHERE id = ?2",
                    rusqlite::params![categoria, player.id],
                )
                .map_err(|e| format!("Falha ao definir categoria do jogador (debug): {e}"))?;
        }

        // UPSERT do standings: atualiza a linha se existir, senão CRIA — sem isso o
        // UPDATE não afeta nada quando o jogador nunca correu (standings vazio) e o
        // cenário de campeão não surtia efeito (posição/pódio ausentes).
        let updated = db
            .conn
            .execute(
                "UPDATE standings SET posicao = ?1, pontos = ?2, vitorias = ?3, podios = ?4,
                     categoria = ?5
                 WHERE temporada_id = ?6 AND piloto_id = ?7",
                rusqlite::params![pos, pts, wins, wins + 3, categoria, season.id, player.id],
            )
            .map_err(|e| format!("Falha ao forcar classificacao (debug): {e}"))?;
        if updated == 0 {
            db.conn
                .execute(
                    "INSERT INTO standings
                        (temporada_id, piloto_id, categoria, posicao, pontos, vitorias, podios, poles, corridas)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 14)",
                    rusqlite::params![season.id, player.id, categoria, pos, pts, wins, wins + 3],
                )
                .map_err(|e| format!("Falha ao criar classificacao (debug): {e}"))?;
        }
    }

    // DEBUG: um campeão/mediano chega FAMOSO — a fama que ele acumularia na temporada
    // (que o atalho de debug pula ao não correr) é forçada aqui, pra dar pra testar o
    // interesse de mercado do estrelato (Fase 2a). Só no debug; não afeta o jogo real.
    let forced_fama: Option<f64> = match scenario {
        "first" => Some(82.0), // Estrela
        "fifth" => Some(55.0), // Nome forte
        _ => None,             // "no_team": não mexe
    };
    if let Some(fama) = forced_fama {
        db.conn
            .execute(
                "UPDATE drivers SET midia = ?1 WHERE id = ?2",
                rusqlite::params![fama, player.id],
            )
            .map_err(|e| format!("Falha ao forcar fama do jogador (debug): {e}"))?;
    }
    Ok(())
}

/// DEBUG: relatório do dry-run do leilão de poaching (Fase 2b.2).
#[derive(Debug, Serialize)]
pub(crate) struct PoachDebugReport {
    /// `true` = o mundo foi TEMPERADO pra garantir briga (não é o estado real do save).
    pub forced: bool,
    pub nota: String,
    pub auctions: Vec<crate::market::pipeline::PoachAudit>,
}

/// DEBUG: roda o passe de poaching **de verdade** e desfaz tudo (transação com
/// rollback), devolvendo o raio-x de cada leilão. O leilão só acontece entre IAs e
/// não tem tela até o 2b.3 — isto é a janela pra vê-lo. Não altera o save.
///
/// Se o save, como está, não gera nenhum assédio (ninguém tem caixa pra multa ou
/// não há upgrade claro), roda de novo TEMPERANDO o mundo (`forced`): fama 95 nos
/// melhores pilotos da categoria do jogador + caixa gordo nos times dela. Como tudo
/// é desfeito, é só uma simulação — mas mostra o leilão brigando de verdade.
pub(crate) fn debug_poaching_auctions_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<PoachDebugReport, String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let categoria = player
        .categoria_atual
        .clone()
        .unwrap_or_else(|| "gt3".to_string());

    // Passada 1: o mundo como ele está.
    let real = debug_run_poaching_dry(&db, season.numero + 1, None)?;
    if !real.is_empty() {
        return Ok(PoachDebugReport {
            forced: false,
            nota: format!(
                "{} assédio(s) que aconteceriam AGORA, do seu save como está. \
                 Nada foi salvo (a simulação é desfeita).",
                real.len()
            ),
            auctions: real,
        });
    }

    // Passada 2: temperando o mundo, já que o save não tinha briga nenhuma.
    let forced = debug_run_poaching_dry(&db, season.numero + 1, Some(&categoria))?;
    let nota = if forced.is_empty() {
        format!(
            "Nem temperando saiu briga em '{categoria}' — provavelmente não há dois times \
             regulares com as duas vagas cheias na categoria. Nada foi salvo."
        )
    } else {
        format!(
            "Seu save, como está, não gera assédio nenhum agora. Então TEMPEREI o mundo \
             (fama 95 nos melhores de '{categoria}' + caixa gordo nos times de lá) só pra \
             te mostrar o leilão brigando: {} assédio(s). É simulação — nada foi salvo.",
            forced.len()
        )
    };
    Ok(PoachDebugReport {
        forced: true,
        nota,
        auctions: forced,
    })
}

/// Roda o passe numa transação e SEMPRE desfaz. `spice` = categoria a temperar
/// (fama alta + caixa) antes de rodar, ou `None` pra usar o mundo como está.
fn debug_run_poaching_dry(
    db: &Database,
    new_season_number: i32,
    spice: Option<&str>,
) -> Result<Vec<crate::market::pipeline::PoachAudit>, String> {
    let tx = db
        .conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao abrir transacao de debug: {e}"))?;

    if let Some(categoria) = spice {
        // Caixa gordo nos times da categoria (pra terem como pagar multa e salário).
        tx.execute(
            "UPDATE teams SET cash_balance = cash_balance + 8000000 WHERE categoria = ?1",
            rusqlite::params![categoria],
        )
        .map_err(|e| format!("Falha ao temperar caixa (debug): {e}"))?;
        // Os 3 melhores pilotos da categoria viram ídolos (fama 95).
        tx.execute(
            "UPDATE drivers SET midia = 95 WHERE id IN (
                 SELECT id FROM drivers
                 WHERE categoria_atual = ?1 AND is_jogador = 0 AND status = 'Ativo'
                 ORDER BY skill DESC LIMIT 3
             )",
            rusqlite::params![categoria],
        )
        .map_err(|e| format!("Falha ao temperar fama (debug): {e}"))?;
    }

    let teams =
        team_queries::get_all_teams(&tx).map_err(|e| format!("Falha ao carregar times: {e}"))?;
    let mut rng = StdRng::seed_from_u64(2026);
    let mut report = crate::market::proposals::MarketReport::default();
    let mut audit = Vec::new();
    let result = crate::market::pipeline::run_poaching_pass(
        &tx,
        &teams,
        new_season_number,
        &mut rng,
        &mut report,
        &mut audit,
    );
    // Desfaz SEMPRE — inclusive se o passe falhou no meio.
    tx.rollback()
        .map_err(|e| format!("Falha ao desfazer simulacao de debug: {e}"))?;
    result?;
    Ok(audit)
}

/// DEBUG: carimba a posição final do jogador no ARQUIVO da temporada mais recente.
/// Roda DEPOIS do avanço (o avanço recalcula standings só de quem correu e exclui o
/// jogador agente livre, gravando posição `null` no arquivo). Sem isso, o cenário de
/// campeão/mediano nunca vira pódio e o mercado não oferta promoção.
pub(crate) fn debug_stamp_player_championship_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    scenario: &str,
) -> Result<(), String> {
    let position: i32 = match scenario {
        "first" => 1,
        "fifth" => 5,
        _ => return Ok(()), // "no_team": não carimba
    };
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;

    // Categoria de exibição: última por contrato, senão rookie.
    let categoria: String = db
        .conn
        .query_row(
            "SELECT categoria FROM contracts WHERE piloto_id = ?1 ORDER BY temporada_fim DESC LIMIT 1",
            rusqlite::params![player.id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar categoria do jogador (debug): {e}"))?
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| "mazda_rookie".to_string());

    // Temporada arquivada mais recente do jogador (agregado → sempre 1 linha, valor Option).
    let latest: Option<i32> = db
        .conn
        .query_row(
            "SELECT MAX(season_number) FROM driver_season_archive WHERE piloto_id = ?1",
            rusqlite::params![player.id],
            |r| r.get::<_, Option<i32>>(0),
        )
        .map_err(|e| format!("Falha ao buscar arquivo do jogador (debug): {e}"))?;

    match latest {
        Some(season_number) => {
            db.conn
                .execute(
                    "UPDATE driver_season_archive
                     SET posicao_campeonato = ?1, categoria = ?2
                     WHERE piloto_id = ?3 AND season_number = ?4",
                    rusqlite::params![position, categoria, player.id, season_number],
                )
                .map_err(|e| format!("Falha ao carimbar posicao do jogador (debug): {e}"))?;
        }
        None => {
            // Sem arquivo ainda: cria uma linha pro ano da temporada ativa (edge case).
            let season = season_queries::get_active_season(&db.conn)
                .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
                .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
            db.conn
                .execute(
                    "INSERT INTO driver_season_archive
                        (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}')",
                    rusqlite::params![
                        player.id,
                        (season.numero - 1).max(1),
                        season.ano,
                        player.nome,
                        categoria,
                        position,
                        0.0
                    ],
                )
                .map_err(|e| format!("Falha ao criar arquivo do jogador (debug): {e}"))?;
        }
    }
    Ok(())
}

pub(crate) fn get_player_proposals_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<Vec<PlayerProposalView>, String> {
    let (db, career_dir, _meta) = open_career_resources(base_dir, career_id)?;
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

    // Prazo no card (Fase B): semanas restantes = semana_limite − semana atual da janela.
    let current_week = load_preseason_plan(&career_dir)
        .ok()
        .flatten()
        .map(|plan| plan.state.current_week);
    if let Some(week) = current_week {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT id, semana_limite FROM market_proposals
                 WHERE temporada_id = ?1 AND piloto_id = ?2 AND status = 'Pendente'
                   AND semana_limite IS NOT NULL",
            )
            .map_err(|e| format!("Falha ao preparar prazos das propostas: {e}"))?;
        let deadlines: std::collections::HashMap<String, i32> = stmt
            .query_map(rusqlite::params![season.id, player.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })
            .map_err(|e| format!("Falha ao consultar prazos: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("Falha ao ler prazos: {e}"))?;
        for view in &mut proposals {
            if let Some(limite) = deadlines.get(&view.proposal_id) {
                view.semanas_restantes = Some((limite - week).max(0));
            }
        }
    }
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

    if !accept
        && remaining == 0
        && contract_queries::get_active_regular_contract_for_pilot(&db.conn, &player.id)
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

    warn_if_noncritical(
        sync_preseason_pending_flag(&career_dir, remaining > 0),
        "Falha ao sincronizar indicador de propostas pendentes",
    );
    let headlines = news_items
        .iter()
        .map(|item| item.titulo.clone())
        .collect::<Vec<_>>();

    let message = if accept {
        let role = if proposal.papel == TeamRole::Numero1 {
            "N1"
        } else {
            "N2"
        };
        rust_i18n::t!(
            "career.message.proposal_signed",
            team = proposal.equipe_nome.as_str(),
            role = role
        )
        .to_string()
    } else if let Some(team_name) = &new_team_name {
        rust_i18n::t!(
            "career.message.proposal_declined_reallocated",
            team = proposal.equipe_nome.as_str(),
            new_team = team_name.as_str()
        )
        .to_string()
    } else if remaining > 0 {
        rust_i18n::t!(
            "career.message.proposal_declined_emergency",
            team = proposal.equipe_nome.as_str()
        )
        .to_string()
    } else {
        rust_i18n::t!(
            "career.message.proposal_declined",
            team = proposal.equipe_nome.as_str()
        )
        .to_string()
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
    // Gate da Janela de Transferências: a pré-temporada só finaliza quando a janela
    // fechou (plan.state.is_complete, garantido acima). O jogador já está garantido
    // num assento pela garantia de porta no fecho — não há mais "propostas pendentes".

    let mut rng = rand::thread_rng();

    // 1. Invariante: Garantir que todas as equipes regulares tenham lineup completo antes de iniciar
    fill_all_remaining_vacancies(&db.conn, season.numero, &mut rng)
        .map_err(|e| format!("Falha ao preencher vagas remanescentes: {e}"))?;

    // 1b. Invariante: Garantir que N1/N2 de toda equipe regular está alinhado com o lineup final.
    // Normaliza equipes preenchidas por fallback que não passaram pelo UpdateHierarchy do mercado.
    crate::hierarchy::transition::validate_and_normalize_team_hierarchies(&db.conn)?;

    if season.fase == SeasonPhase::PreTemporada {
        season_queries::update_season_fase(&db.conn, &season.id, &SeasonPhase::Temporada)
            .map_err(|e| format!("Falha ao iniciar temporada apos pre-temporada: {e}"))?;
    }

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

/// Categorias onde um piloto pode realmente pegar vaga, espelhando o predicado
/// `eligible` do motor de transferências: tier do assento em [tier−1, tier+1] E licença
/// suficiente (com a exceção do +1 tier, cuja licença é concedida na assinatura).
/// Não considera o bônus de craque (skill≥80 pula 2 tiers) — sem skill neste payload.
fn eligible_categories_for(driver_tier: u8, license: u8) -> Vec<String> {
    crate::constants::categories::get_all_categories()
        .iter()
        .filter(|cat| {
            let seat_tier = cat.tier;
            let required = cat.licenca_necessaria.unwrap_or(0);
            let close = (seat_tier as i16 - driver_tier as i16).abs() <= 1;
            let promotion = seat_tier == driver_tier + 1;
            close && (license >= required || promotion)
        })
        .map(|cat| cat.id.to_string())
        .collect()
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
            // Faixa de nível (tier da categoria onde corre hoje) — chave do agrupamento.
            let market_tier = crate::constants::categories::get_category_config(&r.categoria)
                .map(|config| config.tier);
            let eligible_categories =
                eligible_categories_for(market_tier.unwrap_or(0), r.max_license_level.unwrap_or(0));
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
                market_tier,
                seasons_idle: r.seasons_idle,
                eligible_categories,
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

/// Inverte o favorito do piloto (watchlist) e devolve o NOVO estado (true = agora
/// favoritado). Puramente cosmético — alimenta a ênfase do feed do mercado e o filtro
/// "Favoritos" na aba de pilotos; não toca na simulação.
pub(crate) fn toggle_driver_favorite_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    driver_id: &str,
) -> Result<bool, String> {
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    crate::db::queries::favorites::toggle_favorite(&db.conn, driver_id)
        .map_err(|e| format!("Falha ao alternar favorito: {e}"))
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
        car_performance: team.effective_car_performance(),
        car_performance_rating: normalize_car_performance(team.effective_car_performance()),
        reputacao: team.reputacao,
        companheiro_nome: companion.as_ref().map(|driver| driver.nome.clone()),
        companheiro_skill: companion
            .as_ref()
            .map(|driver| driver.atributos.skill.round().clamp(0.0, 100.0) as u8),
        status: proposal.status.as_str().to_string(),
        semanas_restantes: None, // preenchido pelo chamador que conhece a semana atual
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
    ensure_driver_can_join_division(
        tx,
        &player.id,
        &player.nome,
        &team.categoria,
        team.classe.as_deref(),
    )?;
    let mut contract = crate::models::contract::Contract::new(
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
    contract.classe = team.classe.clone();
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
        backfill_team_vacancy(tx, &previous_team_id, season.numero, season.ano)?;
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
    let drivers_by_id = active_regular_contracts
        .iter()
        .filter_map(|contract| {
            driver_queries::get_driver(conn, &contract.piloto_id)
                .ok()
                .map(|driver| (contract.piloto_id.clone(), driver))
        })
        .collect::<HashMap<_, _>>();
    active_regular_contracts.sort_by(|a, b| {
        let a_is_player = drivers_by_id
            .get(&a.piloto_id)
            .map(|driver| driver.is_jogador)
            .unwrap_or(false);
        let b_is_player = drivers_by_id
            .get(&b.piloto_id)
            .map(|driver| driver.is_jogador)
            .unwrap_or(false);
        b_is_player
            .cmp(&a_is_player)
            .then_with(|| b.temporada_inicio.cmp(&a.temporada_inicio))
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| b.id.cmp(&a.id))
    });

    let mut keep_n1 = None;
    let mut keep_n2 = None;
    let mut displaced_driver_ids = HashSet::new();
    let mut contract_ids_in_slots = HashSet::new();
    let mut role_fixed = false;

    for contract in active_regular_contracts {
        if contract_ids_in_slots.contains(&contract.id) {
            continue;
        }
        let slot = match contract.papel {
            TeamRole::Numero1 => &mut keep_n1,
            TeamRole::Numero2 => &mut keep_n2,
        };
        if slot.is_none() {
            contract_ids_in_slots.insert(contract.id.clone());
            *slot = Some(contract);
            continue;
        }

        if keep_n1.is_none() {
            contract_ids_in_slots.insert(contract.id.clone());
            keep_n1 = Some(contract);
        } else if keep_n2.is_none() {
            contract_ids_in_slots.insert(contract.id.clone());
            keep_n2 = Some(contract);
        } else {
            contract_queries::update_contract_status(
                conn,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular excedente '{}': {e}",
                    contract.id
                )
            })?;
            displaced_driver_ids.insert(contract.piloto_id);
        }
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
            && driver_has_required_license_for_division(
                conn,
                &player.id,
                &vacancy.team.categoria,
                vacancy.team.classe.as_deref(),
            )?
        {
            vacancies.push(vacancy);
        }
    }
    if vacancies.is_empty() {
        for vacancy in list_team_vacancies(conn)? {
            if driver_has_required_license_for_division(
                conn,
                &player.id,
                &vacancy.team.categoria,
                vacancy.team.classe.as_deref(),
            )? {
                vacancies.push(vacancy);
            }
        }
    }
    // Melhor vaga = melhor CARRO efetivo (peças > coluna legada). Num grid spec ninguém
    // desempata pelo pacote e a ordem de entrada manda — que é a verdade da pista.
    vacancies.sort_by(|a, b| {
        b.team
            .effective_car_performance()
            .total_cmp(&a.team.effective_car_performance())
    });

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
            && driver_has_required_license_for_division(
                conn,
                &player.id,
                &vacancy.team.categoria,
                vacancy.team.classe.as_deref(),
            )?
        {
            vacancies.push(vacancy);
        }
    }
    if vacancies.is_empty() {
        for vacancy in list_team_vacancies(conn)? {
            if driver_has_required_license_for_division(
                conn,
                &player.id,
                &vacancy.team.categoria,
                vacancy.team.classe.as_deref(),
            )? {
                vacancies.push(vacancy);
            }
        }
    }
    vacancies.sort_by(|a, b| {
        a.team
            .effective_car_performance()
            .total_cmp(&b.team.effective_car_performance())
    });
    let Some(vacancy) = vacancies.into_iter().next() else {
        return Ok(None);
    };
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao iniciar transacao de alocacao forcada: {e}"))?;
    ensure_driver_can_join_division(
        &tx,
        &player.id,
        &player.nome,
        &vacancy.team.categoria,
        vacancy.team.classe.as_deref(),
    )?;

    let mut contract = crate::models::contract::Contract::new(
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
    contract.classe = vacancy.team.classe.clone();
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

pub(crate) fn backfill_team_vacancy(
    conn: &rusqlite::Connection,
    team_id: &str,
    season_number: i32,
    season_year: i32,
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
            driver_has_required_license_for_division(
                conn,
                &driver.id,
                &team.categoria,
                team.classe.as_deref(),
            )
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
        let mut rookie = crate::evolution::rookies::generate_rookies(
            1,
            season_year,
            &mut existing_names,
            &mut rng,
        )
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
        grant_driver_license_for_division_if_needed(
            conn,
            &rookie.id,
            &team.categoria,
            team.classe.as_deref(),
        )?;
        rookie
    };
    ensure_driver_can_join_division(
        conn,
        &replacement.id,
        &replacement.nome,
        &team.categoria,
        team.classe.as_deref(),
    )?;

    let mut contract = crate::models::contract::Contract::new(
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
    contract.classe = team.classe.clone();
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
        event.executed
            || !matches!(&event.event, PendingAction::UpdateHierarchy { team_id: current, .. } if current == team_id)
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
                .unwrap_or_else(|| rust_i18n::t!("career.message.no_driver").to_string()),
            n2_id: n2.as_ref().map(|candidate| candidate.0.clone()),
            n2_name: n2
                .as_ref()
                .map(|candidate| candidate.1.clone())
                .unwrap_or_else(|| rust_i18n::t!("career.message.no_driver").to_string()),
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

pub(super) fn open_career_resources_read_only(
    base_dir: &Path,
    career_id: &str,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    open_career_resources_with_repair(base_dir, career_id, false)
}

pub(super) fn open_career_resources_for_category_read(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<(Database, std::path::PathBuf, SaveMeta), String> {
    let (db, career_dir, meta) = open_career_resources_read_only(base_dir, career_id)?;
    let _ = category;
    Ok((db, career_dir, meta))
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
    let meta = read_save_meta(&meta_path)?;
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
        if !categories::is_valid_competitive_division(
            &contract.categoria,
            contract.classe.as_deref(),
        ) {
            contract_queries::update_contract_status(
                &tx,
                &contract.id,
                &ContractStatus::Rescindido,
            )
            .map_err(|e| {
                format!(
                    "Falha ao rescindir contrato regular com divisao invalida '{}': {e}",
                    contract.id
                )
            })?;
            affected_team_ids.insert(contract.equipe_id.clone());
            continue;
        }

        let Some(team) = teams_by_id.get(&contract.equipe_id) else {
            continue;
        };
        if !categories::uses_regular_contracts(&team.categoria) {
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
        .filter(|team| categories::uses_regular_teams(&team.categoria))
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
            let pending_temporada_races = calendar_queries::count_pending_races_in_phase(
                conn,
                &active_season.id,
                &SeasonPhase::Temporada,
            )
            .map_err(|e| format!("Falha ao contar corridas da temporada pendentes: {e}"))?;
            let has_pending = pending_regular_races + pending_temporada_races > 0;
            if active_season.fase.is_racing() && has_pending {
                let mut rng = rand::thread_rng();
                fill_all_remaining_vacancies(conn, active_season.numero, &mut rng)
                    .map_err(|e| format!("Falha ao preencher vagas regulares pendentes: {e}"))?;
            }
        }
    }
    Ok(())
}

// Melhor posição de chegada (ignorando DNF) usada como desempate de classificação.
// Quem não terminou nenhuma corrida fica com o pior valor possível, então cai para
// o fim do grupo empatado em vez de subir por não ter resultado.
fn best_finish_position(results: &[Option<RoundResult>]) -> i32 {
    results
        .iter()
        .filter_map(|result| result.as_ref())
        .filter(|result| !result.is_dnf)
        .map(|result| result.position)
        .min()
        .unwrap_or(i32::MAX)
}

/// Um piloto de interesse do jogador (Nemesis ou Rival) — o mínimo para decorar o
/// nome nas telas. Vem do motor de rivalidade (intensidade percebida acumulada).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RivalInterest {
    pub driver_id: String,
    pub driver_name: String,
    /// Intensidade percebida (0–100) no momento — para ordenar/depurar.
    pub perceived: f64,
    /// Nome determinístico da rivalidade ("A Revanche de Interlagos"), do 1º capítulo.
    /// `None` até haver um episódio registrado.
    pub label: Option<String>,
    /// Retrospecto direto (h2h): capítulos que o JOGADOR levou a melhor.
    pub h2h_player_wins: i32,
    /// Capítulos que o RIVAL levou a melhor.
    pub h2h_rival_wins: i32,
    /// Total de capítulos registrados do par.
    pub chapters: i32,
}

/// Os 3 pilotos de interesse mostrados ao jogador: 1 Nemesis + até 2 Rivais.
/// O motor rastreia mais; só estes recebem marcador nas telas.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerInterests {
    pub nemesis: Option<RivalInterest>,
    pub rivais: Vec<RivalInterest>,
}

/// Intensidade mínima para ser Nemesis ("rivalidade clara"). Abaixo disso, sem Nemesis.
const NEMESIS_MIN_PERCEIVED: f64 = 40.0;
/// Intensidade mínima para ser Rival mostrado ("rivalidade inicial").
const RIVAL_MIN_PERCEIVED: f64 = 20.0;
/// Margem de histerese: o Nemesis reinante só é destituído se outro rival o superar em
/// intensidade por mais que isto — evita o Nemesis trocar toda semana no empate técnico.
const NEMESIS_HYSTERESIS_MARGIN: f64 = 10.0;

/// Seleciona os pilotos de interesse do jogador a partir do estado acumulado do motor
/// de rivalidade: Nemesis = maior intensidade (se ≥ 40); Rivais = os 2 seguintes
/// (se ≥ 20). Sem histerese ainda (a acumulação do eixo histórico já dá estabilidade;
/// histerese persistida é um refino futuro). Atravessa categorias.
pub(crate) fn get_player_interests_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<PlayerInterests, String> {
    let (db, _dir, _meta) = open_career_resources_read_only(base_dir, career_id)?;
    let current = crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
    let interests = select_player_interests(&db.conn, current.as_deref());
    // Persiste a eventual troca de Nemesis (o estado da histerese). Best-effort — só
    // aqui (no load, infrequente); o overlay lê e não escreve.
    let new_id = interests.nemesis.as_ref().map(|n| n.driver_id.as_str());
    if new_id != current.as_deref() {
        let _ = crate::db::queries::player_nemesis::set_current_nemesis(&db.conn, new_id);
    }
    Ok(interests)
}

/// Núcleo da seleção (Nemesis + Rivais) sobre uma conexão já aberta — reusado pelo
/// comando e pelo overlay. `current_nemesis` = o Nemesis reinante (para a histerese);
/// passe `None` para seleção pura por intensidade. NÃO escreve nada (quem persiste a
/// troca é o caller). Best-effort: erro/sem jogador → vazio.
pub(crate) fn select_player_interests(
    conn: &rusqlite::Connection,
    current_nemesis: Option<&str>,
) -> PlayerInterests {
    let empty = PlayerInterests {
        nemesis: None,
        rivais: Vec::new(),
    };

    let player_id: String = match conn.query_row(
        "SELECT id FROM drivers WHERE is_jogador = 1 LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(id) => id,
        Err(_) => return empty,
    };

    let mut rivalries = match crate::rivalry::get_pilot_rivalries(conn, &player_id) {
        Ok(r) => r,
        Err(_) => return empty,
    };
    rivalries.sort_by(|a, b| {
        b.perceived_intensity
            .partial_cmp(&a.perceived_intensity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let name_of = |id: &str| {
        crate::db::queries::drivers::get_driver(conn, id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| id.to_string())
    };
    let to_interest = |r: &crate::rivalry::PilotRivalrySummary| {
        // Um só fetch de episódios do par → label (1º capítulo) + retrospecto h2h.
        let eps = crate::db::queries::rivalry_episodes::get_episodes_for_pair(
            conn,
            &player_id,
            &r.rival_id,
        )
        .unwrap_or_default();
        let label = eps
            .first()
            .map(crate::db::queries::rivalry_episodes::rivalry_label);
        let mut pw = 0;
        let mut rw = 0;
        for e in &eps {
            match e.winner_id.as_deref() {
                Some(w) if w == player_id.as_str() => pw += 1,
                Some(w) if w == r.rival_id.as_str() => rw += 1,
                _ => {}
            }
        }
        RivalInterest {
            driver_id: r.rival_id.clone(),
            driver_name: name_of(&r.rival_id),
            perceived: r.perceived_intensity,
            label,
            h2h_player_wins: pw,
            h2h_rival_wins: rw,
            chapters: eps.len() as i32,
        }
    };

    let top = rivalries.first();
    // Reinante ainda presente e acima do piso de Nemesis?
    let reign = current_nemesis
        .and_then(|cur| rivalries.iter().find(|r| r.rival_id == cur))
        .filter(|r| r.perceived_intensity >= NEMESIS_MIN_PERCEIVED);

    // Histerese: mantém o reinante, salvo se outro o superar pela margem.
    let nemesis_summary: Option<&crate::rivalry::PilotRivalrySummary> = match (reign, top) {
        (Some(cur), Some(top)) => {
            if top.rival_id != cur.rival_id
                && top.perceived_intensity > cur.perceived_intensity + NEMESIS_HYSTERESIS_MARGIN
            {
                Some(top)
            } else {
                Some(cur)
            }
        }
        (Some(cur), None) => Some(cur),
        (None, Some(top)) if top.perceived_intensity >= NEMESIS_MIN_PERCEIVED => Some(top),
        _ => None,
    };

    let nemesis_id = nemesis_summary.map(|r| r.rival_id.clone());
    let nemesis = nemesis_summary.map(|r| to_interest(r));

    let rivais: Vec<RivalInterest> = rivalries
        .iter()
        .filter(|r| Some(&r.rival_id) != nemesis_id.as_ref())
        .filter(|r| r.perceived_intensity >= RIVAL_MIN_PERCEIVED)
        .take(2)
        .map(|r| to_interest(r))
        .collect();

    PlayerInterests { nemesis, rivais }
}

pub(crate) fn get_drivers_by_category_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<DriverSummary>, String> {
    let category = category.trim().to_lowercase();
    let (db, career_dir, _) =
        open_career_resources_for_category_read(base_dir, career_id, &category)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let total_rounds = count_calendar_entries(&db.conn, &season.id, &category)
        .map_err(|e| format!("Falha ao contar corridas da categoria: {e}"))?
        as usize;

    if categories::is_multiclass_category(&category) {
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
                midia: driver.atributos.midia.round().clamp(0.0, 100.0) as u8,
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
                is_aposentado: driver.status == crate::models::enums::DriverStatus::Aposentado,
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
            // Desempate por melhor chegada na pista: sem isso, pilotos empatados
            // (tipicamente todo o pelotão de 0 ponto) caíam direto no nome, então
            // o 20º podia aparecer atrás do 26º. Menor posição = melhor.
            .then_with(|| best_finish_position(&a.results).cmp(&best_finish_position(&b.results)))
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
                midia: driver.atributos.midia.round().clamp(0.0, 100.0) as u8,
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
                is_aposentado: driver.status == crate::models::enums::DriverStatus::Aposentado,
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
             ORDER BY total_points DESC, total_wins DESC, total_podiums DESC,
                      COALESCE(MIN(CASE WHEN r.dnf = 0 THEN r.posicao_final END), 9999) ASC,
                      d.nome ASC",
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

pub(crate) fn calculate_consecutive_team_tenure(
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
    let (db, _, _) = open_career_resources_for_category_read(base_dir, career_id, &category)?;
    let previous_champions = get_previous_champions_in_base_dir(base_dir, career_id, &category)?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;
    let active_season_number = season.numero;

    if categories::is_multiclass_category(&category) {
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
    let teams = if !categories::uses_regular_teams(&category)
        && categories::runs_in_special_phase(&category)
    {
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
            // Escalar do carro que o SIM usa (peças > coluna legada) — calculado antes do
            // literal porque os campos `nome`/`nome_curto` movem o `team`.
            let car_performance = team.effective_car_performance();

            TeamStanding {
                posicao: 0,
                id: team_id.clone(),
                nome: team.nome,
                nome_curto: team.nome_curto,
                cor_primaria: team.cor_primaria,
                cash_balance: team.cash_balance,
                car_performance,
                car_level: team.car.as_ref().map(|c| c.display_level()).unwrap_or(1),
                confiabilidade: team.confiabilidade,
                pit_crew_quality: team.pit_crew_quality,
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
                historico_vitorias: team.historico_vitorias,
                historico_podios: team.historico_podios,
                historico_titulos_construtores: team.historico_titulos_construtores,
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
            let car_performance = team.effective_car_performance();

            Ok(TeamStanding {
                posicao: index as i32 + 1,
                id: team_id.clone(),
                nome: team.nome,
                nome_curto: team.nome_curto,
                cor_primaria: team.cor_primaria,
                cash_balance: team.cash_balance,
                car_performance,
                car_level: team.car.as_ref().map(|c| c.display_level()).unwrap_or(1),
                confiabilidade: team.confiabilidade,
                pit_crew_quality: team.pit_crew_quality,
                founded_year,
                pontos: row.points.round() as i32,
                vitorias: row.wins,
                piloto_1_nome: driver_names.first().cloned(),
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
                historico_vitorias: team.historico_vitorias,
                historico_podios: team.historico_podios,
                historico_titulos_construtores: team.historico_titulos_construtores,
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
    let (db, _, _) = open_career_resources_for_category_read(base_dir, career_id, &category)?;
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
            thematic_slot: race.thematic_slot.as_str().to_string(),
            event_interest: None,
            public_fame_share: None,
        })
        .collect())
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
        car_performance: team.effective_car_performance(),
        car_level: team.car.as_ref().map(|c| c.display_level()).unwrap_or(1),
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

pub(crate) fn build_primary_rival_summary(
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

fn cleanup_legacy_special_state_for_9d_transition(
    conn: &rusqlite::Connection,
    season_number: i32,
) -> Result<(), String> {
    conn.execute(
        "UPDATE contracts
         SET status = 'Expirado'
         WHERE tipo = 'Especial'
           AND status = 'Ativo'
           AND temporada_inicio = ?1",
        rusqlite::params![season_number],
    )
    .map_err(|e| format!("Falha ao expirar contratos especiais legados: {e}"))?;

    conn.execute(
        "UPDATE drivers
         SET categoria_especial_ativa = NULL
         WHERE categoria_especial_ativa IS NOT NULL",
        [],
    )
    .map_err(|e| format!("Falha ao limpar categoria especial ativa legada: {e}"))?;

    Ok(())
}

#[cfg(test)]
#[path = "career/tests/mod.rs"]
mod tests;
