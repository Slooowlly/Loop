//! Bloco de RESULTADOS de uma etapa concluída, no formato do aiseason do iRacing.
//!
//! Transforma a corrida SIMULADA no app (`race_results`) em
//! `results.session_results` (Qualifying + Race), pra o iRacing reconhecer a etapa
//! como já disputada (mostra "Resultados") e avançar pra próxima.
//!
//! Não guardamos tempos de volta individuais — aproximamos pela média (tempo total /
//! voltas). Posições/largadas/gaps são exatos. `position` no iRacing é 0-indexed.

use serde_json::{json, Value};

/// Uma linha de resultado (do nosso `race_results` + dados do piloto).
pub struct ResultDriver {
    pub finish: i64, // posição final 1-indexed
    pub start: i64,  // largada 1-indexed
    pub laps: i64,
    pub total_ms: f64,
    pub gap_ms: f64,
    pub incidents: i64,
    pub dnf: bool,
    pub dnf_reason: Option<String>,
    pub has_fastest: bool,
    pub car_number: String,
    pub cust_id: i64,
    pub name: String,
    pub car_id: i64,
    pub car_class_id: i64,
}

fn avg_lap_ms(d: &ResultDriver) -> f64 {
    if d.laps > 0 {
        d.total_ms / d.laps as f64
    } else {
        0.0
    }
}

fn race_entry(d: &ResultDriver) -> Value {
    let lap_ms = avg_lap_ms(d);
    let best_ms = if d.has_fastest { lap_ms * 0.97 } else { lap_ms };
    json!({
        "id": d.finish,
        "incidents": d.incidents,
        "lap": 0,
        "lapsDriven": 0,
        "lastLapTime": lap_ms / 1000.0,
        "time": d.total_ms / 1000.0,
        "position": d.finish - 1,
        "best_lap_num": 1,
        "best_lap_time": best_ms,
        "laps_complete": d.laps,
        "reason_out": if d.dnf {
            d.dnf_reason.clone().unwrap_or_else(|| "DNF".to_string())
        } else {
            "Running".to_string()
        },
        "laps_lead": if d.finish == 1 { d.laps } else { 0 },
        "interval": d.gap_ms / 1000.0,
        "carNumber": d.car_number,
        "car_class_id": d.car_class_id,
        "car_id": d.car_id,
        "cust_id": d.cust_id,
        "display_name": d.name,
        "finish_position_in_class": d.finish - 1,
        "starting_position": d.start - 1,
    })
}

fn qualy_entry(d: &ResultDriver, pole_ms: f64) -> Value {
    // Tempo de quali aproximado: pole + incremento por posição de largada.
    let q_ms = pole_ms + (d.start - 1).max(0) as f64 * 150.0;
    json!({
        "id": d.start,
        "incidents": 0,
        "lap": 1,
        "lapsDriven": 0,
        "lastLapTime": q_ms / 1000.0,
        "time": q_ms / 1000.0,
        "position": d.start - 1,
        "best_lap_num": 1,
        "best_lap_time": q_ms,
        "laps_complete": 1,
        "reason_out": "Running",
        "laps_lead": 0,
        "interval": if d.start == 1 { 0.0 } else { -1.0 },
        "carNumber": d.car_number,
        "car_class_id": d.car_class_id,
        "car_id": d.car_id,
        "cust_id": d.cust_id,
        "display_name": d.name,
        "finish_position_in_class": d.start - 1,
        "starting_position": d.start - 1,
    })
}

/// Monta o bloco `results` (Qualifying + Race) de uma etapa concluída.
pub fn build_results(drivers: &[ResultDriver]) -> Value {
    let laps = drivers.iter().map(|d| d.laps).max().unwrap_or(0);
    let pole_ms = drivers
        .iter()
        .filter(|d| d.laps > 0)
        .map(avg_lap_ms)
        .fold(f64::MAX, f64::min);
    let pole_ms = if pole_ms == f64::MAX {
        90_000.0
    } else {
        pole_ms
    };
    let avg_lap = if laps > 0 && !drivers.is_empty() {
        drivers.iter().map(|d| d.total_ms).sum::<f64>()
            / drivers.len() as f64
            / laps as f64
            / 1000.0
    } else {
        0.0
    };

    let mut qualy: Vec<&ResultDriver> = drivers.iter().collect();
    qualy.sort_by_key(|d| d.start);
    let mut race: Vec<&ResultDriver> = drivers.iter().collect();
    race.sort_by_key(|d| d.finish);

    let q_results: Vec<Value> = qualy.iter().map(|d| qualy_entry(d, pole_ms)).collect();
    let r_results: Vec<Value> = race.iter().map(|d| race_entry(d)).collect();

    json!({
        "session_results": [
            {
                "simsession_type": 5, "simsession_type_name": "Qualifying",
                "simsession_number": 0, "averageLapTime": -1, "lapsComplete": 0,
                "numCautionFlags": 0, "numCautionLaps": 0, "numLeadChanges": 0,
                "official": true, "sessionNumber": 0, "results": q_results
            },
            {
                "simsession_type": 6, "simsession_type_name": "Race",
                "simsession_number": 1, "averageLapTime": avg_lap, "lapsComplete": laps,
                "numCautionFlags": 0, "numCautionLaps": 0, "numLeadChanges": 1,
                "official": true, "sessionNumber": 1, "results": r_results
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(finish: i64, start: i64, total_ms: f64, gap_ms: f64, fastest: bool) -> ResultDriver {
        ResultDriver {
            finish,
            start,
            laps: 10,
            total_ms,
            gap_ms,
            incidents: 0,
            dnf: false,
            dnf_reason: None,
            has_fastest: fastest,
            car_number: finish.to_string(),
            cust_id: 990000 + finish,
            name: format!("Driver {finish}"),
            car_id: 67,
            car_class_id: 74,
        }
    }

    #[test]
    fn monta_quali_e_corrida() {
        let drivers = vec![
            d(1, 1, 907500.0, 0.0, true),
            d(2, 6, 930831.0, 23331.0, false),
            d(3, 2, 930927.0, 23427.0, false),
        ];
        let v = build_results(&drivers);
        let sr = v["session_results"].as_array().unwrap();
        assert_eq!(sr.len(), 2);
        assert_eq!(sr[0]["simsession_type_name"], "Qualifying");
        assert_eq!(sr[1]["simsession_type_name"], "Race");
        let race = sr[1]["results"].as_array().unwrap();
        // Corrida ordenada por chegada; position 0-indexed.
        assert_eq!(race[0]["position"], 0);
        assert_eq!(race[0]["display_name"], "Driver 1");
        assert_eq!(race[0]["laps_lead"], 10); // vencedor lidera
        assert_eq!(race[1]["starting_position"], 5); // largou 6º → 0-indexed 5
                                                     // Quali ordenada por largada.
        let q = sr[0]["results"].as_array().unwrap();
        assert_eq!(q[0]["position"], 0);
    }
}
