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

    /// Durabilidade em corridas: quantas corridas até atingir 100% de desgaste. As asas foram
    /// DIFERENCIADAS (redesign 2026-07-22 §4.7): a dianteira leva mais dano (zebra/contato) e é
    /// mais frágil (3); a traseira é mais robusta (4). Isso, somado ao `unit_seed`, garante que
    /// dianteira e traseira NUNCA quebrem em lockstep, e espalha o antigo cluster de durab-3.
    pub fn durability(self) -> u8 {
        match self {
            PartType::Chassis => 5,
            PartType::Engine => 3,
            PartType::FrontWing => 3,
            PartType::RearWing => 4,
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

    /// Nome legível da peça, **adaptado ao carro da categoria** (só nomenclatura). As peças
    /// aerodinâmicas mudam de nome conforme o carro: um Mazda MX-5 não tem asa (vira
    /// "parachoque"); GT/Toyota/BMW têm asa traseira. Usado na narrativa da quebra.
    pub fn display_name(self, category_id: &str) -> String {
        let sem_asa = category_id.starts_with("mazda"); // Mazda MX-5: sem aerofólio
        let key = match self {
            PartType::Chassis => "chassis",
            PartType::Engine => "engine",
            // Mazda MX-5: sem aero → parachoque; GT/BMW/Toyota/proto: splitter/asa.
            PartType::FrontWing => {
                if sem_asa {
                    "front_wing_bumper"
                } else {
                    "front_wing_aero"
                }
            }
            PartType::RearWing => {
                if sem_asa {
                    "rear_wing_bumper"
                } else {
                    "rear_wing_aero"
                }
            }
            PartType::Underbody => "underbody",
            PartType::Sidepods => "sidepods",
            PartType::Cooling => "cooling",
            PartType::Gearbox => "gearbox",
            PartType::Brakes => "brakes",
            PartType::Suspension => "suspension",
            PartType::Electronics => "electronics",
        };
        let full = format!("part.{key}");
        rust_i18n::t!(&full).to_string()
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

    /// **Os números do carro continuam sendo os do GPRO, e são eles que definem o tradeoff
    /// desempenho × confiabilidade do jogo inteiro.**
    ///
    /// [`PartType::durability`] e [`PartType::pha_per_level`] vieram do modelo do GPRO por
    /// aproximação (nível 9 ÷ 9), não de nenhuma medição do Loop. Junto com
    /// `car::wear::level_durability_mult` (a tenda) e `car::sim_bridge::CAR_PERF_MAX`, eles
    /// decidem quanto ritmo um nível compra e quanta vida ele cobra — a decisão central do
    /// Sistema de Nível do Carro.
    ///
    /// **O que falta é uma varredura**, e ela não cabe aqui: medir isto é rodar temporadas
    /// variando cada coeficiente e olhando a distribuição de resultado e de quebra, que é o
    /// que `simulation::calibracao::varredura` faz para os knobs da corrida. Enquanto esses
    /// coeficientes não forem knobs de lá, o teste trava os valores para que a mudança seja
    /// deliberada, e trava também as duas amarrações que já existem e que uma edição
    /// distraída romperia:
    ///
    /// 1. A **durabilidade mínima 3** é o que ancora `ENDURO_SURCHARGE_CAP` — baixá-la deixa
    ///    uma prova de enduro consumir mais de uma vida de peça de novo.
    /// 2. A durabilidade entra em `wear_per_race` como `1/durabilidade`, então ela é a
    ///    unidade da economia inteira de peça, não só do risco de quebra.
    #[test]
    fn os_coeficientes_do_carro_seguem_herdados_do_gpro() {
        let durabilidades: Vec<u8> = PartType::ALL.iter().map(|p| p.durability()).collect();
        assert_eq!(durabilidades, vec![5, 3, 3, 4, 5, 4, 5, 3, 3, 3, 6]);
        assert_eq!(
            durabilidades.iter().copied().min(),
            Some(3),
            "a durabilidade mínima ancora o teto do sobrecusto de enduro"
        );

        // O somatório PHA de cada peça — a "força" que um nível dela compra. É o que uma
        // varredura mediria, e o que uma edição distraída moveria sem perceber.
        let soma_pha = |pt: PartType| {
            let (p, h, a) = pt.pha_per_level();
            p + h + a
        };
        let total: f64 = PartType::ALL.iter().map(|&p| soma_pha(p)).sum();
        assert!(
            (total - 40.31).abs() < 0.01,
            "o PHA total por nível mudou: {total:.2}"
        );
        // O motor é a peça que mais compra ritmo, e a lataria a que menos — a hierarquia que
        // o modelo do GPRO trouxe e que a escolha de upgrade do cérebro de manutenção segue.
        assert_eq!(
            PartType::ALL
                .iter()
                .copied()
                .max_by(|a, b| soma_pha(*a).total_cmp(&soma_pha(*b))),
            Some(PartType::Engine)
        );
        assert_eq!(
            PartType::ALL
                .iter()
                .copied()
                .min_by(|a, b| soma_pha(*a).total_cmp(&soma_pha(*b))),
            Some(PartType::Sidepods)
        );
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
        assert!(
            gearbox_a > engine_a,
            "câmbio deveria dar mais Accel que o motor"
        );
    }

    #[test]
    fn freios_sao_handling_puro() {
        let (p, h, a) = PartType::Brakes.pha_per_level();
        assert!(
            h > 0.0 && p == 0.0 && a == 0.0,
            "freios deveriam ser H puro: P={p} H={h} A={a}"
        );
    }

    #[test]
    fn motor_custa_mais_e_dura_menos_que_cooling() {
        assert!(PartType::Engine.relative_cost() > PartType::Cooling.relative_cost());
        assert_eq!(PartType::Engine.durability(), 3);
        assert_eq!(PartType::Electronics.durability(), 6);
    }

    #[test]
    #[serial_test::serial]
    fn nome_da_peca_adapta_ao_carro() {
        rust_i18n::set_locale("pt-BR"); // display_name resolve no locale ativo.
                                        // Mazda não tem asa → parachoque.
        assert_eq!(
            PartType::RearWing.display_name("mazda_rookie"),
            "Parachoque traseiro"
        );
        assert_eq!(
            PartType::FrontWing.display_name("mazda_amador"),
            "Parachoque dianteiro"
        );
        // GT/BMW/Toyota têm asa traseira e splitter dianteiro.
        assert_eq!(PartType::RearWing.display_name("gt3"), "Asa traseira");
        assert_eq!(PartType::RearWing.display_name("bmw_m2"), "Asa traseira");
        assert_eq!(PartType::FrontWing.display_name("gt3"), "Splitter");
        assert_eq!(PartType::FrontWing.display_name("gt4"), "Splitter");
        // Peça não-aero é constante.
        assert_eq!(PartType::Engine.display_name("mazda_rookie"), "Motor");
        assert_eq!(PartType::Gearbox.display_name("gt3"), "Câmbio");
    }
}
