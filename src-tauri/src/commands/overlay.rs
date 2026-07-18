//! Dados AO VIVO para a torre de tempos (overlay do iRacing).
//!
//! Cruza três mundos:
//!   • PISTA (SDK): posição na classe, pit, melhor volta, pneus — `read_telemetry`
//!     + `race_monitor` (identidade CarIdx→número/classe) + `tire_strategy`.
//!   • CARREIRA (save): nome do piloto, time, cor do time, pontos de campeonato.
//!   • SESSÃO: tipo, voltas, bandeira, categoria, clima.
//!
//! A ponte CarIdx→nosso piloto é o NÚMERO do carro (nós geramos o roster). Mesma
//! resolução de `build_session_race_result`/`iracing_car_colors`, mas ao vivo.
//!
//! Devolve `None` quando não há sessão ativa no iRacing (overlay fica oculto).

use std::collections::HashMap;

use serde::Serialize;

use crate::config::app_config::AppConfig;
use crate::constants::scoring;
use crate::db::connection::Database;
use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
use crate::iracing_sdk::{race_monitor, tire_strategy, CarSnapshot};

/// Diagnóstico do overlay: grava o ESTADO atual em `%TEMP%\iracer_overlay_data.log`,
/// só quando o estado MUDA (o front chama ~1×/s; sem flood). Diz exatamente em qual
/// porta o `get_overlay_data` está saindo (telemetria/cars/cruzamento/sucesso).
fn dbg_state(state: &str) {
    use std::io::Write;
    use std::sync::Mutex;
    static LAST: Mutex<Option<String>> = Mutex::new(None);
    {
        let mut last = match LAST.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if last.as_deref() == Some(state) {
            return;
        }
        *last = Some(state.to_string());
    }
    let mut path = std::env::temp_dir();
    path.push("iracer_overlay_data.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{secs}] {state}");
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayWeather {
    condition: String,       // "clear" | "clouds" | "rain"
    air_temp: Option<i32>,   // °C do ar (None = desconhecido ao vivo)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlaySession {
    #[serde(rename = "type")]
    kind: String, // "R" | "Q" | "P"
    lap: i32,
    total_laps: i32, // 0 = desconhecido (o front esconde o "/total")
    flag: String,    // "green" | "yellow" | "checkered"
    category: String, // id da categoria do evento (ex.: "gt3", "endurance")
    weather: OverlayWeather,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayCar {
    pos: i32, // posição na CLASSE
    name: String,
    team: String,
    color: String, // #rrggbb
    delta: i32,    // posições ganhas(+)/perdidas(-) desde a largada
    stops: i32,
    tire_history: Vec<String>, // ["dry","wet",...] por stint
    points: i32,
    gain: i32, // pontos que ganharia terminando na posição atual
    fastest: String,
    fol: bool, // volta mais rápida da classe
    pit: bool,
    flag: Option<String>, // só do jogador (black), IA não tem canal por carro
    player: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayClass {
    id: String,
    label: String,
    cars: Vec<OverlayCar>,
}

#[derive(Serialize)]
pub struct OverlayData {
    session: OverlaySession,
    classes: Vec<OverlayClass>,
}

fn fmt_lap(secs: f64) -> String {
    if secs <= 0.0 {
        return String::new();
    }
    let total_ms = (secs * 1000.0).round() as i64;
    let m = total_ms / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}.{:03}", m, s, ms)
}

fn wetness_to_condition(w: i32) -> &'static str {
    match w {
        n if n >= 4 => "rain",  // LightlyWet+
        1 => "clear",           // Dry
        _ => "clouds",          // MostlyDry / desconhecido
    }
}

fn compound_str(c: tire_strategy::Compound) -> &'static str {
    match c {
        tire_strategy::Compound::Wet => "wet",
        _ => "dry",
    }
}

/// Mapa `SessionNum -> SessionType` (ex.: "Race", "Open Qualify", "Practice") do
/// YAML. Em cada bloco de sessão o `SessionNum` vem antes do `SessionType`.
fn parse_session_types(yaml: &str) -> HashMap<i32, String> {
    let mut map = HashMap::new();
    let mut cur: Option<i32> = None;
    for line in yaml.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("SessionNum:") {
            cur = v.trim().parse::<i32>().ok();
        } else if let Some(v) = l.strip_prefix("SessionType:") {
            if let Some(n) = cur {
                map.insert(n, v.trim().to_string());
            }
        }
    }
    map
}

