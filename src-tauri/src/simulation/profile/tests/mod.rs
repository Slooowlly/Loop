use crate::models::enums::WeatherCondition;
use crate::simulation::profile::{resolve_simulation_profile, SimulationProfile};

fn profile_for(cat: &str) -> SimulationProfile {
    resolve_simulation_profile(cat, 586, 22.0, WeatherCondition::Dry, 30, 12)
}

#[test]
fn test_rookie_has_more_variance_than_gt3() {
    let rookie = profile_for("mazda_rookie");
    let gt3 = profile_for("gt3");
    assert!(
        rookie.qualifying_variance_multiplier > gt3.qualifying_variance_multiplier,
        "rookie qual_var={} should > gt3={}",
        rookie.qualifying_variance_multiplier,
        gt3.qualifying_variance_multiplier
    );
    assert!(
        rookie.race_variance_multiplier > gt3.race_variance_multiplier,
        "rookie race_var={} should > gt3={}",
        rookie.race_variance_multiplier,
        gt3.race_variance_multiplier
    );
}

#[test]
fn test_endurance_has_higher_tire_degradation_than_gt4() {
    let endurance = profile_for("endurance");
    let gt4 = profile_for("gt4");
    assert!(
        endurance.tire_degradation_rate > gt4.tire_degradation_rate,
        "endurance tire={} should > gt4={}",
        endurance.tire_degradation_rate,
        gt4.tire_degradation_rate
    );
    assert!(
        endurance.physical_degradation_rate > gt4.physical_degradation_rate,
        "endurance phys={} should > gt4={}",
        endurance.physical_degradation_rate,
        gt4.physical_degradation_rate
    );
}

#[test]
fn test_known_track_returns_explicit_lap_time() {
    // Laguna Seca (586) para GT4 deve retornar 77_000ms da tabela
    let profile = resolve_simulation_profile("gt4", 586, 22.0, WeatherCondition::Dry, 30, 12);
    assert_eq!(profile.base_lap_time_ms, 77_000.0);
}

#[test]
fn test_unknown_track_falls_back_to_length_based() {
    // track_id 9999 não existe na tabela nem em tracks.rs → usa 90_000 hardcoded
    let profile = resolve_simulation_profile("gt4", 9999, 22.0, WeatherCondition::Dry, 30, 12);
    assert!(profile.base_lap_time_ms > 0.0);
}

#[test]
fn test_rain_increases_incident_multiplier() {
    let dry = resolve_simulation_profile("gt4", 47, 22.0, WeatherCondition::Dry, 30, 12);
    let rain = resolve_simulation_profile("gt4", 47, 22.0, WeatherCondition::HeavyRain, 30, 12);
    assert!(
        rain.incident_rate_multiplier > dry.incident_rate_multiplier,
        "rain irm={} should > dry={}",
        rain.incident_rate_multiplier,
        dry.incident_rate_multiplier
    );
    assert!(
        rain.rain_sensitivity > dry.rain_sensitivity,
        "rain sensitivity={} should > dry={}",
        rain.rain_sensitivity,
        dry.rain_sensitivity
    );
}

#[test]
fn test_high_temp_increases_tire_degradation() {
    let normal = resolve_simulation_profile("gt4", 47, 22.0, WeatherCondition::Dry, 30, 12);
    let hot = resolve_simulation_profile("gt4", 47, 38.0, WeatherCondition::Dry, 30, 12);
    assert!(
        hot.tire_degradation_rate > normal.tire_degradation_rate,
        "hot tire_degr={} should > normal={}",
        hot.tire_degradation_rate,
        normal.tire_degradation_rate
    );
}

#[test]
fn test_unknown_category_returns_neutral_default_like_values() {
    let profile = resolve_simulation_profile(
        "categoria_inexistente",
        47,
        22.0,
        WeatherCondition::Dry,
        30,
        12,
    );
    // Deve retornar algo válido (não pânico, não zeros)
    assert!(profile.base_lap_time_ms > 0.0);
    assert!(profile.tire_degradation_rate > 0.0);
    assert!(profile.incident_rate_multiplier > 0.0);
}

