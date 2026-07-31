use super::*;

#[test]
fn test_team_templates_gt3_count() {
    assert_eq!(get_team_templates("gt3").len(), 14);
}

fn assert_category_tiers(
    category: &str,
    top_min_performance: f64,
    mid_min_performance: f64,
    expected_top: usize,
    expected_mid: usize,
    expected_bottom: usize,
    check_budget_tiers: bool,
) {
    let teams = get_team_templates(category);

    let top: Vec<_> = teams
        .iter()
        .copied()
        .filter(|team| team.car_performance_base >= top_min_performance)
        .collect();
    let mid: Vec<_> = teams
        .iter()
        .copied()
        .filter(|team| {
            team.car_performance_base >= mid_min_performance
                && team.car_performance_base < top_min_performance
        })
        .collect();
    let bottom: Vec<_> = teams
        .iter()
        .copied()
        .filter(|team| team.car_performance_base < mid_min_performance)
        .collect();

    assert_eq!(
        top.len(),
        expected_top,
        "{category} deve ter {expected_top} equipes de topo"
    );
    assert_eq!(
        mid.len(),
        expected_mid,
        "{category} deve ter {expected_mid} equipes de meio"
    );
    assert_eq!(
        bottom.len(),
        expected_bottom,
        "{category} deve ter {expected_bottom} equipes abaixo da media"
    );

    let weakest_top_budget = top
        .iter()
        .map(|team| team.budget_base)
        .fold(f64::INFINITY, f64::min);
    let strongest_mid_budget = mid
        .iter()
        .map(|team| team.budget_base)
        .fold(f64::NEG_INFINITY, f64::max);
    let weakest_mid_budget = mid
        .iter()
        .map(|team| team.budget_base)
        .fold(f64::INFINITY, f64::min);
    let strongest_bottom_budget = bottom
        .iter()
        .map(|team| team.budget_base)
        .fold(f64::NEG_INFINITY, f64::max);

    let weakest_top_reputation = top
        .iter()
        .map(|team| team.reputacao_base)
        .fold(f64::INFINITY, f64::min);
    let strongest_mid_reputation = mid
        .iter()
        .map(|team| team.reputacao_base)
        .fold(f64::NEG_INFINITY, f64::max);
    let weakest_mid_reputation = mid
        .iter()
        .map(|team| team.reputacao_base)
        .fold(f64::INFINITY, f64::min);
    let strongest_bottom_reputation = bottom
        .iter()
        .map(|team| team.reputacao_base)
        .fold(f64::NEG_INFINITY, f64::max);

    // Em GT3 o orcamento foi deliberadamente desacoplado do car_performance
    // (re-tiering da balanca GT3): as fabricas reais carregam orcamentos altos
    // e as ficticias entram com caixa baixo, independentemente do car_performance
    // base. Por isso a monotonicidade de orcamento por tier so e exigida onde o
    // orcamento ainda acompanha o desempenho (ex.: gt4).
    if check_budget_tiers {
        assert!(
            weakest_top_budget > strongest_mid_budget,
            "{category} deve manter orcamento de topo acima do meio"
        );
        assert!(
            weakest_mid_budget > strongest_bottom_budget,
            "{category} deve manter orcamento de meio acima das equipes fracas"
        );
    }
    assert!(
        weakest_top_reputation > strongest_mid_reputation,
        "{category} deve manter reputacao de topo acima do meio"
    );
    assert!(
        weakest_mid_reputation > strongest_bottom_reputation,
        "{category} deve manter reputacao de meio acima das equipes fracas"
    );
}

#[test]
fn test_gt4_team_templates_have_balanced_tiers() {
    assert_category_tiers("gt4", 8.0, 5.0, 3, 4, 3, true);
}

#[test]
fn test_gt3_team_templates_have_balanced_tiers() {
    // Orcamento desacoplado do desempenho em GT3 (re-tiering da balanca):
    // checa contagem de tiers e monotonicidade de reputacao, mas nao de orcamento.
    assert_category_tiers("gt3", 13.0, 4.0, 5, 6, 3, false);
}

#[test]
fn test_gt3_factory_giants_are_title_contenders() {
    let teams = get_team_templates("gt3");

    for brand in [
        "Mercedes-AMG",
        "Porsche",
        "Ferrari",
        "McLaren",
        "Lamborghini",
    ] {
        let team = teams
            .iter()
            .find(|team| team.marca == Some(brand))
            .expect("GT3 factory brand should exist");

        assert!(
            team.car_performance_base >= 13.0,
            "{brand} deve estar no grupo de disputa pelo titulo"
        );
        // Pos re-tiering da balanca GT3 o orcamento foi desacoplado do desempenho:
        // a maioria das fabricas-titulares carrega caixa alto (Ferrari/AMG/McLaren/
        // Porsche >= 85), mas a Lamborghini foi rebaixada para 58 mantendo o
        // car_performance de disputa. O piso reflete esse novo patamar.
        assert!(
            team.budget_base >= 58.0,
            "{brand} deve ter orcamento de equipe grande"
        );
        assert!(
            team.reputacao_base >= 83.0,
            "{brand} deve ter reputacao de equipe grande"
        );
    }
}

