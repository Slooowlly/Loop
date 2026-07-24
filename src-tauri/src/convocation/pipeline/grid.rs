//! Montagem do grid de cada classe especial: garante as inscrições das equipes,
//! calcula as vagas, coleta candidatos e distribui os pilotos por cota/score.

use super::*;

pub(super) fn ensure_special_team_entries(
    conn: &Connection,
    season_id: &str,
    _season_number: i32,
) -> Result<(), DbError> {
    for cfg in legacy_convocation_classes() {
        let target_slots = target_slots_for_class(conn, cfg)?;
        let mut entries = Vec::new();
        let mut used_team_ids = std::collections::HashSet::new();

        let legacy_special_teams = team_queries::get_teams_by_category_and_class(
            conn,
            cfg.special_category,
            cfg.class_name,
        )?;
        if !legacy_special_teams.is_empty() {
            for team in legacy_special_teams.into_iter().take(target_slots) {
                if !used_team_ids.insert(team.id.clone()) {
                    continue;
                }
                entries.push(special_entry_queries::NewSpecialTeamEntry {
                    team_id: team.id,
                    source_category: cfg.special_category.to_string(),
                    qualified_via: "ClasseEspecial".to_string(),
                    guaranteed_next_year: false,
                });
            }

            special_entry_queries::replace_entries_for_class(
                conn,
                season_id,
                cfg.special_category,
                cfg.class_name,
                &entries,
            )?;
            continue;
        }

        let regular_standings = calculate_constructor_standings(conn, cfg.feeder_category)
            .map_err(DbError::Migration)?;
        for standing in regular_standings {
            if entries.len() >= target_slots {
                break;
            }
            if !used_team_ids.insert(standing.team_id.clone()) {
                continue;
            }
            entries.push(special_entry_queries::NewSpecialTeamEntry {
                team_id: standing.team_id,
                source_category: cfg.feeder_category.to_string(),
                qualified_via: format!("RegularP{}", standing.posicao),
                guaranteed_next_year: false,
            });
        }

        special_entry_queries::replace_entries_for_class(
            conn,
            season_id,
            cfg.special_category,
            cfg.class_name,
            &entries,
        )?;
    }

    Ok(())
}

fn target_slots_for_class(conn: &Connection, cfg: &ClasseConfig) -> Result<usize, DbError> {
    let legacy_special_teams =
        team_queries::get_teams_by_category_and_class(conn, cfg.special_category, cfg.class_name)?;
    if !legacy_special_teams.is_empty() {
        return Ok(legacy_special_teams.len());
    }

    Ok(match cfg.special_category {
        "endurance" => 6,
        _ => 5,
    })
}

pub(super) fn get_special_class_entry_teams(
    conn: &Connection,
    season_id: &str,
    cfg: &ClasseConfig,
) -> Result<Vec<crate::models::team::Team>, DbError> {
    let teams = special_entry_queries::get_entry_teams_for_class(
        conn,
        season_id,
        cfg.special_category,
        cfg.class_name,
    )?;
    if !teams.is_empty() {
        return Ok(teams);
    }

    let legacy_teams =
        team_queries::get_teams_by_category_and_class(conn, cfg.special_category, cfg.class_name)?;
    if !legacy_teams.is_empty() {
        return Ok(legacy_teams);
    }

    team_queries::get_teams_by_category(conn, cfg.feeder_category)
}

