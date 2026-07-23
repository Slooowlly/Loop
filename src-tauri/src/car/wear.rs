//! Desgaste e ciclo de vida das peças — transformações puras (sem DB nem sim).
//!
//! Cada corrida consome `1/durabilidade` de desgaste. No fim da vida (≥100%), o time
//! decide por peça entre **trocar** (zera o desgaste), **esticar** (paga reduzido, roda
//! +1 corrida e a peça morre) ou **degradar** (cai 1 nível/corrida, o Nível do Carro
//! sangra). A eligibilidade de esticar exige desgaste ≤95%. Ver design §5 em
//! `docs/superpowers/specs/2026-07-17-car-level-system-design.md`.
#![allow(dead_code)] // Chunk 2: mecânica pura; o cérebro do time (chunk 3) decide as ações.

use std::collections::HashMap;

use crate::car::{cost, Car, CarPart, PartType};

/// Desgaste máximo (fração) que ainda permite esticar a peça por +1 corrida.
pub const STRETCH_MAX_WEAR: f64 = 0.95;

// ───────────────────────── Identidade da unidade (redesign 2026-07-22 §4.1) ─────────────────────────

/// Variância de vida INDIVIDUAL por unidade instalada (±fração). Uma peça pode ser "boa"
/// (dura mais) ou um "limão" (dura menos). Deriva do `unit_seed` da peça. Calibrável.
const LIFE_VAR: f64 = 0.18;
/// Sal do re-roll do `unit_seed` a cada instalação (nova unidade = nova sorte).
const UNIT_SEED_REROLL_SALT: u32 = 0x632B_E5AB;
/// Base do seed inicial por tipo de peça (as 11 largam com sementes distintas).
const UNIT_SEED_BASE: u32 = 0x9E37_79B9;

/// Misturador splitmix32 — puro, determinístico, boa avalanche em 32 bits.
fn splitmix32(mut x: u32) -> u32 {
    x = x.wrapping_add(0x9E37_79B9);
    x = (x ^ (x >> 16)).wrapping_mul(0x21F0_AAAD);
    x = (x ^ (x >> 15)).wrapping_mul(0x735A_2D97);
    x ^ (x >> 15)
}

/// Seed → [0,1) uniforme.
fn hash01(seed: u32) -> f64 {
    splitmix32(seed) as f64 / (u32::MAX as f64 + 1.0)
}

/// Seed inicial DISTINTO e determinístico por tipo de peça (não-zero). Usado por
/// [`Car::uniform`](crate::car::Car::uniform) e como fallback ao carregar saves antigos
/// (onde `unit_seed == 0`), pra as 11 peças já largarem dessincronizadas.
pub fn initial_unit_seed(part_type: PartType) -> u32 {
    let idx = PartType::ALL
        .iter()
        .position(|&p| p == part_type)
        .unwrap_or(0) as u32;
    splitmix32(UNIT_SEED_BASE ^ idx.wrapping_mul(0x8546_1CE9)) | 1
}

/// Re-rola o `unit_seed` numa nova instalação (a peça trocada é uma unidade NOVA, com sorte
/// própria). Determinístico e encadeado a partir do seed anterior.
pub fn reroll_unit_seed(prev: u32) -> u32 {
    splitmix32(prev ^ UNIT_SEED_REROLL_SALT) | 1
}

/// Escala de vida INDIVIDUAL da unidade (∈ `[1-LIFE_VAR, 1+LIFE_VAR]`). `>1` = peça durável
/// (gasta mais devagar, dura mais corridas); `<1` = limão. É o divisor do desgaste acumulado,
/// então NÃO muda o significado de `wear` (100% = fim da vida nominal) — só a VELOCIDADE com
/// que cada unidade chega lá. Seed `0` (save antigo) cai no fallback por tipo via o chamador.
pub fn life_scale(unit_seed: u32) -> f64 {
    let u = hash01(unit_seed); // [0,1)
    1.0 + LIFE_VAR * (2.0 * u - 1.0)
}

/// Escala de vida efetiva de uma peça já instalada — resolve o `unit_seed` `0` (save antigo)
/// para o fallback determinístico por tipo. Fonte ÚNICA usada pela economia e pela quebra.
pub fn part_life_scale(part: &CarPart) -> f64 {
    let seed = if part.unit_seed == 0 {
        initial_unit_seed(part.part_type)
    } else {
        part.unit_seed
    };
    life_scale(seed)
}

