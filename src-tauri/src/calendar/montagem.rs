//! Montagem de uma etapa do calendário: horário, clima, duração e voltas
//! (extraído de `calendar/mod.rs`).

use rand::Rng;

use crate::constants::categories::CategoryConfig;
use crate::constants::tracks::{get_qualifying_duration, get_rain_chance, TrackInfo};
use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};

use super::entry::CalendarEntry;
use super::janela::display_date_for_category_round;

const SCHEDULE_HOURS: [&str; 5] = ["10:00", "12:00", "14:00", "16:00", "18:00"];

/// Horário exibido na etapa NOTURNA garantida (a hora real de largada exportada
/// pro iRacing vem de `weather::night_start_hour`, atrelada à estação).
const NIGHT_SCHEDULE_HOUR: &str = "21:00";

/// Pista com iluminação (Charlotte Roval) — preferida para a corrida noturna.
const LIT_TRACK_ID: u32 = 556;

/// Uma etapa é noturna quando começa às 20h ou depois (distingue do 18h diurno).
pub fn is_night_horario(horario: &str) -> bool {
    horario
        .split(':')
        .next()
        .and_then(|hh| hh.trim().parse::<i32>().ok())
        .is_some_and(|h| h >= 20)
}

/// Garante ao menos UMA corrida noturna na temporada, NUNCA a primeira nem a
/// última rodada (decisão do user). Preserva a regra "rookie (tier 0) nunca de
/// noite" — pula categorias rookie. Precisa de ≥3 rodadas (para sobrar um miolo).
/// Prefere a pista iluminada (Charlotte) se ela cair no miolo; senão, escolhe uma
/// rodada do miolo de forma determinística pelo `rng`.
pub(crate) fn ensure_night_race<R: Rng>(entries: &mut [CalendarEntry], tier: u8, rng: &mut R) {
    if tier == 0 || entries.len() < 3 {
        return;
    }
    let max_rodada = entries.iter().map(|e| e.rodada).max().unwrap_or(0);
    // Já existe uma noturna no miolo? Então não força outra.
    let has_night = entries
        .iter()
        .any(|e| e.rodada != 1 && e.rodada != max_rodada && is_night_horario(&e.horario));
    if has_night {
        return;
    }
    // Índices elegíveis (miolo): nem a 1ª nem a última rodada.
    let eligible: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.rodada != 1 && e.rodada != max_rodada)
        .map(|(i, _)| i)
        .collect();
    if eligible.is_empty() {
        return;
    }
    // Preferir Charlotte (iluminada) se estiver no miolo; senão, uma rodada aleatória.
    let chosen = eligible
        .iter()
        .copied()
        .find(|&i| entries[i].track_id == LIT_TRACK_ID)
        .unwrap_or_else(|| eligible[rng.gen_range(0..eligible.len())]);
    entries[chosen].horario = NIGHT_SCHEDULE_HOUR.to_string();
}

pub(crate) fn build_calendar_entry<R: Rng>(
    id: String,
    season_id: &str,
    season_year: i32,
    categoria: &str,
    rodada: i32,
    week_of_year: i32,
    season_phase: SeasonPhase,
    thematic_slot: ThematicSlot,
    track: &TrackInfo,
    config: &CategoryConfig,
    rng: &mut R,
) -> CalendarEntry {
    let clima = random_weather(track.track_id, rng);
    let temperatura = random_temperature(clima, rng);
    let duracao_corrida_min = resolve_race_duration(config, rng);
    let duracao_classificacao_min = get_qualifying_duration(track.track_id) as i32;
    let voltas = estimate_laps(track, duracao_corrida_min);
    let (track_name, track_config) = split_track_name(track.nome);

    CalendarEntry {
        id,
        season_id: season_id.to_string(),
        categoria: categoria.to_string(),
        rodada,
        nome: format!("Rodada {} - {}", rodada, track.nome_curto),
        track_id: track.track_id,
        track_name,
        track_config,
        clima,
        temperatura,
        voltas,
        duracao_corrida_min,
        duracao_classificacao_min,
        status: RaceStatus::Pendente,
        horario: SCHEDULE_HOURS[rng.gen_range(0..SCHEDULE_HOURS.len())].to_string(),
        week_of_year,
        season_phase,
        display_date: display_date_for_category_round(
            season_year,
            week_of_year,
            categoria,
            season_phase,
            rodada,
        ),
        thematic_slot,
        season_week: None,
    }
}

pub(crate) fn random_weather(rain_track_id: u32, rng: &mut impl Rng) -> WeatherCondition {
    let rain_chance = get_rain_chance(rain_track_id);
    if rng.gen::<f64>() >= rain_chance {
        return WeatherCondition::Dry;
    }

    let intensity = rng.gen::<f64>();
    if intensity < 0.40 {
        WeatherCondition::Damp
    } else if intensity < 0.80 {
        WeatherCondition::Wet
    } else {
        WeatherCondition::HeavyRain
    }
}

/// Placeholder de temperatura na criação do calendário — a temperatura DEFINITIVA
/// é derivada da história de chuva no export (`weather::story_temperature`) e
/// persistida por cima. Mantida na faixa observada no iRacing [18, 32] pra não
/// destoar antes do 1º export.
pub(crate) fn random_temperature(clima: WeatherCondition, rng: &mut impl Rng) -> f64 {
    let (min, max) = match clima {
        WeatherCondition::Dry => (24.0, 32.0),
        WeatherCondition::Damp => (20.0, 26.0),
        WeatherCondition::Wet => (18.0, 23.0),
        WeatherCondition::HeavyRain => (18.0, 21.0),
    };
    (rng.gen_range(min..=max) * 10.0_f64).round() / 10.0_f64
}

pub(crate) fn resolve_race_duration(config: &CategoryConfig, rng: &mut impl Rng) -> i32 {
    if config.duracao_corrida_min > 0 {
        config.duracao_corrida_min as i32
    } else {
        [120, 180, 240, 360][rng.gen_range(0..4)]
    }
}

pub(crate) fn estimate_laps(track: &TrackInfo, duracao_corrida_min: i32) -> i32 {
    let tempo_volta_estimado_min = track.comprimento_km / 2.0;
    ((duracao_corrida_min as f64 / tempo_volta_estimado_min).ceil() as i32).clamp(5, 50)
}

pub(crate) fn split_track_name(full_name: &str) -> (String, String) {
    if let Some((name, config)) = full_name.split_once(" - ") {
        (name.to_string(), config.to_string())
    } else {
        (full_name.to_string(), "Default".to_string())
    }
}
