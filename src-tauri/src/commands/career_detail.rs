use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use crate::commands::career::count_calendar_entries;
use crate::commands::career_types::{
    CareerMilestone, ContractDetail, DriverBadge, DriverBestSeasonBlock, DriverCareerCategoryStint,
    DriverCareerFirstMarksBlock, DriverCareerHistoryBlock, DriverCareerMobilityBlock,
    DriverCareerPathBlock, DriverCareerPeakBlock, DriverCareerPresenceBlock, DriverCareerRankBlock,
    DriverCareerSpecialEventsBlock, DriverCompetitiveBlock, DriverContractMarketBlock,
    DriverCurrentSummaryBlock, DriverDetail, DriverFormBlock, DriverLicenseInfo, DriverMarketBlock,
    DriverPerformanceBlock, DriverPerformanceReadBlock, DriverProfileBlock, DriverRivalInfo,
    DriverRivalsBlock, DriverSpecialCampaignBlock, DriverSpecialEventEntry,
    DriverSpecialEventRankBlock, DriverTechnicalReadBlock, DriverTechnicalReadItem,
    FormResultEntry, PerformanceStatsBlock, PersonalityInfo, StatsBlock, TagInfo,
};
use crate::commands::race_history::build_driver_histories;
use crate::constants::categories;
use crate::db::queries::drivers as driver_queries;
use crate::models::contract::Contract;
use crate::models::driver::{AttributeTag, Driver, TagLevel};
use crate::models::enums::{DriverStatus, PrimaryPersonality, SecondaryPersonality};
use crate::models::season::Season;
use crate::models::team::Team;

#[derive(Debug, Clone)]
struct HistoricalRaceResult {
    rodada: i32,
    position: i32,
    is_dnf: bool,
    has_fastest_lap: bool,
}

#[derive(Debug, Clone)]
struct ArchivedRecentResults {
    results: Vec<HistoricalRaceResult>,
    form_context: Option<String>,
}

#[derive(Debug, Clone)]
struct CareerSeasonArchiveRow {
    ano: i32,
    categoria: String,
    posicao_campeonato: Option<i32>,
    pontos: f64,
    corridas: i32,
    vitorias: i32,
    podios: i32,
}

#[derive(Debug, Clone)]
struct CareerRaceHistoryRow {
    race_index: i32,
    season_number: i32,
    team_id: String,
    position: i32,
    is_dnf: bool,
}

