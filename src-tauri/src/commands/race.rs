use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::commands::race_history::append_race_result;
use crate::config::app_config::{AppConfig, SaveMeta};
use crate::constants::categories::{
    get_all_categories, get_category_config, is_multiclass_category, runs_in_special_phase,
    CategoryConfig,
};
use crate::constants::historical_timeline::is_team_active_in_year;
use crate::constants::scoring::{get_points_for_position, BONUS_FASTEST_LAP};
use crate::db::connection::Database;
use crate::db::connection::DbError;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::special_team_entries as special_entry_queries;
use crate::db::queries::standings as standings_queries;
use crate::db::queries::standings::ChampionshipContext;
use crate::db::queries::teams as team_queries;
use crate::db::queries::track_history as track_history_queries;
use crate::event_interest::{
    calculate_expected_event_interest, calculate_realized_event_interest, EventInterestContext,
    InterestTier, RealizedEventInterest,
};
use crate::finance::cashflow::{apply_round_cashflow, TeamRoundFinanceContext};
use crate::finance::economy::{
    economy_cost_modifier, economy_income_modifier, global_economic_health_for_season,
    GlobalEconomicHealth,
};
use crate::finance::events::{apply_crisis_event_if_needed, debt_service_for_state};
use crate::finance::planning::{calculate_financial_plan, category_finance_scale};
use crate::finance::state::refresh_team_financial_state;
use crate::models::driver::Driver;
use crate::models::injury::Injury;
use crate::models::season::Season;
use crate::simulation::batch::{
    BriefRaceResult, CategorySimResult, SimHighlight, SimultaneousResults,
};
use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::engine::run_full_race_with_breakdowns;
use crate::simulation::incidents::IncidentResult;
use crate::simulation::race::RaceResult;
use crate::{calendar::CalendarEntry, models::team::Team};

#[path = "race/comum.rs"]
mod comum;
#[path = "race/financas.rs"]
mod financas;
#[path = "race/grade.rs"]
mod grade;
#[path = "race/importacao.rs"]
mod importacao;
#[path = "race/manutencao.rs"]
mod manutencao;
#[path = "race/persistencia.rs"]
mod persistencia;
#[path = "race/simulacao.rs"]
mod simulacao;

pub(crate) use comum::*;
pub(crate) use financas::*;
pub(crate) use grade::*;
pub(crate) use importacao::*;
pub(crate) use manutencao::*;
pub(crate) use persistencia::*;
pub(crate) use simulacao::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceWeekendResult {
    pub player_race: RaceResult,
    pub other_categories: SimultaneousResults,
    /// Avaliação de carreira (expectativa vs resultado, nota, frases) — o MESMO
    /// cérebro do import do iRacing. `None` se não der para avaliar (tela trata).
    #[serde(default)]
    pub evaluation: Option<crate::race_eval::RaceEvaluation>,
    /// Fatura de manutenção do carro (gasolina/pneus; sim offline não tem batida).
    #[serde(default)]
    pub maintenance: MaintenanceBreakdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RacePersistenceMode {
    Playable,
    HistoricalDraft,
}

#[tauri::command]
pub async fn simulate_race_weekend(
    app: AppHandle,
    career_id: String,
    race_id: String,
) -> Result<RaceWeekendResult, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    simulate_race_weekend_in_base_dir(&base_dir, &career_id, &race_id)
}

#[tauri::command]
pub async fn simulate_special_block(
    app: AppHandle,
    career_id: String,
) -> Result<SimultaneousResults, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;

    simulate_special_block_in_base_dir(&base_dir, &career_id)
}

pub(crate) fn simulate_race_weekend_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    race_id: &str,
) -> Result<RaceWeekendResult, String> {
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

    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let mut race_entry = calendar_queries::get_calendar_entry_by_id(&db.conn, race_id)
        .map_err(|e| format!("Falha ao buscar corrida: {e}"))?
        .ok_or_else(|| "Corrida nao encontrada.".to_string())?;

    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    // Validar que a corrida pertence à temporada atual e está pendente
    if race_entry.season_id != active_season.id {
        return Err("A corrida selecionada nao pertence a temporada atual.".to_string());
    }
    if race_entry.status.as_str() != "Pendente" {
        return Err("A corrida selecionada ja foi concluida ou simulada.".to_string());
    }
    let expected_player_race = get_next_player_race(&db.conn, &active_season)?;
    match expected_player_race {
        Some(expected) if expected.id == race_entry.id => {}
        Some(expected) => {
            return Err(format!(
                "A corrida selecionada nao e a proxima corrida valida do jogador. Proxima esperada: {} ({})",
                expected.track_name, expected.categoria
            ));
        }
        None => {
            return Err(
                "O jogador nao possui corrida pendente valida para simular nesta fase.".to_string(),
            );
        }
    }

    // FONTE ÚNICA do clima: resolve pelo MESMO generate_weather do export (e persiste no
    // entry.clima), pra a simulação offline produzir o resultado que o iRacing rodaria.
    if let Some(track) = crate::constants::tracks::get_track(race_entry.track_id) {
        let cal = calendar_queries::get_calendar(&db.conn, &race_entry.season_id, &race_entry.categoria)
            .unwrap_or_default();
        let career_first = cal.iter().all(|e| e.status.as_str() != "Concluida");
        let first_week = cal.iter().map(|e| e.week_of_year).min().unwrap_or(i32::MAX);
        let is_first = career_first && race_entry.week_of_year == first_week;
        race_entry.clima = crate::commands::iracing::resolve_and_persist_race_weather(
            &db.conn,
            career_id,
            track,
            race_entry.week_of_year,
            &race_entry.id,
            is_first,
        );
    }

    let (player_race, player_new_injuries) = simulate_category_race(&mut db, &race_entry, true)?;

    // Repercussão pós-corrida: fama (jogador + IA, modulada por carisma), deriva de
    // carisma e decaimento passivo. MESMA lógica da corrida importada do iRacing.
    let (post_race_bias, interest_tier) =
        apply_post_race_fame(&db.conn, &race_entry, &player_race, &player_new_injuries)?;

    warn_if_side_effect_fails(
        append_race_result(
            &career_dir,
            &race_entry.categoria,
            race_entry.rodada,
            &player_race.race_results,
        ),
        "Falha ao gravar race_results.json da corrida do jogador",
    );
    warn_if_side_effect_fails(
        track_history_queries::record_race_dnfs(
            &db.conn,
            &player_race.race_results,
            &race_entry.track_name,
            active_season.numero,
            race_entry.rodada,
        )
        .map_err(|e| format!("Falha ao registrar historico de DNF da corrida do jogador: {e}")),
        "Falha ao registrar historico de DNF da corrida do jogador",
    );
    match persist_race_news(
        &db.conn,
        &player_race,
        &active_season,
        race_entry.rodada,
        &race_entry.categoria,
        post_race_bias,
        race_entry.thematic_slot,
        &interest_tier,
        &player_race
            .race_results
            .iter()
            .flat_map(|r| r.incidents.clone())
            .collect::<Vec<_>>(),
        &player_new_injuries,
        &[], // sem telemetria real no fluxo simulado
    ) {
        // Corrida do jogador gerou notícia de Corrida → PRÉ-GERA o boletim de IA em
        // background agora, para já estar em cache quando o jogador abrir Notícias
        // (sem sentir a latência de ~5s do servidor).
        Ok(Some(news_id)) => {
            let mut cfg = AppConfig::load_or_default(base_dir);
            let install_id = cfg.get_or_create_install_id();
            let lang = cfg.language.clone();
            spawn_prewarm_boletim(db_path.clone(), news_id, lang, install_id);
        }
        Ok(None) => {}
        Err(e) => eprintln!("Falha ao persistir noticias da corrida do jogador: {e}"),
    }
    let other_categories = simulate_other_categories(
        &mut db,
        &career_dir,
        &race_entry.categoria,
        calendar_queries::calendar_entry_season_week(&race_entry),
        &race_entry.display_date,
        &active_season.id,
        active_season.numero,
    )?;
    warn_if_side_effect_fails(
        update_last_played(&meta_path),
        "Falha ao atualizar meta.json apos a corrida",
    );

    // Cérebro do pós-corrida: mesma avaliação do import do iRacing (expectativa vs
    // resultado, nota, frases). O sim offline NÃO tem telemetria ao vivo, então a
    // tela mostra a leitura de carreira + saldo de posições e esconde os gráficos.
    let evaluation = compute_race_evaluation(&db.conn, &player_race);

    // Conserto na simulação: DNF do jogador cobra como uma batida leve. "leve" cobra
    // 0 (reservado a batidas do iRacing que cruzaram a linha), então mapeamos o DNF de
    // sim pro tier mais leve que cobra ("moderado"). Debitado do caixa, igual ao import.
    let player_entry = player_race.race_results.iter().find(|r| r.is_jogador);
    let mut repair_cost = 0.0;
    let mut repair_severity = "nenhum";
    if player_entry.map(|r| r.is_dnf).unwrap_or(false) {
        repair_severity = "moderado";
        if let Some(pe) = player_entry {
            if let Ok(Some(mut team)) = team_queries::get_team_by_id(&db.conn, &pe.team_id) {
                let mut rng = rand::thread_rng();
                let cost = compute_repair_cost(
                    repair_severity,
                    &team.categoria,
                    team.car_performance,
                    &mut rng,
                );
                if cost > 0.0 {
                    team.cash_balance -= cost;
                    team.last_round_expenses += cost;
                    let _ = team_queries::update_team(&db.conn, &team);
                    repair_cost = cost;
                }
            }
        }
    }

    // Fatura de manutenção do carro (gasolina/pneus + conserto, se houve DNF).
    let maintenance = compute_maintenance_breakdown(
        &race_entry.categoria,
        player_entry.map(|r| r.final_tire_wear).unwrap_or(0.0),
        player_entry.map(|r| r.laps_completed).unwrap_or(0),
        repair_cost,
        repair_severity,
    );

    // Persiste a tela do pós-corrida para reabrir depois pela Home (offline não
    // tem telemetria ao vivo → sem gráficos, mas tem cérebro + saldo + manutenção).
    save_race_screen(
        &career_dir,
        race_id,
        &serde_json::json!({
            "race_result": &player_race,
            "evaluation": &evaluation,
            "telemetry": serde_json::Value::Null,
            "maintenance": &maintenance,
        }),
    );

    Ok(RaceWeekendResult {
        player_race,
        other_categories,
        evaluation,
        maintenance,
    })
}

