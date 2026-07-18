#![allow(dead_code)]

pub struct MultiClassInfo {
    pub class_name: &'static str,
    pub num_equipes: u8,
    pub car_categoria: &'static str,
    pub multiplicador: f64,
}

pub struct CategoryConfig {
    pub id: &'static str,
    pub nome: &'static str,
    pub nome_curto: &'static str,
    pub tier: u8,
    pub nivel: &'static str,
    pub num_equipes: u8,
    pub pilotos_por_equipe: u8,
    pub grid_total: u8,
    pub corridas_por_temporada: u8,
    pub duracao_corrida_min: u8,
    pub monomarca: bool,
    pub multi_classe: bool,
    pub licenca_necessaria: Option<u8>,
    pub usa_pistas_gratuitas: bool,
    pub pistas_fixas: u8,
    pub pistas_variaveis: u8,
    pub classes: &'static [MultiClassInfo],
}

pub type CategoryDefinition = CategoryConfig;

static PRODUCTION_CLASSES: [MultiClassInfo; 3] = [
    MultiClassInfo {
        class_name: "mazda",
        num_equipes: 6,
        car_categoria: "mazda_amador",
        multiplicador: 1.00,
    },
    MultiClassInfo {
        class_name: "toyota",
        num_equipes: 6,
        car_categoria: "toyota_amador",
        multiplicador: 1.00,
    },
    MultiClassInfo {
        class_name: "bmw",
        num_equipes: 6,
        car_categoria: "bmw_m2",
        multiplicador: 1.05,
    },
];

static ENDURANCE_CLASSES: [MultiClassInfo; 3] = [
    MultiClassInfo {
        class_name: "gt4",
        num_equipes: 6,
        car_categoria: "gt4",
        multiplicador: 0.85,
    },
    MultiClassInfo {
        class_name: "gt3",
        num_equipes: 6,
        car_categoria: "gt3",
        multiplicador: 1.00,
    },
    MultiClassInfo {
        class_name: "lmp2",
        num_equipes: 6,
        car_categoria: "lmp2",
        multiplicador: 1.30,
    },
];

static EMPTY_CLASSES: [MultiClassInfo; 0] = [];

static LMP2_REFERENCE_CATEGORY: CategoryConfig = CategoryConfig {
    id: "lmp2",
    nome: "LMP2 Prototype Championship",
    nome_curto: "LMP2",
    tier: 5,
    nivel: "Elite",
    num_equipes: 6,
    pilotos_por_equipe: 2,
    grid_total: 12,
    corridas_por_temporada: 10,
    duracao_corrida_min: 60,
    monomarca: false,
    multi_classe: false,
    licenca_necessaria: Some(4),
    usa_pistas_gratuitas: false,
    pistas_fixas: 4,
    pistas_variaveis: 6,
    classes: &EMPTY_CLASSES,
};