// ───────────────────────── Tenda de durabilidade por NÍVEL (redesign 2026-07-22 §4.8) ─────────────────────────

/// Multiplicador de VIDA pela CURVA-TENDA do nível: pico no 5 (maduro/confiável), caindo
/// simétrico para os extremos — nível baixo é BARATO/mal-feito (frágil), nível alto é de PONTA
/// e altamente estressado (frágil). Cria o tradeoff DESEMPENHO×CONFIABILIDADE: subir de nível dá
/// ritmo mas custa durabilidade. O guard de categoria spec fica em [`part_effective_life`].
pub fn level_durability_mult(level: u8) -> f64 {
    match level.clamp(1, 10) {
        1 => 0.60,
        2 => 0.75,
        3 => 0.88,
        4 => 1.00,
        5 => 1.15,
        6 => 1.00,
        7 => 0.88,
        8 => 0.75,
        9 => 0.60,
        _ => 0.50, // 10 (bleeding edge)
    }
}

/// Vida EFETIVA da unidade = individual (`unit_seed`) × tenda de nível. `apply_tent = false`
/// nas categorias SPEC (teto ≤ 2), onde nível baixo é a NORMA (não sucata) — sem o guard, o
/// carro do iniciante viveria de quebra. É a fonte ÚNICA da vida, usada pela economia (via
/// [`apply_action_then_race`]/`needs_decision`) e pela quebra ao vivo (`LiveBreakdown`).
pub fn part_effective_life(part: &CarPart, apply_tent: bool) -> f64 {
    let base = part_life_scale(part);
    if apply_tent {
        base * level_durability_mult(part.level)
    } else {
        base
    }
}

// ───────────────────────── Confiabilidade do time (redesign 2026-07-22 §4.2) ─────────────────────────

/// Ganho MÁXIMO de vida de peça pela confiabilidade do time (±fração no melhor/pior time).
const REL_LIFE_GAIN: f64 = 0.12;

/// Multiplicador de vida de peça pela CONFIABILIDADE do time, medida pela qualidade da equipe
/// de box (`pit_crew_quality` 0-100 = quão bem cuidam do carro). Time excelente (100) →
/// `1 + REL_LIFE_GAIN` (peças duram mais); neutro (50) → 1.0; fraco (0) → `1 - REL_LIFE_GAIN`.
/// Multiplica a vida EFETIVA na economia → propaga à quebra pelo desgaste de entrada. Substitui
/// o antigo `PLAYER_MAX_RELIEF`: a "proteção via manutenção" do jogador vira o pit crew do time
/// dele (quem investe em equipe ganha confiabilidade); vale igual pra IA (variedade no grid).
pub fn reliability_life_mult(pit_crew_quality: f64) -> f64 {
    let rel = (pit_crew_quality.clamp(0.0, 100.0)) / 100.0; // 0..1
    1.0 + REL_LIFE_GAIN * (2.0 * rel - 1.0)
}
/// Custo de esticar, como fração do preço de uma peça nova do mesmo nível.
pub const STRETCH_COST_FRACTION: f64 = 0.40;

/// Voltas de REFERÊNCIA que ancoram a durabilidade (expressa "em corridas") ao desgaste
/// POR VOLTA: uma corrida deste tamanho consome exatamente `1/durabilidade` da peça —
/// então a cadência de troca da economia fica IDÊNTICA à histórica. É também a ponte
/// usada pelo Sistema de Quebra (risco por volta, ciente do tamanho real da corrida).
/// Ver `docs/superpowers/specs/2026-07-18-car-breakdown-system.md` §5.
pub const REF_RACE_LAPS: f64 = 18.0;

/// O que o time faz com uma peça nesta corrida. Default = `Keep`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartAction {
    /// Roda a peça normalmente (só acumula desgaste).
    Keep,
    /// Compra peça nova no nível atual: zera desgaste e limpa `spent`.
    Replace,
    /// Paga reduzido por +1 corrida; ao fim dela a peça fica `spent` (troca obrigatória).
    Stretch,
    /// Deixa a peça passar de 100%: cai 1 nível nesta corrida (Nível do Carro cai).
    Degrade,
}