pub(crate) fn simulate_special_block_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<SimultaneousResults, String> {
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

    let mut db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let active_season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
        .ok_or_else(|| "Temporada ativa nao encontrada.".to_string())?;

    if active_season.fase != crate::models::enums::SeasonPhase::BlocoEspecial {
        return Err("O fast-sim do bloco especial so pode ocorrer em BlocoEspecial.".to_string());
    }

    let player = driver_queries::get_player_driver(&db.conn)
        .map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    if player.categoria_especial_ativa.is_some() {
        return Err(
            "O jogador participa do bloco especial ativo e deve correr essa fase normalmente."
                .to_string(),
        );
    }

    let mut categories_simulated = Vec::new();
    let mut total_races_simulated = 0;
    let mut highlights = Vec::new();

    for category_id in ["production_challenger", "endurance"] {
        let pending = calendar_queries::get_pending_races_for_category(
            &db.conn,
            &active_season.id,
            category_id,
        )
        .map_err(|e| format!("Falha ao buscar corridas pendentes de {}: {e}", category_id))?;

        if pending.is_empty() {
            continue;
        }

        let category = get_category_config(category_id)
            .ok_or_else(|| format!("Categoria '{}' nao encontrada.", category_id))?;

        let mut summaries = Vec::new();
        for entry in pending {
            let (result, special_injuries) = simulate_category_race(&mut db, &entry, false)?;
            // Fama do MUNDO nas categorias especiais também (jogador ou IA). Best-effort.
            let _ = apply_post_race_fame(&db.conn, &entry, &result, &special_injuries);
            warn_if_side_effect_fails(
                append_race_result(
                    &career_dir,
                    &entry.categoria,
                    entry.rodada,
                    &result.race_results,
                ),
                "Falha ao gravar race_results.json do bloco especial",
            );
            warn_if_side_effect_fails(
                track_history_queries::record_race_dnfs(
                    &db.conn,
                    &result.race_results,
                    &entry.track_name,
                    active_season.numero,
                    entry.rodada,
                )
                .map_err(|e| format!("Falha ao registrar historico de DNF do bloco especial: {e}")),
                "Falha ao registrar historico de DNF do bloco especial",
            );

            let winner = result
                .race_results
                .iter()
                .find(|driver| driver.finish_position == 1);
            summaries.push(BriefRaceResult {
                race_id: entry.id.clone(),
                track_name: entry.track_name.clone(),
                winner_name: winner
                    .map(|driver| driver.pilot_name.clone())
                    .unwrap_or_default(),
                winner_team: winner
                    .map(|driver| driver.team_name.clone())
                    .unwrap_or_default(),
            });
            total_races_simulated += 1;
        }

        if let Some(last) = summaries.last() {
            highlights.push(SimHighlight {
                headline: format!(
                    "{} vence em {} ({})",
                    last.winner_name, last.track_name, category.nome_curto
                ),
                category: category_id.to_string(),
            });
        }

        categories_simulated.push(CategorySimResult {
            category_id: category_id.to_string(),
            category_name: category.nome.to_string(),
            races_simulated: summaries.len() as i32,
            results: summaries,
        });
    }

    warn_if_side_effect_fails(
        persist_other_category_news(&db.conn, &highlights, active_season.numero),
        "Falha ao persistir noticias de outras categorias do bloco especial",
    );
    warn_if_side_effect_fails(
        update_last_played(&meta_path),
        "Falha ao atualizar meta.json apos o bloco especial",
    );

    Ok(SimultaneousResults {
        categories_simulated,
        total_races_simulated,
        highlights,
    })
}

/// Roda o "cérebro" do pós-corrida (`race_eval`) sobre um `RaceResult` + o banco:
/// monta o mérito de cada piloto (skill + carro + forma recente) e avalia o
/// resultado do JOGADOR (expectativa vs resultado, nota, frases). `None` se não
/// houver jogador no resultado — a tela trata o `None` e nunca quebra.
/// Persiste o PAYLOAD da tela de pós-corrida (resultado + avaliação + telemetria)
/// por corrida, para o jogador reabrir a classificação final depois pela Home.
/// Efêmero vira durável: `career_dir/race_screens/<race_id>.json`. Best-effort.
pub(crate) fn save_race_screen(career_dir: &Path, race_id: &str, payload: &serde_json::Value) {
    let dir = career_dir.join("race_screens");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(s) = serde_json::to_string(payload) {
        let _ = std::fs::write(dir.join(format!("{race_id}.json")), s);
    }
}

/// Reabre a tela salva da corrida da rodada `rodada` na categoria. Resolve
/// rodada→race_id pelo calendário da temporada ATIVA e lê o arquivo. `None` se
/// não houver tela salva (corrida antiga / outra categoria / nunca jogada).
#[tauri::command]
pub fn get_saved_race_screen(
    app: AppHandle,
    career_id: String,
    category: String,
    rodada: i32,
) -> Result<Option<serde_json::Value>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let db_path = career_dir.join("career.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let Some(active_season) = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao buscar temporada ativa: {e}"))?
    else {
        return Ok(None);
    };
    let entries = calendar_queries::get_calendar(&db.conn, &active_season.id, &category)
        .map_err(|e| format!("Falha ao buscar calendário: {e}"))?;
    let Some(entry) = entries.into_iter().find(|e| e.rodada == rodada) else {
        return Ok(None);
    };
    let path = career_dir.join("race_screens").join(format!("{}.json", entry.id));
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let mut v = match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
            // Injeta o race_id (não estava no payload salvo) p/ o front reconstruir o clima.
            if let Some(obj) = v.as_object_mut() {
                obj.insert("race_id".into(), serde_json::Value::String(entry.id.clone()));
            }
            Ok(Some(v))
        }
        Err(_) => Ok(None),
    }
}

/// Uma quebra de peça pra UI pós-corrida (Peça 3): já resolvida com o nome do piloto e da peça.
#[derive(serde::Serialize)]
pub struct RaceBreakdownView {
    pub driver_id: String,
    pub driver_name: String,
    /// Chave da peça (`PartType::as_str`).
    pub part: String,
    /// Nome legível da peça na categoria (Motor/Câmbio/Asa dianteira…).
    pub part_name: String,
    pub lap: u32,
    /// "light" | "heavy" | "dnf".
    pub severity: String,
    /// Segundos perdidos no box; `null` = DNF.
    pub penalty_secs: Option<u32>,
    /// Frase do problema concreto (ex.: "motor fundiu por superaquecimento").
    pub label: String,
    pub is_player: bool,
}

/// Quebras de peça de uma corrida, pra tela pós-corrida (resumo no Debrief + detalhe por piloto
/// na Telemetria). Vazio se a corrida não teve quebra (ou é save antigo).
#[tauri::command]
pub fn get_race_breakdowns(
    app: AppHandle,
    career_id: String,
    race_id: String,
) -> Result<Vec<RaceBreakdownView>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let rows = crate::db::queries::race_breakdowns::get_breakdowns_for_race(&db.conn, &race_id)
        .map_err(|e| format!("Falha ao ler quebras: {e}"))?;
    let categoria = calendar_queries::get_calendar_entry_by_id(&db.conn, &race_id)
        .ok()
        .flatten()
        .map(|e| e.categoria)
        .unwrap_or_default();
    let out = rows
        .into_iter()
        .map(|r| {
            let driver = driver_queries::get_driver(&db.conn, &r.driver_id).ok();
            let (driver_name, is_player) = driver
                .as_ref()
                .map(|d| (d.nome.clone(), d.is_jogador))
                .unwrap_or_else(|| (r.driver_id.clone(), false));
            let part_name = crate::car::PartType::from_str(&r.part)
                .map(|pt| pt.display_name(&categoria).to_string())
                .unwrap_or_else(|| r.part.clone());
            RaceBreakdownView {
                driver_id: r.driver_id,
                driver_name,
                part: r.part,
                part_name,
                lap: r.lap,
                severity: r.severity,
                penalty_secs: r.penalty_secs,
                label: r.label,
                is_player,
            }
        })
        .collect();
    Ok(out)
}

fn race_news_importance(
    bias: i32,
    tier: &InterestTier,
    finish_position: i32,
) -> crate::news::NewsImportance {
    use crate::event_interest::InterestTier;
    use crate::news::NewsImportance;
    let tier_score = match tier {
        InterestTier::Baixo => 0,
        InterestTier::Moderado => 1,
        InterestTier::Alto => 2,
        InterestTier::MuitoAlto => 3,
        InterestTier::EventoPrincipal => 4,
    };
    let position_bonus = if finish_position == 1 {
        2
    } else if finish_position <= 3 {
        1
    } else {
        0
    };
    let total = bias + tier_score + position_bonus;
    let importance = if total >= 5 {
        NewsImportance::Destaque
    } else if total >= 3 {
        NewsImportance::Alta
    } else if total >= 1 {
        NewsImportance::Media
    } else {
        NewsImportance::Baixa
    };
    // Vitória sempre dispara pelo menos Alta para que detect_race_trigger acione LeaderWon/ShockWin/etc.
    if finish_position == 1 && matches!(importance, NewsImportance::Baixa | NewsImportance::Media) {
        NewsImportance::Alta
    } else {
        importance
    }
}

