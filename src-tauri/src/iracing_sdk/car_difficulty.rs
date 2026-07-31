//! Inversão **carro → dificuldade da IA** no export (Sistema de Nível do Carro).
//!
//! No sim da carreira o carro te deixa mais rápido; no iRacing os carros de conteúdo grátis
//! são SPEC (idênticos), então não dá pra te acelerar. A única forma do carro existir na
//! pista é pelo avesso: **carro melhor → IA mais fraca**. Cada IA é ajustada RELATIVO ao
//! SEU carro, na pista alvo:
//!
//! ```text
//! nudge(IA) = SKILL_PER_CARPERF · ( adv(carro_IA) − adv(seu_carro) )
//! ```
//!
//! com `adv() =` [`car_advantage`] (funde nível + shape casado à pista; ver `car::sim_bridge`).
//!
//! ## Por que decompomos em BANDA + SPREAD
//! O iRacing ESTICA o roster pra preencher `[minSkill, maxSkill]`, então um shift UNIFORME no
//! roster é apagado pelo esticão. Decompondo `nudge(IA)` em duas parcelas:
//!   - **uniforme** — você vs a MÉDIA do campo → vai na BANDA (move `[minSkill,maxSkill]` e a
//!     âncora da curva; sobrevive ao esticão de forma EXATA). É o efeito dominante.
//!   - **spread por-IA** — zero-mean: quanto CADA IA desvia da média → cavalga o roster como
//!     as nudges de comportamento (o esticão reconcilia).
//!
//! Somadas, reconstroem `adv(IA) − adv(você)` por IA:
//! `band + spread_i = k·(média − você) + k·(adv_i − média) = k·(adv_i − você)`. ✓
//!
//! ## Teto
//! `SKILL_PER_CARPERF = 20 / CAR_PERF_MAX` → o eixo do NÍVEL (1↔10 = 16 car-perf) mapeia em
//! exatos **20** de skill. O foco (shape) soma alguns pontos por cima só em pista peaked.
//!
//! ## Mecanismo 2 — adaptativo cego ao carro
//! O adaptativo mede o RITMO real do jogador (carro spec) vs a frente. Como enfraquecemos a
//! frente de propósito (carro do jogador), ele veria "jogador dominando" e subiria a IA,
//! comendo a vantagem. [`car_pace_credit`] devolve quanto do `pace_vs_front` é EXPLICADO pelo
//! carro — some ao ritmo observado antes de decidir, e sobra o resíduo = as MÃOS do jogador.

use crate::car::sim_bridge::{car_advantage, CAR_PERF_MAX};
use crate::car::Car;
use crate::simulation::car_build::CarAttributeWeights;

/// Skill de IA por unidade de `car_performance`. `20 / 16 = 1,25`: o gap de nível 1↔10
/// (16 car-perf) vira exatos 20 de skill.
pub const SKILL_PER_CARPERF: f64 = 20.0 / CAR_PERF_MAX;

/// Quanto 1 ponto de skill vale em FRAÇÃO de tempo de volta (mecanismo 2). PLACEHOLDER
/// calibrável — ancorar nos offsets de pista medidos em corrida real (o único número novo do
/// sistema que precisa de calibração de verdade). 0,12%/ponto é um chute de partida.
pub const LAP_FRACTION_PER_SKILL: f64 = 0.0012;

/// Vantagem de carro de cada carro na pista (car-perf), na ordem recebida.
pub fn advantages(cars: &[Car], track: CarAttributeWeights) -> Vec<f64> {
    cars.iter().map(|c| car_advantage(c, track)).collect()
}

/// Média das vantagens (0 se vazio).
fn mean(advs: &[f64]) -> f64 {
    if advs.is_empty() {
        0.0
    } else {
        advs.iter().sum::<f64>() / advs.len() as f64
    }
}

