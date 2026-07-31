//! Seed inicial do carro de um time, correlacionado com sua qualidade na categoria.
//!
//! Puro: dado o `category_id` e a qualidade relativa do time (`quality` ∈ [0,1]), devolve
//! um carro inicial cujo Nível respeita o teto suave da categoria. Rookie é spec puro. A
//! orquestração que calcula a qualidade e persiste vive em `market::car_maintenance`. Ver
//! design em `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.
#![allow(dead_code)] // Chunk 4: consumido pela orquestração de seed no career.

use crate::car::cost::category_ceiling;
use crate::car::Car;

/// Fração do teto usada como piso do seed: mesmo o time mais fraco não começa no nível 1
/// nas categorias altas (um GT3 fraco ainda tem um carro de GT3).
const SEED_FLOOR_FRACTION: f64 = 0.4;

/// Carro inicial de um time. `quality` ∈ [0,1] (0 = pior, 1 = melhor da categoria).
/// Interpola entre o piso (fração do teto) e o teto suave da categoria. Rookie = spec.
pub fn seed_car(category_id: &str, quality: f64) -> Car {
    let ceiling = category_ceiling(category_id);
    if ceiling <= 1 {
        return Car::spec_rookie();
    }
    let q = quality.clamp(0.0, 1.0);
    let ceiling_f = ceiling as f64;
    let floor = (ceiling_f * SEED_FLOOR_FRACTION).round().max(1.0);
    let level = (floor + q * (ceiling_f - floor))
        .round()
        .clamp(1.0, ceiling_f) as u8;
    Car::uniform(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rookie_e_sempre_spec_nivel_um() {
        assert_eq!(seed_car("mazda_rookie", 0.0).display_level(), 1);
        assert_eq!(seed_car("mazda_rookie", 1.0).display_level(), 1);
    }

    #[test]
    fn melhor_time_da_categoria_chega_ao_teto() {
        assert_eq!(seed_car("gt3", 1.0).display_level(), 7);
        assert_eq!(seed_car("lmp2", 1.0).display_level(), 8);
    }

    #[test]
    fn pior_time_nao_comeca_no_um_nas_categorias_altas() {
        // gt3 teto 7 → piso = round(2.8) = 3.
        assert_eq!(seed_car("gt3", 0.0).display_level(), 3);
    }

    #[test]
    fn qualidade_maior_da_carro_maior() {
        assert!(seed_car("gt3", 0.9).display_level() >= seed_car("gt3", 0.1).display_level());
    }

    #[test]
    fn nunca_ultrapassa_o_teto() {
        for q in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert!(seed_car("production_challenger", q).display_level() <= 4);
        }
    }
}