fn persist_race_news(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    active_season: &Season,
    round: i32,
    category_id: &str,
    news_importance_bias: i32,
    _thematic_slot: crate::models::enums::ThematicSlot,
    interest_tier: &InterestTier,
    flat_incidents: &[IncidentResult],
    new_injuries: &[Injury],
    // Fatos extras de pano de fundo (ex.: telemetria REAL do SDK numa corrida
    // importada do iRacing). Vazio no fluxo simulado. Entram na seção "Contexto".
    extra_context_facts: &[String],
) -> Result<Option<String>, String> {
    use crate::db::queries::news as news_queries;
    use crate::generators::ids::{next_id, IdType};
    use crate::news::{NewsImportance, NewsItem, NewsType};

    use crate::db::queries::drivers as driver_queries;

    let now = chrono::Local::now().timestamp();
    let mut items: Vec<NewsItem> = Vec::new();
    // Id da notícia de Corrida do jogador — usado para atrelar os fatos do boletim de IA.
    let mut corrida_news_id: Option<String> = None;

    // 1. Corrida — notícia sobre o VENCEDOR da corrida (não o jogador)
    // O sistema editorial foi projetado para compor histórias sobre quem ganhou.
    // A importância Alta garante que detect_race_trigger gera algo além do FallbackRaceResult.
    {
        let winner_id = &race_result.winner_id;
        let winner_name = driver_queries::get_driver(conn, winner_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| winner_id.clone());
        let importance = race_news_importance(news_importance_bias, interest_tier, 1);

        let total_rodadas = crate::constants::categories::get_category_config(category_id)
            .map(|c| c.corridas_por_temporada as i32)
            .unwrap_or(round);
        let fallback_races = total_rodadas - round;

        let track = race_result.track_name.as_str();
        let (titulo, texto) = if fallback_races == 0 {
            (
                rust_i18n::t!("race.news.win_final_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!("race.news.win_final_text", name = winner_name, season = active_season.numero).to_string(),
            )
        } else if fallback_races <= 2 {
            (
                rust_i18n::t!("race.news.win_crucial_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!("race.news.win_crucial_text", name = winner_name, round = round).to_string(),
            )
        } else {
            (
                rust_i18n::t!("race.news.win_title", name = winner_name, track = track).to_string(),
                rust_i18n::t!(
                    "race.news.win_text",
                    name = winner_name,
                    round = round,
                    season = active_season.numero
                )
                .to_string(),
            )
        };

        let winner_team = race_result
            .race_results
            .iter()
            .find(|r| &r.pilot_id == winner_id)
            .map(|r| r.team_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id news: {e:?}"))?;
        corrida_news_id = Some(id.clone());
        items.push(NewsItem {
            id,
            tipo: NewsType::Corrida,
            icone: NewsType::Corrida.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: importance,
            timestamp: now,
            driver_id: Some(winner_id.clone()),
            driver_id_secondary: None,
            team_id: winner_team.map(Some).unwrap_or(None),
        });

        if fallback_races == 0 {
            if let Ok(standings) = crate::db::queries::race_history::get_category_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                if let Some(champion) = standings.into_iter().next() {
                    let champ_id =
                        next_id(conn, IdType::News).unwrap_or_else(|_| "news_champ".to_string());
                    items.push(NewsItem {
                        id: champ_id,
                        tipo: NewsType::FramingSazonal,
                        icone: NewsType::FramingSazonal.icone().to_string(),
                        titulo: rust_i18n::t!("race.news.champion_title", name = champion.pilot_name.as_str(), season = active_season.numero).to_string(),
                        texto: rust_i18n::t!("race.news.champion_text", rounds = total_rodadas, name = champion.pilot_name.as_str()).to_string(),
                        rodada: Some(round),
                        semana_pretemporada: None,
                        temporada: active_season.numero,
                        categoria_id: Some(category_id.to_string()),
                        categoria_nome: None,
                        importancia: NewsImportance::Destaque,
                        timestamp: now,
                        driver_id: Some(champion.pilot_id),
                        driver_id_secondary: None,
                        team_id: None,
                    });
                }
            }
        }
    }

    // 2. Incidentes — um item por DNF + incidentes de hint >= 2 não-DNF
    // Evita duplicatas: se um piloto já tem DNF, não gera segundo item por hint >= 2 dele.
    let mut seen_incident_pilots: HashSet<String> = HashSet::new();
    let mut noticiable: Vec<&IncidentResult> = flat_incidents
        .iter()
        .filter(|i| i.is_dnf || i.narrative_importance_hint >= 2)
        .collect();
    // DNFs primeiro, depois por hint decrescente
    noticiable.sort_by_key(|i| {
        (
            std::cmp::Reverse(i.is_dnf as u8),
            std::cmp::Reverse(i.narrative_importance_hint),
        )
    });

    for inc in noticiable {
        if !seen_incident_pilots.insert(inc.pilot_id.clone()) {
            continue; // piloto já tem notícia nesta rodada
        }
        let driver_name = driver_queries::get_driver(conn, &inc.pilot_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| inc.pilot_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id incident: {e:?}"))?;
        let titulo = if inc.is_dnf {
            format!("{} abandona a corrida após incidente", driver_name)
        } else {
            format!("{} envolvido em incidente durante a prova", driver_name)
        };
        let texto = inc.description.clone();
        let inc_importance = if inc.narrative_importance_hint >= 3 {
            NewsImportance::Destaque
        } else {
            NewsImportance::Alta
        };
        items.push(NewsItem {
            id,
            tipo: NewsType::Incidente,
            icone: NewsType::Incidente.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: inc_importance,
            timestamp: now,
            driver_id: Some(inc.pilot_id.clone()),
            driver_id_secondary: inc.linked_pilot_id.clone(),
            team_id: None,
        });
    }

    // 3. Lesão — uma notícia por piloto lesionado
    for injury in new_injuries {
        let driver_name = driver_queries::get_driver(conn, &injury.pilot_id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| injury.pilot_id.clone());
        let id = next_id(conn, IdType::News).map_err(|e| format!("next_id injury: {e:?}"))?;
        let titulo = "desfalque confirmado".to_string();
        let texto = format!(
            "{} está fora da próxima etapa após lesão confirmada. Situação será reavaliada nos próximos dias.",
            driver_name
        );
        items.push(NewsItem {
            id,
            tipo: NewsType::Lesao,
            icone: NewsType::Lesao.icone().to_string(),
            titulo,
            texto,
            rodada: Some(round),
            semana_pretemporada: None,
            temporada: active_season.numero,
            categoria_id: Some(category_id.to_string()),
            categoria_nome: None,
            importancia: NewsImportance::Alta,
            timestamp: now,
            driver_id: Some(injury.pilot_id.clone()),
            driver_id_secondary: None,
            team_id: None,
        });
    }

    if !items.is_empty() {
        news_queries::insert_news_batch(conn, &items)
            .map_err(|e| format!("insert_news_batch: {e:?}"))?;
    }

    // Boletim de IA (teste via simulação): monta os fatos curados da corrida do
    // jogador e os guarda atrelados à notícia de Corrida, para o comando lazy
    // enviá-los ao servidor quando o jogador abrir a notícia. A fonte trocará
    // para os dados reais do SDK quando a integração corrida-real→carreira existir.
    let returned_news_id = corrida_news_id.clone();
    if let Some(news_id) = corrida_news_id {
        let category_name: &str = match crate::constants::categories::get_category_config(category_id)
        {
            Some(c) => c.nome,
            None => category_id,
        };
        // Lesões ocorridas nesta corrida → viram fatos do boletim (nome resolvido).
        let injury_facts: Vec<String> = new_injuries
            .iter()
            .map(|inj| {
                let name = driver_queries::get_driver(conn, &inj.pilot_id)
                    .map(|d| d.nome)
                    .unwrap_or_else(|_| inj.pilot_id.clone());
                rust_i18n::t!("briefing.ctx.injury", name = name.as_str()).to_string()
            })
            .collect();

        // Contexto de carreira (pano de fundo) dos pilotos em DESTAQUE: vencedor,
        // pódio (2º/3º), maior recuperação e o nosso piloto. Atributos do piloto +
        // histórico de pista — sem dependência de ordem de inserção.
        let mut context_facts: Vec<String> = Vec::new();
        let mut featured: Vec<String> = vec![race_result.winner_id.clone()];
        for d in &race_result.race_results {
            if !d.is_dnf && (d.finish_position == 2 || d.finish_position == 3) {
                featured.push(d.pilot_id.clone());
            }
        }
        if let Some(id) = &race_result.most_positions_gained_id {
            featured.push(id.clone());
        }
        if let Some(p) = race_result.race_results.iter().find(|d| d.is_jogador) {
            featured.push(p.pilot_id.clone());
        }
        featured.sort();
        featured.dedup();

        for pilot_id in &featured {
            let driver = match driver_queries::get_driver(conn, pilot_id) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let is_winner = *pilot_id == race_result.winner_id;

            // Rookie em destaque → valoriza a estreia. Veterano → só o vencedor (evita poluir).
            if driver.temporadas_na_categoria == 0 {
                context_facts.push(
                    rust_i18n::t!("briefing.ctx.rookie_debut", name = driver.nome.as_str())
                        .to_string(),
                );
            } else if is_winner && driver.temporadas_na_categoria >= 5 {
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.veteran",
                        name = driver.nome.as_str(),
                        season = driver.temporadas_na_categoria + 1
                    )
                    .to_string(),
                );
            }

            // Histórico de pista: já abandonou aqui antes? (gosto de superação — só
            // para quem TERMINOU hoje, senão seria o abandono desta própria corrida).
            let dnfd_this_race = race_result
                .race_results
                .iter()
                .any(|d| d.pilot_id == *pilot_id && d.is_dnf);
            if !dnfd_this_race {
                if let Ok(Some(_)) = crate::db::queries::track_history::get_pilot_dnf_at_track(
                    conn,
                    pilot_id,
                    &race_result.track_name,
                ) {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.overcame_dnf_here",
                            name = driver.nome.as_str()
                        )
                        .to_string(),
                    );
                }
            }
        }

        // Grava os DNFs desta corrida no histórico por pista — SÓ AGORA (depois de
        // ler os abandonos ANTERIORES acima), senão o abandono de hoje contaria como
        // "visita anterior" e a narrativa de superação dispararia errado. Camada
        // narrativa, não factual: erro (ex.: reprocessar a mesma etapa) é silencioso.
        let _ = crate::db::queries::track_history::record_race_dnfs(
            conn,
            &race_result.race_results,
            &race_result.track_name,
            active_season.numero,
            round,
        );

        // --- Recordes e marcos da categoria (todas as temporadas) — peso histórico. ---
        // Os agregados já incluem a corrida atual (persistida antes daqui).
        {
            let winner_id = &race_result.winner_id;
            let winner_name = driver_queries::get_driver(conn, winner_id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| winner_id.clone());
            let records = crate::db::queries::race_history::get_category_records(conn, category_id)
                .ok();

            // Sequência de vitórias do vencedor (feito em destaque).
            if let Ok(streak) = crate::db::queries::race_history::get_win_streak(
                conn,
                winner_id,
                &active_season.id,
                category_id,
            ) {
                if streak >= 3 {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.win_streak",
                            name = winner_name.as_str(),
                            n = streak
                        )
                        .to_string(),
                    );
                }
            }

            // Carreira do vencedor na categoria (para caça a rival e recorde batido).
            let winner_career = crate::db::queries::race_history::get_driver_category_career(
                conn,
                winner_id,
                category_id,
            )
            .ok();

            // Caça a um rival que AINDA está no grid: vencedor a poucas vitórias de
            // igualar alguém logo acima dele no total histórico da categoria.
            if let Some(wc) = &winner_career {
                if let Ok(actives) =
                    crate::db::queries::race_history::get_active_category_win_counts(
                        conn,
                        category_id,
                    )
                {
                    let target = actives
                        .iter()
                        .filter(|a| {
                            a.pilot_id != *winner_id
                                && a.value > wc.wins
                                && a.value - wc.wins <= 3
                        })
                        .min_by_key(|a| a.value - wc.wins);
                    if let Some(t) = target {
                        let diff = t.value - wc.wins;
                        let plural = if diff == 1 {
                            rust_i18n::t!("briefing.ctx.win_singular")
                        } else {
                            rust_i18n::t!("briefing.ctx.win_plural")
                        };
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.chasing_rival",
                                wins = wc.wins,
                                name = winner_name.as_str(),
                                diff = diff,
                                word = plural,
                                target = t.pilot_name.as_str(),
                                value = t.value
                            )
                            .to_string(),
                        );
                    }
                }
            }

            // Recorde de vitórias da categoria SUPERADO hoje: o vencedor passou a marca
            // anterior (era o 2º+1). Diz há quanto tempo a marca resistia, sem nomear o
            // dono anterior. Só vale se a marca era antiga (>= 2 anos) e não-trivial.
            if let (Some(recs), Some(wc)) = (records.as_ref(), winner_career.as_ref()) {
                let is_new_record = recs
                    .most_wins
                    .as_ref()
                    .map_or(false, |m| m.pilot_id == *winner_id && m.value == wc.wins);
                if is_new_record
                    && wc.wins >= 3
                    && recs.second_most_wins == Some(wc.wins - 1)
                {
                    if let Ok(Some(year)) =
                        crate::db::queries::race_history::first_year_reaching_wins(
                            conn,
                            category_id,
                            wc.wins - 1,
                        )
                    {
                        let dur = active_season.ano - year;
                        if dur >= 2 {
                            context_facts.push(
                                rust_i18n::t!(
                                    "briefing.ctx.new_win_record",
                                    name = winner_name.as_str(),
                                    years = dur
                                )
                                .to_string(),
                            );
                        }
                    }
                }
            }

            // Vencedor é quem mais venceu pela própria equipe na categoria.
            if let Some(cur) = race_result
                .race_results
                .iter()
                .find(|d| d.pilot_id == *winner_id)
            {
                if let Ok(Some(top)) =
                    crate::db::queries::race_history::get_team_top_winner_in_category(
                        conn,
                        &cur.team_id,
                        category_id,
                    )
                {
                    if top.pilot_id == *winner_id && top.value >= 2 {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.team_top_winner",
                                name = winner_name.as_str(),
                                team = cur.team_name.as_str(),
                                wins = top.value
                            )
                            .to_string(),
                        );
                    }
                }
            }

            // Memória temporal dos recordes (vitórias e pódios): registra QUANDO o
            // recorde all-time da categoria é batido, para notícias de "recorde
            // quebrado" com data e o rodapé do mundo. Condição: o recordista PONTUOU
            // hoje na métrica (então o recorde avançou nesta corrida) e é dono ISOLADO
            // do topo (2º colocado == valor-1). `previous = valor-1` (cada corrida soma
            // no máximo 1). Pisos evitam marcos triviais. Idempotente por valor.
            if let Some(recs) = records.as_ref() {
                let record_milestone = |metric: &str, r: &crate::db::queries::race_history::CategoryRecord| {
                    let _ = crate::db::queries::milestones::insert_milestone(
                        conn,
                        category_id,
                        &crate::db::queries::milestones::RecordMilestone {
                            metric: metric.to_string(),
                            pilot_id: r.pilot_id.clone(),
                            pilot_name: r.pilot_name.clone(),
                            value: r.value,
                            previous_value: Some(r.value - 1),
                            context: String::new(),
                            season_number: active_season.numero,
                            ano: active_season.ano,
                            round,
                        },
                    );
                };
                // Vitórias: o recordista é o vencedor de hoje e abriu a marca.
                if let Some(r) = recs.most_wins.as_ref() {
                    if r.pilot_id == *winner_id
                        && r.value >= 5
                        && recs.second_most_wins == Some(r.value - 1)
                    {
                        record_milestone("wins", r);
                    }
                }
                // Pódios: o recordista subiu ao pódio hoje e abriu a marca.
                if let Some(r) = recs.most_podiums.as_ref() {
                    let podium_today = race_result.race_results.iter().any(|d| {
                        d.pilot_id == r.pilot_id && !d.is_dnf && (1..=3).contains(&d.finish_position)
                    });
                    if podium_today
                        && r.value >= 10
                        && recs.second_most_podiums == Some(r.value - 1)
                    {
                        record_milestone("podiums", r);
                    }
                }
                // Poles: o recordista fez a pole de hoje e abriu a marca.
                if let Some(r) = recs.most_poles.as_ref() {
                    if r.pilot_id == race_result.pole_sitter_id
                        && r.value >= 5
                        && recs.second_most_poles == Some(r.value - 1)
                    {
                        record_milestone("poles", r);
                    }
                }
            }

            // Recordes de VITÓRIA do vencedor de hoje: mais vitórias numa temporada,
            // dono da pista (mais vitórias no circuito) e maior sequência de vitórias.
            {
                use crate::db::queries::{milestones, race_history};
                let win_milestone =
                    |metric: &str, value: i32, previous: Option<i32>, context: String| {
                        let _ = milestones::insert_milestone(
                            conn,
                            category_id,
                            &milestones::RecordMilestone {
                                metric: metric.to_string(),
                                pilot_id: winner_id.clone(),
                                pilot_name: winner_name.clone(),
                                value,
                                previous_value: previous,
                                context,
                                season_number: active_season.numero,
                                ano: active_season.ano,
                                round,
                            },
                        );
                    };

                // (a) Mais vitórias numa única temporada (recorde de `standings`, que só
                // tem temporadas encerradas → a atual não entra e não conta duplicado).
                if let Ok(season_wins) = race_history::get_category_wins_this_season(
                    conn,
                    winner_id,
                    &active_season.id,
                    category_id,
                ) {
                    let prev = race_history::get_category_single_season_win_record(conn, category_id)
                        .ok()
                        .flatten()
                        .map(|r| r.value);
                    if season_wins >= 5 && prev.map_or(true, |p| season_wins > p) {
                        win_milestone("season_wins", season_wins, prev, String::new());
                    }
                }

                // (b) Dono da pista: vencedor virou o maior vencedor isolado do circuito.
                if let Ok(track_wins) = race_history::get_pilot_track_wins(
                    conn,
                    winner_id,
                    category_id,
                    &race_result.track_name,
                ) {
                    let others = race_history::get_track_win_leader_excluding(
                        conn,
                        category_id,
                        &race_result.track_name,
                        winner_id,
                    )
                    .unwrap_or(0);
                    if track_wins >= 3 && track_wins > others {
                        win_milestone(
                            "track_wins",
                            track_wins,
                            Some(track_wins - 1),
                            race_result.track_name.clone(),
                        );
                    }
                }

                // (c) Maior sequência de vitórias (na temporada). O "recorde atual" vive
                // nos próprios marcos → só anuncia quando supera o maior já registrado.
                if let Ok(streak) =
                    race_history::get_win_streak(conn, winner_id, &active_season.id, category_id)
                {
                    let streak = streak as i32;
                    let prev = milestones::get_max_milestone_value(conn, category_id, "win_streak")
                        .ok()
                        .flatten();
                    if streak >= 4 && prev.map_or(true, |p| streak > p) {
                        win_milestone("win_streak", streak, prev, String::new());
                    }
                }

                // (d) Maior VENCEDORA da história — a EQUIPE do vencedor de hoje virou a
                // dona isolada do recorde de vitórias da categoria. Guarda a equipe nos
                // campos pilot_* (o rodapé trata a métrica `team_wins` como time).
                if let Some(w) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == *winner_id)
                {
                    let team_wins =
                        crate::db::queries::teams::get_team_category_wins(conn, &w.team_id, category_id)
                            .unwrap_or(0);
                    let others = crate::db::queries::teams::get_category_team_win_leader_excluding(
                        conn,
                        category_id,
                        &w.team_id,
                    )
                    .unwrap_or(0);
                    if team_wins >= 5 && team_wins > others {
                        let _ = milestones::insert_milestone(
                            conn,
                            category_id,
                            &milestones::RecordMilestone {
                                metric: "team_wins".to_string(),
                                pilot_id: w.team_id.clone(),
                                pilot_name: w.team_name.clone(),
                                value: team_wins,
                                previous_value: Some(team_wins - 1),
                                context: String::new(),
                                season_number: active_season.numero,
                                ano: active_season.ano,
                                round,
                            },
                        );
                    }

                    // (e) Recorde de DOBRADINHAS (1-2): só conta quando HOJE foi uma
                    // dobradinha da equipe do vencedor (ele em 1º + outro carro em 2º).
                    let one_two_today = race_result.race_results.iter().any(|d| {
                        d.team_id == w.team_id && !d.is_dnf && d.finish_position == 2
                    });
                    if one_two_today {
                        let count = crate::db::queries::teams::get_team_category_one_two(
                            conn,
                            &w.team_id,
                            category_id,
                        )
                        .unwrap_or(0);
                        let others = crate::db::queries::teams::get_category_one_two_leader_excluding(
                            conn,
                            category_id,
                            &w.team_id,
                        )
                        .unwrap_or(0);
                        if count >= 3 && count > others {
                            let _ = milestones::insert_milestone(
                                conn,
                                category_id,
                                &milestones::RecordMilestone {
                                    metric: "one_two".to_string(),
                                    pilot_id: w.team_id.clone(),
                                    pilot_name: w.team_name.clone(),
                                    value: count,
                                    previous_value: Some(count - 1),
                                    context: String::new(),
                                    season_number: active_season.numero,
                                    ano: active_season.ano,
                                    round,
                                },
                            );
                        }
                    }
                }
            }

            // Recorde de VOLTA da pista: o tempo de volta não é histórico, então o
            // recorde vive em `track_lap_records` (atualizado a cada corrida). Compara a
            // volta mais rápida de HOJE (em memória) com o recorde guardado; se for mais
            // rápida (ou não houver recorde), atualiza. Só emite MARCO quando SUPERA um
            // recorde existente — o inaugural fica guardado em silêncio.
            if !race_result.fastest_lap_id.is_empty() {
                if let Some(fl) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == race_result.fastest_lap_id && d.best_lap_time_ms > 0.0)
                {
                    let lap_ms = fl.best_lap_time_ms.round() as i32;
                    let prev = crate::db::queries::milestones::get_track_lap_record(
                        conn,
                        category_id,
                        &race_result.track_name,
                    )
                    .ok()
                    .flatten();
                    let is_record = prev.as_ref().map_or(true, |(_, _, pms)| lap_ms < *pms);
                    if is_record {
                        let _ = crate::db::queries::milestones::upsert_track_lap_record(
                            conn,
                            category_id,
                            &race_result.track_name,
                            &fl.pilot_id,
                            &fl.pilot_name,
                            lap_ms,
                            active_season.numero,
                            round,
                        );
                        if let Some((_, _, pms)) = prev {
                            let _ = crate::db::queries::milestones::insert_milestone(
                                conn,
                                category_id,
                                &crate::db::queries::milestones::RecordMilestone {
                                    metric: "lap_record".to_string(),
                                    pilot_id: fl.pilot_id.clone(),
                                    pilot_name: fl.pilot_name.clone(),
                                    value: lap_ms,
                                    previous_value: Some(pms),
                                    context: race_result.track_name.clone(),
                                    season_number: active_season.numero,
                                    ano: active_season.ano,
                                    round,
                                },
                            );
                        }
                    }
                }
            }

            // Maior RECUPERAÇÃO da categoria numa corrida (o sim já aponta quem mais
            // ganhou posições hoje). Compara com o recorde histórico ANTES de hoje; só
            // anuncia quando SUPERA um recorde existente (inaugural fica só no histórico).
            if let Some(gain_id) = race_result.most_positions_gained_id.as_ref() {
                if let Some(cur) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.pilot_id == *gain_id && !d.is_dnf && d.grid_position > 0)
                {
                    let gained = cur.grid_position - cur.finish_position;
                    if gained >= 6 {
                        let prev = crate::db::queries::race_history::get_category_comeback_record(
                            conn,
                            category_id,
                            &active_season.id,
                            round,
                        )
                        .ok()
                        .flatten();
                        if let Some(p) = prev {
                            if gained > p.value {
                                let _ = crate::db::queries::milestones::insert_milestone(
                                    conn,
                                    category_id,
                                    &crate::db::queries::milestones::RecordMilestone {
                                        metric: "comeback".to_string(),
                                        pilot_id: cur.pilot_id.clone(),
                                        pilot_name: cur.pilot_name.clone(),
                                        value: gained,
                                        previous_value: Some(p.value),
                                        context: String::new(),
                                        season_number: active_season.numero,
                                        ano: active_season.ano,
                                        round,
                                    },
                                );
                            }
                        }
                    }
                }
            }

            // Recordes escalares (idade/jejum/caóticos) e "de azar" (coroas cumulativas).
            {
                use crate::db::queries::{milestones, race_history};

                // Emite um marco escalar quando o candidato supera o recorde existente.
                let scalar = |kind: &str,
                              subj_id: &str,
                              subj_name: &str,
                              value: i32,
                              context: &str,
                              higher: bool| {
                    if let Ok(Some(prev)) = milestones::update_scalar_and_check(
                        conn,
                        category_id,
                        kind,
                        subj_id,
                        subj_name,
                        value,
                        context,
                        active_season.numero,
                        round,
                        higher,
                    ) {
                        let _ = milestones::insert_milestone(
                            conn,
                            category_id,
                            &milestones::RecordMilestone {
                                metric: kind.to_string(),
                                pilot_id: subj_id.to_string(),
                                pilot_name: subj_name.to_string(),
                                value,
                                previous_value: Some(prev),
                                context: context.to_string(),
                                season_number: active_season.numero,
                                ano: active_season.ano,
                                round,
                            },
                        );
                    }
                };

                // Piloto mais jovem / mais velho a vencer (idade do vencedor hoje).
                let winner_age = driver_queries::get_driver(conn, winner_id)
                    .map(|d| d.idade as i32)
                    .unwrap_or(0);
                if winner_age > 0 {
                    scalar("youngest_winner", winner_id, &winner_name, winner_age, "", false);
                    scalar("oldest_winner", winner_id, &winner_name, winner_age, "", true);
                }

                // Corrida mais caótica da história (mais abandonos numa etapa).
                if race_result.total_dnfs > 0 {
                    scalar(
                        "most_chaotic_race",
                        &race_result.track_name,
                        &race_result.track_name,
                        race_result.total_dnfs,
                        &race_result.track_name,
                        true,
                    );
                }

                // Maior jejum quebrado: o vencedor voltou a vencer após anos sem ganhar.
                if let Ok(Some(prev_win)) = race_history::get_pilot_previous_win_season(
                    conn,
                    winner_id,
                    category_id,
                    active_season.numero,
                    round,
                ) {
                    let drought = active_season.numero - prev_win;
                    if drought >= 3 {
                        scalar("drought_broken", winner_id, &winner_name, drought, "", true);
                    }
                }

                // "De azar" (coroas que trocam de dono): azarão, batedor, poleiro sem
                // título, maior pontuador. Só anuncia quando o dono muda e passa do piso.
                let crown = |kind: &str, leader: Option<race_history::CategoryRecord>, floor: i32| {
                    if let Some(l) = leader {
                        if let Ok(Some((prev_name, prev_val))) =
                            milestones::update_leader_and_check_crown(
                                conn,
                                category_id,
                                kind,
                                &l.pilot_id,
                                &l.pilot_name,
                                l.value,
                                floor,
                                active_season.numero,
                                round,
                            )
                        {
                            let _ = milestones::insert_milestone(
                                conn,
                                category_id,
                                &milestones::RecordMilestone {
                                    metric: kind.to_string(),
                                    pilot_id: l.pilot_id,
                                    pilot_name: l.pilot_name,
                                    value: l.value,
                                    previous_value: Some(prev_val),
                                    context: prev_name,
                                    season_number: active_season.numero,
                                    ano: active_season.ano,
                                    round,
                                },
                            );
                        }
                    }
                };
                crown(
                    "most_starts_no_win",
                    race_history::get_category_most_starts_no_win(conn, category_id).ok().flatten(),
                    30,
                );
                crown(
                    "most_career_dnfs",
                    race_history::get_category_most_career_dnfs(conn, category_id).ok().flatten(),
                    20,
                );
                crown(
                    "most_poles_no_title",
                    race_history::get_category_most_poles_no_title(conn, category_id).ok().flatten(),
                    5,
                );
                crown(
                    "most_career_points",
                    race_history::get_category_most_career_points(conn, category_id).ok().flatten(),
                    300,
                );
            }

            // Por destaque: o RECORDISTA aparece sempre que está em evidência (descreve
            // quem ele é, independe do resultado de hoje). Marcos de número redondo só
            // para quem REALMENTE fez aquilo hoje (venceu / subiu ao pódio / largou).
            for pilot_id in &featured {
                let is_winner = pilot_id == winner_id;
                let is_player = race_result
                    .race_results
                    .iter()
                    .any(|d| d.pilot_id == *pilot_id && d.is_jogador);
                let Ok(career) = crate::db::queries::race_history::get_driver_category_career(
                    conn,
                    pilot_id,
                    category_id,
                ) else {
                    continue;
                };
                let name = driver_queries::get_driver(conn, pilot_id)
                    .map(|d| d.nome)
                    .unwrap_or_else(|_| pilot_id.clone());
                let recs = records.as_ref();
                let holds = |rec: fn(&crate::db::queries::race_history::CategoryRecords) -> &Option<crate::db::queries::race_history::CategoryRecord>| {
                    recs.and_then(|r| rec(r).as_ref())
                        .map_or(false, |m| m.pilot_id == *pilot_id)
                };
                let is_wins_record = holds(|r| &r.most_wins);
                let is_podiums_record = holds(|r| &r.most_podiums);
                let is_starts_record = holds(|r| &r.most_starts);

                // Recordes históricos da categoria (estado — vale sempre que aparece).
                if is_wins_record {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.record_wins",
                            name = name.as_str(),
                            wins = career.wins
                        )
                        .to_string(),
                    );
                }
                if is_podiums_record {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.record_podiums",
                            name = name.as_str(),
                            podiums = career.podiums
                        )
                        .to_string(),
                    );
                }
                if is_starts_record {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.record_starts",
                            name = name.as_str(),
                            starts = career.starts
                        )
                        .to_string(),
                    );
                }

                // Marcos de número redondo — só vencedor e jogador, e só se fez hoje.
                if is_winner || is_player {
                    let podium_today = race_result.race_results.iter().any(|d| {
                        d.pilot_id == *pilot_id
                            && !d.is_dnf
                            && (1..=3).contains(&d.finish_position)
                    });
                    if is_winner && !is_wins_record && [5, 10, 25, 50, 75, 100].contains(&career.wins)
                    {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.nth_win",
                                n = career.wins,
                                name = name.as_str()
                            )
                            .to_string(),
                        );
                    }
                    if podium_today
                        && !is_podiums_record
                        && [25, 50, 100, 150].contains(&career.podiums)
                    {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.nth_podium",
                                name = name.as_str(),
                                n = career.podiums
                            )
                            .to_string(),
                        );
                    }
                    if !is_starts_record && [50, 100, 150, 200, 250].contains(&career.starts) {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.nth_start",
                                n = career.starts,
                                name = name.as_str()
                            )
                            .to_string(),
                        );
                    }
                }
            }
        }

        // --- Duelo interno: quem levou a melhor sobre o companheiro de equipe. ---
        // Só para o vencedor e o jogador (foco), lendo o próprio resultado — o "carro
        // irmão" é a referência mais justa da corrida do piloto. Par deduplicado.
        {
            let mut focus: Vec<&str> = vec![race_result.winner_id.as_str()];
            if let Some(p) = race_result.race_results.iter().find(|d| d.is_jogador) {
                focus.push(p.pilot_id.as_str());
            }
            let mut seen_pairs: std::collections::HashSet<(String, String)> =
                std::collections::HashSet::new();
            for pid in focus {
                let Some(me) = race_result.race_results.iter().find(|d| d.pilot_id == pid) else {
                    continue;
                };
                if me.team_id.is_empty() {
                    continue;
                }
                let Some(mate) = race_result
                    .race_results
                    .iter()
                    .find(|d| d.team_id == me.team_id && d.pilot_id != me.pilot_id)
                else {
                    continue;
                };
                let key = if me.pilot_id <= mate.pilot_id {
                    (me.pilot_id.clone(), mate.pilot_id.clone())
                } else {
                    (mate.pilot_id.clone(), me.pilot_id.clone())
                };
                if !seen_pairs.insert(key) {
                    continue;
                }
                // Quem terminou à frente: melhor posição, ou o único a completar.
                let me_ahead = match (me.is_dnf, mate.is_dnf) {
                    (false, true) => true,
                    (true, false) => false,
                    (false, false) => me.finish_position < mate.finish_position,
                    (true, true) => continue, // ambos fora → sem duelo interno
                };
                let (ahead, team, behind) = if me_ahead {
                    (&me.pilot_name, &me.team_name, &mate.pilot_name)
                } else {
                    (&mate.pilot_name, &mate.team_name, &me.pilot_name)
                };
                context_facts.push(
                    rust_i18n::t!(
                        "briefing.ctx.internal_duel",
                        team = team.as_str(),
                        ahead = ahead.as_str(),
                        behind = behind.as_str()
                    )
                    .to_string(),
                );
            }
        }

        // --- Quadro do campeonato: o que o resultado significa para a temporada. ---
        // Os resultados desta corrida já estão em `race_results` (persistidos em
        // simulate_category_race, antes daqui), então os standings já a incluem.
        let total_rodadas = crate::constants::categories::get_category_config(category_id)
            .map(|c| c.corridas_por_temporada as i32)
            .unwrap_or(round);
        let races_left = (total_rodadas - round).max(0);

        // "Valor de uma vitória" nesta categoria = pontos do vencedor desta corrida.
        // Vira o limiar de "briga apertada" sem depender da escala de pontos: um gap
        // menor que isso é recuperável numa única corrida → a disputa segue viva.
        let win_value = race_result
            .race_results
            .iter()
            .find(|d| d.pilot_id == race_result.winner_id)
            .map(|d| d.points_earned)
            .unwrap_or(0)
            .max(1) as f64;

        // Reta final / próxima é a decisiva.
        match races_left {
            0 => context_facts.push(rust_i18n::t!("briefing.ctx.season_last").to_string()),
            1 => context_facts.push(rust_i18n::t!("briefing.ctx.season_one_left").to_string()),
            2 => context_facts.push(rust_i18n::t!("briefing.ctx.season_two_left").to_string()),
            // "Reta final" só faz sentido quando a temporada de fato já passou da
            // metade. Numa temporada curta (ex.: 5 etapas), 4 restantes significa
            // que só a 1ª rodada foi disputada — não é reta final.
            n if n <= 4 && round * 2 > total_rodadas => context_facts
                .push(rust_i18n::t!("briefing.ctx.season_final_stretch", n = n).to_string()),
            _ => {}
        }

        // Brigas no campeonato só fazem sentido depois de algumas corridas (gap real).
        if round >= 2 {
            // Pilotos: título em aberto (P1×P2) OU, com o líder encaminhado, o vice (P2×P3).
            if let Ok(st) = crate::db::queries::race_history::get_category_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                if st.len() >= 2 {
                    let gap12 = (st[0].points - st[1].points).round();
                    if gap12 <= win_value {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.title_fight",
                                leader = st[0].pilot_name.as_str(),
                                gap = gap12 as i32,
                                second = st[1].pilot_name.as_str()
                            )
                            .to_string(),
                        );
                    } else if st.len() >= 3 {
                        let gap23 = (st[1].points - st[2].points).round();
                        if gap23 <= win_value {
                            context_facts.push(
                                rust_i18n::t!(
                                    "briefing.ctx.vice_fight",
                                    leader = st[0].pilot_name.as_str(),
                                    second = st[1].pilot_name.as_str(),
                                    third = st[2].pilot_name.as_str(),
                                    gap = gap23 as i32
                                )
                                .to_string(),
                            );
                        }
                    }
                }
            }

            // Equipes: mesma lógica (ponta OU vice).
            if let Ok(ts) = crate::db::queries::race_history::get_team_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                if ts.len() >= 2 {
                    let gap12 = (ts[0].points - ts[1].points).round();
                    if gap12 <= win_value {
                        context_facts.push(
                            rust_i18n::t!(
                                "briefing.ctx.teams_top_fight",
                                a = ts[0].team_name.as_str(),
                                b = ts[1].team_name.as_str(),
                                gap = gap12 as i32
                            )
                            .to_string(),
                        );
                    } else if ts.len() >= 3 {
                        let gap23 = (ts[1].points - ts[2].points).round();
                        if gap23 <= win_value {
                            context_facts.push(
                                rust_i18n::t!(
                                    "briefing.ctx.teams_vice_fight",
                                    a = ts[1].team_name.as_str(),
                                    b = ts[2].team_name.as_str(),
                                    gap = gap23 as i32
                                )
                                .to_string(),
                            );
                        }
                    }
                }
            }
        }

        // Final da temporada: piloto em destaque que trocou de equipe e foi parar
        // num time que terminou ATRÁS do que ele deixou — "a aposta saiu cara".
        // Só na última etapa, e só na mesma categoria (troca lateral, não promoção).
        if races_left == 0 {
            if let Ok(ts) = crate::db::queries::race_history::get_team_standings(
                conn,
                &active_season.id,
                category_id,
            ) {
                let pos_of = |team_id: &str| {
                    ts.iter().find(|t| t.team_id == team_id).map(|t| t.position)
                };
                let prev_season = active_season.numero - 1;

                // Candidatos (jogador tem prioridade; senão, a virada mais dramática).
                struct SwitchRegret {
                    pilot_name: String,
                    old_team: String,
                    new_team: String,
                    old_pos: i32,
                    new_pos: i32,
                    is_player: bool,
                }
                let mut candidates: Vec<SwitchRegret> = Vec::new();

                for pilot_id in &featured {
                    let Some(cur) = race_result
                        .race_results
                        .iter()
                        .find(|d| d.pilot_id == *pilot_id)
                    else {
                        continue;
                    };
                    let contracts = match crate::db::queries::contracts::get_contracts_for_pilot(
                        conn, pilot_id,
                    ) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    // Equipe na temporada passada, mesma categoria, time diferente do atual.
                    let Some(prev) = contracts.iter().find(|c| {
                        c.categoria.as_str() == category_id
                            && c.temporada_inicio <= prev_season
                            && c.temporada_fim >= prev_season
                            && c.equipe_id != cur.team_id
                    }) else {
                        continue;
                    };
                    if let (Some(new_pos), Some(old_pos)) =
                        (pos_of(&cur.team_id), pos_of(&prev.equipe_id))
                    {
                        // O time que ele DEIXOU terminou À FRENTE do que ele escolheu.
                        if old_pos < new_pos {
                            candidates.push(SwitchRegret {
                                pilot_name: cur.pilot_name.clone(),
                                old_team: prev.equipe_nome.clone(),
                                new_team: cur.team_name.clone(),
                                old_pos,
                                new_pos,
                                is_player: cur.is_jogador,
                            });
                        }
                    }
                }

                // Emite no máximo um (evita poluir): jogador primeiro, senão maior virada.
                if let Some(r) = candidates
                    .iter()
                    .find(|c| c.is_player)
                    .or_else(|| candidates.iter().max_by_key(|c| c.new_pos - c.old_pos))
                {
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.switch_regret",
                            pilot = r.pilot_name.as_str(),
                            old_team = r.old_team.as_str(),
                            new_team = r.new_team.as_str(),
                            old_pos = r.old_pos,
                            new_pos = r.new_pos
                        )
                        .to_string(),
                    );
                }
            }
        }

        // --- Arco de rivalidade (a "novela"): registra o capítulo de hoje no log de
        // episódios e recapitula o arco para os destaques que se cruzaram na pista. ---
        record_rivalry_episodes(
            conn,
            race_result,
            flat_incidents,
            category_id,
            round,
            active_season.numero,
            active_season.ano,
        );
        for fact in rivalry_arc_facts(conn, race_result, &featured, active_season.numero, round) {
            context_facts.push(fact);
        }

        // Desempenho e forma: esperado×real, forma recente, histórico de pista e
        // confronto entre companheiros (pano de fundo dos destaques).
        for fact in
            performance_context_facts(conn, race_result, &featured, active_season, round, category_id)
        {
            context_facts.push(fact);
        }

        // Telemetria REAL do SDK (só corrida importada do iRacing): ritmo, duelo,
        // erro mais caro, melhor momento — cor sobre a corrida do próprio jogador.
        for fact in extra_context_facts {
            context_facts.push(fact.clone());
        }

        // Peça 3 · notícia: PENALIDADES de quebra (não-DNF) — "perdeu tempo arrumando a peça X,
        // problema leve/grave". Os DNFs de quebra já entram pelo beat Abandono (Camada B); aqui
        // entram as paradas `!black`. Vazio no sim offline (só corrida ao vivo dispara quebra).
        let race_id_for_breakdowns = crate::db::queries::calendar::get_calendar(
            conn,
            &active_season.id,
            category_id,
        )
        .ok()
        .and_then(|entries| entries.into_iter().find(|e| e.rodada == round).map(|e| e.id));
        if let Some(rid) = &race_id_for_breakdowns {
            if let Ok(bds) = crate::db::queries::race_breakdowns::get_breakdowns_for_race(conn, rid) {
                let mut count = 0;
                for b in bds.iter().filter(|b| b.severity != "dnf") {
                    if count >= 6 {
                        break;
                    }
                    let Some(dr) = race_result
                        .race_results
                        .iter()
                        .find(|d| d.pilot_id == b.driver_id)
                    else {
                        continue;
                    };
                    let part_name = crate::car::PartType::from_str(&b.part)
                        .map(|pt| pt.display_name(category_id).to_string())
                        .unwrap_or_else(|| b.part.clone());
                    let grav = if b.severity == "heavy" {
                        rust_i18n::t!("briefing.ctx.severity_heavy")
                    } else {
                        rust_i18n::t!("briefing.ctx.severity_light")
                    };
                    context_facts.push(
                        rust_i18n::t!(
                            "briefing.ctx.breakdown_pit",
                            name = dr.pilot_name.as_str(),
                            team = dr.team_name.as_str(),
                            secs = b.penalty_secs.unwrap_or(0),
                            part = part_name.as_str(),
                            label = b.label.as_str(),
                            severity = grav
                        )
                        .to_string(),
                    );
                    count += 1;
                }
            }
        }

        let ctx = crate::narrative::build_race_context(
            race_result,
            &crate::narrative::RaceContextInput {
                category_name,
                year: active_season.ano,
                round,
                injuries: &injury_facts,
                incidents: flat_incidents,
                context_facts: &context_facts,
            },
        );
        // Mapa nome da equipe → cor primária das equipes desta corrida. O front usa
        // para colorir os nomes das equipes citadas no boletim. Dedup por team_id.
        let mut team_colors = serde_json::Map::new();
        let mut seen_teams: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for d in &race_result.race_results {
            if d.team_name.is_empty() || !seen_teams.insert(d.team_id.as_str()) {
                continue;
            }
            let color: Option<String> = conn
                .query_row(
                    "SELECT cor_primaria FROM teams WHERE id = ?1",
                    rusqlite::params![d.team_id],
                    |r| r.get::<_, String>(0),
                )
                .ok();
            if let Some(c) = color {
                if !c.trim().is_empty() {
                    team_colors.insert(d.team_name.clone(), serde_json::Value::String(c));
                }
            }
        }
        let teams_json = serde_json::Value::Object(team_colors).to_string();

        if let Err(e) =
            crate::db::queries::ai_story::store_race_facts(conn, &news_id, &ctx.facts, &teams_json)
        {
            eprintln!("[narrative] Falha ao guardar fatos do boletim de IA: {e:?}");
        }
    }

    Ok(returned_news_id)
}

