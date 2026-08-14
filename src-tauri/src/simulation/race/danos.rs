//! Dano latente pós-colisão: o carro que sobreviveu à batida carrega a avaria e testa, a cada
//! segmento, se ela se manifesta — em perda de posições ou em abandono.

use rand::Rng;

use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::SimDriver;
use crate::simulation::incidents::{
    IncidentResult, IncidentSeverity, IncidentType, CHANCE_DE_ABANDONO_NA_MANIFESTACAO,
    IRM_DANO_LATENTE_COM_ABANDONO, IRM_DANO_LATENTE_SEM_ABANDONO,
};

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
        let piloto = drivers.iter().find(|d| d.id == state.driver_id);
        let anonimo = rust_i18n::t!("race.incident.unknown_driver").to_string();
        let driver_name = piloto.map(|d| d.nome.as_str()).unwrap_or(&anonimo);
        // A classe do carro DESTE piloto; sem ela, a da corrida. Ver `SimDriver::vehicle_class`.
        let classe = piloto
            .and_then(|d| d.vehicle_class)
            .unwrap_or(vehicle_class);

        let mut indices_to_remove: Vec<usize> = Vec::new();

        for (i, pd) in state.pending_damage.iter_mut().enumerate() {
            if rng.gen::<f64>() < pd.manifest_chance {
                // Dano manifestou — determinar se é DNF. Perder posições é o desfecho COMUM
                // (o carro segue torto); o abandono é a minoria. Ver a constante.
                let is_dnf =
                    pd.is_dnf_capable && rng.gen::<f64>() < CHANCE_DE_ABANDONO_NA_MANIFESTACAO;
                // Re-renderizar o catálogo com o nome correto e severidade correta
                let (desc, cat_id) = if let Some(sel) = catalog.select_and_render(
                    classe,
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
                        rust_i18n::t!("race.incident.latent_damage_dnf", name = driver_name)
                            .to_string(),
                        None,
                    )
                } else {
                    (
                        rust_i18n::t!("race.incident.latent_damage_positions", name = driver_name)
                            .to_string(),
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
                    // Os dois desfechos podem machucar, em ordem: abandonar com o carro
                    // avariado (15%, mesmo peso da pane crítica) pesa mais que seguir na
                    // pista perdendo posições (5%). Os `1.5`/`1.0` que estavam aqui davam
                    // 37,5% e 25%.
                    injury_risk_multiplier: if is_dnf {
                        IRM_DANO_LATENTE_COM_ABANDONO
                    } else {
                        IRM_DANO_LATENTE_SEM_ABANDONO
                    },
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

                if is_dnf {
                    // ABANDONOU: o carro parou, e um carro parado não manifesta a segunda
                    // avaria. Sem este corte o laço seguia testando os outros
                    // `pending_damage` do mesmo piloto no mesmo segmento, e cada um que
                    // manifestasse como abandono sobrescrevia `dnf_reason`/`dnf_segment` e
                    // empilhava mais um incidente `is_dnf = true` — o resultado saía com
                    // dois abandonos para o mesmo carro e o motivo publicado era o do
                    // último dano, não o que de fato o tirou da corrida.
                    //
                    // O corte MUDA o consumo de RNG, e não tem como não mudar: os danos
                    // restantes deste piloto deixam de sortear a manifestação, e o
                    // deslocamento se propaga para todos os pilotos seguintes do segmento.
                    // Só é alcançável no caso que era bugado (piloto com 2+ danos latentes
                    // que abandona antes do último), e a alternativa — sortear e jogar fora
                    // — preservaria a sequência ao custo de escrever um sorteio morto.
                    break;
                }
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

// `#[path]` explícito: este módulo é carregado por `#[path = "race/danos.rs"]`, então o
// diretório dos filhos dele é `race/` e não `race/danos/`. Sem esta linha, o `mod tests`
// daqui resolveria para `race/tests/mod.rs` — o arquivo de testes da CORRIDA — e o
// sequestraria para dentro deste módulo.
#[cfg(test)]
#[path = "danos/tests.rs"]
mod tests;