#[derive(Debug, Clone)]
struct SpecialContractRow {
    season_number: i32,
    year: i32,
    category: String,
    class_name: Option<String>,
    team_id: String,
    team_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct SpecialCampaignAggregate {
    year: i32,
    category: String,
    class_name: Option<String>,
    team_name: Option<String>,
    points: i32,
    wins: i32,
    podiums: i32,
}

pub(crate) fn build_driver_detail_payload(
    conn: &Connection,
    career_dir: &Path,
    season: &Season,
    driver: &Driver,
    contract: Option<&Contract>,
    team: Option<&Team>,
    role: Option<String>,
) -> Result<DriverDetail, String> {
    let category_id = resolve_driver_category(driver, contract, team);
    let status = driver_detail_status(driver, contract.is_some());
    let personality_primaria = driver
        .personalidade_primaria
        .as_ref()
        .map(convert_primary_personality);
    let personalidade_secundaria = driver
        .personalidade_secundaria
        .as_ref()
        .map(convert_secondary_personality);
    let tags = convert_tags(&driver.get_visible_tags());
    let (qualidades, defeitos) = split_driver_tags(&tags);
    let contract_detail = contract
        .as_ref()
        .map(|value| build_contract_detail(value, season.numero, season.ano));
    let mut recent_results = category_id
        .as_deref()
        .map(|category| {
            build_recent_results_for_driver(conn, career_dir, &season.id, category, &driver.id)
        })
        .transpose()?
        .unwrap_or_default();
    let mut form_context = None;
    if recent_results.is_empty() {
        let archived = build_archived_recent_results_for_driver(conn, season.numero, &driver.id)?;
        form_context = archived.form_context;
        recent_results = archived.results;
    }
    let championship_position = category_id
        .as_deref()
        .map(|category| find_championship_position(conn, category, &driver.id))
        .transpose()?
        .flatten();
    let teammate = find_teammate(conn, driver, team)?;
    let badges = build_driver_badges(driver, category_id.as_deref());

    Ok(DriverDetail {
        id: driver.id.clone(),
        nome: driver.nome.clone(),
        nacionalidade: driver.nacionalidade.clone(),
        idade: driver.idade as i32,
        genero: driver.genero.clone(),
        is_jogador: driver.is_jogador,
        status: status.clone(),
        equipe_id: team.as_ref().map(|value| value.id.clone()),
        equipe_nome: team.as_ref().map(|value| value.nome.clone()),
        equipe_cor_primaria: team.as_ref().map(|value| value.cor_primaria.clone()),
        equipe_cor_secundaria: team.as_ref().map(|value| value.cor_secundaria.clone()),
        papel: role.clone(),
        personalidade_primaria: personality_primaria.clone(),
        personalidade_secundaria: personalidade_secundaria.clone(),
        motivacao: driver.motivacao.round().clamp(0.0, 100.0) as u8,
        tags: tags.clone(),
        stats_temporada: build_season_stats_block(driver),
        stats_carreira: build_career_stats_block(driver),
        contrato: contract_detail.clone(),
        perfil: build_driver_profile_block(
            driver,
            &status,
            team,
            role.as_deref(),
            category_id.as_deref(),
            badges,
        ),
        competitivo: DriverCompetitiveBlock {
            personalidade_primaria: personality_primaria,
            personalidade_secundaria: personalidade_secundaria,
            motivacao: driver.motivacao.round().clamp(0.0, 100.0) as u8,
            qualidades,
            defeitos,
            neutro: tags.is_empty() && !driver.is_jogador,
        },
        leitura_tecnica: build_driver_technical_read_block(driver),
        performance: build_driver_performance_block(driver, &recent_results),
        forma: build_driver_form_block(&recent_results, form_context.as_deref()),
        resumo_atual: build_current_summary_block(driver, &recent_results, championship_position),
        leitura_desempenho: build_performance_read_block(
            driver,
            team,
            teammate.as_ref(),
            championship_position,
        ),
        trajetoria: build_driver_career_path_block(
            conn,
            driver,
            team,
            contract,
            category_id.as_deref(),
            season.ano,
        )?,
        rankings_carreira: build_career_rank_block(conn, driver)?,
        rivais: build_driver_rivals_block(conn, driver)?,
        contrato_mercado: DriverContractMarketBlock {
            contrato: contract_detail,
            mercado: Some(build_driver_market_block(
                driver,
                contract,
                team,
                season.numero,
            )),
        },
        relacionamentos: None,
        reputacao: None,
        saude: None,
    })
}

fn convert_tags(tags: &[AttributeTag]) -> Vec<TagInfo> {
    tags.iter()
        .map(|tag| TagInfo {
            attribute_name: tag.attribute_name.to_string(),
            tag_text: tag.tag_text.to_string(),
            level: match tag.level {
                TagLevel::DefeitoGrave => "defeito_grave".to_string(),
                TagLevel::Defeito => "defeito".to_string(),
                TagLevel::Qualidade => "qualidade".to_string(),
                TagLevel::QualidadeAlta => "qualidade_alta".to_string(),
                TagLevel::Elite => "elite".to_string(),
            },
            color: match tag.level {
                TagLevel::DefeitoGrave => "#f85149".to_string(),
                TagLevel::Defeito => "#db6d28".to_string(),
                TagLevel::Qualidade => "#3fb950".to_string(),
                TagLevel::QualidadeAlta => "#58a6ff".to_string(),
                TagLevel::Elite => "#bc8cff".to_string(),
            },
        })
        .collect()
}

fn convert_primary_personality(personality: &PrimaryPersonality) -> PersonalityInfo {
    match personality {
        PrimaryPersonality::Ambicioso => PersonalityInfo {
            tipo: "Ambicioso".to_string(),
            emoji: "\u{1F3C6}".to_string(),
            descricao: "Quer subir de categoria sempre".to_string(),
        },
        PrimaryPersonality::Consolidador => PersonalityInfo {
            tipo: "Consolidador".to_string(),
            emoji: "\u{1F3E0}".to_string(),
            descricao: "Prefere ser o melhor onde esta".to_string(),
        },
        PrimaryPersonality::Mercenario => PersonalityInfo {
            tipo: "Mercenario".to_string(),
            emoji: "\u{1F4B0}".to_string(),
            descricao: "Vai onde pagam mais".to_string(),
        },
        PrimaryPersonality::Leal => PersonalityInfo {
            tipo: "Leal".to_string(),
            emoji: "\u{2764}\u{FE0F}".to_string(),
            descricao: "Prefere ficar na equipe atual".to_string(),
        },
    }
}

fn convert_secondary_personality(personality: &SecondaryPersonality) -> PersonalityInfo {
    match personality {
        SecondaryPersonality::CabecaQuente => PersonalityInfo {
            tipo: "Cabeca Quente".to_string(),
            emoji: "\u{1F525}".to_string(),
            descricao: "Esquenta quando perde posicoes".to_string(),
        },
        SecondaryPersonality::SangueFrio => PersonalityInfo {
            tipo: "Sangue Frio".to_string(),
            emoji: "\u{1F9CA}".to_string(),
            descricao: "Mantem calma sob pressao".to_string(),
        },
        SecondaryPersonality::Apostador => PersonalityInfo {
            tipo: "Apostador".to_string(),
            emoji: "\u{1F3B0}".to_string(),
            descricao: "Faz manobras arriscadas".to_string(),
        },
        SecondaryPersonality::Calculista => PersonalityInfo {
            tipo: "Calculista".to_string(),
            emoji: "\u{1F6E1}\u{FE0F}".to_string(),
            descricao: "Prefere consistencia a brilhantismo".to_string(),
        },
        SecondaryPersonality::Showman => PersonalityInfo {
            tipo: "Showman".to_string(),
            emoji: "\u{1F451}".to_string(),
            descricao: "Vive para o espetaculo".to_string(),
        },
        SecondaryPersonality::TeamPlayer => PersonalityInfo {
            tipo: "Team Player".to_string(),
            emoji: "\u{1F91D}".to_string(),
            descricao: "Time em primeiro".to_string(),
        },
        SecondaryPersonality::Solitario => PersonalityInfo {
            tipo: "Solitario".to_string(),
            emoji: "\u{1F624}".to_string(),
            descricao: "Corre por si mesmo".to_string(),
        },
        SecondaryPersonality::Estudioso => PersonalityInfo {
            tipo: "Estudioso".to_string(),
            emoji: "\u{1F4DA}".to_string(),
            descricao: "Sempre quer melhorar".to_string(),
        },
    }
}

fn driver_detail_status(driver: &Driver, has_active_contract: bool) -> String {
    match driver.status {
        DriverStatus::Ativo => {
            if has_active_contract {
                "ativo".to_string()
            } else {
                "livre".to_string()
            }
        }
        DriverStatus::Lesionado => "lesionado".to_string(),
        DriverStatus::Aposentado => "aposentado".to_string(),
        DriverStatus::Suspenso => "suspenso".to_string(),
    }
}

fn build_season_stats_block(driver: &Driver) -> StatsBlock {
    StatsBlock {
        corridas: driver.stats_temporada.corridas as i32,
        pontos: driver.stats_temporada.pontos.round() as i32,
        vitorias: driver.stats_temporada.vitorias as i32,
        podios: driver.stats_temporada.podios as i32,
        poles: driver.stats_temporada.poles as i32,
        melhor_resultado: driver.melhor_resultado_temp.unwrap_or(0) as i32,
        dnfs: driver.stats_temporada.dnfs as i32,
    }
}

fn build_career_stats_block(driver: &Driver) -> StatsBlock {
    StatsBlock {
        corridas: driver.stats_carreira.corridas as i32,
        pontos: driver.stats_carreira.pontos_total.round() as i32,
        vitorias: driver.stats_carreira.vitorias as i32,
        podios: driver.stats_carreira.podios as i32,
        poles: driver.stats_carreira.poles as i32,
        melhor_resultado: 0,
        dnfs: driver.stats_carreira.dnfs as i32,
    }
}

fn build_contract_detail(
    contract: &Contract,
    current_season: i32,
    current_year: i32,
) -> ContractDetail {
    let base_year = current_year - current_season + 1;

    ContractDetail {
        equipe_nome: contract.equipe_nome.clone(),
        papel: match contract.papel.as_str() {
            "Numero1" => "N1".to_string(),
            _ => "N2".to_string(),
        },
        salario_anual: contract.salario_anual,
        temporada_inicio: contract.temporada_inicio,
        temporada_fim: contract.temporada_fim,
        ano_inicio: base_year + contract.temporada_inicio - 1,
        ano_fim: base_year + contract.temporada_fim - 1,
        anos_restantes: contract.anos_restantes(current_season),
        status: contract.status.as_str().to_string(),
    }
}

fn resolve_driver_category(
    driver: &Driver,
    contract: Option<&Contract>,
    team: Option<&Team>,
) -> Option<String> {
    driver
        .categoria_atual
        .clone()
        .or_else(|| contract.map(|value| value.categoria.clone()))
        .or_else(|| team.map(|value| value.categoria.clone()))
}

fn split_driver_tags(tags: &[TagInfo]) -> (Vec<TagInfo>, Vec<TagInfo>) {
    let mut qualidades = Vec::new();
    let mut defeitos = Vec::new();

    for tag in tags {
        if matches!(tag.level.as_str(), "qualidade" | "qualidade_alta" | "elite") {
            qualidades.push(tag.clone());
        } else if matches!(tag.level.as_str(), "defeito" | "defeito_grave") {
            defeitos.push(tag.clone());
        }
    }

    (qualidades, defeitos)
}

fn build_driver_technical_read_block(driver: &Driver) -> DriverTechnicalReadBlock {
    let resistencia = driver.atributos.fitness * 0.65 + driver.atributos.gestao_pneus * 0.35;

    DriverTechnicalReadBlock {
        itens: vec![
            build_technical_read_item("velocidade", "Velocidade", driver.atributos.skill),
            build_technical_read_item(
                "consistencia",
                "Consistencia",
                driver.atributos.consistencia,
            ),
            build_technical_read_item("racecraft", "Racecraft", driver.atributos.racecraft),
            build_technical_read_item("resistencia", "Resistencia", resistencia),
        ],
    }
}

fn build_technical_read_item(chave: &str, label: &str, value: f64) -> DriverTechnicalReadItem {
    let (nivel, tom) = technical_level_for_value(value);

    DriverTechnicalReadItem {
        chave: chave.to_string(),
        label: label.to_string(),
        nivel: nivel.to_string(),
        tom: tom.to_string(),
    }
}

fn technical_level_for_value(value: f64) -> (&'static str, &'static str) {
    let value = value.clamp(0.0, 100.0);
    if value < 12.5 {
        ("Muito fraco", "danger")
    } else if value < 25.0 {
        ("Fraco", "danger")
    } else if value < 37.5 {
        ("Abaixo do esperado", "warning")
    } else if value < 50.0 {
        ("Instavel", "warning")
    } else if value < 62.5 {
        ("Competente", "neutral")
    } else if value < 75.0 {
        ("Forte", "info")
    } else if value < 87.5 {
        ("Muito forte", "success")
    } else {
        ("Elite", "elite")
    }
}

fn build_driver_market_block(
    driver: &Driver,
    contract: Option<&Contract>,
    team: Option<&Team>,
    current_season: i32,
) -> DriverMarketBlock {
    let category_id = resolve_driver_category(driver, contract, team);
    let base_salary = salary_baseline_for_category(category_id.as_deref());
    let skill_factor = 0.72 + driver.atributos.skill.clamp(0.0, 100.0) / 68.0;
    let career_factor = 1.0
        + driver.stats_carreira.titulos as f64 * 0.16
        + driver.stats_carreira.vitorias as f64 * 0.018
        + driver.stats_carreira.podios as f64 * 0.008;
    let media_factor = 0.9 + driver.atributos.midia.clamp(0.0, 100.0) / 500.0;
    let salario_estimado = contract
        .map(|value| value.salario_anual)
        .unwrap_or(base_salary * skill_factor * career_factor)
        .max(5_000.0)
        .round();
    let value_multiplier = 2.2 + driver.atributos.desenvolvimento.clamp(0.0, 100.0) / 70.0;
    let valor_mercado = (salario_estimado * value_multiplier * media_factor).round();
    let chance_transferencia = transfer_chance_for_driver(driver, contract, current_season);

    DriverMarketBlock {
        valor_mercado: Some(valor_mercado),
        salario_estimado: Some(salario_estimado),
        chance_transferencia: Some(chance_transferencia),
    }
}

fn salary_baseline_for_category(category_id: Option<&str>) -> f64 {
    match category_id
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
    {
        Some(0) => 10_000.0,
        Some(1) => 27_500.0,
        Some(2) => 55_000.0,
        Some(3) => 105_000.0,
        Some(4) => 200_000.0,
        Some(5) => 165_000.0,
        _ => 20_000.0,
    }
}

fn transfer_chance_for_driver(
    driver: &Driver,
    contract: Option<&Contract>,
    current_season: i32,
) -> u8 {
    let Some(contract) = contract else {
        return 100;
    };

    let remaining = contract.anos_restantes(current_season);
    let contract_pressure = if remaining <= 0 {
        54.0
    } else if remaining == 1 {
        34.0
    } else {
        14.0
    };
    let motivation_pressure = (70.0 - driver.motivacao).max(0.0) * 0.45;
    let market_pull = (driver.atributos.skill - 60.0).max(0.0) * 0.28;

    (contract_pressure + motivation_pressure + market_pull)
        .round()
        .clamp(5.0, 95.0) as u8
}

fn build_driver_profile_block(
    driver: &Driver,
    status: &str,
    team: Option<&Team>,
    role: Option<&str>,
    category_id: Option<&str>,
    badges: Vec<DriverBadge>,
) -> DriverProfileBlock {
    let (bandeira, nacionalidade_label) = split_nationality(&driver.nacionalidade);

    DriverProfileBlock {
        nome: driver.nome.clone(),
        bandeira,
        nacionalidade: nacionalidade_label,
        idade: driver.idade as i32,
        genero: driver.genero.clone(),
        status: status.to_string(),
        is_jogador: driver.is_jogador,
        equipe_nome: team.map(|value| value.nome.clone()),
        papel: role.map(str::to_string),
        licenca: derive_driver_license(category_id, driver),
        badges,
        equipe_cor_primaria: team.map(|value| value.cor_primaria.clone()),
        equipe_cor_secundaria: team.map(|value| value.cor_secundaria.clone()),
    }
}

fn split_nationality(nacionalidade: &str) -> (String, String) {
    let mut parts = nacionalidade.split_whitespace();
    let bandeira = parts.next().unwrap_or("\u{1F3C1}").to_string();
    let label = parts.collect::<Vec<_>>().join(" ");
    (bandeira, label)
}

fn derive_driver_license(category_id: Option<&str>, driver: &Driver) -> DriverLicenseInfo {
    let (nivel, sigla) = match category_id
        .and_then(categories::get_category_config)
        .map(|config| config.tier)
    {
        Some(0) => ("Rookie", "R"),
        Some(1) => ("Amador", "A"),
        Some(2) => ("Pro", "P"),
        Some(3) => ("Super Pro", "SP"),
        Some(_) => ("Elite", "E"),
        None if driver.stats_carreira.titulos > 0 => ("Elite", "E"),
        None if driver.stats_carreira.corridas >= 25 => ("Super Pro", "SP"),
        None if driver.stats_carreira.corridas >= 12 => ("Pro", "P"),
        None if driver.stats_carreira.corridas >= 5 => ("Amador", "A"),
        _ => ("Rookie", "R"),
    };

    DriverLicenseInfo {
        nivel: nivel.to_string(),
        sigla: sigla.to_string(),
    }
}

fn build_driver_badges(driver: &Driver, category_id: Option<&str>) -> Vec<DriverBadge> {
    let mut badges = Vec::new();

    if driver.is_jogador {
        badges.push(DriverBadge {
            label: "VOCE".to_string(),
            variant: "player".to_string(),
        });
    }

    if category_id
        .and_then(categories::get_category_config)
        .is_some_and(|config| config.tier == 0)
        || driver.corridas_na_categoria < 5
    {
        badges.push(DriverBadge {
            label: "ROOKIE".to_string(),
            variant: "info".to_string(),
        });
    }

    if driver.stats_carreira.titulos > 0 {
        badges.push(DriverBadge {
            label: "CAMPEAO".to_string(),
            variant: "warning".to_string(),
        });
    }

    badges
}

fn build_recent_results_for_driver(
    conn: &Connection,
    career_dir: &Path,
    season_id: &str,
    category: &str,
    driver_id: &str,
) -> Result<Vec<HistoricalRaceResult>, String> {
    let total_rounds = count_calendar_entries(conn, season_id, category)
        .map_err(|e| format!("Falha ao contar corridas da categoria: {e}"))?
        as usize;

    if total_rounds == 0 {
        return Ok(Vec::new());
    }

    let histories =
        build_driver_histories(career_dir, category, total_rounds, &[driver_id.to_string()])?;

    Ok(histories
        .into_iter()
        .next()
        .map(|history| {
            history
                .results
                .into_iter()
                .enumerate()
                .filter_map(|(index, result)| {
                    result.map(|value| HistoricalRaceResult {
                        rodada: index as i32 + 1,
                        position: value.position,
                        is_dnf: value.is_dnf,
                        has_fastest_lap: value.has_fastest_lap,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

fn build_archived_recent_results_for_driver(
    conn: &Connection,
    current_season_number: i32,
    driver_id: &str,
) -> Result<ArchivedRecentResults, String> {
    let archive_row: Option<(String, String)> = conn
        .query_row(
            "SELECT categoria, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1 AND season_number < ?2
             ORDER BY season_number DESC
             LIMIT 1",
            rusqlite::params![driver_id, current_season_number],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(|e| format!("Falha ao buscar forma historica do piloto '{driver_id}': {e}"))?;

    let Some((archive_category, snapshot_json)) = archive_row else {
        return Ok(ArchivedRecentResults {
            results: Vec::new(),
            form_context: None,
        });
    };
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot_json).map_err(|e| {
        format!("Falha ao interpretar forma historica do piloto '{driver_id}': {e}")
    })?;
    let result_values = snapshot
        .get("ultimos_resultados")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let form_context =
        archived_form_context_for_empty_results(&archive_category, &snapshot, &result_values);

    let results = result_values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let position = value
                .get("position")
                .or_else(|| value.get("chegada"))
                .and_then(serde_json::Value::as_i64)? as i32;
            let is_dnf = value
                .get("is_dnf")
                .or_else(|| value.get("dnf"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(HistoricalRaceResult {
                rodada: index as i32 + 1,
                position,
                is_dnf,
                has_fastest_lap: false,
            })
        })
        .collect();

    Ok(ArchivedRecentResults {
        results,
        form_context,
    })
}

fn archived_form_context_for_empty_results(
    archive_category: &str,
    snapshot: &serde_json::Value,
    result_values: &[serde_json::Value],
) -> Option<String> {
    if !result_values.is_empty() {
        return None;
    }

    let races = snapshot
        .get("corridas")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    if races > 0 {
        return None;
    }

    let snapshot_category = snapshot
        .get("categoria")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(archive_category);

    if archive_category.trim().is_empty() && snapshot_category.trim().is_empty() {
        Some("sem_time_temporada_passada".to_string())
    } else {
        Some("sem_corridas_temporada_passada".to_string())
    }
}

fn build_driver_performance_block(
    driver: &Driver,
    results: &[HistoricalRaceResult],
) -> DriverPerformanceBlock {
    let top_10 = results
        .iter()
        .filter(|result| !result.is_dnf && result.position <= 10)
        .count() as i32;
    let fastest_laps = results
        .iter()
        .filter(|result| result.has_fastest_lap)
        .count() as i32;
    let fora_top_10 = results
        .iter()
        .filter(|result| !result.is_dnf && result.position > 10)
        .count() as i32;
    let can_reuse_season_derivations = driver.stats_carreira.temporadas <= 1
        || driver.stats_carreira.corridas == driver.stats_temporada.corridas;

    DriverPerformanceBlock {
        temporada: PerformanceStatsBlock {
            vitorias: driver.stats_temporada.vitorias as i32,
            podios: driver.stats_temporada.podios as i32,
            top_10: Some(top_10),
            fora_top_10: Some(fora_top_10),
            poles: driver.stats_temporada.poles as i32,
            voltas_rapidas: Some(fastest_laps),
            hat_tricks: None,
            corridas: driver.stats_temporada.corridas as i32,
            dnfs: driver.stats_temporada.dnfs as i32,
        },
        carreira: PerformanceStatsBlock {
            vitorias: driver.stats_carreira.vitorias as i32,
            podios: driver.stats_carreira.podios as i32,
            top_10: can_reuse_season_derivations.then_some(top_10),
            fora_top_10: can_reuse_season_derivations.then_some(fora_top_10),
            poles: driver.stats_carreira.poles as i32,
            voltas_rapidas: can_reuse_season_derivations.then_some(fastest_laps),
            hat_tricks: None,
            corridas: driver.stats_carreira.corridas as i32,
            dnfs: driver.stats_carreira.dnfs as i32,
        },
    }
}

fn build_driver_form_block(
    results: &[HistoricalRaceResult],
    form_context: Option<&str>,
) -> DriverFormBlock {
    let recent_form_source: Vec<HistoricalRaceResult> = results
        .iter()
        .rev()
        .take(10)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let legacy_recent_source: Vec<HistoricalRaceResult> = recent_form_source
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let media_chegada = average_finish(&recent_form_source);
    let tendencia = calculate_form_trend(&recent_form_source);
    let momento = match media_chegada {
        Some(value) if value <= 5.0 => "forte".to_string(),
        Some(value) if value <= 10.0 => "estavel".to_string(),
        Some(_) => "em_baixa".to_string(),
        None => "sem_dados".to_string(),
    };

    DriverFormBlock {
        ultimas_10: recent_form_source
            .into_iter()
            .map(|result| FormResultEntry {
                rodada: result.rodada,
                chegada: (!result.is_dnf).then_some(result.position),
                dnf: result.is_dnf,
            })
            .collect(),
        ultimas_5: legacy_recent_source
            .into_iter()
            .map(|result| FormResultEntry {
                rodada: result.rodada,
                chegada: (!result.is_dnf).then_some(result.position),
                dnf: result.is_dnf,
            })
            .collect(),
        media_chegada,
        tendencia,
        momento,
        contexto: form_context.map(str::to_string),
    }
}

fn build_current_summary_block(
    driver: &Driver,
    results: &[HistoricalRaceResult],
    championship_position: Option<i32>,
) -> DriverCurrentSummaryBlock {
    if driver.stats_carreira.corridas == 0 && results.is_empty() {
        return DriverCurrentSummaryBlock {
            veredito: "Estreante".to_string(),
            tom: "info".to_string(),
            posicao_campeonato: championship_position,
            pontos: driver.stats_temporada.pontos.round() as i32,
            vitorias: driver.stats_temporada.vitorias as i32,
            podios: driver.stats_temporada.podios as i32,
            top_10: Some(0),
            media_recente: None,
            tendencia: "desconhecida".to_string(),
        };
    }

    let form = build_driver_form_block(results, None);
    let top_10 = results
        .iter()
        .filter(|result| !result.is_dnf && result.position <= 10)
        .count() as i32;
    let verdict_score = driver.stats_temporada.vitorias as i32 * 14
        + driver.stats_temporada.podios as i32 * 5
        + top_10 * 2
        + championship_position
            .map(|position| (18 - position).max(0))
            .unwrap_or(0);
    let result_count = results.len();
    let dnf_count = results.iter().filter(|result| result.is_dnf).count();
    let dnf_rate = if result_count > 0 {
        dnf_count as f64 / result_count as f64
    } else {
        0.0
    };
    let average_recent = form.media_chegada;
    let has_enough_evidence = result_count >= 3;
    let is_bad_average = average_recent.is_some_and(|average| average > 10.0);
    let is_critical_average = average_recent.is_some_and(|average| average >= 16.0);
    let is_low_in_championship = championship_position.is_some_and(|position| position >= 15);
    let is_very_low_in_championship = championship_position.is_some_and(|position| position >= 20);
    let (veredito, tom) = if verdict_score >= 45 {
        ("Excelente", "success")
    } else if verdict_score >= 24 {
        ("Bom", "success")
    } else if verdict_score >= 10 {
        ("Regular", "warning")
    } else if has_enough_evidence
        && (is_critical_average
            || dnf_rate >= 0.4
            || (is_very_low_in_championship && is_bad_average))
    {
        ("Crítico", "danger")
    } else if has_enough_evidence && (is_bad_average || is_low_in_championship) {
        ("Ruim", "danger")
    } else {
        ("Avaliação", "info")
    };

    DriverCurrentSummaryBlock {
        veredito: veredito.to_string(),
        tom: tom.to_string(),
        posicao_campeonato: championship_position,
        pontos: driver.stats_temporada.pontos.round() as i32,
        vitorias: driver.stats_temporada.vitorias as i32,
        podios: driver.stats_temporada.podios as i32,
        top_10: Some(top_10),
        media_recente: form.media_chegada,
        tendencia: form.tendencia,
    }
}

fn build_performance_read_block(
    driver: &Driver,
    team: Option<&Team>,
    teammate: Option<&Driver>,
    championship_position: Option<i32>,
) -> DriverPerformanceReadBlock {
    let expected = team.and_then(expected_position_for_team);
    let delta = match (expected, championship_position) {
        (Some(expected_position), Some(position)) => Some(expected_position - position),
        _ => None,
    };
    let teammate_points = teammate.map(|value| value.stats_temporada.pontos.round() as i32);
    let reading = match delta {
        Some(value) if value >= 3 => "Entrega acima do pacote atual.",
        Some(value) if value <= -3 => "Entrega abaixo do esperado para o pacote.",
        Some(_) => "Entrega dentro do esperado para o pacote.",
        None => "Sem contexto suficiente para comparar com o pacote.",
    };

    DriverPerformanceReadBlock {
        esperado_posicao: expected,
        entregue_posicao: championship_position,
        delta_posicao: delta,
        car_performance: team.map(|value| value.car_performance),
        companheiro_nome: teammate.map(|value| value.nome.clone()),
        companheiro_pontos: teammate_points,
        piloto_pontos: driver.stats_temporada.pontos.round() as i32,
        leitura: reading.to_string(),
    }
}

fn expected_position_for_team(team: &Team) -> Option<i32> {
    let perf = team.car_performance;
    Some(if perf >= 13.0 {
        2
    } else if perf >= 10.0 {
        4
    } else if perf >= 7.0 {
        7
    } else if perf >= 4.0 {
        10
    } else {
        14
    })
}

fn build_career_rank_block(
    conn: &Connection,
    driver: &Driver,
) -> Result<DriverCareerRankBlock, String> {
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para rankings de carreira: {e}"))?;

    Ok(DriverCareerRankBlock {
        corridas: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.corridas),
        vitorias: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.vitorias),
        podios: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.podios),
        titulos: rank_driver_by(&drivers, &driver.id, |value| value.stats_carreira.titulos),
    })
}

fn rank_driver_by<F>(drivers: &[Driver], driver_id: &str, metric: F) -> Option<i32>
where
    F: Fn(&Driver) -> u32,
{
    let mut ranked: Vec<(&str, u32)> = drivers
        .iter()
        .map(|driver| (driver.id.as_str(), metric(driver)))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked
        .iter()
        .position(|(id, _)| *id == driver_id)
        .map(|index| index as i32 + 1)
}

fn build_driver_rivals_block(
    conn: &Connection,
    driver: &Driver,
) -> Result<DriverRivalsBlock, String> {
    let rivalries = crate::rivalry::get_pilot_rivalries(conn, &driver.id)
        .map_err(|e| format!("Falha ao carregar rivalidades do piloto: {e}"))?;
    let mut itens = Vec::new();

    for rivalry in rivalries.into_iter().take(4) {
        let rival_name = driver_queries::get_driver(conn, &rivalry.rival_id)
            .map(|rival| rival.nome)
            .unwrap_or_else(|_| rivalry.rival_id.clone());
        itens.push(DriverRivalInfo {
            driver_id: rivalry.rival_id,
            nome: rival_name,
            tipo: rivalry.tipo.as_str().to_string(),
            intensidade: rivalry.perceived_intensity.round().clamp(0.0, 100.0) as u8,
            intensidade_historica: rivalry.historical_intensity.round().clamp(0.0, 100.0) as u8,
            atividade_recente: rivalry.recent_activity.round().clamp(0.0, 100.0) as u8,
        });
    }

    Ok(DriverRivalsBlock { itens })
}

fn find_championship_position(
    conn: &Connection,
    category: &str,
    driver_id: &str,
) -> Result<Option<i32>, String> {
    let mut drivers = driver_queries::get_drivers_by_category(conn, category)
        .map_err(|e| format!("Falha ao carregar classificacao da categoria: {e}"))?;
    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .partial_cmp(&a.stats_temporada.pontos)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
            .then_with(|| b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    Ok(drivers
        .iter()
        .position(|driver| driver.id == driver_id)
        .map(|index| index as i32 + 1))
}

fn find_teammate(
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
) -> Result<Option<Driver>, String> {
    let Some(team) = team else {
        return Ok(None);
    };
    let teammate_id = [team.piloto_1_id.as_ref(), team.piloto_2_id.as_ref()]
        .into_iter()
        .flatten()
        .find(|id| id.as_str() != driver.id);
    let Some(teammate_id) = teammate_id else {
        return Ok(None);
    };

    driver_queries::get_driver(conn, teammate_id)
        .map(Some)
        .map_err(|e| format!("Falha ao carregar companheiro de equipe: {e}"))
}

fn average_finish(results: &[HistoricalRaceResult]) -> Option<f64> {
    let finishes: Vec<i32> = results
        .iter()
        .filter(|result| !result.is_dnf)
        .map(|result| result.position)
        .collect();

    if finishes.is_empty() {
        return None;
    }

    let total: i32 = finishes.iter().sum();
    Some(total as f64 / finishes.len() as f64)
}

fn calculate_form_trend(results: &[HistoricalRaceResult]) -> String {
    if results.len() < 3 {
        return "\u{2192}".to_string();
    }

    let split_index = results.len() / 2;
    let previous = average_finish(&results[..split_index]);
    let recent = average_finish(&results[split_index..]);

    match (previous, recent) {
        (Some(previous), Some(recent)) if recent + 0.25 < previous => "\u{2197}".to_string(),
        (Some(previous), Some(recent)) if recent > previous + 0.25 => "\u{2198}".to_string(),
        _ => "\u{2192}".to_string(),
    }
}

fn build_career_history_block(
    conn: &Connection,
    driver_id: &str,
) -> Result<DriverCareerHistoryBlock, String> {
    let seasons = load_career_season_archive_rows(conn, driver_id)?;
    let races = load_career_race_history_rows(conn, driver_id)?;

    let active_seasons: Vec<&CareerSeasonArchiveRow> = seasons
        .iter()
        .filter(|season| season.corridas > 0)
        .collect();
    let mut categories = HashSet::new();
    for season in &active_seasons {
        if !season.categoria.trim().is_empty() {
            categories.insert(season.categoria.clone());
        }
    }

    let presenca = DriverCareerPresenceBlock {
        tempo_carreira: career_duration_from_archive(&seasons),
        temporadas_disputadas: active_seasons.len() as i32,
        anos_desempregado: seasons
            .iter()
            .filter(|season| season.corridas == 0 && season.categoria.trim().is_empty())
            .count() as i32,
        periodos_desempregado: unemployment_periods(&seasons),
        corridas: active_seasons
            .iter()
            .map(|season| season.corridas)
            .sum::<i32>(),
        categorias_disputadas: categories.len() as i32,
    };

    let primeiros_marcos = DriverCareerFirstMarksBlock {
        primeiro_podio_corrida: races
            .iter()
            .find(|race| !race.is_dnf && race.position <= 3)
            .map(|race| race.race_index),
        primeira_vitoria_corrida: races
            .iter()
            .find(|race| !race.is_dnf && race.position == 1)
            .map(|race| race.race_index),
        primeiro_dnf_corrida: races
            .iter()
            .find(|race| race.is_dnf)
            .map(|race| race.race_index),
    };

    let auge = DriverCareerPeakBlock {
        melhor_temporada: best_career_season(&active_seasons),
        maior_sequencia_vitorias: longest_win_streak(&races),
    };

    let mobility_counts = count_category_mobility(&active_seasons);
    let team_summary = summarize_team_mobility(&races);
    let mobilidade = DriverCareerMobilityBlock {
        promocoes: mobility_counts.0,
        rebaixamentos: mobility_counts.1,
        equipes_defendidas: team_summary.0,
        tempo_medio_por_equipe: team_summary.1,
    };
    let eventos_especiais = build_special_events_block(conn, driver_id)?;

    Ok(DriverCareerHistoryBlock {
        presenca,
        primeiros_marcos,
        auge,
        mobilidade,
        eventos_especiais,
    })
}

fn build_special_events_block(
    conn: &Connection,
    driver_id: &str,
) -> Result<DriverCareerSpecialEventsBlock, String> {
    if !sqlite_table_exists(conn, "contracts")? {
        return Ok(DriverCareerSpecialEventsBlock::default());
    }

    let contracts = load_special_contract_rows(conn, driver_id)?;
    if contracts.is_empty() {
        return Ok(DriverCareerSpecialEventsBlock::default());
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

    Ok(DriverCareerSpecialEventsBlock {
        participacoes: contracts.len() as i32,
        convocacoes: contracts.len() as i32,
        vitorias,
        podios,
        rankings,
        melhor_campanha,
        ultimo_evento,
        timeline,
    })
}

fn sqlite_table_exists(conn: &Connection, table_name: &str) -> Result<bool, String> {
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

fn load_special_contract_rows(
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
        if categories::is_especial(&contract.category) {
            rows.push(contract);
        }
    }
    Ok(rows)
}

fn build_special_event_rank_block(
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

fn load_special_contract_counts(conn: &Connection) -> Result<Vec<(String, i32)>, String> {
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
        if categories::is_especial(&category) {
            *counts.entry(pilot_id).or_insert(0) += 1;
        }
    }

    Ok(counts.into_iter().collect())
}

fn load_special_result_counts(conn: &Connection) -> Result<HashMap<String, (i32, i32)>, String> {
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
        if categories::is_especial(&category) {
            let entry = counts.entry(pilot_id).or_insert((0, 0));
            entry.0 += wins;
            entry.1 += podiums;
        }
    }

    Ok(counts)
}

fn rank_special_event_metric(rows: &[(String, i32)], driver_id: &str) -> Option<i32> {
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

fn load_special_campaign_aggregates(
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

fn unemployment_periods(seasons: &[CareerSeasonArchiveRow]) -> Vec<String> {
    let mut periods = Vec::new();
    let mut current_start: Option<i32> = None;
    let mut current_end: Option<i32> = None;

    for season in seasons {
        let unemployed = season.corridas == 0 && season.categoria.trim().is_empty();
        if unemployed {
            match current_end {
                Some(end) if season.ano == end + 1 => current_end = Some(season.ano),
                Some(end) => {
                    periods.push(format_year_period(current_start.unwrap_or(end), end));
                    current_start = Some(season.ano);
                    current_end = Some(season.ano);
                }
                None => {
                    current_start = Some(season.ano);
                    current_end = Some(season.ano);
                }
            }
        } else if let Some(end) = current_end {
            periods.push(format_year_period(current_start.unwrap_or(end), end));
            current_start = None;
            current_end = None;
        }
    }

    if let Some(end) = current_end {
        periods.push(format_year_period(current_start.unwrap_or(end), end));
    }

    periods
}

fn career_duration_from_archive(seasons: &[CareerSeasonArchiveRow]) -> i32 {
    let Some(first_year) = seasons.iter().map(|season| season.ano).min() else {
        return 0;
    };
    let Some(last_year) = seasons.iter().map(|season| season.ano).max() else {
        return 0;
    };

    (last_year - first_year + 1).max(0)
}

fn format_year_period(start: i32, end: i32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}->{end}")
    }
}

fn load_career_season_archive_rows(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<CareerSeasonArchiveRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT season_number, ano, categoria, posicao_campeonato, pontos, snapshot_json
             FROM driver_season_archive
             WHERE piloto_id = ?1
             ORDER BY season_number ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico de temporadas do piloto: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            let snapshot_json: String = row.get(5)?;
            let snapshot: serde_json::Value =
                serde_json::from_str(&snapshot_json).unwrap_or_default();
            let categoria: String = row.get(2)?;
            Ok(CareerSeasonArchiveRow {
                ano: row.get(1)?,
                categoria: snapshot
                    .get("categoria")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(categoria.as_str())
                    .to_string(),
                posicao_campeonato: row.get(3)?,
                pontos: snapshot
                    .get("pontos")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(row.get(4)?),
                corridas: snapshot
                    .get("corridas")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0) as i32,
                vitorias: snapshot
                    .get("vitorias")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0) as i32,
                podios: snapshot
                    .get("podios")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0) as i32,
            })
        })
        .map_err(|e| format!("Falha ao consultar historico de temporadas do piloto: {e}"))?;

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row.map_err(|e| format!("Falha ao ler historico de temporada: {e}"))?);
    }
    Ok(rows)
}

