//! Dano por BATIDA no carro do jogador — puro e testável.
//!
//! A batida vem do monitor ao vivo (`race_monitor`): G-force → score → severidade, e os
//! componentes `lat/long/vert` dão a DIREÇÃO do impacto. Aqui traduzimos isso em dano nas
//! PEÇAS + custo imediato na fatura, e devolvemos o carro danificado.
//!
//! Regra (do design): a peça atingida **destrói** se já estava perto do fim (wear ≥ 0.85)
//! ou se a batida foi catastrófica → troca (peça nova, custo CHEIO). Senão **amassa** → o
//! wear sobe (a peça vira surrada, carrega risco) e cobra um custo PARCIAL ∝ dano. Todo
//! impacto custa na hora (o medo de bater), e o wear danificado PERSISTE — as próximas
//! corridas ficam mais arriscadas e, se o time for pobre, o cérebro de manutenção pode
//! DEGRADAR a peça (cair de nível) por falta de caixa, piorando o carro de vez.
//!
//! Só o carro do JOGADOR passa por aqui (a IA não é afetada, por decisão de design).
#![allow(dead_code)] // wiring no import da corrida vem depois.

use crate::car::cost::part_cost;
use crate::car::{Car, PartType};

/// Wear em que uma peça atingida é DESTRUÍDA (em vez de só amassada).
const DESTROY_WEAR_THRESHOLD: f64 = 0.85;
/// Custo do amassado = `part_cost × dano × este fator`. Botão do "medo de bater".
const DENT_COST_FACTOR: f64 = 0.60;
/// Teto do wear que um amassado pode atingir (não destrói sozinho — só a via de destruição).
const DENT_WEAR_CAP: f64 = 1.0;

/// Direção do impacto (do sinal dominante do G-force).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactDirection {
    Front,
    Rear,
    Side,
    Vertical,
}

impl ImpactDirection {
    /// Rótulo estável para persistir/serializar (ex.: no monitor).
    pub fn as_str(self) -> &'static str {
        match self {
            ImpactDirection::Front => "front",
            ImpactDirection::Rear => "rear",
            ImpactDirection::Side => "side",
            ImpactDirection::Vertical => "vertical",
        }
    }

    /// Reconstrói do rótulo persistido; desconhecido → `Front` (frontal é o mais comum).
    pub fn from_str(label: &str) -> ImpactDirection {
        match label {
            "rear" => ImpactDirection::Rear,
            "side" => ImpactDirection::Side,
            "vertical" => ImpactDirection::Vertical,
            _ => ImpactDirection::Front,
        }
    }
}

/// Severidade da batida (mapeada da severidade do monitor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashSeverity {
    Light,
    Moderate,
    Heavy,
    Catastrophic,
}

impl CrashSeverity {
    /// Dano de wear que um amassado adiciona. `None` = destrói (catastrófica).
    fn dent_wear(self) -> Option<f64> {
        match self {
            CrashSeverity::Light => Some(0.15),
            CrashSeverity::Moderate => Some(0.30),
            CrashSeverity::Heavy => Some(0.50),
            CrashSeverity::Catastrophic => None,
        }
    }

    /// Mapeia da severidade textual do monitor (`race_monitor::SEVERITIES`). Best-effort —
    /// alinhar com os rótulos reais no wiring. Desconhecida → `Moderate` (meio-termo seguro).
    pub fn from_label(label: &str) -> CrashSeverity {
        let l = label.to_lowercase();
        if l.contains("catastr") || l.contains("destru") {
            CrashSeverity::Catastrophic // "destruído" também destrói as peças
        } else if l.contains("grav") || l.contains("sever") {
            CrashSeverity::Heavy
        } else if l.contains("mod") || l.contains("méd") || l.contains("med") {
            CrashSeverity::Moderate
        } else if l.contains("lev") {
            CrashSeverity::Light
        } else {
            CrashSeverity::Moderate
        }
    }
}