/// **Delta da BANDA** (skill): você vs a MÉDIA do campo. `>0` sobe a IA (seu carro é PIOR que
/// a média → mais difícil); `<0` baixa (seu carro é MELHOR → mais fácil). Aplicado ao sweet
/// spot / `max_skill` (e ao piso junto), move a banda inteira — exato sob o esticão do iRacing.
pub fn band_skill_delta(player_adv: f64, ai_advs: &[f64]) -> f64 {
    if ai_advs.is_empty() {
        return 0.0;
    }
    SKILL_PER_CARPERF * (mean(ai_advs) - player_adv)
}

/// **Spread por-IA** (skill, zero-mean): quanto `ai_adv` desvia da média do campo. Some ao
/// skill da IA no roster — carro acima da média sobe, abaixo desce. Junto do delta da banda
/// (que carrega o "você vs campo"), reconstrói `adv(IA) − adv(você)` por IA.
pub fn ai_spread_nudge(ai_adv: f64, mean_ai_adv: f64) -> f64 {
    SKILL_PER_CARPERF * (ai_adv - mean_ai_adv)
}

/// Média das vantagens da IA (para o `ai_spread_nudge` no roster). Público p/ o chamador
/// computar uma vez e reusar por IA.
pub fn field_mean(ai_advs: &[f64]) -> f64 {
    mean(ai_advs)
}