#[test]
fn test_gt3_contains_required_manufacturers() {
    let brands: std::collections::HashSet<_> = get_team_templates("gt3")
        .into_iter()
        .filter_map(|team| team.marca)
        .collect();

    for brand in [
        "Ferrari",
        "Porsche",
        "Ford Mustang",
        "Chevrolet",
        "BMW",
        "Mercedes-AMG",
        "Lamborghini",
        "McLaren",
        "Acura",
        "Aston Martin",
        "Audi",
    ] {
        assert!(
            brands.contains(brand),
            "GT3 sem fabricante obrigatorio: {brand}"
        );
    }
}

#[test]
fn test_gt3_manufacturers_are_not_repeated_and_exotics_are_weakest() {
    let teams = get_team_templates("gt3");
    let mut brands = std::collections::HashSet::new();
    let exotic_teams: Vec<_> = teams
        .iter()
        .copied()
        .filter(|team| team.marca.is_none())
        .collect();

    for team in teams.iter().copied().filter(|team| team.marca.is_some()) {
        let brand = team.marca.expect("brand");
        assert!(brands.insert(brand), "fabricante GT3 repetido: {brand}");
    }

    assert_eq!(brands.len(), 11);
    assert_eq!(exotic_teams.len(), 3);
    assert!(exotic_teams
        .iter()
        .all(|team| team.car_performance_base <= 2.0));
}

#[test]
fn test_gt3_factory_colors_match_required_brands() {
    let teams = get_team_templates("gt3");
    let colors_for = |brand| {
        teams
            .iter()
            .find(|team| team.marca == Some(brand))
            .map(|team| (team.cor_primaria, team.cor_secundaria))
            .expect("GT3 brand should exist")
    };

    assert_eq!(colors_for("Ferrari"), ("#dc0000", "#dc0000"));
    assert_eq!(colors_for("Mercedes-AMG"), ("#00d2be", "#00d2be"));
    assert_eq!(colors_for("Lamborghini"), ("#ffd100", "#ffd100"));
    assert_eq!(colors_for("McLaren"), ("#ff8700", "#ff8700"));
    assert_eq!(colors_for("Porsche"), ("#111111", "#111111"));
}

#[test]
fn test_gt3_team_names_are_simple() {
    let names: Vec<_> = get_team_templates("gt3")
        .into_iter()
        .map(|team| team.nome)
        .collect();

    assert_eq!(
        names,
        vec![
            "Mercedes-AMG",
            "Porsche",
            "Ferrari",
            "McLaren",
            "Lamborghini",
            "BMW",
            "Audi",
            "Aston Martin",
            "Chevrolet",
            "Ford Mustang",
            "Acura",
            "Obsidian",
            "Kitsune",
            "Valkyrie",
        ]
    );
}

#[test]
fn test_bmw_m2_palette_has_one_unique_color_per_team() {
    let teams = get_team_templates("bmw_m2");
    let mut colors = std::collections::HashSet::new();

    for team in teams {
        assert_eq!(
            team.cor_primaria, team.cor_secundaria,
            "BMW M2 com primaria diferente da secundaria: {}",
            team.nome
        );
        assert!(
            colors.insert(team.cor_primaria),
            "cor primaria BMW M2 repetida: {} ({})",
            team.cor_primaria,
            team.nome
        );
    }

    assert_eq!(colors.len(), 10);
}

#[test]
fn test_bmw_m2_team_names_are_distinct() {
    let names: Vec<_> = get_team_templates("bmw_m2")
        .into_iter()
        .map(|team| team.nome)
        .collect();

    assert_eq!(
        names,
        vec![
            "Bayern Division",
            "M Power",
            "Blue Propeller",
            "Munich Speed Works",
            "Isar Track",
            "Eifel Sprint",
            "Corporate Express",
            "Roundel",
            "Southern Cross",
            "Black Forest Works",
        ]
    );
}

#[test]
fn test_team_templates_production_count() {
    assert_eq!(get_team_templates("production_challenger").len(), 18);
    assert_eq!(
        templates_for_class("production_challenger", "mazda").len(),
        6
    );
    assert_eq!(
        templates_for_class("production_challenger", "toyota").len(),
        6
    );
    assert_eq!(templates_for_class("production_challenger", "bmw").len(), 6);
}

#[test]
fn test_team_templates_lmp2_are_endurance_class_templates() {
    let lmp2_templates = templates_for_class("endurance", "lmp2");
    assert_eq!(lmp2_templates.len(), 6);
    assert!(lmp2_templates
        .iter()
        .all(|team| team.categoria == "endurance" && team.classe == Some("lmp2")));
    assert!(get_team_templates("lmp2").is_empty());
}

