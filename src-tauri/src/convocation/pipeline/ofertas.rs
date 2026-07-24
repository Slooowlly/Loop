//! Ofertas de convocação para o JOGADOR: os filtros de elegibilidade por classe
//! (categoria atual, rookie excepcional, histórico de carro/equipe) e a montagem
//! das ofertas que a UI mostra.

use super::*;

fn is_primary_current_category_for_class(cfg: &ClasseConfig, category: &str) -> bool {
    cfg.feeder_category == category
}

fn is_exceptional_rookie_for_class(player: &Driver, cfg: &ClasseConfig) -> bool {
    let Some(current_category) = player.categoria_atual.as_deref() else {
        return false;
    };

    let rookie_matches = matches!(
        (cfg.class_name, current_category),
        ("mazda", "mazda_rookie") | ("toyota", "toyota_rookie")
    );
    let exceptional = player.atributos.skill >= 84.0
        || (player.melhor_resultado_temp == Some(1) && player.stats_temporada.vitorias >= 2);

    rookie_matches && exceptional
}

fn contract_matches_class_lane(
    contract: &crate::models::contract::Contract,
    cfg: &ClasseConfig,
) -> bool {
    if contract.categoria == cfg.special_category
        && contract.classe.as_deref() == Some(cfg.class_name)
    {
        return true;
    }

    match cfg.class_name {
        "mazda" => matches!(contract.categoria.as_str(), "mazda_amador" | "mazda_rookie"),
        "toyota" => matches!(
            contract.categoria.as_str(),
            "toyota_amador" | "toyota_rookie"
        ),
        "bmw" => contract.categoria == "bmw_m2",
        "gt4" => contract.categoria == "gt4",
        "gt3" => contract.categoria == "gt3",
        _ => false,
    }
}

fn player_has_same_car_history(
    contracts: &[crate::models::contract::Contract],
    cfg: &ClasseConfig,
) -> bool {
    contracts
        .iter()
        .any(|contract| contract_matches_class_lane(contract, cfg))
}

fn player_has_team_history(contracts: &[crate::models::contract::Contract], team_id: &str) -> bool {
    contracts
        .iter()
        .any(|contract| contract.equipe_id == team_id)
}

fn player_offer_quality_score(player: &Driver) -> f64 {
    let champion_bonus = if player.melhor_resultado_temp == Some(1) {
        8.0
    } else {
        0.0
    };
    let wins_bonus = (player.stats_temporada.vitorias.min(5) as f64) * 2.0;
    player.atributos.skill + champion_bonus + wins_bonus
}

fn fallback_quality_threshold(cfg: &ClasseConfig) -> f64 {
    match cfg.special_category {
        "endurance" => 90.0,
        _ => 82.0,
    }
}

pub(super) fn build_player_special_offers(
    conn: &Connection,
    season_id: &str,
    player: &Driver,
) -> Result<Vec<PlayerSpecialOffer>, DbError> {
    let papel = if player.atributos.skill >= 85.0 {
        TeamRole::Numero1
    } else {
        TeamRole::Numero2
    };
    let current_category = player.categoria_atual.as_deref();
    let current_category_is_regular = current_category.and_then(get_category_config).is_some();
    let has_active_regular_contract =
        contract_queries::has_active_regular_contract(conn, &player.id)?;
    let contract_history = contract_queries::get_contracts_for_pilot(conn, &player.id)?;
    let quality_score = player_offer_quality_score(player);

    let mut preferred: Vec<(i32, String, String, String, String)> = Vec::new();
    let mut fallback: Vec<(i32, String, String, String, String)> = Vec::new();

    for cfg in legacy_convocation_classes() {
        let teams = get_special_class_entry_teams(conn, season_id, cfg)?;

        for team in teams {
            let team_history = player_has_team_history(&contract_history, &team.id);
            let primary_current_fit = current_category
                .is_some_and(|category| is_primary_current_category_for_class(cfg, category));
            let rookie_exception = is_exceptional_rookie_for_class(player, cfg);
            let same_car_history = player_has_same_car_history(&contract_history, cfg);
            let license_ok =
                driver_has_required_license_for_category(conn, &player.id, cfg.special_category)
                    .map_err(DbError::InvalidData)?;

            let preferred_priority = if team_history && !current_category_is_regular {
                Some(520)
            } else if primary_current_fit {
                Some(500)
            } else if rookie_exception {
                Some(460)
            } else if !has_active_regular_contract && same_car_history {
                Some(400)
            } else if team_history {
                Some(320)
            } else {
                None
            };

            if let Some(priority) = preferred_priority {
                preferred.push((
                    priority + (team.car_strength() * 0.16).round() as i32,
                    team.id,
                    team.nome,
                    cfg.special_category.to_string(),
                    cfg.class_name.to_string(),
                ));
                continue;
            }

            if license_ok && quality_score >= fallback_quality_threshold(cfg) {
                fallback.push((
                    100 + (team.car_strength() * 0.16).round() as i32,
                    team.id,
                    team.nome,
                    cfg.special_category.to_string(),
                    cfg.class_name.to_string(),
                ));
            }
        }
    }

    preferred.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.2.cmp(&right.2)));
    fallback.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.2.cmp(&right.2)));

    let mut selected = Vec::new();
    let mut seen_team_ids = std::collections::HashSet::new();

    for entry in preferred.into_iter().chain(fallback.into_iter()) {
        if seen_team_ids.insert(entry.1.clone()) {
            selected.push(entry);
        }
        if selected.len() == 3 {
            break;
        }
    }

    Ok(selected
        .into_iter()
        .map(
            |(_, team_id, team_name, special_category, class_name)| PlayerSpecialOffer {
                id: format!(
                    "PSO-{season_id}-{}-{}-{}",
                    player.id,
                    team_id,
                    papel.as_str()
                ),
                player_driver_id: player.id.clone(),
                team_id,
                team_name,
                special_category,
                class_name,
                papel: papel.clone(),
                status: "Pendente".to_string(),
            },
        )
        .collect())
}
