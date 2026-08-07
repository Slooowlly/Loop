//! Eventos especiais na ficha do piloto: participacoes, campanhas agregadas e o ranking global dessas metricas.

use rusqlite::OptionalExtension;

use super::*;

/// O bloco de especiais e, junto, o detalhe que abre no hover das linhas dele.
///
/// Os dois saem da MESMA leitura de contratos e campanhas: separar em duas
/// funcoes publicas custaria as consultas duas vezes para desenhar a mesma lista.
pub(super) fn build_special_events_block(
    conn: &Connection,
    driver_id: &str,
) -> Result<
    (
        DriverCareerSpecialEventsBlock,
        HashMap<String, Vec<DriverCareerDetailEntry>>,
    ),
    String,
> {
    if !sqlite_table_exists(conn, "contracts")? {
        return Ok((DriverCareerSpecialEventsBlock::default(), HashMap::new()));
    }

    let contracts = load_special_contract_rows(conn, driver_id)?;
    if contracts.is_empty() {
        return Ok((DriverCareerSpecialEventsBlock::default(), HashMap::new()));
    }

    let campaigns = load_special_campaign_aggregates(conn, driver_id, &contracts)?;
    let vitorias = campaigns.iter().map(|campaign| campaign.wins).sum::<i32>();
    let podios = campaigns
        .iter()
        .map(|campaign| campaign.podiums)
        .sum::<i32>();
    let rankings = build_special_event_rank_block(conn, driver_id)?;
    let melhor_campanha = campaigns
        .iter()
        .max_by(|a, b| {
            a.points
                .cmp(&b.points)
                .then_with(|| a.wins.cmp(&b.wins))
                .then_with(|| a.podiums.cmp(&b.podiums))
                .then_with(|| a.year.cmp(&b.year))
        })
        .map(|campaign| DriverSpecialCampaignBlock {
            ano: campaign.year,
            categoria: campaign.category.clone(),
            classe: campaign.class_name.clone(),
            equipe: campaign.team_name.clone(),
            pontos: campaign.points,
            vitorias: campaign.wins,
            podios: campaign.podiums,
        });

    let timeline: Vec<DriverSpecialEventEntry> = contracts
        .iter()
        .map(|contract| DriverSpecialEventEntry {
            ano: contract.year,
            categoria: contract.category.clone(),
            classe: contract.class_name.clone(),
            equipe: contract.team_name.clone(),
        })
        .collect();
    let ultimo_evento = timeline.iter().max_by_key(|item| item.ano).cloned();

    let cores = load_special_team_colors(conn, &contracts)?;
    let mut detalhes: HashMap<String, Vec<DriverCareerDetailEntry>> = HashMap::new();
    // Participacoes e convocacoes contam os MESMOS contratos — o painel e um so,
    // porque inventar dois recortes de uma lista identica seria mentira.
    detalhes.insert(
        "especiais".to_string(),
        contracts
            .iter()
            .map(|contract| DriverCareerDetailEntry {
                periodo: Some(contract.year.to_string()),
                categoria: timeline_division_key(
                    &contract.category,
                    contract.class_name.as_deref(),
                ),
                equipe: contract.team_name.clone(),
                equipe_cor: cores.get(&contract.team_id).cloned(),
                equipe_id: cores
                    .contains_key(&contract.team_id)
                    .then(|| contract.team_id.clone()),
                ..Default::default()
            })
            .collect(),
    );
    detalhes.insert(
        "especiais_vitorias".to_string(),
        campaigns
            .iter()
            .filter(|campaign| campaign.wins > 0)
            .map(|campaign| DriverCareerDetailEntry {
                periodo: Some(campaign.year.to_string()),
                categoria: timeline_division_key(
                    &campaign.category,
                    campaign.class_name.as_deref(),
                ),
                equipe: campaign.team_name.clone(),
                equipe_cor: campaign
                    .team_name
                    .as_ref()
                    .and_then(|nome| {
                        contracts
                            .iter()
                            .find(|contract| contract.team_name.as_ref() == Some(nome))
                    })
                    .and_then(|contract| cores.get(&contract.team_id).cloned()),
                equipe_id: campaign
                    .team_name
                    .as_ref()
                    .and_then(|nome| {
                        contracts
                            .iter()
                            .find(|contract| contract.team_name.as_ref() == Some(nome))
                    })
                    .filter(|contract| cores.contains_key(&contract.team_id))
                    .map(|contract| contract.team_id.clone()),
                contagem: Some(campaign.wins),
                resumo: Some(format!("{} pts", campaign.points)),
                ..Default::default()
            })
            .collect(),
    );
    detalhes.retain(|_, linhas| !linhas.is_empty());

    Ok((
        DriverCareerSpecialEventsBlock {
            participacoes: contracts.len() as i32,
            convocacoes: contracts.len() as i32,
            vitorias,
            podios,
            rankings,
            melhor_campanha,
            ultimo_evento,
            timeline,
        },
        detalhes,
    ))
}

