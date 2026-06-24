//! Leitura dos RESULTADOS OFICIAIS persistidos pelo iRacing no JSON do aiseason.
//!
//! Quando o jogador sai de uma corrida de IA, o iRacing simula o resto e GRAVA o
//! resultado final de volta em `aiseasons/<Season>.json`, em
//! `events[i].results`. É a fonte autoritativa e PERSISTIDA (sobrevive a fechar o
//! sim) — diferente da reconstrução ao vivo, que some quando o jogador sai cedo.
//!
//! É JSON puro, então navegamos com `serde_json::Value` direto. O índice do evento
//! vem do "post-it" gravado no export (mapa evento→corrida da carreira).
//!
//! ATENÇÃO: a BATIDA do jogador NÃO está aqui — quando o iRacing AI-completa a
//! corrida ele põe o jogador em último mas zera incidents/reason. O custo de
//! conserto continua vindo do monitor ao vivo (`peak_crash_score`).

use serde_json::Value;

/// `simsession_type` do iRacing: 5 = quali, 6 = corrida.
const SIMSESSION_RACE: i64 = 6;
const SIMSESSION_QUALIFY: i64 = 5;

/// Uma linha do resultado de corrida (um carro), já normalizada.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiResultRow {
    /// Posição GERAL (1-based; o JSON é 0-based, já somamos 1).
    pub position: i32,
    /// Posição na CLASSE (1-based; idem).
    pub class_position: i32,
    /// Largada / grid (1-based; idem). 0 se ausente.
    pub grid_position: i32,
    pub incidents: i32,
    /// "Running" = classificado; qualquer outra coisa = abandono.
    pub reason_out: String,
    /// Melhor volta em ms; <= 0 = sem volta válida.
    pub best_lap_time_ms: f64,
    /// Gap ao líder em ms (`interval`); 0 para o líder.
    pub interval_ms: f64,
    pub laps_lead: i32,
    pub laps_complete: i32,
    /// Número do carro (`carNumber`, vem como string no JSON).
    pub car_number: i32,
    /// `cust_id` do iRacing — casa o JOGADOR (== cached_custid).
    pub cust_id: i64,
    pub display_name: String,
}

impl AiResultRow {
    pub fn is_dnf(&self) -> bool {
        !self.reason_out.eq_ignore_ascii_case("Running")
    }
}

/// Resultado de UM evento (uma corrida) do aiseason.
#[derive(Debug, Clone, Default)]
pub struct AiEventResult {
    pub track_id: i64,
    /// Voltas completas da corrida (`race_summary.laps_complete`). 0 = sem resultado.
    pub laps_complete: i32,
    /// Resultado da CORRIDA (session 6).
    pub rows: Vec<AiResultRow>,
    /// Resultado da QUALI (session 5) — fonte do tempo da pole. Vazio se não houve.
    pub qualify: Vec<AiResultRow>,
}

impl AiEventResult {
    pub fn is_final(&self) -> bool {
        self.laps_complete > 0 && !self.rows.is_empty()
    }
}

fn as_i32(v: &Value) -> i32 {
    // Aceita número ou string numérica (carNumber vem como "64").
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        .unwrap_or(0) as i32
}

fn as_i64(v: &Value) -> i64 {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        .unwrap_or(0)
}

fn as_f64(v: &Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        .unwrap_or(0.0)
}

