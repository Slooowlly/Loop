use crate::constants::categories::get_category_config;

// Fonte ÚNICA das faixas de skill por tier. A segunda fonte que existia em
// driver.rs (`tier_base_range`, com valores próprios, e `difficulty_bias` com outra
// escala de dificuldade) não existe mais: a geração de piloto
// (`models/driver_generation.rs`), o crescimento (`evolution/growth.rs`) e a escada
// do mercado (`market/pipeline/consolidacao.rs`) leem daqui.

pub struct SkillRangeConfig {
    pub tier: u8,
    pub skill_min: u8,
    pub skill_max: u8,
    pub skill_media: u8,
}

// Faixas de skill por tier. Design: rookie e cup SE SOBREPÕEM de propósito — um
// campeão rookie (topo efetivo ~62, prodígio ~74) vale um piloto cup, e pode
// competir de igual lá em cima (o anti-"sempre a mesma equipe" vem do shuffle do
// pareamento em world.rs, não daqui). A partir do t2 os PISOS sobem de forma
// progressiva, com o SALTO grande no t3 (gt4) — subir a partir do cup fica cada vez
// mais difícil. Os tetos sobem pouco (aces do tier de cima ≈ topo do de baixo).
// O rookie (tier 0) fica intacto; seu limite efetivo é tratado à parte em
// `effective_skill_bounds` (25–62, prodígio 63–74).
static SKILL_RANGES: [SkillRangeConfig; 5] = [
    SkillRangeConfig {
        tier: 0,
        skill_min: 25,
        skill_max: 70,
        skill_media: 40,
    },
    SkillRangeConfig {
        tier: 1,
        skill_min: 38,
        skill_max: 72,
        skill_media: 55,
    },
    SkillRangeConfig {
        tier: 2,
        skill_min: 55,
        skill_max: 78,
        skill_media: 66,
    },
    SkillRangeConfig {
        tier: 3,
        skill_min: 64,
        skill_max: 82,
        skill_media: 73,
    },
    SkillRangeConfig {
        tier: 4,
        skill_min: 74,
        skill_max: 88,
        skill_media: 81,
    },
];

pub fn get_skill_range_by_tier(tier: u8) -> Option<&'static SkillRangeConfig> {
    SKILL_RANGES.iter().find(|range| range.tier == tier)
}

/// Faixa pelo id da categoria. Hoje só os testes chamam — o código de produção
/// resolve o tier antes e usa `get_skill_range_by_tier`. O allow é PONTUAL de
/// propósito: no arquivo inteiro ele esconderia a próxima função a ficar órfã.
#[allow(dead_code)]
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
        assert_eq!(range.skill_min, 74);
        assert_eq!(range.skill_max, 88);
    }

    #[test]
    fn test_tier_floors_escalate_monotonically() {
        // Cada tier começa acima do PISO do anterior — subir fica progressivamente
        // mais difícil (o campo de cima é mais forte). NÃO implica ausência de
        // sobreposição de faixas: rookie↔cup se sobrepõem de propósito.
        for tier in 1..=4u8 {
            let lower = get_skill_range_by_tier(tier - 1).expect("lower tier");
            let upper = get_skill_range_by_tier(tier).expect("upper tier");
            assert!(
                upper.skill_min > lower.skill_min,
                "piso do tier {tier} ({}) deve superar o do tier {} ({})",
                upper.skill_min,
                tier - 1,
                lower.skill_min
            );
        }
    }

    #[test]
    fn test_rookie_champion_overlaps_cup_field() {
        // Um campeão rookie (topo efetivo ~62, prodígio ~74) deve valer um piloto cup:
        // a faixa do cup precisa englobar esse nível (topo do cup >= ~topo do rookie).
        let cup = get_skill_range_by_tier(1).expect("cup tier");
        assert!(
            cup.skill_min < 62 && cup.skill_max >= 72,
            "cup ({}-{}) deve sobrepor o topo do rookie (~62-74)",
            cup.skill_min,
            cup.skill_max
        );
    }
}
