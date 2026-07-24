use crate::simulation::track_profile::TrackCharacter;

/// Perfil de simulação canônico para uma corrida.
/// Centraliza todos os multiplicadores e parâmetros de tuning.
/// Resolvido uma vez por corrida e injetado no SimulationContext.
#[derive(Debug, Clone)]
pub struct SimulationProfile {
    /// Tempo base de volta em ms (do pole sitter ideal).
    pub base_lap_time_ms: f64,
    /// Taxa de desgaste de pneu por segmento.
    pub tire_degradation_rate: f64,
    /// Taxa de desgaste físico por segmento.
    pub physical_degradation_rate: f64,
    /// Multiplicador global de taxa de incidentes.
    pub incident_rate_multiplier: f64,
    /// Escala da variância no qualifying.
    pub qualifying_variance_multiplier: f64,
    /// Escala da variância no score de corrida.
    pub race_variance_multiplier: f64,
    /// Amplifica (>1.0) ou atenua (<1.0) o efeito da chuva.
    pub rain_sensitivity: f64,
    /// Amplifica caos adicional na largada (colisões/erros no Start).
    pub start_chaos_multiplier: f64,
    /// Dificuldade da pista (>1.0 = mais exigente, adaptabilidade vale mais).
    pub track_difficulty_multiplier: f64,
    /// Dificuldade de ultrapassagem (>1.0 = mais difícil).
    pub overtaking_difficulty_multiplier: f64,
    /// Spread de pace entre pilotos (>1.0 = mais separação).
    pub race_pace_spread_multiplier: f64,
    /// Caráter esportivo da pista (determina pesos de atributos).
    pub track_character: TrackCharacter,
}
