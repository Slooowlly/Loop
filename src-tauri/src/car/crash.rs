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
//! O dano de BATIDA (`apply_crash_damage`) é só do carro do JOGADOR — ele depende do G-force
//! do SDK, que a IA não tem. Já o castigo do **contato de disputa** (`apply_contact_wear`)
//! vale para a GRADE TODA: a sim offline sabe quantos roda-a-roda cada carro levou, e essa é
//! a via pela qual a batida da IA finalmente vira desgaste, peça trocada e buraco no caixa.
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

/// Wear que UM contato de disputa deixa na peça castigada. Bem abaixo do amassado de uma
/// batida de verdade (`Light` = 0.15): é roda-a-roda, não impacto. Mas ACUMULA — um carro que
/// passou a corrida se batendo chega na próxima com peça mais perto do fim.
///
/// O desgaste é o efeito COLATERAL do contato, não o mecanismo: medido, ele é inerte (uma
/// equipe troca ~58 peças por temporada e leva ~1,4 pancadas, então o desgaste extra some no
/// ruído). Quem faz o contato pesar é o custo imediato — ver [`CONTACT_COST_FRACTION`].
pub const CONTACT_WEAR_PER_HIT: f64 = 0.06;

/// Custo de reparo de UM contato, como fração do preço da peça castigada.
///
/// É o mesmo princípio do `DENT_COST_FACTOR` da batida do jogador — "o medo de bater" — só que
/// agora vale para a grade toda e é cobrado na fatura da rodada em que aconteceu. Cobrar na
/// hora é o que dá ao contato um efeito visível no caixa: o canal do desgaste tem de vencer uma
/// linha de base de dezenas de trocas por temporada e não vence.
pub const CONTACT_COST_FRACTION: f64 = 0.10;

/// Peças que um contato de disputa castiga, na ordem em que os contatos as visitam. Roda-a-roda
/// é lateral e de aero dianteiro; a suspensão entra porque entra em tudo.
const CONTACT_HIT_PARTS: [PartType; 3] = [
    PartType::FrontWing,
    PartType::Sidepods,
    PartType::Suspension,
];

/// O que os contatos de disputa fizeram com o carro, para a fatura da rodada.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContactDamage {
    /// Peças destruídas: já estavam no fim da vida (`wear ≥ 0.85`) quando levaram a pancada.
    /// Não são reparadas aqui — o chamador força a TROCA (a débito, se não houver caixa).
    pub destroyed: Vec<PartType>,
    /// Custo IMEDIATO de reparo dos amassados, na fatura desta rodada. Não inclui a peça
    /// destruída: essa é cobrada pelo preço cheio na troca forçada, e cobrar aqui duplicaria.
    pub cost: f64,
}

/// Castigo de `hits` contatos de disputa no carro (in-place): amassa a peça, cobra o reparo e
/// aponta o que se destruiu.
///
/// Vale para a grade toda, IA inclusive: é o elo que faltava entre "bateu na corrida" e "o
/// carro piorou / o time gastou". Antes só o carro do jogador tinha consequência de batida.
///
/// Os contatos se espalham pelas peças em rodízio (`CONTACT_HIT_PARTS`) em vez de sortear:
/// mantém a manutenção reprodutível e evita que a azarada de sempre concentre todo o castigo.
pub fn apply_contact_wear(car: &mut Car, category_id: &str, hits: u32) -> ContactDamage {
    apply_contact_wear_with(car, category_id, hits, CONTACT_WEAR_PER_HIT)
}

