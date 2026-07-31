//! Dossie historico e relatorio financeiro de uma equipe.
//!
//! Extraido de `career.rs`: agregacoes de corrida, marcos, timeline, rivais,
//! streaks e os rotulos/formatadores usados so por esse subsistema.
//!
//! Fachada: a montagem do dossie vive aqui; as pecas moram nos submodulos de
//! `career_team_dossier/`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use rusqlite::OptionalExtension;

use crate::commands::career::{
    get_teams_standings_in_base_dir, open_career_resources_for_category_read,
    open_career_resources_read_only,
};
use crate::commands::career_types::{
    TeamFinanceCashPoint, TeamFinanceReport, TeamFinanceRound, TeamHistoryCategoryStep,
    TeamHistoryChampionshipLine, TeamHistoryChampionshipRun, TeamHistoryDossier,
    TeamHistoryFormRace, TeamHistoryHighlight, TeamHistoryIdentity,
    TeamHistoryManagement, TeamHistoryMilestone, TeamHistoryMovement, TeamHistoryOutsideSeason,
    TeamHistoryOwnershipEvent, TeamHistoryRecord, TeamHistoryResultSpread, TeamHistoryRival,
    TeamHistorySeasonResult, TeamHistorySport, TeamHistoryTimelineItem, TeamHistoryTitleCategory,
    TeamRecordsCategory, TeamRecordsRanking, TeamRecordsRow,
};
use crate::constants::categories;
use crate::db::queries::teams as team_queries;

#[path = "career_team_dossier/categorias.rs"]
mod categorias;
#[path = "career_team_dossier/esportivo.rs"]
mod esportivo;
#[path = "career_team_dossier/fatos.rs"]
mod fatos;
#[path = "career_team_dossier/financas.rs"]
mod financas;
#[path = "career_team_dossier/gestao.rs"]
mod gestao;
#[path = "career_team_dossier/identidade.rs"]
mod identidade;
#[path = "career_team_dossier/pilotos.rs"]
mod pilotos;
#[path = "career_team_dossier/rotulos.rs"]
mod rotulos;

pub(crate) use financas::*;
// Consumidos pela propria fachada e pelos irmaos, via `use super::*`.
use categorias::*;
use esportivo::*;
use fatos::*;
use gestao::*;
use identidade::*;
use pilotos::*;
use rotulos::*;

