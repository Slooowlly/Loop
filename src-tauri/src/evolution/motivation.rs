use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::evolution::growth::SeasonStats;
use crate::models::driver::Driver;
use crate::models::enums::PrimaryPersonality;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotivationReport {
    pub driver_id: String,
    pub old_motivation: u8,
    pub new_motivation: u8,
    pub delta: i8,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct MotivationContext {
    pub was_champion: bool,
    pub was_promoted: bool,
    pub was_relegated: bool,
    pub contract_renewed: bool,
    pub lost_seat: bool,
    pub seasons_in_category: i32,
}

pub fn adjust_end_of_season_motivation(
    driver: &mut Driver,
    stats: &SeasonStats,
    ctx: &MotivationContext,
    _rng: &mut impl Rng,
) -> MotivationReport {
    let old_motivation = driver.motivacao.round().clamp(0.0, 100.0) as u8;
    let mut delta = 0_i32;
    let mut reasons = Vec::new();

    if ctx.was_champion {
        delta += 20;
        reasons.push("Campeao da temporada! (+20)".to_string());
    }
    if ctx.was_promoted {
        delta += 15;
        reasons.push("Promovido de categoria (+15)".to_string());
    }
    if stats.posicao_campeonato <= 3 && !ctx.was_champion {
        delta += 8;
        reasons.push("Top 3 no campeonato (+8)".to_string());
    }

    let top_half = stats.posicao_campeonato <= (stats.total_pilotos / 2).max(1);
    if top_half && stats.posicao_campeonato > 3 {
        delta += 3;
        reasons.push("Resultado solido (+3)".to_string());
    }
    if ctx.contract_renewed {
        delta += 5;
        reasons.push("Contrato renovado (+5)".to_string());
    }
    if ctx.lost_seat {
        delta -= 10;
        reasons.push("Perdeu a vaga na equipe (-10)".to_string());
    }
    if ctx.was_relegated {
        delta -= 8;
        reasons.push("Rebaixado de categoria (-8)".to_string());
    }
    if ctx.seasons_in_category >= 3 {
        delta -= 2;
        reasons.push("Estagnacao na mesma categoria (-2)".to_string());
    }
    if stats.dnfs >= 3 {
        delta -= 3;
        reasons.push("Muitos abandonos na temporada (-3)".to_string());
    }

    match driver.personalidade_primaria {
        Some(PrimaryPersonality::Ambicioso)
            if !ctx.was_promoted && ctx.seasons_in_category >= 2 =>
        {
            delta -= 5;
            reasons.push("Ambicioso frustrado por nao subir (-5)".to_string());
        }
        Some(PrimaryPersonality::Consolidador) if top_half => {
            delta += 3;
            reasons.push("Consolidador satisfeito com resultado (+3)".to_string());
        }
        _ => {}
    }

    let new_motivation_value = (driver.motivacao + delta as f64).clamp(0.0, 100.0);
    driver.motivacao = new_motivation_value;

    let new_motivation = new_motivation_value.round().clamp(0.0, 100.0) as u8;
    MotivationReport {
        driver_id: driver.id.clone(),
        old_motivation,
        new_motivation,
        delta: new_motivation as i8 - old_motivation as i8,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_champion_motivation_boost() {
        let mut driver = sample_driver(50.0, None);
        let stats = SeasonStats {
            posicao_campeonato: 1,
            total_pilotos: 20,
            pontos: 120,
            vitorias: 4,
            podios: 6,
            corridas: 8,
            dnfs: 0,
        };
        let mut rng = StdRng::seed_from_u64(1);

        let ctx = MotivationContext {
            was_champion: true,
            was_promoted: false,
            was_relegated: false,
            contract_renewed: false,
            lost_seat: false,
            seasons_in_category: 1,
        };
        let report = adjust_end_of_season_motivation(&mut driver, &stats, &ctx, &mut rng);

        assert!(report.delta >= 20);
        assert!(driver.motivacao > 50.0);
    }

    #[test]
    fn test_stagnation_penalty() {
        let mut driver = sample_driver(60.0, Some(PrimaryPersonality::Ambicioso));
        let stats = SeasonStats {
            posicao_campeonato: 8,
            total_pilotos: 20,
            pontos: 40,
            vitorias: 0,
            podios: 1,
            corridas: 8,
            dnfs: 1,
        };
        let mut rng = StdRng::seed_from_u64(2);

        let ctx = MotivationContext {
            was_champion: false,
            was_promoted: false,
            was_relegated: false,
            contract_renewed: false,
            lost_seat: false,
            seasons_in_category: 3,
        };
        let report = adjust_end_of_season_motivation(&mut driver, &stats, &ctx, &mut rng);

        assert!(report.delta < 0);
        assert!(report
            .reasons
            .iter()
            .any(|reason| reason.contains("Estagnacao")));
    }

    #[test]
    fn test_motivation_clamped_0_100() {
        let stats = SeasonStats {
            posicao_campeonato: 20,
            total_pilotos: 20,
            pontos: 0,
            vitorias: 0,
            podios: 0,
            corridas: 8,
            dnfs: 4,
        };
        let mut low_driver = sample_driver(2.0, Some(PrimaryPersonality::Ambicioso));
        let mut rng_low = StdRng::seed_from_u64(3);
        let low_ctx = MotivationContext {
            was_champion: false,
            was_promoted: false,
            was_relegated: true,
            contract_renewed: false,
            lost_seat: true,
            seasons_in_category: 4,
        };
        let low_report =
            adjust_end_of_season_motivation(&mut low_driver, &stats, &low_ctx, &mut rng_low);
        assert_eq!(low_report.new_motivation, 0);

        let mut high_driver = sample_driver(95.0, Some(PrimaryPersonality::Consolidador));
        let mut rng_high = StdRng::seed_from_u64(4);
        let high_ctx = MotivationContext {
            was_champion: true,
            was_promoted: true,
            was_relegated: false,
            contract_renewed: true,
            lost_seat: false,
            seasons_in_category: 1,
        };
        let high_report = adjust_end_of_season_motivation(
            &mut high_driver,
            &SeasonStats {
                posicao_campeonato: 1,
                total_pilotos: 20,
                pontos: 120,
                vitorias: 4,
                podios: 7,
                corridas: 8,
                dnfs: 0,
            },
            &high_ctx,
            &mut rng_high,
        );
        assert_eq!(high_report.new_motivation, 100);
    }

    fn sample_driver(motivation: f64, personality: Option<PrimaryPersonality>) -> Driver {
        let mut driver = Driver::new(
            "P003".to_string(),
            "Piloto Motivado".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.motivacao = motivation;
        driver.personalidade_primaria = personality;
        driver
    }
}
