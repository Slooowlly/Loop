#![allow(dead_code)]

use crate::calendar::CalendarEntry;
use crate::constants::tracks::get_track;
use crate::finance::morale::{morale_pace_delta, morale_reliability_delta};
use crate::models::driver::Driver;
use crate::models::enums::WeatherCondition;
use crate::models::team::Team;

use super::car_build::{
    effective_car_performance_from_shape, quali_car_performance_from_shape, vies_de_pico,
};
use super::catalog::{vehicle_class_from_category, VehicleClass};
use super::profile::resolve_simulation_profile;
use super::track_profile::{get_track_simulation_data, pack_density_factor, TrackCharacter};
use crate::car::sim_bridge::{car_performance_from, car_shape_weights};

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
    /// Motivação atual (0–100, recalculada no fim de temporada). Um desmotivado corre
    /// abaixo do próprio skill (ver `simulation::pressure::motivation_pace_delta`).
    pub motivacao: f64,
    /// Trim de CORRIDA do carro: o ritmo sustentado, de tanque cheio. É o que
    /// `race/**` lê, e continua sendo exatamente o que sempre foi.
    pub car_performance: f64,
    /// Trim de CLASSIFICAÇÃO do mesmo carro: a volta de tanque vazio. Derivado do
    /// `car_shape` (apoio crava a volta, ponta rende no stint) em
    /// [`crate::simulation::car_build::quali_car_performance_from_shape`], NÃO
    /// sorteado. Consumido **só** por `simulation::qualifying` — um carro é rápido
    /// no sábado ou no domingo, e agora isso é uma escolha de projeto, não o mesmo
    /// número duas vezes. Sem carro semeado no save, cai de volta no trim de corrida.
    pub car_performance_quali: f64,
    /// **Viés de pico do shape do carro**, em [-1, 1]: positivo = carro de apoio (handling),
    /// que crava a volta e vive de aerodinâmica; negativo = carro de ponta/eficiência
    /// (power). É o MESMO eixo que `car_build::vies_de_pico` já derivava para o trim de
    /// classificação — aqui ele serve ao ar sujo, porque quanto mais o carro depende de
    /// apoio, mais ele sofre atrás de outro. Reaproveitado de propósito, em vez de um eixo
    /// novo. Sem carro semeado no save, 0.0 (neutro).
    pub vies_de_pico: f64,
    /// **Qualidade da chamada de estratégia da equipe** (0–100). É por aqui que a equipe
    /// finalmente vira um ator DENTRO da corrida: equipe boa acerta a volta da parada, equipe
    /// ruim erra para um dos lados da janela. Sai do `pit_crew_quality` do time, que até agora
    /// só pesava na quebra de peça. Ver `race::estrategia::planejar_paradas`.
    pub qualidade_de_estrategia: f64,
    pub car_reliability: f64,
    pub team_id: String,
    pub team_name: String,
    pub corridas_na_categoria: i32,
    /// Multiplicador da taxa de erro por pressão de campeonato (1.0 = neutro,
    /// <1 clutch, >1 choke). Calculado no setup da corrida (ver simulation::pressure).
    pub pressure_error_mult: f64,
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
            motivacao: driver.motivacao,
            // Moral viva (ideia 3): efeito sutil e SIMÉTRICO — um time em alta corre
            // um tico melhor e mais confiável; em crise, pior e mais frágil. Vale
            // para todo carro do grid (jogador simulado + IA).
            car_performance: team.car_performance + morale_pace_delta(team.morale),
            // Sem a pista e sem o carro semeado não há shape pra ler: os dois trims são o
            // mesmo número. Quem separa é `from_driver_team_and_track`.
            car_performance_quali: team.car_performance + morale_pace_delta(team.morale),
            // Sem shape lido, o carro é neutro no eixo apoio × ponta.
            vies_de_pico: 0.0,
            qualidade_de_estrategia: team.pit_crew_quality,
            car_reliability: (team.confiabilidade + morale_reliability_delta(team.morale))
                .clamp(0.0, 100.0),
            team_id: team.id.clone(),
            team_name: team.nome.clone(),
            corridas_na_categoria: driver.corridas_na_categoria as i32,
            pressure_error_mult: 1.0,
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
        // Sistema de Nível do Carro: a magnitude do carro vira o car_performance-base e o
        // shape (contínuo) casa com a pista. Save antigo sem carro semeado → fallback ao
        // escalar legado. Re-aplicamos o delta de moral aqui (a confiabilidade já veio do
        // construtor base com a moral e não é reescrita).
        //
        // Os DOIS trims saem daqui, do mesmo carro e do mesmo shape: o de corrida (ritmo
        // sustentado) e o de classificação (volta única). Ver `car_build`.
        let (base_car_performance, base_car_quali, vies) = match &team.car {
            Some(car) => {
                let magnitude = car_performance_from(car);
                let shape = car_shape_weights(car);
                (
                    effective_car_performance_from_shape(magnitude, shape, track_weights),
                    quali_car_performance_from_shape(magnitude, shape, track_weights),
                    vies_de_pico(shape),
                )
            }
            // Save antigo sem carro semeado: sem shape não há como separar os trims nem
            // saber o quanto o carro vive de apoio.
            None => (team.car_performance, team.car_performance, 0.0),
        };
        let moral = morale_pace_delta(team.morale);
        sim_driver.car_performance = base_car_performance + moral;
        sim_driver.car_performance_quali = base_car_quali + moral;
        sim_driver.vies_de_pico = vies;
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
            season_week: None,
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
            season_week: None,
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

    /// Carro com shape de potência (motor/câmbio/cooling/eletrônica no talo).
    fn power_car() -> crate::car::Car {
        let mut car = crate::car::Car::uniform(5);
        for part in [
            crate::car::PartType::Engine,
            crate::car::PartType::Gearbox,
            crate::car::PartType::Cooling,
            crate::car::PartType::Electronics,
        ] {
            car.set_level(part, 10);
        }
        car
    }

    #[test]
    fn test_carro_de_potencia_rende_mais_na_pista_de_power() {
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
        team.car = Some(power_car());

        // Monza (239) = power-heavy → shape casa; Tsukuba (324) = accel → shape erra.
        let monza = SimDriver::from_driver_team_and_track(&driver, &team, 239);
        let tsukuba = SimDriver::from_driver_team_and_track(&driver, &team, 324);

        assert!(
            monza.car_performance > tsukuba.car_performance,
            "carro de potência deveria render mais em power (monza={}, tsukuba={})",
            monza.car_performance,
            tsukuba.car_performance
        );
    }

    #[test]
    fn test_sem_carro_semeado_cai_no_fallback_do_escalar() {
        let driver = Driver::create_player(
            "P003".to_string(),
            "Carlos Legacy".to_string(),
            "Brasileiro".to_string(),
            20,
        );
        let mut team = placeholder_team_from_db(
            "T003".to_string(),
            "Team Legacy".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.car_performance = 8.0;
        team.car = None; // save antigo, sem carro

        let sim_driver = SimDriver::from_driver_team_and_track(&driver, &team, 239);

        // Sem carro: usa o escalar (8.0) + moral, sem delta de shape.
        assert!((sim_driver.car_performance - 8.0).abs() < 3.0);
    }
}
