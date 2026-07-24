//! Definicoes estaticas das familias e faixas do Atlas de equipes, e a montagem
//! do payload de familia/faixa (metadados sem dados de temporada).

use super::*;

#[derive(Debug, Clone)]
pub(super) struct TeamHistoryBandDef {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) category: &'static str,
    pub(super) class_name: Option<&'static str>,
    pub(super) is_special: bool,
}

#[derive(Debug, Clone)]
pub(super) struct TeamHistoryFamilyDef {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) bands: &'static [TeamHistoryBandDef],
}

pub(super) const MAZDA_BANDS: [TeamHistoryBandDef; 3] = [
    TeamHistoryBandDef {
        key: "production_mazda",
        label: "Mazda Production",
        category: "production_challenger",
        class_name: Some("mazda"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "mazda_amador",
        label: "Mazda Championship",
        category: "mazda_amador",
        class_name: None,
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "mazda_rookie",
        label: "Mazda Rookie",
        category: "mazda_rookie",
        class_name: None,
        is_special: false,
    },
];

const TOYOTA_BANDS: [TeamHistoryBandDef; 3] = [
    TeamHistoryBandDef {
        key: "production_toyota",
        label: "Toyota Production",
        category: "production_challenger",
        class_name: Some("toyota"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "toyota_amador",
        label: "Toyota Cup",
        category: "toyota_amador",
        class_name: None,
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "toyota_rookie",
        label: "Toyota Rookie",
        category: "toyota_rookie",
        class_name: None,
        is_special: false,
    },
];

const BMW_BANDS: [TeamHistoryBandDef; 2] = [
    TeamHistoryBandDef {
        key: "production_bmw",
        label: "BMW Production",
        category: "production_challenger",
        class_name: Some("bmw"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "bmw_m2",
        label: "BMW M2",
        category: "bmw_m2",
        class_name: None,
        is_special: false,
    },
];

const GT4_BANDS: [TeamHistoryBandDef; 2] = [
    TeamHistoryBandDef {
        key: "endurance_gt4",
        label: "GT4 Endurance",
        category: "endurance",
        class_name: Some("gt4"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "gt4",
        label: "GT4",
        category: "gt4",
        class_name: None,
        is_special: false,
    },
];

const GT3_BANDS: [TeamHistoryBandDef; 2] = [
    TeamHistoryBandDef {
        key: "endurance_gt3",
        label: "GT3 Endurance",
        category: "endurance",
        class_name: Some("gt3"),
        is_special: false,
    },
    TeamHistoryBandDef {
        key: "gt3",
        label: "GT3",
        category: "gt3",
        class_name: None,
        is_special: false,
    },
];

const LMP2_BANDS: [TeamHistoryBandDef; 1] = [TeamHistoryBandDef {
    key: "endurance_lmp2",
    label: "LMP2",
    category: "endurance",
    class_name: Some("lmp2"),
    is_special: false,
}];

pub(super) const FAMILY_DEFS: [TeamHistoryFamilyDef; 6] = [
    TeamHistoryFamilyDef {
        id: "mazda",
        label: "Mazda",
        bands: &MAZDA_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "toyota",
        label: "Toyota",
        bands: &TOYOTA_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "bmw",
        label: "BMW",
        bands: &BMW_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "gt4",
        label: "GT4",
        bands: &GT4_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "gt3",
        label: "GT3",
        bands: &GT3_BANDS,
    },
    TeamHistoryFamilyDef {
        id: "lmp2",
        label: "LMP2",
        bands: &LMP2_BANDS,
    },
];

/// Year the band's (category, class) combination first existed.
///
/// For shared categories like `endurance`, a class can debut after the
/// category itself: the GT4 class only joined endurance in 2002 even though
/// the endurance category started in 2000. The band's start is therefore the
/// later of the two so the "category did not exist" band reflects the class.
pub(super) fn band_start_year(band: &TeamHistoryBandDef) -> i32 {
    let category_start = category_start_year(band.category);
    match band.class_name {
        Some(class_name) => {
            category_start.max(timeline_class_start_year(band.category, Some(class_name)))
        }
        None => category_start,
    }
}

pub(super) fn resolve_family(family: &str) -> &'static TeamHistoryFamilyDef {
    FAMILY_DEFS
        .iter()
        .find(|value| value.id == family)
        .unwrap_or(&FAMILY_DEFS[0])
}

pub(super) fn family_payload(family: &TeamHistoryFamilyDef) -> GlobalTeamHistoryFamily {
    GlobalTeamHistoryFamily {
        id: family.id.to_string(),
        label: family.label.to_string(),
        bands: family.bands.iter().map(family_band_payload).collect(),
    }
}

fn family_band_payload(band: &TeamHistoryBandDef) -> GlobalTeamHistoryFamilyBand {
    GlobalTeamHistoryFamilyBand {
        key: band.key.to_string(),
        label: band.label.to_string(),
        category: band.category.to_string(),
        class_name: band.class_name.map(str::to_string),
        starts_year: band_start_year(band),
        is_special: band.is_special,
    }
}
