#![allow(dead_code)]

use crate::calendar::CalendarEntry;
use crate::constants::tracks::get_track;
use crate::models::driver::Driver;
use crate::models::enums::WeatherCondition;
use crate::models::team::Team;

use super::car_build::effective_car_performance;
use super::catalog::{vehicle_class_from_category, VehicleClass};
use super::profile::resolve_simulation_profile;
use super::track_profile::{get_track_simulation_data, pack_density_factor, TrackCharacter};

#[derive(Debug, Clone)]
pub struct SimulationContext {
    pub category_id: String,
    pub category_tier: u8,
    pub track_id: u32,
    pub track_name: String,
    pub weather: WeatherCondition,
    pub temperature: f64,
    pub total_laps: i32,
    pub race_duration_minutes: i32,
    pub is_championship_deciding: bool,
    pub incidents_enabled: bool,

    // --- Campos resolvidos pelo SimulationProfile ---
    pub base_lap_time_ms: f64,
    pub tire_degradation_rate: f64,
    pub physical_degradation_rate: f64,
    pub incident_rate_multiplier: f64,
    pub qualifying_variance_multiplier: f64,
    pub race_variance_multiplier: f64,
    pub rain_sensitivity: f64,
    pub start_chaos_multiplier: f64,
    pub track_difficulty_multiplier: f64,
    pub overtaking_difficulty_multiplier: f64,
    pub race_pace_spread_multiplier: f64,
    /// Caráter esportivo da pista (determina pesos de atributos).
    pub track_character: TrackCharacter,
    /// Densidade do pelotão pela extensão da pista (>1.0 = pack mais compacto).
    pub pack_density_factor: f64,
    /// Classe de veículo da categoria — determina flavor text do catálogo de incidentes.
    pub vehicle_class: VehicleClass,
}

impl SimulationContext {
    pub fn from_calendar_entry(
        entry: &CalendarEntry,
        category_tier: u8,
        is_championship_deciding: bool,
    ) -> Self {
        let profile = resolve_simulation_profile(
            &entry.categoria,
            entry.track_id,
            entry.temperatura,
            entry.clima,
            entry.duracao_corrida_min,
            entry.voltas,
        );

        Self {
            category_id: entry.categoria.clone(),
            category_tier,
            track_id: entry.track_id,
            track_name: entry.track_name.clone(),
            weather: entry.clima,
            temperature: entry.temperatura,
            total_laps: entry.voltas,
            race_duration_minutes: entry.duracao_corrida_min,
            is_championship_deciding,
            incidents_enabled: true,
            base_lap_time_ms: profile.base_lap_time_ms,
            tire_degradation_rate: profile.tire_degradation_rate,
            physical_degradation_rate: profile.physical_degradation_rate,
            incident_rate_multiplier: profile.incident_rate_multiplier,
            qualifying_variance_multiplier: profile.qualifying_variance_multiplier,
            race_variance_multiplier: profile.race_variance_multiplier,
            rain_sensitivity: profile.rain_sensitivity,
            start_chaos_multiplier: profile.start_chaos_multiplier,
            track_difficulty_multiplier: profile.track_difficulty_multiplier,
            overtaking_difficulty_multiplier: profile.overtaking_difficulty_multiplier,
            race_pace_spread_multiplier: profile.race_pace_spread_multiplier,
            track_character: profile.track_character,
            pack_density_factor: get_track(entry.track_id)
                .map(|t| pack_density_factor(t.comprimento_km))
                .unwrap_or(1.0),
            vehicle_class: vehicle_class_from_category(&entry.categoria),
        }
    }