pub static CATEGORIES: [CategoryConfig; 9] = [
    CategoryConfig {
        id: "mazda_rookie",
        nome: "Mazda MX-5 Rookie Cup",
        nome_curto: "Mazda Rookie",
        tier: 0,
        nivel: "Rookie",
        num_equipes: 6,
        pilotos_por_equipe: 2,
        grid_total: 12,
        corridas_por_temporada: 5,
        duracao_corrida_min: 15,
        monomarca: true,
        multi_classe: false,
        licenca_necessaria: None,
        usa_pistas_gratuitas: true,
        pistas_fixas: 0,
        pistas_variaveis: 5,
        classes: &EMPTY_CLASSES,
    },
    CategoryConfig {
        id: "toyota_rookie",
        nome: "Toyota GR86 Rookie Cup",
        nome_curto: "Toyota Rookie",
        tier: 0,
        nivel: "Rookie",
        num_equipes: 6,
        pilotos_por_equipe: 2,
        grid_total: 12,
        corridas_por_temporada: 5,
        duracao_corrida_min: 15,
        monomarca: true,
        multi_classe: false,
        licenca_necessaria: None,
        usa_pistas_gratuitas: true,
        pistas_fixas: 0,
        pistas_variaveis: 5,
        classes: &EMPTY_CLASSES,
    },
    CategoryConfig {
        id: "mazda_amador",
        nome: "Mazda MX-5 Championship",
        nome_curto: "Mazda Championship",
        tier: 1,
        nivel: "Amador",
        num_equipes: 10,
        pilotos_por_equipe: 2,
        grid_total: 20,
        corridas_por_temporada: 8,
        duracao_corrida_min: 25,
        monomarca: true,
        multi_classe: false,
        licenca_necessaria: Some(0),
        usa_pistas_gratuitas: true,
        pistas_fixas: 2,
        pistas_variaveis: 6,
        classes: &EMPTY_CLASSES,
    },
    CategoryConfig {
        id: "toyota_amador",
        nome: "Toyota GR86 Cup",
        nome_curto: "Toyota Cup",
        tier: 1,
        nivel: "Amador",
        num_equipes: 10,
        pilotos_por_equipe: 2,
        grid_total: 20,
        corridas_por_temporada: 8,
        duracao_corrida_min: 25,
        monomarca: true,
        multi_classe: false,
        licenca_necessaria: Some(0),
        usa_pistas_gratuitas: true,
        pistas_fixas: 2,
        pistas_variaveis: 6,
        classes: &EMPTY_CLASSES,
    },
    CategoryConfig {
        id: "bmw_m2",
        nome: "BMW M2 CS Racing",
        nome_curto: "BMW M2",
        tier: 2,
        nivel: "Pro",
        num_equipes: 10,
        pilotos_por_equipe: 2,
        grid_total: 20,
        corridas_por_temporada: 8,
        duracao_corrida_min: 25,
        monomarca: true,
        multi_classe: false,
        licenca_necessaria: Some(1),
        usa_pistas_gratuitas: true,
        pistas_fixas: 2,
        pistas_variaveis: 6,
        classes: &EMPTY_CLASSES,
    },
    CategoryConfig {
        id: "production_challenger",
        nome: "Production Car Challenger",
        nome_curto: "Production",
        tier: 2,
        nivel: "Especial",
        num_equipes: 18,
        pilotos_por_equipe: 2,
        grid_total: 36,
        corridas_por_temporada: 10,
        duracao_corrida_min: 30,
        monomarca: false,
        multi_classe: true,
        licenca_necessaria: Some(1),
        usa_pistas_gratuitas: true,
        pistas_fixas: 2,
        pistas_variaveis: 8,
        classes: &PRODUCTION_CLASSES,
    },
    CategoryConfig {
        id: "gt4",
        nome: "GT4 Series",
        nome_curto: "GT4",
        tier: 3,
        nivel: "Super Pro",
        num_equipes: 10,
        pilotos_por_equipe: 2,
        grid_total: 20,
        corridas_por_temporada: 10,
        duracao_corrida_min: 30,
        monomarca: false,
        multi_classe: false,
        licenca_necessaria: Some(2),
        usa_pistas_gratuitas: false,
        pistas_fixas: 3,
        pistas_variaveis: 7,
        classes: &EMPTY_CLASSES,
    },
    CategoryConfig {
        id: "gt3",
        nome: "GT3 Championship",
        nome_curto: "GT3",
        tier: 4,
        nivel: "Master",
        num_equipes: 14,
        pilotos_por_equipe: 2,
        grid_total: 28,
        corridas_por_temporada: 14,
        duracao_corrida_min: 50,
        monomarca: false,
        multi_classe: false,
        licenca_necessaria: Some(3),
        usa_pistas_gratuitas: false,
        pistas_fixas: 4,
        pistas_variaveis: 10,
        classes: &EMPTY_CLASSES,
    },
    CategoryConfig {
        id: "endurance",
        nome: "Endurance Championship",
        nome_curto: "Endurance",
        tier: 6,
        nivel: "Especial",
        num_equipes: 18,
        pilotos_por_equipe: 2,
        grid_total: 36,
        corridas_por_temporada: 6,
        duracao_corrida_min: 0,
        monomarca: false,
        multi_classe: true,
        licenca_necessaria: Some(4),
        usa_pistas_gratuitas: false,
        pistas_fixas: 2,
        pistas_variaveis: 4,
        classes: &ENDURANCE_CLASSES,
    },
];

pub const CALENDAR_CONFLICTS: [(&str, &str); 2] = [
    ("mazda_rookie", "toyota_rookie"),
    ("mazda_amador", "toyota_amador"),
];

pub fn get_category(id: &str) -> Option<&'static CategoryConfig> {
    get_category_config(id)
}

