//! Régua de pontuação: soma por categoria, diversidade, eficiência e coroas de prestígio.

use super::*;

pub(super) fn total_stats(stats: &[CategoryStats]) -> CategoryStats {
    stats
        .iter()
        .fold(CategoryStats::default(), |mut total, entry| {
            total.points += entry.points;
            total.wins += entry.wins;
            total.podiums += entry.podiums;
            total.poles += entry.poles;
            total.races += entry.races;
            total.titles += entry.titles;
            total.dnfs += entry.dnfs;
            total
        })
}

pub(super) fn score_category_stats(stats: &CategoryStats) -> f64 {
    balanced_score(
        &stats.category,
        stats.titles,
        stats.wins,
        stats.podiums,
        stats.poles,
        stats.points,
        stats.races,
        stats.dnfs,
    )
}

pub(super) fn category_multiplier(category: &str) -> f64 {
    match category {
        "mazda_rookie" | "toyota_rookie" => 0.75,
        "mazda_amador" | "toyota_amador" => 0.85,
        "bmw_m2" => 0.95,
        "gt4" => 1.08,
        "production_challenger" => 1.12,
        "gt3" => 1.22,
        "lmp2" => 1.28,
        "endurance" => 1.30,
        _ => 1.0,
    }
}

pub(super) fn balanced_score(
    category: &str,
    titles: i32,
    wins: i32,
    podiums: i32,
    poles: i32,
    points: f64,
    races: i32,
    dnfs: i32,
) -> f64 {
    let normalized_points = points.max(0.0).sqrt() * 0.4;
    let race_bonus = (races.max(0) as f64).sqrt() * 0.5;
    // Título pesa mais que volume de vitória: o que conta é CONVERTER vitórias em
    // campeonato, não só vencer muito (título 650 vs vitória 40 ≈ 16 vitórias/título).
    let base = titles as f64 * 650.0
        + wins as f64 * 40.0
        + podiums as f64 * 4.0
        + poles as f64 * 7.0
        + normalized_points
        + race_bonus
        - dnfs.max(0) as f64 * 1.5;
    round_one(base.max(0.0) * category_multiplier(category))
}

/// Categorias-base (sem classe) em que o piloto foi CAMPEÃO ao menos uma vez.
pub(super) fn distinct_title_categories(stats: &[CategoryStats]) -> HashSet<String> {
    stats
        .iter()
        .filter(|entry| entry.titles > 0)
        .map(|entry| {
            entry
                .category
                .split(':')
                .next()
                .unwrap_or(&entry.category)
                .to_string()
        })
        .collect()
}

/// Bônus de amplitude: cada categoria DISTINTA conquistada além da primeira soma
/// 8% ao índice. Premia diversidade de títulos (subir a escada vencendo) em vez de
/// farmar a mesma categoria.
pub(super) fn diversity_multiplier(stats: &[CategoryStats]) -> f64 {
    let distinct = distinct_title_categories(stats).len();
    1.0 + 0.08 * distinct.saturating_sub(1) as f64
}

/// Se o piloto foi campeão em alguma CLASSE específica (ex.: "lmp2"), em qualquer
/// categoria. LMP2 não é categoria autônoma — só existe como classe da Endurance,
/// então "ganhou lmp2" = tem título com `class_name == "lmp2"`.
pub(super) fn won_class_title(stats: &[CategoryStats], class: &str) -> bool {
    stats
        .iter()
        .any(|entry| entry.titles > 0 && entry.class_name.as_deref() == Some(class))
}

/// Nº de CLASSES distintas em que o piloto foi campeão numa categoria-base
/// (usado nas especiais multiclasse: Production = mazda/toyota/bmw, Endurance =
/// gt4/gt3/lmp2). Título sem classe conta como 1.
pub(super) fn titled_class_count(stats: &[CategoryStats], base_category: &str) -> usize {
    stats
        .iter()
        .filter(|entry| {
            entry.titles > 0
                && entry.category.split(':').next().unwrap_or(&entry.category) == base_category
        })
        .map(|entry| entry.class_name.clone().unwrap_or_default())
        .collect::<HashSet<String>>()
        .len()
}

