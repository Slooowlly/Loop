//! Helpers temporais do calendário: janelas de temporada, semanas e datas
//! visuais (extraídos de `calendar/mod.rs`).

use chrono::{Datelike, NaiveDate, Weekday};

use crate::models::enums::SeasonPhase;

// ── Constantes de calendário ──────────────────────────────────────────────────

/// Janelas mensais da temporada.
/// O dia exato continua flexível, mas cada bloco precisa caber na sua faixa do ano.
const REGULAR_WINDOW_START_MONTH: u32 = 2;
const REGULAR_WINDOW_END_MONTH: u32 = 8;
const SPECIAL_WINDOW_START_MONTH: u32 = 9;
const SPECIAL_WINDOW_END_MONTH: u32 = 12;

/// Distribui N rodadas uniformemente entre [start_week, end_week].
/// rodada é 1-based.
pub(crate) fn week_for_rodada(rodada: i32, total: i32, start: i32, end: i32) -> i32 {
    if total <= 1 {
        return start;
    }
    start + (rodada - 1) * (end - start) / (total - 1)
}

pub(crate) fn calendar_week_for_round(
    year: i32,
    season_phase: SeasonPhase,
    rodada: i32,
    total: i32,
) -> i32 {
    let (week_start, week_end) = season_week_window(year, season_phase);
    week_for_rodada(rodada, total, week_start, week_end)
}

pub(crate) fn display_date_for_category_week(
    year: i32,
    week: i32,
    category_id: &str,
    season_phase: SeasonPhase,
) -> String {
    display_date_for_weekday(
        year,
        week,
        resolve_calendar_weekday(category_id, season_phase),
    )
}

pub(crate) fn display_date_for_category_round(
    year: i32,
    week: i32,
    category_id: &str,
    season_phase: SeasonPhase,
    rodada: i32,
) -> String {
    if season_phase == SeasonPhase::BlocoRegular && category_id == "lmp2" {
        let weekday = if rodada % 2 == 1 {
            Weekday::Sat
        } else {
            Weekday::Sun
        };
        return display_date_for_weekday(year, week, weekday);
    }

    display_date_for_category_week(year, week, category_id, season_phase)
}

pub(crate) fn resolve_calendar_weekday(category_id: &str, _season_phase: SeasonPhase) -> Weekday {
    // Dia fixo por categoria. Mazda/Toyota compartilham o dia e alternam semanas
    // (a separação por semana é feita pela janela disjunta no gerador 9D).
    match category_id {
        "mazda_rookie" | "toyota_rookie" => Weekday::Mon,
        "mazda_amador" | "toyota_amador" => Weekday::Tue,
        "bmw_m2" => Weekday::Wed,
        "gt4" => Weekday::Thu,
        "gt3" => Weekday::Fri,
        "production_challenger" => Weekday::Sat,
        "endurance" | "lmp2" => Weekday::Sun,
        _ => Weekday::Sat,
    }
}

pub(crate) fn season_week_window(year: i32, phase: SeasonPhase) -> (i32, i32) {
    let (start_date, end_date) = season_date_window(year, phase);
    (
        start_date.iso_week().week() as i32,
        end_date.iso_week().week() as i32,
    )
}

pub(crate) fn season_date_window(year: i32, phase: SeasonPhase) -> (NaiveDate, NaiveDate) {
    match phase {
        // Começo mais para o fim de fevereiro para abrir espaço real ao mercado de dezembro-fevereiro.
        SeasonPhase::BlocoRegular => (
            last_weekday_of_month(year, REGULAR_WINDOW_START_MONTH, Weekday::Sat),
            nth_weekday_of_month(year, REGULAR_WINDOW_END_MONTH, Weekday::Sat, 3),
        ),
        // Deixa a virada agosto/setembro para convocação e preserva o restante de dezembro para o mercado aberto.
        SeasonPhase::BlocoEspecial => (
            nth_weekday_of_month(year, SPECIAL_WINDOW_START_MONTH, Weekday::Sun, 2),
            last_weekday_of_month(year, SPECIAL_WINDOW_END_MONTH, Weekday::Sun),
        ),
        _ => (
            last_weekday_of_month(year, REGULAR_WINDOW_START_MONTH, Weekday::Sat),
            nth_weekday_of_month(year, REGULAR_WINDOW_END_MONTH, Weekday::Sat, 3),
        ),
    }
}

/// Queda de data quando `year`/`month` não formam uma data válida.
///
/// Só é alcançável com ano fora da faixa do `chrono` (~±262 mil anos), que o jogo não
/// produz — mas estas funções montam a JANELA DA TEMPORADA, no caminho de criação de
/// carreira, e ali um `expect` derruba o app inteiro. Uma data de queda deixa o
/// calendário sair torto em vez de não sair.
fn data_de_queda(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or_default()
}

pub(crate) fn nth_weekday_of_month(year: i32, month: u32, weekday: Weekday, nth: u32) -> NaiveDate {
    let Some(first_day) = NaiveDate::from_ymd_opt(year, month.clamp(1, 12), 1) else {
        return data_de_queda(year);
    };
    let offset = (7 + weekday.num_days_from_monday() as i64
        - first_day.weekday().num_days_from_monday() as i64)
        % 7;
    let day = 1 + offset as u32 + (nth.saturating_sub(1) * 7);
    // Passou do fim do mês (5ª ocorrência num mês que só tem 4) → cai para o último dia.
    NaiveDate::from_ymd_opt(year, month, day)
        .or_else(|| last_day_of_month(year, month))
        .unwrap_or(first_day)
}

pub(crate) fn last_weekday_of_month(year: i32, month: u32, weekday: Weekday) -> NaiveDate {
    let Some(mut current) = last_day_of_month(year, month) else {
        return data_de_queda(year);
    };
    // Sete passos bastam para achar qualquer dia da semana; o laço limitado troca o
    // `expect` do `pred_opt` por parada garantida.
    for _ in 0..7 {
        if current.weekday() == weekday {
            return current;
        }
        match current.pred_opt() {
            Some(anterior) => current = anterior,
            None => return current,
        }
    }
    current
}

pub(crate) fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
}

/// Converte week_of_year + year em uma data visual ISO "YYYY-MM-DD" (Sábado da semana).
/// Apenas para display legado — a lógica temporal 9D usa season_week.
#[cfg(test)]
pub(crate) fn week_to_display_date(year: i32, week: i32) -> String {
    display_date_for_weekday(year, week, Weekday::Sat)
}

pub(crate) fn display_date_for_weekday(year: i32, week: i32, weekday: Weekday) -> String {
    NaiveDate::from_isoywd_opt(year, week.clamp(1, 52) as u32, weekday)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| format!("{}-01-01", year))
}

/// Deriva uma data visual a partir da régua 9D (season_week) em vez de week_of_year.
/// Resolve o ano civil correto via season_week_to_calendar_year antes de delegar
/// a display_date_for_weekday — necessário porque sw 1–4 pertencem ao ano anterior.
#[allow(dead_code)]
pub(crate) fn display_date_for_season_week(
    season_week: u8,
    season_year: i32,
    weekday: Weekday,
) -> Result<String, String> {
    use crate::models::temporal::{season_week_to_calendar_year, season_week_to_week_of_year};
    let woy = season_week_to_week_of_year(season_week)?;
    let cal_year = season_week_to_calendar_year(season_week, season_year)?;
    Ok(display_date_for_weekday(cal_year, woy as i32, weekday))
}