/// Peças atingidas por direção de impacto. Suspensão entra em tudo (é o que mais sofre em
/// qualquer batida). Frente = aero diant./freios; traseira = aero tras./câmbio; lado =
/// laterais/chassi; fundo = assoalho.
fn hit_parts(direction: ImpactDirection) -> &'static [PartType] {
    match direction {
        ImpactDirection::Front => &[PartType::FrontWing, PartType::Suspension, PartType::Brakes],
        ImpactDirection::Rear => &[PartType::RearWing, PartType::Gearbox, PartType::Suspension],
        ImpactDirection::Side => &[PartType::Sidepods, PartType::Suspension, PartType::Chassis],
        ImpactDirection::Vertical => &[PartType::Underbody, PartType::Suspension],
    }
}

/// Direção do impacto a partir dos picos de aceleração (m/s²) do SDK: o eixo de maior
/// magnitude vence. `long` grande = frente/traseira; `lat` = lado; `vert` = fundo (zebra/salto).
pub fn impact_direction(lat: f64, long: f64, vert: f64) -> ImpactDirection {
    let (al, alo, av) = (lat.abs(), long.abs(), vert.abs());
    if av >= al && av >= alo {
        ImpactDirection::Vertical
    } else if alo >= al {
        // Freada/frontal = desaceleração forte (long negativo); traseira = long positivo.
        if long <= 0.0 {
            ImpactDirection::Front
        } else {
            ImpactDirection::Rear
        }
    } else {
        ImpactDirection::Side
    }
}

/// O que a batida fez com o carro (para a fatura e a narrativa).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrashDamage {
    /// Peças destruídas (trocadas por novas — custo cheio).
    pub destroyed: Vec<PartType>,
    /// Peças amassadas: `(peça, wear adicionado)` — custo parcial, wear persiste.
    pub dented: Vec<(PartType, f64)>,
    /// Custo total imediato na fatura.
    pub cost: f64,
}