/// Pré-gera o boletim de IA em BACKGROUND logo após a corrida, para que ele já
/// esteja em cache quando o jogador abrir a aba de Notícias (sem sentir a latência
/// do servidor). Roda numa thread própria com conexão própria ao banco. Silencioso:
/// se falhar (rede/cooldown), o caminho lazy de abrir a notícia tenta de novo.
fn spawn_prewarm_boletim(
    db_path: std::path::PathBuf,
    news_id: String,
    lang: String,
    install_id: String,
) {
    std::thread::spawn(move || {
        let db = match Database::open_existing(&db_path) {
            Ok(d) => d,
            Err(_) => return,
        };
        let row = match crate::db::queries::ai_story::get_story(&db.conn, &news_id) {
            Ok(Some(r)) => r,
            _ => return,
        };
        if row.story.is_some() {
            return; // já gerado em algum momento — nada a fazer
        }
        // reading_seconds = None → tamanho padrão do servidor (a adaptação por
        // engajamento continua valendo no caminho lazy, se ainda não houver cache).
        if let Ok(story) =
            crate::narrative::client::fetch_story(&row.facts, &lang, &install_id, None)
        {
            let _ = crate::db::queries::ai_story::set_story(&db.conn, &news_id, &story);
        }
    });
}

/// Registra um CAPÍTULO de rivalidade por corrida em que dois rivais interagiram
/// (colisão, duelo de posições coladas, ou briga na ponta). Constrói a memória que o
/// boletim recapitula depois. As intensidades já foram atualizadas na transação da
/// corrida, então aqui só lemos o estado e gravamos o episódio.
fn record_rivalry_episodes(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    flat_incidents: &[IncidentResult],
    categoria: &str,
    rodada: i32,
    temporada: i32,
    ano: i32,
) {
    use crate::db::queries::drivers as driver_queries;
    use crate::models::rivalry::normalize_pair;
    use crate::simulation::incidents::IncidentType;
    use std::collections::{HashMap, HashSet};

    // pilot -> (posição final, dnf)
    let mut info: HashMap<&str, (i32, bool)> = HashMap::new();
    for d in &race_result.race_results {
        info.insert(d.pilot_id.as_str(), (d.finish_position, d.is_dnf));
    }

    // pares que colidiram nesta corrida (normalizados)
    let mut collided: HashSet<(String, String)> = HashSet::new();
    for inc in flat_incidents {
        if inc.incident_type == IncidentType::Collision {
            if let Some(other) = &inc.linked_pilot_id {
                if let Some(pair) = normalize_pair(&inc.pilot_id, other) {
                    collided.insert((pair.piloto1_id, pair.piloto2_id));
                }
            }
        }
    }

    let rivalries = match crate::db::queries::rivalries::get_all_rivalries(conn) {
        Ok(r) => r,
        Err(_) => return,
    };

    for riv in rivalries {
        let Some((pos_a, dnf_a)) = info.get(riv.piloto1_id.as_str()).copied() else {
            continue;
        };
        let Some((pos_b, dnf_b)) = info.get(riv.piloto2_id.as_str()).copied() else {
            continue;
        };

        let perceived = riv.perceived_intensity();
        let pair_key = (riv.piloto1_id.clone(), riv.piloto2_id.clone());
        let did_collide = collided.contains(&pair_key);

        // Colisão sempre vira capítulo (é a origem); duelo/ponta só se já é notável.
        if !did_collide && perceived < 30.0 {
            continue;
        }

        let both_finished = !dnf_a && !dnf_b;
        let close = both_finished && (pos_a - pos_b).abs() <= 3;
        let top_front = both_finished && pos_a <= 5 && pos_b <= 5;

        let interaction = if did_collide {
            "colisao"
        } else if close {
            "duelo"
        } else if top_front {
            "campeonato"
        } else {
            continue; // sem interação de verdade hoje
        };

        // Quem levou a melhor: melhor posição, ou o único a completar.
        let winner_id = if both_finished {
            match pos_a.cmp(&pos_b) {
                std::cmp::Ordering::Less => Some(riv.piloto1_id.clone()),
                std::cmp::Ordering::Greater => Some(riv.piloto2_id.clone()),
                std::cmp::Ordering::Equal => None,
            }
        } else if !dnf_a {
            Some(riv.piloto1_id.clone())
        } else if !dnf_b {
            Some(riv.piloto2_id.clone())
        } else {
            None
        };

        let name = |id: &str| {
            driver_queries::get_driver(conn, id)
                .map(|d| d.nome)
                .unwrap_or_else(|_| id.to_string())
        };
        let na = name(&riv.piloto1_id);
        let nb = name(&riv.piloto2_id);
        let summary = match interaction {
            "colisao" => format!("contato entre {na} e {nb} em {}", race_result.track_name),
            "duelo" => match &winner_id {
                Some(w) => {
                    let (wn, wp, lp) = if *w == riv.piloto1_id {
                        (&na, pos_a, pos_b)
                    } else {
                        (&nb, pos_b, pos_a)
                    };
                    format!("{wn} levou a melhor no duelo direto, {wp}º contra {lp}º")
                }
                None => format!("duelo parelho entre {na} e {nb}"),
            },
            _ => format!("{na} e {nb} brigaram por posições de ponta ({pos_a}º e {pos_b}º)"),
        };

        let ep = crate::db::queries::rivalry_episodes::RivalryEpisode {
            piloto1_id: riv.piloto1_id.clone(),
            piloto2_id: riv.piloto2_id.clone(),
            temporada,
            rodada,
            ano,
            categoria: categoria.to_string(),
            track_name: race_result.track_name.clone(),
            interaction: interaction.to_string(),
            winner_id,
            summary,
            perceived,
        };
        let _ = crate::db::queries::rivalry_episodes::insert_episode(conn, &ep);
    }
}