fn load_career_race_history_rows(
    conn: &Connection,
    driver_id: &str,
) -> Result<Vec<CareerRaceHistoryRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                COALESCE(s.numero, 0) AS season_number,
                COALESCE(NULLIF(r.equipe_id, ''), '-') AS equipe_id,
                r.posicao_final,
                r.dnf
             FROM race_results r
             INNER JOIN calendar c ON c.id = r.race_id
             LEFT JOIN seasons s ON s.id = COALESCE(c.season_id, c.temporada_id)
             WHERE r.piloto_id = ?1
             ORDER BY COALESCE(s.numero, 0) ASC, c.rodada ASC, r.id ASC",
        )
        .map_err(|e| format!("Falha ao preparar historico corrida-a-corrida: {e}"))?;
    let mapped = stmt
        .query_map(rusqlite::params![driver_id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)? != 0,
            ))
        })
        .map_err(|e| format!("Falha ao consultar historico corrida-a-corrida: {e}"))?;

    let mut rows = Vec::new();
    for (index, row) in mapped.enumerate() {
        let (season_number, team_id, position, is_dnf) =
            row.map_err(|e| format!("Falha ao ler historico corrida-a-corrida: {e}"))?;
        rows.push(CareerRaceHistoryRow {
            race_index: index as i32 + 1,
            season_number,
            team_id,
            position,
            is_dnf,
        });
    }
    Ok(rows)
}

