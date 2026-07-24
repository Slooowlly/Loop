use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use crate::commands::career::count_calendar_entries;
use crate::commands::career_types::{
    CareerMilestone, ContractDetail, DriverActiveInjuryBlock, DriverBadge, DriverBestSeasonBlock,
    DriverCareerCategoryStint, DriverCareerFirstMarksBlock, DriverCareerHistoryBlock,
    DriverCareerInjuryBlock, DriverCareerMobilityBlock, DriverCareerPathBlock,
    DriverCareerPeakBlock, DriverCareerPresenceBlock, DriverCareerRankBlock,
    DriverCareerSpecialEventsBlock, DriverCompetitiveBlock, DriverContractMarketBlock,
    DriverCurrentSummaryBlock, DriverDetail, DriverFormBlock, DriverHealthBlock, DriverLicenseInfo,
    DriverMarketBlock, DriverPerformanceBlock, DriverPerformanceReadBlock, DriverProfileBlock,
    DriverRivalInfo, DriverRivalsBlock, DriverSpecialCampaignBlock, DriverSpecialEventEntry,
    DriverSpecialEventRankBlock, DriverStardomBlock, DriverTechnicalReadBlock,
    DriverTechnicalReadItem, FormResultEntry, PerformanceStatsBlock, PersonalityInfo, StatsBlock,
    TagInfo,
};
use crate::commands::race_history::build_driver_histories;
use crate::constants::categories;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::injuries as injury_queries;
use crate::db::queries::teams as team_queries;
use crate::models::contract::Contract;
use crate::models::driver::{AttributeTag, Driver, TagLevel};
use crate::models::enums::{DriverStatus, InjuryType, PrimaryPersonality, SecondaryPersonality};
use crate::models::season::Season;
use crate::models::team::Team;
use crate::simulation::injuries::{injury_display_name, injury_name_pool};

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
    let health = build_driver_health_block(conn, driver)?;

    Ok(DriverDetail {
        id: driver.id.clone(),
        nome: driver.nome.clone(),
        nacionalidade: driver.nacionalidade.clone(),
        idade: driver.idade as i32,
        genero: driver.genero.clone(),
        is_jogador: driver.is_jogador,
        is_favorito: crate::db::queries::favorites::is_favorite(conn, &driver.id)
            .unwrap_or(false),
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
            personalidade_secundaria,
            motivacao: driver.motivacao.round().clamp(0.0, 100.0) as u8,
            qualidades,
            defeitos,
            neutro: tags.is_empty() && !driver.is_jogador,
        },
        leitura_tecnica: build_driver_technical_read_block(driver),
        estrelato: build_driver_stardom_block(driver),
        performance: build_driver_performance_block(driver, &recent_results),
        forma: build_driver_form_block(&recent_results, form_context.as_deref()),
        resumo_atual: build_current_summary_block(driver, &recent_results, championship_position),
        leitura_desempenho: build_performance_read_block(
            conn,
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
        saude: health,
    })
}

fn build_driver_health_block(
    conn: &Connection,
    driver: &Driver,
) -> Result<Option<DriverHealthBlock>, String> {
    let Some(injury) = injury_queries::get_active_injury_for_pilot(conn, &driver.id)
        .map_err(|e| format!("Falha ao buscar lesao ativa do piloto: {e}"))?
    else {
        return Ok(None);
    };

    let race = calendar_queries::get_calendar_entry_by_id(conn, &injury.race_occurred)
        .map_err(|e| format!("Falha ao buscar corrida da lesao ativa: {e}"))?;
    let occurred_label = race.as_ref().map(|entry| {
        format!(
            "R{} - {}",
            entry.rodada,
            if entry.track_name.is_empty() {
                entry.nome.as_str()
            } else {
                entry.track_name.as_str()
            }
        )
    });
    let injury_name = injury.injury_name.trim().to_string();
    let injury_name = if injury_name.is_empty() {
        fallback_injury_display_name(&injury.injury_type, &injury.id).to_string()
    } else {
        injury_name
    };

    Ok(Some(DriverHealthBlock {
        saude_geral: None,
        lesao_ativa: Some(DriverActiveInjuryBlock {
            nome: Some(injury_name),
            tipo: injury.injury_type.as_str().to_string(),
            corrida_ocorrida_id: injury.race_occurred,
            corrida_ocorrida_rotulo: occurred_label,
            corrida_ocorrida_rodada: race.as_ref().map(|entry| entry.rodada),
            corrida_ocorrida_pista: race.as_ref().map(|entry| entry.track_name.clone()),
            corridas_total: injury.races_total,
            corridas_restantes: injury.races_remaining,
        }),
    }))
}

