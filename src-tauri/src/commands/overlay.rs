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
    flag: Option<String>, // "black" = DNF (!dq); só do jogador ao vivo, IA vem do motor de quebra
    player: bool,
    /// Papel de rivalidade relativo ao jogador: "nemesis" | "rival" | None. Alimenta
    /// o marcador 💥/🔥 ao lado do nome na torre.
    rival_role: Option<String>,
    /// Alerta de QUEBRA de peça pendente: "light" (triângulo laranja) | "heavy"
    /// (vermelho) | None. Some quando o carro sai do box reparado. Preenchido pelo
    /// motor de quebra ao vivo (wiring da grade toda — fase 2).
    alert: Option<String>,
    /// Tempo PARADO no box (s) da última parada, exposto só nas ~3 voltas seguintes
    /// (badge sobre os pneus). None fora dessa janela. Fonte: tire_strategy.
    pit_secs: Option<i32>,
    /// Ícones da coluna de paradas: 1º = pneu de largada, depois um por PARADA — "dry"/"wet"
    /// (troca de pneu), "fuel" (só abasteceu), "part" (reparo de peça → triângulo no lugar do
    /// pneu). Diferencia POR QUE o piloto parou. Fonte: tire_strategy + voltas de reparo.
    pit_icons: Vec<String>,
    /// FLASH: a linha do piloto pisca por ~5 s quando ele acabou de quebrar (em sincronia com
    /// o rádio do engenheiro). Fonte: `race_monitor::get_breakdown_flashes`.
    flash: bool,
    /// Composto de pneu ESCOLHIDO pelo carro (`CarIdxTireCompound`): índice 0-based por
    /// série, -1 = desconhecido. Preenchido inclusive pra IA e ANTES da largada — a mesma
    /// info que o RaceLab expõe. O mapa índice→nome fica na UI (é por série).
    tire_compound: i32,
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

