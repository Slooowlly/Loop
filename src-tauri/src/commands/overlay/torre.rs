//! Torre de tempos AO VIVO: cruza pista (SDK), carreira (save) e sessão.

use std::collections::HashMap;

use serde::Serialize;

use super::formato::{
    best_positive_lap, compound_str, dbg_state, fmt_lap, history_matches_subsession, name_key,
    parse_session_types, roster_with_telemetry, session_kind, tower_order_key,
    wetness_to_condition, NEUTRAL_TEAM_COLOR,
};
use crate::config::app_config::AppConfig;
use crate::constants::scoring;
use crate::db::connection::Database;
use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
use crate::iracing_sdk::{race_monitor, tire_strategy};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayWeather {
    condition: String,       // "clear" | "clouds" | "rain"
    air_temp: Option<i32>,   // °C do ar (None = desconhecido ao vivo)
    /// Arco de chuva da corrida atual (frações 0..1 + tipo de tempo), o mesmo clima
    /// determinístico que o export gravou. Vazio quando a prova é seca / sem dado — o
    /// front só mostra a faixa quando há chuva a antecipar.
    rain_arc: Vec<crate::iracing_sdk::weather::WeatherTimelinePoint>,
    /// Progresso da corrida AGORA (0..1) para o front marcar o "você está aqui" no arco.
    /// None fora de corrida ou sem total de voltas conhecido.
    now_frac: Option<f64>,
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
    /// Tempo PARADO no box (s) da última parada, exposto só nas ~2 voltas seguintes
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
        std::fs::read_to_string(crate::commands::iracing::numbers_path(&base_dir, &career_id))
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
            .unwrap_or_else(|| NEUTRAL_TEAM_COLOR.to_string());
        Some((d.nome, d.stats_temporada.pontos.round() as i32, team_name, color))
    };

    // Join de RESERVA nome→driver_id, montado SOB DEMANDA (só quando algum carro falha
    // o join por número) e uma única vez por chamada — uma varredura no elenco em vez
    // de uma consulta por carro. No caminho saudável nem chega a ser construído.
    let mut by_name: Option<HashMap<String, String>> = None;

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
    // Diagnóstico do funil. `sem_piloto_fallback` não descarta mais nada: mede quantos
    // carros entraram com identidade do SDK por não terem casado com a carreira — se vier
    // alto, o mapa de números está fora de sincronia com a sessão.
    let mut sem_pos_incluido = 0usize;
    let mut sem_piloto_fallback = 0usize;
    // Por qual chave cada carro se identificou, e quantos resolveram mas vieram SEM
    // equipe (piloto no elenco, contrato ativo não) — a torre fica cinza nos dois casos,
    // mas a correção é diferente, então os números precisam vir separados.
    let mut join_nome = 0usize;
    let mut join_numero = 0usize;
    let mut sem_equipe = 0usize;

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
        // Identidade que o PRÓPRIO SDK dá ao carro (o `UserName` do YAML).
        let sdk_name = feedback
            .driver_names
            .get(&meta.idx)
            .map(|n| n.trim())
            .filter(|n| !n.is_empty());
        // Identidade: tenta as chaves EM ORDEM e fica na primeira que RESOLVE de fato
        // no elenco — o teste é o piloto existir, não a chave existir.
        //
        // Para o jogador manda a identidade do save. Para a IA a 1ª chave é o NOME do
        // SDK, não o número: o nome vem do YAML DESTA sessão (nós o escrevemos no export
        // do roster), então é verdade de campo. O mapa de números é uma tabela à parte,
        // guardada por carreira, que só cresce e nunca reconcilia — envelhecido ele não
        // apenas falha, ele casa o carro com o piloto ERRADO (número 12 → um id de outro
        // grid) e a torre mostra nome/equipe trocados. Por isso ele é a chave de reserva,
        // boa justamente para sessão online, onde o `UserName` é a conta iRacing real e
        // não bate com o elenco.
        let mut matched = if is_self {
            player_driver
                .as_ref()
                .map(|d| d.id.clone())
                .and_then(|id| resolve(&id).map(|t| (id, t)))
        } else {
            let map = by_name.get_or_insert_with(|| {
                dq::get_all_drivers(&db.conn)
                    .map(|ds| ds.into_iter().map(|d| (name_key(&d.nome), d.id)).collect())
                    .unwrap_or_default()
            });
            let hit = sdk_name
                .and_then(|n| map.get(&name_key(n)).cloned())
                .and_then(|id| resolve(&id).map(|t| (id, t)));
            if hit.is_some() {
                join_nome += 1;
            }
            hit
        };

        // Reserva: o número fixo da carreira.
        if matched.is_none() && !is_self {
            matched = by_number
                .get(&(meta.car_number as i64))
                .cloned()
                .and_then(|id| resolve(&id).map(|t| (id, t)));
            if matched.is_some() {
                join_numero += 1;
            }
        }

        let (driver_id, (name, points, team, color)) = match matched {
            Some((id, t)) => (Some(id), t),
            None => {
                // O carro EXISTE na pista: não pode sumir da torre só porque não achamos
                // dono na carreira. Entra com a identidade do SDK (nome do YAML, senão o
                // número) e sem dados de carreira. Antes era `continue`, e um mapa de
                // números fora de sincronia esvaziava a torre inteira — sobrava só o
                // jogador, que resolve pela identidade do save.
                sem_piloto_fallback += 1;
                (
                    None,
                    (
                        sdk_name
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("#{}", meta.car_number)),
                        0,
                        String::new(),
                        NEUTRAL_TEAM_COLOR.to_string(),
                    ),
                )
            }
        };
        if class_pos <= 0 {
            sem_pos_incluido += 1;
        }
        if driver_id.is_some() && team.is_empty() {
            sem_equipe += 1;
        }

        let strat = tire_by_idx.get(&meta.idx);
        let tire_history: Vec<String> = strat
            .map(|s| s.stints.iter().map(|st| compound_str(st.compound).to_string()).collect())
            .unwrap_or_default();
        let stops = strat.map(|s| s.tire_changes).unwrap_or(0);

        // Tracker de tempo de pit: última parada do carro, só nas ~2 voltas seguintes.
        let pit_secs = strat.and_then(|s| s.stops.last()).and_then(|last| {
            let since = lead_lap_now - last.lap;
            (0..=2).contains(&since).then(|| last.box_secs.round() as i32)
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
            "by_class_vazio: tele_cars={} meta_elegivel={} roster={} player_idx={} | sem_piloto_fallback={}",
            tele.cars.len(),
            eligible_meta_count,
            by_number.len(),
            player_idx,
            sem_piloto_fallback
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

    // Arco de chuva da corrida ATUAL (a próxima pendente da categoria do jogador): trazemos
    // o MESMO clima determinístico do export pra torre ao vivo, e só quando de fato chove (há
    // arco a antecipar). `now_frac` = progresso por voltas, só em corrida, p/ o front marcar
    // o AGORA. Falha silenciosa → sem arco (a torre segue com condição + temperatura).
    let (rain_arc, weather_now_frac) = {
        let next = crate::db::queries::seasons::get_active_season(&db.conn)
            .ok()
            .flatten()
            .and_then(|s| {
                crate::db::queries::calendar::get_next_race(&db.conn, &s.id, &category)
                    .ok()
                    .flatten()
            });
        match next.and_then(|entry| {
            crate::commands::iracing::build_race_weather_timeline(&db.conn, &career_id, &entry.id).ok()
        }) {
            Some(tl) if tl.is_wet_race => {
                let now = (kind == "R" && total_laps > 0)
                    .then(|| (f64::from(lead_lap) / f64::from(total_laps)).clamp(0.0, 1.0));
                (tl.points, now)
            }
            _ => (Vec::new(), None),
        }
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
            rain_arc,
            now_frac: weather_now_frac,
        },
    };

    // Reporta também o FUNIL, não só o resultado: "1 carro" tem duas causas opostas
    // — sessão realmente com 1 carro (tele=1) ou grid cheio cujo número não casou com
    // o elenco (tele=22). Sem isto os dois casos logam igual.
    dbg_state(&format!(
        "ok: {} classes, {} carros ({} sem posição) | funil: tele={} elegiveis={} roster={} \
         | join: nome={} numero={} fallback={} sem_equipe={}",
        classes.len(),
        classes.iter().map(|c| c.cars.len()).sum::<usize>(),
        sem_pos_incluido,
        tele.cars.len(),
        eligible_meta_count,
        by_number.len(),
        join_nome,
        join_numero,
        sem_piloto_fallback,
        sem_equipe
    ));
    Ok(Some(OverlayData { session, classes }))
}