#[test]
fn test_nordschleife_has_high_difficulty() {
    let profile = resolve_simulation_profile("gt3", 249, 22.0, WeatherCondition::Dry, 60, 5);
    assert!(
        profile.track_difficulty_multiplier >= 1.5,
        "Nordschleife should have difficulty >= 1.5, got {}",
        profile.track_difficulty_multiplier
    );
}

#[test]
fn test_roval_has_lower_overtaking_difficulty() {
    let roval = resolve_simulation_profile("gt4", 554, 22.0, WeatherCondition::Dry, 30, 12); // Charlotte Roval
    let road = resolve_simulation_profile("gt4", 212, 22.0, WeatherCondition::Dry, 30, 12); // Interlagos (Technical)
    assert!(
        roval.overtaking_difficulty_multiplier < road.overtaking_difficulty_multiplier,
        "roval={} should < road={}",
        roval.overtaking_difficulty_multiplier,
        road.overtaking_difficulty_multiplier
    );
}

#[test]
fn test_sebring_has_higher_tire_stress_than_tsukuba() {
    let sebring = resolve_simulation_profile("gt4", 95, 22.0, WeatherCondition::Dry, 30, 12);
    let tsukuba = resolve_simulation_profile("gt4", 324, 22.0, WeatherCondition::Dry, 30, 12);
    assert!(
        sebring.tire_degradation_rate > tsukuba.tire_degradation_rate,
        "Sebring tire={} should > Tsukuba={}",
        sebring.tire_degradation_rate,
        tsukuba.tire_degradation_rate
    );
}

#[test]
fn test_le_mans_has_higher_physical_stress_than_lime_rock() {
    let le_mans = resolve_simulation_profile("gt4", 268, 22.0, WeatherCondition::Dry, 30, 12);
    let lime_rock = resolve_simulation_profile("gt4", 353, 22.0, WeatherCondition::Dry, 30, 12);
    assert!(
        le_mans.physical_degradation_rate > lime_rock.physical_degradation_rate,
        "Le Mans phys={} should > Lime Rock={}",
        le_mans.physical_degradation_rate,
        lime_rock.physical_degradation_rate
    );
}

#[test]
fn test_tight_track_has_higher_overtaking_diff_than_flowing() {
    let hungaroring = resolve_simulation_profile("gt4", 413, 22.0, WeatherCondition::Dry, 30, 12); // Tight
    let spa = resolve_simulation_profile("gt4", 523, 22.0, WeatherCondition::Dry, 30, 12); // Flowing
    assert!(
        hungaroring.overtaking_difficulty_multiplier > spa.overtaking_difficulty_multiplier,
        "Tight={} should > Flowing={}",
        hungaroring.overtaking_difficulty_multiplier,
        spa.overtaking_difficulty_multiplier
    );
}

#[test]
fn test_gt3_has_lower_incident_rate_than_rookie() {
    let rookie = profile_for("mazda_rookie");
    let gt3 = profile_for("gt3");
    assert!(
        gt3.incident_rate_multiplier < rookie.incident_rate_multiplier,
        "gt3={} should < rookie={}",
        gt3.incident_rate_multiplier,
        rookie.incident_rate_multiplier
    );
}