/// O histórico capturado é da MESMA sessão que está rolando agora?
///
/// Igualdade simples — cobre os dois mundos:
///   • ONLINE/hosted: `WeekendInfo:SubSessionID` > 0, único por evento.
///   • OFFLINE (aiseason de IA — o caso do jogo): o iRacing manda SubSessionID = **0**.
///
/// Antes exigíamos `> 0`, então offline caía em `0 == 0` → **false**, e o overlay descartava
/// TODO o histórico ao vivo: sem `grid_by_idx` (delta sempre "— 0") e sem `tire_by_idx`
/// (a coluna de pneu nunca acumulava as paradas). O histórico já é resetado por
/// tentativa/`session_num` (`record_history`), então confiar no id igual — inclusive 0 —
/// é seguro contra dados de uma corrida anterior.
fn history_matches_subsession(history_id: i64, current_id: i64) -> bool {
    history_id == current_id
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
    fn history_match_exige_ids_iguais_online_e_offline() {
        // Online: ids iguais e positivos casam; diferentes não.
        assert!(history_matches_subsession(4242, 4242));
        assert!(!history_matches_subsession(4242, 4243));
        // Transição de evento online (histórico ainda não resetado) NÃO casa.
        assert!(!history_matches_subsession(0, 4242));
        assert!(!history_matches_subsession(4242, 0));
        // OFFLINE (aiseason de IA): SubSessionID = 0 nos dois lados → CASA. Sem isto o
        // overlay descartava grid/delta e os ícones de pneu na corrida de IA.
        assert!(history_matches_subsession(0, 0));
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

    // Papéis de rivalidade do jogador (Nemesis/Rivais) → marcador na torre.
    let rival_roles: std::collections::HashMap<String, &'static str> = {
        let current =
            crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
        let interests =
            crate::commands::career::select_player_interests(&db.conn, current.as_deref());
        let mut m = std::collections::HashMap::new();
        if let Some(n) = &interests.nemesis {
            m.insert(n.driver_id.clone(), "nemesis");
        }
        for r in &interests.rivais {
            m.insert(r.driver_id.clone(), "rival");
        }
        m
    };

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

    // Carro DESTACADO na torre = quem a câmera assiste (`CamCarIdx`). No replay/spectate
    // segue quem você olha; dirigindo normal a câmera fica no seu carro, então isto vira
    // o próprio jogador. Só vale se for um carro real do grid (não o pace, presente no
    // roster); senão cai no carro do jogador. Identidade/black flag continuam no jogador.
    let cam_idx = tele.cam_car_idx;
    let cam_is_valid =
        cam_idx >= 0 && feedback.cars_yaml_meta.iter().any(|m| m.idx == cam_idx && !m.is_pace);
    let focus_idx = if cam_is_valid { cam_idx } else { player_idx };

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

    // Volta do líder AGORA — janela do tracker de tempo de pit (maior volta completa
    // + 1). O total de voltas/estado do cabeçalho é recalculado no fim.
    let lead_lap_now = tele.cars.iter().map(|c| c.lap_completed).max().unwrap_or(0) + 1;

    // Alertas de quebra ao vivo por car_idx (triângulo laranja/vermelho; DNF → bandeira preta).
    let breakdown_by_idx: HashMap<i32, &'static str> =
        race_monitor::get_breakdown_alerts().into_iter().collect();
    // Voltas de reparo de peça por car_idx (→ ícone "part" no lugar do pneu na coluna de paradas).
    let repair_laps_by_idx: HashMap<i32, Vec<u32>> = {
        let mut m: HashMap<i32, Vec<u32>> = HashMap::new();
        for (idx, lap) in race_monitor::get_breakdown_repair_laps() {
            m.entry(idx).or_default().push(lap);
        }
        m
    };
    // car_idx que devem PISCAR agora (quebrou nos últimos 5 s).
    let flash_idxs: std::collections::HashSet<i32> =
        race_monitor::get_breakdown_flashes().into_iter().collect();

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

        // `is_self` = o carro do JOGADOR (resolve nome pela identidade do save + dona da
        // black flag ao vivo). `is_focused` = o carro ASSISTIDO na câmera (a linha em
        // destaque). Dirigindo normal os dois coincidem; no replay o destaque migra.
        let is_self = meta.idx == player_idx;
        let is_focused = meta.idx == focus_idx;
        let driver_id = if is_self {
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

        // Tracker de tempo de pit: última parada do carro, só nas ~3 voltas seguintes.
        let pit_secs = strat.and_then(|s| s.stops.last()).and_then(|last| {
            let since = lead_lap_now - last.lap;
            (0..=3).contains(&since).then(|| last.box_secs.round() as i32)
        });

        // Alerta de quebra: "light"/"heavy" → triângulo; "dnf" → bandeira preta.
        let bd = breakdown_by_idx.get(&meta.idx).copied();
        let alert = match bd {
            Some("light") => Some("light".to_string()),
            Some("heavy") => Some("heavy".to_string()),
            _ => None,
        };

        // Ícones da coluna de paradas: pneu de largada, depois um por parada (troca/abastece/repara).
        let pit_icons: Vec<String> = strat
            .map(|s| {
                let reps = repair_laps_by_idx.get(&meta.idx);
                let mut v = vec![compound_str(s.start_compound).to_string()];
                for stop in &s.stops {
                    let is_repair = reps.is_some_and(|ls| {
                        ls.iter().any(|&l| (l as i32 - stop.lap).abs() <= 1)
                    });
                    let icon = if is_repair {
                        "part"
                    } else if stop.tire_change {
                        if stop.track_wet {
                            "wet"
                        } else {
                            "dry"
                        }
                    } else {
                        "fuel"
                    };
                    v.push(icon.to_string());
                }
                v
            })
            .unwrap_or_default();

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
                // DNF (quebra) → bandeira preta pra grade toda; senão, black flag viva do jogador.
                flag: if bd == Some("dnf") || (is_self && player_black) {
                    Some("black".to_string())
                } else {
                    None
                },
                player: is_focused,
                rival_role: driver_id
                    .as_deref()
                    .and_then(|id| rival_roles.get(id).map(|s| s.to_string())),
                alert,
                pit_secs,
                pit_icons,
                flash: flash_idxs.contains(&meta.idx),
                tire_compound: car.map(|snapshot| snapshot.tire_compound).unwrap_or(-1),
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

// ───────────────────────── RÁDIO DA EQUIPE (feed de quebras) ─────────────────────────

/// Uma mensagem do overlay do engenheiro sobre uma quebra na grade.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownMessage {
    /// Índice no log da corrida — cursor crescente pro front saber o que é NOVO.
    id: usize,
    /// "light" | "heavy" | "dnf" — tinge o acento do card.
    severity: String,
    /// Frase principal (voz do engenheiro, 3ª pessoa sobre o piloto).
    text: String,
    /// Detalhe concreto do problema (o `problem_label`), como subtítulo.
    detail: String,
}

/// Peça com preposição/artigo pra frase do engenheiro ("no motor", "na suspensão").
fn part_com_artigo(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "no motor",
        "gearbox" => "no câmbio",
        "brakes" => "nos freios",
        "suspension" => "na suspensão",
        "cooling" => "no arrefecimento",
        "front_wing" => "na asa dianteira",
        "rear_wing" => "na asa traseira",
        "sidepods" => "nas laterais",
        "underbody" => "no assoalho",
        "chassis" => "no chassi",
        "electronics" => "na parte elétrica",
        _ => "no carro",
    }
}