/// Igual a [`apply_contact_wear`], com o castigo de desgaste explícito. Existe para o harness
/// de medição varrer valores (`simulation::race::tests::medicao`) — a produção usa a constante.
pub fn apply_contact_wear_with(
    car: &mut Car,
    category_id: &str,
    hits: u32,
    wear_per_hit: f64,
) -> ContactDamage {
    let mut out = ContactDamage::default();
    for i in 0..hits as usize {
        let pt = CONTACT_HIT_PARTS[i % CONTACT_HIT_PARTS.len()];
        let Some(part) = car.parts.iter_mut().find(|p| p.part_type == pt) else {
            continue;
        };
        if part.wear >= DESTROY_WEAR_THRESHOLD {
            if !out.destroyed.contains(&pt) {
                out.destroyed.push(pt);
            }
            continue;
        }
        part.wear = (part.wear + wear_per_hit).min(DENT_WEAR_CAP);
        out.cost += part_cost(category_id, pt, part.level) * CONTACT_COST_FRACTION;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contato_acumula_desgaste_sem_destruir_peca_nova() {
        let mut car = Car::uniform(5); // tudo novo
        let d = apply_contact_wear(&mut car, "gt3", 3);

        assert!(
            d.destroyed.is_empty(),
            "peça nova não se destrói num encostão"
        );
        // Três contatos = um em cada peça do rodízio.
        for pt in CONTACT_HIT_PARTS {
            let w = car.part(pt).unwrap().wear;
            assert!(
                (w - CONTACT_WEAR_PER_HIT).abs() < 1e-9,
                "{pt:?} deveria ter levado exatamente um contato, tem wear={w}"
            );
        }
    }

    /// **Bater cobra na hora.** É este o canal pelo qual o contato pesa no orçamento — o
    /// desgaste sozinho some no ruído das dezenas de trocas por temporada.
    #[test]
    fn contato_cobra_reparo_imediato_proporcional_a_peca() {
        let mut car = Car::uniform(5);
        let d = apply_contact_wear(&mut car, "gt3", 1);

        let asa = car.part(PartType::FrontWing).unwrap();
        let esperado = part_cost("gt3", PartType::FrontWing, asa.level) * CONTACT_COST_FRACTION;
        assert!(
            (d.cost - esperado).abs() < 1e-6,
            "um contato devia cobrar {esperado}, cobrou {}",
            d.cost
        );
        assert!(d.cost > 0.0);
    }

    #[test]
    fn contato_destroi_peca_que_ja_estava_no_fim_da_vida() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::FrontWing, 0.90); // acima do limiar de destruição

        let d = apply_contact_wear(&mut car, "gt3", 1);

        assert_eq!(d.destroyed, vec![PartType::FrontWing]);
        // Não é reparada aqui — quem troca é o cérebro de manutenção, a débito.
        assert!((car.part(PartType::FrontWing).unwrap().wear - 0.90).abs() < 1e-9);
        // E não é cobrada aqui: a troca forçada cobra o preço CHEIO. Cobrar duplicaria.
        assert_eq!(
            d.cost, 0.0,
            "peça destruída não pode ser cobrada duas vezes"
        );
    }

    #[test]
    fn sem_contato_o_carro_nao_muda() {
        let mut car = Car::uniform(5);
        let antes = car.clone();

        assert_eq!(
            apply_contact_wear(&mut car, "gt3", 0),
            ContactDamage::default()
        );
        assert_eq!(car, antes);
    }

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
        let d = apply_crash_damage(
            &mut car,
            "gt3",
            CrashSeverity::Moderate,
            ImpactDirection::Front,
        );
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
        let d = apply_crash_damage(
            &mut car,
            "gt3",
            CrashSeverity::Heavy,
            ImpactDirection::Front,
        );
        // Suspensão nova (0.0) + grave (0.5) → wear 0.5, não destruída.
        let susp = car.part(PartType::Suspension).unwrap();
        assert!(
            (susp.wear - 0.5).abs() < 1e-9,
            "wear deveria subir 0.5, deu {}",
            susp.wear
        );
        assert!(d.cost > 0.0, "amassado deveria custar (medo de bater)");
        // Custo parcial < custo cheio de repor tudo.
        let cheio: f64 = [PartType::FrontWing, PartType::Suspension, PartType::Brakes]
            .iter()
            .map(|&p| part_cost("gt3", p, 5))
            .sum();
        assert!(
            d.cost < cheio,
            "amassado ({}) deveria custar menos que troca cheia ({cheio})",
            d.cost
        );
    }

    #[test]
    fn peca_perto_do_fim_e_destruida_com_custo_cheio() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Suspension, 0.90); // já perto do fim
        let d = apply_crash_damage(
            &mut car,
            "gt3",
            CrashSeverity::Light,
            ImpactDirection::Front,
        );
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
            assert!(
                (car.part(pt).unwrap().wear).abs() < 1e-9,
                "{pt:?} deveria virar nova"
            );
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
        assert_eq!(
            CrashSeverity::from_label("Catastrófico"),
            CrashSeverity::Catastrophic
        );
        assert_eq!(CrashSeverity::from_label("grave"), CrashSeverity::Heavy);
        assert_eq!(
            CrashSeverity::from_label("moderada"),
            CrashSeverity::Moderate
        );
        assert_eq!(CrashSeverity::from_label("leve"), CrashSeverity::Light);
    }
}