pub fn get_category_config(id: &str) -> Option<&'static CategoryConfig> {
    CATEGORIES
        .iter()
        .find(|category| category.id == id)
        .or_else(|| (id == "lmp2").then_some(&LMP2_REFERENCE_CATEGORY))
}

pub fn get_all_categories() -> &'static [CategoryConfig] {
    &CATEGORIES
}

pub fn get_categories_by_tier(tier: u8) -> Vec<&'static CategoryConfig> {
    CATEGORIES
        .iter()
        .filter(|category| category.tier == tier)
        .collect()
}

pub fn has_calendar_conflict(cat_a: &str, cat_b: &str) -> bool {
    CALENDAR_CONFLICTS.iter().any(|(left, right)| {
        (cat_a == *left && cat_b == *right) || (cat_a == *right && cat_b == *left)
    })
}

pub fn get_feeder_categories(id: &str) -> Vec<&'static str> {
    match id {
        "mazda_amador" => vec!["mazda_rookie"],
        "toyota_amador" => vec!["toyota_rookie"],
        "bmw_m2" => vec!["mazda_amador", "toyota_amador"],
        "production_challenger" => vec!["mazda_amador", "toyota_amador", "bmw_m2"],
        "gt4" => vec![
            "bmw_m2",
            "production_challenger",
            "mazda_amador",
            "toyota_amador",
        ],
        "gt3" => vec!["gt4"],
        // Endurance recruta SÓ do gt3 (não mais do gt4): o gt4 não cria gente pro
        // ápice direto. Antes o feeder [gt4, gt3] fazia o endurance skimmar o craque
        // do gt4 pulando o gt3 — a elite contornava o GT3 inteiro e ele deflacionava.
        // Com [gt3], o único caminho ao topo passa pelo gt3, que vira a antecâmara do
        // endurance e acumula a elite (a "circulação gt3↔endurance" do design).
        "endurance" => vec!["gt3"],
        _ => vec![],
    }
}

pub fn is_especial(cat_id: &str) -> bool {
    matches!(cat_id, "production_challenger" | "endurance")
}

pub fn runs_in_special_phase(cat_id: &str) -> bool {
    is_especial(cat_id)
}

pub fn is_multiclass_category(cat_id: &str) -> bool {
    get_category_config(cat_id).is_some_and(|category| category.multi_classe)
}

pub fn uses_regular_teams(cat_id: &str) -> bool {
    cat_id != "lmp2" && get_category_config(cat_id).is_some()
}

pub fn uses_regular_contracts(cat_id: &str) -> bool {
    cat_id != "lmp2" && get_category_config(cat_id).is_some()
}

pub fn is_valid_competitive_division(category_id: &str, class_id: Option<&str>) -> bool {
    let category_id = category_id.trim();
    let class_id = normalized_class_id(class_id);

    match category_id {
        "production_challenger" => {
            matches!(class_id.as_deref(), Some("mazda" | "toyota" | "bmw"))
        }
        "endurance" => matches!(class_id.as_deref(), Some("gt4" | "gt3" | "lmp2")),
        "mazda_rookie" | "toyota_rookie" | "mazda_amador" | "toyota_amador" | "bmw_m2" | "gt4"
        | "gt3" => class_id.is_none(),
        "lmp2" => false,
        _ => false,
    }
}

pub fn competitive_division_key(category_id: &str, class_id: Option<&str>) -> String {
    let category_id = category_id.trim();
    match normalized_class_id(class_id) {
        Some(class_id) if !class_id.is_empty() => format!("{category_id}:{class_id}"),
        _ => category_id.to_string(),
    }
}

pub fn competitive_division_label(category_id: &str, class_id: Option<&str>) -> String {
    let category_id = category_id.trim();
    let class_id = normalized_class_id(class_id);

    match (category_id, class_id.as_deref()) {
        ("endurance", Some("gt3")) => "GT3 Endurance".to_string(),
        ("endurance", Some("gt4")) => "GT4 Endurance".to_string(),
        ("endurance", Some("lmp2")) => "LMP2".to_string(),
        ("production_challenger", Some("mazda")) => "Mazda Production".to_string(),
        ("production_challenger", Some("toyota")) => "Toyota Production".to_string(),
        ("production_challenger", Some("bmw")) => "BMW Production".to_string(),
        ("gt3", None) => "GT3".to_string(),
        ("gt4", None) => "GT4".to_string(),
        ("lmp2", None) => "LMP2".to_string(),
        _ => get_category_config(category_id)
            .map(|category| category.nome_curto.to_string())
            .unwrap_or_else(|| competitive_division_key(category_id, class_id.as_deref())),
    }
}

