//! Comandos da Janela de Transferências interativa (Fase 2).
//!
//! O jogador avança a janela semana a semana: vê suas ofertas, aceita uma ou
//! espera, e o estado (serializado) evolui e é persistido entre os comandos. Ao
//! fechar, todas as assinaturas da janela são aplicadas no banco.

use std::path::Path;

use crate::config::app_config::AppConfig;
use crate::constants::categories;
use crate::db::connection::Database;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::market::pipeline;
use crate::market::transfer_window::{PlayerOffer, Signing};
use crate::simulation::math::normalize_car_performance;

/// Oferta da janela ENRIQUECIDA para a UI (nome/cor do time, categoria, carro,
/// companheiro de equipe) — montada a partir do `PlayerOffer` puro do motor.
#[derive(serde::Serialize)]
pub struct PlayerOfferView {
    pub seat_id: String,
    pub team_id: String,
    pub team_name: String,
    pub team_color: String,
    pub team_color_secondary: String,
    pub category: String,
    pub category_label: String,
    pub category_tier: u8,
    pub class: Option<String>,
    pub role: String,
    pub salary: f64,
    pub car_performance_rating: u8,
    pub teammate_name: Option<String>,
    pub teammate_skill: Option<u8>,
    // ── Estatísticas do companheiro (tooltip do card) ──
    /// Temporadas consecutivas do companheiro na equipe (None se desconhecido).
    pub teammate_tenure: Option<i32>,
    pub teammate_age: Option<u32>,
    pub teammate_races: Option<u32>,
    pub teammate_wins: Option<u32>,
    pub teammate_podiums: Option<u32>,
    pub teammate_poles: Option<u32>,
    pub teammate_titles: Option<u32>,
    pub teammate_career_points: Option<f64>,
    pub teammate_salary: Option<f64>,
    /// Estrelato do companheiro: fama (`midia`) e carisma, 0–100. Deixa o jogador
    /// pesar se a vaga é ao lado de um astro (mais holofote/patrocínio) na decisão.
    pub teammate_fama: Option<u8>,
    pub teammate_carisma: Option<u8>,
    /// Até 2 atributos mais fortes e mais fracos do companheiro (rótulos em PT).
    pub teammate_strengths: Vec<String>,
    pub teammate_weaknesses: Vec<String>,
    // ── Ficha da equipe (dados ricos para o modal robusto de ofertas) ──
    pub team_reputation: u8,
    pub team_reliability: u8,
    /// Caixa real da equipe (dinheiro em conta), não um índice.
    pub team_cash: f64,
    pub team_founded_year: i32,
    pub team_country: String,
    pub team_titles_drivers: i32,
    pub team_titles_constructors: i32,
    pub team_historic_wins: i32,
    pub team_historic_podiums: i32,
    /// Posição final da equipe na última temporada arquivada (None = estreante/sem histórico).
    pub team_last_position: Option<i32>,
    // ── Foco + Vínculo (ideia 4: segurar-vs-vender do lado do jogador) ──
    /// Fase atual do time que faz a oferta (ex.: "Celeiro de talentos", "Dinastia").
    pub team_focus: String,
    /// Nível do vínculo do jogador com este time (1–6; 1 = sem história ainda).
    pub bond_level: u8,
    /// Selo qualitativo do vínculo ("Recém-chegado" … "Casa").
    pub bond_label: String,
    /// Anos do contrato ofertado — plurianual num time-casa/leal (contrato de projeto).
    pub offer_duration: i32,
    /// Interesse ATIVO (Fase 2a do estrelato): o time cobiça o nome do jogador pela
    /// fama → oferta com salário-prêmio, sempre destacada na Janela.
    #[serde(default)]
    pub active_interest: bool,
}

/// Estado da janela apresentado à UI.
#[derive(serde::Serialize)]
pub struct TransferWindowPayload {
    pub week: u32,
    pub closed: bool,
    /// Ofertas que o jogador recebeu nesta semana (pra decidir).
    pub player_offers: Vec<PlayerOfferView>,
    /// Assinaturas da janela até agora (feed do mercado).
    pub signings: Vec<Signing>,
    /// Categoria efetiva do jogador (atual, senão a última por contrato) — a UI ordena as
    /// ofertas pela MARCA do jogador primeiro. None se nunca teve categoria.
    pub player_category: Option<String>,
    /// Tier da categoria do jogador (convenção do backend) — pra UI destacar promoção.
    pub player_tier: Option<u8>,
    /// Nome do piloto do jogador — usado na assinatura da tela de contrato.
    pub player_name: Option<String>,
}