/// Aplica o dano da batida ao carro do jogador (in-place) e devolve o resumo + custo.
/// Destruída → peça nova (wear zera), custo cheio. Amassada → wear sobe (cap 1.0), custo
/// parcial. O carro danificado é persistido depois; o cérebro de manutenção responde à
/// próxima corrida (trocar/degradar conforme o caixa).
pub fn apply_crash_damage(
    car: &mut Car,
    category_id: &str,
    severity: CrashSeverity,
    direction: ImpactDirection,
) -> CrashDamage {
    let mut out = CrashDamage::default();
    let dent = severity.dent_wear(); // None = destrói tudo que pegar

    for &pt in hit_parts(direction) {
        let Some(part) = car.parts.iter_mut().find(|p| p.part_type == pt) else {
            continue;
        };
        let destroy = dent.is_none() || part.wear >= DESTROY_WEAR_THRESHOLD;
        if destroy {
            out.cost += part_cost(category_id, pt, part.level);
            part.wear = 0.0; // peça nova instalada
            part.spent = false;
            out.destroyed.push(pt);
        } else {
            let dmg = dent.unwrap();
            let before = part.wear;
            part.wear = (part.wear + dmg).min(DENT_WEAR_CAP);
            let added = part.wear - before;
            out.cost += part_cost(category_id, pt, part.level) * added * DENT_COST_FACTOR;
            out.dented.push((pt, added));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direcao_vem_do_eixo_dominante() {
        assert_eq!(impact_direction(0.0, -40.0, 5.0), ImpactDirection::Front);
        assert_eq!(impact_direction(0.0, 40.0, 5.0), ImpactDirection::Rear);
        assert_eq!(impact_direction(35.0, 5.0, 5.0), ImpactDirection::Side);
        assert_eq!(impact_direction(5.0, 5.0, 30.0), ImpactDirection::Vertical);
    }

    #[test]
    fn frontal_atinge_pecas_da_frente() {
        let mut car = Car::uniform(5); // tudo novo (wear 0)
        let d = apply_crash_damage(&mut car, "gt3", CrashSeverity::Moderate, ImpactDirection::Front);
        // As peças da frente amassaram (nenhuma destruída — todas novas).
        assert!(d.destroyed.is_empty());
        let hit: Vec<PartType> = d.dented.iter().map(|(p, _)| *p).collect();
        assert!(hit.contains(&PartType::FrontWing));
        assert!(hit.contains(&PartType::Brakes));
        assert!(hit.contains(&PartType::Suspension));
        // A asa traseira NÃO foi tocada.
        assert!(!hit.contains(&PartType::RearWing));
    }

    #[test]
    fn peca_nova_amassa_e_cobra_parcial() {
        let mut car = Car::uniform(5);
        let d = apply_crash_damage(&mut car, "gt3", CrashSeverity::Heavy, ImpactDirection::Front);
        // Suspensão nova (0.0) + grave (0.5) → wear 0.5, não destruída.
        let susp = car.part(PartType::Suspension).unwrap();
        assert!((susp.wear - 0.5).abs() < 1e-9, "wear deveria subir 0.5, deu {}", susp.wear);
        assert!(d.cost > 0.0, "amassado deveria custar (medo de bater)");
        // Custo parcial < custo cheio de repor tudo.
        let cheio: f64 = [PartType::FrontWing, PartType::Suspension, PartType::Brakes]
            .iter()
            .map(|&p| part_cost("gt3", p, 5))
            .sum();
        assert!(d.cost < cheio, "amassado ({}) deveria custar menos que troca cheia ({cheio})", d.cost);
    }

    #[test]
    fn peca_perto_do_fim_e_destruida_com_custo_cheio() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Suspension, 0.90); // já perto do fim
        let d = apply_crash_damage(&mut car, "gt3", CrashSeverity::Light, ImpactDirection::Front);
        // Suspensão destruída → peça nova (wear 0) + custo cheio dela.
        assert!(d.destroyed.contains(&PartType::Suspension));
        assert!((car.part(PartType::Suspension).unwrap().wear).abs() < 1e-9);
        assert!(d.cost >= part_cost("gt3", PartType::Suspension, 5));
    }

    #[test]
    fn catastrofica_destroi_mesmo_peca_nova() {
        let mut car = Car::uniform(5); // tudo novo
        let d = apply_crash_damage(
            &mut car,
            "gt3",
            CrashSeverity::Catastrophic,
            ImpactDirection::Rear,
        );
        // Todas as peças da traseira destruídas, apesar de novas.
        for pt in [PartType::RearWing, PartType::Gearbox, PartType::Suspension] {
            assert!(d.destroyed.contains(&pt), "{pt:?} deveria ser destruída");
            assert!((car.part(pt).unwrap().wear).abs() < 1e-9, "{pt:?} deveria virar nova");
        }
        assert!(d.dented.is_empty());
    }

    #[test]
    fn amassado_nao_passa_do_teto() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Suspension, 0.80); // abaixo do limiar de destruição (0.85)
        apply_crash_damage(&mut car, "gt3", CrashSeverity::Heavy, ImpactDirection::Side);
        // 0.80 + 0.50 = 1.30 → capado em 1.0 (amassado não destrói sozinho).
        assert!((car.part(PartType::Suspension).unwrap().wear - DENT_WEAR_CAP).abs() < 1e-9);
    }

    #[test]
    fn severidade_do_rotulo() {
        assert_eq!(CrashSeverity::from_label("Catastrófico"), CrashSeverity::Catastrophic);
        assert_eq!(CrashSeverity::from_label("grave"), CrashSeverity::Heavy);
        assert_eq!(CrashSeverity::from_label("moderada"), CrashSeverity::Moderate);
        assert_eq!(CrashSeverity::from_label("leve"), CrashSeverity::Light);
    }
}