/// A cor de cada equipe de evento especial. Le so a cor: o nome ja veio no
/// contrato, e montar a equipe inteira exigiria o schema todo de `teams`.
fn load_special_team_colors(
    conn: &Connection,
    contracts: &[SpecialContractRow],
) -> Result<HashMap<String, String>, String> {
    let mut cores = HashMap::new();
    let mut stmt = match conn.prepare("SELECT cor_primaria FROM teams WHERE id = ?1") {
        Ok(stmt) => stmt,
        Err(_) => return Ok(cores),
    };
    for contract in contracts {
        if contract.team_id.trim().is_empty() || cores.contains_key(&contract.team_id) {
            continue;
        }
        let cor = stmt
            .query_row(rusqlite::params![contract.team_id], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|e| format!("Falha ao carregar cor de equipe especial: {e}"))?;
        if let Some(cor) = cor {
            cores.insert(contract.team_id.clone(), cor);
        }
    }
    Ok(cores)
}

pub(super) fn sqlite_table_exists(conn: &Connection, table_name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
         )",
        rusqlite::params![table_name],
        |row| row.get::<_, i32>(0),
    )
    .map(|value| value != 0)
    .map_err(|e| format!("Falha ao verificar tabela '{table_name}': {e}"))
}

pub(super) fn load_special_contract_rows(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<SpecialContractRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                CAST(c.temporada_inicio AS INTEGER) AS season_number,
                COALESCE(s.ano, CAST(c.temporada_inicio AS INTEGER)) AS ano,
                c.categoria,
                c.classe,
                c.equipe_id,
                NULLIF(c.equipe_nome, '') AS equipe_nome
             FROM contracts c
             LEFT JOIN seasons s ON s.numero = CAST(c.temporada_inicio AS INTEGER)
             WHERE c.piloto_id = ?1 AND c.tipo = 'Especial'
             ORDER BY CAST(c.temporada_inicio AS INTEGER) ASC, c.categoria ASC, c.classe ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico de eventos especiais: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            Ok(SpecialContractRow {
                season_number: row.get(0)?,
                year: row.get(1)?,
                category: row.get(2)?,
                class_name: row.get(3)?,
                team_id: row.get(4)?,
                team_name: row.get(5)?,
            })
        })
        .map_err(|e| format!("Falha ao consultar historico de eventos especiais: {e}"))?;

    let mut rows = Vec::new();
    for row in mapped {
        let contract =
            row.map_err(|e| format!("Falha ao ler historico de evento especial: {e}"))?;
        if categories::runs_in_special_phase(&contract.category) {
            rows.push(contract);
        }
    }
    Ok(rows)
}

pub(super) fn build_special_event_rank_block(
    conn: &Connection,
    driver_id: &str,
) -> Result<DriverSpecialEventRankBlock, String> {
    let contract_counts = load_special_contract_counts(conn)?;
    let result_counts = load_special_result_counts(conn)?;
    let wins: Vec<(String, i32)> = result_counts
        .iter()
        .map(|(pilot_id, (wins, _))| (pilot_id.clone(), *wins))
        .collect();
    let podiums: Vec<(String, i32)> = result_counts
        .iter()
        .map(|(pilot_id, (_, podiums))| (pilot_id.clone(), *podiums))
        .collect();

    Ok(DriverSpecialEventRankBlock {
        participacoes: rank_special_event_metric(&contract_counts, driver_id),
        convocacoes: rank_special_event_metric(&contract_counts, driver_id),
        vitorias: rank_special_event_metric(&wins, driver_id),
        podios: rank_special_event_metric(&podiums, driver_id),
    })
}