/// Posição final da equipe na temporada arquivada mais recente (None se estreante
/// ou se a tabela de histórico ainda não existir no save).
fn last_championship_position(conn: &rusqlite::Connection, team_id: &str) -> Option<i32> {
    conn.query_row(
        "SELECT posicao_campeonato FROM team_season_archive
         WHERE team_id = ?1 AND posicao_campeonato IS NOT NULL
         ORDER BY season_number DESC
         LIMIT 1",
        [team_id],
        |row| row.get::<_, i32>(0),
    )
    .ok()
}

/// Pontos fortes e fracos do piloto: os 2 maiores e os 2 menores entre um conjunto
/// CURADO de atributos de pilotagem (exclui skill geral, mídia, estilo e meta).
fn driver_strengths_weaknesses(
    attrs: &crate::models::driver::DriverAttributes,
) -> (Vec<String>, Vec<String>) {
    let mut scored: Vec<(&str, f64)> = vec![
        ("Consistência", attrs.consistencia),
        ("Corpo a corpo", attrs.racecraft),
        ("Defesa", attrs.defesa),
        ("Classificação", attrs.ritmo_classificacao),
        ("Gestão de pneus", attrs.gestao_pneus),
        ("Largada", attrs.habilidade_largada),
        ("Adaptabilidade", attrs.adaptabilidade),
        ("Pilotagem na chuva", attrs.fator_chuva),
    ];
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let strengths = scored.iter().take(2).map(|(l, _)| l.to_string()).collect();
    let weaknesses = scored
        .iter()
        .rev()
        .take(2)
        .map(|(l, _)| l.to_string())
        .collect();
    (strengths, weaknesses)
}

/// Enriquece uma oferta do motor com os dados de exibição (time, categoria, carro).
fn build_offer_view(
    conn: &rusqlite::Connection,
    offer: &PlayerOffer,
    season: i32,
) -> Result<PlayerOfferView, String> {
    let team = team_queries::get_team_by_id(conn, &offer.team_id)
        .map_err(|e| format!("Falha ao carregar equipe da oferta: {e}"))?
        .ok_or_else(|| "Equipe da oferta nao encontrada.".to_string())?;
    // Ideia 4: vínculo do jogador com este time + foco atual do time (segurar-vs-vender).
    let player_bond = driver_queries::get_player_driver(conn)
        .ok()
        .map(|p| crate::market::bond::get_bond(conn, &p.id, &team.id).unwrap_or(0.0))
        .unwrap_or(0.0);
    let offer_focus = crate::finance::focus::get_focus(conn, &team.id)
        .map(|(f, _)| f)
        .unwrap_or(crate::finance::focus::TeamFocus::MeioDeGrid);
    let category = categories::get_category_config(&offer.category);
    // Companheiro = piloto da OUTRA vaga do time (se já preenchida).
    let companion_id = if offer.is_n1 {
        team.piloto_2_id.clone()
    } else {
        team.piloto_1_id.clone()
    };
    let companion = companion_id
        .as_deref()
        .map(|id| driver_queries::get_driver(conn, id))
        .transpose()
        .map_err(|e| format!("Falha ao carregar companheiro de equipe: {e}"))?;
    let companion_ref = companion.as_ref();
    let teammate_tenure = companion_ref.and_then(|d| {
        crate::commands::career::calculate_consecutive_team_tenure(conn, &d.id, &team.id, season)
    });
    let teammate_salary = companion_ref.and_then(|d| {
        contract_queries::get_active_contract_for_pilot(conn, &d.id)
            .ok()
            .flatten()
            .map(|c| c.salario_anual)
    });
    let (teammate_strengths, teammate_weaknesses) = companion_ref
        .map(|d| driver_strengths_weaknesses(&d.atributos))
        .unwrap_or_default();
    Ok(PlayerOfferView {
        seat_id: offer.seat_id.clone(),
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        team_color: team.cor_primaria.clone(),
        team_color_secondary: team.cor_secundaria.clone(),
        category: offer.category.clone(),
        category_label: category
            .map(|c| c.nome_curto.to_string())
            .unwrap_or_else(|| offer.category.clone()),
        category_tier: category.map(|c| c.tier).unwrap_or(0),
        class: offer.class.clone(),
        role: if offer.is_n1 { "N1" } else { "N2" }.to_string(),
        salary: offer.salary,
        car_performance_rating: normalize_car_performance(team.car_performance)
            .round()
            .clamp(0.0, 100.0) as u8,
        teammate_name: companion_ref.map(|d| d.nome.clone()),
        teammate_skill: companion_ref.map(|d| d.atributos.skill.round().clamp(0.0, 100.0) as u8),
        teammate_tenure,
        teammate_age: companion_ref.map(|d| d.idade),
        teammate_races: companion_ref.map(|d| d.stats_carreira.corridas),
        teammate_wins: companion_ref.map(|d| d.stats_carreira.vitorias),
        teammate_podiums: companion_ref.map(|d| d.stats_carreira.podios),
        teammate_poles: companion_ref.map(|d| d.stats_carreira.poles),
        teammate_titles: companion_ref.map(|d| d.stats_carreira.titulos),
        teammate_career_points: companion_ref.map(|d| d.stats_carreira.pontos_total),
        teammate_salary,
        teammate_fama: companion_ref.map(|d| d.atributos.midia.round().clamp(0.0, 100.0) as u8),
        teammate_carisma: companion_ref
            .map(|d| d.atributos.carisma.round().clamp(0.0, 100.0) as u8),
        teammate_strengths,
        teammate_weaknesses,
        team_reputation: team.reputacao.round().clamp(0.0, 100.0) as u8,
        team_reliability: team.confiabilidade.round().clamp(0.0, 100.0) as u8,
        team_cash: team.cash_balance,
        team_founded_year: team.ano_fundacao,
        team_country: team.pais_sede.clone(),
        team_titles_drivers: team.historico_titulos_pilotos,
        team_titles_constructors: team.historico_titulos_construtores,
        team_historic_wins: team.historico_vitorias,
        team_historic_podiums: team.historico_podios,
        team_last_position: last_championship_position(conn, &team.id),
        team_focus: offer_focus.label().to_string(),
        bond_level: crate::market::bond::bond_level(player_bond),
        bond_label: crate::market::bond::bond_level_label(player_bond).to_string(),
        offer_duration: crate::market::renewal::player_offer_duration(offer_focus, player_bond),
        active_interest: offer.active_interest,
    })
}

