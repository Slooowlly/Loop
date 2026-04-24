use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::constants::scoring::RACE_SCORE_TO_LAP_MS;

use super::catalog::IncidentCatalog;
use super::context::{SimDriver, SimulationContext};
use super::incidents::{
    process_segment_incidents, IncidentResult, IncidentSeverity, IncidentType, PendingDamage,
};
use super::math::{adjusted_weather_multiplier, normalize_car_performance};
use super::qualifying::QualifyingResult;
use super::track_profile::TrackCharacter;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RaceSegment {
    Start,
    Early,
    Mid,
    Late,
    Finish,
}

impl RaceSegment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "START",
            Self::Early => "EARLY",
            Self::Mid => "MID",
            Self::Late => "LATE",
            Self::Finish => "FINISH",
        }
    }

    /// Ordinal para comparação de segmento em DNF ordering (maior = mais tarde na corrida).
    fn ordinal(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::Early => 1,
            Self::Mid => 2,
            Self::Late => 3,
            Self::Finish => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassificationStatus {
    Finished,
    Dnf,
}

#[derive(Debug, Clone)]
pub struct RaceState {
    pub driver_id: String,
    pub tire_wear: f64,
    pub physical_condition: f64,
    pub cumulative_score: f64,
    pub is_dnf: bool,
    pub current_position: i32,
    pub incidents: Vec<IncidentResult>,
    pub dnf_reason: Option<String>,
    pub dnf_segment: Option<RaceSegment>,
    /// Danos latentes pós-colisão aguardando manifestação.
    pub pending_damage: Vec<PendingDamage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceDriverResult {
    pub pilot_id: String,
    pub pilot_name: String,
    pub team_id: String,
    pub team_name: String,
    pub grid_position: i32,
    pub finish_position: i32,
    pub positions_gained: i32,
    pub best_lap_time_ms: f64,
    pub total_race_time_ms: f64,
    pub gap_to_winner_ms: f64,
    pub is_dnf: bool,
    pub dnf_reason: Option<String>,
    pub dnf_segment: Option<String>,
    #[serde(default)]
    pub incidents_count: i32,
    #[serde(default)]
    pub incidents: Vec<IncidentResult>,
    pub has_fastest_lap: bool,
    pub points_earned: i32,
    pub is_jogador: bool,
    pub laps_completed: i32,
    pub final_tire_wear: f64,
    pub final_physical: f64,
    pub classification_status: ClassificationStatus,
    /// Descrição de conveniência do pior incidente (narrative_importance_hint >= 2).
    /// Campo derivado — não é fonte factual primária.
    #[serde(default)]
    pub notable_incident: Option<String>,
    /// ID da entry do catálogo do incidente que causou o DNF.
    #[serde(default)]
    pub dnf_catalog_id: Option<String>,
    /// Segmento de origem do dano (pode diferir do segmento do DNF para dano latente).
    #[serde(default)]
    pub damage_origin_segment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceResult {
    pub qualifying_results: Vec<QualifyingResult>,
    pub race_results: Vec<RaceDriverResult>,
    pub pole_sitter_id: String,
    pub winner_id: String,
    pub fastest_lap_id: String,
    pub total_laps: i32,
    pub weather: String,
    pub track_name: String,
    #[serde(default)]
    pub total_incidents: i32,
    #[serde(default)]
    pub total_dnfs: i32,
    /// Incidentes com narrative_importance_hint >= 1.
    #[serde(default)]
    pub main_incident_count: i32,
    /// Pilot IDs com incidente headline (hint >= 2).
    #[serde(default)]
    pub notable_incident_pilot_ids: Vec<String>,
    /// Piloto que mais ganhou posições.
    #[serde(default)]
    pub most_positions_gained_id: Option<String>,
}

pub fn simulate_race(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    catalog: &IncidentCatalog,
    is_endurance: bool,
    rng: &mut impl Rng,
) -> RaceResult {
    let total_drivers = qualifying.len() as i32;
    let mut states: Vec<RaceState> = qualifying
        .iter()
        .map(|result| RaceState {
            driver_id: result.pilot_id.clone(),
            tire_wear: 1.0,
            physical_condition: 1.0,
            cumulative_score: (total_drivers - result.position + 1) as f64 * 2.0,
            is_dnf: false,
            current_position: result.position,
            incidents: Vec::new(),
            dnf_reason: None,
            dnf_segment: None,
            pending_damage: Vec::new(),
        })
        .collect();

    for segment in [
        RaceSegment::Start,
        RaceSegment::Early,
        RaceSegment::Mid,
        RaceSegment::Late,
        RaceSegment::Finish,
    ] {
        if ctx.incidents_enabled {
            // Processar danos latentes ANTES dos rolls normais do segmento
            process_pending_damage(
                &mut states,
                segment,
                drivers,
                catalog,
                ctx.vehicle_class,
                is_endurance,
                rng,
            );

            let result = process_segment_incidents(
                drivers,
                &states,
                segment,
                ctx.weather,
                ctx.is_championship_deciding,
                ctx.incident_rate_multiplier,
                ctx.start_chaos_multiplier,
                ctx.pack_density_factor,
                catalog,
                ctx.vehicle_class,
                is_endurance,
                rng,
            );

            for incident in result.incidents {
                if let Some(state) = states.iter_mut().find(|s| s.driver_id == incident.pilot_id) {
                    if incident.is_dnf {
                        state.is_dnf = true;
                        state.dnf_reason = Some(incident.description.clone());
                        state.dnf_segment = Some(segment);
                    }
                    state.incidents.push(incident);
                }
            }

            // Aplicar novos danos latentes gerados neste segmento
            for (driver_id, pd) in result.new_pending_damage {
                if let Some(state) = states.iter_mut().find(|s| s.driver_id == driver_id) {
                    state.pending_damage.push(pd);
                }
            }
        }

        let seg_str = segment.as_str();
        for state in &mut states {
            if state.is_dnf {
                continue;
            }

            if let Some(driver) = drivers.iter().find(|driver| driver.id == state.driver_id) {
                let mut segment_score = calculate_segment_score(driver, state, segment, ctx, rng);
                let penalty: f64 = state
                    .incidents
                    .iter()
                    .filter(|incident| incident.segment == seg_str && !incident.is_dnf)
                    .map(|incident| incident.positions_lost as f64 * 2.0)
                    .sum();
                segment_score = (segment_score - penalty).max(0.0);
                state.cumulative_score += segment_score;
                apply_tire_degradation(state, driver, ctx);
                apply_physical_degradation(state, driver, ctx);
            }
        }

        states.sort_by(|a, b| match (a.is_dnf, b.is_dnf) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => b
                .cumulative_score
                .partial_cmp(&a.cumulative_score)
                .unwrap_or(std::cmp::Ordering::Equal),
        });

        for (index, state) in states.iter_mut().enumerate() {
            state.current_position = index as i32 + 1;
        }
    }

    let mut race_results = build_race_results(drivers, qualifying, ctx, &states, rng);
    let pole_sitter_id = qualifying
        .first()
        .map(|result| result.pilot_id.clone())
        .unwrap_or_default();
    let winner_id = race_results
        .first()
        .map(|result| result.pilot_id.clone())
        .unwrap_or_default();
    let total_incidents: i32 = race_results.iter().map(|r| r.incidents_count).sum();
    let total_dnfs = race_results.iter().filter(|r| r.is_dnf).count() as i32;

    // Aggregate narrative fields
    let main_incident_count: i32 = race_results
        .iter()
        .flat_map(|r| &r.incidents)
        .filter(|i| i.narrative_importance_hint >= 1)
        .count() as i32;

    let notable_incident_pilot_ids: Vec<String> = race_results
        .iter()
        .filter(|r| r.notable_incident.is_some())
        .map(|r| r.pilot_id.clone())
        .collect();

    let most_positions_gained_id = race_results
        .iter()
        .filter(|r| !r.is_dnf && r.positions_gained > 0)
        .max_by_key(|r| r.positions_gained)
        .map(|r| r.pilot_id.clone());

    RaceResult {
        qualifying_results: qualifying.to_vec(),
        race_results: std::mem::take(&mut race_results),
        pole_sitter_id,
        winner_id,
        fastest_lap_id: String::new(),
        total_laps: ctx.total_laps,
        weather: ctx.weather.as_str().to_string(),
        track_name: ctx.track_name.clone(),
        total_incidents,
        total_dnfs,
        main_incident_count,
        notable_incident_pilot_ids,
        most_positions_gained_id,
    }
}

fn build_race_results(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    states: &[RaceState],
    rng: &mut impl Rng,
) -> Vec<RaceDriverResult> {
    // Lookup maps para evitar O(n²)
    let driver_map: HashMap<&str, &SimDriver> =
        drivers.iter().map(|d| (d.id.as_str(), d)).collect();
    let quali_map: HashMap<&str, &QualifyingResult> = qualifying
        .iter()
        .map(|q| (q.pilot_id.as_str(), q))
        .collect();

    // Separar finishers e DNFs para ordenação correta
    let mut finishers: Vec<&RaceState> = states.iter().filter(|s| !s.is_dnf).collect();
    let mut dnfs: Vec<&RaceState> = states.iter().filter(|s| s.is_dnf).collect();

    // Finishers: por cumulative_score desc
    finishers.sort_by(|a, b| {
        b.cumulative_score
            .partial_cmp(&a.cumulative_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // DNFs: segmento mais tardio primeiro; desempate por cumulative_score
    dnfs.sort_by(|a, b| {
        let seg_ord_b = b.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
        let seg_ord_a = a.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
        seg_ord_b.cmp(&seg_ord_a).then_with(|| {
            b.cumulative_score
                .partial_cmp(&a.cumulative_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let ordered: Vec<&RaceState> = finishers.into_iter().chain(dnfs).collect();

    let winner_score = ordered.first().map(|s| s.cumulative_score).unwrap_or(0.0);
    let winner_lap_time_ms = ctx.base_lap_time_ms;
    let winner_total_time_ms = winner_lap_time_ms * ctx.total_laps as f64;

    ordered
        .iter()
        .enumerate()
        .filter_map(|(finish_idx, state)| {
            let driver = driver_map.get(state.driver_id.as_str())?;
            let qualifying_result = quali_map.get(state.driver_id.as_str())?;

            let lap_time_ms = ctx.base_lap_time_ms
                + (winner_score - state.cumulative_score).max(0.0) * RACE_SCORE_TO_LAP_MS;
            let best_lap_factor = rng.gen_range(0.97..=1.0);
            let best_lap_time_ms = lap_time_ms * best_lap_factor;

            let laps_completed = if state.is_dnf {
                estimate_laps_at_dnf(state.dnf_segment, ctx.total_laps)
            } else {
                ctx.total_laps
            };

            let total_race_time_ms = if state.is_dnf {
                // Tempo proporcional às voltas completadas + pequeno overhead
                winner_total_time_ms * (laps_completed as f64 / ctx.total_laps as f64) * 1.05
            } else {
                lap_time_ms * ctx.total_laps as f64
            };

            // gap sempre >= 0
            let gap_to_winner_ms = (total_race_time_ms - winner_total_time_ms).max(0.0);

            // Incidente mais importante para campo de conveniência
            let notable_incident = state
                .incidents
                .iter()
                .filter(|i| i.narrative_importance_hint >= 2)
                .max_by_key(|i| i.narrative_importance_hint)
                .map(|i| i.description.clone());

            let dnf_incident = state.incidents.iter().find(|i| i.is_dnf);
            let dnf_catalog_id = dnf_incident.and_then(|i| i.catalog_id.clone());
            let damage_origin_segment = dnf_incident.and_then(|i| i.damage_origin_segment.clone());

            let classification_status = if state.is_dnf {
                ClassificationStatus::Dnf
            } else {
                ClassificationStatus::Finished
            };

            let finish_position = finish_idx as i32 + 1;

            Some(RaceDriverResult {
                pilot_id: driver.id.clone(),
                pilot_name: driver.nome.clone(),
                team_id: driver.team_id.clone(),
                team_name: driver.team_name.clone(),
                grid_position: qualifying_result.position,
                finish_position,
                positions_gained: qualifying_result.position - finish_position,
                best_lap_time_ms,
                total_race_time_ms,
                gap_to_winner_ms,
                is_dnf: state.is_dnf,
                dnf_reason: state.dnf_reason.clone(),
                dnf_segment: state.dnf_segment.map(|s| s.as_str().to_string()),
                incidents_count: state.incidents.len() as i32,
                incidents: state.incidents.clone(),
                has_fastest_lap: false,
                points_earned: 0,
                is_jogador: driver.is_jogador,
                laps_completed,
                final_tire_wear: state.tire_wear,
                final_physical: state.physical_condition,
                classification_status,
                notable_incident,
                dnf_catalog_id,
                damage_origin_segment,
            })
        })
        .collect()
}

fn estimate_laps_at_dnf(segment: Option<RaceSegment>, total_laps: i32) -> i32 {
    let fraction = match segment {
        Some(RaceSegment::Start) => 0.10,
        Some(RaceSegment::Early) => 0.30,
        Some(RaceSegment::Mid) => 0.50,
        Some(RaceSegment::Late) => 0.70,
        Some(RaceSegment::Finish) => 0.90,
        None => 0.05,
    };
    ((total_laps as f64 * fraction) as i32).max(1)
}

#[derive(Debug, Clone, Copy)]
struct SegmentWeights {
    skill: f64,
    habilidade_largada: f64,
    racecraft: f64,
    car_performance: f64,
    gestao_pneus: f64,
    fitness: f64,
    mentalidade: f64,
    confianca: f64,
}

fn segment_weights(segment: RaceSegment) -> SegmentWeights {
    match segment {
        RaceSegment::Start => SegmentWeights {
            skill: 0.20,
            habilidade_largada: 0.35,
            racecraft: 0.25,
            car_performance: 0.20,
            gestao_pneus: 0.0,
            fitness: 0.0,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Early => SegmentWeights {
            skill: 0.35,
            habilidade_largada: 0.0,
            racecraft: 0.20,
            car_performance: 0.30,
            gestao_pneus: 0.15,
            fitness: 0.0,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Mid => SegmentWeights {
            skill: 0.35,
            habilidade_largada: 0.0,
            racecraft: 0.0,
            car_performance: 0.30,
            gestao_pneus: 0.20,
            fitness: 0.15,
            mentalidade: 0.0,
            confianca: 0.0,
        },
        RaceSegment::Late => SegmentWeights {
            skill: 0.25,
            habilidade_largada: 0.0,
            racecraft: 0.0,
            car_performance: 0.20,
            gestao_pneus: 0.25,
            fitness: 0.20,
            mentalidade: 0.10,
            confianca: 0.0,
        },
        RaceSegment::Finish => SegmentWeights {
            skill: 0.25,
            habilidade_largada: 0.0,
            racecraft: 0.25,
            car_performance: 0.20,
            gestao_pneus: 0.0,
            fitness: 0.0,
            mentalidade: 0.10,
            confianca: 0.20,
        },
    }
}

fn calculate_segment_score(
    driver: &SimDriver,
    state: &RaceState,
    segment: RaceSegment,
    ctx: &SimulationContext,
    rng: &mut impl Rng,
) -> f64 {
    let weights = segment_weights(segment);
    let mut score = driver.skill as f64 * weights.skill
        + driver.habilidade_largada as f64 * weights.habilidade_largada
        + driver.racecraft as f64 * weights.racecraft
        + normalize_car_performance(driver.car_performance) * weights.car_performance
        + driver.gestao_pneus as f64 * weights.gestao_pneus
        + driver.fitness as f64 * weights.fitness
        + driver.mentalidade as f64 * weights.mentalidade
        + driver.confianca as f64 * weights.confianca;

    // Penalidade de pneu
    let tire_penalty = (1.0 - state.tire_wear) * 0.15;
    score *= 1.0 - tire_penalty;

    // Penalidade de fadiga (apenas Late e Finish)
    if matches!(segment, RaceSegment::Late | RaceSegment::Finish) {
        let fatigue_penalty = (1.0 - state.physical_condition) * 0.10;
        score *= 1.0 - fatigue_penalty;
    }

    // Chuva com sensibilidade do contexto
    score *= adjusted_weather_multiplier(ctx.weather, driver.fator_chuva, ctx.rain_sensitivity);

    // Bônus contextual em pista difícil: adaptabilidade vale mais
    if ctx.track_difficulty_multiplier > 1.0 {
        let difficulty_bonus =
            (driver.adaptabilidade as f64 / 100.0) * (ctx.track_difficulty_multiplier - 1.0) * 0.05;
        let consistency_bonus =
            (driver.consistencia as f64 / 100.0) * (ctx.track_difficulty_multiplier - 1.0) * 0.03;
        score += difficulty_bonus + consistency_bonus;
    }

    // Bias de caráter de pista: pequenos ajustes relativos de atributos (skill, car, adaptabilidade)
    let (char_skill_bias, char_car_bias, char_adapt_bias) = match ctx.track_character {
        TrackCharacter::Flowing => (0.02_f64, 0.02, -0.03),
        TrackCharacter::Technical => (0.00, 0.00, 0.00),
        TrackCharacter::Tight => (-0.03, -0.04, 0.05),
        TrackCharacter::Roval => (0.04, 0.03, -0.05),
    };
    score += driver.skill as f64 * char_skill_bias
        + normalize_car_performance(driver.car_performance) * char_car_bias
        + driver.adaptabilidade as f64 * char_adapt_bias;

    // Comprime ou expande spread de habilidade (endurance = campo mais fechado, rookie = mais aberto)
    let midpoint = 60.0_f64;
    score = midpoint + (score - midpoint) * ctx.race_pace_spread_multiplier;

    if driver.corridas_na_categoria < 10 {
        let inexperience_factor = (10 - driver.corridas_na_categoria).max(0) as f64 * 0.003;
        score *= 1.0 - inexperience_factor;
    }

    // Variância escalada pelo perfil da categoria
    let base_variance = (100.0 - driver.consistencia as f64) / 100.0 * 5.0;
    let scaled_variance = base_variance * ctx.race_variance_multiplier;

    // Caos extra na largada amplificado por densidade do pelotão
    let actual_variance = if segment == RaceSegment::Start {
        scaled_variance * ctx.start_chaos_multiplier * ctx.pack_density_factor
    } else {
        scaled_variance
    };

    score += rng.gen_range(-actual_variance..=actual_variance);
    score.max(5.0)
}

fn apply_tire_degradation(state: &mut RaceState, driver: &SimDriver, ctx: &SimulationContext) {
    let mgmt_factor = 1.0 - (driver.gestao_pneus as f64 / 100.0 * 0.50);
    let smoothness_factor = 1.0 - (driver.smoothness as f64 / 100.0 * 0.20);
    let duration_factor = (ctx.race_duration_minutes as f64 / 30.0).max(0.25);
    let actual_degradation =
        ctx.tire_degradation_rate * mgmt_factor * smoothness_factor * duration_factor;
    state.tire_wear = (state.tire_wear - actual_degradation).max(0.1);
}

fn apply_physical_degradation(state: &mut RaceState, driver: &SimDriver, ctx: &SimulationContext) {
    let fit_factor = 1.0 - (driver.fitness as f64 / 100.0 * 0.60);
    let duration_factor = (ctx.race_duration_minutes as f64 / 30.0).max(0.25);
    let actual_degradation = ctx.physical_degradation_rate * fit_factor * duration_factor;
    state.physical_condition = (state.physical_condition - actual_degradation).max(0.2);
}

/// Processa danos latentes pós-colisão antes dos rolls normais do segmento.
/// Para cada piloto não-DNF com pending_damage, testa a chance de manifestação.
fn process_pending_damage(
    states: &mut Vec<RaceState>,
    segment: RaceSegment,
    drivers: &[SimDriver],
    catalog: &IncidentCatalog,
    vehicle_class: super::catalog::VehicleClass,
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
                    super::catalog::IncidentSource::PostCollision,
                    super::catalog::TriggerType::PostCollision,
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

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use crate::models::driver::Driver;
    use crate::models::enums::WeatherCondition;
    use crate::models::team::placeholder_team_from_db;
    use crate::simulation::car_build::CarBuildProfile;
    use crate::simulation::context::SimulationContext;
    use crate::simulation::track_profile::TrackCharacter;

    use super::*;
    use crate::simulation::qualifying::simulate_qualifying;

    fn sample_context(duration: i32, weather: WeatherCondition) -> SimulationContext {
        SimulationContext {
            weather,
            race_duration_minutes: duration,
            ..SimulationContext::test_default()
        }
    }

    fn sample_context_with_incidents(
        duration: i32,
        weather: WeatherCondition,
    ) -> SimulationContext {
        SimulationContext {
            incidents_enabled: true,
            weather,
            race_duration_minutes: duration,
            ..SimulationContext::test_default()
        }
    }

    fn build_driver(
        id: &str,
        skill: f64,
        racecraft: f64,
        pneus: f64,
        fitness: f64,
        car: f64,
    ) -> SimDriver {
        let mut driver = Driver::create_player(
            id.to_string(),
            format!("Driver {}", id),
            "🇧🇷 Brasileiro".to_string(),
            20,
        );
        driver.is_jogador = false;
        driver.atributos.skill = skill;
        driver.atributos.consistencia = 88.0;
        driver.atributos.racecraft = racecraft;
        driver.atributos.ritmo_classificacao = skill;
        driver.atributos.habilidade_largada = 70.0;
        driver.atributos.gestao_pneus = pneus;
        driver.atributos.fitness = fitness;
        driver.atributos.mentalidade = 72.0;
        driver.atributos.confianca = 70.0;
        driver.atributos.adaptabilidade = 68.0;
        driver.atributos.fator_chuva = 50.0;

        let mut team = placeholder_team_from_db(
            format!("T{}", id),
            format!("Team {}", id),
            "gt4".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = car;
        team.confiabilidade = 80.0;

        SimDriver::from_driver_and_team(&driver, &team)
    }

    fn build_grid() -> Vec<SimDriver> {
        (0..12)
            .map(|index| {
                build_driver(
                    &format!("{:03}", index + 1),
                    60.0 + index as f64,
                    60.0,
                    65.0,
                    70.0,
                    8.0,
                )
            })
            .collect()
    }

    fn build_driver_for_track(id: &str, profile: CarBuildProfile, track_id: u32) -> SimDriver {
        let mut driver = Driver::create_player(
            id.to_string(),
            format!("Driver {}", id),
            "Brasileiro".to_string(),
            20,
        );
        driver.is_jogador = false;
        driver.atributos.skill = 75.0;
        driver.atributos.consistencia = 88.0;
        driver.atributos.racecraft = 75.0;
        driver.atributos.ritmo_classificacao = 75.0;
        driver.atributos.habilidade_largada = 70.0;
        driver.atributos.gestao_pneus = 70.0;
        driver.atributos.fitness = 72.0;
        driver.atributos.mentalidade = 72.0;
        driver.atributos.confianca = 70.0;
        driver.atributos.adaptabilidade = 68.0;
        driver.atributos.fator_chuva = 50.0;

        let mut team = placeholder_team_from_db(
            format!("T{}", id),
            format!("Team {}", id),
            "gt4".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 8.0;
        team.confiabilidade = 80.0;
        team.car_build_profile = profile;

        SimDriver::from_driver_team_and_track(&driver, &team, track_id)
    }

    #[test]
    fn test_race_returns_all_drivers() {
        let grid = build_grid();
        let mut rng = StdRng::seed_from_u64(21);
        let ctx = sample_context(30, WeatherCondition::Dry);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            &mut rng,
        );

        assert_eq!(result.race_results.len(), 12);
    }

    #[test]
    fn test_race_acceleration_profile_beats_wrong_profile_at_tsukuba() {
        let accel_driver = build_driver_for_track("ACC", CarBuildProfile::AccelerationExtreme, 325);
        let power_driver = build_driver_for_track("PWR", CarBuildProfile::PowerExtreme, 325);
        let ctx = SimulationContext {
            track_id: 325,
            track_character: TrackCharacter::Tight,
            ..sample_context(25, WeatherCondition::Dry)
        };
        let mut rng = StdRng::seed_from_u64(456);
        let grid = vec![accel_driver.clone(), power_driver.clone()];
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            &mut rng,
        );

        let accel_pos = result
            .race_results
            .iter()
            .find(|item| item.pilot_id == accel_driver.id)
            .expect("accel classification")
            .finish_position;
        let power_pos = result
            .race_results
            .iter()
            .find(|item| item.pilot_id == power_driver.id)
            .expect("power classification")
            .finish_position;

        assert!(
            accel_pos < power_pos,
            "expected acceleration profile to race ahead at Tsukuba"
        );
    }

    #[test]
    fn test_race_positions_sequential() {
        let grid = build_grid();
        let mut rng = StdRng::seed_from_u64(22);
        let ctx = sample_context(30, WeatherCondition::Dry);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            &mut rng,
        );
        assert_eq!(
            result
                .race_results
                .iter()
                .map(|value| value.finish_position)
                .collect::<Vec<_>>(),
            (1..=12).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_race_tire_degradation() {
        let grid = build_grid();
        let mut rng = StdRng::seed_from_u64(23);
        let ctx = sample_context(45, WeatherCondition::Dry);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            &mut rng,
        );

        assert!(result
            .race_results
            .iter()
            .all(|driver| driver.final_tire_wear < 1.0));
    }

    #[test]
    fn test_race_physical_degradation() {
        let grid = build_grid();
        let mut rng = StdRng::seed_from_u64(24);
        let ctx = sample_context(45, WeatherCondition::Dry);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            &mut rng,
        );

        assert!(result
            .race_results
            .iter()
            .all(|driver| driver.final_physical < 1.0));
    }

    #[test]
    fn test_race_positions_gained_calculated() {
        let grid = build_grid();
        let mut rng = StdRng::seed_from_u64(25);
        let ctx = sample_context(30, WeatherCondition::Dry);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            &mut rng,
        );

        assert!(result.race_results.iter().all(|driver| {
            driver.positions_gained == driver.grid_position - driver.finish_position
        }));
    }

    #[test]
    fn test_race_good_driver_tends_to_win() {
        let ace = build_driver("ACE", 95.0, 90.0, 85.0, 88.0, 15.0);
        let grid: Vec<SimDriver> = std::iter::once(ace.clone())
            .chain(
                (0..11)
                    .map(|index| build_driver(&format!("R{index}"), 60.0, 60.0, 60.0, 60.0, 6.0)),
            )
            .collect();

        let mut wins = 0;
        for seed in 0..50 {
            let mut rng = StdRng::seed_from_u64(seed);
            let ctx = sample_context(35, WeatherCondition::Dry);
            let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
            let result = simulate_race(
                &grid,
                &qualifying,
                &ctx,
                &IncidentCatalog::empty(),
                false,
                &mut rng,
            );
            if result.winner_id == ace.id {
                wins += 1;
            }
        }

        assert!(wins >= 35, "ace driver only won {} times", wins);
    }

    #[test]
    fn test_race_bad_tires_hurt_late_segments() {
        let tire_saver = build_driver("SAVE", 78.0, 72.0, 92.0, 75.0, 10.0);
        let tire_abuser = build_driver("ABUSE", 78.0, 72.0, 25.0, 75.0, 10.0);
        let grid = vec![tire_saver.clone(), tire_abuser.clone()];

        let mut saver_better = 0;
        for seed in 0..30 {
            let mut rng = StdRng::seed_from_u64(seed);
            let ctx = sample_context(60, WeatherCondition::Dry);
            let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
            let result = simulate_race(
                &grid,
                &qualifying,
                &ctx,
                &IncidentCatalog::empty(),
                false,
                &mut rng,
            );
            if result.winner_id == tire_saver.id {
                saver_better += 1;
            }
        }

        assert!(saver_better >= 20);
    }

    #[test]
    fn test_rookie_category_experience_penalizes_race_score() {
        let mut rookie = build_driver("ROOKIE", 80.0, 78.0, 75.0, 76.0, 10.0);
        rookie.corridas_na_categoria = 2;

        let mut veteran = rookie.clone();
        veteran.id = "VETERAN".to_string();
        veteran.nome = "Driver VETERAN".to_string();
        veteran.corridas_na_categoria = 18;

        let ctx = SimulationContext {
            race_variance_multiplier: 0.0,
            ..sample_context(30, WeatherCondition::Dry)
        };
        let state = RaceState {
            driver_id: rookie.id.clone(),
            tire_wear: 1.0,
            physical_condition: 1.0,
            cumulative_score: 0.0,
            is_dnf: false,
            current_position: 1,
            incidents: Vec::new(),
            dnf_reason: None,
            dnf_segment: None,
            pending_damage: Vec::new(),
        };

        let mut rookie_rng = StdRng::seed_from_u64(101);
        let rookie_score =
            calculate_segment_score(&rookie, &state, RaceSegment::Mid, &ctx, &mut rookie_rng);

        let mut veteran_rng = StdRng::seed_from_u64(101);
        let veteran_score =
            calculate_segment_score(&veteran, &state, RaceSegment::Mid, &ctx, &mut veteran_rng);

        assert!(
            rookie_score < veteran_score,
            "rookie_score={rookie_score} should be lower than veteran_score={veteran_score}"
        );
    }

    #[test]
    fn test_smoothness_reduces_tire_degradation() {
        let ctx = sample_context(45, WeatherCondition::Dry);
        let mut smooth = build_driver("SMOOTH", 75.0, 72.0, 70.0, 74.0, 10.0);
        smooth.smoothness = 92;

        let mut rough = smooth.clone();
        rough.id = "ROUGH".to_string();
        rough.nome = "Driver ROUGH".to_string();
        rough.smoothness = 18;

        let mut smooth_state = RaceState {
            driver_id: smooth.id.clone(),
            tire_wear: 1.0,
            physical_condition: 1.0,
            cumulative_score: 0.0,
            is_dnf: false,
            current_position: 1,
            incidents: Vec::new(),
            dnf_reason: None,
            dnf_segment: None,
            pending_damage: Vec::new(),
        };
        let mut rough_state = smooth_state.clone();
        rough_state.driver_id = rough.id.clone();

        apply_tire_degradation(&mut smooth_state, &smooth, &ctx);
        apply_tire_degradation(&mut rough_state, &rough, &ctx);

        assert!(
            smooth_state.tire_wear > rough_state.tire_wear,
            "smooth tire_wear={} should be greater than rough tire_wear={}",
            smooth_state.tire_wear,
            rough_state.tire_wear
        );
    }

    #[test]
    fn test_incidents_can_generate_dnfs_when_enabled() {
        let mut risky = build_driver("RISK", 65.0, 30.0, 50.0, 55.0, 5.0);
        risky.consistencia = 20;
        risky.aggression = 95;
        risky.experiencia = 10;
        risky.car_reliability = 20.0;

        let grid = vec![risky.clone()];
        let mut found_dnf = false;

        for seed in 0..300 {
            let mut rng = StdRng::seed_from_u64(seed);
            let ctx = sample_context_with_incidents(50, WeatherCondition::HeavyRain);
            let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
            let result = simulate_race(
                &grid,
                &qualifying,
                &ctx,
                &IncidentCatalog::empty(),
                false,
                &mut rng,
            );

            if result.race_results.iter().any(|driver| driver.is_dnf) {
                found_dnf = true;
                break;
            }
        }

        assert!(
            found_dnf,
            "expected at least one DNF with incidents enabled"
        );
    }

    #[test]
    fn test_race_result_tracks_total_incidents() {
        let mut risky = build_driver("RISK", 65.0, 30.0, 50.0, 55.0, 5.0);
        risky.consistencia = 25;
        risky.aggression = 90;
        risky.experiencia = 15;
        risky.car_reliability = 25.0;

        let grid = vec![risky];
        let mut rng = StdRng::seed_from_u64(999);
        let ctx = sample_context_with_incidents(45, WeatherCondition::Wet);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            &mut rng,
        );

        let sum: i32 = result.race_results.iter().map(|d| d.incidents_count).sum();
        assert_eq!(result.total_incidents, sum);
    }

    #[test]
    fn test_dnf_ordering_later_segment_ahead_of_earlier() {
        let driver_a = build_driver("A", 70.0, 70.0, 70.0, 70.0, 8.0);
        let driver_b = build_driver("B", 70.0, 70.0, 70.0, 70.0, 8.0);

        // Simular manualmente: A abandona no Late, B abandona no Early
        let state_a = RaceState {
            driver_id: "A".to_string(),
            tire_wear: 0.6,
            physical_condition: 0.8,
            cumulative_score: 200.0,
            is_dnf: true,
            current_position: 1,
            incidents: Vec::new(),
            dnf_reason: Some("Engine".to_string()),
            dnf_segment: Some(RaceSegment::Late),
            pending_damage: Vec::new(),
        };
        let state_b = RaceState {
            driver_id: "B".to_string(),
            tire_wear: 0.9,
            physical_condition: 0.95,
            cumulative_score: 50.0,
            is_dnf: true,
            current_position: 2,
            incidents: Vec::new(),
            dnf_reason: Some("Crash".to_string()),
            dnf_segment: Some(RaceSegment::Early),
            pending_damage: Vec::new(),
        };

        // A (Late DNF) deve ter laps_completed > B (Early DNF)
        let laps_a = estimate_laps_at_dnf(state_a.dnf_segment, 20);
        let laps_b = estimate_laps_at_dnf(state_b.dnf_segment, 20);
        assert!(
            laps_a > laps_b,
            "Late DNF laps={laps_a} should > Early DNF laps={laps_b}"
        );

        // Na ordenação de DNFs, A (Late) deve vir antes de B (Early)
        let mut dnfs = vec![&state_a, &state_b];
        dnfs.sort_by(|a, b| {
            let seg_ord_b = b.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
            let seg_ord_a = a.dnf_segment.map(|s| s.ordinal()).unwrap_or(0);
            seg_ord_b.cmp(&seg_ord_a).then_with(|| {
                b.cumulative_score
                    .partial_cmp(&a.cumulative_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        assert_eq!(
            dnfs[0].driver_id, "A",
            "Late DNF driver should rank ahead of Early DNF"
        );

        let _ = (driver_a, driver_b); // suppress unused warnings
    }

    #[test]
    fn test_dnf_gap_never_negative() {
        let mut risky = build_driver("RISK", 50.0, 30.0, 50.0, 50.0, 5.0);
        risky.consistencia = 15;
        risky.aggression = 95;
        risky.car_reliability = 15.0;

        let grid: Vec<SimDriver> = std::iter::once(risky)
            .chain((0..11).map(|i| build_driver(&format!("R{i}"), 70.0, 70.0, 70.0, 70.0, 8.0)))
            .collect();

        for seed in 0..50 {
            let mut rng = StdRng::seed_from_u64(seed);
            let ctx = sample_context_with_incidents(40, WeatherCondition::Wet);
            let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
            let result = simulate_race(
                &grid,
                &qualifying,
                &ctx,
                &IncidentCatalog::empty(),
                false,
                &mut rng,
            );

            for r in &result.race_results {
                assert!(
                    r.gap_to_winner_ms >= 0.0,
                    "gap_to_winner_ms={} must be >= 0 for driver {}",
                    r.gap_to_winner_ms,
                    r.pilot_id
                );
            }
        }
    }

    #[test]
    fn test_dnf_laps_completed_coherence() {
        let laps_start = estimate_laps_at_dnf(Some(RaceSegment::Start), 30);
        let laps_early = estimate_laps_at_dnf(Some(RaceSegment::Early), 30);
        let laps_mid = estimate_laps_at_dnf(Some(RaceSegment::Mid), 30);
        let laps_late = estimate_laps_at_dnf(Some(RaceSegment::Late), 30);
        let laps_finish = estimate_laps_at_dnf(Some(RaceSegment::Finish), 30);

        assert!(laps_start < laps_early);
        assert!(laps_early < laps_mid);
        assert!(laps_mid < laps_late);
        assert!(laps_late < laps_finish);
        assert!(laps_finish < 30);
    }

    #[test]
    fn test_endurance_more_tire_degradation_than_sprint() {
        use crate::simulation::profile::resolve_simulation_profile;

        let endurance_profile =
            resolve_simulation_profile("endurance", 288, 25.0, WeatherCondition::Dry, 0, 10);
        let gt4_profile =
            resolve_simulation_profile("gt4", 47, 25.0, WeatherCondition::Dry, 30, 12);

        assert!(
            endurance_profile.tire_degradation_rate > gt4_profile.tire_degradation_rate,
            "endurance tire_degr={} should > gt4={}",
            endurance_profile.tire_degradation_rate,
            gt4_profile.tire_degradation_rate
        );
    }
}