fn fallback_injury_display_name(injury_type: &InjuryType, key: &str) -> String {
    let pool = injury_name_pool(injury_type.clone());
    let index = key
        .bytes()
        .fold(0usize, |acc, byte| acc.wrapping_add(byte as usize))
        % pool.len();
    injury_display_name(pool[index])
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

/// Monta o card de personalidade (display) resolvendo nome+descrição do locale ativo
/// por `key`. `tipo` é DISPLAY, desacoplado do token de serialização em enums.rs.
fn personality_info(key: &str, emoji: &str) -> PersonalityInfo {
    let name_key = format!("career.personality.{key}.name");
    let desc_key = format!("career.personality.{key}.desc");
    PersonalityInfo {
        tipo: rust_i18n::t!(&name_key).to_string(),
        emoji: emoji.to_string(),
        descricao: rust_i18n::t!(&desc_key).to_string(),
    }
}

fn convert_primary_personality(personality: &PrimaryPersonality) -> PersonalityInfo {
    let (key, emoji) = match personality {
        PrimaryPersonality::Ambicioso => ("ambicioso", "\u{1F3C6}"),
        PrimaryPersonality::Consolidador => ("consolidador", "\u{1F3E0}"),
        PrimaryPersonality::Mercenario => ("mercenario", "\u{1F4B0}"),
        PrimaryPersonality::Leal => ("leal", "\u{2764}\u{FE0F}"),
    };
    personality_info(key, emoji)
}

fn convert_secondary_personality(personality: &SecondaryPersonality) -> PersonalityInfo {
    let (key, emoji) = match personality {
        SecondaryPersonality::CabecaQuente => ("cabeca_quente", "\u{1F525}"),
        SecondaryPersonality::SangueFrio => ("sangue_frio", "\u{1F9CA}"),
        SecondaryPersonality::Apostador => ("apostador", "\u{1F3B0}"),
        SecondaryPersonality::Calculista => ("calculista", "\u{1F6E1}\u{FE0F}"),
        SecondaryPersonality::Showman => ("showman", "\u{1F451}"),
        SecondaryPersonality::TeamPlayer => ("team_player", "\u{1F91D}"),
        SecondaryPersonality::Solitario => ("solitario", "\u{1F624}"),
        SecondaryPersonality::Estudioso => ("estudioso", "\u{1F4DA}"),
    };
    personality_info(key, emoji)
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
    contract
        .and_then(|value| regular_division_key(&value.categoria, value.classe.as_deref()))
        .or_else(|| {
            team.and_then(|value| regular_division_key(&value.categoria, value.classe.as_deref()))
        })
        .or_else(|| regular_category(driver.categoria_atual.as_deref()))
}

fn regular_category(category: Option<&str>) -> Option<String> {
    let category = category?.trim();
    if category.is_empty() {
        return None;
    }
    if let Some((base_category, class_name)) = category.split_once(':') {
        return regular_division_key(base_category, Some(class_name));
    }
    if categories::is_especial(category) {
        None
    } else {
        Some(category.to_string())
    }
}

fn regular_division_key(category: &str, class_name: Option<&str>) -> Option<String> {
    categories::is_valid_competitive_division(category, class_name)
        .then(|| categories::competitive_division_key(category, class_name))
}

fn competitive_division_label_from_key(key: &str) -> Option<String> {
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    if let Some((category, class_name)) = key.split_once(':') {
        return categories::is_valid_competitive_division(category, Some(class_name))
            .then(|| categories::competitive_division_label(category, Some(class_name)));
    }
    categories::get_category_config(key).map(|category| category.nome_curto.to_string())
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

/// Monta o bloco de ESTRELATO (fama + carisma) para a ficha do piloto.
/// Fama (`midia`) usa a MESMA classificação de tier do mercado (visibility.rs) pra
/// ficar coerente com o resto do jogo; carisma tem escala descritiva própria; e o
/// `resumo` traduz a DINÂMICA (carisma modula a fama) em uma linha.
fn build_driver_stardom_block(driver: &Driver) -> DriverStardomBlock {
    let fama = driver.atributos.midia.clamp(0.0, 100.0);
    let carisma = driver.atributos.carisma.clamp(0.0, 100.0);

    let (nivel_fama, tom_fama) = fama_level_for_value(fama);
    let (nivel_carisma, tom_carisma) = carisma_level_for_value(carisma);
    let resumo = stardom_reading(fama, carisma);

    DriverStardomBlock {
        fama: fama.round() as u8,
        carisma: carisma.round() as u8,
        nivel_fama: nivel_fama.to_string(),
        tom_fama: tom_fama.to_string(),
        nivel_carisma: nivel_carisma.to_string(),
        tom_carisma: tom_carisma.to_string(),
        resumo,
    }
}

/// Escala de FAMA para exibição — 6 níveis, mais rica que os 4 tiers de mercado
/// internos (o display pode ser mais granular que a lógica comercial de
/// salário/patrocínio). Vai de Anônimo a Ídolo; o topo é aspiracional e raro.
fn fama_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value <= 15.0 {
        ("anonimo", "neutral")
    } else if value <= 30.0 {
        ("discreto", "neutral")
    } else if value <= 50.0 {
        ("conhecido", "info")
    } else if value <= 70.0 {
        ("nome_forte", "info")
    } else if value <= 87.0 {
        ("estrela", "success")
    } else {
        ("idolo", "elite")
    };
    let full = format!("driver_read.fama.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

fn carisma_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value < 30.0 {
        ("apagado", "danger")
    } else if value < 45.0 {
        ("reservado", "warning")
    } else if value < 60.0 {
        ("cativante", "neutral")
    } else if value < 75.0 {
        ("magnetico", "info")
    } else if value < 88.0 {
        ("carismatico", "success")
    } else {
        ("idolo_natural", "elite")
    };
    let full = format!("driver_read.carisma.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
}

/// Leitura de uma linha: como o carisma (retenção/conversão) conversa com a fama
/// (estoque). Alto carisma + baixa fama = pólvora seca; baixo carisma + alta fama =
/// holofote volátil construído só pelo resultado.
fn stardom_reading(fama: f64, carisma: f64) -> String {
    let fama_alta = fama >= 60.0;
    let carisma_alto = carisma >= 60.0;

    let key = match (fama_alta, carisma_alto) {
        (true, true) => "idol_consolidated",
        (false, true) => "powder_keg",
        (true, false) => "volatile_spotlight",
        (false, false) => "off_radar",
    };
    let full = format!("driver_read.stardom.{key}");
    rust_i18n::t!(&full).to_string()
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

fn technical_level_for_value(value: f64) -> (String, &'static str) {
    let value = value.clamp(0.0, 100.0);
    let (key, tom) = if value < 12.5 {
        ("muito_fraco", "danger")
    } else if value < 25.0 {
        ("fraco", "danger")
    } else if value < 37.5 {
        ("abaixo", "warning")
    } else if value < 50.0 {
        ("instavel", "warning")
    } else if value < 62.5 {
        ("competente", "neutral")
    } else if value < 75.0 {
        ("forte", "info")
    } else if value < 87.5 {
        ("muito_forte", "success")
    } else {
        ("elite", "elite")
    };
    let full = format!("driver_read.technical.{key}");
    (rust_i18n::t!(&full).to_string(), tom)
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
            veredito: rust_i18n::t!("driver_read.verdict.rookie").to_string(),
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
    let (verdict_key, tom) = if verdict_score >= 45 {
        ("excellent", "success")
    } else if verdict_score >= 24 {
        ("good", "success")
    } else if verdict_score >= 10 {
        ("fair", "warning")
    } else if has_enough_evidence
        && (is_critical_average
            || dnf_rate >= 0.4
            || (is_very_low_in_championship && is_bad_average))
    {
        ("critical", "danger")
    } else if has_enough_evidence && (is_bad_average || is_low_in_championship) {
        ("bad", "danger")
    } else {
        ("evaluating", "info")
    };

    let verdict_full = format!("driver_read.verdict.{verdict_key}");
    DriverCurrentSummaryBlock {
        veredito: rust_i18n::t!(&verdict_full).to_string(),
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
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
    teammate: Option<&Driver>,
    championship_position: Option<i32>,
) -> DriverPerformanceReadBlock {
    let expected = team.and_then(|value| expected_position_for_team(conn, value));
    let delta = match (expected, championship_position) {
        (Some(expected_position), Some(position)) => Some(expected_position - position),
        _ => None,
    };
    let teammate_points = teammate.map(|value| value.stats_temporada.pontos.round() as i32);
    let reading = match delta {
        Some(value) if value >= 3 => rust_i18n::t!("driver_read.delivery.above"),
        Some(value) if value <= -3 => rust_i18n::t!("driver_read.delivery.below"),
        Some(_) => rust_i18n::t!("driver_read.delivery.within"),
        None => rust_i18n::t!("driver_read.delivery.no_context"),
    };

    DriverPerformanceReadBlock {
        esperado_posicao: expected,
        entregue_posicao: championship_position,
        delta_posicao: delta,
        car_performance: team.map(|value| value.effective_car_performance()),
        companheiro_nome: teammate.map(|value| value.nome.clone()),
        companheiro_pontos: teammate_points,
        piloto_pontos: driver.stats_temporada.pontos.round() as i32,
        leitura: reading.to_string(),
    }
}

/// Dois carros dentro desta distância de `car_performance` estão em EMPATE TÉCNICO — o
/// pacote não separa os dois. Carros de mesmo nível dão magnitude idêntica, então a margem
/// só existe pro caminho legado (equipe sem peças persistidas, escalar contínuo).
const CAR_TIE_EPSILON: f64 = 1e-6;

/// Assentos OCUPADOS da equipe (0–2) — o grid REAL, não a capacidade nominal.
fn filled_seats(team: &Team) -> i32 {
    i32::from(team.piloto_1_id.is_some()) + i32::from(team.piloto_2_id.is_some())
}

/// Posição ESPERADA pelo pacote (carro), RELATIVA ao grid da categoria.
///
/// Era uma tabela de limiares ABSOLUTOS sobre o escalar de carro, e ela mentia por dois
/// lados: o escalar não tem escala comum entre categorias (as de cima estouram o topo da
/// tabela e o grid inteiro "espera" P2), e num grid SPEC — rookie, todo carro no nível 1 —
/// o escalar é IDÊNTICO pra todo mundo, então a tabela dava posição de fundo pra todas as
/// equipes de uma vez. Aqui a equipe é ranqueada pelo carro EFETIVO ([`Team::effective_car_performance`])
/// contra as rivais do mesmo grid (categoria + classe) e a expectativa é o meio da faixa de
/// assentos do seu rank. Grid spec (todo mundo empatado) → todo mundo espera o meio do grid,
/// que é a leitura honesta quando o carro não separa ninguém e o resultado é só piloto.
fn expected_position_for_team(conn: &Connection, team: &Team) -> Option<i32> {
    let rivals = team_queries::get_teams_by_category(conn, &team.categoria).ok()?;
    let grid: Vec<(f64, i32)> = rivals
        .iter()
        .filter(|rival| rival.classe == team.classe)
        .map(|rival| (rival.effective_car_performance(), filled_seats(rival)))
        .collect();

    expected_position_from_grid(team.effective_car_performance(), &grid)
}

/// Núcleo PURO do rank: dado o carro da equipe e o grid `(carro, assentos ocupados)`, cai no
/// MEIO da faixa de assentos do bloco em que ela está. Assentos com carro estritamente melhor
/// ficam à frente; o bloco do empate técnico inclui a própria equipe. `None` quando o bloco
/// está vazio (equipe sem assento ocupado — não há expectativa a dar).
fn expected_position_from_grid(mine: f64, grid: &[(f64, i32)]) -> Option<i32> {
    let mut seats_ahead = 0;
    let mut seats_tied = 0;
    for &(perf, seats) in grid {
        let delta = perf - mine;
        if delta > CAR_TIE_EPSILON {
            seats_ahead += seats;
        } else if delta >= -CAR_TIE_EPSILON {
            seats_tied += seats;
        }
    }

    if seats_tied == 0 {
        return None;
    }
    Some(seats_ahead + (seats_tied + 1) / 2)
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
    let injury_counts = injury_queries::count_injuries_by_severity_for_pilot(conn, driver_id)
        .map_err(|e| format!("Falha ao contar lesoes historicas do piloto: {e}"))?;
    let lesoes = DriverCareerInjuryBlock {
        leves: injury_counts.leves,
        moderadas: injury_counts.moderadas,
        graves: injury_counts.graves,
    };
    let eventos_especiais = build_special_events_block(conn, driver_id)?;

    Ok(DriverCareerHistoryBlock {
        presenca,
        primeiros_marcos,
        auge,
        mobilidade,
        lesoes,
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
        if categories::runs_in_special_phase(&contract.category) {
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
        if categories::runs_in_special_phase(&category) {
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
        if categories::runs_in_special_phase(&category) {
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

fn career_debut_year_from_archive(seasons: &[CareerSeasonArchiveRow], fallback_year: i32) -> i32 {
    let archive_year = seasons
        .iter()
        .filter(|season| {
            let category = season.categoria.trim();
            season.corridas > 0 && !category.is_empty() && !categories::is_especial(category)
        })
        .map(|season| season.ano)
        .min();

    match (archive_year, fallback_year > 0) {
        (Some(year), true) => fallback_year.min(year),
        (Some(year), false) => year,
        (None, true) => fallback_year,
        (None, false) => 0,
    }
}

fn inferred_debut_year_from_driver(driver: &Driver, current_year: i32) -> i32 {
    if current_year <= 0 {
        return driver.ano_inicio_carreira as i32;
    }

    let career_seasons = driver.stats_carreira.temporadas as i32;
    if career_seasons > 0 {
        (current_year - career_seasons + 1).max(1)
    } else {
        // Sem temporada fechada, toda largada dele é da temporada em curso: a
        // estreia é este ano. `ano_inicio_carreira` é pano de fundo (o ano do
        // kart, aos 16), não estreia — mesma régua do ranking global.
        current_year
    }
}

/// Anos de carreira. Sem NENHUMA largada não existe carreira: o piloto ainda é
/// um novato. `temporadas` sozinho não serve de prova — ele cresce todo fim de
/// ano mesmo para quem passou a temporada inteira sem assento.
fn career_years_from_debut(
    driver: &Driver,
    archived_starts: i32,
    current_year: i32,
    debut_year: i32,
) -> i32 {
    if current_year <= 0 || debut_year <= 0 {
        return 0;
    }

    // Um DNF também é largada: ele alinhou no grid.
    let starts = archived_starts
        .max(driver.stats_carreira.corridas as i32)
        .max(driver.stats_carreira.dnfs as i32)
        .max(driver.stats_temporada.corridas as i32)
        .max(driver.stats_temporada.dnfs as i32);
    if starts <= 0 {
        return 0;
    }

    (current_year - debut_year + 1).max(0)
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
        .filter(|season| {
            let category = season.categoria.trim();
            season.corridas > 0 && !category.is_empty() && !categories::is_especial(category)
        })
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

    if let Some(category) = regular_category(current_category) {
        match timeline.last_mut() {
            Some(last) if last.categoria == category => {
                last.ano_fim = last.ano_fim.max(current_year);
            }
            Some(last) if last.ano_inicio == current_year => {
                last.categoria = category;
                last.ano_fim = current_year;
            }
            _ => timeline.push(DriverCareerCategoryStint {
                categoria: category,
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
    let season_archive = load_career_season_archive_rows(conn, &driver.id)?;
    let archive_debut_year = career_debut_year_from_archive(&season_archive, 0);
    let debut_year = if archive_debut_year > 0 {
        archive_debut_year
    } else {
        inferred_debut_year_from_driver(driver, current_year)
    };
    let mut marcos = vec![CareerMilestone {
        tipo: "estreia".to_string(),
        titulo: rust_i18n::t!("career.milestone.debut_title").to_string(),
        descricao: rust_i18n::t!("career.milestone.debut_desc", year = debut_year).to_string(),
    }];

    if driver.stats_carreira.titulos > 0 {
        let titulos = driver.stats_carreira.titulos;
        let descricao = if titulos == 1 {
            rust_i18n::t!("career.milestone.titles_desc_one").to_string()
        } else {
            rust_i18n::t!("career.milestone.titles_desc_other", count = titulos).to_string()
        };
        marcos.push(CareerMilestone {
            tipo: "titulo".to_string(),
            titulo: rust_i18n::t!("career.milestone.titles_title").to_string(),
            descricao,
        });
    }

    if let Some(category_label) = category_id.and_then(competitive_division_label_from_key) {
        marcos.push(CareerMilestone {
            tipo: "categoria".to_string(),
            titulo: rust_i18n::t!("career.milestone.current_title").to_string(),
            descricao: rust_i18n::t!("career.milestone.current_desc", category = category_label)
                .to_string(),
        });
    }

    let archived_starts = season_archive
        .iter()
        .map(|season| season.corridas)
        .sum::<i32>();
    let mut historico = build_career_history_block(conn, &driver.id)?;
    historico.presenca.tempo_carreira =
        career_years_from_debut(driver, archived_starts, current_year, debut_year);

    Ok(DriverCareerPathBlock {
        ano_estreia: debut_year,
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
#[path = "career_detail/tests/mod.rs"]
mod tests;
