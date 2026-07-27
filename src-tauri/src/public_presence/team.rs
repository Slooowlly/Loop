// Presença pública de equipe — grandeza própria, derivada da mídia do lineup.
//
// Deliberadamente SEM tier: o valor é contínuo e assim é consumido (multiplicador
// linear de patrocínio em `commands/race/financas.rs`). Não confundir com
// `market::visibility::MarketVisibilityTier`, que classifica o atributo `midia`
// de um piloto individual e cujos limiares (25/85) vêm das tags visuais em
// `models/driver.rs`. As duas grandezas não são comensuráveis: aqui o score é uma
// média ponderada de duas mídias, sem sistema de labels por trás.

/// Deriva presença pública de equipe a partir da mídia dos pilotos do lineup ativo.
///
/// Fórmula: `top_driver_media * 0.7 + second_driver_media * 0.3`
///
/// O piloto mais midiático domina o perfil público da equipe; o segundo contribui
/// de forma subordinada. Sem pilotos → 0.0. Resultado no intervalo 0.0..=100.0.
pub fn derive_team_public_presence(driver_medias: &[f64]) -> f64 {
    let mut sorted: Vec<f64> = driver_medias.iter().map(|m| m.clamp(0.0, 100.0)).collect();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let top = sorted.first().copied().unwrap_or(0.0);
    let second = sorted.get(1).copied().unwrap_or(0.0);
    top * 0.7 + second * 0.3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_lineup_is_zero() {
        assert!(derive_team_public_presence(&[]).abs() < 1e-9);
    }

    #[test]
    fn test_single_driver_uses_top_weight_only() {
        assert!((derive_team_public_presence(&[10.0]) - 7.0).abs() < 1e-9);
    }

    #[test]
    fn test_two_low_drivers() {
        assert!((derive_team_public_presence(&[15.0, 10.0]) - 13.5).abs() < 1e-9);
    }

    #[test]
    fn test_mixed_lineup() {
        assert!((derive_team_public_presence(&[40.0, 20.0]) - 34.0).abs() < 1e-9);
    }

    #[test]
    fn test_high_low_lineup() {
        assert!((derive_team_public_presence(&[80.0, 50.0]) - 71.0).abs() < 1e-9);
    }

    #[test]
    fn test_two_high_lineup() {
        assert!((derive_team_public_presence(&[95.0, 80.0]) - 90.5).abs() < 1e-9);
    }

    #[test]
    fn test_ordem_de_entrada_nao_importa() {
        let a = derive_team_public_presence(&[30.0, 80.0]);
        let b = derive_team_public_presence(&[80.0, 30.0]);
        assert!((a - b).abs() < 1e-9);
    }

    #[test]
    fn test_terceiro_piloto_nao_conta() {
        let dois = derive_team_public_presence(&[80.0, 50.0]);
        let tres = derive_team_public_presence(&[80.0, 50.0, 90.0]);
        // o de 90 vira o top; o de 80 vira o segundo; o de 50 é ignorado
        assert!((dois - 71.0).abs() < 1e-9);
        assert!((tres - 87.0).abs() < 1e-9);
    }

    #[test]
    fn test_clamp_de_entrada_fora_de_faixa() {
        assert!((derive_team_public_presence(&[150.0, -20.0]) - 70.0).abs() < 1e-9);
    }

    #[test]
    fn test_monotonicity() {
        // Aumentar o piloto top deve nunca diminuir o score
        let p1 = derive_team_public_presence(&[50.0, 30.0]);
        let p2 = derive_team_public_presence(&[70.0, 30.0]);
        let p3 = derive_team_public_presence(&[90.0, 30.0]);
        assert!(p1 <= p2);
        assert!(p2 <= p3);
    }
}
