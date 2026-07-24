//! Seed inicial dos carros: qualidade relativa de cada time dentro da categoria
//! e a persistência do carro na criação da carreira.

use super::*;

// ===================== Seed inicial dos carros =====================

/// Qualidade relativa (0..1) de cada time DENTRO da sua categoria, medida pelo
/// `car_performance` (o escalar legado que já reflete orçamento/prestígio no seed).
pub(super) fn category_quality(teams: &[Team]) -> HashMap<String, f64> {
    // Min/max de car_performance por categoria.
    let mut bounds: HashMap<&str, (f64, f64)> = HashMap::new();
    for team in teams {
        let entry = bounds
            .entry(team.categoria.as_str())
            .or_insert((f64::INFINITY, f64::NEG_INFINITY));
        entry.0 = entry.0.min(team.car_performance);
        entry.1 = entry.1.max(team.car_performance);
    }

    let mut quality = HashMap::new();
    for team in teams {
        let (min, max) = bounds
            .get(team.categoria.as_str())
            .copied()
            .unwrap_or((0.0, 0.0));
        let spread = max - min;
        let q = if spread.abs() < f64::EPSILON {
            0.5
        } else {
            ((team.car_performance - min) / spread).clamp(0.0, 1.0)
        };
        quality.insert(team.id.clone(), q);
    }
    quality
}

/// Semeia e persiste o carro inicial de cada time (correlacionado com a qualidade na
/// categoria; rookie = spec). Chamado uma vez na criação da carreira, logo após inserir
/// os times.
pub fn seed_and_persist_team_cars(conn: &Connection, teams: &[Team]) -> Result<(), DbError> {
    let quality = category_quality(teams);
    for team in teams {
        let q = quality.get(&team.id).copied().unwrap_or(0.5);
        let car = seed_car(&team.categoria, q);
        team_car::upsert_team_car(conn, &team.id, &car)?;
    }
    Ok(())
}
