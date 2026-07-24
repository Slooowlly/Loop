//! Passe de prioridade da ambição de Slam: antes do mercado comum, quem persegue um
//! Slam escolhe primeiro a categoria que falta na coleção.
//!
//! A decisão em si mora em [`crate::market::slam_ambition`]; aqui fica só a aplicação
//! no mercado (escolher a vaga alvo, assinar, registrar o histórico).

use super::*;

/// Categoria-alvo do slam de um piloto, se ele é um slam-chaser ativo (Ambicioso
/// com slam alcançável). `Chase` → a base a coletar; `Stay` → a categoria atual;
/// `None` → não persegue slam (ou deve subir normal).
pub(super) fn slam_target_category(
    conn: &Connection,
    driver: &Driver,
) -> Result<Option<(String, Option<String>)>, String> {
    if driver.personalidade_primaria != Some(PrimaryPersonality::Ambicioso) {
        return Ok(None);
    }
    let (history, current_results) = read_slam_history(conn, driver)?;
    let current = driver.categoria_atual.clone().unwrap_or_default();
    Ok(
        match slam_ambition::decide(
            &history,
            &current,
            driver.atributos.skill,
            true,
            &current_results,
        ) {
            Some(SlamDecision::Chase {
                category, class, ..
            }) => Some((category, class)),
            Some(SlamDecision::Stay { .. }) => Some((current, None)),
            None => None,
        },
    )
}

/// Passe prioritário do slam-chasing: pilotos ambiciosos (personalidade Ambicioso)
/// escolhem PRIMEIRO a melhor vaga (por car_performance) da categoria-alvo do seu
/// slam, antes da disputa normal. Remove as vagas/pilotos usados das listas.
#[allow(dead_code)] // superada pela Janela de Transferências (slam vira bônus no score)
pub(super) fn apply_slam_priority_pass(
    conn: &Connection,
    vacancies: &mut Vec<Vacancy>,
    available: &mut Vec<AvailableDriver>,
    new_season_number: i32,
    rng: &mut impl Rng,
    report: &mut MarketReport,
) -> Result<(), String> {
    // (driver_id, categoria-alvo, classe-alvo, skill) de cada slam-chaser.
    let mut chasers: Vec<(String, String, Option<String>, f64)> = Vec::new();
    for candidate in available.iter() {
        if let Some((category, class)) = slam_target_category(conn, &candidate.driver)? {
            chasers.push((
                candidate.driver.id.clone(),
                category,
                class,
                candidate.driver.atributos.skill,
            ));
        }
    }
    // O mais qualificado escolhe primeiro.
    chasers.sort_by(|a, b| b.3.total_cmp(&a.3));

    for (driver_id, category, class, _) in chasers {
        let Some(driver_index) = available.iter().position(|c| c.driver.id == driver_id) else {
            continue;
        };
        // Melhor vaga (por car_performance) na categoria-alvo; classe deve bater se exigida.
        let best = vacancies
            .iter()
            .enumerate()
            .filter(|(_, vacancy)| {
                vacancy.categoria == category
                    && match &class {
                        Some(target) => vacancy.classe.as_deref() == Some(target.as_str()),
                        None => true,
                    }
            })
            .max_by(|(_, a), (_, b)| a.car_strength.total_cmp(&b.car_strength))
            .map(|(index, _)| index);
        let Some(vacancy_index) = best else {
            continue; // sem vaga na categoria-alvo → cai pro mercado normal
        };

        let candidate = available[driver_index].clone();
        let vacancy = vacancies[vacancy_index].clone();
        let salary = calculate_offer_salary(&vacancy, &candidate.driver, rng);
        let duration = if vacancy.category_tier >= 4 { 3 } else { 2 };
        sign_driver_to_team(
            conn,
            &candidate.driver,
            &vacancy,
            new_season_number,
            salary,
            duration,
            vacancy.papel_necessario.clone(),
        )?;
        report.proposals_made += 1;
        report.proposals_accepted += 1;
        report.new_signings.push(SigningInfo {
            driver_id: candidate.driver.id.clone(),
            driver_name: candidate.driver.nome.clone(),
            team_id: vacancy.team_id.clone(),
            team_name: vacancy.team_name.clone(),
            categoria: vacancy.categoria.clone(),
            papel: vacancy.papel_necessario.as_str().to_string(),
            tipo: "slam".to_string(),
        });
        vacancies.remove(vacancy_index);
        available.remove(driver_index);
    }
    Ok(())
}

/// Monta o histórico de slam de um piloto a partir do archive: todos os títulos
/// (categoria-base + classe) e o resultado campeão-ou-não por temporada na
/// categoria atual (antigo→recente). Vazio se não houver archive.
pub(crate) fn read_slam_history(
    conn: &Connection,
    driver: &Driver,
) -> Result<(Vec<slam_ambition::TitleWin>, Vec<bool>), String> {
    let current = driver.categoria_atual.clone().unwrap_or_default();
    let mut stmt = match conn.prepare(
        "SELECT categoria, posicao_campeonato, snapshot_json
         FROM driver_season_archive WHERE piloto_id = ?1 ORDER BY season_number ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok((Vec::new(), Vec::new())),
    };
    let rows = stmt
        .query_map(params![driver.id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i32>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar historico de slam: {e}"))?;

    let mut history = Vec::new();
    let mut current_results = Vec::new();
    for row in rows {
        let (categoria, posicao, snapshot_json) =
            row.map_err(|e| format!("Falha ao ler historico de slam: {e}"))?;
        let snapshot: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap_or_default();
        let cat_field = snapshot
            .get("categoria")
            .and_then(|value| value.as_str())
            .unwrap_or(&categoria);
        let base = cat_field.split(':').next().unwrap_or(cat_field).to_string();
        let class = snapshot
            .get("classe")
            .or_else(|| snapshot.get("class_name"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| {
                cat_field
                    .split_once(':')
                    .map(|(_, class)| class.to_string())
            });
        let titulos = snapshot
            .get("titulos")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let champion = posicao == Some(1) || titulos > 0;
        if champion {
            history.push(slam_ambition::TitleWin {
                category: base.clone(),
                class: class.clone(),
            });
        }
        if base == current {
            current_results.push(champion);
        }
    }
    Ok((history, current_results))
}
