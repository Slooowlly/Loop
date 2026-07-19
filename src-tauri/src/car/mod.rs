//! Modelo do carro: 11 peças → PHA → Nível → custo.
//!
//! Puro e testável — não toca DB nem simulação (isso é wiring dos chunks 5+). O carro
//! é um **vetor**: a magnitude (total PHA) alimenta o `car_performance` da simulação; a
//! direção (P/H/A) é o shape que casa com a pista. O jogador só vê o **Nível do Carro
//! (1–10)** = média dos níveis das peças. Ver design em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.
#![allow(dead_code)] // Chunk 1: modelo puro; wiring na simulação/finanças vem nos chunks 5+.

pub mod breakdown;
pub mod cost;
pub mod crash;
pub mod parts;
pub mod seed;
pub mod sim_bridge;
pub mod wear;

/// Harness de experimento (Monte Carlo) do sistema de quebra — só em teste, throwaway.
#[cfg(test)]
mod breakdown_sim;

pub use parts::PartType;

use serde::{Deserialize, Serialize};

/// Uma peça instalada no carro: tipo, nível (1..=10) e desgaste (`0.0` = nova,
/// `1.0` = 100% desgastada; acima de `1.0` = em sobreuso). `spent` marca a peça que
/// esgotou seu bônus de "esticar" — precisa ser trocada obrigatoriamente.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CarPart {
    pub part_type: PartType,
    pub level: u8,
    pub wear: f64,
    pub spent: bool,
}

/// O carro completo: as 11 peças.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Car {
    pub parts: Vec<CarPart>,
}

impl Car {
    /// Carro com todas as peças no mesmo nível, sem desgaste. Helper de seed/teste.
    pub fn uniform(level: u8) -> Car {
        Car {
            parts: PartType::ALL
                .iter()
                .map(|&part_type| CarPart {
                    part_type,
                    level,
                    wear: 0.0,
                    spent: false,
                })
                .collect(),
        }
    }

    /// Carro spec de rookie: tudo no nível 1 (todos os carros idênticos, sem shape
    /// relevante). O jogador vê Nível 1 e não gerencia peças.
    pub fn spec_rookie() -> Car {
        Car::uniform(1)
    }

    /// Define o nível de uma peça específica (no-op se a peça não existir).
    pub fn set_level(&mut self, part_type: PartType, level: u8) {
        if let Some(part) = self.parts.iter_mut().find(|p| p.part_type == part_type) {
            part.level = level;
        }
    }

    /// Define o desgaste de uma peça específica (no-op se a peça não existir).
    /// Helper de seed/teste.
    pub fn set_wear(&mut self, part_type: PartType, wear: f64) {
        if let Some(part) = self.parts.iter_mut().find(|p| p.part_type == part_type) {
            part.wear = wear;
        }
    }

    /// Referência imutável a uma peça pelo tipo (helper de teste).
    pub fn part(&self, part_type: PartType) -> Option<&CarPart> {
        self.parts.iter().find(|p| p.part_type == part_type)
    }

    /// Vetor PHA do carro = soma das contribuições de cada peça no seu nível.
    /// `(Power, Handling, Acceleration)`.
    pub fn pha(&self) -> (f64, f64, f64) {
        self.parts.iter().fold((0.0, 0.0, 0.0), |(p, h, a), part| {
            let (pp, ph, pa) = part.part_type.pha_per_level();
            let level = part.level as f64;
            (p + pp * level, h + ph * level, a + pa * level)
        })
    }

    /// Magnitude escalar do carro = total de PHA (`P + H + A`). É o que alimenta o
    /// `car_performance` da simulação no chunk 5 (a dominância age sobre isto).
    pub fn magnitude(&self) -> f64 {
        let (p, h, a) = self.pha();
        p + h + a
    }

    /// Nível do Carro exibido (1..=10) = média aritmética dos níveis das peças,
    /// arredondada e clampada. ÚNICA leitura que o jogador vê.
    pub fn display_level(&self) -> u8 {
        if self.parts.is_empty() {
            return 1;
        }
        let sum: u32 = self.parts.iter().map(|p| p.level as u32).sum();
        let mean = sum as f64 / self.parts.len() as f64;
        (mean.round() as i64).clamp(1, 10) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carro_uniforme_e_aproximadamente_balanceado() {
        let (p, h, a) = Car::uniform(6).pha();
        let max = p.max(h).max(a);
        let min = p.min(h).min(a);
        assert!(
            (max - min) / max < 0.05,
            "carro uniforme deveria ser ~balanceado: P={p} H={h} A={a}"
        );
    }

    #[test]
    fn build_focado_em_potencia_fica_viesado_para_power() {
        let mut car = Car::uniform(1);
        for part in [
            PartType::Engine,
            PartType::Gearbox,
            PartType::Cooling,
            PartType::Electronics,
        ] {
            car.set_level(part, 10);
        }
        let (p, h, _a) = car.pha();
        assert!(p > h, "build de potência deveria ter P>H: P={p} H={h}");
    }

    #[test]
    fn magnitude_cresce_com_o_nivel() {
        assert!(Car::uniform(8).magnitude() > Car::uniform(3).magnitude());
    }

    #[test]
    fn nivel_exibido_e_media_arredondada_e_clampada() {
        assert_eq!(Car::uniform(1).display_level(), 1);
        assert_eq!(Car::uniform(7).display_level(), 7);
        assert_eq!(Car::uniform(10).display_level(), 10);
        // média = (10 peças * 5 + 1 * 10) / 11 = 60/11 = 5.45 → 5
        let mut car = Car::uniform(5);
        car.set_level(PartType::Engine, 10);
        assert_eq!(car.display_level(), 5);
    }

    #[test]
    fn rookie_spec_e_nivel_um() {
        let car = Car::spec_rookie();
        assert_eq!(car.display_level(), 1);
        assert_eq!(car.parts.len(), 11);
        assert!(car.parts.iter().all(|p| p.level == 1));
    }
}