/// Coroas de prestígio (bônus fixos, acumulam). Hierarquia definida pelo user:
///   prod(1) < Cup Slam < GT Slam < Production Slam < GT Super Slam < Endurance Slam.
/// - Cup Slam   = campeão mazda_amador + toyota_amador + bmw_m2.
/// - Production = especial multiclasse (escala por classe; 3 classes = Production Slam).
/// - GT Slam    = gt4 + gt3; vira GT SUPER SLAM ao somar LMP2 (super substitui o slam).
/// - Endurance  = especial multiclasse (escala; 3 classes = Endurance Slam = ouro).
pub(super) fn crown_bonus(stats: &[CategoryStats]) -> f64 {
    let cats = distinct_title_categories(stats);
    let mut bonus = 0.0;

    // Cup Slam (entrada).
    if ["mazda_amador", "toyota_amador", "bmw_m2"]
        .iter()
        .all(|cat| cats.contains(*cat))
    {
        bonus += 1500.0;
    }

    // Production (especial), escalando até o Production Slam (3 classes).
    bonus += match titled_class_count(stats, "production_challenger") {
        0 => 0.0,
        1 => 800.0,
        2 => 1800.0,
        _ => 3500.0, // Production Slam (mazda + toyota + bmw)
    };

    // GT Slam (gt4 + gt3) → GT Super Slam ao somar LMP2. O super SUBSTITUI o slam.
    // LMP2 só existe como classe da Endurance → detecta pela classe, não pela base.
    if cats.contains("gt4") && cats.contains("gt3") {
        bonus += if won_class_title(stats, "lmp2") {
            5000.0
        } else {
            2500.0
        };
    }

    // Endurance (especial), escalando até o Endurance Slam (vale ouro).
    bonus += match titled_class_count(stats, "endurance") {
        0 => 0.0,
        1 => 2000.0,
        2 => 4500.0,
        _ => 8000.0, // Endurance Slam (gt4 + gt3 + lmp2)
    };

    bonus
}

/// Multiplicador de EFICIÊNCIA: premia quem converteu em título rápido (títulos por
/// temporada) e venceu muito por corrida. Só vale com volume mínimo (guarda contra
/// carreira-relâmpago) e tem teto. Sempre ≥ 1.0 — é upside, não pune carreira longa.
pub(super) fn efficiency_multiplier(stats: &[CategoryStats]) -> f64 {
    let total_wins: i32 = stats.iter().map(|entry| entry.wins).sum();
    let total_races: i32 = stats.iter().map(|entry| entry.races).sum();
    let total_titles: i32 = stats.iter().map(|entry| entry.titles).sum();
    let seasons = stats.iter().filter(|entry| entry.races > 0).count() as i32;
    if total_races < 30 || seasons < 3 {
        return 1.0;
    }
    let win_rate = total_wins as f64 / total_races.max(1) as f64;
    let title_rate = total_titles as f64 / seasons.max(1) as f64;
    (1.0 + 0.7 * win_rate + 1.4 * title_rate).clamp(1.0, 2.6)
}

/// Índice histórico unificado — ativos e aposentados pontuam pela MESMA régua
/// (soma por categoria × diversidade × eficiência + bônus de coroa).
pub(super) fn compute_historical_index(stats: &[CategoryStats]) -> f64 {
    let base: f64 = stats.iter().map(score_category_stats).sum();
    let value =
        base * diversity_multiplier(stats) * efficiency_multiplier(stats) + crown_bonus(stats);
    round_one(value.max(0.0))
}

/// Índice histórico (pedigree de CARREIRA) de UM piloto, para uso fora da tela de ranking
/// (ex: valor de mercado nas propostas formais). Mesma régua de `compute_historical_index`,
/// mas ignora títulos de construtores (aproximação barata: não constrói o mapa global de
/// títulos de equipe). Devolve 0.0 se não há arquivo de temporadas.
pub(crate) fn historical_index_for_driver(
    conn: &Connection,
    driver: &Driver,
) -> Result<f64, String> {
    let category = regular_category(driver.categoria_atual.as_deref(), None);
    // Um piloto só: filtra o agregado de `race_results` em vez de varrer a tabela.
    let real_career = RealCareerIndex::for_driver(conn, &driver.id)?;
    let stats = load_driver_category_stats(
        conn,
        driver,
        category.as_deref(),
        &TeamTitleStatsByDriver::new(),
        &real_career,
    )?;
    Ok(compute_historical_index(&stats))
}
