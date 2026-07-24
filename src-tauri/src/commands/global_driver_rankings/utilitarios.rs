//! Utilitários: existência de tabela, leitura de JSON e conversões numéricas.

use super::*;

pub(super) fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
        params![table],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|e| format!("Falha ao verificar tabela {table}: {e}"))
}

pub(super) fn json_i32(value: &Value, key: &str) -> i32 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

pub(super) fn json_i32_option(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value as i32)
}

pub(super) fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

pub(super) fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(super) fn round_one(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub(super) fn years_since(start_year: i32, current_year: i32) -> Option<i32> {
    if start_year <= 0 || current_year <= 0 || current_year < start_year {
        return None;
    }
    Some(current_year - start_year + 1)
}

pub(super) fn parse_year(value: &str) -> Option<i32> {
    parse_positive_i32(value).filter(|year| *year >= 1900)
}

pub(super) fn parse_positive_i32(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok().filter(|year| *year > 0)
}
