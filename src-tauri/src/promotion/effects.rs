use rand::Rng;
use rusqlite::Connection;

use crate::db::queries::teams as team_queries;
use crate::finance::events::parachute_payment_for_relegation;
use crate::finance::planning::category_finance_scale;
use crate::models::team::Team;
use crate::promotion::{MovementType, TeamAttributeDelta};

pub fn calculate_promotion_effects(team: &Team, rng: &mut impl Rng) -> TeamAttributeDelta {
    TeamAttributeDelta {
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        movement_type: MovementType::Promocao,
        // Anti-snowball: a promoção NÃO dá mais carro de graça. O ganho fixo
        // (+5..10, independente de caixa) permitia que uma equipe rookie — carro
        // spec, que nunca investiu no carro — chegasse na categoria de cima já com
        // um carro forte, pulando o gating econômico e subindo a escada sem parar.
        // Agora a equipe MANTÉM o número do carro dela (sem reset): como a banda de
        // carro da categoria nova é mais alta, ela entra por baixo e tem que ganhar
        // evolução via a economia da própria categoria (offseason em cashflow.rs).
        // Prêmio da promoção segue nos demais atributos (orçamento, estrutura, etc.).
        car_performance_delta: 0.0,
        budget_delta: rng.gen_range(5.0..=15.0),
        facilities_delta: rng.gen_range(0.0..=5.0),
        engineering_delta: rng.gen_range(0.0..=3.0),
        morale_multiplier: 1.15,
        reputacao_delta: rng.gen_range(3.0..=8.0),
    }
}

pub fn calculate_relegation_effects(team: &Team, rng: &mut impl Rng) -> TeamAttributeDelta {
    TeamAttributeDelta {
        team_id: team.id.clone(),
        team_name: team.nome.clone(),
        movement_type: MovementType::Rebaixamento,
        car_performance_delta: rng.gen_range(-16.0..=-10.0),
        budget_delta: rng.gen_range(-30.0..=-20.0),
        facilities_delta: rng.gen_range(-15.0..=-8.0),
        engineering_delta: rng.gen_range(-15.0..=-8.0),
        morale_multiplier: 0.60,
        reputacao_delta: rng.gen_range(-25.0..=-15.0),
    }
}

/// Soft-landing da promoção (Ideia 1), LIGADO por padrão (desligável via
/// `IRACER_PROMO_SOFT_LANDING=0` para A/B no Monte Carlo).
///
/// Em vez de manter o carro EXATO da categoria de baixo (comportamento default,
/// `car_performance_delta: 0.0`), que deixava o campeão promovido isolado em
/// último — 30s atrás de um campo com carros muito mais fortes —, posiciona o
/// carro do promovido logo ACIMA do pior incumbente da categoria de destino.
///
/// Intenção de design: o campeão da categoria inferior deve, teoricamente, ser
/// "um pouco melhor que a pior da categoria superior": entra com chance real de
/// brigar pela PERMANÊNCIA, mas longe de brigar por título. Quem passa a ser o
/// mais ameaçado de rebaixamento é o pior incumbente — o fundo da categoria gira.
///
/// - `current_car` = car_performance atual do promovido (carro da categoria de baixo).
/// - `field_cars`  = car_performance dos incumbentes que PERMANECEM na categoria
///   de destino (exclui o próprio promovido e a rebaixada, que já saiu na troca).
/// - `margin`      = quão acima do pior incumbente o promovido aterrissa (pontos
///   de carro; calibrável no Monte Carlo).
///
/// Retorna o delta a aplicar sobre `current_car` — pode ser NEGATIVO: se o carro
/// do promovido já for alto (amador dominante), clampa PRA BAIXO, porque a
/// tecnologia da categoria inferior não traduz pra superior. `None` se o campo
/// estiver vazio (nada a posicionar).
pub fn promotion_car_landing_delta(
    current_car: f64,
    field_cars: &[f64],
    margin: f64,
) -> Option<f64> {
    if field_cars.is_empty() {
        return None;
    }
    let worst = field_cars.iter().copied().fold(f64::INFINITY, f64::min);
    let median = median_of(field_cars);
    // Alvo = logo acima do pior incumbente. Teto de segurança na mediana do campo
    // (anti-título: nunca aterrissa no meio do pelotão, mesmo com margem grande ou
    // campo degenerado); piso no pior (nunca isolado abaixo do lanterna atual).
    let target = (worst + margin).clamp(worst, median.max(worst));
    Some(target - current_car)
}

