use std::path::Path;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Local, Weekday};
use rand::Rng;
use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::calendar::display_date_for_season_week;
use crate::constants::timeline::MARKET_DURATION_WEEKS;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::meta as meta_queries;
use crate::db::queries::rivalries as rivalry_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::finance::cashflow::{apply_offseason_competitiveness_impact, PENALTY_FADE_YEARS};
use crate::finance::planning::{category_finance_scale, derive_budget_index_from_money};
use crate::finance::state::refresh_team_financial_state;
use crate::finance::strategy::{
    advance_strategic_plan, apply_elite_resource_floor, designate_elite_teams,
};
use crate::market::pit_strategy::{
    recalculate_pit_crew_quality, recalculate_pit_strategy_risk, PreviousTeamStanding,
};
use crate::market::proposals::MarketProposal;
use crate::market::sync::sync_team_slots_from_active_regular_contracts;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::license::repair_missing_licenses_for_current_categories;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PreSeasonPhase {
    ContractExpiry,
    Transfers,
    PlayerProposals,
    RookiePlacement,
    Finalization,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreSeasonState {
    pub season_number: i32,
    pub current_week: i32,
    pub total_weeks: i32,
    pub phase: PreSeasonPhase,
    pub is_complete: bool,
    pub player_has_pending_proposals: bool,
    /// Verdadeiro se o jogador já tem um contrato regular ativo para esta temporada.
    #[serde(default)]
    pub player_has_team: bool,
    #[serde(default)]
    pub current_display_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketEventType {
    ContractExpired,
    ContractRenewed,
    TransferCompleted,
    TransferRejected,
    RookieSigned,
    PlayerProposalReceived,
    HierarchyUpdated,
    PreSeasonComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub event_type: MarketEventType,
    pub headline: String,
    pub description: String,
    pub driver_id: Option<String>,
    pub driver_name: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
    pub from_team: Option<String>,
    pub to_team: Option<String>,
    pub categoria: Option<String>,
    #[serde(default)]
    pub from_categoria: Option<String>,
    #[serde(default)]
    pub movement_kind: Option<String>,
    #[serde(default)]
    pub championship_position: Option<i32>,
    #[serde(default)]
    pub seasons_at_previous: Option<i32>,
    /// Vínculo do piloto deste evento com o JOGADOR, p/ o feed dar ênfase:
    /// `"rival"` (rivalidade ativa) | `"raced"` (já correu contra você E entrou na
    /// sua categoria atual) | `"favorite"` (favoritado — reservado, sem sistema ainda).
    /// `None` = sem vínculo (maioria do feed). Prioridade: favorite > rival > raced.
    #[serde(default)]
    pub relation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekResult {
    pub week_number: i32,
    pub phase: PreSeasonPhase,
    pub events: Vec<MarketEvent>,
    pub is_last_week: bool,
    pub player_proposals: Vec<MarketProposal>,
    pub remaining_vacancies: i32,
    pub next_phase: PreSeasonPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreSeasonPlan {
    pub state: PreSeasonState,
    pub planned_events: Vec<PlannedEvent>,
    pub executed_weeks: Vec<WeekResult>,
    /// Dispensas (contratos que terminaram sem renovação) capturadas no início —
    /// emitidas no feed na 1ª semana avançada ("quem perdeu a vaga").
    #[serde(default)]
    pub pending_departures: Vec<MarketEvent>,
    /// Categoria de cada piloto no INÍCIO da pré-temporada (antes das pré-passes, que
    /// limpam o `categoria_atual` dos dispensados) — origem p/ promovido/rebaixado.
    #[serde(default)]
    pub category_snapshot: std::collections::HashMap<String, String>,
    /// Equipe anterior de cada piloto no INÍCIO da pré-temporada (antes das pré-passes):
    /// driver_id → (nome da equipe, temporadas consecutivas nela). Origem p/ o popup de
    /// detalhe da transferência (de qual equipe veio + quanto tempo ficou lá).
    #[serde(default)]
    pub previous_team: std::collections::HashMap<String, (String, i32)>,
    /// Proposta de QUEBRA DE CONTRATO ao jogador (Fase 2b.3): computada uma vez no
    /// setup (rara), mostrada como o leilão ao vivo e limpa quando ele decide. None
    /// na esmagadora maioria das janelas.
    #[serde(default)]
    pub player_poach_offer: Option<crate::market::pipeline::PlayerPoachOffer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedEvent {
    pub week: i32,
    pub event: PendingAction,
    pub executed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// Enum serializado para o plano de pré-temporada; as variantes carregam contexto
// editorial completo e boxear só para uniformizar o tamanho não compensa.
#[allow(clippy::large_enum_variant)]
pub enum PendingAction {
    PhaseMarker {
        phase: PreSeasonPhase,
    },
    ExpireContract {
        contract_id: String,
        driver_id: String,
        driver_name: String,
        team_id: String,
        team_name: String,
    },
    RenewContract {
        driver_id: String,
        driver_name: String,
        team_id: String,
        team_name: String,
        new_salary: f64,
        new_duration: i32,
        new_role: String,
    },
    Transfer {
        driver_id: String,
        driver_name: String,
        from_team_id: Option<String>,
        from_team_name: Option<String>,
        #[serde(default)]
        from_categoria: Option<String>,
        to_team_id: String,
        to_team_name: String,
        salary: f64,
        duration: i32,
        role: String,
    },
    PlayerProposal {
        proposal: MarketProposal,
    },
    PlaceRookie {
        driver: Driver,
        team_id: String,
        team_name: String,
        salary: f64,
        duration: i32,
        role: String,
    },
    UpdateHierarchy {
        team_id: String,
        team_name: String,
        n1_id: Option<String>,
        n1_name: String,
        n2_id: Option<String>,
        n2_name: String,
        // Estado hierárquico anterior (capturado no início da preseason).
        // #[serde(default)] para compatibilidade com saves anteriores que não têm esses campos.
        #[serde(default)]
        prev_n1_id: Option<String>,
        #[serde(default)]
        prev_n2_id: Option<String>,
        #[serde(default)]
        prev_tensao: f64,
        #[serde(default = "default_estavel")]
        prev_status: String,
        #[serde(default)]
        prev_categoria: String,
    },
}

fn default_estavel() -> String {
    "estavel".to_string()
}

#[cfg(test)]
static PRESEASON_CLONE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
struct TempPreseasonClone {
    path: std::path::PathBuf,
    conn: Option<Connection>,
}

#[cfg(test)]
impl TempPreseasonClone {
    fn new(source: &Connection) -> Result<Self, String> {
        let path = clone_connection_to_temp(source)?;
        let conn = Connection::open(&path)
            .map_err(|e| format!("Falha ao abrir clone temporario do banco: {e}"))?;
        Ok(Self {
            path,
            conn: Some(conn),
        })
    }

    fn connection(&self) -> &Connection {
        self.conn
            .as_ref()
            .expect("clone temporario da preseason ja foi liberado")
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
impl Drop for TempPreseasonClone {
    fn drop(&mut self) {
        let _ = self.conn.take();
        if let Err(err) = cleanup_temp_db(&self.path) {
            eprintln!("Falha ao limpar clone temporario da preseason: {err}");
        }
    }
}

pub fn initialize_preseason(
    conn: &Connection,
    season_number: i32,
    rng: &mut impl Rng,
) -> Result<PreSeasonPlan, String> {
    let season_id = get_season_id_by_number(conn, season_number)?
        .ok_or_else(|| format!("Temporada {season_number} nao encontrada"))?;
    reset_market_state(conn, &season_id, &PreSeasonPhase::Transfers)?;
    repair_missing_licenses_for_current_categories(conn)?;
    assign_seasonal_team_attributes(conn, season_number, &season_id)?;

    // Contratos ativos ANTES das pré-passes — p/ detectar as DISPENSAS (terminaram e
    // não foram renovados) e narrá-las no feed da 1ª semana.
    let contracts_before = contract_queries::get_all_active_regular_contracts(conn)
        .map_err(|e| format!("Falha ao carregar contratos antes das pre-passes: {e}"))?;

    // Snapshot das categorias ANTES das pré-passes (que limpam o categoria_atual dos
    // dispensados) — origem p/ inferir promovido/rebaixado nas assinaturas da escada.
    let category_snapshot: std::collections::HashMap<String, String> =
        driver_queries::get_all_drivers(conn)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|d| d.categoria_atual.map(|c| (d.id, c)))
            .collect();

    // Snapshot da EQUIPE anterior (+ tempo de casa) ANTES das pré-passes — origem p/ o
    // popup de detalhe da transferência (de qual equipe veio e há quantas temporadas).
    let previous_team: std::collections::HashMap<String, (String, i32)> = contracts_before
        .iter()
        .map(|c| {
            let tenure = crate::commands::career::calculate_consecutive_team_tenure(
                conn,
                &c.piloto_id,
                &c.equipe_id,
                season_number - 1,
            )
            .unwrap_or(1);
            (c.piloto_id.clone(), (c.equipe_nome.clone(), tenure))
        })
        .collect();

    // ── PRÉ-PASSES REAIS (sem rollback-replay): expira contratos que terminaram,
    // renova (slam-aware) e rebaixa por mérito — aplicadas DE VERDADE no banco. ──
    let prepass_report = crate::market::pipeline::run_market_prepasses(conn, season_number, rng)
        .map_err(|e| format!("Falha ao aplicar pre-passes do mercado: {e}"))?;

    // Feed da 1ª semana: dispensas (×) + promoções/rebaixamentos por mérito (↑/↓), que
    // acontecem nas pré-passes e antes ficavam invisíveis.
    let mut pending_departures =
        build_departure_events(conn, season_number, &contracts_before, &previous_team)?;
    pending_departures.extend(merit_move_events(&prepass_report, &previous_team));

    // O mercado ao vivo é a escada paginada conduzida por advance_week (sem motor de
    // janela persistido): cada semana preenche vagas em todos os tiers e oferta vagas
    // ao jogador. A pré-temporada fecha quando não há mais o que preencher.

    // O jogador já tem time se saiu da pré-passe com contrato regular ativo
    // (renovou / contrato plurianual); senão é agente livre dentro da janela.
    let player_has_team = driver_queries::get_player_driver(conn)
        .ok()
        .and_then(|player| {
            contract_queries::get_active_regular_contract_for_pilot(conn, &player.id).ok()
        })
        .flatten()
        .is_some();

    // Semanas VARIÁVEIS: a conclusão é dirigida pela janela (flag closed), não por um
    // total fixo. `total_weeks` fica só como teto p/ a data exibida não estourar.
    let total_weeks = i32::from(MARKET_DURATION_WEEKS);
    let mut state = PreSeasonState {
        season_number,
        current_week: 1,
        total_weeks,
        phase: PreSeasonPhase::Transfers,
        is_complete: false,
        player_has_pending_proposals: false,
        player_has_team,
        current_display_date: None,
    };
    refresh_preseason_state_display_date(conn, &season_id, &mut state)?;

    // Quebra de contrato do jogador (Fase 2b.3): se um time claramente melhor cobiça o
    // jogador CONTRATADO, guarda o leilão pra ele ver e decidir. Raro; None quase sempre.
    let player_poach_offer =
        crate::market::pipeline::compute_player_poach_offer(conn, season_number)?;

    Ok(PreSeasonPlan {
        state,
        planned_events: Vec::new(),
        executed_weeks: Vec::new(),
        pending_departures,
        category_snapshot,
        previous_team,
        player_poach_offer,
    })
}

/// Tamanho do aporte de última chance, como fração do caixa-médio da categoria.
/// Calibrado para que ~20% das equipes em all-in escapem do colapso (o resto
/// ainda é vendido). Maior = mais escapam.
const LAST_CHANCE_PACKAGE_FACTOR: f64 = 0.25;

/// Aplica o aporte de última chance a uma equipe entrando no ano de all-in:
/// abate a maior parte da dívida e reforça o caixa. Não recalcula o estado
/// financeiro (o chamador o faz).
fn apply_last_chance_package(team: &mut crate::models::team::Team) {
    let scale = category_finance_scale(&team.categoria);
    let package = scale.expected_cash_midpoint() * LAST_CHANCE_PACKAGE_FACTOR;
    // 70% do pacote abate dívida, 30% vira capital de giro.
    team.debt_balance = (team.debt_balance - package * 0.70).max(0.0);
    team.cash_balance += package * 0.30;
}

fn assign_seasonal_team_attributes(
    conn: &Connection,
    season_number: i32,
    season_id: &str,
) -> Result<(), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let previous_standings = load_previous_team_standings(conn, season_number)?;
    // Pilar D: as 3 elites por classe premium (Production auto por reputação;
    // GT3/Endurance com as marcas reais fixas). Recebem plano de dinastia + piso de
    // recursos abaixo, sustentando carro no topo temporada após temporada.
    let elite_ids = designate_elite_teams(&teams);

    // Ano de carreira (0 = primeiro ano), usado para esmaecer a penalidade fictícia
    // GT3 ao longo dos primeiros anos. Saves antigos NÃO têm a meta "career_start_year"
    // (semeada só em carreiras novas); para eles usamos PENALTY_FADE_YEARS, que zera a
    // penalidade (fator 1.0) e deixa esses saves intocados.
    let career_year: i32 = match meta_queries::get_meta_value(conn, "career_start_year")
        .map_err(|e| format!("Falha ao ler career_start_year: {e}"))?
        .and_then(|v| v.parse::<i32>().ok())
    {
        Some(career_start_year) => {
            let season_year: i32 = conn
                .query_row(
                    "SELECT ano FROM seasons WHERE numero = ?1",
                    rusqlite::params![season_number],
                    |row| row.get(0),
                )
                .map_err(|e| format!("Falha ao buscar ano da temporada {season_number}: {e}"))?;
            season_year - career_start_year
        }
        None => PENALTY_FADE_YEARS as i32,
    };

    let mut categories = teams
        .iter()
        .map(|team| team.categoria.clone())
        .collect::<Vec<_>>();
    categories.sort();
    categories.dedup();

    for category in categories {
        let category_teams = teams
            .iter()
            .filter(|team| team.categoria == category)
            .cloned()
            .collect::<Vec<_>>();
        if category_teams.is_empty() {
            continue;
        }

        let calendar = calendar_queries::get_calendar(conn, season_id, &category)
            .map_err(|e| format!("Falha ao carregar calendario de {category}: {e}"))?;
        if calendar.is_empty() {
            continue;
        }

        for team in &category_teams {
            let mut updated_team = team.clone();
            // O shape/nível do carro agora vive no Sistema de Nível do Carro (tabela
            // team_car), decidido pelo cérebro de manutenção por corrida — não mais por
            // um perfil discreto escolhido aqui. O custo do carro vira depreciação real
            // na fatura (chunk 6).
            updated_team.pit_strategy_risk = recalculate_pit_strategy_risk(team, &category_teams);
            updated_team.budget = derive_budget_index_from_money(&updated_team);
            refresh_team_financial_state(&mut updated_team);
            // Pilar D: elite recebe o piso de recursos (patrocínio de dinastia) antes
            // de recalcular o estado financeiro — assim nunca cai em colapso e sustenta
            // o investimento máximo no carro.
            let is_elite = elite_ids.contains(&updated_team.id);
            if is_elite {
                apply_elite_resource_floor(&mut updated_team);
                updated_team.budget = derive_budget_index_from_money(&updated_team);
                refresh_team_financial_state(&mut updated_team);
            }
            // Equipe entrando na 2ª temporada consecutiva de colapso vai de all-in
            // numa tentativa de se salvar antes de ser vendida. Recebe um aporte de
            // "última chance" (investidores injetam capital): abate parte da dívida
            // e reforça o caixa, dando uma chance real — mas só quem também render
            // na pista escapa do colapso; o resto ainda termina vendido.
            let collapse_streak =
                team_queries::get_collapse_streak(conn, &updated_team.id).unwrap_or(0);
            updated_team.season_strategy = if collapse_streak == 1 && !is_elite {
                apply_last_chance_package(&mut updated_team);
                refresh_team_financial_state(&mut updated_team);
                "all_in".to_string()
            } else {
                // Pilar C: a estratégia da temporada vem do plano de 3 temporadas
                // da equipe (arco sustentado), não de uma escolha reativa anual.
                // Pilar D: elites rodam Elite Dominance permanente (dinastia).
                advance_strategic_plan(conn, &updated_team, is_elite)
                    .map_err(|e| {
                        format!(
                            "Falha ao avançar plano estratégico da equipe {}: {e}",
                            updated_team.nome
                        )
                    })?
                    .to_string()
            };
            // Foco vigente (da temporada que passou) decide quanto o time canaliza
            // para o carro neste offseason — a consequência real do foco (ideia 4).
            // Lido ANTES do update_team_focus abaixo (que só recalcula a fase nova).
            let current_focus = crate::finance::focus::get_focus(conn, &updated_team.id)
                .map(|(foco, _)| foco)
                .unwrap_or(crate::finance::focus::TeamFocus::MeioDeGrid);
            apply_offseason_competitiveness_impact(&mut updated_team, career_year, current_focus);
            updated_team.pit_crew_quality = recalculate_pit_crew_quality(
                &updated_team,
                previous_standings.get(&team.id).copied(),
            );
            refresh_team_financial_state(&mut updated_team);
            updated_team.budget = derive_budget_index_from_money(&updated_team);
            team_queries::update_team(conn, &updated_team).map_err(|e| {
                format!(
                    "Falha ao salvar perfil sazonal do carro para equipe {}: {e}",
                    updated_team.nome
                )
            })?;

            // Foco da equipe (ideia 4): deriva a fase atual do estado já calculado,
            // com histerese; promoção/rebaixamento (categoria_anterior) é evento duro
            // que fura a histerese e troca na hora. O retorno (virada) alimentará a
            // notícia da mudança de foco.
            let (plan_type, _) = team_queries::get_strategic_plan(conn, &updated_team.id)
                .unwrap_or_else(|_| ("sustainable".to_string(), 0));
            let hard_event = updated_team.categoria_anterior.is_some();
            crate::finance::focus::update_team_focus(
                conn,
                &updated_team,
                is_elite,
                &plan_type,
                hard_event,
            )
            .map_err(|e| format!("Falha ao atualizar foco da equipe {}: {e}", updated_team.nome))?;
        }
    }

    Ok(())
}

fn load_previous_team_standings(
    conn: &Connection,
    season_number: i32,
) -> Result<std::collections::HashMap<String, PreviousTeamStanding>, String> {
    if season_number <= 1 {
        return Ok(std::collections::HashMap::new());
    }

    let Some(previous_season_id) = get_season_id_by_number(conn, season_number - 1)? else {
        return Ok(std::collections::HashMap::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT equipe_id, categoria, SUM(pontos) AS total_pontos
             FROM standings
             WHERE temporada_id = ?1 AND equipe_id IS NOT NULL AND TRIM(equipe_id) <> ''
             GROUP BY equipe_id, categoria
             ORDER BY categoria ASC, total_pontos DESC, equipe_id ASC",
        )
        .map_err(|e| format!("Falha ao preparar standings anteriores por equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params![previous_season_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar standings anteriores por equipe: {e}"))?;

    let mut grouped = std::collections::HashMap::<String, Vec<(String, f64)>>::new();
    for row in rows {
        let (team_id, category, total_points) =
            row.map_err(|e| format!("Falha ao ler standings anteriores por equipe: {e}"))?;
        grouped
            .entry(category)
            .or_default()
            .push((team_id, total_points));
    }

    let mut result = std::collections::HashMap::new();
    for teams_in_category in grouped.into_values() {
        let total_teams = teams_in_category.len();
        for (index, (team_id, _)) in teams_in_category.into_iter().enumerate() {
            result.insert(
                team_id,
                PreviousTeamStanding {
                    position: index as i32 + 1,
                    total_teams,
                },
            );
        }
    }

    Ok(result)
}

/// Avança UMA semana da pré-temporada. O mercado é a Janela de Transferências: a IA
/// assina e o jogador aceita `player_choice` (id da vaga) ou espera (`None`). A
/// pré-temporada fecha quando a janela fecha (semanas variáveis), não num total fixo.
pub fn advance_week(
    conn: &Connection,
    plan: &mut PreSeasonPlan,
    player_choice: Option<&str>,
) -> Result<WeekResult, String> {
    if plan.state.is_complete {
        return Err("Pre-temporada ja esta completa".to_string());
    }

    repair_missing_licenses_for_current_categories(conn)?;
    let week = plan.state.current_week;
    let season = plan.state.season_number;
    let season_id = get_season_id_by_number(conn, season)?
        .ok_or_else(|| format!("Temporada {season} nao encontrada"))?;
    let mut rng = StdRng::seed_from_u64(season as u64);

    // O jogador aceitou uma oferta nesta semana → assina antes da escada (libera o
    // assento dele da reserva e evita que a escada o preencha por baixo).
    if let Some(seat) = player_choice {
        crate::market::pipeline::sign_player_to_vacancy(conn, season, seat)?;
    }

    // Propostas formais de MÉRITO desta semana ("Proposta recebida"): equipes que
    // escolheriam o jogador o cortejam nominalmente. Os assentos dessas propostas também
    // são segurados (não podem ser preenchidos pela IA enquanto a proposta vive).
    let proposal_seats =
        crate::market::pipeline::generate_player_window_proposals(conn, season, week, &mut rng)?;

    // Reserva alguns assentos pro jogador (se agente livre ativo) — a escada não os
    // preenche, garantindo escolha real E que na última semana haja vaga vazia pra ele,
    // sem dispensar ninguém. Une os assentos das propostas formais.
    let reserved: std::collections::HashSet<String> =
        crate::market::pipeline::player_reserved_seats(conn, season)?
            .into_iter()
            .chain(proposal_seats)
            .collect();

    // Categorias de ORIGEM (snapshot do INÍCIO da pré-temporada, antes das pré-passes
    // limparem o categoria_atual dos dispensados) — pra inferir promovido/rebaixado.
    let category_snapshot = plan.category_snapshot.clone();
    // Equipe anterior (+ tempo de casa) p/ o popup de detalhe da transferência.
    let previous_team = plan.previous_team.clone();

    // Escada (ladder fill) paginada: preenche ~6 vagas em TODOS os tiers (agente
    // livre → rookie → promoção da categoria de baixo), poupando os assentos reservados.
    let mut report = crate::market::proposals::MarketReport::default();
    crate::market::pipeline::fill_vacancies_paced(
        conn,
        season,
        Some(6),
        &reserved,
        &mut report,
        &mut rng,
    )?;

    // Mapeia as assinaturas da escada → eventos de feed.
    let drivers = driver_queries::get_all_drivers(conn).unwrap_or_default();
    let driver_names: std::collections::HashMap<&str, &str> = drivers
        .iter()
        .map(|d| (d.id.as_str(), d.nome.as_str()))
        .collect();
    let teams = team_queries::get_all_teams(conn).unwrap_or_default();
    let team_names: std::collections::HashMap<&str, &str> = teams
        .iter()
        .map(|t| (t.id.as_str(), t.nome.as_str()))
        .collect();
    // Mapeia uma assinatura da escada → evento de feed. Closure reutilizada tanto
    // pelas assinaturas paginadas quanto pelo preenchimento final da última semana.
    let map_signing = |signing: &crate::market::proposals::SigningInfo| -> MarketEvent {
        let dname = driver_names
            .get(signing.driver_id.as_str())
            .copied()
            .unwrap_or(signing.driver_name.as_str());
        let tname = team_names
            .get(signing.team_id.as_str())
            .copied()
            .unwrap_or(signing.team_name.as_str());
        // Origem do piloto (categoria no início) → promovido/rebaixado/lateral.
        let from_cat = category_snapshot.get(signing.driver_id.as_str()).cloned();
        // Estreia (rookie) não tem equipe anterior por definição — não anexa snapshot.
        let is_rookie = matches!(signing.tipo.as_str(), "rookie" | "rookie_emergencia");
        let prev = if is_rookie {
            None
        } else {
            previous_team.get(signing.driver_id.as_str()).cloned()
        };
        let movement_kind = if is_rookie {
            "rookie".to_string()
        } else {
            let from_tier = from_cat
                .as_deref()
                .and_then(crate::constants::categories::get_category_config)
                .map(|c| c.tier);
            let to_tier = crate::constants::categories::get_category_config(&signing.categoria)
                .map(|c| c.tier);
            // Mesma equipe = RENOVAÇÃO (re-assinou o próprio assento), não troca lateral.
            let same_team = prev.as_ref().is_some_and(|(team, _)| team.as_str() == tname);
            match (from_tier, to_tier) {
                (Some(f), Some(t)) if t > f => "promotion",
                (Some(f), Some(t)) if t < f => "relegation",
                _ if same_team => "renewal",
                (Some(_), Some(_)) => "lateral",
                _ => "signing",
            }
            .to_string()
        };
        let (from_team, seasons_at_previous) = match prev {
            Some((team, tenure)) => (Some(team), Some(tenure)),
            None => (None, None),
        };
        MarketEvent {
            event_type: MarketEventType::TransferCompleted,
            headline: format!("{dname} -> {tname}"),
            description: rust_i18n::t!("market.event.deal", category = signing.categoria.as_str())
                .to_string(),
            driver_id: Some(signing.driver_id.clone()),
            driver_name: Some(dname.to_string()),
            team_id: Some(signing.team_id.clone()),
            team_name: Some(tname.to_string()),
            from_team,
            to_team: Some(tname.to_string()),
            categoria: Some(signing.categoria.clone()),
            from_categoria: from_cat,
            movement_kind: Some(movement_kind),
            championship_position: None,
            seasons_at_previous,
            relation: None,
        }
    };
    let mut events: Vec<MarketEvent> = report.new_signings.iter().map(&map_signing).collect();

    // Na 1ª semana avançada, anexa (no topo) as DISPENSAS capturadas no início — o
    // jogador vê quem perdeu a vaga antes das contratações.
    if week == 1 && !plan.pending_departures.is_empty() {
        let mut feed = std::mem::take(&mut plan.pending_departures);
        feed.extend(events);
        events = feed;
    }

    sync_team_slots_from_active_contracts(conn)?;
    let remaining = count_remaining_vacancies(conn)?;

    // O jogador pode ter assinado nesta semana (aceitou uma oferta) — reflete no
    // estado pra a UI e o gate de finalização não ficarem defasados.
    plan.state.player_has_team = driver_queries::get_player_driver(conn)
        .ok()
        .and_then(|player| {
            contract_queries::get_active_regular_contract_for_pilot(conn, &player.id).ok()
        })
        .flatten()
        .is_some();

    // Fecha quando só restam os assentos reservados (nada mais a preencher) ou ao
    // bater o teto de semanas.
    let is_last_week =
        remaining <= reserved.len() as i32 || week >= i32::from(MARKET_DURATION_WEEKS);
    if is_last_week {
        // Garante porta ao jogador (num dos assentos vazios que a escada segurou pra ele,
        // sem dispensar ninguém) e preenche TODAS as vagas restantes — nenhum time corre
        // sem piloto.
        crate::market::pipeline::ensure_player_seated(conn, season)?;
        // O preenchimento final assina vários pilotos de uma vez. Captura essas
        // assinaturas num report e mapeia p/ feed — senão a última semana ficaria
        // muda (os pilotos preenchidos no fechamento sumiam do "fechamento da semana").
        let mut final_report = crate::market::proposals::MarketReport::default();
        crate::market::pipeline::fill_all_remaining_vacancies_reported(
            conn,
            season,
            &mut rng,
            &mut final_report,
        )?;
        events.extend(final_report.new_signings.iter().map(&map_signing));
        plan.state.current_week = week + 1;
        plan.state.phase = PreSeasonPhase::Complete;
        plan.state.is_complete = true;
        events.push(MarketEvent {
            event_type: MarketEventType::PreSeasonComplete,
            headline: rust_i18n::t!("market.event.window_closed_headline").to_string(),
            description: rust_i18n::t!("market.event.window_closed_desc").to_string(),
            driver_id: None,
            driver_name: None,
            team_id: None,
            team_name: None,
            from_team: None,
            to_team: None,
            categoria: None,
            from_categoria: None,
            movement_kind: None,
            championship_position: None,
            seasons_at_previous: None,
            relation: None,
        });
        update_market_state(conn, &season_id, "Fechado", &PreSeasonPhase::Complete, true)?;
    } else {
        plan.state.current_week += 1;
        plan.state.phase = PreSeasonPhase::Transfers;
        update_market_state(
            conn,
            &season_id,
            "Aberto",
            &PreSeasonPhase::Transfers,
            false,
        )?;
    }

    // Marca cada evento com seu vínculo ao jogador (rival / já-correu-contra) — o feed
    // mostra TODOS, mas dá ênfase aos marcados. Não filtra nada.
    tag_player_relations(conn, &mut events);

    let next_phase = plan.state.phase.clone();
    refresh_preseason_state_display_date(conn, &season_id, &mut plan.state)?;
    let result = WeekResult {
        week_number: week,
        phase: PreSeasonPhase::Transfers,
        events,
        is_last_week,
        player_proposals: Vec::new(),
        remaining_vacancies: remaining,
        next_phase,
    };
    plan.executed_weeks.push(result.clone());
    Ok(result)
}

/// Anota cada evento do feed com seu vínculo ao JOGADOR, para o front dar ênfase
/// (mostra TODOS os eventos como sempre; só marca os relevantes). Regras:
/// - `"rival"`: o piloto tem rivalidade ativa com o jogador (qualquer categoria).
/// - `"raced"`: o jogador já dividiu um grid com o piloto E o evento é na categoria
///   ATUAL do jogador (ex.: "alguém com quem já corri entrou na minha Mazda Cup").
///   Restrito à categoria atual de propósito: numa carreira longa o jogador já correu
///   contra quase todo mundo, então sem o filtro o selo apareceria em todos.
/// - `"favorite"`: reservado p/ quando existir sistema de favoritar (prioridade máxima).
/// Prioridade: favorite > rival > raced. Eventos sem `driver_id` (ex.: fechamento da
/// janela) ficam sem vínculo.
fn tag_player_relations(conn: &Connection, events: &mut [MarketEvent]) {
    let Ok(player) = driver_queries::get_player_driver(conn) else {
        return;
    };
    let player_id = player.id;
    let player_cat = player.categoria_atual;

    // Favoritos do jogador (watchlist) — prioridade máxima na ênfase.
    let favorites = crate::db::queries::favorites::get_favorite_ids(conn).unwrap_or_default();

    // Rivais ativos: o "outro" piloto de cada rivalidade do jogador.
    let rivals: std::collections::HashSet<String> =
        rivalry_queries::get_rivalries_for_pilot(conn, &player_id)
            .unwrap_or_default()
            .into_iter()
            .map(|r| {
                if r.piloto1_id == player_id {
                    r.piloto2_id
                } else {
                    r.piloto1_id
                }
            })
            .collect();

    // Pilotos com quem o jogador já dividiu um grid (qualquer corrida da carreira).
    let mut raced: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT DISTINCT r2.piloto_id
           FROM race_results r1
           JOIN race_results r2 ON r1.race_id = r2.race_id
          WHERE r1.piloto_id = ?1 AND r2.piloto_id != ?1",
    ) {
        if let Ok(rows) = stmt.query_map([&player_id], |row| row.get::<_, String>(0)) {
            raced.extend(rows.flatten());
        }
    }

    for ev in events.iter_mut() {
        let Some(did) = ev.driver_id.as_deref() else {
            continue;
        };
        // Prioridade: favorito > rival > já-correu. Para por aqui na 1ª match.
        if favorites.contains(did) {
            ev.relation = Some("favorite".to_string());
        } else if rivals.contains(did) {
            ev.relation = Some("rival".to_string());
        } else if raced.contains(did) && ev.categoria.as_deref() == player_cat.as_deref() {
            ev.relation = Some("raced".to_string());
        }
    }
}

/// Eventos de DISPENSA: contratos que terminaram (temporada_fim < season) cujo piloto
/// NÃO tem contrato ativo após as pré-passes (não renovou nem foi reaproveitado);
/// exclui o jogador. Narrativa "quem perdeu a vaga" pro feed da 1ª semana.
/// Eventos de PROMOÇÃO/REBAIXAMENTO por mérito (das pré-passes) pro feed — antes
/// aconteciam de forma invisível. Lê o report das pré-passes pelos tipos
/// `promocao_merito` (↑) e `rebaixamento` (↓).
fn merit_move_events(
    report: &crate::market::proposals::MarketReport,
    previous_team: &std::collections::HashMap<String, (String, i32)>,
) -> Vec<MarketEvent> {
    report
        .new_signings
        .iter()
        .filter_map(|s| {
            let movement_kind = match s.tipo.as_str() {
                "promocao_merito" => "promotion",
                "rebaixamento" => "relegation",
                _ => return None,
            };
            let prev = previous_team.get(s.driver_id.as_str()).cloned();
            let (from_team, seasons_at_previous) = match prev {
                Some((team, tenure)) => (Some(team), Some(tenure)),
                None => (None, None),
            };
            Some(MarketEvent {
                event_type: MarketEventType::TransferCompleted,
                headline: format!("{} -> {}", s.driver_name, s.team_name),
                description: rust_i18n::t!("market.event.deal", category = s.categoria.as_str())
                    .to_string(),
                driver_id: Some(s.driver_id.clone()),
                driver_name: Some(s.driver_name.clone()),
                team_id: Some(s.team_id.clone()),
                team_name: Some(s.team_name.clone()),
                from_team,
                to_team: Some(s.team_name.clone()),
                categoria: Some(s.categoria.clone()),
                from_categoria: None,
                movement_kind: Some(movement_kind.to_string()),
                championship_position: None,
                seasons_at_previous,
                relation: None,
            })
        })
        .collect()
}

fn build_departure_events(
    conn: &Connection,
    season_number: i32,
    contracts_before: &[Contract],
    previous_team: &std::collections::HashMap<String, (String, i32)>,
) -> Result<Vec<MarketEvent>, String> {
    let active_after: std::collections::HashSet<String> =
        contract_queries::get_all_active_regular_contracts(conn)
            .map_err(|e| format!("Falha ao carregar contratos pos pre-passes: {e}"))?
            .into_iter()
            .map(|c| c.piloto_id)
            .collect();
    let drivers = driver_queries::get_all_drivers(conn).unwrap_or_default();
    let is_player: std::collections::HashSet<&str> = drivers
        .iter()
        .filter(|d| d.is_jogador)
        .map(|d| d.id.as_str())
        .collect();
    let mut events = Vec::new();
    for c in contracts_before {
        if c.temporada_fim >= season_number
            || active_after.contains(&c.piloto_id)
            || is_player.contains(c.piloto_id.as_str())
        {
            continue;
        }
        events.push(MarketEvent {
            event_type: MarketEventType::ContractExpired,
            headline: rust_i18n::t!(
                "market.event.departure_headline",
                driver = c.piloto_nome.as_str(),
                team = c.equipe_nome.as_str()
            )
            .to_string(),
            description: rust_i18n::t!("market.event.contract_ended").to_string(),
            driver_id: Some(c.piloto_id.clone()),
            driver_name: Some(c.piloto_nome.clone()),
            team_id: Some(c.equipe_id.clone()),
            team_name: Some(c.equipe_nome.clone()),
            from_team: Some(c.equipe_nome.clone()),
            to_team: None,
            categoria: Some(c.categoria.clone()),
            from_categoria: Some(c.categoria.clone()),
            movement_kind: Some("departure".to_string()),
            championship_position: None,
            seasons_at_previous: previous_team
                .get(c.piloto_id.as_str())
                .map(|(_, tenure)| *tenure),
            relation: None,
        });
    }
    Ok(events)
}

pub fn refresh_preseason_state_display_date(
    conn: &Connection,
    season_id: &str,
    state: &mut PreSeasonState,
) -> Result<(), String> {
    state.current_display_date =
        compute_preseason_display_date(conn, season_id, state.current_week, state.total_weeks)?;
    Ok(())
}

pub fn save_preseason_plan(save_path: &Path, plan: &PreSeasonPlan) -> Result<(), String> {
    std::fs::create_dir_all(save_path)
        .map_err(|e| format!("Falha ao criar diretorio da pre-temporada: {e}"))?;
    let json = serde_json::to_string_pretty(plan)
        .map_err(|e| format!("Falha ao serializar plano da pre-temporada: {e}"))?;
    std::fs::write(preseason_plan_path(save_path), json)
        .map_err(|e| format!("Falha ao salvar plano da pre-temporada: {e}"))
}

pub fn load_preseason_plan(save_path: &Path) -> Result<Option<PreSeasonPlan>, String> {
    let path = preseason_plan_path(save_path);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Falha ao ler plano da pre-temporada: {e}"))?;
    let plan = serde_json::from_str(&content)
        .map_err(|e| format!("Falha ao parsear plano da pre-temporada: {e}"))?;
    Ok(Some(plan))
}

pub fn delete_preseason_plan(save_path: &Path) -> Result<(), String> {
    let path = preseason_plan_path(save_path);
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|e| format!("Falha ao apagar plano da pre-temporada: {e}"))
}

fn preseason_plan_path(save_path: &Path) -> std::path::PathBuf {
    save_path.join("preseason_plan.json")
}

fn compute_preseason_display_date(
    conn: &Connection,
    season_id: &str,
    current_week: i32,
    _total_weeks: i32,
) -> Result<Option<String>, String> {
    let season = season_queries::get_season_by_id(conn, season_id)
        .map_err(|e| format!("Falha ao carregar temporada da pre-temporada: {e}"))?
        .ok_or_else(|| format!("Temporada {season_id} nao encontrada"))?;
    let season_week = current_week.clamp(1, i32::from(MARKET_DURATION_WEEKS)) as u8;
    display_date_for_season_week(season_week, season.ano, Weekday::Sat).map(Some)
}

fn get_season_id_by_number(
    conn: &Connection,
    season_number: i32,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT id FROM seasons WHERE numero = ?1 LIMIT 1",
        rusqlite::params![season_number],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("Falha ao buscar temporada {season_number}: {e}"))
}

fn reset_market_state(
    conn: &Connection,
    season_id: &str,
    phase: &PreSeasonPhase,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM market_proposals WHERE temporada_id = ?1",
        rusqlite::params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar propostas antigas da pre-temporada: {e}"))?;
    conn.execute(
        "DELETE FROM market WHERE temporada_id = ?1",
        rusqlite::params![season_id],
    )
    .map_err(|e| format!("Falha ao limpar estado antigo do mercado: {e}"))?;
    conn.execute(
        "INSERT INTO market (temporada_id, status, fase, inicio, fim)
         VALUES (?1, 'Aberto', ?2, ?3, '')",
        rusqlite::params![season_id, phase_label(phase), timestamp_now()],
    )
    .map_err(|e| format!("Falha ao inicializar estado do mercado: {e}"))?;
    Ok(())
}

fn update_market_state(
    conn: &Connection,
    season_id: &str,
    status: &str,
    phase: &PreSeasonPhase,
    completed: bool,
) -> Result<(), String> {
    let end_value = if completed {
        timestamp_now()
    } else {
        String::new()
    };
    conn.execute(
        "UPDATE market
         SET status = ?1, fase = ?2, fim = CASE WHEN ?3 = '' THEN fim ELSE ?3 END
         WHERE temporada_id = ?4",
        rusqlite::params![status, phase_label(phase), end_value, season_id],
    )
    .map_err(|e| format!("Falha ao atualizar estado do mercado: {e}"))?;
    Ok(())
}

fn phase_label(phase: &PreSeasonPhase) -> &'static str {
    match phase {
        PreSeasonPhase::ContractExpiry => "ContractExpiry",
        PreSeasonPhase::Transfers => "Transfers",
        PreSeasonPhase::PlayerProposals => "PlayerProposals",
        PreSeasonPhase::RookiePlacement => "RookiePlacement",
        PreSeasonPhase::Finalization => "Finalization",
        PreSeasonPhase::Complete => "Complete",
    }
}

fn timestamp_now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[cfg(test)]
fn clone_connection_to_temp(conn: &Connection) -> Result<std::path::PathBuf, String> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| format!("Falha ao checkpointar banco antes do clone: {e}"))?;
    let temp_path = next_preseason_clone_path()?;
    let escaped = temp_path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{escaped}';"))
        .map_err(|e| format!("Falha ao clonar banco para planejamento da pre-temporada: {e}"))?;
    Ok(temp_path)
}

#[cfg(test)]
fn next_preseason_clone_path() -> Result<std::path::PathBuf, String> {
    let pid = std::process::id();
    for _ in 0..64 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("Falha ao gerar timestamp do clone: {e}"))?
            .as_nanos();
        let counter = PRESEASON_CLONE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = std::env::temp_dir().join(format!(
            "iracerapp_preseason_clone_{pid}_{nanos}_{counter}.db"
        ));

        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Falha ao reservar caminho unico para clone temporario da pre-temporada".to_string())
}

