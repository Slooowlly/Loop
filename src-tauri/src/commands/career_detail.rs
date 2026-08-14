use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;

use crate::commands::career::count_calendar_entries;
use crate::commands::career_types::{
    CareerMilestone, ContractDetail, DriverActiveInjuryBlock, DriverBadge, DriverBestSeasonBlock,
    DriverCareerArcBlock, DriverCareerBenchmarkBlock, DriverCareerCategoryStint,
    DriverCareerDetailEntry, DriverCareerDroughtBlock, DriverCareerFirstMarksBlock,
    DriverCareerHistoryBlock, DriverCareerInjuryBlock, DriverCareerMobilityBlock,
    DriverCareerPathBlock, DriverCareerPeakBlock, DriverCareerPresenceBlock,
    DriverCareerQualifyingBlock, DriverCareerRankBlock, DriverCareerRankEntry,
    DriverCareerReliabilityBlock, DriverCareerSpecialEventsBlock, DriverCareerTeammateBlock,
    DriverChampionshipCurvePoint, DriverCompetitiveBlock, DriverContractMarketBlock,
    DriverCurrentSummaryBlock, DriverDetail, DriverFormBlock, DriverHealthBlock, DriverLicenseInfo,
    DriverMarketBlock, DriverMarketCurvePoint, DriverPerformanceBlock, DriverPerformanceReadBlock,
    DriverProfileBlock, DriverRivalInfo, DriverRivalMeeting, DriverRivalTeammateSpell,
    DriverRivalsBlock, DriverSpecialCampaignBlock, DriverSpecialEventEntry,
    DriverSpecialEventRankBlock, DriverStardomBlock, DriverTeammateDuel, DriverTechnicalReadBlock,
    DriverTechnicalReadItem, DriverTitleEntry, FormResultEntry, FormSeasonBlock,
    PerformanceStatsBlock, PersonalityInfo, StatsBlock, TagInfo, TransferForceBreakdown,
};
use crate::commands::global_driver_rankings::valid_archived_title_count;
use crate::commands::race_history::build_driver_histories;
use crate::common::numeros::formatar_decimal;
use crate::constants::categories;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::injuries as injury_queries;
use crate::db::queries::teams as team_queries;
use crate::generators::nationality::nationality_display_label;
use crate::models::contract::Contract;
use crate::models::driver::{AttributeTag, Driver, DriverAttributes, TagLevel};
use crate::models::driver_tags::visible_tags_in_grid;
use crate::models::enums::{
    ContractStatus, DriverStatus, InjuryType, PrimaryPersonality, SecondaryPersonality,
};
use crate::models::season::Season;
use crate::models::team::Team;
use crate::simulation::injuries::{injury_display_name, injury_name_pool};

#[path = "career_detail/campeonato.rs"]
mod campeonato;
#[path = "career_detail/classificacao.rs"]
mod classificacao;
#[path = "career_detail/detalhes.rs"]
mod detalhes;
#[path = "career_detail/divisao.rs"]
mod divisao;
#[path = "career_detail/duelos.rs"]
mod duelos;
#[path = "career_detail/especiais.rs"]
mod especiais;
#[path = "career_detail/historico.rs"]
mod historico;
#[path = "career_detail/leitura.rs"]
mod leitura;
#[path = "career_detail/mercado.rs"]
mod mercado;
#[path = "career_detail/perfil.rs"]
mod perfil;
#[path = "career_detail/recordes.rs"]
mod recordes;
#[path = "career_detail/resultados.rs"]
mod resultados;
#[path = "career_detail/rivais.rs"]
mod rivais;
#[path = "career_detail/tipos.rs"]
mod tipos;
#[path = "career_detail/trajetoria.rs"]
mod trajetoria;

