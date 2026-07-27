//! O CAMPO DE MERITO da corrida: a unica construcao do `Vec<DriverMerit>` que alimenta o `race_eval`.

use super::*;
use crate::race_eval::{compute_merit, DriverMerit};
use std::collections::HashMap;

/// Valor neutro (meia-escala 0–100) quando o piloto ou a equipe não resolvem no
/// banco. Nunca REMOVEMOS a linha do campo: `race_eval` usa `field.len()` como
/// tamanho do grid, e encolher o campo desloca a posição-potencial de todo mundo.
const MERIT_FALLBACK: f64 = 50.0;

/// Monta o campo de mérito (skill + carro + forma recente) de TODOS os carros da
/// corrida — a base da posição ESPERADA em [`crate::race_eval::evaluate`].
///
/// Fonte única de propósito: o debrief do jogador ([`compute_race_evaluation`]) e o
/// pano de fundo do boletim ([`performance_context_facts`]) montavam este campo
/// separadamente e chegavam a méritos incompatíveis — um usava
/// [`Team::car_strength`] (0–100), o outro a coluna crua `car_performance`
/// (domínio −5..16, e que o sistema de peças nem atualiza). Com escalas diferentes
/// o mesmo piloto tinha duas posições-potencial, duas metas e dois `Assessment` na
/// MESMA corrida — o debrief dizia "dia de somar" e o boletim, "muito abaixo do
/// esperado".
pub(super) fn build_merit_field(
    conn: &rusqlite::Connection,
    result: &RaceResult,
) -> Vec<DriverMerit> {
    let field_size = result.race_results.len().max(1) as i32;
    let mut car_cache: HashMap<String, f64> = HashMap::new();

    result
        .race_results
        .iter()
        .map(|r| {
            let driver = driver_queries::get_driver(conn, &r.pilot_id).ok();
            let skill = driver
                .as_ref()
                .map(|d| d.atributos.skill)
                .unwrap_or(MERIT_FALLBACK);
            let car = *car_cache.entry(r.team_id.clone()).or_insert_with(|| {
                team_queries::get_team_by_id(conn, &r.team_id)
                    .ok()
                    .flatten()
                    .map(|t| t.car_strength())
                    .unwrap_or(MERIT_FALLBACK)
            });
            let recent = driver
                .as_ref()
                .and_then(|d| recent_avg_finish(&d.ultimos_resultados));
            DriverMerit {
                pilot_id: r.pilot_id.clone(),
                // Corridas na pista entram na Fase 2 (afinamento) — ver `compute_merit`.
                merit: compute_merit(skill, car, recent, field_size, 0),
            }
        })
        .collect()
}
