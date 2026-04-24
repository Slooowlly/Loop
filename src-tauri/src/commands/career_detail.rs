use std::path::Path;

use rusqlite::Connection;

use crate::commands::career::count_calendar_entries;
use crate::commands::career_types::{
    CareerMilestone, ContractDetail, DriverBadge, DriverCareerPathBlock, DriverCompetitiveBlock,
    DriverContractMarketBlock, DriverDetail, DriverFormBlock, DriverLicenseInfo,
    DriverPerformanceBlock, DriverProfileBlock, FormResultEntry, PerformanceStatsBlock,
    PersonalityInfo, StatsBlock, TagInfo,
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
        .map(|value| build_contract_detail(value, season.numero));
    let leader_id = category_id
        .as_deref()
        .map(|category| get_driver_leader_id(conn, category))
        .transpose()?
        .flatten();
    let recent_results = category_id
        .as_deref()
        .map(|category| {
            build_recent_results_for_driver(conn, career_dir, &season.id, category, &driver.id)
        })
        .transpose()?
        .unwrap_or_default();
    let badges = build_driver_badges(driver, category_id.as_deref(), leader_id.as_deref());

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
        performance: build_driver_performance_block(driver, &recent_results),
        forma: build_driver_form_block(&recent_results),
        trajetoria: build_driver_career_path_block(driver, team, contract, category_id.as_deref()),
        contrato_mercado: DriverContractMarketBlock {
            contrato: contract_detail,
            mercado: None,
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

fn build_contract_detail(contract: &Contract, current_season: i32) -> ContractDetail {
    ContractDetail {
        equipe_nome: contract.equipe_nome.clone(),
        papel: match contract.papel.as_str() {
            "Numero1" => "N1".to_string(),
            _ => "N2".to_string(),
        },
        salario_anual: contract.salario_anual,
        temporada_inicio: contract.temporada_inicio,
        temporada_fim: contract.temporada_fim,
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

fn build_driver_badges(
    driver: &Driver,
    category_id: Option<&str>,
    leader_id: Option<&str>,
) -> Vec<DriverBadge> {
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

    if leader_id == Some(driver.id.as_str()) {
        badges.push(DriverBadge {
            label: "LIDER".to_string(),
            variant: "success".to_string(),
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

fn get_driver_leader_id(conn: &Connection, category: &str) -> Result<Option<String>, String> {
    let mut drivers = driver_queries::get_drivers_by_category(conn, category)
        .map_err(|e| format!("Falha ao buscar standings da categoria: {e}"))?;

    drivers.sort_by(|a, b| {
        b.stats_temporada
            .pontos
            .total_cmp(&a.stats_temporada.pontos)
            .then_with(|| b.stats_temporada.vitorias.cmp(&a.stats_temporada.vitorias))
            .then_with(|| b.stats_temporada.podios.cmp(&a.stats_temporada.podios))
            .then_with(|| a.nome.cmp(&b.nome))
    });

    Ok(drivers.first().map(|driver| driver.id.clone()))
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

fn build_driver_form_block(results: &[HistoricalRaceResult]) -> DriverFormBlock {
    let ultimas_5_source: Vec<HistoricalRaceResult> = results
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let media_chegada = average_finish(&ultimas_5_source);
    let tendencia = calculate_form_trend(&ultimas_5_source);
    let momento = match media_chegada {
        Some(value) if value <= 5.0 => "forte".to_string(),
        Some(value) if value <= 10.0 => "estavel".to_string(),
        Some(_) => "em_baixa".to_string(),
        None => "sem_dados".to_string(),
    };

    DriverFormBlock {
        ultimas_5: ultimas_5_source
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
    }
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

fn build_driver_career_path_block(
    driver: &Driver,
    team: Option<&Team>,
    contract: Option<&Contract>,
    category_id: Option<&str>,
) -> DriverCareerPathBlock {
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

    DriverCareerPathBlock {
        ano_estreia: driver.ano_inicio_carreira as i32,
        equipe_estreia: contract
            .filter(|value| value.temporada_inicio <= 1)
            .map(|value| value.equipe_nome.clone())
            .or_else(|| team.map(|value| value.nome.clone())),
        categoria_atual: category_id.map(str::to_string),
        temporadas_na_categoria: driver.temporadas_na_categoria as i32,
        corridas_na_categoria: driver.corridas_na_categoria as i32,
        titulos: driver.stats_carreira.titulos as i32,
        foi_campeao: driver.stats_carreira.titulos > 0,
        marcos,
    }
}
