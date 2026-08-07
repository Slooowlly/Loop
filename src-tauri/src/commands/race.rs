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
    calculate_expected_event_interest, calculate_realized_event_interest, to_repercussion_summary,
    EventInterestContext, EventRepercussionSummary, InterestTier, RealizedEventInterest,
};
use crate::finance::cashflow::{apply_round_cashflow, TeamRoundFinanceContext};
use crate::finance::economy::{
    economy_cost_modifier, economy_income_modifier, global_economic_health_for_season,
    GlobalEconomicHealth,
};
use crate::finance::events::{apply_crisis_event_if_needed, debt_service_for_state};
use crate::finance::planning::{calculate_financial_plan, category_finance_scale_for};
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
#[path = "race/despesa.rs"]
mod despesa;
#[path = "race/fatos.rs"]
mod fatos;
#[path = "race/fatura.rs"]
pub mod fatura;
#[path = "race/financas.rs"]
mod financas;
#[path = "race/grade.rs"]
mod grade;
#[path = "race/importacao.rs"]
mod importacao;
#[path = "race/manutencao.rs"]
mod manutencao;
#[path = "race/merito.rs"]
mod merito;
#[path = "race/modificadores.rs"]
mod modificadores;
#[path = "race/noticias.rs"]
mod noticias;
#[path = "race/persistencia.rs"]
mod persistencia;
#[path = "race/simulacao.rs"]
mod simulacao;

pub(crate) use comum::*;
pub(crate) use despesa::*;
pub(crate) use financas::*;
pub(crate) use importacao::*;
pub(crate) use manutencao::*;
pub(crate) use modificadores::*;
pub(crate) use persistencia::*;
pub(crate) use simulacao::*;
// Só o próprio race.rs (e os irmãos, via `use super::*`) consomem estes quatro.
use fatos::*;
use grade::*;
use merito::*;
use noticias::*;

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
    /// Repercussão pública do evento: o que se esperava × o que a corrida entregou.
    /// `None` quando o jogador não corre esta categoria ou a categoria não tem config.
    #[serde(default)]
    pub event_repercussion: Option<EventRepercussionSummary>,
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
        let cal =
            calendar_queries::get_calendar(&db.conn, &race_entry.season_id, &race_entry.categoria)
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
    let fame_outcome =
        apply_post_race_fame(&db.conn, &race_entry, &player_race, &player_new_injuries)?;
    let post_race_bias = fame_outcome.news_importance_bias;
    let interest_tier = fame_outcome.interest_tier;
    let event_repercussion = fame_outcome.player_repercussion;

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
    let mut player_team = player_entry.and_then(|pe| {
        team_queries::get_team_by_id(&db.conn, &pe.team_id)
            .ok()
            .flatten()
    });
    if player_entry.map(|r| r.is_dnf).unwrap_or(false) {
        repair_severity = "moderado";
        if let Some(team) = player_team.as_mut() {
            let mut rng = rand::thread_rng();
            let cost = compute_repair_cost(
                repair_severity,
                &team.categoria,
                team.classe.as_deref(),
                team.car_performance,
                &mut rng,
            );
            if cost > 0.0 {
                team.cash_balance -= cost;
                team.last_round_expenses += cost;
                let _ = team_queries::update_team(&db.conn, team);
                repair_cost = cost;
            }
        }
    }

    // Fatura do fim de semana: a decomposição do custo de operação JÁ debitado desta
    // rodada (carro, logística, equipe) + o conserto, se houve DNF.
    let maintenance = player_team
        .as_ref()
        .map(|team| {
            compute_maintenance_breakdown(
                &db.conn,
                team,
                &player_race,
                race_entry.track_id,
                race_entry.duracao_corrida_min,
                get_category_config(&race_entry.categoria)
                    .map(|c| c.corridas_por_temporada as f64)
                    .unwrap_or(12.0),
                global_economic_health_for_season(active_season.numero as i32),
                repair_cost,
                repair_severity,
                active_season.numero as i32,
                race_entry.rodada,
            )
        })
        .unwrap_or_default();

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
            "event_repercussion": &event_repercussion,
        }),
    );

    Ok(RaceWeekendResult {
        player_race,
        other_categories,
        evaluation,
        maintenance,
        event_repercussion,
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

/// A FATURA da etapa que o jogador lê: as sete linhas físicas do fim de semana, os
/// quatro canais de receita, o conserto se houve e o rodapé do custo fixo do ano.
///
/// `None` quando a rodada não moveu o caixa da equipe do jogador (corrida de bloco
/// especial, carreira sem jogador, corrida nunca disputada) — ver
/// [`fatura::fatura_da_rodada_in_base_dir`].
#[tauri::command]
pub fn get_stage_invoice(
    app: AppHandle,
    career_id: String,
    race_id: String,
) -> Result<Option<crate::commands::career_types::StageInvoiceDto>, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    fatura::fatura_da_rodada_in_base_dir(&base_dir, &career_id, &race_id)
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
    let path = career_dir
        .join("race_screens")
        .join(format!("{}.json", entry.id));
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let mut v = match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
            // Injeta o race_id (não estava no payload salvo) p/ o front reconstruir o clima.
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "race_id".into(),
                    serde_json::Value::String(entry.id.clone()),
                );
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

#[cfg(test)]
#[path = "race/tests/mod.rs"]
mod tests;
