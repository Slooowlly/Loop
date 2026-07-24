//! Trajetoria do piloto: ano de estreia, linha do tempo de categorias, anos de carreira e os marcos exibidos na ficha.

use super::*;

pub(super) fn career_debut_year_from_archive(seasons: &[CareerSeasonArchiveRow], fallback_year: i32) -> i32 {
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

pub(super) fn build_category_timeline(
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

pub(super) fn build_driver_career_path_block(
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