fn best_career_season(seasons: &[&CareerSeasonArchiveRow]) -> Option<DriverBestSeasonBlock> {
    seasons
        .iter()
        .copied()
        .max_by(|a, b| {
            best_season_score(a)
                .cmp(&best_season_score(b))
                .then_with(|| a.pontos.total_cmp(&b.pontos))
                .then_with(|| a.vitorias.cmp(&b.vitorias))
                .then_with(|| a.podios.cmp(&b.podios))
        })
        .map(|season| DriverBestSeasonBlock {
            ano: season.ano,
            categoria: season.categoria.clone(),
            posicao_campeonato: season.posicao_campeonato,
            pontos: season.pontos.round() as i32,
            vitorias: season.vitorias,
            podios: season.podios,
        })
}

fn best_season_score(season: &CareerSeasonArchiveRow) -> i32 {
    let position_score = season
        .posicao_campeonato
        .map(|position| (50 - position).max(0) * 100)
        .unwrap_or(0);
    position_score + season.vitorias * 15 + season.podios * 5 + season.pontos.round() as i32
}

fn longest_win_streak(races: &[CareerRaceHistoryRow]) -> i32 {
    let mut current = 0;
    let mut best = 0;
    for race in races {
        if !race.is_dnf && race.position == 1 {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}

fn count_category_mobility(seasons: &[&CareerSeasonArchiveRow]) -> (i32, i32) {
    let mut promocoes = 0;
    let mut rebaixamentos = 0;
    let mut previous_tier = None;
    for season in seasons {
        let Some(tier) =
            categories::get_category_config(&season.categoria).map(|config| config.tier)
        else {
            continue;
        };
        if let Some(previous) = previous_tier {
            if tier > previous {
                promocoes += 1;
            } else if tier < previous {
                rebaixamentos += 1;
            }
        }
        previous_tier = Some(tier);
    }
    (promocoes, rebaixamentos)
}

fn summarize_team_mobility(races: &[CareerRaceHistoryRow]) -> (i32, Option<f64>) {
    let mut teams = HashSet::new();
    let mut team_seasons = HashSet::new();
    for race in races {
        if race.team_id == "-" {
            continue;
        }
        teams.insert(race.team_id.clone());
        team_seasons.insert((race.season_number, race.team_id.clone()));
    }
    let team_count = teams.len() as i32;
    let average = if team_count > 0 {
        let raw = team_seasons.len() as f64 / team_count as f64;
        Some((raw * 10.0).round() / 10.0)
    } else {
        None
    };
    (team_count, average)
}

fn build_category_timeline(
    seasons: &[CareerSeasonArchiveRow],
    current_category: Option<&str>,
    current_year: i32,
) -> Vec<DriverCareerCategoryStint> {
    let mut active_seasons: Vec<&CareerSeasonArchiveRow> = seasons
        .iter()
        .filter(|season| season.corridas > 0 && !season.categoria.trim().is_empty())
        .collect();
    active_seasons.sort_by_key(|season| season.ano);

    let mut timeline: Vec<DriverCareerCategoryStint> = Vec::new();
    for season in active_seasons {
        let category = season.categoria.trim();
        if let Some(last) = timeline.last_mut() {
            if last.categoria == category {
                last.ano_fim = season.ano;
                continue;
            }
        }

        timeline.push(DriverCareerCategoryStint {
            categoria: category.to_string(),
            ano_inicio: season.ano,
            ano_fim: season.ano,
        });
    }

    if let Some(category) = current_category.filter(|value| !value.trim().is_empty()) {
        match timeline.last_mut() {
            Some(last) if last.categoria == category => {
                last.ano_fim = last.ano_fim.max(current_year);
            }
            Some(last) if last.ano_inicio == current_year => {
                last.categoria = category.to_string();
                last.ano_fim = current_year;
            }
            _ => timeline.push(DriverCareerCategoryStint {
                categoria: category.to_string(),
                ano_inicio: current_year,
                ano_fim: current_year,
            }),
        }
    }

    timeline
}

fn build_driver_career_path_block(
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
    contract: Option<&Contract>,
    category_id: Option<&str>,
    current_year: i32,
) -> Result<DriverCareerPathBlock, String> {
    let mut marcos = vec![CareerMilestone {
        tipo: "estreia".to_string(),
        titulo: "Estreia".to_string(),
        descricao: format!("Iniciou a carreira em {}", driver.ano_inicio_carreira),
    }];

    if driver.stats_carreira.titulos > 0 {
        marcos.push(CareerMilestone {
            tipo: "titulo".to_string(),
            titulo: "Titulos".to_string(),
            descricao: format!("Ja conquistou {} titulo(s)", driver.stats_carreira.titulos),
        });
    }

    if let Some(category) = category_id.and_then(categories::get_category_config) {
        marcos.push(CareerMilestone {
            tipo: "categoria".to_string(),
            titulo: "Momento atual".to_string(),
            descricao: format!("Compete hoje em {}", category.nome_curto),
        });
    }

    let mut historico = build_career_history_block(conn, &driver.id)?;
    historico.presenca.tempo_carreira =
        (current_year - driver.ano_inicio_carreira as i32 + 1).max(1);
    let season_archive = load_career_season_archive_rows(conn, &driver.id)?;

    Ok(DriverCareerPathBlock {
        ano_estreia: driver.ano_inicio_carreira as i32,
        equipe_estreia: contract
            .filter(|value| value.temporada_inicio <= 1)
            .map(|value| value.equipe_nome.clone())
            .or_else(|| team.map(|value| value.nome.clone())),
        categoria_atual: category_id.map(str::to_string),
        categorias_timeline: build_category_timeline(&season_archive, category_id, current_year),
        temporadas_na_categoria: driver.temporadas_na_categoria as i32,
        corridas_na_categoria: driver.corridas_na_categoria as i32,
        titulos: driver.stats_carreira.titulos as i32,
        foi_campeao: driver.stats_carreira.titulos > 0,
        historico,
        marcos,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_archived_recent_results_for_driver, build_career_history_block,
        build_category_timeline, build_current_summary_block, build_driver_form_block,
        CareerSeasonArchiveRow, HistoricalRaceResult,
    };
    use crate::models::driver::Driver;

    fn sample_driver() -> Driver {
        let mut driver = Driver::new(
            "P001".to_string(),
            "Piloto Teste".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            22,
            2024,
        );
        driver.stats_carreira.corridas = 5;
        driver.stats_temporada.corridas = 5;
        driver
    }

    fn finish(rodada: i32, position: i32) -> HistoricalRaceResult {
        HistoricalRaceResult {
            rodada,
            position,
            is_dnf: false,
            has_fastest_lap: false,
        }
    }

    #[test]
    fn current_summary_uses_avaliacao_instead_of_em_avaliacao() {
        let driver = sample_driver();
        let results = vec![finish(1, 12), finish(2, 13)];

        let summary = build_current_summary_block(&driver, &results, None);

        assert_eq!(summary.veredito, "Avaliação");
        assert_eq!(summary.tom, "info");
    }

    #[test]
    fn current_summary_names_bad_and_critical_seasons() {
        let driver = sample_driver();
        let bad_results = vec![finish(1, 11), finish(2, 12), finish(3, 13)];
        let critical_results = vec![finish(1, 18), finish(2, 19), finish(3, 20)];

        let bad = build_current_summary_block(&driver, &bad_results, Some(16));
        let critical = build_current_summary_block(&driver, &critical_results, Some(22));

        assert_eq!(bad.veredito, "Ruim");
        assert_eq!(bad.tom, "danger");
        assert_eq!(critical.veredito, "Crítico");
        assert_eq!(critical.tom, "danger");
    }

    #[test]
    fn archived_recent_results_marks_previous_season_without_team() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (
                'P001', 25, 2024, 'Piloto Teste', '', NULL, 0.0,
                '{\"corridas\":0,\"categoria\":\"\",\"ultimos_resultados\":[]}'
            );
            ",
        )
        .expect("archive setup");

        let archived =
            build_archived_recent_results_for_driver(&conn, 26, "P001").expect("archive results");

        assert!(archived.results.is_empty());
        assert_eq!(
            archived.form_context.as_deref(),
            Some("sem_time_temporada_passada")
        );
    }

    #[test]
    fn driver_form_block_exposes_previous_season_without_team_context() {
        let form = build_driver_form_block(&[], Some("sem_time_temporada_passada"));

        assert_eq!(form.momento, "sem_dados");
        assert_eq!(form.contexto.as_deref(), Some("sem_time_temporada_passada"));
    }

    #[test]
    fn career_history_block_derives_presence_marks_peak_and_mobility() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0
            );

            INSERT INTO seasons (id, numero, ano) VALUES
                ('S001', 1, 2020),
                ('S002', 2, 2021),
                ('S003', 3, 2022),
                ('S004', 4, 2023),
                ('S005', 5, 2024);

            INSERT INTO driver_season_archive
                (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
            VALUES
                ('P001', 1, 2020, 'Piloto Teste', 'mazda_rookie', 4, 50.0,
                 '{\"corridas\":5,\"vitorias\":0,\"podios\":1,\"pontos\":50,\"categoria\":\"mazda_rookie\"}'),
                ('P001', 2, 2021, 'Piloto Teste', 'mazda_amador', 2, 180.0,
                 '{\"corridas\":8,\"vitorias\":3,\"podios\":5,\"pontos\":180,\"categoria\":\"mazda_amador\"}'),
                ('P001', 3, 2022, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 4, 2023, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 5, 2024, 'Piloto Teste', 'gt4', 5, 90.0,
                 '{\"corridas\":10,\"vitorias\":1,\"podios\":2,\"pontos\":90,\"categoria\":\"gt4\"}'),
                ('P001', 6, 2025, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 7, 2026, 'Piloto Teste', 'bmw_m2', 1, 220.0,
                 '{\"corridas\":8,\"vitorias\":4,\"podios\":6,\"pontos\":220,\"categoria\":\"bmw_m2\"}');
            ",
        )
        .expect("history schema");

        for (season, races) in [("S001", 5), ("S002", 8), ("S004", 10), ("S005", 8)] {
            for rodada in 1..=races {
                conn.execute(
                    "INSERT INTO calendar (id, temporada_id, season_id, rodada, categoria)
                     VALUES (?1, ?2, ?2, ?3, 'mazda_rookie')",
                    rusqlite::params![format!("{season}_R{rodada:02}"), season, rodada],
                )
                .expect("calendar");
            }
        }

        for (race_id, team_id, position, dnf) in [
            ("S001_R01", "T1", 5, 0),
            ("S001_R02", "T1", 4, 0),
            ("S001_R03", "T1", 3, 0),
            ("S001_R04", "T1", 12, 1),
            ("S001_R05", "T1", 4, 0),
            ("S002_R01", "T2", 2, 0),
            ("S002_R02", "T2", 1, 0),
            ("S002_R03", "T2", 1, 0),
            ("S002_R04", "T2", 1, 0),
            ("S002_R05", "T2", 4, 0),
            ("S004_R01", "T3", 9, 0),
            ("S005_R01", "T3", 1, 0),
        ] {
            conn.execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos)
                 VALUES (?1, 'P001', ?2, ?3, ?4, 0.0)",
                rusqlite::params![race_id, team_id, position, dnf],
            )
            .expect("race result");
        }

        let history = build_career_history_block(&conn, "P001").expect("history block");

        assert_eq!(history.presenca.temporadas_disputadas, 4);
        assert_eq!(history.presenca.tempo_carreira, 7);
        assert_eq!(history.presenca.anos_desempregado, 3);
        assert_eq!(
            history.presenca.periodos_desempregado,
            vec!["2022->2023".to_string(), "2025".to_string()]
        );
        assert_eq!(history.presenca.categorias_disputadas, 4);
        assert_eq!(history.primeiros_marcos.primeiro_podio_corrida, Some(3));
        assert_eq!(history.primeiros_marcos.primeira_vitoria_corrida, Some(7));
        assert_eq!(history.primeiros_marcos.primeiro_dnf_corrida, Some(4));
        assert_eq!(history.auge.maior_sequencia_vitorias, 3);
        assert_eq!(
            history.auge.melhor_temporada.as_ref().map(|item| item.ano),
            Some(2026)
        );
        assert_eq!(
            history
                .auge
                .melhor_temporada
                .as_ref()
                .map(|item| item.categoria.as_str()),
            Some("bmw_m2")
        );
        assert_eq!(history.mobilidade.promocoes, 2);
        assert_eq!(history.mobilidade.rebaixamentos, 1);
        assert_eq!(history.mobilidade.equipes_defendidas, 3);
        assert!((history.mobilidade.tempo_medio_por_equipe.unwrap() - 1.3).abs() < 0.05);
    }

    #[test]
    fn category_timeline_compresses_category_stints_and_returns() {
        let seasons = vec![
            season_archive_row(2017, "mazda_rookie", 5),
            season_archive_row(2018, "mazda_rookie", 5),
            season_archive_row(2022, "mazda_amador", 8),
            season_archive_row(2023, "mazda_amador", 8),
            season_archive_row(2024, "", 0),
            season_archive_row(2025, "mazda_rookie", 5),
        ];

        let timeline = build_category_timeline(&seasons, Some("mazda_rookie"), 2025);

        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].categoria, "mazda_rookie");
        assert_eq!(timeline[0].ano_inicio, 2017);
        assert_eq!(timeline[0].ano_fim, 2018);
        assert_eq!(timeline[1].categoria, "mazda_amador");
        assert_eq!(timeline[1].ano_inicio, 2022);
        assert_eq!(timeline[2].categoria, "mazda_rookie");
        assert_eq!(timeline[2].ano_inicio, 2025);
    }

    #[test]
    fn career_history_block_derives_special_event_summary() {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0
            );
            CREATE TABLE contracts (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                piloto_nome TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                equipe_nome TEXT NOT NULL,
                temporada_inicio INTEGER NOT NULL,
                temporada_fim INTEGER NOT NULL,
                duracao_anos INTEGER NOT NULL,
                salario_anual REAL NOT NULL DEFAULT 0.0,
                papel TEXT NOT NULL DEFAULT 'Numero1',
                status TEXT NOT NULL DEFAULT 'Expirado',
                tipo TEXT NOT NULL DEFAULT 'Especial',
                categoria TEXT NOT NULL,
                classe TEXT,
                created_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO seasons (id, numero, ano) VALUES
                ('S006', 6, 2026),
                ('S008', 8, 2028);

            INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                temporada_fim, duracao_anos, tipo, categoria, classe, status
            ) VALUES
                ('CSP1', 'P001', 'Piloto Teste', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP2', 'P001', 'Piloto Teste', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP3', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP4', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP5', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP6', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP7', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP8', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP9', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP10', 'P003', 'Piloto Ranking 3', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP11', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP12', 'P003', 'Piloto Ranking 3', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP13', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP14', 'P004', 'Piloto Ranking 4', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP15', 'P004', 'Piloto Ranking 4', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP16', 'P004', 'Piloto Ranking 4', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP17', 'P004', 'Piloto Ranking 4', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP18', 'P005', 'Piloto Ranking 5', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP19', 'P005', 'Piloto Ranking 5', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP20', 'P005', 'Piloto Ranking 5', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado');

            INSERT INTO calendar (id, temporada_id, season_id, rodada, categoria) VALUES
                ('SP6_R01', 'S006', 'S006', 1, 'production_challenger'),
                ('SP6_R02', 'S006', 'S006', 2, 'production_challenger'),
                ('SP8_R01', 'S008', 'S008', 1, 'endurance'),
                ('SP8_R02', 'S008', 'S008', 2, 'endurance');

            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
                ('SP6_R01', 'P001', 'SP1', 2, 0, 18.0),
                ('SP6_R02', 'P001', 'SP1', 6, 0, 8.0),
                ('SP8_R01', 'P001', 'SP2', 1, 0, 25.0),
                ('SP8_R02', 'P001', 'SP2', 3, 0, 17.0),
                ('SP6_R01', 'P002', 'SP1', 1, 0, 25.0),
                ('SP6_R02', 'P002', 'SP1', 1, 0, 25.0),
                ('SP8_R01', 'P002', 'SP2', 2, 0, 18.0),
                ('SP8_R02', 'P002', 'SP2', 2, 0, 18.0),
                ('SP6_R01', 'P003', 'SP1', 2, 0, 18.0),
                ('SP6_R02', 'P003', 'SP1', 2, 0, 18.0),
                ('SP8_R01', 'P003', 'SP2', 2, 0, 18.0),
                ('SP8_R02', 'P003', 'SP2', 2, 0, 18.0),
                ('SP6_R01', 'P004', 'SP1', 3, 0, 15.0),
                ('SP6_R02', 'P004', 'SP1', 3, 0, 15.0),
                ('SP8_R01', 'P004', 'SP2', 3, 0, 15.0),
                ('SP8_R02', 'P004', 'SP2', 3, 0, 15.0);
            ",
        )
        .expect("special event schema");

        let history = build_career_history_block(&conn, "P001").expect("history block");
        let special = history.eventos_especiais;

        assert_eq!(special.participacoes, 2);
        assert_eq!(special.convocacoes, 2);
        assert_eq!(special.vitorias, 1);
        assert_eq!(special.podios, 3);
        assert_eq!(special.rankings.participacoes, Some(5));
        assert_eq!(special.rankings.convocacoes, Some(5));
        assert_eq!(special.rankings.vitorias, Some(2));
        assert_eq!(special.rankings.podios, Some(4));
        assert_eq!(special.timeline.len(), 2);
        assert_eq!(special.timeline[0].ano, 2026);
        assert_eq!(special.timeline[0].categoria, "production_challenger");
        assert_eq!(special.timeline[0].classe.as_deref(), Some("bmw"));
        assert_eq!(special.timeline[1].ano, 2028);
        assert_eq!(
            special.ultimo_evento.as_ref().map(|item| item.ano),
            Some(2028)
        );
        assert_eq!(
            special
                .melhor_campanha
                .as_ref()
                .map(|campaign| (campaign.ano, campaign.pontos)),
            Some((2028, 42))
        );
    }

    fn season_archive_row(ano: i32, categoria: &str, corridas: i32) -> CareerSeasonArchiveRow {
        CareerSeasonArchiveRow {
            ano,
            categoria: categoria.to_string(),
            posicao_campeonato: None,
            pontos: 0.0,
            corridas,
            vitorias: 0,
            podios: 0,
        }
    }
}
