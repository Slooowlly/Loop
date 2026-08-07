//! Feed da pré-temporada: vínculo de cada evento com o jogador, movimentos por
//! mérito e as dispensas de quem ficou sem vaga.

use super::*;

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
pub(super) fn tag_player_relations(conn: &Connection, events: &mut [MarketEvent]) {
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

/// Eventos de RENOVAÇÃO das pré-passes: quem a equipe decidiu segurar. Não movem
/// ninguém de lugar — por isso saem na semana em que o grid não muda, como confirmação
/// de quem ficou, e não junto das dispensas.
pub(super) fn renewal_events(
    report: &crate::market::proposals::MarketReport,
    previous_team: &std::collections::HashMap<String, (String, i32)>,
) -> Vec<MarketEvent> {
    report
        .new_signings
        .iter()
        .filter(|s| s.tipo == "renovacao")
        .map(|s| MarketEvent {
            event_type: MarketEventType::ContractRenewed,
            headline: format!("{} -> {}", s.driver_name, s.team_name),
            description: rust_i18n::t!("market.event.renewed").to_string(),
            driver_id: Some(s.driver_id.clone()),
            driver_name: Some(s.driver_name.clone()),
            team_id: Some(s.team_id.clone()),
            team_name: Some(s.team_name.clone()),
            from_team: Some(s.team_name.clone()),
            to_team: Some(s.team_name.clone()),
            categoria: Some(s.categoria.clone()),
            from_categoria: Some(s.categoria.clone()),
            movement_kind: Some("renewal".to_string()),
            championship_position: None,
            seasons_at_previous: previous_team
                .get(s.driver_id.as_str())
                .map(|(_, tenure)| *tenure),
            relation: None,
        })
        .collect()
}

/// Eventos de DISPENSA: contratos que terminaram (temporada_fim < season) cujo piloto
/// NÃO tem contrato ativo após a passada de contratos (não renovou nem foi
/// reaproveitado); exclui o jogador. É o "quem perdeu a vaga" da semana das saídas.
pub(super) fn build_departure_events(
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
    // Quem se aposentou: o contrato pode estar longe de vencer (veterano com plurianual),
    // então a regra do fim de contrato não o alcança. Ele sai por outro motivo, e o feed
    // precisa dizer qual — senão o assento que a semana 1 mostrava ocupado só evapora.
    let aposentados: std::collections::HashSet<&str> = drivers
        .iter()
        .filter(|d| d.status == crate::models::enums::DriverStatus::Aposentado)
        .map(|d| d.id.as_str())
        .collect();

    let mut events = Vec::new();
    for c in contracts_before {
        let aposentou = aposentados.contains(c.piloto_id.as_str());
        if (c.temporada_fim >= season_number && !aposentou)
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
            description: if aposentou {
                rust_i18n::t!("market.event.retired").to_string()
            } else {
                rust_i18n::t!("market.event.contract_ended").to_string()
            },
            driver_id: Some(c.piloto_id.clone()),
            driver_name: Some(c.piloto_nome.clone()),
            team_id: Some(c.equipe_id.clone()),
            team_name: Some(c.equipe_nome.clone()),
            from_team: Some(c.equipe_nome.clone()),
            to_team: None,
            categoria: Some(c.categoria.clone()),
            from_categoria: Some(c.categoria.clone()),
            movement_kind: Some(if aposentou { "retirement" } else { "departure" }.to_string()),
            championship_position: None,
            seasons_at_previous: previous_team
                .get(c.piloto_id.as_str())
                .map(|(_, tenure)| *tenure),
            relation: None,
        });
    }
    Ok(events)
}