pub(super) fn load_special_contract_counts(
    conn: &Connection,
) -> Result<Vec<(String, i32)>, String> {
    if !sqlite_table_exists(conn, "contracts")? {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare("SELECT piloto_id, categoria FROM contracts WHERE tipo = 'Especial'")
        .map_err(|e| format!("Falha ao preparar ranking de eventos especiais: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar ranking de eventos especiais: {e}"))?;

    let mut counts: HashMap<String, i32> = HashMap::new();
    for row in rows {
        let (pilot_id, category) =
            row.map_err(|e| format!("Falha ao ler ranking de eventos especiais: {e}"))?;
        if categories::runs_in_special_phase(&category) {
            *counts.entry(pilot_id).or_insert(0) += 1;
        }
    }

    Ok(counts.into_iter().collect())
}

pub(super) fn load_special_result_counts(
    conn: &Connection,
) -> Result<HashMap<String, (i32, i32)>, String> {
    if !sqlite_table_exists(conn, "race_results")? || !sqlite_table_exists(conn, "calendar")? {
        return Ok(HashMap::new());
    }

    let mut stmt = conn
        .prepare(
            "SELECT
                r.piloto_id,
                c.categoria,
                COALESCE(SUM(CASE WHEN r.dnf = 0 AND r.posicao_final = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN r.dnf = 0 AND r.posicao_final <= 3 THEN 1 ELSE 0 END), 0)
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             GROUP BY r.piloto_id, c.categoria",
        )
        .map_err(|e| format!("Falha ao preparar ranking de resultados especiais: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar ranking de resultados especiais: {e}"))?;

    let mut counts: HashMap<String, (i32, i32)> = HashMap::new();
    for row in rows {
        let (pilot_id, category, wins, podiums) =
            row.map_err(|e| format!("Falha ao ler ranking de resultados especiais: {e}"))?;
        if categories::runs_in_special_phase(&category) {
            let entry = counts.entry(pilot_id).or_insert((0, 0));
            entry.0 += wins;
            entry.1 += podiums;
        }
    }

    Ok(counts)
}

pub(super) fn rank_special_event_metric(rows: &[(String, i32)], driver_id: &str) -> Option<i32> {
    let mut ranked: Vec<(&str, i32)> = rows
        .iter()
        .filter(|(_, value)| *value > 0)
        .map(|(pilot_id, value)| (pilot_id.as_str(), *value))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked
        .iter()
        .position(|(pilot_id, _)| *pilot_id == driver_id)
        .map(|index| index as i32 + 1)
}

pub(super) fn load_special_campaign_aggregates(
    conn: &Connection,
    driver_id: &str,
    contracts: &[SpecialContractRow],
) -> Result<Vec<SpecialCampaignAggregate>, String> {
    if !sqlite_table_exists(conn, "race_results")? || !sqlite_table_exists(conn, "calendar")? {
        return Ok(Vec::new());
    }

    let mut campaigns = Vec::new();
    for contract in contracts {
        let (points, wins, podiums): (f64, i32, i32) = conn
            .query_row(
                "SELECT
                    COALESCE(SUM(r.pontos), 0.0),
                    COALESCE(SUM(CASE WHEN r.dnf = 0 AND r.posicao_final = 1 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN r.dnf = 0 AND r.posicao_final <= 3 THEN 1 ELSE 0 END), 0)
                 FROM race_results r
                 INNER JOIN calendar c ON c.id = r.race_id
                 LEFT JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
                 WHERE r.piloto_id = ?1
                   AND r.equipe_id = ?2
                   AND c.categoria = ?3
                   AND COALESCE(s.numero, 0) = ?4",
                rusqlite::params![
                    driver_id,
                    contract.team_id,
                    contract.category,
                    contract.season_number
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| format!("Falha ao agregar campanha especial: {e}"))?;

        campaigns.push(SpecialCampaignAggregate {
            year: contract.year,
            category: contract.category.clone(),
            class_name: contract.class_name.clone(),
            team_name: contract.team_name.clone(),
            points: points.round() as i32,
            wins,
            podiums,
        });
    }
    Ok(campaigns)
}
