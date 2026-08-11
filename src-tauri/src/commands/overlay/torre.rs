//! Torre de tempos AO VIVO: cruza pista (SDK), carreira (save) e sessão.

use std::collections::HashMap;

use serde::Serialize;

use super::formato::{
    best_positive_lap, compound_str, contagem_de_voltas, dbg_state, fmt_lap,
    history_matches_subsession, name_key, parse_session_times, parse_session_types,
    roster_with_telemetry, session_kind, tempo_util, wetness_to_condition, ContagemVoltas,
    NEUTRAL_TEAM_COLOR, SENTINELA_TEMPO_S,
};
use super::ordem::{modo_da_sessao, ordem_pre_sessao, ordenar, OrderInput, PreSinal};
use crate::config::app_config::AppConfig;
use crate::constants::scoring;
use crate::db::connection::Database;
use crate::db::queries::{contracts as cq, drivers as dq, teams as tq};
use crate::iracing_sdk::{race_monitor, tire_strategy};

/// A quantas voltas do fim o servidor de IA começa a ser aquecido. Três dá folga para o
/// container subir (cold start pode passar de 20s) antes da bandeirada, sem antecipar
/// tanto a ponto de o Cloud Run já ter escalado de volta a zero quando a corrida acabar.
const WARMUP_VOLTAS_RESTANTES: i32 = 3;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayWeather {
    condition: String,     // "clear" | "clouds" | "rain"
    air_temp: Option<i32>, // °C do ar (None = desconhecido ao vivo)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlaySession {
    #[serde(rename = "type")]
    kind: String, // "R" | "Q" | "P"
    lap: i32,
    total_laps: i32, // 0 = desconhecido (o front esconde o "/total")
    /// Segundos já corridos da sessão. None em sessão sem duração (prova por voltas).
    elapsed_s: Option<i32>,
    /// Duração total da sessão em segundos (quali costuma ser 480 = 8 min). None quando
    /// a sessão é por voltas — aí o header mostra a volta, não o relógio.
    duration_s: Option<i32>,
    /// Segundos que ainda faltam para a sessão acabar. None quando o iRacing manda o
    /// sentinela de "ilimitado". É o que segura o relógio da classificatória quando a
    /// DURAÇÃO não é conhecida: sem total não dá para mostrar "3:12/8:00", mas "faltam
    /// 5:12" continua sendo a informação que importa ali.
    remaining_s: Option<i32>,
    flag: String,     // "green" | "yellow" | "checkered"
    category: String, // id da categoria do evento (ex.: "gt3", "endurance")
    weather: OverlayWeather,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayCar {
    /// `CarIdx` do iRacing — identidade ESTÁVEL da linha. Não muda quando o piloto
    /// troca de posição, entra no box ou some da telemetria, e é por isso que existe:
    /// o front usa como chave pra saber que "esta linha é a mesma de antes, só mudou
    /// de lugar" e animar o deslize. Sem ela só sobraria o nome.
    idx: i32,
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
    /// A MESMA melhor volta de `fastest`, em milissegundos (0 = ainda não marcou). O
    /// texto já formatado não serve para conta: é dele que a torre tira o INTERVALO para
    /// o melhor tempo da classe, que é o que a classificação mostra no lugar das posições
    /// ganhas/perdidas da corrida.
    best_ms: i64,
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

/// O que a torre precisa saber de um piloto do lado da CARREIRA (o join do save).
/// Fica de fora do `OverlayCar` porque nada disto vai pro front — só ordena.
struct DriverInfo {
    nome: String,
    pontos: i32,
    equipe: String,
    cor: String,
    pre: PreSinal,
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
    // Melhor volta VÁLIDA da classificatória por carro, travada do `CarIdxBestLapTime`.
    // É a única fonte de tempo da quali: volta anulada por limite de pista não entra, e
    // o valor sobrevive ao carro voltar pra garagem (onde ele sai de `tele.cars`).
    let qualy_best_valid: HashMap<i32, f64> =
        race_monitor::get_qualy_best_valid().into_iter().collect();
    let current_subsession_id = race_monitor::get_subsession_id();
    let history_is_current =
        history_matches_subsession(history.subsession_id, current_subsession_id);
    let is_green = race_monitor::poll().is_green;

    // Tipo de sessão (do YAML) — precisa vir ANTES do cruzamento: ele decide o critério
    // de ordenação da torre e se a estratégia de pneu vale (só em CORRIDA).
    let yaml = crate::iracing_sdk::read_session()
        .map(|s| s.session_yaml)
        .unwrap_or_default();
    let kind = session_kind(&parse_session_types(&yaml), tele.session_num);
    let is_race = kind == "R";
    // Duração declarada de cada sessão do fim de semana — reserva do relógio do cabeçalho
    // (ver onde `duration_s` é montado, no fim).
    let session_times = parse_session_times(&yaml);

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

    // Estratégia de pneu por CarIdx (stints + nº de trocas). SÓ em corrida: em treino
    // e classificatória os carros começam PARADOS na caixa e saem pra pista, o que a
    // inferência leria como uma parada de box gigante — a coluna de pneu vinha cheia
    // de stints fantasmas antes mesmo da largada.
    let tire_by_idx: HashMap<i32, tire_strategy::CarTireStrategy> = if is_race && history_is_current
    {
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

    let numbers: HashMap<String, i64> = std::fs::read_to_string(
        crate::commands::iracing::numbers_path(&base_dir, &career_id),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default();
    let by_number: HashMap<i64, String> = numbers.into_iter().map(|(id, n)| (n, id)).collect();

    let player_driver = dq::get_player_driver(&db.conn).ok();

    // Categoria do evento = a do time do jogador (via contrato ativo).
    let category = player_driver
        .as_ref()
        .and_then(|p| {
            cq::get_active_contract_for_pilot(&db.conn, &p.id)
                .ok()
                .flatten()
        })
        .and_then(|c| tq::get_team_by_id(&db.conn, &c.equipe_id).ok().flatten())
        .map(|t| t.categoria)
        .or_else(|| {
            player_driver
                .as_ref()
                .and_then(|p| p.categoria_atual.clone())
        })
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

    // Resolve driver_id → o que a torre mostra do piloto na carreira.
    let resolve = |driver_id: &str| -> Option<DriverInfo> {
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
        let temp = &d.stats_temporada;
        Some(DriverInfo {
            pontos: temp.pontos.round() as i32,
            pre: PreSinal {
                corridas_temporada: temp.corridas as i32,
                pontos: temp.pontos.round() as i32,
                vitorias: temp.vitorias as i32,
                podios: temp.podios as i32,
                expectativa: crate::commands::season_preview::perception_score(&d),
                conhecido: true,
            },
            nome: d.nome,
            equipe: team_name,
            cor: color,
        })
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
    let cam_is_valid = cam_idx >= 0
        && feedback
            .cars_yaml_meta
            .iter()
            .any(|m| m.idx == cam_idx && !m.is_pace);
    let focus_idx = if cam_is_valid { cam_idx } else { player_idx };

    // Melhor volta registrada da sessão atual, e a regra muda com a sessão:
    //
    //   • CLASSIFICATÓRIA: só volta VÁLIDA. A classificatória é o tempo que vale, então
    //     volta anulada por limite de pista não pode ordenar nem aparecer. As voltas
    //     cruas de `get_qualy_laps` (o `CarIdxLastLapTime`) registram a volta cortada
    //     com o tempo que ela marcou, e por isso não servem aqui.
    //   • CORRIDA: as voltas do histórico, cruas mesmo. Na corrida a melhor volta é
    //     informação de ritmo e vale ser mostrada ainda que anulada.
    //   • TREINO: nada — não reutiliza dado potencialmente stale.
    let mut recorded_best_lap: HashMap<i32, f64> = HashMap::new();
    match kind {
        "Q" => recorded_best_lap = qualy_best_valid.clone(),
        "R" if history_is_current => {
            for lap in &history.car_laps {
                if lap.time > 0.0 {
                    recorded_best_lap
                        .entry(lap.car_idx)
                        .and_modify(|secs| *secs = secs.min(lap.time))
                        .or_insert(lap.time);
                }
            }
        }
        _ => {}
    }

    // Melhor volta da QUALI por carro (ms) = grid pra ordenar quem ainda não tem
    // posição oficial. Mesma régua da linha de cima: o grid sai da classificatória,
    // então ele também só conhece volta válida. Em treino: vazio → a ordenação cai
    // direto no nº do carro.
    let qualy_best_ms: HashMap<i32, i64> = if kind != "P" {
        qualy_best_valid
            .iter()
            .map(|(idx, secs)| (*idx, (secs * 1000.0).round() as i64))
            .collect()
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
    // (carro, melhor_volta_s, entrada de ordenação, posição de largada, sinal prévio)
    let mut by_class: HashMap<i64, Vec<(OverlayCar, f64, OrderInput, i32, PreSinal)>> =
        HashMap::new();
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

        let (driver_id, info) = match matched {
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
                    DriverInfo {
                        nome: sdk_name
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("#{}", meta.car_number)),
                        pontos: 0,
                        equipe: String::new(),
                        cor: NEUTRAL_TEAM_COLOR.to_string(),
                        // Sem dono na carreira não há campeonato nem expectativa: este
                        // carro fica atrás de quem tem hierarquia, ordenado pelo número.
                        pre: PreSinal::default(),
                    },
                )
            }
        };
        if class_pos <= 0 {
            sem_pos_incluido += 1;
        }
        if driver_id.is_some() && info.equipe.is_empty() {
            sem_equipe += 1;
        }

        let strat = tire_by_idx.get(&meta.idx);
        let tire_history: Vec<String> = strat
            .map(|s| {
                s.stints
                    .iter()
                    .map(|st| compound_str(st.compound).to_string())
                    .collect()
            })
            .unwrap_or_default();
        let stops = strat.map(|s| s.tire_changes).unwrap_or(0);

        // Tracker de tempo de pit: última parada do carro, só nas ~2 voltas seguintes.
        let pit_secs = strat.and_then(|s| s.stops.last()).and_then(|last| {
            let since = lead_lap_now - last.lap;
            (0..=2)
                .contains(&since)
                .then(|| last.box_secs.round() as i32)
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
                    let is_repair =
                        reps.is_some_and(|ls| ls.iter().any(|&l| (l as i32 - stop.lap).abs() <= 1));
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

        class_label.entry(meta.class_id).or_insert_with(|| {
            feedback
                .class_names
                .get(&meta.class_id)
                .cloned()
                .unwrap_or_default()
        });

        // Tudo que a ordenação da torre precisa deste carro. A posição/delta/pontos
        // ficam em 0 aqui: só existem depois de a classe inteira ser ordenada.
        let order_input = OrderInput {
            class_position: class_pos,
            lap_completed: car.map(|s| s.lap_completed).unwrap_or(-1),
            lap_dist_pct: car.map(|s| s.lap_dist_pct).unwrap_or(-1.0),
            best_lap_ms: if best_lap > 0.0 {
                (best_lap * 1000.0).round() as i64
            } else {
                i64::MAX
            },
            qualy_best_ms: qualy_best_ms.get(&meta.idx).copied().unwrap_or(i64::MAX),
            // Preenchido depois: a ordem prévia é relativa aos outros carros da CLASSE.
            pre_ordem: i64::MAX,
            car_number: meta.car_number as i64,
        };

        by_class.entry(meta.class_id).or_default().push((
            OverlayCar {
                idx: meta.idx,
                pos: 0,
                name: info.nome,
                team: info.equipe,
                color: info.cor,
                delta: 0,
                stops,
                tire_history,
                points: info.pontos,
                gain: 0,
                fastest: fmt_lap(best_lap),
                best_ms: if best_lap > 0.0 {
                    (best_lap * 1000.0).round() as i64
                } else {
                    0
                },
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
            order_input,
            grid_pos,
            info.pre,
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

    // Ordena cada classe pelo critério do MOMENTO (ver `ordem.rs`): tempo em treino/
    // quali, grid antes do verde, progresso real na pista com a corrida rolando e a
    // posição oficial depois da bandeirada. A posição mostrada é o lugar nessa ordem —
    // não o `CarIdxClassPosition` cru, que só se move quando o carro cruza a linha.
    let modo = modo_da_sessao(kind, tele.session_state);
    let mut classes: Vec<OverlayClass> = Vec::new();
    for (class_id, mut cars) in by_class {
        // Ordem de ANTES do tempo (campeonato / expectativa de pré-temporada): é o
        // desempate de todo mundo que ainda não marcou volta — sem ela a torre da
        // classificatória começa numa fila por número de carro.
        let previa = ordem_pre_sessao(
            &cars
                .iter()
                .map(|(_, _, _, _, pre)| *pre)
                .collect::<Vec<_>>(),
        );
        let inputs: Vec<OrderInput> = cars
            .iter()
            .enumerate()
            .map(|(i, (_, _, input, _, _))| OrderInput {
                pre_ordem: previa[i],
                ..*input
            })
            .collect();
        let ordem = ordenar(modo, &inputs);
        let mut rank = vec![0usize; cars.len()];
        for (lugar, &i) in ordem.iter().enumerate() {
            rank[i] = lugar;
        }
        // Agora que a posição existe, derivam-se dela o delta pro grid e os pontos.
        for (i, (c, _, _, grid_pos, _)) in cars.iter_mut().enumerate() {
            c.pos = rank[i] as i32 + 1;
            c.delta = if *grid_pos > 0 { *grid_pos - c.pos } else { 0 };
            c.gain =
                scoring::get_points_for_position(c.pos.clamp(0, 255) as u8, is_endurance) as i32;
        }
        // volta mais rápida da classe (menor tempo positivo)
        if let Some((best_i, _)) = cars
            .iter()
            .enumerate()
            .filter(|(_, (_, secs, _, _, _))| *secs > 0.0)
            .min_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap())
        {
            cars[best_i].0.fol = true;
        }
        cars.sort_by_key(|(c, _, _, _, _)| c.pos);
        let label = class_label.get(&class_id).cloned().unwrap_or_default();
        let id = label.trim().to_lowercase().replace(' ', "");
        classes.push(OverlayClass {
            id,
            label: label.trim().to_uppercase(),
            cars: cars.into_iter().map(|(c, _, _, _, _)| c).collect(),
        });
    }

    // Estado 5 (Checkered) e 6 (CoolDown): a bandeirada caiu. O mesmo corte que
    // `modo_da_sessao` usa para passar a torre à ordem oficial.
    let encerrada = tele.session_state >= 5;

    // Depois da bandeirada o `lap_completed` da grade CONTINUA subindo: quem segue
    // girando na volta de desaceleração fecha mais uma volta. Por isso a contagem passa
    // a usar o retrato que o monitor congelou no instante em que a corrida acabou. Sem
    // retrato (app aberto já com a prova encerrada) sobra o valor ao vivo, e o teto fica
    // por conta do total da prova.
    let volta_final = race_monitor::get_final_lead_lap();
    let max_completed = tele.cars.iter().map(|c| c.lap_completed).max().unwrap_or(0);
    let max_completed = if encerrada && volta_final > 0 {
        volta_final
    } else {
        max_completed
    };

    // Sessão por TEMPO (o caso normal no Loop: quali de 8 min, corrida de X min) versus
    // por VOLTAS. O iRacing manda valores-sentinela de "ilimitado" — 604800 s e 32767
    // voltas —, então os dois lados precisam de teto pra não virarem número de verdade.
    let timed = tempo_util(tele.session_time_total);
    // Duração da sessão: o canal ao vivo primeiro e, quando ele vem sentinelado, a que o
    // YAML DECLARA para esta sessão (`SessionTime: 480.0000 sec`). A classificatória é
    // justamente onde o canal ao vivo costuma faltar, e sem essa reserva o cabeçalho
    // ficava sem relógio nenhum — era o buraco por onde a quali caía na apresentação de
    // voltas da corrida. A contagem de VOLTAS abaixo continua olhando só o canal ao vivo:
    // quem decide se a prova é por tempo ou por voltas é o `timed`, e ele não muda aqui.
    let duration_s = timed
        .then_some(tele.session_time_total)
        .or_else(|| session_times.get(&tele.session_num).copied())
        .map(|secs| secs.round() as i32);
    // Zero é restante VÁLIDO (a sessão acabou de fechar), então aqui o piso é o zero e não
    // o `tempo_util` — o que se quer cortar é só o sentinela de "ilimitado".
    let remaining_s = (tele.session_time_remain >= 0.0
        && tele.session_time_remain < SENTINELA_TEMPO_S)
        .then(|| tele.session_time_remain.round() as i32);
    let elapsed_s = match (duration_s, remaining_s) {
        (Some(total), Some(restante)) => Some((total - restante).clamp(0, total)),
        _ => None,
    };

    // Ritmo de referência pra estimar quantas voltas ainda cabem: a melhor volta de
    // quem está na pista.
    let ref_lap = tele
        .cars
        .iter()
        .map(|c| c.best_lap_time)
        .filter(|t| *t > 0.0)
        .fold(f64::INFINITY, f64::min);

    // Em prova por TEMPO não existe total fixo, então estimamos quantas voltas ainda
    // cabem pelo tempo restante dividido pelo ritmo de referência, arredondando AO MAIS
    // PRÓXIMO. Nada de arredondar pra cima: a melhor volta já é mais rápida que o ritmo
    // real de corrida (tráfego, combustível, pneu), então a divisão por ela JÁ
    // superestima quantas voltas cabem — o `ceil` empilharia viés em cima de viés. O
    // total é estimativa e pode mexer ±1 durante a prova.
    let estimativa_restantes = (timed && kind == "R" && ref_lap.is_finite())
        .then(|| (tele.session_time_remain.max(0.0) / ref_lap).round() as i32);

    let ContagemVoltas {
        lap: lead_lap,
        total: total_laps,
    } = contagem_de_voltas(
        max_completed,
        !timed,
        tele.session_laps_total,
        tele.session_laps_remain_ex,
        encerrada,
        estimativa_restantes,
    );

    // Reta final da corrida: aquece o servidor de IA. Quando a bandeirada cair, o debrief
    // do engenheiro e o boletim da revista são pedidos quase juntos, e a PRIMEIRA dessas
    // chamadas paga o cold start do Cloud Run inteiro — é ela que faz o jogador esperar.
    // O texto em si não dá pra adiantar (os fatos só existem depois do resultado), então o
    // que se antecipa é o container. `spawn_warmup` tem guarda de intervalo, o que importa
    // porque isto roda no poll da torre. Vale para prova por tempo também: `total_laps` já
    // chega aqui estimado pelo ritmo quando o iRacing manda o sentinela.
    if kind == "R" && total_laps > 0 && total_laps - lead_lap <= WARMUP_VOLTAS_RESTANTES {
        crate::narrative::client::spawn_warmup();
    }

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
        elapsed_s,
        duration_s,
        remaining_s,
        flag: flag.to_string(),
        category,
        weather: OverlayWeather {
            condition: wetness_to_condition(tele.track_wetness).to_string(),
            air_temp,
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
