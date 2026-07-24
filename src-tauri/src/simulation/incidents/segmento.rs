//! A passagem do segmento: para cada carro na pista, sorteia pane, erro e colisão nessa ordem,
//! garante um incidente por piloto por segmento e pendura o dano latente de quem se bateu.

use rand::Rng;

use crate::models::enums::WeatherCondition;

use crate::simulation::catalog::{IncidentCatalog, IncidentSource, TriggerType, VehicleClass};
use crate::simulation::context::SimDriver;
use crate::simulation::race::{RaceSegment, RaceState};

use super::risco::make_incident;
use super::sorteios::{
    avg_neighbor_aggression, find_neighbor, resolve_collision_consequence, roll_collision,
    roll_driver_error, roll_mechanical,
};
use super::tipos::{IncidentSeverity, IncidentType, PendingDamage, SegmentIncidentResult};

/// Incidentes do segmento com a pane mecânica do catálogo LIGADA — o comportamento histórico.
/// Ver [`process_segment_incidents_cfg`] para o caminho onde o Sistema de Quebra assume a pane.
#[allow(clippy::too_many_arguments)]
pub fn process_segment_incidents(
    drivers: &[SimDriver],
    states: &[RaceState],
    segment: RaceSegment,
    weather: WeatherCondition,
    is_championship_deciding: bool,
    incident_rate_multiplier: f64,
    start_chaos_multiplier: f64,
    pack_density_factor: f64,
    catalog: &IncidentCatalog,
    vehicle_class: VehicleClass,
    is_endurance: bool,
    rng: &mut impl Rng,
) -> SegmentIncidentResult {
    process_segment_incidents_cfg(
        drivers,
        states,
        segment,
        weather,
        is_championship_deciding,
        incident_rate_multiplier,
        start_chaos_multiplier,
        pack_density_factor,
        catalog,
        vehicle_class,
        is_endurance,
        true,
        rng,
    )
}

