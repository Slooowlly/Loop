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

/// Pista com iluminação (Charlotte Roval) - preferida para a corrida noturna.
/// O id vem do catálogo (`constants/tracks/dados.rs`).
use crate::constants::tracks::LIT_TRACK_ID;

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

/// Nome da etapa — o que vai GRAVADO na coluna `nome` da tabela `calendar`.
///
/// Sai do `rust-i18n` e não de um `format!("Rodada {}")` cru: um jogador em en-US
/// criava a temporada e o banco guardava "Rodada 3" para sempre. Prosa persistida nasce
/// no locale ATIVO da geração, que é a convenção do resto do motor (o motivo de
/// abandono faz igual — ver a nota das listas de palavra-chave em `race_signals`).
///
/// `db::queries::calendar::mapeamento` CHAMA esta função como fallback da linha legada
/// sem `nome` — antes era um espelho com o literal em PT, que precisava ser mantido em
/// sincronia à mão e não era.
pub(crate) fn nome_da_etapa(rodada: i32, nome_curto: &str) -> String {
    rust_i18n::t!("calendar.round_name", round = rodada, track = nome_curto).to_string()
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
        nome: nome_da_etapa(rodada, track.nome_curto),
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

    // A distribuição é a oficial de `constants::scoring` — os cortes viviam copiados
    // aqui e recalibrar num lugar deixava o outro para trás. Um único sorteio, os
    // mesmos limites de antes.
    let mut intensity = rng.gen::<f64>();
    for (condition, weight) in crate::constants::scoring::RAIN_INTENSITY_DISTRIBUTION {
        if intensity < weight {
            return condition;
        }
        intensity -= weight;
    }
    WeatherCondition::HeavyRain
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

/// Piso de voltas de uma etapa. Vale para qualquer duração: uma prova com menos que isso
/// não tem os cinco segmentos que o motor de corrida precisa para existir.
pub(crate) const PISO_DE_VOLTAS: i32 = 5;

/// Duração (min) até a qual a etapa é tratada como SPRINT para efeito de teto de voltas.
pub(crate) const DURACAO_DE_SPRINT_MIN: i32 = 60;

/// Teto de voltas de uma etapa de sprint.
pub(crate) const TETO_DE_VOLTAS_SPRINT: i32 = 50;

/// Teto de voltas de uma etapa longa (acima de [`DURACAO_DE_SPRINT_MIN`]).
pub(crate) const TETO_DE_VOLTAS_LONGA: i32 = 150;

/// O teto de voltas que vale para esta duração.
///
/// **Por que o teto deixou de ser 50 para todo mundo.** O clamp único em 50 nasceu como
/// sanidade de sprint e foi herdado pelas provas longas sem ninguém medir. A medição (B19)
/// mostrou o custo: em 120 min **76%** das pistas do catálogo batem no teto, em 180 min
/// **95%**, em 240 min **96%** e em 360 min **98%** — ou seja, no Endurance o número de
/// voltas da etapa praticamente não é mais uma função da pista, é a constante 50. E o que
/// depende dele depende de verdade: a janela de parada (`planejar_paradas` escolhe uma volta
/// dentro de 35–65% do total, e com 50 voltas o grid inteiro escolhe entre ~17 e ~32), a
/// contagem de voltas no segmento e a conta de desgaste por volta.
///
/// O teto continua existindo — a alternativa de remover o limite deixaria uma prova de 6 h
/// em pista curta passar de 300 voltas, e o custo computacional por volta não é zero. 150 é
/// folgado o bastante para a duração mais longa do calendário (360 min) sair do teto na
/// maioria das pistas e apertado o bastante para continuar sendo um limite.
///
/// A fronteira em 60 min é a mesma que separa sprint de prova longa no resto do jogo: abaixo
/// dela nada muda, e o contrato antigo (5..=50) segue valendo letra por letra.
pub(crate) fn teto_de_voltas(duracao_corrida_min: i32) -> i32 {
    if duracao_corrida_min <= DURACAO_DE_SPRINT_MIN {
        TETO_DE_VOLTAS_SPRINT
    } else {
        TETO_DE_VOLTAS_LONGA
    }
}

pub(crate) fn estimate_laps(track: &TrackInfo, duracao_corrida_min: i32) -> i32 {
    let tempo_volta_estimado_min = track.comprimento_km / 2.0;
    ((duracao_corrida_min as f64 / tempo_volta_estimado_min).ceil() as i32)
        .clamp(PISO_DE_VOLTAS, teto_de_voltas(duracao_corrida_min))
}

pub(crate) fn split_track_name(full_name: &str) -> (String, String) {
    if let Some((name, config)) = full_name.split_once(" - ") {
        (name.to_string(), config.to_string())
    } else {
        (full_name.to_string(), "Default".to_string())
    }
}
