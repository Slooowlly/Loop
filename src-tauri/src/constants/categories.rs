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
        num_equipes: 5,
        car_categoria: "mazda_amador",
        multiplicador: 1.00,
    },
    MultiClassInfo {
        class_name: "toyota",
        num_equipes: 5,
        car_categoria: "toyota_amador",
        multiplicador: 1.00,
    },
    MultiClassInfo {
        class_name: "bmw",
        num_equipes: 5,
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
        num_equipes: 5,
        car_categoria: "lmp2",
        multiplicador: 1.30,
    },
];

static EMPTY_CLASSES: [MultiClassInfo; 0] = [];

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
        num_equipes: 15,
        pilotos_por_equipe: 2,
        grid_total: 30,
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
        tier: 5,
        nivel: "Especial",
        num_equipes: 17,
        pilotos_por_equipe: 2,
        grid_total: 34,
        corridas_por_temporada: 6,
        duracao_corrida_min: 0,
        monomarca: false,
        multi_classe: true,
        licenca_necessaria: Some(3),
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
    CATEGORIES.iter().find(|category| category.id == id)
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
        "endurance" => vec!["gt3"],
        _ => vec![],
    }
}

pub fn is_especial(cat_id: &str) -> bool {
    matches!(cat_id, "production_challenger" | "endurance")
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
        assert_eq!(config.num_equipes, 15);
        assert!(config.multi_classe);
        assert_eq!(config.nivel, "Especial");
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
    fn test_is_especial() {
        assert!(is_especial("production_challenger"));
        assert!(is_especial("endurance"));
        assert!(!is_especial("gt3"));
        assert!(!is_especial("mazda_rookie"));
    }
}
