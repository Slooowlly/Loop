//! Mapeamento de linha do banco para CalendarEntry: leitura tolerante de colunas
//! legadas, parse estrito dos enums e validacao dos inteiros nao negativos.

use super::*;

/// As colunas que o `calendar_from_row` lê — a projeção que substitui o `SELECT *`.
///
/// Inclui os pares legado/atual que o mapeador consulta em cascata (`season_id` com
/// `temporada_id`, `track_name` com `pista`, `duracao_corrida_min` com `duracao`): a
/// leitura tolerante só funciona se as duas colunas vierem na linha.
pub(crate) const COLUNAS_CALENDAR: &[&str] = &[
    "id",
    "season_id",
    "temporada_id",
    "categoria",
    "rodada",
    "nome",
    "track_id",
    "track_name",
    "pista",
    "track_config",
    "clima",
    "temperatura",
    "voltas",
    "duracao_corrida_min",
    "duracao",
    "duracao_classificacao_min",
    "status",
    "horario",
    "week_of_year",
    "season_phase",
    "data",
    "thematic_slot",
    "season_week",
];

pub(crate) fn colunas_select_calendar() -> &'static str {
    static SQL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SQL.get_or_init(|| COLUNAS_CALENDAR.join(", "))
}

pub(crate) fn calendar_entry_season_week(entry: &CalendarEntry) -> i32 {
    entry
        .season_week
        .map(|week| week as i32)
        .unwrap_or(entry.week_of_year + 4)
}

pub(crate) fn collect_entries<T>(mapped: T) -> Result<Vec<CalendarEntry>, DbError>
where
    T: IntoIterator<Item = rusqlite::Result<CalendarEntry>>,
{
    let mut entries = Vec::new();
    for row in mapped {
        entries.push(row?);
    }
    Ok(entries)
}

pub(crate) fn calendar_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarEntry> {
    Ok(CalendarEntry {
        id: row.get("id")?,
        season_id: optional_string(row, "season_id")?
            .or_else(|| optional_string(row, "temporada_id").ok().flatten())
            .unwrap_or_default(),
        categoria: row.get("categoria")?,
        rodada: row.get("rodada")?,
        nome: optional_string(row, "nome")?.unwrap_or_else(|| {
            let pista = optional_string(row, "track_name")
                .ok()
                .flatten()
                .or_else(|| optional_string(row, "pista").ok().flatten())
                .unwrap_or_default();
            // Mesma chave i18n da montagem, e não um `format!("Rodada {}")` cru: a
            // linha legada sem `nome` é justamente a que ninguém traduziu, e montar
            // o rótulo aqui em português colocava "Rodada 3" na tela em en-US toda
            // vez que o save era antigo.
            crate::calendar::nome_da_etapa(row.get::<_, i32>("rodada").unwrap_or(0), &pista)
        }),
        track_id: parse_non_negative_u32(row, "track_id", 0)?,
        track_name: optional_string(row, "track_name")?
            .or_else(|| optional_string(row, "pista").ok().flatten())
            .unwrap_or_default(),
        track_config: optional_string(row, "track_config")?.unwrap_or_default(),
        clima: WeatherCondition::from_str_strict(&row.get::<_, String>("clima")?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        temperatura: optional_f64(row, "temperatura")?.unwrap_or(25.0),
        voltas: parse_non_negative_i32(row, "voltas", 10)?,
        duracao_corrida_min: optional_i64(row, "duracao_corrida_min")?
            .or_else(|| optional_i64(row, "duracao").ok().flatten())
            .map(|value| parse_non_negative_i32_value("duracao_corrida_min", value))
            .transpose()?
            .unwrap_or(60),
        duracao_classificacao_min: parse_non_negative_i32(row, "duracao_classificacao_min", 15)?,
        status: RaceStatus::from_str_strict(
            &optional_string(row, "status")?.unwrap_or_else(|| "Pendente".to_string()),
        )
        .map_err(rusqlite::Error::InvalidParameterName)?,
        horario: optional_string(row, "horario")?.unwrap_or_else(|| "14:00".to_string()),
        week_of_year: parse_non_negative_i32(row, "week_of_year", 0)?,
        season_phase: match optional_string(row, "season_phase")? {
            None => SeasonPhase::BlocoRegular,
            Some(s) => {
                SeasonPhase::from_str_strict(&s).map_err(rusqlite::Error::InvalidParameterName)?
            }
        },
        display_date: optional_string(row, "data")?.unwrap_or_default(),
        thematic_slot: match optional_string(row, "thematic_slot")? {
            // NULL no banco (saves pré-v12) → NaoClassificado
            None => ThematicSlot::NaoClassificado,
            // string presente: parse estrito — string inválida é erro, não fallback silencioso
            Some(s) => {
                ThematicSlot::from_str_strict(&s).map_err(rusqlite::Error::InvalidParameterName)?
            }
        },
        // Coluna adicionada em v33; None para saves anteriores ou entradas pré-backfill.
        // unwrap_or(None) absorve InvalidColumnName quando a coluna ainda não existe
        // (ex.: testes que criam schema mínimo sem rodar run_all).
        season_week: row
            .get::<_, Option<i64>>("season_week")
            .unwrap_or(None)
            .and_then(|v| if v > 0 { Some(v as u32) } else { None }),
    })
}

pub(crate) fn invalid_calendar_data_error(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

pub(crate) fn parse_non_negative_u32(
    row: &rusqlite::Row<'_>,
    column_name: &str,
    default: u32,
) -> rusqlite::Result<u32> {
    optional_i64(row, column_name)?
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                invalid_calendar_data_error(format!(
                    "Campo '{column_name}' invalido: esperado inteiro nao negativo, recebido {value}"
                ))
            })
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

pub(crate) fn parse_non_negative_i32(
    row: &rusqlite::Row<'_>,
    column_name: &str,
    default: i32,
) -> rusqlite::Result<i32> {
    optional_i64(row, column_name)?
        .map(|value| parse_non_negative_i32_value(column_name, value))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

pub(crate) fn parse_non_negative_i32_value(column_name: &str, value: i64) -> rusqlite::Result<i32> {
    let parsed = i32::try_from(value).map_err(|_| {
        invalid_calendar_data_error(format!(
            "Campo '{column_name}' invalido: esperado inteiro nao negativo, recebido {value}"
        ))
    })?;
    if parsed < 0 {
        return Err(invalid_calendar_data_error(format!(
            "Campo '{column_name}' invalido: esperado inteiro nao negativo, recebido {value}"
        )));
    }
    Ok(parsed)
}

pub(crate) fn optional_string(
    row: &rusqlite::Row<'_>,
    column_name: &str,
) -> rusqlite::Result<Option<String>> {
    match row.get_ref(column_name)? {
        rusqlite::types::ValueRef::Null => Ok(None),
        _ => row.get(column_name).map(Some),
    }
}

pub(crate) fn optional_i64(
    row: &rusqlite::Row<'_>,
    column_name: &str,
) -> rusqlite::Result<Option<i64>> {
    match row.get_ref(column_name)? {
        rusqlite::types::ValueRef::Null => Ok(None),
        _ => row.get(column_name).map(Some),
    }
}

pub(crate) fn optional_f64(
    row: &rusqlite::Row<'_>,
    column_name: &str,
) -> rusqlite::Result<Option<f64>> {
    match row.get_ref(column_name)? {
        rusqlite::types::ValueRef::Null => Ok(None),
        _ => row.get(column_name).map(Some),
    }
}
