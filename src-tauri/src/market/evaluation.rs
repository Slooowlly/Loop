pub fn evaluate_driver_performance(
    posicao_campeonato: i32,
    total_pilotos: i32,
    vitorias: i32,
    consistencia: f64,
    expectativa_posicao: i32,
) -> f64 {
    if total_pilotos <= 0 {
        return 50.0;
    }

    let mut score = 50.0;
    let diferenca = expectativa_posicao - posicao_campeonato;

    if diferenca >= 5 {
        score += 30.0;
    } else if diferenca >= 2 {
        score += 15.0;
    } else if diferenca >= -2 {
        score += 0.0;
    } else if diferenca >= -5 {
        score -= 15.0;
    } else {
        score -= 30.0;
    }

    score += (vitorias.max(0) as f64 * 5.0).min(15.0);
    score += (consistencia - 50.0) / 5.0;

    score.clamp(0.0, 100.0)
}

pub fn estimate_expected_position(car_performance: f64, total_positions: i32) -> i32 {
    if total_positions <= 1 {
        return 1;
    }

    let normalized = ((car_performance + 5.0) / 21.0).clamp(0.0, 1.0);
    let expected = ((1.0 - normalized) * (total_positions as f64 - 1.0)) as i32 + 1;
    expected.clamp(1, total_positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_champion_high_score() {
        let score = evaluate_driver_performance(1, 20, 5, 72.0, 6);
        assert!(score >= 85.0);
    }

    #[test]
    fn test_evaluate_last_place_low_score() {
        let score = evaluate_driver_performance(20, 20, 0, 40.0, 8);
        assert!(score <= 25.0);
    }

    #[test]
    fn test_expected_position_high_car_perf() {
        let strong = estimate_expected_position(15.0, 10);
        let weak = estimate_expected_position(-4.0, 10);

        assert!(strong < weak);
        assert_eq!(strong, 1);
    }

    #[test]
    fn test_expected_position_respects_full_driver_grid_size() {
        let midfield_car = 5.0;
        let expected_two_cars = estimate_expected_position(midfield_car, 10);
        let expected_two_drivers = estimate_expected_position(midfield_car, 20);

        assert!(
            expected_two_drivers > expected_two_cars,
            "grid de pilotos maior deve ampliar a expectativa nominal"
        );
    }
}