/// Fração de desgaste que uma corrida de REFERÊNCIA adiciona a esta peça
/// (= `1/durabilidade`). Valor histórico preservado EXATAMENTE — a economia não muda.
pub fn wear_per_race(part_type: PartType) -> f64 {
    1.0 / part_type.durability() as f64
}

/// Fração de desgaste que uma peça acumula por VOLTA (= `wear_per_race / REF_RACE_LAPS`
/// = `1/(durabilidade × voltas-ref)`). Base COMPARTILHADA: a economia soma um bloco de
/// corrida (via [`wear_per_race`], inalterada); o Sistema de Quebra soma por volta na
/// corrida ao vivo, então uma corrida do tamanho de referência acumula o mesmo total, e
/// corridas mais longas/curtas escalam proporcionalmente (o "tamanho importa" da quebra).
pub fn wear_per_lap(part_type: PartType) -> f64 {
    wear_per_race(part_type) / REF_RACE_LAPS
}

/// A peça pode ser esticada? Só se não estiver esgotada e o desgaste for ≤95%.
pub fn can_stretch(part: &CarPart) -> bool {
    !part.spent && part.wear <= STRETCH_MAX_WEAR
}

/// Nível que a peça nova terá ao substituir esta. **PENALIDADE DE SOBREUSO:** uma peça
/// esticada (`spent`) — forçada além do limite — só pode ser reposta por uma **UM NÍVEL
/// ABAIXO** (nível 4 esticado → só dá pra comprar nível 3). Sem isso, esticar seria
/// sempre grátis e todo time ficaria forçando peça pra sempre.
pub fn replacement_level(part: &CarPart) -> u8 {
    if part.spent {
        part.level.saturating_sub(1).max(1)
    } else {
        part.level
    }
}

/// Custo de trocar por uma peça nova. Peça esticada é reposta um nível abaixo (mais
/// barata, mas com perda de nível — a punição do sobreuso).
pub fn replace_cost(category_id: &str, part: &CarPart) -> f64 {
    cost::part_cost(category_id, part.part_type, replacement_level(part))
}

/// Custo de esticar = fração do preço de uma peça nova.
pub fn stretch_cost(category_id: &str, part: &CarPart) -> f64 {
    STRETCH_COST_FRACTION * replace_cost(category_id, part)
}

/// Aplica a ação escolhida e roda a corrida (acumula desgaste) para uma peça. O
/// `wear_mult` escala o desgaste desta corrida pelas condições reais (pista × clima × —
/// só no jogador — estilo de pilotagem). `1.0` = corrida de referência neutra.
fn apply_action_then_race(
    part: &mut CarPart,
    action: PartAction,
    wear_mult: f64,
    apply_tent: bool,
    rel_mult: f64,
) {
    match action {
        PartAction::Replace => {
            // A punição do sobreuso incide AQUI: repor uma peça esticada cai um nível.
            part.level = replacement_level(part);
            part.wear = 0.0;
            part.spent = false;
            // Unidade NOVA → nova sorte de vida (redesign 2026-07-22 §4.1).
            part.unit_seed = reroll_unit_seed(part.unit_seed);
        }
        PartAction::Degrade => {
            // Só derruba nível se a peça de fato passou de 100%.
            if part.wear >= 1.0 {
                part.level = part.level.saturating_sub(1).max(1);
            }
        }
        PartAction::Keep | PartAction::Stretch => {}
    }

    // A corrida aconteceu: acumula desgaste, modulado pelas condições reais E pela vida EFETIVA
    // da unidade (individual × tenda de nível × confiabilidade do time) — peça durável/nível-5/de
    // time confiável gasta devagar; limão/nível-extremo/time fraco, rápido. Dessincroniza peças de
    // mesma durabilidade, cria o tradeoff de nível E a variedade de confiabilidade por time.
    part.wear +=
        wear_per_race(part.part_type) * wear_mult / (part_effective_life(part, apply_tent) * rel_mult);

    // Peça esticada esgota o bônus ao fim da sua corrida extra.
    if action == PartAction::Stretch {
        part.spent = true;
    }
}

/// Avança uma corrida no carro inteiro, aplicando as decisões do time por peça
/// (peças ausentes no mapa recebem `Keep`). Desgaste NEUTRO (mult 1.0) — corrida de
/// referência. Aplica a tenda de nível (categoria gerida) e confiabilidade NEUTRA (rel 1.0);
/// use [`advance_race_scaled`] para controlar `apply_tent`/`rel_mult`.
pub fn advance_race(car: &mut Car, decisions: &HashMap<PartType, PartAction>) {
    advance_race_scaled(car, decisions, |_| 1.0, true, 1.0);
}

