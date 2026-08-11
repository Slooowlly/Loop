//! Classificacoes de equipes e das categorias especiais, incluindo o fallback de resultados recentes.

use super::*;

pub(crate) fn get_regular_standings_participant_ids(
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

pub(crate) fn get_special_driver_standings_from_results(
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

pub(crate) fn query_special_driver_standing_rows(
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

pub(crate) fn merge_recent_results_fallback(
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

pub(crate) fn parse_recent_results_json(value: &serde_json::Value) -> Vec<RoundResult> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_recent_result_entry)
        .collect()
}

pub(crate) fn parse_recent_result_entry(value: &serde_json::Value) -> Option<RoundResult> {
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
        .ok_or_else(errors::active_season_not_found)?;
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

    // Recordes DA CATEGORIA, em duas consultas agregadas (não uma por equipe).
    let category_titles = team_queries::get_category_constructor_titles_by_team(&db.conn, &category)
        .unwrap_or_default();
    let category_wins =
        team_queries::get_category_wins_by_team(&db.conn, &category).unwrap_or_default();

    let mut standings: Vec<TeamStanding> = teams
        .into_iter()
        .map(|team| {
            let team_id = team.id.clone();
            let slot_1 = get_driver_slot_info(
                &db,
                team.piloto_1_id.as_ref(),
                &team_id,
                active_season_number,
            );
            let slot_2 = get_driver_slot_info(
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
                piloto_1_id: slot_1.id,
                piloto_1_nome: slot_1.nome,
                piloto_1_tenure_seasons: slot_1.tenure_seasons,
                piloto_1_contrato_vence: slot_1.contrato_vence,
                piloto_1_aposentado: slot_1.aposentado,
                piloto_2_id: slot_2.id,
                piloto_2_nome: slot_2.nome,
                piloto_2_tenure_seasons: slot_2.tenure_seasons,
                piloto_2_contrato_vence: slot_2.contrato_vence,
                piloto_2_aposentado: slot_2.aposentado,
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
                categoria_titulos: category_titles.get(&team_id).copied().unwrap_or(0),
                categoria_vitorias: category_wins.get(&team_id).copied().unwrap_or(0),
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

pub(crate) fn previous_team_positions_by_team(
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

pub(crate) fn get_special_team_standings_from_results(
    conn: &rusqlite::Connection,
    season: &Season,
    category: &str,
    previous_champions: &PreviousChampions,
) -> Result<Vec<TeamStanding>, String> {
    let rows = query_special_team_standing_rows(conn, &season.id, category)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let category_titles =
        team_queries::get_category_constructor_titles_by_team(conn, category).unwrap_or_default();
    let category_wins = team_queries::get_category_wins_by_team(conn, category).unwrap_or_default();

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
                piloto_1_id: driver_names.first().map(|(id, _)| id.clone()),
                piloto_1_nome: driver_names.first().map(|(_, nome)| nome.clone()),
                piloto_1_tenure_seasons: None,
                // Grid de fase especial: sem contrato regular por trás, nada a vencer.
                piloto_1_contrato_vence: false,
                piloto_1_aposentado: false,
                piloto_2_id: driver_names.get(1).map(|(id, _)| id.clone()),
                piloto_2_nome: driver_names.get(1).map(|(_, nome)| nome.clone()),
                piloto_2_tenure_seasons: None,
                piloto_2_contrato_vence: false,
                piloto_2_aposentado: false,
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
                categoria_titulos: category_titles.get(&team_id).copied().unwrap_or(0),
                categoria_vitorias: category_wins.get(&team_id).copied().unwrap_or(0),
            })
        })
        .collect()
}

pub(crate) fn query_special_team_standing_rows(
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

/// Os dois pilotos que mais correram pela equipe na fase especial, em `(id, nome)`.
///
/// O id vem junto porque o grid do mercado abre a ficha do piloto por id — sem ele
/// as categorias de fase especial seriam as únicas em que clicar num nome do grid
/// não faz nada.
pub(crate) fn query_special_team_driver_names(
    conn: &rusqlite::Connection,
    season_id: &str,
    category: &str,
    team_id: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.piloto_id, d.nome
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
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar pilotos da equipe especial: {e}"))?;

    let mut names = Vec::new();
    for row in mapped {
        names.push(row.map_err(|e| format!("Falha ao ler piloto da equipe especial: {e}"))?);
    }
    Ok(names)
}

/// Níveis das 11 peças do carro de cada equipe da categoria.
///
/// O comparativo da aba Minha Equipe mostrava `car_level`, que é a MÉDIA das onze —
/// e média esconde justamente o que a comparação precisa: duas equipes em "Nível 4"
/// podem ter chassis 9 / motor 1 e chassis 4 / motor 4. São investimentos opostos
/// com a mesma etiqueta.
///
/// Equipe sem carro persistido (save antigo, categoria spec recém-criada) sai da
/// lista em vez de entrar com onze zeros: um polígono colapsado no centro afirmaria
/// que a equipe tem o pior carro do grid, e o que se sabe é que ela não tem carro
/// gravado.
pub(crate) fn get_teams_car_parts_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
) -> Result<Vec<TeamCarParts>, String> {
    let category = category.trim().to_lowercase();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let teams = team_queries::get_teams_by_category(&db.conn, &category)
        .map_err(|e| format!("Falha ao buscar equipes da categoria: {e}"))?;

    let mut result = Vec::with_capacity(teams.len());
    for team in teams {
        let Some(car) = team_car_queries::get_team_car(&db.conn, &team.id)
            .map_err(|e| format!("Falha ao carregar carro da equipe: {e}"))?
        else {
            continue;
        };

        let parts = car
            .parts
            .iter()
            .map(|part| TeamCarPartLevel {
                key: part.part_type.as_str().to_string(),
                level: part.level,
            })
            .collect();

        result.push(TeamCarParts {
            team_id: team.id,
            nome: team.nome,
            nome_curto: team.nome_curto,
            cor_primaria: team.cor_primaria,
            parts,
        });
    }

    Ok(result)
}