fn normalized_class_id(class_id: Option<&str>) -> Option<String> {
    class_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn get_target_categories(id: &str) -> Vec<&'static str> {
    match id {
        "mazda_rookie" => vec!["mazda_amador"],
        "toyota_rookie" => vec!["toyota_amador"],
        "mazda_amador" => vec!["bmw_m2", "gt4"],
        "toyota_amador" => vec!["bmw_m2", "gt4"],
        "bmw_m2" => vec!["gt4"],
        "production_challenger" => vec!["gt4"],
        "gt4" => vec!["gt3"],
        "gt3" => vec!["endurance"],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_category_config_gt3() {
        let config = get_category_config("gt3").expect("gt3 should exist");
        assert_eq!(config.tier, 4);
        assert_eq!(config.num_equipes, 14);
        assert_eq!(config.grid_total, 28);
    }

    #[test]
    fn test_get_category_config_production_challenger() {
        let config = get_category_config("production_challenger")
            .expect("production_challenger should exist");
        assert_eq!(config.tier, 2);
        assert_eq!(config.num_equipes, 18);
        assert_eq!(config.grid_total, 36);
        assert!(config.multi_classe);
        assert_eq!(config.nivel, "Especial");
    }

    #[test]
    fn test_special_multiclass_capacity_is_reserved_for_real_divisions() {
        let production = get_category_config("production_challenger")
            .expect("production_challenger should exist");
        let production_classes: Vec<(&str, u8, &str)> = production
            .classes
            .iter()
            .map(|class| (class.class_name, class.num_equipes, class.car_categoria))
            .collect();
        assert_eq!(
            production_classes,
            vec![
                ("mazda", 6, "mazda_amador"),
                ("toyota", 6, "toyota_amador"),
                ("bmw", 6, "bmw_m2"),
            ]
        );
        assert_eq!(
            production
                .classes
                .iter()
                .map(|class| class.num_equipes)
                .sum::<u8>(),
            production.num_equipes
        );

        let endurance = get_category_config("endurance").expect("endurance should exist");
        let endurance_classes: Vec<(&str, u8, &str)> = endurance
            .classes
            .iter()
            .map(|class| (class.class_name, class.num_equipes, class.car_categoria))
            .collect();
        assert_eq!(
            endurance_classes,
            vec![("gt4", 6, "gt4"), ("gt3", 6, "gt3"), ("lmp2", 6, "lmp2")]
        );
        assert_eq!(endurance.num_equipes, 18);
        assert_eq!(endurance.grid_total, 36);
        assert_eq!(
            endurance
                .classes
                .iter()
                .map(|class| class.num_equipes)
                .sum::<u8>(),
            endurance.num_equipes
        );
    }

    #[test]
    fn test_get_category_config_invalid() {
        assert!(get_category_config("inexistente").is_none());
    }

    #[test]
    fn test_categories_by_tier_0() {
        let ids: Vec<&str> = get_categories_by_tier(0)
            .into_iter()
            .map(|category| category.id)
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"mazda_rookie"));
        assert!(ids.contains(&"toyota_rookie"));
    }

    #[test]
    fn test_calendar_conflict_rookies() {
        assert!(has_calendar_conflict("mazda_rookie", "toyota_rookie"));
    }

    #[test]
    fn test_calendar_conflict_unrelated() {
        assert!(!has_calendar_conflict("mazda_rookie", "gt3"));
    }

    #[test]
    fn test_all_categories_count() {
        assert_eq!(get_all_categories().len(), 9);
    }

    #[test]
    fn test_lmp2_is_reference_class_inside_endurance() {
        let gt3_config = get_category_config("gt3").expect("gt3 should exist");
        let config = get_category_config("lmp2").expect("lmp2 should exist");
        assert_eq!(gt3_config.licenca_necessaria, Some(3));
        assert_eq!(config.nome_curto, "LMP2");
        assert_eq!(config.tier, 5);
        assert_eq!(config.nivel, "Elite");
        assert_eq!(config.num_equipes, 6);
        assert_eq!(config.grid_total, 12);
        assert!(!config.multi_classe);
        assert_eq!(config.classes.len(), 0);
        assert_eq!(config.licenca_necessaria, Some(4));
        let endurance = get_category_config("endurance").expect("endurance should exist");
        assert!(endurance
            .classes
            .iter()
            .any(|class| class.class_name == "lmp2" && class.car_categoria == "lmp2"));
        assert!(!get_all_categories()
            .iter()
            .any(|category| category.id == "lmp2"));
        assert_eq!(get_target_categories("gt3"), vec!["endurance"]);
        assert_eq!(get_feeder_categories("endurance"), vec!["gt3"]);
    }

    #[test]
    fn test_is_especial() {
        assert!(is_especial("production_challenger"));
        assert!(is_especial("endurance"));
        assert!(!is_especial("lmp2"));
        assert!(!is_especial("gt3"));
        assert!(!is_especial("mazda_rookie"));
    }

    #[test]
    fn test_specific_category_semantics_keep_legacy_special_flag_separate() {
        assert!(runs_in_special_phase("production_challenger"));
        assert!(runs_in_special_phase("endurance"));
        assert!(!runs_in_special_phase("lmp2"));

        assert!(is_multiclass_category("production_challenger"));
        assert!(is_multiclass_category("endurance"));
        assert!(!is_multiclass_category("gt3"));

        assert!(uses_regular_teams("production_challenger"));
        assert!(uses_regular_teams("endurance"));
        assert!(!uses_regular_teams("lmp2"));
        assert!(!uses_regular_teams("inexistente"));

        assert!(uses_regular_contracts("production_challenger"));
        assert!(uses_regular_contracts("endurance"));
        assert!(!uses_regular_contracts("lmp2"));
    }

    #[test]
    fn competitive_division_validation_requires_class_for_meta_categories() {
        for category in [
            "mazda_rookie",
            "toyota_rookie",
            "mazda_amador",
            "toyota_amador",
            "bmw_m2",
            "gt4",
            "gt3",
        ] {
            assert!(is_valid_competitive_division(category, None));
        }

        assert!(is_valid_competitive_division(
            "production_challenger",
            Some("mazda")
        ));
        assert!(is_valid_competitive_division(
            "production_challenger",
            Some("toyota")
        ));
        assert!(is_valid_competitive_division(
            "production_challenger",
            Some("bmw")
        ));
        assert!(is_valid_competitive_division("endurance", Some("gt4")));
        assert!(is_valid_competitive_division("endurance", Some("gt3")));
        assert!(is_valid_competitive_division("endurance", Some("lmp2")));

        assert!(!is_valid_competitive_division(
            "production_challenger",
            None
        ));
        assert!(!is_valid_competitive_division("endurance", None));
        assert!(!is_valid_competitive_division("lmp2", None));
        assert!(!is_valid_competitive_division(
            "production_challenger",
            Some("gt3")
        ));
        assert!(!is_valid_competitive_division("endurance", Some("mazda")));
        assert!(!is_valid_competitive_division("inexistente", None));
    }

    #[test]
    fn competitive_division_key_and_label_keep_regular_and_multiclass_separate() {
        assert_eq!(competitive_division_key("gt3", None), "gt3");
        assert_eq!(
            competitive_division_key("endurance", Some("gt3")),
            "endurance:gt3"
        );
        assert_eq!(
            competitive_division_key("production_challenger", Some("mazda")),
            "production_challenger:mazda"
        );

        assert_eq!(competitive_division_label("gt3", None), "GT3");
        assert_eq!(
            competitive_division_label("endurance", Some("gt3")),
            "GT3 Endurance"
        );
        assert_eq!(
            competitive_division_label("endurance", Some("gt4")),
            "GT4 Endurance"
        );
        assert_eq!(
            competitive_division_label("endurance", Some("lmp2")),
            "LMP2"
        );
        assert_eq!(
            competitive_division_label("production_challenger", Some("mazda")),
            "Mazda Production"
        );
        assert_eq!(
            competitive_division_label("production_challenger", Some("toyota")),
            "Toyota Production"
        );
        assert_eq!(
            competitive_division_label("production_challenger", Some("bmw")),
            "BMW Production"
        );
        assert_eq!(competitive_division_label("lmp2", None), "LMP2");
    }
}