/// "Race"/"Qualify"/"Practice" -> "R"/"Q"/"P".
fn session_kind(session_types: &HashMap<i32, String>, session_num: i32) -> &'static str {
    match session_types.get(&session_num).map(|s| s.as_str()) {
        Some(s) if s.contains("Qualify") => "Q",
        Some(s) if s.contains("Practice") || s.contains("Warmup") => "P",
        _ => "R",
    }
}

fn tower_order_key(pos: i32, unclassified: &(i64, i64)) -> (bool, i64, i64) {
    if pos > 0 {
        (false, i64::from(pos), 0)
    } else {
        (true, unclassified.0, unclassified.1)
    }
}

fn best_positive_lap(live: Option<f64>, recorded: Option<f64>) -> f64 {
    match (
        live.filter(|secs| *secs > 0.0),
        recorded.filter(|secs| *secs > 0.0),
    ) {
        (Some(live), Some(recorded)) => live.min(recorded),
        (Some(live), None) => live,
        (None, Some(recorded)) => recorded,
        (None, None) => 0.0,
    }
}

fn history_matches_subsession(history_id: i64, current_id: i64) -> bool {
    history_id > 0 && history_id == current_id
}

fn roster_with_telemetry<'a>(
    roster: &'a [race_monitor::YamlCarMeta],
    telemetry: &'a [CarSnapshot],
) -> Vec<(&'a race_monitor::YamlCarMeta, Option<&'a CarSnapshot>)> {
    let telemetry_by_idx: HashMap<i32, &CarSnapshot> =
        telemetry.iter().map(|car| (car.idx, car)).collect();

    roster
        .iter()
        .filter(|meta| !meta.is_pace)
        .map(|meta| (meta, telemetry_by_idx.get(&meta.idx).copied()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        best_positive_lap, history_matches_subsession, roster_with_telemetry, tower_order_key,
    };
    use crate::iracing_sdk::{race_monitor::YamlCarMeta, CarSnapshot};

    #[test]
    fn best_positive_lap_uses_recorded_when_live_is_absent_or_zero() {
        assert_eq!(best_positive_lap(None, Some(82.4)), 82.4);
        assert_eq!(best_positive_lap(Some(0.0), Some(82.4)), 82.4);
    }

    #[test]
    fn best_positive_lap_uses_live_when_recorded_is_absent_or_zero() {
        assert_eq!(best_positive_lap(Some(81.7), None), 81.7);
        assert_eq!(best_positive_lap(Some(81.7), Some(0.0)), 81.7);
    }

    #[test]
    fn best_positive_lap_chooses_the_lower_positive_time() {
        assert_eq!(best_positive_lap(Some(81.7), Some(82.4)), 81.7);
        assert_eq!(best_positive_lap(Some(82.4), Some(81.7)), 81.7);
    }

    #[test]
    fn best_positive_lap_returns_zero_without_a_positive_time() {
        assert_eq!(best_positive_lap(None, None), 0.0);
        assert_eq!(best_positive_lap(Some(0.0), Some(0.0)), 0.0);
    }

    #[test]
    fn history_match_exige_ids_positivos_e_iguais() {
        assert!(history_matches_subsession(4242, 4242));
        assert!(!history_matches_subsession(4242, 4243));
        assert!(!history_matches_subsession(0, 4242));
        assert!(!history_matches_subsession(4242, 0));
        assert!(!history_matches_subsession(0, 0));
    }

    #[test]
    fn roster_with_telemetry_keeps_all_non_pace_cars_and_joins_available_snapshot() {
        let roster = vec![
            YamlCarMeta {
                idx: 1,
                is_ai: true,
                is_pace: false,
                class_id: 10,
                car_number: 11,
            },
            YamlCarMeta {
                idx: 2,
                is_ai: true,
                is_pace: false,
                class_id: 10,
                car_number: 22,
            },
            YamlCarMeta {
                idx: 3,
                is_ai: false,
                is_pace: true,
                class_id: 10,
                car_number: 0,
            },
        ];
        let telemetry = vec![CarSnapshot {
            idx: 2,
            class_position: 1,
            ..CarSnapshot::default()
        }];

        let joined = roster_with_telemetry(&roster, &telemetry);
        let joined_indices: Vec<(i32, Option<i32>)> = joined
            .into_iter()
            .map(|(meta, car)| (meta.idx, car.map(|snapshot| snapshot.idx)))
            .collect();

        assert_eq!(joined_indices, vec![(1, None), (2, Some(2))]);
    }

    #[test]
    fn tower_order_key_puts_classified_cars_first() {
        let classified = tower_order_key(12, &(i64::MAX, 99));
        let unclassified = tower_order_key(0, &(80_000, 1));

        assert!(classified < unclassified);
    }

    #[test]
    fn tower_order_key_sorts_classified_cars_by_official_position() {
        let first = tower_order_key(1, &(i64::MAX, 99));
        let second = tower_order_key(2, &(70_000, 1));

        assert!(first < second);
    }

    #[test]
    fn tower_order_key_sorts_unclassified_cars_by_best_qualifying_lap() {
        let faster = tower_order_key(0, &(79_999, 99));
        let slower = tower_order_key(0, &(80_000, 1));

        assert!(faster < slower);
    }

    #[test]
    fn tower_order_key_uses_car_number_as_tiebreaker_and_fallback() {
        let lower_number_with_same_lap = tower_order_key(0, &(80_000, 7));
        let higher_number_with_same_lap = tower_order_key(0, &(80_000, 12));
        let lower_number_without_lap = tower_order_key(0, &(i64::MAX, 7));
        let higher_number_without_lap = tower_order_key(0, &(i64::MAX, 12));

        assert!(lower_number_with_same_lap < higher_number_with_same_lap);
        assert!(lower_number_without_lap < higher_number_without_lap);
    }
}

#[tauri::command]
pub fn get_overlay_data(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Option<OverlayData>, String> {
    use tauri::Manager;

    // ── PISTA: telemetria ao vivo. Sem sessão → overlay oculto. ──
    let tele = match crate::iracing_sdk::read_telemetry() {
        Ok(t) => t,
        Err(e) => {
            dbg_state(&format!("telemetria_falhou: {e:?}"));
            return Ok(None);
        }
    };
    // O YAML é a fonte do roster; `tele.cars` contém apenas carros presentes no
    // mundo e, antes da sessão, pode estar vazio mesmo com todo o grid conhecido.
    let feedback = race_monitor::get_feedback();
    let roster_cars = roster_with_telemetry(&feedback.cars_yaml_meta, &tele.cars);
    if roster_cars.is_empty() {
        dbg_state(&format!(
            "roster_sem_carros_elegiveis: tele_cars={} meta={}",
            tele.cars.len(),
            feedback.cars_yaml_meta.len()
        ));
        return Ok(None);
    }
    let eligible_meta_count = roster_cars.len();

    let history = race_monitor::get_history();
    let qualy_laps = race_monitor::get_qualy_laps();
    let current_subsession_id = race_monitor::get_subsession_id();
    let history_is_current =
        history_matches_subsession(history.subsession_id, current_subsession_id);
    let is_green = race_monitor::poll().is_green;

    // Posição no grid (delta desde a largada) vem do histórico, quando já existe.
    let grid_by_idx: HashMap<i32, i32> = if history_is_current {
        history
            .cars_meta
            .iter()
            .map(|m| (m.idx, m.grid_class_position))
            .collect()
    } else {
        HashMap::new()
    };

    // Estratégia de pneu por CarIdx (stints + nº de trocas).
    let tire_by_idx: HashMap<i32, tire_strategy::CarTireStrategy> = if history_is_current {
        tire_strategy::infer_all(&history.pit_stops, history.weather.clone())
            .into_iter()
            .map(|s| (s.car_idx, s))
            .collect()
    } else {
        HashMap::new()
    };

    // ── CARREIRA: banco + números ──
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| {
        dbg_state(&format!("db_falhou: {}", db_path.display()));
        format!("Falha ao abrir banco: {e}")
    })?;

    let numbers: HashMap<String, i64> =
        std::fs::read_to_string(super::iracing::numbers_path(&base_dir, &career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: HashMap<i64, String> = numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let player_driver = dq::get_player_driver(&db.conn).ok();

    // Categoria do evento = a do time do jogador (via contrato ativo).
    let category = player_driver
        .as_ref()
        .and_then(|p| cq::get_active_contract_for_pilot(&db.conn, &p.id).ok().flatten())
        .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
        .map(|t| t.categoria)
        .or_else(|| player_driver.as_ref().and_then(|p| p.categoria_atual.clone()))
        .unwrap_or_default();
    let is_endurance = category == "endurance";

    // Resolve driver_id → (nome, pontos, time, cor).
    let resolve = |driver_id: &str| -> Option<(String, i32, String, String)> {
        let d = dq::get_driver(&db.conn, driver_id).ok()?;
        let team = cq::get_active_contract_for_pilot(&db.conn, driver_id)
            .ok()
            .flatten()
            .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten());
        let team_name = team.as_ref().map(|t| t.nome.clone()).unwrap_or_default();
        let color = team
            .as_ref()
            .map(|t| t.cor_primaria.clone())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| "#7d8590".to_string());
        Some((d.nome, d.stats_temporada.pontos.round() as i32, team_name, color))
    };

    let player_idx = tele.player_car_idx;
    let player_black = (tele.session_flags as u32 & 0x0001_0000) != 0;

    // Tipo de sessão (do YAML) — necessário ANTES do loop: o grid da qualy só é
    // usado pra ordenar fora de treino livre ("P"), onde o `qualy_laps` pode ser
    // resto de outro fim de semana.
    let yaml = crate::iracing_sdk::read_session()
        .map(|s| s.session_yaml)
        .unwrap_or_default();
    let kind = session_kind(&parse_session_types(&yaml), tele.session_num);

    // Melhor volta registrada da sessão atual: quali alimenta apenas Q, histórico
    // da corrida alimenta apenas R. Treino não reutiliza dados potencialmente stale.
    let recorded_laps = match kind {
        "Q" => qualy_laps.as_slice(),
        "R" if history_is_current => history.car_laps.as_slice(),
        _ => &[],
    };
    let mut recorded_best_lap: HashMap<i32, f64> = HashMap::new();
    for lap in recorded_laps {
        if lap.time > 0.0 {
            recorded_best_lap
                .entry(lap.car_idx)
                .and_modify(|secs| *secs = secs.min(lap.time))
                .or_insert(lap.time);
        }
    }

    // Melhor volta da QUALI por carro (ms) = grid pra ordenar quem ainda não tem
    // posição oficial. Em treino: vazio → a ordenação cai direto no nº do carro.
    let qualy_best_ms: HashMap<i32, i64> = if kind != "P" {
        let mut best: HashMap<i32, i64> = HashMap::new();
        for lap in &qualy_laps {
            if lap.time > 0.0 {
                let ms = (lap.time * 1000.0).round() as i64;
                best.entry(lap.car_idx)
                    .and_modify(|v| *v = (*v).min(ms))
                    .or_insert(ms);
            }
        }
        best
    } else {
        HashMap::new()
    };

    // ── Monta carros, agrupados por classe (class_id) ──
    // (carro, melhor_volta_s, chave_pra_nao_classificado)
    let mut by_class: HashMap<i64, Vec<(OverlayCar, f64, (i64, i64))>> = HashMap::new();
    let mut class_label: HashMap<i64, String> = HashMap::new();
    // Diagnóstico: por que cada carro ficou de fora (alguma porta está zerando tudo).
    let mut sem_pos_incluido = 0usize;
    let mut skip_sem_piloto = 0usize;

    for (meta, car) in roster_cars {
        // Sem posição oficial (pré-sessão/sem volta cronometrada) o carro ENTRA
        // assim mesmo — ordenado depois por grid da qualy → número, pra torre
        // existir desde a qualy. `delta`/`gain` só fazem sentido com posição.
        let class_pos = car.map(|snapshot| snapshot.class_position).unwrap_or(0);
        let best_lap = best_positive_lap(
            car.map(|snapshot| snapshot.best_lap_time),
            recorded_best_lap.get(&meta.idx).copied(),
        );

        let is_player = meta.idx == player_idx;
        let driver_id = if is_player {
            player_driver.as_ref().map(|d| d.id.clone())
        } else {
            by_number.get(&(meta.car_number as i64)).cloned()
        };
        let (name, points, team, color) = match driver_id.as_deref().and_then(resolve) {
            Some(t) => t,
            None => {
                skip_sem_piloto += 1;
                continue; // não resolveu no nosso elenco → fora
            }
        };
        if class_pos <= 0 {
            sem_pos_incluido += 1;
        }

        let strat = tire_by_idx.get(&meta.idx);
        let tire_history: Vec<String> = strat
            .map(|s| s.stints.iter().map(|st| compound_str(st.compound).to_string()).collect())
            .unwrap_or_default();
        let stops = strat.map(|s| s.tire_changes).unwrap_or(0);

        let grid_pos = grid_by_idx.get(&meta.idx).copied().unwrap_or(0);
        let delta = if class_pos > 0 && grid_pos > 0 {
            grid_pos - class_pos
        } else {
            0
        };
        let gain = if class_pos > 0 {
            scoring::get_points_for_position(class_pos.clamp(0, 255) as u8, is_endurance) as i32
        } else {
            0
        };

        class_label
            .entry(meta.class_id)
            .or_insert_with(|| feedback.class_names.get(&meta.class_id).cloned().unwrap_or_default());

        // Chave p/ não-classificado: melhor tempo da qualy (grid) → nº do carro.
        let unclass_key = (
            qualy_best_ms.get(&meta.idx).copied().unwrap_or(i64::MAX),
            meta.car_number as i64,
        );

        by_class.entry(meta.class_id).or_default().push((
            OverlayCar {
                pos: class_pos.max(0),
                name,
                team,
                color,
                delta,
                stops,
                tire_history,
                points,
                gain,
                fastest: fmt_lap(best_lap),
                fol: false, // definido após agrupar
                pit: car.map(|snapshot| snapshot.on_pit_road).unwrap_or(false),
                flag: if is_player && player_black {
                    Some("black".to_string())
                } else {
                    None
                },
                player: is_player,
            },
            best_lap,
            unclass_key,
        ));
    }

    if by_class.is_empty() {
        dbg_state(&format!(
            "by_class_vazio: tele_cars={} meta_elegivel={} roster={} player_idx={} | skips: sem_piloto={}",
            tele.cars.len(),
            eligible_meta_count,
            by_number.len(),
            player_idx,
            skip_sem_piloto
        ));
        return Ok(None);
    }

    // Ordena cada classe (posição oficial → grid da qualy → número) e marca a
    // volta mais rápida da classe.
    let mut classes: Vec<OverlayClass> = Vec::new();
    for (class_id, mut cars) in by_class {
        cars.sort_by_key(|(c, _, key)| tower_order_key(c.pos, key));
        // volta mais rápida da classe (menor tempo positivo)
        if let Some((best_i, _)) = cars
            .iter()
            .enumerate()
            .filter(|(_, (_, secs, _))| *secs > 0.0)
            .min_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap())
        {
            cars[best_i].0.fol = true;
        }
        let label = class_label.get(&class_id).cloned().unwrap_or_default();
        let id = label.trim().to_lowercase().replace(' ', "");
        classes.push(OverlayClass {
            id,
            label: label.trim().to_uppercase(),
            cars: cars.into_iter().map(|(c, _, _)| c).collect(),
        });
    }

    // Volta do líder = maior volta completa + 1. Voltas totais = concluídas do
    // líder + estimativa do iRacing (vale em corrida por tempo).
    let max_completed = tele.cars.iter().map(|c| c.lap_completed).max().unwrap_or(0);
    let lead_lap = max_completed + 1;
    let total_laps = if tele.session_laps_remain_ex > 0 && tele.session_laps_remain_ex < 10_000 {
        max_completed + tele.session_laps_remain_ex
    } else {
        0
    };

    // (o tipo de sessão — `kind` — já foi resolvido antes do loop de carros)
    let checkered = tele.session_state == 5;
    let flag = if checkered {
        "checkered"
    } else if is_green {
        "green"
    } else {
        "yellow"
    };

    let air_temp = if tele.air_temp > 0.0 {
        Some(tele.air_temp.round() as i32)
    } else {
        None
    };

    let session = OverlaySession {
        kind: kind.to_string(),
        lap: lead_lap,
        total_laps,
        flag: flag.to_string(),
        category,
        weather: OverlayWeather {
            condition: wetness_to_condition(tele.track_wetness).to_string(),
            air_temp,
        },
    };

    dbg_state(&format!(
        "ok: {} classes, {} carros ({} sem posição)",
        classes.len(),
        classes.iter().map(|c| c.cars.len()).sum::<usize>(),
        sem_pos_incluido
    ));
    Ok(Some(OverlayData { session, classes }))
}
