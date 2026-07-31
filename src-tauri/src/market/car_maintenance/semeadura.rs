//! Seed inicial dos carros: qualidade relativa de cada time dentro da categoria
//! e a persistência do carro na criação da carreira.

use super::*;

// ===================== Seed inicial dos carros =====================

/// Qualidade relativa (0..1) de cada time DENTRO da sua CLASSE, medida pelo
/// `car_performance` (o escalar legado que já reflete orçamento/prestígio no seed).
///
/// O agrupamento é por `(categoria, classe)`, não por categoria: no Endurance as três
/// classes (GT4, GT3, LMP2) dividem a mesma categoria com escalas de carro diferentes, e
/// medir todas contra um min/max único fazia a melhor equipe de GT4 parecer medíocre
/// porque o topo da escala era o LMP2.
pub(super) fn category_quality(teams: &[Team]) -> HashMap<String, f64> {
    type ClassKey<'a> = (&'a str, Option<&'a str>);

    fn class_key(team: &Team) -> ClassKey<'_> {
        (
            team.categoria.as_str(),
            team.classe.as_deref().filter(|classe| !classe.is_empty()),
        )
    }

    let mut bounds: HashMap<ClassKey<'_>, (f64, f64)> = HashMap::new();
    for team in teams {
        let entry = bounds
            .entry(class_key(team))
            .or_insert((f64::INFINITY, f64::NEG_INFINITY));
        entry.0 = entry.0.min(team.car_performance);
        entry.1 = entry.1.max(team.car_performance);
    }

    let mut quality = HashMap::new();
    for team in teams {
        let (min, max) = bounds.get(&class_key(team)).copied().unwrap_or((0.0, 0.0));
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
/// classe; rookie = spec). Chamado uma vez na criação da carreira — clássica ou draft
/// histórico —, logo após inserir os times.
pub fn seed_and_persist_team_cars(conn: &Connection, teams: &[Team]) -> Result<(), DbError> {
    let quality = category_quality(teams);
    for team in teams {
        let q = quality.get(&team.id).copied().unwrap_or(0.5);
        // Chave com classe: o teto do carro é o da classe (`"endurance:gt3"`).
        let car = seed_car(&team.car_category_key(), q);
        team_car::upsert_team_car(conn, &team.id, &car)?;
    }
    Ok(())
}
