//! Trajetoria do piloto: ano de estreia, linha do tempo de categorias, anos de carreira e os marcos exibidos na ficha.

use super::*;

pub(super) fn career_debut_year_from_archive(
    seasons: &[CareerSeasonArchiveRow],
    fallback_year: i32,
) -> i32 {
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

pub(super) fn inferred_debut_year_from_driver(driver: &Driver, current_year: i32) -> i32 {
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
pub(super) fn career_years_from_debut(
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

/// A divisao que uma temporada arquivada representa na escada.
///
/// As categorias do bloco especial (`endurance`, `production_challenger`) eram
/// DESCARTADAS aqui, e com elas anos inteiros de carreira: um piloto que passou
/// catorze temporadas na Endurance aparecia com a escada vazia de 2012 a 2025 —
/// enquanto o card ao lado listava os titulos que ele ganhou justamente nesses
/// anos. Pior, a temporada EM CURSO na Endurance era desenhada (ela chega por
/// `categoria_atual`, ja com a classe), entao a mesma categoria existia como
/// presente e sumia como passado.
///
/// A especial nao e uma exibicao fora do campeonato: ela tem classe, calendario
/// e campeao proprios, e no arquivo do piloto ela vem como a categoria da
/// temporada inteira. O que ela precisa e da CLASSE para virar a divisao de
/// verdade (`endurance` + `gt3` -> `endurance:gt3`), que e a mesma chave com que
/// o ano corrente ja e desenhado.
pub(super) fn timeline_division_key(categoria: &str, classe: Option<&str>) -> Option<String> {
    let categoria = categoria.trim();
    if categoria.is_empty() {
        return None;
    }
    if let Some((base, class_in_key)) = categoria.split_once(':') {
        return regular_division_key(base, Some(class_in_key));
    }
    if !categories::is_especial(categoria) {
        return Some(categoria.to_string());
    }
    // Snapshot antigo, sem classe: fica a categoria-base. Perde a classe, mas
    // pinta o ano — uma celula generica diz mais que um buraco.
    regular_division_key(categoria, classe).or_else(|| Some(categoria.to_string()))
}

pub(super) fn build_category_timeline(
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
        let Some(category) = timeline_division_key(&season.categoria, season.classe.as_deref())
        else {
            continue;
        };
        if let Some(last) = timeline.last_mut() {
            if last.categoria == category {
                last.ano_fim = season.ano;
                continue;
            }
        }

        timeline.push(DriverCareerCategoryStint {
            categoria: category,
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

pub(super) fn build_driver_career_path_block(
    conn: &Connection,
    driver: &Driver,
    team: Option<&Team>,
    contract: Option<&Contract>,
    category_id: Option<&str>,
    season: &Season,
    hoje: Option<PosicaoDeHoje>,
) -> Result<DriverCareerPathBlock, String> {
    let current_year = season.ano;
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
        equipe_estreia: debut_team_name(&historico).or_else(|| {
            // Sem corrida-a-corrida nao ha estreia registrada: sobra o piloto que
            // ainda nao largou, e ai o contrato do primeiro ano E a estreia. O
            // `team` cru fica so para o save sem contrato — nunca para quem tem
            // historico, que era de onde vinha a equipe ATUAL travestida de
            // estreia numa carreira de treze anos.
            contract
                .filter(|value| value.temporada_inicio <= 1)
                .map(|value| value.equipe_nome.clone())
                .or_else(|| team.map(|value| value.nome.clone()))
        }),
        categoria_atual: category_id.map(str::to_string),
        categorias_timeline: build_category_timeline(&season_archive, category_id, current_year),
        temporadas_na_categoria: driver.temporadas_na_categoria as i32,
        corridas_na_categoria: driver.corridas_na_categoria as i32,
        titulos: driver.stats_carreira.titulos as i32,
        foi_campeao: driver.stats_carreira.titulos > 0,
        titulos_detalhe: build_driver_title_entries(
            &season_archive,
            &load_title_team_lookup(conn, &season_archive)?,
        ),
        historico,
        marcos,
        curva_campeonato: build_driver_championship_curve(
            conn,
            &driver.id,
            &season_archive,
            season.numero,
            season.ano,
            category_id,
            team,
            hoje,
        ),
    })
}

/// A equipe da PRIMEIRA corrida da carreira, lida do mesmo detalhe que o hover
/// de "Tempo de carreira" mostra.
///
/// A fonte e `detalhes["tempo_carreira"]`, cuja primeira linha e `races.first()`
/// (ver `build_career_details`). Ler dali — e nao recontar o corrida-a-corrida —
/// e o que garante que a etiqueta da linha do tempo e o hover nunca digam
/// equipes diferentes: e literalmente o mesmo dado.
fn debut_team_name(historico: &DriverCareerHistoryBlock) -> Option<String> {
    historico
        .detalhes
        .get("tempo_carreira")?
        .first()?
        .equipe
        .clone()
}

/// Uma temporada do arquivo virou titulo?
///
/// A regra e `valid_archived_title_count`, a MESMA do ranking mundial: uma linha
/// com posicao 1 mas sem nenhuma corrida disputada nao e um campeonato, e a
/// ficha nao pode contar diferente da tabela que ordena o mundo.
pub(super) fn archived_season_is_title(season: &CareerSeasonArchiveRow) -> bool {
    valid_archived_title_count(
        season.titulos,
        season.posicao_campeonato,
        season.pontos,
        season.vitorias,
        season.podios,
        season.poles,
        season.corridas,
    ) > 0
}

/// Nome e cor das equipes com que o piloto foi campeao. So dos ids que aparecem
/// nos titulos — sao um a tres na carreira inteira, entao nao vale varrer a
/// tabela de equipes. A identidade e a ATUAL do `team_id`: o nome que a equipe
/// tinha naquele ano nao e arquivado em lugar nenhum.
pub(super) fn load_title_team_lookup(
    conn: &Connection,
    season_archive: &[CareerSeasonArchiveRow],
) -> Result<HashMap<String, (String, String)>, String> {
    let mut lookup = HashMap::new();
    for season in season_archive
        .iter()
        .filter(|s| archived_season_is_title(s))
    {
        let Some(team_id) = season.equipe_id.as_deref() else {
            continue;
        };
        if lookup.contains_key(team_id) {
            continue;
        }
        let team = team_queries::get_team_by_id(conn, team_id)
            .map_err(|e| format!("Falha ao carregar equipe de titulo do piloto: {e}"))?;
        if let Some(team) = team {
            lookup.insert(team_id.to_string(), (team.nome, team.cor_primaria));
        }
    }
    Ok(lookup)
}

/// Um titulo por entrada, com ano, categoria e equipe, do mais recente para o
/// mais antigo. Titulos de construtores ficam de fora — sao da equipe, nao do
/// piloto.
pub(super) fn build_driver_title_entries(
    season_archive: &[CareerSeasonArchiveRow],
    team_lookup: &HashMap<String, (String, String)>,
) -> Vec<DriverTitleEntry> {
    let mut entries: Vec<DriverTitleEntry> = season_archive
        .iter()
        .filter(|season| archived_season_is_title(season))
        .map(|season| {
            let team = season
                .equipe_id
                .as_deref()
                .and_then(|team_id| team_lookup.get(team_id));
            DriverTitleEntry {
                ano: season.ano,
                categoria: season.categoria.clone(),
                equipe: team.map(|(nome, _)| nome.clone()),
                equipe_cor: team.map(|(_, cor)| cor.clone()),
            }
        })
        .collect();
    entries.sort_by(|left, right| right.ano.cmp(&left.ano));
    entries
}
