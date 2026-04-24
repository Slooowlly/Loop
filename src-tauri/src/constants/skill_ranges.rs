#![allow(dead_code)]

use crate::constants::categories::get_category_config;

// NOTA: driver.rs usa tier_base_range() com valores diferentes:
//   tier 0: 20-48 (spec: 25-70)
//   tier 1: 28-56 (spec: 35-70)
//   tier 2: 38-65 (spec: 45-75)
//   tier 3: 48-75 (spec: 55-80)
//   tier 4: 58-85 (spec: 65-85)
// driver.rs também usa Facil/Normal/Dificil/Elite em difficulty_bias()
// Essas inconsistências serão resolvidas quando driver.rs for atualizado
// para consumir as constantes deste módulo.

pub struct SkillRangeConfig {
    pub tier: u8,
    pub skill_min: u8,
    pub skill_max: u8,
    pub skill_media: u8,
}

static SKILL_RANGES: [SkillRangeConfig; 5] = [
    SkillRangeConfig {
        tier: 0,
        skill_min: 25,
        skill_max: 70,
        skill_media: 40,
    },
    SkillRangeConfig {
        tier: 1,
        skill_min: 35,
        skill_max: 70,
        skill_media: 50,
    },
    SkillRangeConfig {
        tier: 2,
        skill_min: 45,
        skill_max: 75,
        skill_media: 60,
    },
    SkillRangeConfig {
        tier: 3,
        skill_min: 55,
        skill_max: 80,
        skill_media: 68,
    },
    SkillRangeConfig {
        tier: 4,
        skill_min: 65,
        skill_max: 85,
        skill_media: 78,
    },
];

pub fn get_skill_range_by_tier(tier: u8) -> Option<&'static SkillRangeConfig> {
    SKILL_RANGES.iter().find(|range| range.tier == tier)
}

pub fn get_skill_range(category_id: &str) -> Option<&'static SkillRangeConfig> {
    let tier = get_category_config(category_id)?.tier.min(4);
    get_skill_range_by_tier(tier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_range_rookie() {
        let range = get_skill_range_by_tier(0).expect("rookie tier should exist");
        assert_eq!(range.skill_min, 25);
        assert_eq!(range.skill_max, 70);
    }

    #[test]
    fn test_skill_range_gt3() {
        let range = get_skill_range("gt3").expect("gt3 should map to a skill range");
        assert_eq!(range.skill_min, 65);
        assert_eq!(range.skill_max, 85);
    }
}