pub(super) fn montar_grid_classe(
    conn: &Connection,
    cfg: &ClasseConfig,
    _season_number: i32,
    season_id: &str,
    globally_excluded: &std::collections::HashSet<String>,
) -> Result<GridClasse, DbError> {
    // 1. Equipes regulares classificadas para a classe especial.
    let teams = get_special_class_entry_teams(conn, season_id, cfg)?;
    if teams.is_empty() {
        return Err(DbError::NotFound(format!(
            "Nenhuma equipe para {}/{}",
            cfg.special_category, cfg.class_name
        )));
    }

    let total_assentos = teams.len() * 2;
    let cotas = calcular_cotas(total_assentos);

    // 2. Candidatos de todas as fontes
    let candidatos = coletar_candidatos(
        conn,
        cfg.special_category,
        cfg.class_name,
        cfg.feeder_category,
    )?;

    // 3. Calcular scores e separar por fonte (excluir já alocados globalmente)
    let mut fonte_a: Vec<(String, f64)> = Vec::new();
    let mut fonte_b: Vec<(String, f64)> = Vec::new();
    let mut fonte_c: Vec<(String, f64)> = Vec::new();
    let mut fonte_d: Vec<(String, f64)> = Vec::new();

    for c in candidatos
        .iter()
        .filter(|c| !globally_excluded.contains(&c.driver_id))
    {
        let historico = contract_queries::get_especial_contract_count(
            conn,
            &c.driver_id,
            cfg.special_category,
            cfg.class_name,
        )
        .unwrap_or(0);
        let score = calcular_score(&c.driver, &c.fonte, historico);
        match c.fonte {
            FonteConvocacao::MeritoRegular => fonte_a.push((c.driver_id.clone(), score)),
            FonteConvocacao::ContinuidadeHistorica => fonte_b.push((c.driver_id.clone(), score)),
            FonteConvocacao::PoolGlobal => fonte_c.push((c.driver_id.clone(), score)),
            FonteConvocacao::Wildcard => fonte_d.push((c.driver_id.clone(), score)),
        }
    }

    // 4. Ordenar cada fonte por score desc
    for v in [&mut fonte_a, &mut fonte_b, &mut fonte_c, &mut fonte_d] {
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    // 5. Selecionar por cota com overflow B/C → A
    let mut selecionados: Vec<(String, FonteConvocacao, f64)> = Vec::new();

    // D (wildcard): máximo 1
    let d_count = cotas.wildcard.min(fonte_d.len());
    for (id, score) in fonte_d.iter().take(d_count) {
        selecionados.push((id.clone(), FonteConvocacao::Wildcard, *score));
    }

    // B (continuidade)
    let b_count = cotas.continuidade.min(fonte_b.len());
    let b_overflow = cotas.continuidade.saturating_sub(b_count);
    for (id, score) in fonte_b.iter().take(b_count) {
        selecionados.push((id.clone(), FonteConvocacao::ContinuidadeHistorica, *score));
    }

    // C (pool)
    let c_count = cotas.pool_global.min(fonte_c.len());
    let c_overflow = cotas.pool_global.saturating_sub(c_count);
    for (id, score) in fonte_c.iter().take(c_count) {
        selecionados.push((id.clone(), FonteConvocacao::PoolGlobal, *score));
    }

    // A (mérito) + overflow de B e C
    let a_total = cotas.merito_regular + b_overflow + c_overflow;

    // Remover da pool A quem já foi selecionado via outra fonte
    let ja_selecionados: std::collections::HashSet<String> =
        selecionados.iter().map(|(id, _, _)| id.clone()).collect();

    let mut idx = 0;
    for (id, score) in &fonte_a {
        if ja_selecionados.contains(id) {
            continue;
        }
        if idx >= a_total {
            break;
        }
        selecionados.push((id.clone(), FonteConvocacao::MeritoRegular, *score));
        idx += 1;
    }

    // 6. Ordenar selecionados por score desc para distribuição equitativa
    selecionados.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // 7. Distribuir: posição 2i → team[i] N1, posição 2i+1 → team[i] N2
    let mut assignments: Vec<DriverAssignment> = Vec::new();
    for (i, (driver_id, fonte, score)) in selecionados.iter().enumerate() {
        let team_idx = i / 2;
        if team_idx >= teams.len() {
            break; // mais pilotos que assentos (não deve ocorrer, mas defensivo)
        }
        let papel = if i % 2 == 0 {
            TeamRole::Numero1
        } else {
            TeamRole::Numero2
        };
        assignments.push(DriverAssignment {
            driver_id: driver_id.clone(),
            team_id: teams[team_idx].id.clone(),
            papel,
            fonte: fonte_label(fonte),
            score: *score,
        });
    }

    Ok(GridClasse {
        class_name: cfg.class_name.to_string(),
        assignments,
    })
}

fn fonte_label(fonte: &FonteConvocacao) -> String {
    match fonte {
        FonteConvocacao::MeritoRegular => "MeritoRegular".into(),
        FonteConvocacao::ContinuidadeHistorica => "ContinuidadeHistorica".into(),
        FonteConvocacao::PoolGlobal => "PoolGlobal".into(),
        FonteConvocacao::Wildcard => "Wildcard".into(),
    }
}