    /// Helper para testes — fornece valores padrão neutros para campos adicionados.
    /// Use `..SimulationContext::test_default()` em struct literals de teste.
    #[cfg(test)]
    pub fn test_default() -> Self {
        Self {
            category_id: "gt4".to_string(),
            category_tier: 3,
            track_id: 47,
            track_name: "Laguna Seca".to_string(),
            weather: WeatherCondition::Dry,
            temperature: 22.0,
            total_laps: 12,
            race_duration_minutes: 30,
            is_championship_deciding: false,
            incidents_enabled: false,
            base_lap_time_ms: 77_000.0,
            tire_degradation_rate: 0.020,
            physical_degradation_rate: 0.010,
            incident_rate_multiplier: 1.0,
            qualifying_variance_multiplier: 1.0,
            race_variance_multiplier: 1.0,
            rain_sensitivity: 1.0,
            start_chaos_multiplier: 1.0,
            track_difficulty_multiplier: 1.0,
            overtaking_difficulty_multiplier: 1.0,
            race_pace_spread_multiplier: 1.0,
            track_character: TrackCharacter::Technical,
            pack_density_factor: 1.0,
            vehicle_class: VehicleClass::StreetBased,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimDriver {
    pub id: String,
    pub nome: String,
    pub is_jogador: bool,
    pub skill: u8,
    pub consistencia: u8,
    pub racecraft: u8,
    pub defesa: u8,
    pub ritmo_classificacao: u8,
    pub gestao_pneus: u8,
    pub habilidade_largada: u8,
    pub adaptabilidade: u8,
    pub fator_chuva: u8,
    pub fitness: u8,
    pub experiencia: u8,
    pub aggression: u8,
    pub smoothness: u8,
    pub mentalidade: u8,
    pub confianca: u8,
    pub car_performance: f64,
    pub car_reliability: f64,
    pub team_id: String,
    pub team_name: String,
    pub corridas_na_categoria: i32,
}

impl SimDriver {
    pub fn from_driver_and_team(driver: &Driver, team: &Team) -> Self {
        Self {
            id: driver.id.clone(),
            nome: driver.nome.clone(),
            is_jogador: driver.is_jogador,
            skill: as_u8(driver.atributos.skill),
            consistencia: as_u8(driver.atributos.consistencia),
            racecraft: as_u8(driver.atributos.racecraft),
            defesa: as_u8(driver.atributos.defesa),
            ritmo_classificacao: as_u8(driver.atributos.ritmo_classificacao),
            gestao_pneus: as_u8(driver.atributos.gestao_pneus),
            habilidade_largada: as_u8(driver.atributos.habilidade_largada),
            adaptabilidade: as_u8(driver.atributos.adaptabilidade),
            fator_chuva: as_u8(driver.atributos.fator_chuva),
            fitness: as_u8(driver.atributos.fitness),
            experiencia: as_u8(driver.atributos.experiencia),
            aggression: as_u8(driver.atributos.aggression),
            smoothness: as_u8(driver.atributos.smoothness),
            mentalidade: as_u8(driver.atributos.mentalidade),
            confianca: as_u8(driver.atributos.confianca),
            car_performance: team.car_performance,
            car_reliability: team.confiabilidade,
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            corridas_na_categoria: driver.corridas_na_categoria as i32,
        }
    }

    pub fn from_driver_team_and_track(driver: &Driver, team: &Team, track_id: u32) -> Self {
        let track = get_track_simulation_data(track_id);
        let track_weights = (
            track.acceleration_weight,
            track.power_weight,
            track.handling_weight,
        );
        let mut sim_driver = Self::from_driver_and_team(driver, team);
        sim_driver.car_performance =
            effective_car_performance(team.car_performance, team.car_build_profile, track_weights);
        sim_driver
    }
}

fn as_u8(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use crate::calendar::CalendarEntry;
    use crate::models::driver::Driver;
    use crate::models::enums::{DriverStatus, RaceStatus, WeatherCondition};
    use crate::models::team::placeholder_team_from_db;

    use super::*;

    #[test]
    fn test_context_from_calendar_entry() {
        let entry = CalendarEntry {
            id: "R001".to_string(),
            season_id: "S001".to_string(),
            categoria: "gt3".to_string(),
            rodada: 1,
            nome: "Rodada 1 - Spa".to_string(),
            track_id: 100,
            track_name: "Spa".to_string(),
            track_config: "Full".to_string(),
            clima: WeatherCondition::Wet,
            temperatura: 18.5,
            voltas: 20,
            duracao_corrida_min: 45,
            duracao_classificacao_min: 15,
            status: RaceStatus::Pendente,
            horario: "14:00".to_string(),
            week_of_year: 5,
            season_phase: crate::models::enums::SeasonPhase::BlocoRegular,
            display_date: "2024-02-03".to_string(),
            thematic_slot: crate::models::enums::ThematicSlot::NaoClassificado,
        };

        let ctx = SimulationContext::from_calendar_entry(&entry, 4, true);

        assert_eq!(ctx.category_id, "gt3");
        assert_eq!(ctx.category_tier, 4);
        assert_eq!(ctx.track_name, "Spa");
        assert_eq!(ctx.weather, WeatherCondition::Wet);
        assert!(ctx.is_championship_deciding);
        assert!(ctx.incidents_enabled);
        // Deve ter profile resolvido (não hardcodes)
        assert!(ctx.base_lap_time_ms > 0.0);
        assert!(ctx.tire_degradation_rate > 0.0);
        // Chuva deve elevar rain_sensitivity
        assert!(ctx.rain_sensitivity > 1.0);
    }

    #[test]
    fn test_context_gt3_has_different_profile_than_rookie() {
        let make_entry = |cat: &str| CalendarEntry {
            id: "R001".to_string(),
            season_id: "S001".to_string(),
            categoria: cat.to_string(),
            rodada: 1,
            nome: "Rodada 1".to_string(),
            track_id: 47,
            track_name: "Laguna Seca".to_string(),
            track_config: "Full".to_string(),
            clima: WeatherCondition::Dry,
            temperatura: 22.0,
            voltas: 12,
            duracao_corrida_min: 30,
            duracao_classificacao_min: 15,
            status: RaceStatus::Pendente,
            horario: "14:00".to_string(),
            week_of_year: 5,
            season_phase: crate::models::enums::SeasonPhase::BlocoRegular,
            display_date: "2024-02-03".to_string(),
            thematic_slot: crate::models::enums::ThematicSlot::NaoClassificado,
        };

        let rookie_ctx =
            SimulationContext::from_calendar_entry(&make_entry("mazda_rookie"), 0, false);
        let gt3_ctx = SimulationContext::from_calendar_entry(&make_entry("gt3"), 4, false);

        assert!(rookie_ctx.qualifying_variance_multiplier > gt3_ctx.qualifying_variance_multiplier);
        assert!(rookie_ctx.incident_rate_multiplier > gt3_ctx.incident_rate_multiplier);
        assert!(rookie_ctx.start_chaos_multiplier > gt3_ctx.start_chaos_multiplier);
    }

    #[test]
    fn test_sim_driver_from_driver_and_team() {
        let mut driver = Driver::create_player(
            "P001".to_string(),
            "Joao Silva".to_string(),
            "🇧🇷 Brasileiro".to_string(),
            20,
        );
        driver.is_jogador = true;
        driver.status = DriverStatus::Ativo;
        driver.corridas_na_categoria = 7;
        driver.atributos.skill = 82.0;
        driver.atributos.gestao_pneus = 61.0;
        driver.atributos.ritmo_classificacao = 77.0;

        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Team Test".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 12.5;

        let sim_driver = SimDriver::from_driver_and_team(&driver, &team);

        assert_eq!(sim_driver.id, "P001");
        assert_eq!(sim_driver.team_id, "T001");
        assert_eq!(sim_driver.skill, 82);
        assert_eq!(sim_driver.gestao_pneus, 61);
        assert_eq!(sim_driver.car_reliability, team.confiabilidade);
        assert_eq!(sim_driver.corridas_na_categoria, 7);
    }

    #[test]
    fn test_sim_driver_uses_track_adjusted_car_performance_when_profile_matches() {
        let driver = Driver::create_player(
            "P002".to_string(),
            "Carlos Match".to_string(),
            "Brasileiro".to_string(),
            20,
        );
        let mut team = placeholder_team_from_db(
            "T002".to_string(),
            "Team Match".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 8.0;
        team.car_build_profile = crate::simulation::car_build::CarBuildProfile::PowerExtreme;

        let sim_driver = SimDriver::from_driver_team_and_track(&driver, &team, 93);

        assert!(sim_driver.car_performance > team.car_performance);
    }

    #[test]
    fn test_sim_driver_uses_track_adjusted_car_performance_when_profile_is_wrong() {
        let driver = Driver::create_player(
            "P003".to_string(),
            "Carlos Mismatch".to_string(),
            "Brasileiro".to_string(),
            20,
        );
        let mut team = placeholder_team_from_db(
            "T003".to_string(),
            "Team Mismatch".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 8.0;
        team.car_build_profile = crate::simulation::car_build::CarBuildProfile::PowerExtreme;

        let sim_driver = SimDriver::from_driver_team_and_track(&driver, &team, 325);

        assert!(sim_driver.car_performance < team.car_performance);
    }
}
