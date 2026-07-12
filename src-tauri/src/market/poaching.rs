//! Poaching / quebra de contrato na janela de mercado (Fase 2b do estrelato).
//! **Lógica PURA:** a multa de rescisão e o "vale a pena arrancar?" do assediante.
//! O dinheiro time→time e o passe na janela ficam no `pipeline` (tocam o banco).
//!
//! Regra travada com o user: a quebra acontece na JANELA (off-season), não no meio
//! da temporada — um piloto SOB contrato pode ser arrancado por um time disposto a
//! pagar a multa. Gatilho = FAMA + MÉRITO.

use crate::fame::fame_commercial_units;

/// Multiplicador de pedigree da multa: mistura MÉRITO (skill) e FAMA. Um astro custa
/// muito mais pra arrancar que um medíocre anônimo. Faixa ~[0.6 .. 1.8].
pub fn poach_pedigree(skill: f64, fama: f64) -> f64 {
    let skill_norm = ((skill.clamp(0.0, 100.0) - 40.0) / 60.0).clamp(0.0, 1.0);
    let fame_norm = fama.clamp(0.0, 100.0) / 100.0;
    0.6 + 0.7 * skill_norm + 0.5 * fame_norm
}

/// Multa de rescisão pra arrancar um piloto SOB contrato: `salário × anos_restantes
/// × pedigree`. `years_remaining` clampa em ≥1 (só se poacha quem tem ao menos 1 ano
/// pela frente; quem expira vira agente livre naturalmente). Nunca negativa.
pub fn buyout_fee(salary_anual: f64, years_remaining: i32, skill: f64, fama: f64) -> f64 {
    let years = years_remaining.max(1) as f64;
    (salary_anual.max(0.0) * years * poach_pedigree(skill, fama)).round()
}

/// Valor de um piloto para um assediante: mérito (skill) + apelo comercial da fama
/// ponderado pela necessidade do time (o MESMO termo da escada). Base pra decidir se
/// o alvo é upgrade que justifica a multa.
pub fn poach_target_value(skill: f64, fama: f64, need_factor: f64) -> f64 {
    skill + fame_commercial_units(fama) * need_factor
}

/// Margem mínima de UPGRADE (em pontos de valor) pra o assediante topar a dor de
/// cabeça de arrancar um contratado — evita troca-troca sem ganho real.
pub const POACH_UPGRADE_MARGIN: f64 = 8.0;

/// Fração MÁXIMA do caixa que um time topa queimar numa multa (não esvazia o cofre).
pub const POACH_CASH_FRACTION: f64 = 0.6;

/// O alvo é um upgrade claro sobre o piloto que ele substituiria?
pub fn is_clear_upgrade(target_value: f64, incumbent_value: f64) -> bool {
    target_value >= incumbent_value + POACH_UPGRADE_MARGIN
}

/// O time consegue pagar a multa sem torrar o caixa?
pub fn can_afford_buyout(team_cash: f64, buyout: f64) -> bool {
    buyout > 0.0 && buyout <= team_cash * POACH_CASH_FRACTION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn craque_famoso_custa_muito_mais_que_mediocre_anonimo() {
        let astro = buyout_fee(300_000.0, 2, 92.0, 90.0);
        let mediocre = buyout_fee(300_000.0, 2, 60.0, 15.0);
        assert!(astro > mediocre * 1.4, "astro={astro}, mediocre={mediocre}");
        // Ordem de grandeza esperada do exemplo do design (~$720k pro astro de 2 anos).
        assert!((600_000.0..=1_200_000.0).contains(&astro), "astro={astro}");
    }

    #[test]
    fn multa_escala_com_anos_restantes() {
        let um = buyout_fee(200_000.0, 1, 80.0, 60.0);
        let tres = buyout_fee(200_000.0, 3, 80.0, 60.0);
        // ~3× (folga de arredondamento — cada chamada faz `.round()`).
        assert!((tres - um * 3.0).abs() <= 3.0, "um={um}, tres={tres}");
    }

    #[test]
    fn anos_zero_ou_negativo_conta_como_um() {
        // Quem "expira" ainda tem multa de 1 ano se for arrancado antes de virar livre.
        assert_eq!(
            buyout_fee(100_000.0, 0, 70.0, 50.0),
            buyout_fee(100_000.0, 1, 70.0, 50.0)
        );
    }

    #[test]
    fn pedigree_limitado_e_monotonico() {
        assert!(poach_pedigree(40.0, 0.0) >= 0.6 - 1e-9);
        assert!(poach_pedigree(100.0, 100.0) <= 1.8 + 1e-9);
        assert!(poach_pedigree(92.0, 90.0) > poach_pedigree(60.0, 20.0));
    }

    #[test]
    fn upgrade_exige_margem() {
        // Igual não é upgrade; precisa passar da margem.
        assert!(!is_clear_upgrade(80.0, 80.0));
        assert!(!is_clear_upgrade(85.0, 80.0)); // +5 < margem 8
        assert!(is_clear_upgrade(90.0, 80.0)); // +10 ≥ margem
    }

    #[test]
    fn so_paga_multa_que_cabe_no_caixa() {
        // Caixa $1mi, fração 0.6 → topa até $600k.
        assert!(can_afford_buyout(1_000_000.0, 500_000.0));
        assert!(!can_afford_buyout(1_000_000.0, 700_000.0));
        // Multa nula/negativa nunca "cabe" (não há o que pagar).
        assert!(!can_afford_buyout(1_000_000.0, 0.0));
    }
}
