//! As tabelas de risco: chances-base, o quanto cada segmento e cada condição de pista pesam,
//! e os fatores derivados (risco de lesão e importância narrativa) que carimbam o incidente.

use crate::models::enums::WeatherCondition;

use crate::simulation::race::RaceSegment;

use super::tipos::{IncidentResult, IncidentSeverity, IncidentType};

pub(crate) const MECHANICAL_BASE_CHANCE: f64 = 0.015;
pub(crate) const DRIVER_ERROR_BASE_CHANCE: f64 = 0.017;
pub(crate) const COLLISION_BASE_CHANCE: f64 = 0.006;
pub(crate) const SEGMENTS: f64 = 5.0;

pub(crate) fn mechanical_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 0.5,
        RaceSegment::Early => 0.8,
        RaceSegment::Mid => 1.0,
        RaceSegment::Late => 1.2,
        RaceSegment::Finish => 1.5,
    }
}

pub(crate) fn driver_error_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 1.5,
        RaceSegment::Early => 1.0,
        RaceSegment::Mid => 1.0,
        RaceSegment::Late => 1.2,
        RaceSegment::Finish => 1.5,
    }
}

pub(crate) fn collision_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 2.5,
        RaceSegment::Early => 1.0,
        RaceSegment::Mid => 0.8,
        RaceSegment::Late => 0.8,
        RaceSegment::Finish => 1.2,
    }
}

pub(crate) fn rain_base(weather: WeatherCondition) -> f64 {
    match weather {
        WeatherCondition::Dry => 0.0,
        WeatherCondition::Damp => 0.30,
        WeatherCondition::Wet => 0.60,
        WeatherCondition::HeavyRain => 1.00,
    }
}

pub(crate) fn rain_collision_mult(weather: WeatherCondition) -> f64 {
    match weather {
        WeatherCondition::Dry => 1.0,
        WeatherCondition::Damp => 1.2,
        WeatherCondition::Wet => 1.4,
        WeatherCondition::HeavyRain => 1.6,
    }
}

pub(crate) fn compute_irm(incident_type: IncidentType, severity: IncidentSeverity) -> f64 {
    match (incident_type, severity) {
        (IncidentType::Collision, IncidentSeverity::Critical) => 1.5,
        (IncidentType::Collision, IncidentSeverity::Major) => 0.45,
        (IncidentType::DriverError, IncidentSeverity::Critical) => 1.0,
        (IncidentType::Mechanical, IncidentSeverity::Critical) => 0.6,
        _ => 0.0,
    }
}

pub(crate) fn injury_base_chance(incident_type: IncidentType) -> f64 {
    match incident_type {
        IncidentType::Collision => 0.50,
        IncidentType::DriverError => 0.40,
        IncidentType::Mechanical => 0.25,
    }
}

pub(crate) fn compute_narrative_hint(
    severity: IncidentSeverity,
    incident_type: IncidentType,
) -> u8 {
    match (severity, incident_type) {
        (IncidentSeverity::Critical, _) => 2,
        (IncidentSeverity::Major, IncidentType::Collision) => 1,
        _ => 0,
    }
}

pub(crate) fn make_incident(
    pilot_id: String,
    incident_type: IncidentType,
    severity: IncidentSeverity,
    segment: &str,
    positions_lost: i32,
    is_dnf: bool,
    description: String,
    linked_pilot_id: Option<String>,
    is_two_car_incident: bool,
    catalog_id: Option<String>,
    damage_origin_segment: Option<String>,
) -> IncidentResult {
    IncidentResult {
        injury_risk_multiplier: compute_irm(incident_type, severity),
        narrative_importance_hint: compute_narrative_hint(severity, incident_type),
        pilot_id,
        incident_type,
        severity,
        segment: segment.to_string(),
        positions_lost,
        is_dnf,
        description,
        linked_pilot_id,
        is_two_car_incident,
        catalog_id,
        damage_origin_segment,
    }
}
