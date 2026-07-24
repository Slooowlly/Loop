//! Contexto da temporada: paridade de material e a tese dominante da matéria.

// ── Contexto da temporada (material, título, tese) ───────────────────────────────

/// Paridade de material entre as equipes da categoria — SEM número.
pub(super) enum Material {
    Uniform,
    Unequal { best: String, worst: String },
    Unknown,
}

/// A tese dominante da temporada (§4 bloco 1).
pub(super) enum Thesis {
    RookieSeason,
    VacantThrone,
    UnequalMachinery,
    OpenOnTalent,
}

impl Thesis {
    pub(super) fn id(&self) -> &'static str {
        match self {
            Thesis::RookieSeason => "rookies",
            Thesis::VacantThrone => "vacant_throne",
            Thesis::UnequalMachinery => "unequal",
            Thesis::OpenOnTalent => "open_talent",
        }
    }
}

/// Elege a tese: grid majoritariamente estreante > material desigual > trono vago > aberta.
pub(super) fn select_thesis(rookie_share: f64, material: &Material, throne_vacant: bool) -> Thesis {
    if rookie_share >= 0.6 {
        Thesis::RookieSeason
    } else if matches!(material, Material::Unequal { .. }) {
        Thesis::UnequalMachinery
    } else if throne_vacant {
        Thesis::VacantThrone
    } else {
        Thesis::OpenOnTalent
    }
}
