//! Tipos do incidente: a natureza, a gravidade, o registro que sai do segmento e o dano
//! latente que fica pendurado no carro depois de uma batida.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum IncidentType {
    Mechanical,
    DriverError,
    Collision,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum IncidentSeverity {
    Minor,
    Major,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentResult {
    pub pilot_id: String,
    pub incident_type: IncidentType,
    pub severity: IncidentSeverity,
    pub segment: String,
    pub positions_lost: i32,
    pub is_dnf: bool,
    pub description: String,
    #[serde(default)]
    pub linked_pilot_id: Option<String>,
    pub is_two_car_incident: bool,
    pub injury_risk_multiplier: f64,
    pub narrative_importance_hint: u8,
    /// ID da entry do catálogo de incidentes. None para incidentes sem catálogo (catálogo vazio
    /// ou versões anteriores do motor).
    #[serde(default)]
    pub catalog_id: Option<String>,
    /// Segmento onde o dano se originou (para dano pós-colisão latente).
    /// Difere de `segment` quando o dano foi causado por colisão anterior.
    #[serde(default)]
    pub damage_origin_segment: Option<String>,
}

/// Dano pós-colisão com possibilidade de manifestação latente em segmentos futuros.
#[derive(Debug, Clone)]
pub struct PendingDamage {
    /// ID da entry do catálogo (PostCollision).
    pub catalog_id: String,
    /// Segmento onde a colisão originou o dano.
    pub origin_segment: String,
    /// Chance de manifestação neste segmento; aumenta +0.15 por segmento sem manifestação.
    pub manifest_chance: f64,
    /// true se a colisão original era Major (dano pode causar DNF).
    pub is_dnf_capable: bool,
}

/// Retorno de `process_segment_incidents`, carregando incidentes do segmento e novos danos latentes.
pub struct SegmentIncidentResult {
    pub incidents: Vec<IncidentResult>,
    /// Pares (driver_id, PendingDamage) a serem adicionados aos estados correspondentes.
    pub new_pending_damage: Vec<(String, PendingDamage)>,
}
