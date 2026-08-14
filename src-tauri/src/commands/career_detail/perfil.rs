//! Perfil e identidade do piloto: status, personalidade, tags, licenca, badges, contrato exibido, blocos de estatistica e saude.

use super::*;

pub(super) fn build_driver_health_block(
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
            // A CHAVE do fio ("light"/"moderate"/…), não a grafia do banco. A ficha
            // usava `as_str()` e mandava "Moderada" para a tela, que a imprimia crua:
            // em en-US saía português, e acentuar "Critica" no backend — o que a
            // regra de copy do projeto pede — teria trocado o rótulo em silêncio.
            tipo: injury.injury_type.chave().to_string(),
            corrida_ocorrida_id: injury.race_occurred,
            corrida_ocorrida_rotulo: occurred_label,
            corrida_ocorrida_rodada: race.as_ref().map(|entry| entry.rodada),
            corrida_ocorrida_pista: race.as_ref().map(|entry| entry.track_name.clone()),
            corridas_total: injury.races_total,
            corridas_restantes: injury.races_remaining,
        }),
    }))
}

pub(super) fn fallback_injury_display_name(injury_type: &InjuryType, key: &str) -> String {
    let pool = injury_name_pool(injury_type.clone());
    let index = key
        .bytes()
        .fold(0usize, |acc, byte| acc.wrapping_add(byte as usize))
        % pool.len();
    injury_display_name(pool[index])
}

pub(super) fn convert_tags(tags: &[AttributeTag]) -> Vec<TagInfo> {
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
pub(super) fn personality_info(key: &str, emoji: &str) -> PersonalityInfo {
    let name_key = format!("career.personality.{key}.name");
    let desc_key = format!("career.personality.{key}.desc");
    PersonalityInfo {
        tipo: rust_i18n::t!(&name_key).to_string(),
        emoji: emoji.to_string(),
        descricao: rust_i18n::t!(&desc_key).to_string(),
    }
}

pub(super) fn convert_primary_personality(personality: &PrimaryPersonality) -> PersonalityInfo {
    let (key, emoji) = match personality {
        PrimaryPersonality::Ambicioso => ("ambicioso", "\u{1F3C6}"),
        PrimaryPersonality::Consolidador => ("consolidador", "\u{1F3E0}"),
        PrimaryPersonality::Mercenario => ("mercenario", "\u{1F4B0}"),
        PrimaryPersonality::Leal => ("leal", "\u{2764}\u{FE0F}"),
    };
    personality_info(key, emoji)
}

pub(super) fn convert_secondary_personality(personality: &SecondaryPersonality) -> PersonalityInfo {
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

pub(super) fn driver_detail_status(driver: &Driver, has_active_contract: bool) -> String {
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

pub(super) fn build_season_stats_block(driver: &Driver) -> StatsBlock {
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

pub(super) fn build_career_stats_block(driver: &Driver) -> StatsBlock {
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

pub(super) fn build_contract_detail(
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

pub(super) fn split_driver_tags(tags: &[TagInfo]) -> (Vec<TagInfo>, Vec<TagInfo>) {
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

pub(super) fn build_driver_profile_block(
    driver: &Driver,
    status: &str,
    team: Option<&Team>,
    role: Option<&str>,
    category_id: Option<&str>,
    badges: Vec<DriverBadge>,
) -> DriverProfileBlock {
    // A bandeira e o gentílico saem do rótulo de DISPLAY, não do gravado: o save
    // congelou a forma em vigor na geração do piloto (e saves antigos gravaram sem
    // acento), e a ficha precisa falar o idioma de quem está lendo agora.
    let (bandeira, nacionalidade_label) = split_nationality(&nationality_display_label(
        &driver.nacionalidade,
        &driver.genero,
    ));

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

pub(super) fn split_nationality(nacionalidade: &str) -> (String, String) {
    let mut parts = nacionalidade.split_whitespace();
    let bandeira = parts.next().unwrap_or("\u{1F3C1}").to_string();
    let label = parts.collect::<Vec<_>>().join(" ");
    (bandeira, label)
}

pub(super) fn derive_driver_license(
    category_id: Option<&str>,
    driver: &Driver,
) -> DriverLicenseInfo {
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

pub(super) fn build_driver_badges(driver: &Driver, category_id: Option<&str>) -> Vec<DriverBadge> {
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