/// Tabela de recordes de todas as equipes do grupo de uma categoria.
///
/// É o destino dos cards de record do dossiê: clicar em "Vitórias" abre esta
/// lista ordenada por vitórias, no mesmo recorte em que o card dizia "11º de 19".
/// Por isso o agregado é o MESMO — `aggregate_team_history` sobre os fatos do
/// grupo — e não uma segunda contagem que poderia divergir da primeira.
///
/// Equipe sem corrida no grupo não entra: a lista é de quem disputou, e uma
/// linha de zeros só empurraria as outras para baixo.
pub(crate) fn get_team_records_ranking_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    category: &str,
    scope: &str,
    class: Option<&str>,
) -> Result<TeamRecordsRanking, String> {
    let category = category.trim().to_lowercase();
    // A classe só faz sentido em categoria multiclasse; nas outras ela chega
    // vazia e é ignorada de qualquer forma por `keep_family_facts`.
    let class = class
        .map(|c| c.trim().to_lowercase())
        .filter(|c| !c.is_empty());
    let scope_kind = RecordScopeKind::parse(scope);
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let category_ids = scope_kind.categories(&category);
    let family = scope_kind.family(&category, class.as_deref());
    // O recorte por marca acontece AQUI, e não na consulta: a Production entra na
    // lista de categorias porque faz parte da escada, e é a classe do carro que
    // decide se aquela corrida foi contra a gente ou contra um campeonato irmão.
    let all_facts = keep_family_facts(
        load_team_race_facts(&db.conn, &category_ids)?,
        family.as_deref(),
    );
    let aggregates = aggregate_team_history(&all_facts);
    let titles_by_team = keep_family_titles(
        load_constructor_titles_by_team(&db.conn, &category_ids)?,
        family.as_deref(),
    );
    let cards = load_team_cards(&db.conn);

    // Os totais de carreira, para cada linha poder dizer "5 de 87". Sem eles o
    // recorte age em silêncio: um "5" solto se parece com uma equipe que mal
    // correu, e não com uma que correu 87 vezes e só 5 aqui.
    //
    // Em amplitude mundial os dois já são a mesma conta — repetir a leitura seria
    // pagar duas vezes pela mesma resposta.
    let (world_aggregates, world_titles) = if scope_kind == RecordScopeKind::World {
        (aggregates.clone(), titles_by_team.clone())
    } else {
        let world_ids = RecordScopeKind::World.categories(&category);
        let world_facts = load_team_race_facts(&db.conn, &world_ids)?;
        (
            aggregate_team_history(&world_facts),
            load_constructor_titles_by_team(&db.conn, &world_ids)?,
        )
    };

    // Janela de anos por equipe DENTRO do recorte. Sai dos mesmos fatos que a
    // contagem, então o período e o número de corridas nunca falam de tempos
    // diferentes — uma equipe com 5 corridas na Mazda Rookie mostra os anos em
    // que fez essas 5, não os da carreira toda.
    let mut janelas: HashMap<&str, (i32, i32)> = HashMap::new();
    for fact in &all_facts {
        let janela = janelas
            .entry(fact.team_id.as_str())
            .or_insert((fact.season_year, fact.season_year));
        janela.0 = janela.0.min(fact.season_year);
        janela.1 = janela.1.max(fact.season_year);
    }

    let mut rows: Vec<TeamRecordsRow> = aggregates
        .into_iter()
        .map(|(team_id, entry)| {
            let races = entry.races.max(0);
            let wins = entry.wins.max(0);
            let podiums = entry.podiums.max(0);
            let card = cards.get(&team_id).cloned().unwrap_or_default();
            let (primeiro, ultimo) = janelas
                .get(team_id.as_str())
                .copied()
                .unwrap_or((0, 0));
            let carreira = world_aggregates.get(&team_id).cloned().unwrap_or_default();
            TeamRecordsRow {
                total_titles: world_titles
                    .get(&team_id)
                    .map(|titles| titles.len() as i32)
                    .unwrap_or(0),
                total_wins: carreira.wins.max(0),
                total_podiums: carreira.podiums.max(0),
                total_races: carreira.races.max(0),
                first_year: if primeiro > 0 { primeiro.to_string() } else { String::new() },
                last_year: if ultimo > 0 { ultimo.to_string() } else { String::new() },
                team: card.name,
                color: card.color,
                category: if card.category_id.is_empty() {
                    String::new()
                } else {
                    team_history_category_label(&card.category_id)
                },
                category_id: card.category_id,
                active: card.active,
                titles: titles_by_team
                    .get(&team_id)
                    .map(|titles| titles.len() as i32)
                    .unwrap_or(0),
                wins,
                podiums,
                races,
                podium_rate: percentage(podiums, races),
                win_rate: percentage(wins, races),
                team_id,
            }
        })
        .collect();

    // Ordem de chegada é por títulos e depois vitórias — a mesma hierarquia de
    // "quem é a maior equipe daqui" que a aba Records usa. O frontend reordena
    // pela métrica clicada; isto é só o estado de repouso, e ordenar por id
    // deixaria a lista parecendo aleatória em quem abrir sem clicar em nada.
    rows.sort_by(|a, b| {
        b.titles
            .cmp(&a.titles)
            .then_with(|| b.wins.cmp(&a.wins))
            .then_with(|| b.podiums.cmp(&a.podiums))
            .then_with(|| a.team.cmp(&b.team))
    });

    Ok(TeamRecordsRanking {
        scope: scope_kind.label(&category, class.as_deref()),
        scope_kind: scope_kind.id().to_string(),
        scope_categories: category_ids
            .iter()
            .map(|id| team_history_category_label(id))
            .collect(),
        scope_family: family.clone().unwrap_or_default(),
        // A escada expande as multiclasse: a Production não é um campeonato, são
        // três correndo na mesma pista, e escolher "Production" inteira misturava
        // Mazda, Toyota e BMW num número só.
        categories: TEAM_RECORD_LADDER
            .iter()
            .flat_map(|id| {
                let classes = category_classes(id);
                let group_label = team_history_group_label(id);
                let label = team_history_category_label(id);
                if classes.is_empty() {
                    return vec![TeamRecordsCategory {
                        key: id.to_string(),
                        id: id.to_string(),
                        class: String::new(),
                        label,
                        group_label,
                    }];
                }
                classes
                    .into_iter()
                    .map(|class| TeamRecordsCategory {
                        key: format!("{id}:{class}"),
                        id: id.to_string(),
                        label: format!("{label} · {}", multiclass_label(&class)),
                        class,
                        group_label: group_label.clone(),
                    })
                    .collect()
            })
            .collect(),
        category,
        rows,
    })
}

