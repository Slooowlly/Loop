//! Evento de venda / nova diretoria para equipes cronicamente falidas.
//!
//! Quando uma equipe passa duas temporadas consecutivas em colapso financeiro
//! (a segunda já em modo all-in), ela é "vendida": entra uma nova diretoria que
//! quita a dívida, injeta um caixa moderado e refunda o projeto esportivo.
//!
//! A IDENTIDADE é preservada — nome, cores, país, ano de fundação, histórico de
//! títulos e resultados continuam intactos. O que muda é a gestão: os atributos
//! de performance são re-sorteados numa faixa ampla ("aposta"), de modo que a
//! nova diretoria pode se revelar brilhante ou incompetente.

use rand::Rng;

use crate::finance::planning::category_finance_scale;
use crate::finance::state::refresh_team_financial_state;
use crate::models::team::Team;

/// Fração do caixa-médio da categoria injetada na venda (caixa moderado).
const SALE_CASH_FACTOR: f64 = 0.45;

/// O que a venda fez com as finanças da equipe — usado para registrar o evento
/// histórico exibido na ficha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamSaleOutcome {
    pub debt_cleared: f64,
    pub cash_injected: f64,
}

/// Renova uma equipe falida sob nova diretoria. Preserva identidade e histórico;
/// re-sorteia finanças e atributos de performance. Retorna o impacto financeiro
/// para registro histórico.
pub fn apply_team_sale(team: &mut Team, rng: &mut impl Rng) -> TeamSaleOutcome {
    let scale = category_finance_scale(&team.categoria);

    let debt_cleared = team.debt_balance.max(0.0);
    let cash_injected = scale.expected_cash_midpoint() * SALE_CASH_FACTOR;

    // ── Finanças: dívida zerada, caixa moderado ──
    team.debt_balance = 0.0;
    team.cash_balance = cash_injected;
    team.parachute_payment_remaining = 0.0;
    team.last_round_income = 0.0;
    team.last_round_expenses = 0.0;
    team.last_round_net = 0.0;
    team.season_strategy = "balanced".to_string();

    // ── Aposta ampla: nova diretoria pode ser ótima ou medíocre ──
    // Atributos 0-100 sorteados numa faixa larga.
    team.confiabilidade = rng.gen_range(20.0..=90.0);
    team.engineering = rng.gen_range(20.0..=90.0);
    team.facilities = rng.gen_range(20.0..=90.0);
    team.aerodinamica = rng.gen_range(20.0..=90.0);
    team.motor = rng.gen_range(20.0..=90.0);
    team.chassi = rng.gen_range(20.0..=90.0);
    team.reputacao = rng.gen_range(20.0..=90.0);
    team.pit_crew_quality = rng.gen_range(20.0..=90.0);
    // car_performance vive em [-5, 16]; faixa ampla porém plausível.
    team.car_performance = rng.gen_range(-2.0..=12.0);
    // Moral renovada pelo otimismo da nova gestão.
    team.morale = rng.gen_range(0.9..=1.3);

    // Recalcula o estado financeiro: com dívida zero e caixa moderado, deve sair
    // do colapso.
    refresh_team_financial_state(team);

    TeamSaleOutcome {
        debt_cleared,
        cash_injected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;
    use rand::{rngs::StdRng, SeedableRng};

    fn collapsed_team() -> Team {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Escuderia Histórica".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = -100_000.0;
        team.debt_balance = 40_000_000.0;
        team.financial_state = "collapse".to_string();
        team.car_performance = -4.0;
        team
    }

    #[test]
    fn sale_clears_debt_and_injects_cash() {
        let mut team = collapsed_team();
        let mut rng = StdRng::seed_from_u64(1);
        apply_team_sale(&mut team, &mut rng);

        assert_eq!(team.debt_balance, 0.0, "dívida deve ser quitada");
        assert!(team.cash_balance > 0.0, "caixa deve ser injetado");
        assert_ne!(
            team.financial_state, "collapse",
            "deve sair do colapso após a venda"
        );
    }

    #[test]
    fn sale_preserves_identity_and_history() {
        let mut team = collapsed_team();
        team.historico_vitorias = 42;
        team.historico_titulos_construtores = 3;
        let nome = team.nome.clone();
        let cor = team.cor_primaria.clone();
        let fundacao = team.ano_fundacao;

        let mut rng = StdRng::seed_from_u64(2);
        apply_team_sale(&mut team, &mut rng);

        assert_eq!(team.nome, nome, "nome preservado");
        assert_eq!(team.cor_primaria, cor, "cor preservada");
        assert_eq!(team.ano_fundacao, fundacao, "ano de fundação preservado");
        assert_eq!(team.historico_vitorias, 42, "histórico preservado");
        assert_eq!(
            team.historico_titulos_construtores, 3,
            "títulos preservados"
        );
    }

    #[test]
    fn sale_attributes_stay_in_valid_ranges() {
        let mut team = collapsed_team();
        let mut rng = StdRng::seed_from_u64(3);
        apply_team_sale(&mut team, &mut rng);

        assert!((-5.0..=16.0).contains(&team.car_performance));
        assert!((0.0..=100.0).contains(&team.engineering));
        assert!((0.0..=100.0).contains(&team.reputacao));
    }
}
