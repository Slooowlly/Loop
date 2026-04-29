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
        car_performance_delta: rng.gen_range(5.0..=10.0),
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

pub fn apply_attribute_deltas(
    conn: &Connection,
    team_id: &str,
    delta: &TeamAttributeDelta,
) -> Result<(), String> {
    let mut team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao buscar equipe '{team_id}': {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' nao encontrada"))?;

    team.car_performance = (team.car_performance + delta.car_performance_delta).clamp(-5.0, 16.0);
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
        assert!(delta.car_performance_delta > 0.0);
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

        assert_eq!(updated.car_performance, 16.0);
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
}