/// Recapitula o ARCO de rivalidade para os destaques que se cruzaram HOJE: origem,
/// número de capítulos, retrospecto direto (h2h), o capítulo de hoje e revanche.
/// Só para rivalidades já claras (percebida ≥ 40) com capítulo registrado nesta corrida.
fn rivalry_arc_facts(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    featured: &[String],
    temporada: i32,
    rodada: i32,
) -> Vec<String> {
    use crate::db::queries::drivers as driver_queries;
    use crate::models::rivalry::RivalryType;
    use std::collections::HashSet;

    let mut out = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let name = |id: &str| {
        driver_queries::get_driver(conn, id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| id.to_string())
    };

    for pilot_id in featured {
        let rivs = match crate::rivalry::get_pilot_rivalries(conn, pilot_id) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for r in rivs {
            if r.perceived_intensity < 40.0 {
                continue;
            }
            // O rival precisa ter corrido hoje.
            if !race_result
                .race_results
                .iter()
                .any(|d| d.pilot_id == r.rival_id)
            {
                continue;
            }
            // Par normalizado para deduplicar (o par pode vir por ambos os lados).
            let (a, b) = if *pilot_id <= r.rival_id {
                (pilot_id.clone(), r.rival_id.clone())
            } else {
                (r.rival_id.clone(), pilot_id.clone())
            };
            if !seen.insert((a.clone(), b.clone())) {
                continue;
            }

            let eps = match crate::db::queries::rivalry_episodes::get_episodes_for_pair(conn, &a, &b)
            {
                Ok(e) => e,
                Err(_) => continue,
            };
            // Só recapitula se houve capítulo HOJE (a rivalidade se manifestou na corrida).
            let Some(today) = eps
                .last()
                .filter(|e| e.temporada == temporada && e.rodada == rodada)
            else {
                continue;
            };

            let na = name(&a);
            let nb = name(&b);
            let nivel = crate::rivalry::intensity_level(r.perceived_intensity).label();
            let chapters = eps.len();
            let ano_origem = eps.first().map(|e| e.ano).unwrap_or(0);

            // Retrospecto direto (h2h).
            let mut wins_a = 0;
            let mut wins_b = 0;
            for e in &eps {
                match e.winner_id.as_deref() {
                    Some(w) if w == a => wins_a += 1,
                    Some(w) if w == b => wins_b += 1,
                    _ => {}
                }
            }

            // Revanche: hoje X venceu e no capítulo ANTERIOR quem venceu foi o outro.
            let revenge = eps.len() >= 2
                && today.winner_id.is_some()
                && eps
                    .get(eps.len() - 2)
                    .and_then(|p| p.winner_id.as_deref())
                    .zip(today.winner_id.as_deref())
                    .map_or(false, |(prev_w, today_w)| prev_w != today_w);

            let origem = match r.tipo {
                RivalryType::Colisao => rust_i18n::t!("briefing.rivalry.origin_collision"),
                RivalryType::Companheiros => rust_i18n::t!("briefing.rivalry.origin_teammates"),
                RivalryType::Campeonato => rust_i18n::t!("briefing.rivalry.origin_championship"),
                RivalryType::Pista => rust_i18n::t!("briefing.rivalry.origin_track"),
            };

            let mut s = rust_i18n::t!(
                "briefing.rivalry.opener",
                a = na.as_str(),
                b = nb.as_str(),
                level = nivel,
                origin = origem
            )
            .to_string();
            if ano_origem > 0 && chapters > 1 {
                s.push_str(&rust_i18n::t!(
                    "briefing.rivalry.chapters",
                    chapters = chapters,
                    year = ano_origem
                ));
            }
            s.push_str(&rust_i18n::t!(
                "briefing.rivalry.today",
                summary = today.summary.as_str()
            ));
            if wins_a > 0 || wins_b > 0 {
                if wins_a == wins_b {
                    s.push_str(&rust_i18n::t!(
                        "briefing.rivalry.h2h_tied",
                        a = wins_a,
                        b = wins_b
                    ));
                } else {
                    let (leader, hi, lo) = if wins_a > wins_b {
                        (&na, wins_a, wins_b)
                    } else {
                        (&nb, wins_b, wins_a)
                    };
                    s.push_str(&rust_i18n::t!(
                        "briefing.rivalry.h2h_leader",
                        leader = leader.as_str(),
                        hi = hi,
                        lo = lo
                    ));
                }
            }
            if revenge {
                if let Some(tw) = today.winner_id.as_deref() {
                    let twn = if tw == a { &na } else { &nb };
                    s.push_str(&rust_i18n::t!("briefing.rivalry.revenge", name = twn.as_str()));
                }
            }
            out.push(s);
        }
    }
    out
}