/// **Crédito de ritmo do carro** (mecanismo 2): a fração de volta que o `pace_vs_front` do
/// jogador GANHOU por termos enfraquecido a frente (as 2 IAs mais rápidas). SOME ao
/// `pace_vs_front` observado antes de o adaptativo decidir — sobra o resíduo = as MÃOS.
///
/// `>0` quando seu carro é MELHOR que a frente (você "deveria" estar à frente → desconta a
/// vantagem, o adaptativo não credita às suas mãos). `0` sem carros da frente.
pub fn car_pace_credit(player_adv: f64, front_advs: &[f64]) -> f64 {
    if front_advs.is_empty() {
        return 0.0;
    }
    let skill_gap = SKILL_PER_CARPERF * (player_adv - mean(front_advs));
    skill_gap * LAP_FRACTION_PER_SKILL
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::car::{Car, PartType};

    // Pista ~equilibrada (shape irrelevante) e pista de potência (Monza-like).
    const BALANCED: CarAttributeWeights = (34.0, 33.0, 33.0);
    const MONZA: CarAttributeWeights = (10.0, 70.0, 20.0);

    /// Carro focado em potência (motor/câmbio/cooling/eletrônica no talo), mesmo nível-base.
    fn power_car(level: u8) -> Car {
        let mut c = Car::uniform(level);
        for p in [
            PartType::Engine,
            PartType::Gearbox,
            PartType::Cooling,
            PartType::Electronics,
        ] {
            c.set_level(p, 10);
        }
        c
    }

    #[test]
    fn cenario_a_carro_no_nivel_do_campo_quase_nao_mexe() {
        // Jogador L6; campo L5/L6/L7. Média = L6 → banda ~0; spreads pequenos e simétricos.
        let player = car_advantage(&Car::uniform(6), BALANCED);
        let field = advantages(
            &[Car::uniform(5), Car::uniform(6), Car::uniform(7)],
            BALANCED,
        );
        let band = band_skill_delta(player, &field);
        assert!(band.abs() < 0.01, "banda deveria ~0, veio {band}");
        let m = field_mean(&field);
        assert!(ai_spread_nudge(field[0], m) < -1.5); // L5 desce ~2,2
        assert!(ai_spread_nudge(field[2], m) > 1.5); //  L7 sobe ~2,2
    }

    #[test]
    fn cenario_b_carrao_baixa_a_banda() {
        // Jogador L9 num campo L6 → banda NEGATIVA (~-6,7): IA mais fraca, corrida mais fácil.
        let player = car_advantage(&Car::uniform(9), BALANCED);
        let field = advantages(
            &[Car::uniform(6), Car::uniform(6), Car::uniform(6)],
            BALANCED,
        );
        let band = band_skill_delta(player, &field);
        assert!((band - (-6.67)).abs() < 0.2, "esperava ~-6,7, veio {band}");
    }

    #[test]
    fn cenario_c_carro_ruim_sobe_a_banda() {
        // Jogador L3 num campo L7 → banda POSITIVA (~+8,9): a frente endurece e abre.
        let player = car_advantage(&Car::uniform(3), BALANCED);
        let field = advantages(&[Car::uniform(7), Car::uniform(7)], BALANCED);
        let band = band_skill_delta(player, &field);
        assert!((band - 8.89).abs() < 0.2, "esperava ~+8,9, veio {band}");
    }

    #[test]
    fn cenario_d_foco_vira_o_jogo_em_pista_peaked() {
        // Duas IAs mesmo nível-base; uma focada em potência. Em Monza a focada tem vantagem
        // MAIOR (spread positivo) e em pista equilibrada a diferença some.
        let balanced_ai = car_advantage(&Car::uniform(7), MONZA);
        let power_ai = car_advantage(&power_car(7), MONZA);
        assert!(
            power_ai > balanced_ai,
            "em Monza o carro de potência deveria valer mais"
        );
        // Mesmos carros numa pista equilibrada: o shape quase não separa.
        let bal_flat = car_advantage(&Car::uniform(7), BALANCED);
        let pow_flat = car_advantage(&power_car(7), BALANCED);
        assert!(
            (pow_flat - bal_flat).abs() < (power_ai - balanced_ai),
            "o shape deveria pesar MAIS em Monza do que na pista equilibrada"
        );
    }

    #[test]
    fn cenario_e_credito_desconta_o_carro_do_ritmo() {
        // Carro do jogador MELHOR que a frente → crédito POSITIVO (desconta a vantagem).
        let player = car_advantage(&Car::uniform(9), BALANCED);
        let front = advantages(&[Car::uniform(6), Car::uniform(6)], BALANCED);
        let credit = car_pace_credit(player, &front);
        assert!(
            credit > 0.0,
            "carro melhor que a frente → crédito > 0, veio {credit}"
        );
        // Carro PIOR que a frente → crédito negativo (você está atrás por carro, não mãos).
        let weak = car_advantage(&Car::uniform(3), BALANCED);
        assert!(car_pace_credit(weak, &front) < 0.0);
        // Sem frente → 0.
        assert_eq!(car_pace_credit(player, &[]), 0.0);
    }

    #[test]
    fn rookie_spec_zera_tudo() {
        // Todos spec (nível 1) → carros IDÊNTICOS → banda 0 e spread 0 (a vantagem absoluta
        // cancela: jogador e IA têm o mesmo carro). Rookie corre na pilotagem pura. A
        // magnitude do nível 1 é 0; sobra só um resíduo de shape que se anula por ser igual
        // em todos.
        let player = car_advantage(&Car::spec_rookie(), BALANCED);
        let field = advantages(&[Car::spec_rookie(), Car::spec_rookie()], BALANCED);
        assert!(
            (player - field[0]).abs() < 1e-9,
            "carros idênticos → mesma vantagem"
        );
        assert_eq!(band_skill_delta(player, &field), 0.0);
        assert_eq!(ai_spread_nudge(field[0], field_mean(&field)), 0.0);
    }

    #[test]
    fn teto_de_20_no_eixo_do_nivel() {
        // Nível 1 vs nível 10 em pista equilibrada (shape ~0) → banda ~20 (o teto do nível).
        let player = car_advantage(&Car::uniform(1), BALANCED);
        let field = advantages(&[Car::uniform(10)], BALANCED);
        let band = band_skill_delta(player, &field);
        assert!((band - 20.0).abs() < 0.3, "esperava ~20, veio {band}");
    }
}
