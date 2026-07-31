//! Ponte pura entre o modelo do carro (11 peças) e a simulação.
//!
//! Traduz o `Car` em dois valores que o sim consome: a **magnitude** vira o
//! `car_performance` (escalar) e o **shape** vira um vetor de pesos (acc, power,
//! handling) para o casamento com a pista. Mapeamento PROVISÓRIO (preserva ~a faixa
//! histórica de `car_performance`); a calibração fina é o chunk 8. Ver design §8 em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.
#![allow(dead_code)] // Chunk 5: consumido pela integração no sim (context.rs) em seguida.

use crate::car::Car;
use crate::simulation::car_build::{effective_car_performance_from_shape, CarAttributeWeights};
use crate::simulation::track_profile::BALANCED_CAR_WEIGHTS;

/// Topo da faixa de `car_performance` que um carro nível 10 alcança (domínio do
/// `normalize_car_performance`: −5..16). Calibrável no chunk 8.
pub const CAR_PERF_MAX: f64 = 16.0;

/// Magnitude → `car_performance` do sim. Linear entre um carro uniforme nível 1 (→ 0) e
/// nível 10 (→ `CAR_PERF_MAX`). Rookie spec (tudo nível 1) → 0 para todos (idênticos).
pub fn car_performance_from(car: &Car) -> f64 {
    let min = Car::uniform(1).magnitude();
    let max = Car::uniform(10).magnitude();
    if max <= min {
        return 0.0;
    }
    (((car.magnitude() - min) / (max - min)) * CAR_PERF_MAX).max(0.0)
}

/// Shape do carro como pesos `(acceleration, power, handling)` somando 100 — a mesma
/// ordem/escala que os pesos de pista e o `dot_match_score`. Carro sem PHA → balanceado.
pub fn car_shape_weights(car: &Car) -> CarAttributeWeights {
    let (p, h, a) = car.pha();
    let total = p + h + a;
    if total <= 0.0 {
        return BALANCED_CAR_WEIGHTS;
    }
    (a / total * 100.0, p / total * 100.0, h / total * 100.0)
}

/// **Vantagem de carro nesta pista** (unidades de `car_performance`): funde a MAGNITUDE
/// (nível) com o CASAMENTO do shape (foco) com a pista, exatamente como o sim faria pra te
/// deixar mais rápido. Carro spec de rookie (tudo nível 1, sem shape) → 0.
///
/// É o escalar que o EXPORT inverte em skill de IA: no iRacing o carro é spec (não dá pra
/// te acelerar), então um carro melhor só pode ENFRAQUECER a IA. `track_weights` na ordem
/// `(acceleration, power, handling)` — a mesma de [`get_track_simulation_data`].
///
/// [`get_track_simulation_data`]: crate::simulation::track_profile::get_track_simulation_data
pub fn car_advantage(car: &Car, track_weights: CarAttributeWeights) -> f64 {
    effective_car_performance_from_shape(
        car_performance_from(car),
        car_shape_weights(car),
        track_weights,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::car::PartType;

    #[test]
    fn nivel_1_mapeia_para_zero_e_nivel_10_para_o_topo() {
        assert!(car_performance_from(&Car::uniform(1)).abs() < 1e-9);
        assert!((car_performance_from(&Car::uniform(10)) - CAR_PERF_MAX).abs() < 1e-9);
    }

    #[test]
    fn car_performance_cresce_com_o_nivel() {
        assert!(car_performance_from(&Car::uniform(7)) > car_performance_from(&Car::uniform(3)));
    }

    #[test]
    fn shape_soma_cem_e_reflete_o_vies() {
        let (acc, power, handling) = car_shape_weights(&Car::uniform(5));
        assert!((acc + power + handling - 100.0).abs() < 1e-6);

        // Carro focado em potência (motor/câmbio/cooling/eletrônica no talo).
        let mut power_car = Car::uniform(1);
        for part in [
            PartType::Engine,
            PartType::Gearbox,
            PartType::Cooling,
            PartType::Electronics,
        ] {
            power_car.set_level(part, 10);
        }
        let (_a, p, h) = car_shape_weights(&power_car);
        assert!(
            p > h,
            "carro de potência deveria pesar mais em power: P={p} H={h}"
        );
    }

    #[test]
    fn shape_de_carro_neutro_e_balanceado() {
        // Carro uniforme é ~balanceado → pesos próximos entre si.
        let (acc, power, handling) = car_shape_weights(&Car::uniform(6));
        let max = acc.max(power).max(handling);
        let min = acc.min(power).min(handling);
        assert!(
            max - min < 2.0,
            "uniforme deveria ser ~balanceado: {acc},{power},{handling}"
        );
    }
}