/// Fatos de DESEMPENHO e FORMA para os destaques, como pano de fundo do boletim:
/// 1) esperado×real (reaproveita o cérebro `race_eval`: largada + mérito do conjunto);
/// 2) forma recente (últimas 5 corridas na categoria);
/// 3) histórico no circuito (já venceu aqui);
/// 4) confronto entre companheiros de equipe.
/// Tudo gated com folga para não inundar o contexto. Lógica de DB aqui; o módulo
/// `narrative` permanece puro.
fn performance_context_facts(
    conn: &rusqlite::Connection,
    race_result: &RaceResult,
    featured: &[String],
    active_season: &Season,
    round: i32,
    category_id: &str,
) -> Vec<String> {
    use crate::db::queries::drivers as driver_queries;
    use crate::db::queries::race_history as rh;
    use crate::race_eval::{compute_merit, evaluate, Assessment, DriverMerit, RaceEvalInput};
    use std::collections::HashMap;

    let mut out: Vec<String> = Vec::new();
    let rows = &race_result.race_results;
    let field_size = rows.len().max(1) as i32;
    if rows.is_empty() {
        return out;
    }

    // Carrega cada participante uma vez (skill + nome) e o car_performance por equipe.
    let mut drivers: HashMap<String, Driver> = HashMap::new();
    for d in rows {
        if let std::collections::hash_map::Entry::Vacant(e) = drivers.entry(d.pilot_id.clone()) {
            if let Ok(drv) = driver_queries::get_driver(conn, &d.pilot_id) {
                e.insert(drv);
            }
        }
    }
    let mut car_perf: HashMap<String, f64> = HashMap::new();
    for d in rows {
        if let std::collections::hash_map::Entry::Vacant(e) = car_perf.entry(d.team_id.clone()) {
            let cp: f64 = conn
                .query_row(
                    "SELECT car_performance FROM teams WHERE id = ?1",
                    rusqlite::params![d.team_id],
                    |r| r.get(0),
                )
                .unwrap_or(50.0);
            e.insert(cp);
        }
    }
    let name_of = |id: &str| -> String {
        drivers
            .get(id)
            .map(|d| d.nome.clone())
            .unwrap_or_else(|| id.to_string())
    };

    // Campo de mérito (skill + carro pesam igual) — base da posição ESPERADA.
    let field: Vec<DriverMerit> = rows
        .iter()
        .map(|d| {
            let skill = drivers.get(&d.pilot_id).map(|x| x.atributos.skill).unwrap_or(50.0);
            let car = car_perf.get(&d.team_id).copied().unwrap_or(50.0);
            DriverMerit {
                pilot_id: d.pilot_id.clone(),
                merit: compute_merit(skill, car, None, field_size, 0),
            }
        })
        .collect();

    // ── 1) Esperado×real: só sinais FORTES (muito acima / muito abaixo). ──────────
    // O pole que floppou já é coberto pelo beat de Decepção; o DNF, pelo de Abandono.
    struct ExpCand {
        text: String,
        is_player: bool,
    }
    let mut exp: Vec<ExpCand> = Vec::new();
    for pilot_id in featured {
        let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
            continue;
        };
        if d.is_dnf {
            continue;
        }
        let ev = evaluate(&RaceEvalInput {
            player_id: pilot_id.clone(),
            grid_position: d.grid_position,
            finish_position: d.finish_position,
            is_dnf: false,
            incidents: d.incidents_count,
            field: field.clone(),
        });
        let is_pole = *pilot_id == race_result.pole_sitter_id;
        let name = name_of(pilot_id);
        let text = match ev.assessment {
            Assessment::MuitoAcima => rust_i18n::t!(
                "briefing.perf.much_above",
                name = name.as_str(),
                grid = d.grid_position,
                finish = d.finish_position
            )
            .to_string(),
            Assessment::MuitoAbaixo if !is_pole => rust_i18n::t!(
                "briefing.perf.much_below",
                name = name.as_str(),
                grid = d.grid_position,
                finish = d.finish_position
            )
            .to_string(),
            _ => continue,
        };
        exp.push(ExpCand { text, is_player: d.is_jogador });
    }
    // No máximo 2, com o jogador tendo prioridade.
    exp.sort_by_key(|c| std::cmp::Reverse(c.is_player));
    for c in exp.into_iter().take(2) {
        out.push(c.text);
    }

    // ── 2) Forma recente (últimas 5 na categoria, antes de hoje). ────────────────
    // prioridade: fim de jejum (3) > sequência de pódios (2) > reação (1).
    let mut form: Vec<(i32, String)> = Vec::new();
    for pilot_id in featured {
        let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
            continue;
        };
        if d.is_dnf {
            continue;
        }
        let recent = rh::get_recent_finishes_before(
            conn,
            pilot_id,
            category_id,
            active_season.numero,
            round,
            5,
        )
        .unwrap_or_default();
        if recent.len() < 3 {
            continue; // pouca história → sem leitura de forma confiável
        }
        let name = name_of(pilot_id);
        let recent_wins = recent.iter().filter(|r| r.finish == 1).count();
        let last_two_podiums = recent
            .iter()
            .take(2)
            .filter(|r| !r.is_dnf && r.finish <= 3)
            .count();

        if d.finish_position == 1 && recent_wins == 0 && recent.len() >= 5 {
            form.push((
                3,
                rust_i18n::t!("briefing.perf.end_drought", name = name.as_str()).to_string(),
            ));
        } else if d.finish_position <= 3 && last_two_podiums == 2 {
            form.push((
                2,
                rust_i18n::t!("briefing.perf.podium_streak", name = name.as_str()).to_string(),
            ));
        } else if d.finish_position <= 5 {
            let valid: Vec<i32> = recent.iter().filter(|r| !r.is_dnf).map(|r| r.finish).collect();
            if valid.len() >= 3 {
                let avg = valid.iter().sum::<i32>() as f64 / valid.len() as f64;
                if avg >= field_size as f64 * 0.5 {
                    form.push((
                        1,
                        rust_i18n::t!("briefing.perf.reaction", name = name.as_str()).to_string(),
                    ));
                }
            }
        }
    }
    form.sort_by_key(|(p, _)| std::cmp::Reverse(*p));
    for (_, t) in form.into_iter().take(2) {
        out.push(t);
    }

    // ── 3) Histórico no circuito: destaque que já venceu aqui antes. ─────────────
    if let Ok(Some(track_id)) =
        rh::get_round_track_id(conn, &active_season.id, category_id, round)
    {
        let mut track_facts: Vec<(i32, String)> = Vec::new();
        for pilot_id in featured {
            let Some(d) = rows.iter().find(|x| &x.pilot_id == pilot_id) else {
                continue;
            };
            if d.is_dnf {
                continue;
            }
            let th = rh::get_pilot_track_history(conn, pilot_id, track_id, &active_season.id, round)
                .unwrap_or_default();
            if th.wins < 1 {
                continue;
            }
            let name = name_of(pilot_id);
            let vez = if th.wins == 1 {
                rust_i18n::t!("briefing.perf.time_singular")
            } else {
                rust_i18n::t!("briefing.perf.time_plural")
            };
            let text = if d.finish_position == 1 {
                rust_i18n::t!(
                    "briefing.perf.track_specialist",
                    name = name.as_str(),
                    wins = th.wins,
                    times = vez
                )
                .to_string()
            } else if d.finish_position <= 3 {
                rust_i18n::t!(
                    "briefing.perf.track_good_history",
                    name = name.as_str(),
                    wins = th.wins,
                    times = vez
                )
                .to_string()
            } else {
                continue;
            };
            track_facts.push((th.wins, text));
        }
        track_facts.sort_by_key(|(w, _)| std::cmp::Reverse(*w));
        for (_, t) in track_facts.into_iter().take(2) {
            out.push(t);
        }
    }

    // ── 4) Confronto entre companheiros: par de destaques na MESMA equipe. ───────
    // Emite no máximo 1 (jogador tem prioridade). Exige ambos classificados hoje.
    let mut h2h: Option<(bool, String)> = None;
    'pairs: for i in 0..featured.len() {
        for j in (i + 1)..featured.len() {
            let (Some(a), Some(b)) = (
                rows.iter().find(|x| x.pilot_id == featured[i]),
                rows.iter().find(|x| x.pilot_id == featured[j]),
            ) else {
                continue;
            };
            if a.team_id != b.team_id || a.is_dnf || b.is_dnf {
                continue;
            }
            // Placar do confronto interno na temporada (rodadas em que ambos completaram).
            let ra = rh::get_pilot_season_results(conn, &a.pilot_id, &active_season.id, category_id)
                .unwrap_or_default();
            let rb = rh::get_pilot_season_results(conn, &b.pilot_id, &active_season.id, category_id)
                .unwrap_or_default();
            let rb_map: HashMap<i32, (i32, bool)> =
                rb.iter().map(|(r, f, dnf)| (*r, (*f, *dnf))).collect();
            let (mut wa, mut wb) = (0, 0);
            for (rnd, fa, da) in &ra {
                if let Some((fb, db)) = rb_map.get(rnd) {
                    if *da || *db {
                        continue;
                    }
                    if fa < fb {
                        wa += 1;
                    } else if fb < fa {
                        wb += 1;
                    }
                }
            }
            let (ahead, behind) = if a.finish_position < b.finish_position {
                (a, b)
            } else {
                (b, a)
            };
            let (an, bn) = (name_of(&ahead.pilot_id), name_of(&behind.pilot_id));
            let mut s = rust_i18n::t!(
                "briefing.perf.teammate_h2h",
                ahead = an.as_str(),
                behind = bn.as_str(),
                ap = ahead.finish_position,
                bp = behind.finish_position
            )
            .to_string();
            if wa + wb >= 2 {
                if wa == wb {
                    s.push_str(&rust_i18n::t!("briefing.perf.teammate_tied", a = wa, b = wb));
                } else {
                    let (ln, hi, lo) = if wa > wb {
                        (name_of(&a.pilot_id), wa, wb)
                    } else {
                        (name_of(&b.pilot_id), wb, wa)
                    };
                    s.push_str(&rust_i18n::t!(
                        "briefing.perf.teammate_leader",
                        leader = ln.as_str(),
                        hi = hi,
                        lo = lo
                    ));
                }
            } else {
                s.push('.');
            }
            let involves_player = ahead.is_jogador || behind.is_jogador;
            if h2h.as_ref().map_or(true, |(p, _)| involves_player && !p) {
                h2h = Some((involves_player, s));
                if involves_player {
                    break 'pairs;
                }
            }
        }
    }
    if let Some((_, s)) = h2h {
        out.push(s);
    }

    out
}