/// Converte as linhas de `results` de uma sessão em [`AiResultRow`]s.
fn parse_rows(session: &Value) -> Vec<AiResultRow> {
    session
        .get("results")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|r| AiResultRow {
                    position: as_i32(r.get("position").unwrap_or(&Value::Null)) + 1,
                    class_position: as_i32(
                        r.get("finish_position_in_class").unwrap_or(&Value::Null),
                    ) + 1,
                    grid_position: r
                        .get("starting_position")
                        .map(|v| as_i32(v) + 1)
                        .unwrap_or(0),
                    incidents: as_i32(r.get("incidents").unwrap_or(&Value::Null)),
                    reason_out: r
                        .get("reason_out")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Running")
                        .to_string(),
                    best_lap_time_ms: as_f64(r.get("best_lap_time").unwrap_or(&Value::Null)),
                    interval_ms: as_f64(r.get("interval").unwrap_or(&Value::Null)).max(0.0),
                    laps_lead: as_i32(r.get("laps_lead").unwrap_or(&Value::Null)),
                    laps_complete: as_i32(r.get("laps_complete").unwrap_or(&Value::Null)),
                    car_number: as_i32(r.get("carNumber").unwrap_or(&Value::Null)),
                    cust_id: as_i64(r.get("cust_id").unwrap_or(&Value::Null)),
                    display_name: r
                        .get("display_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Extrai o resultado da CORRIDA (+ quali) do evento `event_index` do JSON.
/// `None` se o evento/sessão de corrida não existir.
pub fn parse_event_result(season_json: &Value, event_index: usize) -> Option<AiEventResult> {
    let results = season_json
        .get("events")?
        .as_array()?
        .get(event_index)?
        .get("results")?;

    let track_id = as_i64(results.get("trackId").unwrap_or(&Value::Null));
    let laps_complete = results
        .get("race_summary")
        .and_then(|s| s.get("laps_complete"))
        .map(as_i32)
        .unwrap_or(0);

    let sessions = results.get("session_results")?.as_array()?;
    let race_session = sessions
        .iter()
        .find(|s| s.get("simsession_type").map(as_i64) == Some(SIMSESSION_RACE))?;
    let rows = parse_rows(race_session);
    let qualify = sessions
        .iter()
        .find(|s| s.get("simsession_type").map(as_i64) == Some(SIMSESSION_QUALIFY))
        .map(parse_rows)
        .unwrap_or_default();

    Some(AiEventResult {
        track_id,
        laps_complete,
        rows,
        qualify,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "events": [{
                "results": {
                    "trackId": 451,
                    "race_summary": { "laps_complete": 7 },
                    "session_results": [
                        { "simsession_type": 5, "results": [] },
                        { "simsession_type": 6, "results": [
                            { "position": 0, "starting_position": 0, "finish_position_in_class": 0,
                              "incidents": 0, "reason_out": "Running", "best_lap_time": 136499.2,
                              "laps_lead": 0, "laps_complete": 7, "carNumber": "8", "cust_id": 9703,
                              "display_name": "Daniel Konig" },
                            { "position": 5, "starting_position": 5, "finish_position_in_class": 5,
                              "incidents": 0, "reason_out": "Running", "best_lap_time": 142649.0,
                              "laps_lead": 7, "laps_complete": 7, "carNumber": "4", "cust_id": 9701,
                              "display_name": "Cooper Hall" },
                            { "position": 11, "starting_position": 11, "finish_position_in_class": 11,
                              "incidents": 0, "reason_out": "Running", "best_lap_time": -1000.0,
                              "laps_lead": 0, "laps_complete": 0, "carNumber": "64", "cust_id": 747998,
                              "display_name": "Rodrigo Carvalho Silva" }
                        ]}
                    ]
                }
            }]
        })
    }

    #[test]
    fn parses_race_event() {
        let ev = parse_event_result(&sample(), 0).unwrap();
        assert!(ev.is_final());
        assert_eq!(ev.track_id, 451);
        assert_eq!(ev.laps_complete, 7);
        assert_eq!(ev.rows.len(), 3);

        let winner = &ev.rows[0];
        assert_eq!(winner.position, 1); // 0-based 0 -> 1
        assert_eq!(winner.grid_position, 1);
        assert_eq!(winner.car_number, 8);
        assert!(!winner.is_dnf());

        // O líder de voltas (Cooper) — laps_lead vem certo.
        assert_eq!(ev.rows[1].laps_lead, 7);

        // O jogador (cust 747998) ficou em último, sem voltas.
        let player = ev.rows.iter().find(|r| r.cust_id == 747998).unwrap();
        assert_eq!(player.position, 12);
        assert_eq!(player.grid_position, 12);
        assert_eq!(player.car_number, 64);
    }

    #[test]
    fn no_result_when_not_final() {
        let mut s = sample();
        s["events"][0]["results"]["race_summary"]["laps_complete"] = serde_json::json!(0);
        let ev = parse_event_result(&s, 0).unwrap();
        assert!(!ev.is_final());
    }

    #[test]
    fn missing_event_is_none() {
        assert!(parse_event_result(&sample(), 9).is_none());
    }
}