/// Igual a [`advance_race`], mas cada peça acumula `wear_per_race × wear_mult(peça)` —
/// o desgaste responde às condições REAIS da corrida (pista + clima da grade toda; +
/// estilo de pilotagem só no carro do jogador). O wear resultante PERSISTE e alimenta o
/// cérebro de manutenção do time (trocar/esticar/degradar). Com `wear_mult ≡ 1.0` o
/// resultado é idêntico a [`advance_race`].
pub fn advance_race_scaled(
    car: &mut Car,
    decisions: &HashMap<PartType, PartAction>,
    wear_mult: impl Fn(PartType) -> f64,
    apply_tent: bool,
    rel_mult: f64,
) {
    for part in car.parts.iter_mut() {
        let action = decisions
            .get(&part.part_type)
            .copied()
            .unwrap_or(PartAction::Keep);
        apply_action_then_race(part, action, wear_mult(part.part_type), apply_tent, rel_mult);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(part_type: PartType, action: PartAction) -> HashMap<PartType, PartAction> {
        let mut d = HashMap::new();
        d.insert(part_type, action);
        d
    }

    // -------- Escala de desgaste por condições (pista × clima × estilo) --------

    /// REGRESSÃO: `advance_race_scaled` com mult ≡ 1.0 é byte-idêntico a `advance_race`.
    /// A economia neutra NÃO pode mudar ao introduzir a modulação por condições.
    #[test]
    fn escala_neutra_e_identica_a_advance_race() {
        let decisions: HashMap<PartType, PartAction> = HashMap::new(); // tudo Keep
        let mut a = Car::uniform(5);
        let mut b = Car::uniform(5);
        advance_race(&mut a, &decisions);
        advance_race_scaled(&mut b, &decisions, |_| 1.0, true, 1.0);
        for &pt in &PartType::ALL {
            assert_eq!(
                a.part(pt).unwrap().wear,
                b.part(pt).unwrap().wear,
                "{pt:?}: escala neutra deveria bater com advance_race"
            );
        }
    }

    /// O multiplicador escala o INCREMENTO desta corrida (2× dobra o desgaste somado). O total
    /// também é dividido pela vida INDIVIDUAL da unidade (redesign §4.1).
    #[test]
    fn multiplicador_escala_o_incremento() {
        let decisions: HashMap<PartType, PartAction> = HashMap::new();
        let mut car = Car::uniform(5); // wear 0
        advance_race_scaled(&mut car, &decisions, |_| 2.0, true, 1.0);
        for &pt in &PartType::ALL {
            let part = car.part(pt).unwrap();
            let esperado = wear_per_race(pt) * 2.0 / part_effective_life(part, true);
            assert!(
                (part.wear - esperado).abs() < 1e-12,
                "{pt:?}: mult 2× deveria somar {esperado}, deu {}",
                part.wear
            );
        }
    }

    /// O mult é POR PEÇA: castigar só o motor não toca as outras peças (o desgaste de cada uma
    /// ainda é dividido pela sua vida individual).
    #[test]
    fn multiplicador_e_por_peca() {
        let decisions: HashMap<PartType, PartAction> = HashMap::new();
        let mut car = Car::uniform(5);
        advance_race_scaled(
            &mut car,
            &decisions,
            |pt| {
                if pt == PartType::Engine {
                    3.0
                } else {
                    1.0
                }
            },
            true,
            1.0,
        );
        let motor = car.part(PartType::Engine).unwrap();
        let freios = car.part(PartType::Brakes).unwrap();
        assert!(
            (motor.wear - wear_per_race(PartType::Engine) * 3.0 / part_effective_life(motor, true))
                .abs()
                < 1e-12
        );
        assert!(
            (freios.wear - wear_per_race(PartType::Brakes) / part_effective_life(freios, true)).abs()
                < 1e-12
        );
    }

    // -------- Confiabilidade do time (§4.2) --------

    #[test]
    fn confiabilidade_escala_com_o_pit_crew() {
        assert!((reliability_life_mult(50.0) - 1.0).abs() < 1e-9, "neutro em 50");
        assert!(reliability_life_mult(100.0) > 1.0, "box top → mais vida");
        assert!(reliability_life_mult(0.0) < 1.0, "box fraco → menos vida");
        assert!(reliability_life_mult(0.0) < reliability_life_mult(100.0));
    }

    #[test]
    fn time_com_box_bom_desgasta_menos_o_carro() {
        // MESMO carro/corrida — só a confiabilidade (pit crew) muda. Box top gasta MENOS.
        let decisions: HashMap<PartType, PartAction> = HashMap::new();
        let mut bom = Car::uniform(5);
        let mut fraco = Car::uniform(5);
        advance_race_scaled(&mut bom, &decisions, |_| 1.0, true, reliability_life_mult(100.0));
        advance_race_scaled(&mut fraco, &decisions, |_| 1.0, true, reliability_life_mult(0.0));
        let soma = |c: &Car| c.parts.iter().map(|p| p.wear).sum::<f64>();
        assert!(soma(&bom) < soma(&fraco), "box top ({}) deveria gastar menos que fraco ({})", soma(&bom), soma(&fraco));
    }

    // -------- Fundação da escala unificada (Fase 1 do Sistema de Quebra) --------

    /// REGRESSÃO: `wear_per_race` continua sendo EXATAMENTE `1/durabilidade` para toda
    /// peça — a economia/finança não muda com a introdução da escala por volta.
    #[test]
    fn wear_per_race_permanece_um_sobre_durabilidade() {
        for &pt in &PartType::ALL {
            assert_eq!(
                wear_per_race(pt),
                1.0 / pt.durability() as f64,
                "wear_per_race de {:?} mudou — a economia NÃO pode se alterar",
                pt
            );
        }
    }

    /// Uma corrida do TAMANHO DE REFERÊNCIA (somando por volta) acumula o mesmo total que
    /// um bloco de corrida — a ponte entre a economia (por corrida) e a quebra (por volta).
    #[test]
    fn desgaste_por_volta_x_voltas_ref_bate_com_a_corrida() {
        for &pt in &PartType::ALL {
            let por_voltas = wear_per_lap(pt) * REF_RACE_LAPS;
            assert!(
                (por_voltas - wear_per_race(pt)).abs() < 1e-12,
                "{:?}: {} voltas de desgaste ({}) deveria bater com a corrida ({})",
                pt,
                REF_RACE_LAPS,
                por_voltas,
                wear_per_race(pt)
            );
        }
    }

    /// Corrida mais LONGA que a referência gasta MAIS por corrida; mais curta gasta menos
    /// (o "tamanho da corrida importa" — usado só na corrida ao vivo, não na economia da IA).
    #[test]
    fn corrida_mais_longa_gasta_mais_por_volta() {
        let pt = PartType::Engine;
        let sprint = wear_per_lap(pt) * 14.0;
        let referencia = wear_per_lap(pt) * REF_RACE_LAPS;
        let enduro = wear_per_lap(pt) * 50.0;
        assert!(sprint < referencia, "sprint curto deveria gastar menos");
        assert!(enduro > referencia, "enduro longo deveria gastar mais");
    }

    #[test]
    fn desgaste_sobe_conforme_a_durabilidade() {
        let mut car = Car::uniform(5);
        advance_race(&mut car, &HashMap::new()); // tudo Keep
        // Motor dura 3 corridas → +1/3 por corrida; Eletrônica dura 6 → +1/6.
        let eng = car.part(PartType::Engine).unwrap();
        let ele = car.part(PartType::Electronics).unwrap();
        assert!((eng.wear - (1.0 / 3.0) / part_effective_life(eng, true)).abs() < 1e-9);
        assert!((ele.wear - (1.0 / 6.0) / part_effective_life(ele, true)).abs() < 1e-9);
    }

    #[test]
    fn esticar_so_habilita_ate_95_por_cento() {
        let ok = CarPart { part_type: PartType::Engine, level: 5, wear: 0.90, spent: false, unit_seed: 0 };
        let alto = CarPart { part_type: PartType::Engine, level: 5, wear: 0.96, spent: false, unit_seed: 0 };
        let esgotada = CarPart { part_type: PartType::Engine, level: 5, wear: 0.5, spent: true, unit_seed: 0 };
        assert!(can_stretch(&ok));
        assert!(!can_stretch(&alto));
        assert!(!can_stretch(&esgotada));
    }

    #[test]
    fn esticar_custa_40_por_cento_de_uma_nova() {
        let part = CarPart { part_type: PartType::Engine, level: 5, wear: 0.9, spent: false, unit_seed: 0 };
        let full = replace_cost("gt3", &part);
        assert!((stretch_cost("gt3", &part) - 0.40 * full).abs() < 1e-6);
    }

    #[test]
    fn esticar_da_mais_uma_corrida_e_depois_a_peca_morre() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Engine, 0.90);
        advance_race(&mut car, &only(PartType::Engine, PartAction::Stretch));
        let engine = car.part(PartType::Engine).unwrap();
        assert!(engine.spent, "peça esticada deveria ficar spent");
        assert!(!can_stretch(engine), "peça spent não pode esticar de novo");
    }

    #[test]
    fn degradar_derruba_um_nivel_e_o_carro_sangra() {
        let mut car = Car::uniform(6);
        car.set_wear(PartType::Engine, 1.0); // passou de 100%
        let magnitude_antes = car.magnitude();
        advance_race(&mut car, &only(PartType::Engine, PartAction::Degrade));
        assert_eq!(car.part(PartType::Engine).unwrap().level, 5);
        assert!(car.magnitude() < magnitude_antes, "PHA deveria cair com a degradação");
    }

    #[test]
    fn degradar_abaixo_de_100_por_cento_nao_derruba_nivel() {
        let mut car = Car::uniform(6);
        car.set_wear(PartType::Engine, 0.5); // ainda dentro da vida
        advance_race(&mut car, &only(PartType::Engine, PartAction::Degrade));
        assert_eq!(car.part(PartType::Engine).unwrap().level, 6, "não deveria degradar dentro da vida");
    }

    #[test]
    fn esticar_e_depois_trocar_cai_um_nivel() {
        let mut car = Car::uniform(4);
        car.set_wear(PartType::Engine, 0.90);
        // Estica: roda a corrida extra ainda no nível 4.
        advance_race(&mut car, &only(PartType::Engine, PartAction::Stretch));
        assert!(car.part(PartType::Engine).unwrap().spent);
        assert_eq!(car.part(PartType::Engine).unwrap().level, 4);
        // Troca obrigatória da peça esticada → cai pra nível 3 (a punição).
        advance_race(&mut car, &only(PartType::Engine, PartAction::Replace));
        let engine = car.part(PartType::Engine).unwrap();
        assert_eq!(engine.level, 3, "peça esticada deve ser reposta um nível abaixo");
        assert!(!engine.spent);
    }

    #[test]
    fn troca_normal_mantem_o_nivel() {
        let mut car = Car::uniform(4);
        car.set_wear(PartType::Engine, 1.0); // fim de vida, mas NÃO esticada
        advance_race(&mut car, &only(PartType::Engine, PartAction::Replace));
        assert_eq!(car.part(PartType::Engine).unwrap().level, 4);
    }

    #[test]
    fn custo_de_repor_peca_esticada_e_de_nivel_abaixo() {
        let spent = CarPart { part_type: PartType::Engine, level: 4, wear: 1.1, spent: true, unit_seed: 0 };
        let fresh = CarPart { part_type: PartType::Engine, level: 4, wear: 1.1, spent: false, unit_seed: 0 };
        assert!(replace_cost("gt3", &spent) < replace_cost("gt3", &fresh));
        assert!((replace_cost("gt3", &spent) - cost::part_cost("gt3", PartType::Engine, 3)).abs() < 1e-6);
    }

    #[test]
    fn trocar_zera_o_desgaste_e_limpa_spent() {
        let mut car = Car::uniform(5);
        car.set_wear(PartType::Engine, 1.2);
        if let Some(p) = car.parts.iter_mut().find(|p| p.part_type == PartType::Engine) {
            p.spent = true;
        }
        advance_race(&mut car, &only(PartType::Engine, PartAction::Replace));
        let engine = car.part(PartType::Engine).unwrap();
        // trocou (wear 0, unidade NOVA → seed re-rolado) e então rodou 1 corrida → 1/3 dividido
        // pela vida EFETIVA da unidade nova (individual × tenda de nível).
        assert!((engine.wear - (1.0 / 3.0) / part_effective_life(engine, true)).abs() < 1e-9);
        assert!(!engine.spent);
    }
}