#[cfg(test)]
fn cleanup_temp_db(path: &Path) -> Result<(), String> {
    fn remove_if_exists(path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!(
                "Falha ao remover arquivo temporario '{}': {err}",
                path.display()
            )),
        }
    }

    remove_if_exists(path)?;
    let wal = std::path::PathBuf::from(format!("{}-wal", path.to_string_lossy()));
    let shm = std::path::PathBuf::from(format!("{}-shm", path.to_string_lossy()));
    remove_if_exists(&wal)?;
    remove_if_exists(&shm)?;
    Ok(())
}

fn sync_team_slots_from_active_contracts(conn: &Connection) -> Result<(), String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao carregar equipes: {e}"))?;
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos: {e}"))?;
    let drivers_by_id = drivers
        .into_iter()
        .map(|driver| (driver.id.clone(), driver))
        .collect::<std::collections::HashMap<_, _>>();
    sync_team_slots_from_active_regular_contracts(conn, &teams, &drivers_by_id)
}

fn count_remaining_vacancies(conn: &Connection) -> Result<i32, String> {
    let teams =
        team_queries::get_all_teams(conn).map_err(|e| format!("Falha ao contar vagas: {e}"))?;
    Ok(teams
        .iter()
        .map(|team| {
            let mut open = 0;
            if team.piloto_1_id.is_none() {
                open += 1;
            }
            if team.piloto_2_id.is_none() {
                open += 1;
            }
            open
        })
        .sum())
}

#[cfg(test)]
mod tests;