/// FONTE ÚNICA das redações do rádio: 3 opções por (peça, severidade), voz do engenheiro,
/// 3ª pessoa. Devolve o TRECHO depois do nome (a peça já vem escrita no texto). A causa
/// concreta (`detail`) é anexada pelo card na mesma linha. Peça/severidade desconhecida cai
/// num genérico. O DNF é tratado à parte (`dnf_frase`), pois a redação é diferente.
fn breakdown_frases(part_key: &str, severity: &str) -> [&'static str; 3] {
    match (severity, part_key) {
        // ── MOTOR ──
        ("light", "engine") => [
            "sente o motor perdendo fôlego",
            "relata o motor engasgando",
            "avisa que o motor não está redondo",
        ],
        ("heavy", "engine") => [
            "está com o motor em pane",
            "perdeu potência e o motor pode não aguentar",
            "relata o motor no limite — situação séria",
        ],
        // ── CÂMBIO ──
        ("light", "gearbox") => [
            "está com o câmbio arisco nas trocas",
            "relata engates falhando no câmbio",
            "sente o câmbio embolando as marchas",
        ],
        ("heavy", "gearbox") => [
            "está com o câmbio travando",
            "perdeu marchas — o câmbio está indo embora",
            "relata o câmbio prestes a parar",
        ],
        // ── FREIOS ──
        ("light", "brakes") => [
            "sente o pedal de freio amolecendo",
            "relata os freios pedindo água",
            "avisa que os freios estão longos",
        ],
        ("heavy", "brakes") => [
            "está praticamente sem freio",
            "relata os freios cozinhando",
            "perdeu o pedal — freios em pane",
        ],
        // ── SUSPENSÃO ──
        ("light", "suspension") => [
            "sente a suspensão reclamando nas zebras",
            "relata o carro batendo demais atrás",
            "avisa de uma folga na suspensão",
        ],
        ("heavy", "suspension") => [
            "está com a suspensão comprometida",
            "relata algo quebrado na suspensão",
            "perdeu firmeza — suspensão em pane",
        ],
        // ── ARREFECIMENTO ──
        ("light", "cooling") => [
            "vê a temperatura subindo aos poucos",
            "relata o arrefecimento no limite",
            "avisa que a água está esquentando",
        ],
        ("heavy", "cooling") => [
            "está com o arrefecimento estourando",
            "relata superaquecimento crítico",
            "perdeu o arrefecimento — temperatura no vermelho",
        ],
        // ── ASA DIANTEIRA ──
        ("light", "front_wing") => [
            "relata a asa dianteira leve",
            "sente o bico perdendo apoio",
            "avisa de dano na asa dianteira",
        ],
        ("heavy", "front_wing") => [
            "está com a asa dianteira danificada",
            "perdeu apoio na frente — asa comprometida",
            "relata a asa dianteira se soltando",
        ],
        // ── ASA TRASEIRA ──
        ("light", "rear_wing") => [
            "sente a traseira solta na reta",
            "relata a asa traseira leve",
            "avisa de dano na asa traseira",
        ],
        ("heavy", "rear_wing") => [
            "está com a asa traseira danificada",
            "perdeu apoio atrás — traseira nervosa",
            "relata a asa traseira cedendo",
        ],
        // ── LATERAIS ──
        ("light", "sidepods") => [
            "relata dano nas laterais",
            "sente o carro puxando de lado",
            "avisa de um amassado nas laterais",
        ],
        ("heavy", "sidepods") => [
            "está com as laterais abertas",
            "perdeu parte da lateral — carro ferido",
            "relata dano sério nas laterais",
        ],
        // ── ASSOALHO ──
        ("light", "underbody") => [
            "sente o assoalho raspando",
            "relata perda de apoio no assoalho",
            "avisa de dano no assoalho",
        ],
        ("heavy", "underbody") => [
            "está com o assoalho comprometido",
            "perdeu downforce — assoalho ferido",
            "relata o assoalho arrastando forte",
        ],
        // ── CHASSI ──
        ("light", "chassis") => [
            "sente o chassi estranho",
            "relata o carro desalinhado",
            "avisa de algo torto no chassi",
        ],
        ("heavy", "chassis") => [
            "está com o chassi comprometido",
            "relata dano estrutural no chassi",
            "perdeu rigidez — chassi ferido",
        ],
        // ── PARTE ELÉTRICA ──
        ("light", "electronics") => [
            "relata oscilações na parte elétrica",
            "sente o painel piscando",
            "avisa de falha elétrica intermitente",
        ],
        ("heavy", "electronics") => [
            "está com pane elétrica",
            "perdeu comandos — parte elétrica em pane",
            "relata o carro cortando — falha elétrica",
        ],
        // ── genérico ──
        ("heavy", _) => [
            "está com um problema grave no carro",
            "relata um problema sério no carro",
            "perdeu desempenho — algo grave no carro",
        ],
        _ => [
            "apresenta um problema no carro",
            "relata algo estranho no carro",
            "avisa de um problema no carro",
        ],
    }
}