/// Igual a [`process_segment_incidents`], mas com `catalog_mechanical`: quando `false`, a pane
/// mecânica genérica do catálogo NÃO é sorteada.
///
/// Existe por causa da fonte única de pane. A pane do catálogo ([`roll_mechanical`]) sorteia
/// sobre a `confiabilidade` ABSTRATA da equipe e não sabe que peças o carro tem — não nomeia
/// culpado, não danifica nada e não conversa com a economia. O Sistema de Quebra faz as quatro
/// coisas, lendo o desgaste real. Onde ele roda, esta aqui sai de cena: manter as duas dobraria
/// a taxa de abandono mecânico e deixaria carro de peça nova fundindo motor sem o aviso
/// pré-corrida que o design promete.
#[allow(clippy::too_many_arguments)]
pub fn process_segment_incidents_cfg(
    drivers: &[SimDriver],
    states: &[RaceState],
    segment: RaceSegment,
    weather: WeatherCondition,
    is_championship_deciding: bool,
    incident_rate_multiplier: f64,
    start_chaos_multiplier: f64,
    pack_density_factor: f64,
    catalog: &IncidentCatalog,
    vehicle_class: VehicleClass,
    is_endurance: bool,
    catalog_mechanical: bool,
    rng: &mut impl Rng,
) -> SegmentIncidentResult {
    let mut incidents = Vec::new();
    let mut new_pending_damage: Vec<(String, PendingDamage)> = Vec::new();
    let total_drivers = states.len() as i32;
    let seg = segment.as_str();
    let mut affected: Vec<String> = Vec::new();

    for state in states {
        if state.is_dnf || affected.contains(&state.driver_id) {
            continue;
        }

        let Some(driver) = drivers.iter().find(|d| d.id == state.driver_id) else {
            continue;
        };

        // Pane do catálogo: só quando o Sistema de Quebra NÃO está no comando desta corrida.
        //
        // O sorteio ACONTECE e o resultado é descartado, em vez de pular a chamada. É de
        // propósito: `roll_mechanical` consome do RNG, então pular deslocaria o fluxo e mudaria
        // TODOS os erros de piloto e batidas seguintes. Descartando, a única coisa que sai da
        // corrida é a pane — o resto do comportamento fica byte-idêntico ao de antes.
        if let Some((severity, is_dnf, pos_lost)) = roll_mechanical(
            driver.car_reliability,
            segment,
            incident_rate_multiplier,
            rng,
        )
        .filter(|_| catalog_mechanical)
        {
            let generic_desc = if is_dnf {
                format!("{} abandona com problema mecanico", driver.nome)
            } else {
                format!(
                    "{} perde {} posicoes por problema mecanico",
                    driver.nome, pos_lost
                )
            };
            let (catalog_id, desc) = match catalog.select_and_render(
                vehicle_class,
                is_endurance,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                is_dnf,
                &driver.nome,
                rng,
            ) {
                Some(sel) => (Some(sel.catalog_id), sel.rendered_text),
                None => (None, generic_desc),
            };
            incidents.push(make_incident(
                driver.id.clone(),
                IncidentType::Mechanical,
                severity,
                seg,
                pos_lost,
                is_dnf,
                desc,
                None,
                false,
                catalog_id,
                None,
            ));
            affected.push(driver.id.clone());
            continue;
        }

        if let Some((severity, is_dnf, pos_lost)) = roll_driver_error(
            driver,
            state,
            segment,
            weather,
            is_championship_deciding,
            incident_rate_multiplier,
            start_chaos_multiplier,
            rng,
        ) {
            // Stall check: DriverError Minor não-DNF pode escalar para DNF por stall
            let (final_severity, final_is_dnf, final_pos_lost, stall_catalog_id) =
                if severity == IncidentSeverity::Minor && !is_dnf {
                    let stall_base = 0.08;
                    let exp_mod = 1.0 - (driver.experiencia as f64 / 100.0 * 0.50);
                    let stall_chance = (stall_base * exp_mod).clamp(0.01, 0.08);

                    if rng.gen::<f64>() < stall_chance {
                        let stall_id = catalog
                            .select_and_render(
                                vehicle_class,
                                is_endurance,
                                IncidentSource::Operational,
                                TriggerType::PostSpinStall,
                                true,
                                &driver.nome,
                                rng,
                            )
                            .map(|sel| (sel.catalog_id, sel.rendered_text));
                        (IncidentSeverity::Major, true, 0, stall_id)
                    } else {
                        (severity, is_dnf, pos_lost, None)
                    }
                } else {
                    (severity, is_dnf, pos_lost, None)
                };

            let (catalog_id, desc) = if let Some((sid, stall_text)) = stall_catalog_id {
                (Some(sid), stall_text)
            } else {
                let generic_desc = if final_is_dnf {
                    format!("{} abandona apos erro de pilotagem", driver.nome)
                } else {
                    format!(
                        "{} comete erro e perde {} posicoes",
                        driver.nome, final_pos_lost
                    )
                };
                match catalog.select_and_render(
                    vehicle_class,
                    is_endurance,
                    IncidentSource::DriverError,
                    TriggerType::Spontaneous,
                    final_is_dnf,
                    &driver.nome,
                    rng,
                ) {
                    Some(sel) => (Some(sel.catalog_id), sel.rendered_text),
                    None => (None, generic_desc),
                }
            };

            incidents.push(make_incident(
                driver.id.clone(),
                IncidentType::DriverError,
                final_severity,
                seg,
                final_pos_lost,
                final_is_dnf,
                desc,
                None,
                false,
                catalog_id,
                None,
            ));
            affected.push(driver.id.clone());
            continue;
        }

        let neighbor_agg =
            avg_neighbor_aggression(&driver.id, state.current_position, drivers, states);

        if let Some(severity) = roll_collision(
            driver,
            state.current_position,
            total_drivers,
            neighbor_agg,
            segment,
            weather,
            incident_rate_multiplier,
            start_chaos_multiplier,
            pack_density_factor,
            rng,
        ) {
            let neighbor_id = find_neighbor(&driver.id, state.current_position, states, &affected);
            let has_neighbor = neighbor_id.is_some();
            let (trig_dnf, trig_lost) = resolve_collision_consequence(rng);
            // Resolução 4: colisões diretas não consultam o catálogo — texto genérico preservado.
            // O catálogo PostCollision é usado apenas para dano latente (Fase 2).
            let trig_desc = if trig_dnf {
                format!("{} abandona apos colisao", driver.nome)
            } else {
                format!("{} perde {} posicoes em colisao", driver.nome, trig_lost)
            };
            incidents.push(make_incident(
                driver.id.clone(),
                IncidentType::Collision,
                severity,
                seg,
                trig_lost,
                trig_dnf,
                trig_desc,
                neighbor_id.clone(),
                has_neighbor,
                None,
                None,
            ));
            affected.push(driver.id.clone());

            // Roll pós-colisão para o trigger (somente se não-DNF)
            if !trig_dnf {
                maybe_add_pending_damage(
                    &driver.id,
                    severity,
                    seg,
                    catalog,
                    vehicle_class,
                    is_endurance,
                    driver.car_reliability,
                    &mut new_pending_damage,
                    rng,
                );
            }

            if let Some(ref neighbor_id) = neighbor_id {
                let neighbor = drivers.iter().find(|d| d.id == *neighbor_id);
                let neighbor_name = neighbor.map(|d| d.nome.as_str()).unwrap_or("Piloto");
                let (nb_dnf, nb_lost) = resolve_collision_consequence(rng);
                let nb_desc = if nb_dnf {
                    format!(
                        "{} abandona apos colisao com {}",
                        neighbor_name, driver.nome
                    )
                } else {
                    format!(
                        "{} perde {} posicoes em colisao com {}",
                        neighbor_name, nb_lost, driver.nome
                    )
                };
                incidents.push(make_incident(
                    neighbor_id.clone(),
                    IncidentType::Collision,
                    severity,
                    seg,
                    nb_lost,
                    nb_dnf,
                    nb_desc,
                    Some(driver.id.clone()),
                    true,
                    None,
                    None,
                ));
                affected.push(neighbor_id.clone());

                // Roll pós-colisão para o neighbor (somente se não-DNF)
                if !nb_dnf {
                    if let Some(nb_driver) = neighbor {
                        maybe_add_pending_damage(
                            neighbor_id,
                            severity,
                            seg,
                            catalog,
                            vehicle_class,
                            is_endurance,
                            nb_driver.car_reliability,
                            &mut new_pending_damage,
                            rng,
                        );
                    }
                }
            }
        }
    }

    SegmentIncidentResult {
        incidents,
        new_pending_damage,
    }
}