/// Converte a TELEMETRIA REAL do SDK (ritmo, duelo, erro mais caro, melhor momento)
/// em fatos de pano de fundo sobre a corrida do JOGADOR — a cor que só existe quando
/// ele correu de verdade no iRacing. O jogador é CITADO (subtrama), nunca protagonista;
/// estes fatos entram na seção "Contexto" e a IA tece quando fizer sentido. Tolerante:
/// cada item só sai se o sinal for confiável (o motor de telemetria já gateia isso).
fn telemetry_context_facts(
    telemetry: &crate::iracing_sdk::telemetry_analysis::TelemetryAnalysis,
    player_name: &str,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if !telemetry.has_telemetry || player_name.trim().is_empty() {
        return out;
    }
    let who = player_name;

    if let Some(p) = &telemetry.pace {
        // Ritmo vs campo (só se a amostra do grid for confiável).
        if p.vs_grid_reliable {
            let delta_s = (p.vs_grid_ms.abs() / 1000.0 * 10.0).round() / 10.0;
            if p.vs_grid_ms <= -200.0 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.faster_than_grid",
                        who = who,
                        delta = format!("{delta_s:.1}")
                    )
                    .to_string(),
                );
            } else if p.vs_grid_ms >= 200.0 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.slower_than_grid",
                        who = who,
                        delta = format!("{delta_s:.1}")
                    )
                    .to_string(),
                );
            }
        }
        // Consistência (só com voltas suficientes).
        if p.consistency_reliable && p.total_laps >= 4 {
            let ratio = p.good_laps as f64 / p.total_laps as f64;
            if ratio >= 0.85 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.consistent",
                        who = who,
                        good = p.good_laps,
                        total = p.total_laps
                    )
                    .to_string(),
                );
            } else if ratio <= 0.5 {
                out.push(
                    rust_i18n::t!(
                        "briefing.tel.inconsistent",
                        who = who,
                        good = p.good_laps,
                        total = p.total_laps
                    )
                    .to_string(),
                );
            }
        }
    }

    // Duelo com o rival mais constante.
    if let Some(r) = &telemetry.rival {
        if !r.pilot_name.trim().is_empty() {
            let gap = (r.avg_gap_s * 10.0).round() / 10.0;
            out.push(
                rust_i18n::t!(
                    "briefing.tel.duel",
                    who = who,
                    laps = r.laps_battled,
                    rival = r.pilot_name.as_str(),
                    gap = format!("{gap:.1}")
                )
                .to_string(),
            );
        }
    }

    // Melhor momento da corrida do jogador.
    if let Some(b) = &telemetry.best_moment {
        let phrase = match b.kind.as_str() {
            "rival_beaten" if !b.rival_name.trim().is_empty() => Some(
                rust_i18n::t!(
                    "briefing.tel.best_rival_beaten",
                    who = who,
                    rival = b.rival_name.as_str()
                )
                .to_string(),
            ),
            "position_gain" if b.positions_gained >= 1 => Some(
                rust_i18n::t!(
                    "briefing.tel.best_position_gain",
                    who = who,
                    n = b.positions_gained
                )
                .to_string(),
            ),
            "recovery" => {
                Some(rust_i18n::t!("briefing.tel.best_recovery", who = who).to_string())
            }
            "clean_streak" if b.streak >= 3 => Some(
                rust_i18n::t!("briefing.tel.best_clean_streak", who = who, n = b.streak)
                    .to_string(),
            ),
            _ => None,
        };
        if let Some(phrase) = phrase {
            out.push(phrase);
        }
    }

    // Erro mais caro (DNF não entra: o beat de Abandono já cobre).
    if let Some(m) = &telemetry.mistake {
        let phrase = match m.kind.as_str() {
            "incident" => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_incident",
                    who = who,
                    lap = m.lap,
                    n = m.positions_lost.max(0)
                )
                .to_string(),
            ),
            "position_loss" if m.positions_lost >= 1 => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_position_loss",
                    who = who,
                    n = m.positions_lost,
                    lap = m.lap
                )
                .to_string(),
            ),
            "pace_drop" if m.time_lost_ms >= 1500.0 => Some(
                rust_i18n::t!(
                    "briefing.tel.mistake_pace_drop",
                    who = who,
                    lap = m.lap,
                    secs = format!("{:.0}", m.time_lost_ms / 1000.0)
                )
                .to_string(),
            ),
            _ => None,
        };
        if let Some(phrase) = phrase {
            out.push(phrase);
        }
    }

    // ── Bandeira amarela REAL (SessionFlags do SDK) ──────────────────────────
    // `yellow_laps` são as voltas do LÍDER em que a corrida esteve sob amarela.
    // Voltas consecutivas viram UM acionamento: é assim que a corrida é vivida e
    // narrada, não como uma lista solta de voltas. Só corrida importada tem isto —
    // no sim offline a amarela não é modelada, então este bloco fica vazio.
    if let Some(charts) = &telemetry.charts {
        let mut yellow = charts.yellow_laps.clone();
        yellow.retain(|l| *l > 0);
        yellow.sort_unstable();
        yellow.dedup();
        if let Some(&first) = yellow.first() {
            let periods = 1 + yellow.windows(2).filter(|w| w[1] - w[0] > 1).count();
            let key = if periods > 1 {
                "briefing.tel.yellow_multi"
            } else {
                "briefing.tel.yellow_single"
            };
            out.push(
                rust_i18n::t!(key, periods = periods, laps = yellow.len(), first = first)
                    .to_string(),
            );
        }
    }

    out
}

