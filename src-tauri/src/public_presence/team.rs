// Sem dependência de market::visibility — thresholds próprios, paralelos por semântica.

/// Tier de presença pública de equipe — espelha MarketVisibilityTier para consistência semântica.
#[derive(Debug, Clone, PartialEq)]
pub enum TeamPublicPresenceTier {
    Baixa,
    Relevante,
    Alta,
    Elite,
}

#[derive(Debug, Clone)]
pub struct TeamPublicPresence {
    pub raw_score: f64,
    pub tier: TeamPublicPresenceTier,
}

/// Deriva presença pública de equipe a partir da mídia dos pilotos do lineup ativo.
///
/// Fórmula: top_driver_media * 0.7 + second_driver_media * 0.3
///
/// O piloto mais midiático domina o perfil público da equipe; o segundo contribui
/// de forma subordinada. Sem pilotos → score 0.0 (Baixa).
/// Thresholds espelham MarketVisibilityTier: Baixa ≤25, Relevante 26–59, Alta 60–84, Elite ≥85.
pub fn derive_team_public_presence(driver_medias: &[f64]) -> TeamPublicPresence {
    let mut sorted: Vec<f64> = driver_medias.iter().map(|m| m.clamp(0.0, 100.0)).collect();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let top = sorted.first().copied().unwrap_or(0.0);
    let second = sorted.get(1).copied().unwrap_or(0.0);
    let raw_score = top * 0.7 + second * 0.3;
    let tier = if raw_score >= 85.0 {
        TeamPublicPresenceTier::Elite
    } else if raw_score >= 60.0 {
        TeamPublicPresenceTier::Alta
    } else if raw_score >= 26.0 {
        TeamPublicPresenceTier::Relevante
    } else {
        TeamPublicPresenceTier::Baixa
    };
    TeamPublicPresence { raw_score, tier }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_lineup_is_baixa() {
        let p = derive_team_public_presence(&[]);
        assert!((p.raw_score - 0.0).abs() < 1e-9);
        assert_eq!(p.tier, TeamPublicPresenceTier::Baixa);
    }

    #[test]
    fn test_single_driver_baixa() {
        let p = derive_team_public_presence(&[10.0]);
        assert!((p.raw_score - 7.0).abs() < 1e-9);
        assert_eq!(p.tier, TeamPublicPresenceTier::Baixa);
    }

    #[test]
    fn test_two_low_drivers_baixa() {
        let p = derive_team_public_presence(&[15.0, 10.0]);
        assert!((p.raw_score - 13.5).abs() < 1e-9);
        assert_eq!(p.tier, TeamPublicPresenceTier::Baixa);
    }

    #[test]
    fn test_mixed_lineup_relevante() {
        let p = derive_team_public_presence(&[40.0, 20.0]);
        assert!((p.raw_score - 34.0).abs() < 1e-9);
        assert_eq!(p.tier, TeamPublicPresenceTier::Relevante);
    }

    #[test]
    fn test_high_low_lineup_alta() {
        let p = derive_team_public_presence(&[80.0, 50.0]);
        assert!((p.raw_score - 71.0).abs() < 1e-9);
        assert_eq!(p.tier, TeamPublicPresenceTier::Alta);
    }

    #[test]
    fn test_two_high_lineup_elite() {
        let p = derive_team_public_presence(&[95.0, 80.0]);
        assert!((p.raw_score - 90.5).abs() < 1e-9);
        assert_eq!(p.tier, TeamPublicPresenceTier::Elite);
    }

    #[test]
    fn test_monotonicity() {
        // Aumentar o piloto top deve nunca diminuir o raw_score
        let p1 = derive_team_public_presence(&[50.0, 30.0]);
        let p2 = derive_team_public_presence(&[70.0, 30.0]);
        let p3 = derive_team_public_presence(&[90.0, 30.0]);
        assert!(p1.raw_score <= p2.raw_score);
        assert!(p2.raw_score <= p3.raw_score);
    }
}
