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
///
/// Este é o trim de **CORRIDA** — o desempenho sustentado, de tanque cheio, ao longo de um
/// stint. É o número que `race/**` lê e ele NÃO muda: o trim de quali abaixo é aditivo por
/// cima do mesmo carro, não uma redefinição deste.
pub fn effective_car_performance_from_shape(
    base_car_performance: f64,
    car_shape: CarAttributeWeights,
    track_weights: CarAttributeWeights,
) -> f64 {
    base_car_performance + track_delta_from_shape(car_shape, track_weights)
}

// ─────────────────────── Trim de classificação (volta única) ───────────────────────
//
// O mesmo carro tem duas caras. Um número só servindo para a volta de tanque vazio E para
// o stint de tanque cheio é o que faz a ordem de sábado ser a de domingo por construção.
//
// A derivação NÃO é sorteada: sai do `car_shape` que já existe. Duas coisas mudam da
// corrida para a volta única, e as duas são leitura do mesmo vetor `(acc, power, handling)`:
//
// 1. **Viés de pico.** Muito apoio (handling) crava a volta e castiga o pneu; eficiente e
//    de ponta (power) larga atrás e sobe no stint. É o eixo que de fato DESCORRELACIONA os
//    dois trims, porque é ortogonal à magnitude do carro.
// 2. **Casamento mais puro com a pista.** Na volta única não há tráfego, gestão de pneu nem
//    estratégia diluindo o quanto o carro serve àquele traçado — o mesmo delta de shape
//    aparece amplificado.
//
// As duas constantes são PROVISÓRIAS: quem fecha os valores é o harness estatístico.

/// Ganho máximo (unidades de `car_performance`) do viés de pico na volta única — o quanto
/// um carro de apoio puro ganha, ou um de ponta pura perde, contra o balanceado. PROVISÓRIO.
pub const GANHO_TRIM_QUALI: f64 = 3.5;

/// Quanto o casamento shape↔pista pesa A MAIS na volta única que na corrida. PROVISÓRIO.
pub const AMPLIFICACAO_SHAPE_QUALI: f64 = 1.35;

/// Viés de PICO do shape ∈ [-1, 1]: positivo = carro de apoio (handling), que crava a volta;
/// negativo = carro de ponta/eficiência (power), que rende no stint. Balanceado → ≈ 0.
pub fn vies_de_pico(car_shape: CarAttributeWeights) -> f64 {
    let (acc, power, handling) = car_shape;
    let total = acc + power + handling;
    if total <= 0.0 {
        return 0.0;
    }
    ((handling - power) / total).clamp(-1.0, 1.0)
}

/// `car_performance` do trim de **CLASSIFICAÇÃO**: mesma magnitude do carro de corrida, com
/// o casamento de shape amplificado e o viés de pico somado. Consumido só por
/// `simulation::qualifying`.
pub fn quali_car_performance_from_shape(
    base_car_performance: f64,
    car_shape: CarAttributeWeights,
    track_weights: CarAttributeWeights,
) -> f64 {
    base_car_performance
        + track_delta_from_shape(car_shape, track_weights) * AMPLIFICACAO_SHAPE_QUALI
        + GANHO_TRIM_QUALI * vies_de_pico(car_shape)
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
    fn vies_de_pico_separa_apoio_de_ponta() {
        assert!(vies_de_pico(HANDLING_SHAPE) > 0.3, "apoio crava a volta");
        assert!(vies_de_pico(POWER_SHAPE) < -0.3, "ponta rende no stint");
        assert!(vies_de_pico(BALANCED_SHAPE).abs() < 0.02);
    }

    #[test]
    fn trim_de_quali_favorece_o_carro_de_apoio_e_pune_o_de_ponta() {
        // Mesma magnitude, mesma pista: o carro de apoio fica melhor na volta única do que
        // é na corrida; o de ponta, pior. É o eixo que descorrelaciona os dois trims.
        let apoio_corrida =
            effective_car_performance_from_shape(10.0, HANDLING_SHAPE, BALANCED_TRACK);
        let apoio_quali = quali_car_performance_from_shape(10.0, HANDLING_SHAPE, BALANCED_TRACK);
        assert!(
            apoio_quali > apoio_corrida,
            "{apoio_quali} vs {apoio_corrida}"
        );

        let ponta_corrida = effective_car_performance_from_shape(10.0, POWER_SHAPE, BALANCED_TRACK);
        let ponta_quali = quali_car_performance_from_shape(10.0, POWER_SHAPE, BALANCED_TRACK);
        assert!(
            ponta_quali < ponta_corrida,
            "{ponta_quali} vs {ponta_corrida}"
        );
    }

    #[test]
    fn trim_de_quali_e_neutro_para_carro_balanceado() {
        // Sem viés e sem shape, os dois trims são o mesmo carro — nada de bônus de graça.
        let corrida = effective_car_performance_from_shape(10.0, BALANCED_SHAPE, BALANCED_TRACK);
        let quali = quali_car_performance_from_shape(10.0, BALANCED_SHAPE, BALANCED_TRACK);
        assert!((quali - corrida).abs() < 0.1, "{quali} vs {corrida}");
    }

    #[test]
    fn trim_de_quali_preserva_a_ordem_de_magnitude() {
        // O trim reembaralha, mas não inverte um carro muito melhor: nível ainda manda.
        let bom = quali_car_performance_from_shape(14.0, POWER_SHAPE, BALANCED_TRACK);
        let ruim = quali_car_performance_from_shape(4.0, HANDLING_SHAPE, BALANCED_TRACK);
        assert!(bom > ruim, "bom={bom} ruim={ruim}");
    }

    #[test]
    fn nivel_domina_em_pista_equilibrada() {
        // Nível alto com shape ERRADO vs nível baixo com shape CERTO.
        let high = effective_car_performance_from_shape(12.0, HANDLING_SHAPE, BALANCED_TRACK);
        let low = effective_car_performance_from_shape(6.0, POWER_SHAPE, BALANCED_TRACK);
        assert!(
            high > low,
            "pista equilibrada: o nível manda (high={high}, low={low})"
        );
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
