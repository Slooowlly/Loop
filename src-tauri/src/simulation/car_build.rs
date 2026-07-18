//! Casamento carro↔pista (shape) na simulação.
//!
//! O `CarBuildProfile` discreto foi APOSENTADO pelo Sistema de Nível do Carro: o shape do
//! carro agora é um vetor CONTÍNUO de pesos `(acc, power, handling)` derivado das 11 peças
//! (ver `crate::car::sim_bridge`). O bônus de shape tem teto que escala com a peakiness da
//! pista — a regra de dominância (o nível manda; o shape só vira o jogo em pista de ponto
//! único). Ver design §8–§9 em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.

use crate::simulation::track_profile::BALANCED_CAR_WEIGHTS;

pub type CarAttributeWeights = (f64, f64, f64);

/// Teto máximo do bônus de shape, atingido só numa pista de ponto único absoluto.
/// Calibrável (chunk 8). Ver design §9 (regra de dominância).
pub const DOMINANCE_MAX_DELTA: f64 = 8.0;

/// Produto escalar (normalizado) entre os pesos do carro e os da pista.
pub fn dot_match_score(weights: CarAttributeWeights, track_weights: CarAttributeWeights) -> f64 {
    let (team_acc, team_power, team_handling) = weights;
    let (track_acc, track_power, track_handling) = track_weights;
    (team_acc * track_acc + team_power * track_power + team_handling * track_handling) / 100.0
}

/// Casamento de um carro perfeitamente balanceado com a pista — a linha de base neutra.
pub fn balanced_match_score(track_weights: CarAttributeWeights) -> f64 {
    dot_match_score(BALANCED_CAR_WEIGHTS, track_weights)
}

/// Peakiness da pista ∈ [0,1]: 0 = perfeitamente equilibrada (≈33/33/33), perto de 1 =
/// exige um único atributo. É o que gradua o quanto o shape pode influenciar: numa pista
/// equilibrada o shape é irrelevante (só o nível decide); numa pista de ponto único ele
/// pode virar o jogo.
pub fn track_peakiness(track_weights: CarAttributeWeights) -> f64 {
    let (a, p, h) = track_weights;
    let total = a + p + h;
    if total <= 0.0 {
        return 0.0;
    }
    let (a, p, h) = (a / total, p / total, h / total);
    let max = a.max(p).max(h);
    let min = a.min(p).min(h);
    max - min
}

/// Bônus de shape a partir do vetor contínuo de pesos do carro `(acc, power, handling)`,
/// com teto que ESCALA pela peakiness (regra de dominância): ~0 em pista equilibrada (o
/// nível/magnitude domina), largo só em pista de ponto único (onde um shape certo pode
/// furar a fila de um nível maior).
pub fn track_delta_from_shape(
    car_shape: CarAttributeWeights,
    track_weights: CarAttributeWeights,
) -> f64 {
    let raw =
        (dot_match_score(car_shape, track_weights) - balanced_match_score(track_weights)) / 2.5;
    let clamp = DOMINANCE_MAX_DELTA * track_peakiness(track_weights);
    raw.clamp(-clamp, clamp)
}

/// `car_performance` efetivo = magnitude (base) + o casamento do shape contínuo com a pista.
pub fn effective_car_performance_from_shape(
    base_car_performance: f64,
    car_shape: CarAttributeWeights,
    track_weights: CarAttributeWeights,
) -> f64 {
    base_car_performance + track_delta_from_shape(car_shape, track_weights)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shapes de exemplo em (acc, power, handling).
    const POWER_SHAPE: CarAttributeWeights = (20.0, 60.0, 20.0);
    const HANDLING_SHAPE: CarAttributeWeights = (20.0, 20.0, 60.0);
    const BALANCED_SHAPE: CarAttributeWeights = (34.0, 33.0, 33.0);
    // Pistas.
    const BALANCED_TRACK: CarAttributeWeights = (34.0, 33.0, 33.0);
    const POWER_TRACK: CarAttributeWeights = (10.0, 70.0, 20.0); // Monza-like

    #[test]
    fn peakiness_zero_no_equilibrio_e_alta_no_ponto_unico() {
        assert!(track_peakiness((34.0, 33.0, 33.0)) < 0.02);
        assert!(track_peakiness((10.0, 70.0, 20.0)) > 0.5);
        assert!(track_peakiness((0.0, 100.0, 0.0)) > 0.99);
    }

    #[test]
    fn shape_balanceado_e_neutro_em_qualquer_pista() {
        assert!(track_delta_from_shape(BALANCED_SHAPE, POWER_TRACK).abs() < 0.01);
    }

    #[test]
    fn shape_certo_positivo_e_errado_negativo_na_pista_de_power() {
        assert!(track_delta_from_shape(POWER_SHAPE, POWER_TRACK) > 3.0);
        assert!(track_delta_from_shape(HANDLING_SHAPE, POWER_TRACK) < 0.0);
    }

    #[test]
    fn shape_e_irrelevante_em_pista_equilibrada() {
        assert!(track_delta_from_shape(POWER_SHAPE, BALANCED_TRACK).abs() < 1.0);
        assert!(track_delta_from_shape(HANDLING_SHAPE, BALANCED_TRACK).abs() < 1.0);
    }

    #[test]
    fn delta_cap_abre_ate_o_maximo_no_ponto_unico_absoluto() {
        // Shape de power puro numa pista de power puro (peakiness 1) → teto.
        let delta = track_delta_from_shape((0.0, 100.0, 0.0), (0.0, 1000.0, 0.0));
        assert_eq!(delta, DOMINANCE_MAX_DELTA);
    }

    #[test]
    fn delta_cap_negativo_no_ponto_unico_absoluto() {
        // Shape de handling puro numa pista de power puro → teto negativo.
        let delta = track_delta_from_shape((0.0, 0.0, 100.0), (0.0, 1000.0, 0.0));
        assert_eq!(delta, -DOMINANCE_MAX_DELTA);
    }

    #[test]
    fn nivel_domina_em_pista_equilibrada() {
        // Nível alto com shape ERRADO vs nível baixo com shape CERTO.
        let high = effective_car_performance_from_shape(12.0, HANDLING_SHAPE, BALANCED_TRACK);
        let low = effective_car_performance_from_shape(6.0, POWER_SHAPE, BALANCED_TRACK);
        assert!(high > low, "pista equilibrada: o nível manda (high={high}, low={low})");
    }

    #[test]
    fn pista_de_ponto_unico_deixa_o_shape_virar_o_jogo() {
        // Nível levemente menor + shape CERTO bate nível maior + shape ERRADO.
        let matched_low = effective_car_performance_from_shape(9.0, POWER_SHAPE, POWER_TRACK);
        let mismatched_high =
            effective_car_performance_from_shape(10.0, HANDLING_SHAPE, POWER_TRACK);
        assert!(
            matched_low > mismatched_high,
            "pista de power: o shape certo vira (low={matched_low}, high={mismatched_high})"
        );
    }
}