pub(crate) fn get_team_history_dossier_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    team_id: &str,
    category: &str,
) -> Result<TeamHistoryDossier, String> {
    let category = category.trim().to_lowercase();
    let team_id = team_id.trim();
    let (db, _, _) = open_career_resources_read_only(base_dir, career_id)?;
    let category_ids = team_history_group_categories(&category);
    // O dossiê compara dentro do MESMO grupo que a aba de recordes, incluindo o
    // recorte por marca: se os dois divergissem, o "9º de 19" do card e a posição
    // na tabela seriam dois números diferentes para a mesma pergunta.
    let family = team_history_group_family(&category);
    let all_facts = keep_family_facts(load_team_race_facts(&db.conn, &category_ids)?, family);
    let selected_facts: Vec<TeamRaceFact> = all_facts
        .iter()
        .filter(|fact| fact.team_id == team_id)
        .cloned()
        .collect();
    let group_titles =
        keep_family_titles(load_constructor_titles_by_team(&db.conn, &category_ids)?, family);
    let drivers_champions = load_drivers_champions(&db.conn, &category_ids);

    // Os RECORDS comparam dentro da CATEGORIA, não do grupo.
    //
    // O grupo continua sendo o recorte da história — a trajetória, os marcos, a
    // linha do tempo e o movimento entre tiers só fazem sentido com a escada
    // inteira à vista. Mas o card responde "onde esta equipe está entre as que
    // correm com ela", e quem corre com ela é a categoria: numa Mazda Rookie, a
    // média e o "17º de 22" que vinham do grupo somavam campeonatos que esta
    // equipe nunca disputou.
    //
    // É também o que faz o card e a tabela de recordes falarem o mesmo número: a
    // tabela abre em "só a categoria", e o rank do card era do grupo.
    //
    // Categoria multiclasse recorta também pela classe do carro DESTA equipe,
    // lida dos fatos dela — na Production há três campeonatos, e o rank de uma
    // Mazda não se mede contra as Toyota.
    let record_class = selected_facts
        .iter()
        .find(|fact| fact.category == category && !fact.class.is_empty())
        .map(|fact| fact.class.clone());
    let record_facts: Vec<TeamRaceFact> = all_facts
        .iter()
        .filter(|fact| fact.category == category)
        .filter(|fact| match record_class.as_deref() {
            Some(class) => fact.class == class,
            None => true,
        })
        .cloned()
        .collect();
    let record_scope = match record_class.as_deref() {
        Some(class) => format!(
            "{} · {}",
            team_history_category_label(&category),
            multiclass_label(class)
        ),
        None => team_history_category_label(&category),
    };
    let aggregates = aggregate_team_history(&record_facts);
    let selected = aggregates.get(team_id).cloned().unwrap_or_default();
    let titles_by_team: HashMap<String, Vec<TeamTitleFact>> = group_titles
        .iter()
        .map(|(id, lista)| {
            let da_categoria: Vec<TeamTitleFact> = lista
                .iter()
                .filter(|title| title.category == category)
                .filter(|title| match record_class.as_deref() {
                    Some(class) => title.class == class,
                    None => true,
                })
                .cloned()
                .collect();
            (id.clone(), da_categoria)
        })
        .filter(|(_, lista)| !lista.is_empty())
        .collect();
    // A galeria de títulos continua sendo a do GRUPO: ela conta a história da
    // equipe, não a comparação — esconder um título de Mazda Championship da
    // ficha de uma equipe que subiu seria apagar o que ela fez.
    let selected_titles = group_titles.get(team_id).cloned().unwrap_or_default();
    let title_count = titles_by_team
        .get(team_id)
        .map(|titles| titles.len() as i32)
        .unwrap_or(0);

    let races = selected.races.max(0);
    let wins = selected.wins.max(0);
    let podiums = selected.podiums.max(0);
    let win_rate = percentage(wins, races);
    let podium_rate = percentage(podiums, races);
    // As temporadas do âncora acompanham os cards: elas ficam na mesma fileira do
    // cabeçalho que títulos, vitórias e pódios, e misturar recortes numa fileira
    // só é o jeito mais rápido de tornar quatro números incomparáveis entre si.
    let seasons = distinct_seasons(
        &record_facts
            .iter()
            .filter(|fact| fact.team_id == team_id)
            .cloned()
            .collect::<Vec<_>>(),
    );
    // Mas "tem histórico" continua sendo do GRUPO: uma equipe que subiu de tier
    // não tem corrida nenhuma na categoria de baixo, e o dossiê inteiro cairia
    // no estado de "sem histórico" por causa de um recorte que só vale para os
    // cards.
    let has_history = !selected_facts.is_empty();
    let sport = TeamHistorySport {
        seasons: season_count_label(seasons.len() as i32),
        current_streak: current_level_streak_label(&selected_facts),
        best_streak: best_real_streak_label(&selected_facts),
        podium_rate: format!("{podium_rate}%"),
        win_rate: format!("{win_rate}%"),
        races,
        wins,
        podiums,
    };

    let season_positions = load_team_season_positions(&db.conn, team_id);
    let (world_first_year, world_last_year) = load_world_year_span(&db.conn);
    let team_names = load_team_names(&db.conn);
    let current_season = load_current_season_number(&db.conn);

    Ok(TeamHistoryDossier {
        team_id: team_id.to_string(),
        category: category.clone(),
        record_scope: record_scope.clone(),
        has_history,
        records: vec![
            count_record(
                "titles",
                rust_i18n::t!("team_dossier.records.titles").to_string(),
                title_count,
                rank_for_titles(&titles_by_team, &aggregates, team_id),
            ),
            count_record(
                "wins",
                rust_i18n::t!("team_dossier.records.wins").to_string(),
                wins,
                rank_for_aggregate(&aggregates, team_id, |entry| entry.wins as f64),
            ),
            count_record(
                "podiums",
                rust_i18n::t!("team_dossier.records.podiums").to_string(),
                podiums,
                rank_for_aggregate(&aggregates, team_id, |entry| entry.podiums as f64),
            ),
            rate_record(
                "podium_rate",
                rust_i18n::t!("team_dossier.records.podium_rate").to_string(),
                podium_rate,
                rank_for_aggregate(&aggregates, team_id, |entry| {
                    if entry.races > 0 {
                        entry.podiums as f64 / entry.races as f64
                    } else {
                        0.0
                    }
                }),
            ),
            rate_record(
                "win_rate",
                rust_i18n::t!("team_dossier.records.win_rate").to_string(),
                win_rate,
                rank_for_aggregate(&aggregates, team_id, |entry| {
                    if entry.races > 0 {
                        entry.wins as f64 / entry.races as f64
                    } else {
                        0.0
                    }
                }),
            ),
        ],
        sport,
        identity: build_real_team_identity(
            &db.conn,
            team_id,
            &category,
            &record_scope,
            &selected_facts,
            &aggregates,
            title_count,
        )?,
        management: build_real_team_management(&db.conn, team_id, &selected_facts)?,
        timeline: build_real_team_timeline(&selected_facts),
        title_categories: selected_titles
            .iter()
            .enumerate()
            .map(|(index, title)| {
                let champion =
                    drivers_champions.get(&format!("{}:{}", title.season_id, title.category));
                let champion_is_team = champion
                    .map(|champ| champ.team_id == team_id)
                    .unwrap_or(false);
                TeamHistoryTitleCategory {
                    category: team_history_category_label(&title.category),
                    category_id: title.category.clone(),
                    year: title.season_year.to_string(),
                    // A paleta rotativa continua aqui só para o v1. O v2 pinta o
                    // card com a cor da CATEGORIA, derivada do `category_id`
                    // acima — seis títulos na mesma categoria saíam em seis cores
                    // diferentes, sugerindo uma distinção que não existia. A
                    // paleta de categorias vive no frontend; duplicá-la no Rust
                    // criaria duas fontes de verdade para a mesma cor.
                    color: history_palette(index),
                    points: format!("{}", title.points.round() as i64),
                    wins: title.wins,
                    champion_driver: champion
                        .map(|champ| champ.driver.clone())
                        .unwrap_or_default(),
                    champion_team: if champion_is_team {
                        String::new()
                    } else {
                        champion.map(|champ| champ.team.clone()).unwrap_or_default()
                    },
                    champion_is_team,
                }
            })
            .collect(),
        category_path: build_real_category_path(&selected_facts),
        ownership_events: load_team_ownership_events(&db.conn, team_id)?,
        highlights: build_team_highlights(&selected_facts, &selected_titles, &season_positions),
        milestones: build_team_milestones(&selected_facts, &selected_titles),
        season_results: build_team_season_results(&selected_facts, &season_positions),
        recent_form: build_team_recent_form(&selected_facts),
        result_spread: build_team_result_spread(&selected_facts),
        lineup: load_team_lineup(&db.conn, team_id)?,
        reliability: load_team_reliability(&db.conn, team_id, &category_ids)?,
        outside_scope_seasons: load_team_seasons_outside_scope(&db.conn, team_id, &category_ids)
            .into_iter()
            .map(|(year, category_id)| TeamHistoryOutsideSeason {
                year: year.to_string(),
                category: team_history_category_label(&category_id),
                category_id,
            })
            .collect(),
        movement: build_team_movement(&selected_facts),
        world_first_year,
        world_last_year,
        // Recebe `all_facts`, e não `selected_facts`: o gráfico é a equipe CONTRA
        // o campo, e o campo já veio na mesma leitura — nenhuma consulta a mais.
        championship_run: build_team_championship_run(
            &all_facts,
            team_id,
            &team_names,
            current_season,
        ),
    })
}