/// Redação de ABANDONO (DNF) — 3 opções. Usa a peça com preposição (`part_com_artigo`).
fn dnf_frase(name: &str, part: &str, variant: usize) -> String {
    match variant % 3 {
        0 => format!("{name} está fora — problemas {part}"),
        1 => format!("{name} abandona a corrida com problemas {part}"),
        _ => format!("{name} foi retirado da corrida — problemas {part}"),
    }
}

/// Causa de exemplo por peça (só pro DEMO — na corrida real vem do `problem_label`).
fn demo_causa(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "superaquecimento",
        "gearbox" => "engrenagem gasta",
        "brakes" => "freios superaquecidos",
        "suspension" => "braço trincado",
        "cooling" => "vazamento de água",
        "front_wing" => "toque na dianteira",
        "rear_wing" => "flap solto",
        "sidepods" => "batida na lateral",
        "underbody" => "assoalho danificado",
        "chassis" => "dano estrutural",
        "electronics" => "falha no chicote",
        _ => "",
    }
}

/// Nomes REAIS do grid da sessão atual (roster do monitor → número → nosso elenco), pra
/// o demo usar pilotos de verdade. `None`/vazio quando não há sessão/save → o tour cai
/// nos nomes fictícios.
fn session_driver_names(app: &tauri::AppHandle, career_id: &str) -> Vec<String> {
    use tauri::Manager;
    let Ok(base_dir) = app.path().app_data_dir() else {
        return Vec::new();
    };
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let Ok(db) = Database::open_existing(&db_path) else {
        return Vec::new();
    };
    let numbers: HashMap<String, i64> =
        std::fs::read_to_string(super::iracing::numbers_path(&base_dir, career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: HashMap<i64, String> = numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let feedback = race_monitor::get_feedback();
    let mut names = Vec::new();
    for meta in &feedback.cars_yaml_meta {
        if meta.is_pace {
            continue;
        }
        if let Some(id) = by_number.get(&(meta.car_number as i64)) {
            if let Ok(d) = dq::get_driver(&db.conn, id) {
                names.push(d.nome);
            }
        }
    }
    names
}

/// Monta o TOUR de exemplos do demo a partir da fonte única (`breakdown_frases`): toda
/// peça × leve/grave × 3 redações + alguns abandonos. Usa `real_names` (pilotos do grid)
/// se houver; senão, nomes fictícios. É o que o overlay cicla no modo demo.
fn demo_tour(real_names: &[String]) -> Vec<BreakdownMessage> {
    const FALLBACK: [&str; 6] = [
        "Ryan Jones",
        "Hunter Jackson",
        "Logan Garcia",
        "Adrian Alvarez",
        "Nathan Brown",
        "Camila Monteiro",
    ];
    const PARTS: [&str; 11] = [
        "engine", "gearbox", "brakes", "suspension", "cooling", "front_wing", "rear_wing",
        "sidepods", "underbody", "chassis", "electronics",
    ];
    let name_at = |i: usize| -> String {
        if real_names.is_empty() {
            FALLBACK[i % FALLBACK.len()].to_string()
        } else {
            real_names[i % real_names.len()].clone()
        }
    };
    let mut out = Vec::new();
    let mut ni = 0usize;
    for part in PARTS {
        for sev in ["light", "heavy"] {
            for frase in breakdown_frases(part, sev) {
                let name = name_at(ni);
                ni += 1;
                out.push(BreakdownMessage {
                    id: out.len(),
                    severity: sev.to_string(),
                    text: format!("{name} {frase}"),
                    detail: demo_causa(part).to_string(),
                });
            }
        }
    }
    for v in 0..3 {
        let name = name_at(ni);
        ni += 1;
        out.push(BreakdownMessage {
            id: out.len(),
            severity: "dnf".to_string(),
            text: dnf_frase(&name, part_com_artigo("gearbox"), v),
            detail: String::new(),
        });
    }
    out
}

/// Lista completa de exemplos do demo (a janela do rádio busca uma vez e cicla localmente).
/// Usa os pilotos REAIS do grid da carreira quando há sessão; senão, nomes fictícios.
#[tauri::command]
pub fn overlay_demo_messages(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<BreakdownMessage>, String> {
    Ok(demo_tour(&session_driver_names(&app, &career_id)))
}

/// Modo DEMO do rádio (VR / fallback): cicla o tour pelo tempo. Troca a cada ~5 s; o `id`
/// cresce, então o front reconhece como novo. Ligado pelo botão das Configurações ou env.
fn demo_breakdown_feed(real_names: &[String]) -> Vec<BreakdownMessage> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let tour = demo_tour(real_names);
    if tour.is_empty() {
        return Vec::new();
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let step = (secs / 5) as usize; // avança um exemplo a cada 5 s
    let mut m = tour[step % tour.len()].clone();
    m.id = step; // id crescente → o front troca o card
    vec![m]
}

// ───────────────────── AVISO PESSOAL (peça do jogador na zona de risco) ─────────────────────

/// Peça com preposição na 2ª pessoa ("no seu motor", "na sua suspensão") — pro engenheiro.
fn part_com_seu(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "no seu motor",
        "gearbox" => "no seu câmbio",
        "brakes" => "nos seus freios",
        "suspension" => "na sua suspensão",
        "cooling" => "no seu arrefecimento",
        "front_wing" => "na sua asa dianteira",
        "rear_wing" => "na sua asa traseira",
        "sidepods" => "nas suas laterais",
        "underbody" => "no seu assoalho",
        "chassis" => "no seu chassi",
        "electronics" => "na sua parte elétrica",
        _ => "no seu carro",
    }
}

/// Monta o AVISO pessoal na VOZ DO ENGENHEIRO (rádio, 1ª pessoa dele → você). Sem número:
/// ele "ouve algo estranho" e alerta que a peça pode dar problema a qualquer momento. 3
/// variações; severidade "warn" → card distinto no front.
fn player_warning_msg(part_key: &str, id: usize) -> BreakdownMessage {
    let onde = part_com_seu(part_key);
    // (abertura, alerta) — a peça (`onde`) é encaixada na abertura.
    let variants: [(&str, &str); 3] = [
        ("Estou ouvindo algo estranho", "pode dar problema a qualquer momento"),
        ("Não gostei de um barulho", "fica de olho — pode falhar a qualquer hora"),
        ("Tem algo esquisito acontecendo", "risco de pane a qualquer momento"),
    ];
    let (open, detail) = variants[id % variants.len()];
    BreakdownMessage {
        id,
        severity: "warn".to_string(),
        text: format!("{open} {onde}"),
        detail: detail.to_string(),
    }
}

/// Tour de avisos do demo: um por peça (desgaste 95–100%). Fonte única do card de aviso.
fn demo_player_warnings() -> Vec<BreakdownMessage> {
    const PARTS: [&str; 11] = [
        "engine", "gearbox", "brakes", "suspension", "cooling", "front_wing", "rear_wing",
        "sidepods", "underbody", "chassis", "electronics",
    ];
    PARTS
        .iter()
        .enumerate()
        .map(|(i, p)| player_warning_msg(p, i))
        .collect()
}

/// AVISOS pessoais do jogador (peça DELE entrou na zona de risco) — o overlay mostra num card
/// DISTINTO (2ª pessoa). Lê o log vivo do monitor. No demo, cicla o tour pelo tempo.
#[tauri::command]
pub fn get_player_warnings() -> Result<Vec<BreakdownMessage>, String> {
    if crate::commands::overlay_window::demo_enabled() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let tour = demo_player_warnings();
        if tour.is_empty() {
            return Ok(Vec::new());
        }
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let step = (secs / 6) as usize; // troca a cada 6 s
        let mut m = tour[step % tour.len()].clone();
        m.id = step;
        return Ok(vec![m]);
    }
    let warns = race_monitor::peek_player_warnings();
    Ok(warns
        .iter()
        .enumerate()
        .map(|(i, w)| player_warning_msg(w.part, i))
        .collect())
}

/// Lista completa dos avisos de exemplo (a janela do rádio busca uma vez e cicla localmente).
#[tauri::command]
pub fn overlay_demo_warnings() -> Vec<BreakdownMessage> {
    demo_player_warnings()
}

/// Feed do RÁDIO DA EQUIPE: as quebras da corrida em andamento viram frases do engenheiro
/// (piloto resolvido pelo número → nosso elenco). O front mostra as NOVAS (por `id`). Não
/// depende de telemetria — lê o log vivo do monitor + o banco da carreira.
#[tauri::command]
pub fn get_breakdown_feed(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<BreakdownMessage>, String> {
    use tauri::Manager;

    // Modo demo: mostra templates ciclando (pra posicionar/ver o overlay do rádio), com
    // os pilotos REAIS do grid. Ligado pelo botão das Configurações OU pela env.
    if crate::commands::overlay_window::demo_enabled() {
        return Ok(demo_breakdown_feed(&session_driver_names(&app, &career_id)));
    }

    let log = race_monitor::peek_breakdown_log();
    if log.is_empty() {
        return Ok(Vec::new());
    }

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let numbers: HashMap<String, i64> =
        std::fs::read_to_string(super::iracing::numbers_path(&base_dir, &career_id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
    let by_number: HashMap<i64, String> = numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let out = log
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let name = by_number
                .get(&(o.car_number as i64))
                .and_then(|id| dq::get_driver(&db.conn, id).ok())
                .map(|d| d.nome)
                .unwrap_or_else(|| format!("Carro #{}", o.car_number));
            // Redação DIRETA a partir da fonte única (`breakdown_frases`): 3 opções por
            // (peça, severidade). Variante ESTÁVEL por carro+ordem (mesma quebra → mesma
            // frase; a grade varia). A causa (`detail`) segue na mesma linha no card.
            let variant = (o.car_number as usize).wrapping_add(i) % 3;
            let text = if o.severity == "dnf" {
                dnf_frase(&name, part_com_artigo(&o.part), variant)
            } else {
                format!("{name} {}", breakdown_frases(&o.part, &o.severity)[variant])
            };
            BreakdownMessage {
                id: i,
                severity: o.severity.clone(),
                text,
                detail: o.label.clone(),
            }
        })
        .collect();

    Ok(out)
}
