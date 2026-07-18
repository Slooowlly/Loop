//! As 11 peças do carro: durabilidade, viés PHA e custo-base relativo.
//!
//! Números derivados do modelo do GPRO: o viés PHA aproxima os dados de nível 9 ÷ 9
//! (contribuição por nível), e o custo relativo normaliza os custos de nível 1 por
//! Cooling = 1.0. Os valores absolutos NÃO são oficiais — são reescalados por
//! categoria em [`super::cost`]. Ver design em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.
#![allow(dead_code)] // Chunk 1: modelo puro; wiring na simulação/finanças vem nos chunks 5+.

use serde::{Deserialize, Serialize};

/// As 11 peças que compõem o carro. Cada peça empurra o carro numa direção PHA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartType {
    Chassis,
    Engine,
    FrontWing,
    RearWing,
    Underbody,
    Sidepods,
    Cooling,
    Gearbox,
    Brakes,
    Suspension,
    Electronics,
}

impl PartType {
    /// Todas as peças, em ordem estável.
    pub const ALL: [PartType; 11] = [
        PartType::Chassis,
        PartType::Engine,
        PartType::FrontWing,
        PartType::RearWing,
        PartType::Underbody,
        PartType::Sidepods,
        PartType::Cooling,
        PartType::Gearbox,
        PartType::Brakes,
        PartType::Suspension,
        PartType::Electronics,
    ];

    /// Chave estável da peça para persistência no banco.
    pub fn as_str(self) -> &'static str {
        match self {
            PartType::Chassis => "chassis",
            PartType::Engine => "engine",
            PartType::FrontWing => "front_wing",
            PartType::RearWing => "rear_wing",
            PartType::Underbody => "underbody",
            PartType::Sidepods => "sidepods",
            PartType::Cooling => "cooling",
            PartType::Gearbox => "gearbox",
            PartType::Brakes => "brakes",
            PartType::Suspension => "suspension",
            PartType::Electronics => "electronics",
        }
    }

    /// Reconstrói a peça a partir da chave persistida (`None` se desconhecida).
    pub fn from_str(value: &str) -> Option<PartType> {
        match value {
            "chassis" => Some(PartType::Chassis),
            "engine" => Some(PartType::Engine),
            "front_wing" => Some(PartType::FrontWing),
            "rear_wing" => Some(PartType::RearWing),
            "underbody" => Some(PartType::Underbody),
            "sidepods" => Some(PartType::Sidepods),
            "cooling" => Some(PartType::Cooling),
            "gearbox" => Some(PartType::Gearbox),
            "brakes" => Some(PartType::Brakes),
            "suspension" => Some(PartType::Suspension),
            "electronics" => Some(PartType::Electronics),
            _ => None,
        }
    }

    /// Durabilidade em corridas: quantas corridas até atingir 100% de desgaste.
    pub fn durability(self) -> u8 {
        match self {
            PartType::Chassis => 5,
            PartType::Engine => 3,
            PartType::FrontWing => 3,
            PartType::RearWing => 3,
            PartType::Underbody => 5,
            PartType::Sidepods => 4,
            PartType::Cooling => 5,
            PartType::Gearbox => 3,
            PartType::Brakes => 3,
            PartType::Suspension => 3,
            PartType::Electronics => 6,
        }
    }

    /// Contribuição PHA por nível: `(Power, Handling, Acceleration)`. A contribuição
    /// da peça no nível `N` é `pha_per_level * N`.
    pub fn pha_per_level(self) -> (f64, f64, f64) {
        match self {
            PartType::Chassis => (0.78, 1.78, 1.44),
            PartType::Engine => (5.78, 0.56, 2.11),
            PartType::FrontWing => (0.22, 2.44, 1.22),
            PartType::RearWing => (0.22, 2.44, 1.22),
            PartType::Underbody => (0.22, 1.22, 0.56),
            PartType::Sidepods => (0.33, 0.67, 0.0),
            PartType::Cooling => (1.22, 0.0, 0.22),
            PartType::Gearbox => (3.22, 0.67, 4.11),
            PartType::Brakes => (0.0, 2.0, 0.0),
            PartType::Suspension => (0.0, 1.56, 1.22),
            PartType::Electronics => (1.44, 0.0, 1.44),
        }
    }

    /// Custo-base relativo (Cooling = 1.0). Reescalado por categoria em [`super::cost`].
    /// Note: Motor e Câmbio são os mais caros — e também os de vida mais curta.
    pub fn relative_cost(self) -> f64 {
        match self {
            PartType::Chassis => 2.844,
            PartType::Engine => 7.286,
            PartType::FrontWing => 3.413,
            PartType::RearWing => 3.309,
            PartType::Underbody => 1.122,
            PartType::Sidepods => 1.012,
            PartType::Cooling => 1.0,
            PartType::Gearbox => 6.816,
            PartType::Brakes => 1.535,
            PartType::Suspension => 2.599,
            PartType::Electronics => 2.065,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tem_onze_pecas() {
        assert_eq!(PartType::ALL.len(), 11);
    }

    #[test]
    fn motor_e_viesado_para_power() {
        let (p, h, a) = PartType::Engine.pha_per_level();
        assert!(p > h && p > a, "motor deveria ser Power: P={p} H={h} A={a}");
    }

    #[test]
    fn cambio_da_mais_acceleration_que_o_motor() {
        let (_, _, gearbox_a) = PartType::Gearbox.pha_per_level();
        let (_, _, engine_a) = PartType::Engine.pha_per_level();
        assert!(gearbox_a > engine_a, "câmbio deveria dar mais Accel que o motor");
    }

    #[test]
    fn freios_sao_handling_puro() {
        let (p, h, a) = PartType::Brakes.pha_per_level();
        assert!(h > 0.0 && p == 0.0 && a == 0.0, "freios deveriam ser H puro: P={p} H={h} A={a}");
    }

    #[test]
    fn motor_custa_mais_e_dura_menos_que_cooling() {
        assert!(PartType::Engine.relative_cost() > PartType::Cooling.relative_cost());
        assert_eq!(PartType::Engine.durability(), 3);
        assert_eq!(PartType::Electronics.durability(), 6);
    }
}