#[test]
fn test_new_production_and_endurance_templates_match_planned_rosters() {
    assert_named_color_roster(
        "production_challenger",
        "mazda",
        &[
            ("Aperture", "APR", "#1B9CFC"),
            ("Backmesa", "BKM", "#F26101"),
            ("Northgate", "NGT", "#023047"),
            ("Kestrel", "KST", "#E76F51"),
            ("Overland", "OVL", "#90BE6D"),
            ("Rookfield", "RKF", "#577590"),
        ],
    );
    assert_named_color_roster(
        "production_challenger",
        "toyota",
        &[
            ("Komorebi", "KMB", "#6D597A"),
            ("Nakatomi", "NKT", "#355070"),
            ("Hikari", "HKR", "#EAAC8B"),
            ("Redwell", "RDW", "#9F4C6E"),
            ("Ashford", "ASF", "#43AA8B"),
            ("Tetsu", "TET", "#D1AB2F"),
        ],
    );
    assert_named_color_roster(
        "production_challenger",
        "bmw",
        &[
            ("Nachtwerk", "NKW", "#8B060F"),
            ("Adler", "ADL", "#944EAB"),
            ("Eisen", "ESN", "#FFB703"),
            ("Kronstadt", "KRN", "#7F5539"),
            ("Vektor", "VKT", "#ADB5BD"),
            ("Lindenhaus", "LNH", "#1F7A57"),
        ],
    );
    assert_named_color_roster(
        "endurance",
        "gt4",
        &[
            ("Waypoint", "WPT", "#A47148"),
            ("Farpoint", "FPT", "#386641"),
            ("Northstar", "NTS", "#1A8F9E"),
            ("Mammoth", "MMT", "#C46A12"),
            ("Atlas", "ATL", "#4A4E69"),
            ("Outpost", "OPT", "#9A8C98"),
        ],
    );
    assert_named_color_roster(
        "endurance",
        "gt3",
        &[
            ("Solaris", "SLS", "#D58063"),
            ("Peregrine", "PRG", "#E0E1DD"),
            ("Arclight", "ARL", "#5E60CE"),
            ("Blackwell", "BWL", "#38B000"),
            ("Stratos", "STR", "#0B132B"),
            ("Helion", "HLN", "#FB8500"),
        ],
    );

    let meridian = templates_for_class("endurance", "lmp2")
        .into_iter()
        .find(|team| team.nome == "Meridian")
        .expect("Meridian LMP2 template should exist");
    assert_eq!(meridian.nome_curto, "MRD");
    assert_eq!(meridian.cor_primaria, "#B5179E");
    assert_eq!(meridian.cor_secundaria, "#B5179E");
}

#[test]
fn test_special_category_templates_have_classes_and_unique_short_names() {
    let special_templates: Vec<_> = get_all_team_templates()
        .iter()
        .filter(|team| matches!(team.categoria, "production_challenger" | "endurance"))
        .collect();
    let production_templates = get_team_templates("production_challenger");
    let endurance_templates = get_team_templates("endurance");
    let production_classes: std::collections::HashSet<_> = production_templates
        .iter()
        .filter_map(|team| team.classe)
        .collect();
    let endurance_classes: std::collections::HashSet<_> = endurance_templates
        .iter()
        .filter_map(|team| team.classe)
        .collect();

    assert_eq!(count_teams(), 102);
    assert_eq!(special_templates.len(), 36);
    assert_eq!(production_templates.len(), 18);
    assert_eq!(endurance_templates.len(), 18);
    assert!(special_templates.iter().all(|team| team.classe.is_some()));
    assert_eq!(
        production_classes,
        std::collections::HashSet::from(["mazda", "toyota", "bmw"])
    );
    assert_eq!(
        endurance_classes,
        std::collections::HashSet::from(["gt4", "gt3", "lmp2"])
    );

    let mut short_names = std::collections::HashSet::new();
    for team in get_all_team_templates() {
        assert!(
            short_names.insert(team.nome_curto),
            "sigla duplicada em templates: {} ({})",
            team.nome_curto,
            team.nome
        );
    }
}

fn templates_for_class(category: &str, class_name: &str) -> Vec<&'static TeamTemplate> {
    get_team_templates(category)
        .into_iter()
        .filter(|team| team.classe == Some(class_name))
        .collect()
}

fn assert_named_color_roster(category: &str, class_name: &str, expected: &[(&str, &str, &str)]) {
    let actual: Vec<_> = templates_for_class(category, class_name)
        .into_iter()
        .map(|team| (team.nome, team.nome_curto, team.cor_primaria))
        .collect();

    assert_eq!(actual, expected);
}

#[test]
fn test_special_class_reference_templates_use_regular_feeders() {
    assert_eq!(
        get_reference_team_template("production_challenger", Some("mazda"))
            .map(|team| team.categoria),
        Some("production_challenger")
    );
    assert_eq!(
        get_reference_team_template("endurance", Some("gt4")).map(|team| team.categoria),
        Some("endurance")
    );
    assert_eq!(
        get_reference_team_template("endurance", Some("lmp2")).map(|team| team.categoria),
        Some("endurance")
    );
}