/// **Guard cruzado das tabelas indexadas por `track_id`.**
///
/// Três tabelas paralelas descrevem a mesma pista: o catálogo canônico
/// (`constants::tracks`), o tempo base por família de carro
/// (`profile::lap_times::base_lap_time_ms_for`) e a identidade esportiva
/// (`track_profile::get_track_simulation_data`). Adicionar pista exige editar as três, e
/// esquecer não dá erro nenhum: o tempo base cai num fallback por `comprimento_km` e a
/// identidade cai num perfil neutro. O esquecimento vira uma pista que corre com números
/// plausíveis e errados.
///
/// Este teste transforma esse silêncio em falha. Ele não exige cobertura total: exige que a
/// lista do que falta seja EXATAMENTE a que está escrita aqui, então incluir pista nova sem
/// os tempos quebra o teste, e preencher os tempos que faltam também quebra — nas duas
/// direções alguém precisa vir aqui e dizer o que fez.
/// As pistas que hoje correm com o tempo base do FALLBACK por comprimento, sem entrada
/// própria na tabela de `lap_times`. São 4 de 107, e a lista saiu desta medição, não de uma
/// decisão: 167 (Okayama Short), 346 (Barcelona National), 451 (Rudskogen) e 489 (Lédenon).
/// Duas delas são variantes CURTAS de traçado que a tabela cobre na versão completa, que é
/// justamente o caso em que o fallback por quilometragem erra menos — e nenhuma foi
/// preenchida aqui porque inventar seis tempos de volta por pista é medição, não digitação.
const ESPERADO_SEM_TEMPO_BASE: &[u32] = &[167, 346, 451, 489];
/// ...e as que correm com a identidade esportiva neutra.
const ESPERADO_SEM_IDENTIDADE: &[u32] = &[];

#[test]
fn as_tres_tabelas_por_track_id_nao_divergem_em_silencio() {
    use crate::constants::tracks::get_all_tracks;
    use crate::simulation::profile::lap_times::base_lap_time_ms_for;
    use crate::simulation::track_profile::{get_track_simulation_data, TrackCharacter};

    // As famílias de carro que `profile::base::car_family_for` sabe produzir.
    const FAMILIAS: [&str; 6] = ["mx5", "gr86", "bmw_m2", "gt4", "gt3", "lmp2"];

    let mut sem_tempo_base: Vec<u32> = Vec::new();
    let mut sem_identidade: Vec<u32> = Vec::new();
    let mut parcial: Vec<(u32, usize)> = Vec::new();

    for track in get_all_tracks() {
        let cobertas = FAMILIAS
            .iter()
            .filter(|f| base_lap_time_ms_for(f, track.track_id).is_some())
            .count();
        match cobertas {
            0 => sem_tempo_base.push(track.track_id),
            n if n < FAMILIAS.len() => parcial.push((track.track_id, n)),
            _ => {}
        }

        // O perfil neutro é o braço `_` do match: se a pista devolve exatamente ele, ou ela
        // não está na tabela, ou está com os valores do fallback, que dá no mesmo.
        let sim = get_track_simulation_data(track.track_id);
        let e_neutro = sim.track_character == TrackCharacter::Technical
            && sim.tire_stress_multiplier == 1.00
            && sim.physical_stress_multiplier == 1.00
            && sim.acceleration_weight == 35.0
            && sim.power_weight == 30.0
            && sim.handling_weight == 35.0;
        if e_neutro {
            sem_identidade.push(track.track_id);
        }
    }

    sem_tempo_base.sort_unstable();
    sem_identidade.sort_unstable();

    assert!(
        parcial.is_empty(),
        "pista com tempo base para ALGUMAS famílias e não para outras — é sempre erro de \
         digitação, a tabela é preenchida por bloco de pista: {parcial:?}"
    );
    assert_eq!(
        sem_tempo_base, ESPERADO_SEM_TEMPO_BASE,
        "mudou a lista de pistas sem tempo base na tabela de `lap_times`. Elas correm com o \
         fallback por comprimento_km, que erra o tempo de volta por categoria. Se você \
         preencheu os tempos, tire o id daqui; se incluiu pista nova, ou preenche os seis \
         tempos ou assume o fallback acrescentando o id."
    );
    assert_eq!(
        sem_identidade, ESPERADO_SEM_IDENTIDADE,
        "mudou a lista de pistas sem identidade esportiva própria em `track_profile`. Elas \
         correm como Technical neutro: sem stress de pneu, sem caráter, com pesos médios."
    );
}