/// Roll de dano pós-colisão: se sucesso, cria um PendingDamage e o adiciona à lista.
fn maybe_add_pending_damage(
    driver_id: &str,
    severity: IncidentSeverity,
    origin_segment: &str,
    catalog: &IncidentCatalog,
    vehicle_class: VehicleClass,
    is_endurance: bool,
    car_reliability: f64,
    pending: &mut Vec<(String, PendingDamage)>,
    rng: &mut impl Rng,
) {
    let base_chance = match severity {
        IncidentSeverity::Major | IncidentSeverity::Critical => 0.45,
        IncidentSeverity::Minor => 0.25,
    };
    let reliability_mod = 1.0 - (car_reliability / 100.0 * 0.40);
    let chance = (base_chance * reliability_mod).clamp(0.05, 0.80);

    if rng.gen::<f64>() < chance {
        if let Some(sel) = catalog.select_and_render(
            vehicle_class,
            is_endurance,
            IncidentSource::PostCollision,
            TriggerType::PostCollision,
            false, // is_dnf: seleciona template inicial (manifestação decide DNF)
            "?",   // placeholder — será substituído quando o dano manifestar
            rng,
        ) {
            pending.push((
                driver_id.to_string(),
                PendingDamage {
                    catalog_id: sel.catalog_id,
                    origin_segment: origin_segment.to_string(),
                    manifest_chance: 0.20,
                    is_dnf_capable: matches!(
                        severity,
                        IncidentSeverity::Major | IncidentSeverity::Critical
                    ),
                },
            ));
        }
    }
}