/// Monta o payload do mercado a partir da escada ao vivo: as ofertas do jogador são
/// derivadas das vagas regulares elegíveis (`pipeline::player_market_offers`). O feed
/// real do mercado vai pelas notícias da semana (não por este payload); `signings`
/// fica vazio. O fecho da janela é dirigido por `preseasonState.is_complete` na UI.
fn build_payload(conn: &rusqlite::Connection, season: i32) -> Result<TransferWindowPayload, String> {
    let offers = pipeline::player_market_offers(conn, season)?;
    let player_offers = offers
        .iter()
        .map(|offer| build_offer_view(conn, offer, season))
        .collect::<Result<Vec<_>, String>>()?;
    // Categoria efetiva do jogador: a atual, senão a última por contrato (mesmo critério
    // do tier de mercado). A UI usa a MARCA disso pra ordenar as ofertas.
    let player_category = driver_queries::get_player_driver(conn).ok().and_then(|p| {
        p.categoria_atual.clone().or_else(|| {
            conn.query_row(
                "SELECT categoria FROM contracts WHERE piloto_id = ?1 ORDER BY temporada_fim DESC LIMIT 1",
                [&p.id],
                |r| r.get::<_, String>(0),
            )
            .ok()
        })
    });
    let player_tier = player_category
        .as_deref()
        .and_then(categories::get_category_config)
        .map(|c| c.tier);
    let player_name = driver_queries::get_player_driver(conn).ok().map(|p| p.nome);
    Ok(TransferWindowPayload {
        week: 0,
        closed: player_offers.is_empty(),
        player_offers,
        signings: Vec::new(),
        player_category,
        player_tier,
        player_name,
    })
}

fn open_career(base_dir: &Path, career_id: &str) -> Result<(Database, i32), String> {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    if !db_path.exists() {
        return Err("Banco da carreira nao encontrado.".to_string());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco da carreira: {e}"))?;
    let season = season_queries::get_active_season(&db.conn)
        .map_err(|e| format!("Falha ao carregar temporada ativa: {e}"))?
        .ok_or_else(|| "Nenhuma temporada ativa.".to_string())?;
    Ok((db, season.numero))
}

/// Ofertas atuais do jogador no mercado (vagas regulares elegíveis na sua faixa).
pub(crate) fn get_transfer_window_state_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<TransferWindowPayload, String> {
    let (db, season) = open_career(base_dir, career_id)?;
    build_payload(&db.conn, season)
}

/// O avanço real do mercado é feito por `advance_market_week` (que chama
/// `preseason::advance_week`). Este comando ficou legado: apenas devolve o estado
/// atual das ofertas, sem avançar nada — `accepted_seat_id` é ignorado.
pub(crate) fn advance_transfer_window_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    _accepted_seat_id: Option<&str>,
) -> Result<TransferWindowPayload, String> {
    let (db, season) = open_career(base_dir, career_id)?;
    build_payload(&db.conn, season)
}