fn persist_other_category_news(
    conn: &rusqlite::Connection,
    highlights: &[SimHighlight],
    season_number: i32,
) -> Result<(), String> {
    use crate::db::queries::news as news_queries;
    use crate::generators::ids::{next_ids, IdType};
    use crate::news::{NewsImportance, NewsItem, NewsType};

    if highlights.is_empty() {
        return Ok(());
    }

    let ids = next_ids(conn, IdType::News, highlights.len() as u32)
        .map_err(|e| format!("next_ids news: {e:?}"))?;
    let now = chrono::Local::now().timestamp();
    let items = highlights
        .iter()
        .zip(ids)
        .map(|(highlight, id)| NewsItem {
            id,
            tipo: NewsType::Corrida,
            icone: NewsType::Corrida.icone().to_string(),
            titulo: highlight.headline.clone(),
            texto: rust_i18n::t!(
                "race.news.other_categories_summary",
                headline = highlight.headline.as_str()
            )
            .to_string(),
            rodada: None,
            semana_pretemporada: None,
            temporada: season_number,
            categoria_id: Some(highlight.category.clone()),
            categoria_nome: get_category_config(&highlight.category)
                .map(|category| category.nome.to_string()),
            importancia: NewsImportance::Media,
            timestamp: now,
            driver_id: None,
            driver_id_secondary: None,
            team_id: None,
        })
        .collect::<Vec<_>>();

    news_queries::insert_news_batch(conn, &items)
        .map_err(|e| format!("insert_news_batch outras categorias: {e:?}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "race/tests/mod.rs"]
mod tests;
