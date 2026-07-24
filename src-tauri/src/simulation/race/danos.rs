//! Dano latente pós-colisão: o carro que sobreviveu à batida carrega a avaria e testa, a cada
//! segmento, se ela se manifesta — em perda de posições ou em abandono.

use rand::Rng;

use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::SimDriver;
use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};

use super::tipos::{RaceSegment, RaceState};

/// Processa danos latentes pós-colisão antes dos rolls normais do segmento.
/// Para cada piloto não-DNF com pending_damage, testa a chance de manifestação.
pub(crate) fn process_pending_damage(
    states: &mut [RaceState],
    segment: RaceSegment,
    drivers: &[SimDriver],
    catalog: &IncidentCatalog,
    vehicle_class: crate::simulation::catalog::VehicleClass,
    is_endurance: bool,
    rng: &mut impl Rng,
) {
    let seg_str = segment.as_str();
    for state in states.iter_mut() {
        if state.is_dnf || state.pending_damage.is_empty() {
            continue;
        }
        let driver_name = drivers
            .iter()
            .find(|d| d.id == state.driver_id)
            .map(|d| d.nome.as_str())
            .unwrap_or("Piloto");

        let mut indices_to_remove: Vec<usize> = Vec::new();

        for (i, pd) in state.pending_damage.iter_mut().enumerate() {
            if rng.gen::<f64>() < pd.manifest_chance {
                // Dano manifestou — determinar se é DNF
                let is_dnf = pd.is_dnf_capable && rng.gen::<f64>() < 0.70;
                // Re-renderizar o catálogo com o nome correto e severidade correta
                let (desc, cat_id) = if let Some(sel) = catalog.select_and_render(
                    vehicle_class,
                    is_endurance,
                    crate::simulation::catalog::IncidentSource::PostCollision,
                    crate::simulation::catalog::TriggerType::PostCollision,
                    is_dnf,
                    driver_name,
                    rng,
                ) {
                    (sel.rendered_text, Some(sel.catalog_id))
                } else if is_dnf {
                    (
                        format!("{} abandona por dano de colisao anterior", driver_name),
                        None,
                    )
                } else {
                    (
                        format!(
                            "{} perde posicoes por dano de colisao anterior",
                            driver_name
                        ),
                        None,
                    )
                };

                let incident = IncidentResult {
                    pilot_id: state.driver_id.clone(),
                    incident_type: IncidentType::Mechanical,
                    severity: if is_dnf {
                        IncidentSeverity::Major
                    } else {
                        IncidentSeverity::Minor
                    },
                    segment: seg_str.to_string(),
                    positions_lost: if is_dnf { 0 } else { 2 },
                    is_dnf,
                    description: desc,
                    linked_pilot_id: None,
                    is_two_car_incident: false,
                    injury_risk_multiplier: if is_dnf { 1.5 } else { 1.0 },
                    narrative_importance_hint: if is_dnf { 2 } else { 1 },
                    catalog_id: cat_id,
                    damage_origin_segment: Some(pd.origin_segment.clone()),
                };

                if is_dnf {
                    state.is_dnf = true;
                    state.dnf_reason = Some(incident.description.clone());
                    state.dnf_segment = Some(segment);
                }
                state.incidents.push(incident);
                indices_to_remove.push(i);
            } else {
                pd.manifest_chance += 0.15;
            }
        }

        // Remover manifestados de trás para frente
        for &i in indices_to_remove.iter().rev() {
            state.pending_damage.remove(i);
        }
    }
}