// Tudo aqui e consumido so pela propria fachada e pelos irmaos (via `use super::*`).
use campeonato::*;
use classificacao::*;
use detalhes::*;
use divisao::*;
use duelos::*;
use especiais::*;
use historico::*;
use leitura::*;
use mercado::*;
use perfil::*;
// Sem glob: depois que os recordes sairam do payload da ficha, este modulo nao
// consome mais nada de `recordes` — ele so reexporta o construtor para a casca
// de `career`, que atende o comando dedicado.
pub(crate) use recordes::build_dossier_ranks;
use resultados::*;
use rivais::*;
use tipos::*;
use trajetoria::*;

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
    // Os tracos saem LIDOS CONTRA O GRID, e nao so contra a escala da escada. E
    // o mesmo grid da regua da leitura tecnica, carregado uma vez e emprestado
    // para as duas leituras — ver `visible_tags_in_grid`.
    let grid = category_id
        .as_deref()
        .map(|category| grid_da_categoria(conn, category, team))
        .unwrap_or_default();
    let tags = convert_tags(&visible_tags_in_grid(&driver.atributos, &grid));
    let (qualidades, defeitos) = split_driver_tags(&tags);
    let contract_detail = contract
        .as_ref()
        .map(|value| build_contract_detail(value, season.numero, season.ano));
    let mut recent_results = category_id
        .as_deref()
        .map(|category| {
            build_recent_results_for_driver(
                conn,
                career_dir,
                &season.id,
                base_category_of(category),
                &driver.id,
            )
        })
        .transpose()?
        .unwrap_or_default();
    let mut form_context = None;
    if recent_results.is_empty() {
        let archived = build_archived_recent_results_for_driver(conn, season.numero, &driver.id)?;
        form_context = archived.form_context;
        recent_results = archived.results;
    }
    let championship = category_id
        .as_deref()
        .map(|category| find_championship_context(conn, category, &driver.id))
        .transpose()?
        .flatten();
    let championship_position = championship.as_ref().map(|value| value.posicao);
    let teammate = find_teammate(conn, driver, team)?;
    let badges = build_driver_badges(driver, category_id.as_deref());
    let health = build_driver_health_block(conn, driver)?;

    Ok(DriverDetail {
        id: driver.id.clone(),
        nome: driver.nome.clone(),
        // Rótulo de DISPLAY, resolvido no locale ativo: o save guarda a forma que
        // estava em vigor quando o piloto nasceu, e sem esta borda um jogador em
        // en-US lia "Britânico" na ficha. Nada aqui volta para o banco.
        nacionalidade: nationality_display_label(&driver.nacionalidade, &driver.genero),
        idade: driver.idade as i32,
        genero: driver.genero.clone(),
        is_jogador: driver.is_jogador,
        is_favorito: crate::db::queries::favorites::is_favorite(conn, &driver.id).unwrap_or(false),
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
        leitura_tecnica: build_driver_technical_read_block(
            conn,
            driver,
            category_id.as_deref(),
            team,
        ),
        estrelato: build_driver_stardom_block(driver),
        arco: build_driver_career_arc_block(driver),
        performance: build_driver_performance_block(driver, &recent_results),
        forma: build_driver_form_block(
            &recent_results,
            form_context.as_deref(),
            build_form_seasons_for_driver(conn, &driver.id, season.numero)?,
        ),
        resumo_atual: build_current_summary_block(driver, &recent_results, championship.as_ref()),
        leitura_desempenho: build_performance_read_block(
            conn,
            driver,
            team,
            teammate.as_ref(),
            championship_position,
            category_id.as_deref(),
        ),
        trajetoria: build_driver_career_path_block(
            conn,
            driver,
            team,
            contract,
            category_id.as_deref(),
            season,
            // A classificacao viva vira o ultimo ponto da curva de campeonato: a
            // temporada em curso nao esta arquivada, e sem ela o grafico pararia
            // no ano passado justamente onde o jogador quer olhar.
            championship.as_ref().map(|valor| PosicaoDeHoje {
                posicao: valor.posicao,
                total: valor.total,
            }),
        )?,
        rankings_carreira: build_career_rank_block(conn, driver)?,
        rankings_grid: build_grid_rank_block(conn, driver, category_id.as_deref(), team),
        rivais: build_driver_rivals_block(conn, driver)?,
        contrato_mercado: DriverContractMarketBlock {
            contrato: contract_detail,
            mercado: Some(build_driver_market_block(
                conn,
                driver,
                contract,
                team,
                season.numero,
            )),
            curva: build_driver_market_curve(conn, driver, contract, team, season)?,
        },
        relacionamentos: None,
        reputacao: None,
        saude: health,
    })
}

fn build_driver_form_block(
    results: &[HistoricalRaceResult],
    form_context: Option<&str>,
    temporadas: Vec<FormSeasonBlock>,
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
        temporadas,
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
    championship: Option<&ChampionshipContext>,
) -> DriverCurrentSummaryBlock {
    let championship_position = championship.map(|value| value.posicao);
    // Antes da primeira largada o grid inteiro esta com zero ponto e a ordem e
    // desempate alfabetico: "0 do lider" valeria para todo mundo e a vantagem
    // sobre o proximo seria um sorteio com cara de resultado.
    let raced = driver.stats_temporada.corridas > 0;
    let (gap_lider, gap_proximo) = match championship.filter(|_| raced) {
        Some(context) => (Some(context.gap_lider), context.gap_proximo),
        None => (None, None),
    };

    if driver.stats_carreira.corridas == 0 && results.is_empty() {
        return DriverCurrentSummaryBlock {
            veredito: rust_i18n::t!("driver_read.verdict.rookie").to_string(),
            tom: "info".to_string(),
            posicao_campeonato: championship_position,
            gap_lider,
            gap_proximo,
            pontos: driver.stats_temporada.pontos.round() as i32,
            vitorias: driver.stats_temporada.vitorias as i32,
            podios: driver.stats_temporada.podios as i32,
            top_10: Some(0),
            media_recente: None,
            tendencia: "desconhecida".to_string(),
        };
    }

    // O resumo so consome a media e a tendencia, que saem das ultimas corridas —
    // a faixa por temporada nao entra nessa conta.
    let form = build_driver_form_block(results, None, Vec::new());
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
        gap_lider,
        gap_proximo,
        pontos: driver.stats_temporada.pontos.round() as i32,
        vitorias: driver.stats_temporada.vitorias as i32,
        podios: driver.stats_temporada.podios as i32,
        top_10: Some(top_10),
        media_recente: form.media_chegada,
        tendencia: form.tendencia,
    }
}

#[cfg(test)]
#[path = "career_detail/tests/mod.rs"]
mod tests;