fn median_of(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

pub fn apply_attribute_deltas(
    conn: &Connection,
    team_id: &str,
    delta: &TeamAttributeDelta,
) -> Result<(), String> {
    let mut team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao buscar equipe '{team_id}': {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' nao encontrada"))?;

    // Sem teto superior (Pilar B): só piso em −5.
    team.car_performance = (team.car_performance + delta.car_performance_delta).max(-5.0);
    team.cash_balance += promotion_budget_delta_to_cash(&team, delta.budget_delta);
    team.facilities = (team.facilities + delta.facilities_delta).clamp(0.0, 100.0);
    team.engineering = (team.engineering + delta.engineering_delta).clamp(0.0, 100.0);
    team.morale = (team.morale * delta.morale_multiplier).clamp(0.5, 1.5);
    team.reputacao = (team.reputacao + delta.reputacao_delta).clamp(0.0, 100.0);
    if delta.movement_type == MovementType::Rebaixamento {
        team.parachute_payment_remaining += parachute_payment_for_relegation(&team);
    }

    team_queries::update_team(conn, &team)
        .map_err(|e| format!("Falha ao atualizar equipe '{}': {e}", team.nome))?;
    Ok(())
}

fn promotion_budget_delta_to_cash(team: &Team, budget_delta: f64) -> f64 {
    let scale = category_finance_scale(&team.categoria);
    let category_cash_window = (scale.cash_max - scale.cash_min).max(1.0);

    category_cash_window * (budget_delta / 100.0) * 0.35
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};
    use rusqlite::Connection;

    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::db::migrations;
    use crate::db::queries::teams as team_queries;
    use crate::models::team::Team;
    use crate::promotion::MovementType;

    #[test]
    fn test_promotion_effects_positive() {
        let mut rng = StdRng::seed_from_u64(40);
        let team = sample_team("gt4", "T001");

        let delta = calculate_promotion_effects(&team, &mut rng);

        assert_eq!(delta.movement_type, MovementType::Promocao);
        // Anti-snowball: promoção não dá mais carro de graça (mantém o número atual).
        assert_eq!(delta.car_performance_delta, 0.0);
        assert!(delta.budget_delta > 0.0);
        assert!(delta.facilities_delta >= 0.0);
        assert!(delta.engineering_delta >= 0.0);
        assert!(delta.morale_multiplier > 1.0);
        assert!(delta.reputacao_delta > 0.0);
    }

    #[test]
    fn test_relegation_effects_negative() {
        let mut rng = StdRng::seed_from_u64(41);
        let team = sample_team("gt4", "T001");

        let delta = calculate_relegation_effects(&team, &mut rng);

        assert_eq!(delta.movement_type, MovementType::Rebaixamento);
        assert!(delta.car_performance_delta < 0.0);
        assert!(delta.budget_delta < 0.0);
        assert!(delta.facilities_delta <= 0.0);
        assert!(delta.engineering_delta < 0.0);
        assert!(delta.morale_multiplier < 1.0);
        assert!(delta.reputacao_delta < 0.0);
    }

    #[test]
    fn test_relegation_effects_are_heavy_enough_to_limit_immediate_bounce_back() {
        let mut rng = StdRng::seed_from_u64(42);
        let team = sample_team("mazda_rookie", "T001");

        let delta = calculate_relegation_effects(&team, &mut rng);

        assert!(delta.car_performance_delta <= -10.0);
        assert!(delta.budget_delta <= -20.0);
        assert!(delta.facilities_delta <= -8.0);
        assert!(delta.engineering_delta <= -8.0);
        assert!(delta.morale_multiplier <= 0.65);
        assert!(delta.reputacao_delta <= -15.0);
    }

    #[test]
    fn test_effects_clamped() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let mut team = sample_team("gt4", "T001");
        team.car_performance = 15.5;
        team.budget = 99.0;
        team.facilities = 99.0;
        team.engineering = 99.0;
        team.morale = 1.45;
        team.reputacao = 99.0;
        let before_cash = team.cash_balance;
        team_queries::insert_team(&conn, &team).expect("insert team");

        let delta = TeamAttributeDelta {
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            movement_type: MovementType::Promocao,
            car_performance_delta: 5.0,
            budget_delta: 10.0,
            facilities_delta: 10.0,
            engineering_delta: 10.0,
            morale_multiplier: 1.15,
            reputacao_delta: 10.0,
        };

        apply_attribute_deltas(&conn, &team.id, &delta).expect("apply deltas");
        let updated = team_queries::get_team_by_id(&conn, &team.id)
            .expect("team query")
            .expect("team exists");

        // Pilar B: car_performance não tem mais teto — o delta aplica integral
        // (15.5 + 5.0). Os demais atributos seguem clampados (100 / 1.5).
        assert_eq!(updated.car_performance, 20.5);
        assert!(updated.cash_balance > before_cash);
        let expected_budget = crate::finance::planning::derive_budget_index_from_money(&updated);
        assert!((updated.budget - expected_budget).abs() < 0.0001);
        assert_eq!(updated.facilities, 100.0);
        assert_eq!(updated.engineering, 100.0);
        assert_eq!(updated.morale, 1.5);
        assert_eq!(updated.reputacao, 100.0);
    }

    #[test]
    fn test_relegation_delta_initializes_parachute_payment() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        migrations::run_all(&conn).expect("schema");
        let team = sample_team("gt4", "T001");
        team_queries::insert_team(&conn, &team).expect("insert team");

        let delta = TeamAttributeDelta {
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            movement_type: MovementType::Rebaixamento,
            car_performance_delta: -4.0,
            budget_delta: -8.0,
            facilities_delta: -1.0,
            engineering_delta: -2.0,
            morale_multiplier: 0.75,
            reputacao_delta: -6.0,
        };

        apply_attribute_deltas(&conn, &team.id, &delta).expect("apply deltas");
        let updated = team_queries::get_team_by_id(&conn, &team.id)
            .expect("team query")
            .expect("team exists");

        assert!(updated.parachute_payment_remaining > 0.0);
    }

    fn sample_team(category: &str, id: &str) -> Team {
        let template = get_team_templates(category)[0];
        let mut rng = StdRng::seed_from_u64(404);
        Team::from_template_with_rng(template, category, id.to_string(), 2025, &mut rng)
    }

    #[test]
    fn test_landing_raises_a_car_that_is_below_the_field_floor() {
        // Campeão de amador (carro 5) sobe para um campo GT3 forte (10..16).
        // Deve aterrissar logo acima do pior (10) — não isolado 5 pontos abaixo.
        let field = [10.0, 12.0, 14.0, 16.0];
        let delta = promotion_car_landing_delta(5.0, &field, 1.0).expect("delta");
        assert!((5.0 + delta - 11.0).abs() < 1e-9, "alvo = pior(10) + margem(1)");
        assert!(delta > 0.0, "carro fraco deve SUBIR até o fundo do campo");
    }

    #[test]
    fn test_landing_clamps_down_a_car_stronger_than_the_field_floor() {
        // Amador dominante (carro 14) sobe para um campo cujo pior é 10.
        // Tecnologia da categoria de baixo não traduz: clampa PRA BAIXO até ~11.
        let field = [10.0, 12.0, 13.0, 16.0];
        let delta = promotion_car_landing_delta(14.0, &field, 1.0).expect("delta");
        assert!(delta < 0.0, "carro forte demais deve DESCER para perto do fundo");
        assert!((14.0 + delta - 11.0).abs() < 1e-9);
    }

    #[test]
    fn test_landing_never_lands_above_field_median() {
        // Margem absurda não pode jogar o promovido pro meio do pelotão (anti-título).
        let field = [10.0, 12.0, 14.0, 16.0]; // mediana = 13
        let delta = promotion_car_landing_delta(5.0, &field, 99.0).expect("delta");
        assert!((5.0 + delta - 13.0).abs() < 1e-9, "teto na mediana do campo");
    }

    #[test]
    fn test_landing_none_when_field_empty() {
        assert!(promotion_car_landing_delta(5.0, &[], 1.0).is_none());
    }

    #[test]
    fn test_median_of_even_and_odd() {
        assert!((median_of(&[1.0, 3.0, 2.0]) - 2.0).abs() < 1e-9);
        assert!((median_of(&[1.0, 2.0, 3.0, 4.0]) - 2.5).abs() < 1e-9);
    }
}
