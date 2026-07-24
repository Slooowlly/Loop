//! Estruturas públicas do bloco especial e a tabela de classes convocadas —
//! compartilhadas por todas as etapas do pipeline.

use super::*;

// ── Estruturas públicas ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverAssignment {
    pub driver_id: String,
    pub team_id: String,
    pub papel: TeamRole,
    pub fonte: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridClasse {
    pub class_name: String,
    pub assignments: Vec<DriverAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvocationResult {
    pub grids: Vec<GridClasse>,
    pub total_contratos: usize,
    pub errors: Vec<String>,
}

// ── Classes convocadas ────────────────────────────────────────────────────────

/// Classes que participam da convocação especial.
pub(super) struct ClasseConfig {
    pub(super) special_category: &'static str,
    pub(super) class_name: &'static str,
    pub(super) feeder_category: &'static str,
}

pub(super) const CLASSES_CONVOCADAS: &[ClasseConfig] = &[
    ClasseConfig {
        special_category: "production_challenger",
        class_name: "mazda",
        feeder_category: "mazda_amador",
    },
    ClasseConfig {
        special_category: "production_challenger",
        class_name: "toyota",
        feeder_category: "toyota_amador",
    },
    ClasseConfig {
        special_category: "production_challenger",
        class_name: "bmw",
        feeder_category: "bmw_m2",
    },
    ClasseConfig {
        special_category: "endurance",
        class_name: "gt4",
        feeder_category: "gt4",
    },
    ClasseConfig {
        special_category: "endurance",
        class_name: "gt3",
        feeder_category: "gt3",
    },
    ClasseConfig {
        special_category: "endurance",
        class_name: "lmp2",
        feeder_category: "endurance",
    },
];

fn uses_regular_special_event_grid(category: &str) -> bool {
    matches!(category, "production_challenger" | "endurance")
}

pub(super) fn legacy_convocation_classes() -> impl Iterator<Item = &'static ClasseConfig> {
    CLASSES_CONVOCADAS
        .iter()
        .filter(|cfg| !uses_regular_special_event_grid(cfg.special_category))
}
