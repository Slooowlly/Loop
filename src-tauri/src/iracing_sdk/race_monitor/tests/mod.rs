use super::*;
use crate::iracing_sdk::{CarSnapshot, IracingTelemetry};

// ── Máquina de apagar o alerta de penalidade (pit-out reparado) ──────────
#[test]
fn alerta_apaga_ao_sair_do_box_depois_de_entrar() {
    // Quebrou fora do box (não entrou ainda) → não apaga.
    let (entered, clear) = pit_clear_step(false, false, false);
    assert!(!entered && !clear);
    // Entra no box (false→true) → marca "entrou", ainda não apaga.
    let (entered, clear) = pit_clear_step(entered, true, false);
    assert!(entered && !clear);
    // Continua no box → segue marcado, sem apagar.
    let (entered, clear) = pit_clear_step(entered, true, true);
    assert!(entered && !clear);
    // Sai do box (true→false) já tendo entrado → APAGA (serviu/reparou).
    let (_entered, clear) = pit_clear_step(entered, false, true);
    assert!(clear, "sair do box após entrar deveria apagar o alerta");
}

#[test]
fn passar_reto_pelo_pit_lane_sem_entrar_nao_apaga() {
    // Nunca entrou no box: uma leitura solta de "saiu" (prev=true) não pode apagar.
    let (entered, clear) = pit_clear_step(false, false, false);
    assert!(!entered && !clear, "sem entrar no box, nada apaga");
}

fn active_attempt() -> Attempt {
    Attempt {
        number: 1,
        status: "active".to_string(),
        started_at_session_time: 0.0,
        laps_completed: 0,
        ended_by: None,
        reason: None,
        worst_crash: None,
        evidence: AttemptEvidence::default(),
        crashes: Vec::new(),
        peak_crash_score: 0.0,
        collided_with_car_number: None,
        peak_impact_dir: None,
        style: crate::car::driving_style::StyleAccumulator::new(),
    }
}

fn on_track(idx: i32, position: i32, lap_completed: i32) -> CarSnapshot {
    CarSnapshot {
        idx,
        position,
        lap_completed,
        track_surface: SURFACE_ON_TRACK,
        ..Default::default()
    }
}

/// Frame de corrida verde: líder (idx 1) em P1 na volta `leader_lap`, jogador
/// (idx 0) em P2 logo atrás.
fn race_frame(session_num: i32, leader_lap: i32) -> IracingTelemetry {
    IracingTelemetry {
        session_num,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![on_track(0, 2, leader_lap - 1), on_track(1, 1, leader_lap)],
        ..Default::default()
    }
}

#[test]
fn parser_extrai_subsession_id_do_weekend_info() {
    let yaml = "WeekendInfo:\n  TrackID: 123\n  SubSessionID: 987654\n";

    assert_eq!(parse_subsession_id(yaml), 987654);
}

/// `DriverInfo` com dois carros: o jogador (idx 7) e um adversário. O nome do carro
/// só pode sair da entrada cujo `CarIdx` casa com o `DriverCarIdx`.
const DRIVER_INFO_YAML: &str = concat!(
    "DriverInfo:\n",
    " DriverCarIdx: 7\n",
    " Drivers:\n",
    " - CarIdx: 3\n",
    "   CarScreenName: Porsche 911 GT3 Cup\n",
    "   CarNumberRaw: 12\n",
    " - CarIdx: 7\n",
    "   CarScreenName: Global Mazda MX-5 Cup\n",
    "   CarNumberRaw: 64\n",
);

#[test]
fn parser_pega_o_carro_do_jogador_e_nao_o_do_vizinho() {
    assert_eq!(
        parse_player_car_name(DRIVER_INFO_YAML).as_deref(),
        Some("Global Mazda MX-5 Cup")
    );
}

#[test]
fn parser_do_carro_devolve_none_quando_o_yaml_nao_ajuda() {
    // Sem DriverCarIdx não dá pra saber qual das entradas é a do jogador — e chutar
    // a primeira mandaria o carro ERRADO na telemetria, pior que não mandar nada.
    let sem_idx = "DriverInfo:\n Drivers:\n - CarIdx: 3\n   CarScreenName: Skip Barber\n";
    assert_eq!(parse_player_car_name(sem_idx), None);
    // Jogador presente, mas sem nome de carro na entrada dele.
    let sem_nome = "DriverInfo:\n DriverCarIdx: 7\n Drivers:\n - CarIdx: 7\n   CarNumberRaw: 64\n";
    assert_eq!(parse_player_car_name(sem_nome), None);
    assert_eq!(parse_player_car_name(""), None);
}

/// Monta um monitor com um histórico de corrida plausível: jogador (idx 0) na mesma
/// classe de dois adversários, mais um pace car que NÃO conta no tamanho da classe.
fn monitor_com_historico() -> RaceMonitor {
    let mut m = RaceMonitor::new();
    m.session_car_name = Some("Global Mazda MX-5 Cup".to_string());
    m.history.player_car_idx = 0;
    m.history.cars_meta = vec![
        CarMeta {
            idx: 0,
            is_ai: false,
            is_pace: false,
            class_id: 10,
            class_position: 4,
            car_number: 64,
            grid_class_position: 9,
        },
        CarMeta {
            idx: 1,
            is_ai: true,
            is_pace: false,
            class_id: 10,
            class_position: 1,
            car_number: 1,
            grid_class_position: 1,
        },
        CarMeta {
            idx: 2,
            is_ai: true,
            is_pace: false,
            class_id: 10,
            class_position: 2,
            car_number: 2,
            grid_class_position: 2,
        },
        CarMeta {
            idx: 3,
            is_ai: true,
            is_pace: false,
            class_id: 99,
            class_position: 1,
            car_number: 3,
            grid_class_position: 1,
        },
        CarMeta {
            idx: 4,
            is_ai: true,
            is_pace: true,
            class_id: 10,
            class_position: 0,
            car_number: 0,
            grid_class_position: 0,
        },
    ];
    m.history.car_laps = vec![
        CarLap {
            car_idx: 0,
            lap: 1,
            time: 95.5,
        },
        CarLap {
            car_idx: 0,
            lap: 2,
            time: 94.2,
        }, // melhor do jogador
        CarLap {
            car_idx: 1,
            lap: 1,
            time: 92.0,
        }, // melhor da classe
        CarLap {
            car_idx: 2,
            lap: 1,
            time: 93.1,
        },
        CarLap {
            car_idx: 3,
            lap: 1,
            time: 80.0,
        }, // outra classe: não conta
        CarLap {
            car_idx: 0,
            lap: 3,
            time: -1.0,
        }, // volta inválida: ignorada
    ];
    m
}

#[test]
fn desfecho_sai_do_historico_com_as_tres_pecas_da_posicao() {
    let m = monitor_com_historico();
    let ev = AttemptEvidence {
        raced: true,
        incident_points: 4,
        off_track: true,
        ..Default::default()
    };

    let o = m.build_race_outcome(&ev, 12, 3, Some("leve".to_string()));

    assert_eq!(o.posicao_final, 4);
    assert_eq!(o.posicao_grid, 9);
    // Só a classe do jogador, e o pace car fora da conta.
    assert_eq!(o.carros_na_classe, 3);
    assert_eq!(o.voltas, 12);
    assert_eq!(o.incidentes, 4);
    assert_eq!(o.restarts, 2); // 3ª tentativa = 2 largadas refeitas
    assert!(o.off_track);
    assert_eq!(o.carro.as_deref(), Some("Global Mazda MX-5 Cup"));
    assert_eq!(o.pior_batida.as_deref(), Some("leve"));
}

#[test]
fn melhor_volta_ignora_volta_invalida_e_outra_classe() {
    let m = monitor_com_historico();
    let o = m.build_race_outcome(&AttemptEvidence::default(), 12, 1, None);

    assert_eq!(o.melhor_volta_s, 94.2);
    // 80.0 é de outra classe: a referência de ritmo tem que ser a classe do jogador.
    assert_eq!(o.melhor_volta_classe_s, 92.0);
}

#[test]
fn desfecho_sem_meta_do_jogador_nao_inventa_posicao() {
    // Corrida que acabou sem o YAML ter enchido o cars_meta: o que dá pra saber
    // continua indo, o que não dá sai zerado (e o telemetry omite do payload).
    let mut m = monitor_com_historico();
    m.history.cars_meta.clear();

    let o = m.build_race_outcome(&AttemptEvidence::default(), 5, 1, None);

    assert_eq!(o.posicao_final, 0);
    assert_eq!(o.posicao_grid, 0);
    assert_eq!(o.carros_na_classe, 0);
    assert_eq!(o.melhor_volta_s, 0.0);
    assert_eq!(o.voltas, 5);
    assert_eq!(o.carro.as_deref(), Some("Global Mazda MX-5 Cup"));
}

#[test]
fn clima_vivo_sem_canais_cai_no_baseline() {
    // Telemetria zerada (canais ainda não populados) → mantém o clima fixo da corrida.
    let baseline = crate::car::breakdown::Weather {
        wetness: 0.0,
        temperature: 27.0,
        humidity: 55.0,
        wind_kmh: 30.0,
    };
    let w = effective_weather(&IracingTelemetry::default(), baseline);
    assert_eq!(w, baseline);
}

#[test]
fn clima_vivo_sobrepoe_canais_presentes() {
    let baseline = crate::car::breakdown::Weather::NEUTRAL;
    let t = IracingTelemetry {
        air_temp: 31.0,
        track_wetness: 7,       // ExtremelyWet → 1.0
        relative_humidity: 0.8, // 80%
        wind_ms: 10.0,          // 36 km/h
        ..Default::default()
    };
    let w = effective_weather(&t, baseline);
    assert_eq!(w.temperature, 31.0);
    assert!((w.wetness - 1.0).abs() < 1e-9);
    assert!((w.humidity - 80.0).abs() < 1e-9);
    assert!((w.wind_kmh - 36.0).abs() < 1e-9);
}

/// Monitor armado com UM carro (o do jogador, nº 7) cuja eletrônica já está na zona de
/// risco, e com o clima baseline SECO. A telemetria traz chuva pesada — é essa diferença
/// entre baseline e vivo que o teste mede.
fn monitor_com_quebra_do_jogador() -> (RaceMonitor, IracingTelemetry) {
    let mut m = RaceMonitor::new();
    m.history.player_car_idx = 0;
    m.car_number[0] = 7;

    let mut car = crate::car::Car::uniform(3);
    car.set_wear(crate::car::PartType::Electronics, 0.91);
    let live = crate::car::breakdown::LiveBreakdown::new(&car, 42, 50.0, (1.0, 1.0, 1.0));
    let mut dir = crate::car::breakdown::BreakdownDirector::new();
    dir.add_car(7, live, Vec::new());
    dir.prime_lap(7, 0);
    m.breakdown = Some(dir);
    // Baseline SECO — se o tick do jogador usar isto, a eletrônica gasta menos.
    m.breakdown_weather = crate::car::breakdown::Weather::NEUTRAL;
    m.breakdown_needs_prime = false;

    let t = IracingTelemetry {
        session_state: STATE_RACING,
        player_car_idx: 0,
        lap_completed: 1,
        track_wetness: 7, // ExtremelyWet → chuva castiga a eletrônica (§3.5)
        air_temp: 31.0,
        relative_humidity: 0.8,
        cars: vec![on_track(0, 1, 1)],
        ..Default::default()
    };
    (m, t)
}

fn desgaste_da_eletronica(m: &RaceMonitor) -> f64 {
    m.breakdown
        .as_ref()
        .expect("diretor armado")
        .car_parts_in_danger(7)
        .into_iter()
        .find(|(_, pt, _)| *pt == crate::car::PartType::Electronics)
        .map(|(_, _, wear)| wear)
        .expect("eletrônica na zona de risco")
}

#[test]
fn quebra_do_jogador_usa_o_clima_vivo_igual_ao_resto_da_grade() {
    // O tick do jogador roda ANTES do da grade e `on_lap_at` só avança pra frente: quem
    // chega primeiro na volta é quem manda. Se os dois usam o MESMO clima, tanto faz a
    // ordem — e é essa invariante que se mede aqui. Com o baseline no tick do jogador, o
    // carro dele rodava sob o clima do export enquanto a grade rodava sob a chuva real.
    let (mut so_grade, t) = monitor_com_quebra_do_jogador();
    so_grade.tick_breakdown_grid(&t);

    let (mut jogador_primeiro, t2) = monitor_com_quebra_do_jogador();
    jogador_primeiro.tick_breakdown_player(&t2);
    jogador_primeiro.tick_breakdown_grid(&t2);

    assert_eq!(
        desgaste_da_eletronica(&jogador_primeiro),
        desgaste_da_eletronica(&so_grade),
        "o tick do jogador tem que gastar a peça sob o mesmo clima que a grade"
    );
}

#[test]
fn chuva_ao_vivo_castiga_mais_a_eletronica_do_jogador_que_o_baseline_seco() {
    // Prova que o teste acima não passa por acidente: sob chuva a eletrônica TEM que gastar
    // mais do que gastaria no seco (§3.5 — chuva é curto/sensor). Se os dois fossem iguais,
    // a asserção de igualdade acima seria vácua.
    let (mut com_chuva, t) = monitor_com_quebra_do_jogador();
    com_chuva.tick_breakdown_player(&t);

    let (mut no_seco, mut t_seco) = monitor_com_quebra_do_jogador();
    t_seco.track_wetness = 1; // Dry
    t_seco.air_temp = 25.0;
    t_seco.relative_humidity = 0.45;
    no_seco.tick_breakdown_player(&t_seco);

    assert!(
        desgaste_da_eletronica(&com_chuva) > desgaste_da_eletronica(&no_seco),
        "chuva ao vivo: {} deveria ser > seco: {}",
        desgaste_da_eletronica(&com_chuva),
        desgaste_da_eletronica(&no_seco)
    );
}

#[test]
fn clima_vivo_pista_seca_zera_o_molhado() {
    // Baseline "molhado" (previsão de chuva) mas a pista secou ao vivo (TrackWetness=1=Dry).
    let baseline = crate::car::breakdown::Weather {
        wetness: 0.7,
        temperature: 22.0,
        humidity: 60.0,
        wind_kmh: 20.0,
    };
    let t = IracingTelemetry {
        track_wetness: 1,
        ..Default::default()
    };
    let w = effective_weather(&t, baseline);
    assert!((w.wetness - 0.0).abs() < 1e-9);
    // Canais não lidos seguem no baseline.
    assert_eq!(w.temperature, 22.0);
    assert_eq!(w.humidity, 60.0);
}

#[test]
fn snapshot_de_quali_fica_disponivel_apos_captura() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;
    m.car_is_pace[7] = false;

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    let quali = IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    };

    m.capture_qualy(&quali);

    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 1);
    let lap = &snapshot[0];
    assert_eq!(lap.car_idx, 7);
    assert_eq!(lap.lap, 1);
    assert!((lap.time - 82.345).abs() < f64::EPSILON);
}

#[test]
fn troca_do_numero_da_quali_reseta_estado_e_captura_a_sessao_nova() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    });
    assert_eq!(m.qualy_laps_snapshot().len(), 1);

    m.set_qualy_session_num(2);
    assert!(
        !m.prev_in_qualy,
        "a nova sessão precisa rearmar a borda de entrada"
    );
    assert!(
        m.qualy_laps_snapshot().is_empty(),
        "as voltas antigas devem sumir imediatamente"
    );
    assert_eq!(m.qualy_car_lap_completed, [0; 64]);

    let mut new_car = on_track(7, 0, 1);
    new_car.last_lap_time = 81.234;
    m.capture_qualy(&IracingTelemetry {
        session_num: 2,
        cars: vec![new_car],
        ..Default::default()
    });
    assert!(m.prev_in_qualy);
    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 1);
    assert!((snapshot[0].time - 81.234).abs() < f64::EPSILON);
    assert_eq!(m.qualy_car_lap_completed[7], 1);
}

#[test]
fn troca_de_subsession_reseta_quali_com_session_num_igual() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);
    m.set_session_subsession_id(1001);

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    });
    assert_eq!(m.qualy_laps_snapshot().len(), 1);
    assert_eq!(m.qualy_car_lap_completed[7], 1);

    m.set_session_subsession_id(1002);

    assert_eq!(m.session_subsession_id, 1002);
    assert_eq!(m.qualy_session_num, 1);
    assert!(!m.prev_in_qualy);
    assert!(m.qualy_laps_snapshot().is_empty());
    assert_eq!(m.qualy_car_lap_completed, [0; 64]);
}

#[test]
fn subsession_zero_nao_descarta_estado_valido() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);
    m.set_session_subsession_id(1001);

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    });

    m.set_session_subsession_id(0);

    assert_eq!(m.session_subsession_id, 1001);
    assert!(m.prev_in_qualy);
    assert_eq!(m.qualy_laps_snapshot().len(), 1);
    assert_eq!(m.qualy_car_lap_completed[7], 1);
}

#[test]
fn historico_da_tentativa_recebe_subsession_atual() {
    let mut m = RaceMonitor::new();
    m.set_session_subsession_id(4242);
    m.attempts.push(active_attempt());

    m.record_history(&race_frame(2, 1));

    assert_eq!(m.history.subsession_id, 4242);
}

#[test]
fn quali_nao_entra_no_historico_da_corrida() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;
    m.attempts.push(active_attempt());

    // Tick da CLASSIFICATÓRIA (session_num == qualy) não grava nada.
    m.record_history(&race_frame(1, 3));
    assert!(
        m.history.laps.is_empty(),
        "a quali não pode entrar no histórico da corrida"
    );

    // Tick da CORRIDA (outro session_num) grava normalmente.
    m.record_history(&race_frame(2, 3));
    assert!(
        !m.history.laps.is_empty(),
        "a corrida deve gravar snapshots"
    );
    assert_eq!(m.hist_session_num, 2);
}

#[test]
fn troca_de_sessao_zera_o_historico() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    // Sessão anterior (ex.: treino, session 0) grava alguns snapshots.
    m.record_history(&race_frame(0, 5));
    assert!(!m.history.laps.is_empty());

    // Corrida (session 2) → histórico limpo, sem herdar a sessão anterior.
    m.record_history(&race_frame(2, 1));
    assert_eq!(m.hist_session_num, 2);
    assert!(
        m.history.laps.iter().all(|s| s.lap <= 1),
        "a corrida não pode herdar as voltas da sessão anterior"
    );
}

#[test]
fn troca_de_posicao_gera_snapshot_na_hora() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    // Volta 3 do líder; jogador (idx 0) em P3.
    let mut f = race_frame(2, 3);
    f.session_time = 10.0;
    f.cars = vec![on_track(0, 3, 2), on_track(1, 1, 3), on_track(2, 2, 3)];
    m.record_history(&f);
    let n0 = m.history.laps.len();
    assert!(n0 >= 1);

    // MESMA volta do líder, mas o jogador passa o idx 2 (P3 → P2) no meio da
    // volta. Líder a 40% da volta → o ponto sai em 3.4, não na virada.
    let mut f2 = race_frame(2, 3);
    f2.session_time = 12.0;
    let mut leader = on_track(1, 1, 3);
    leader.lap_dist_pct = 0.4;
    f2.cars = vec![on_track(0, 2, 2), leader, on_track(2, 3, 2)];
    m.record_history(&f2);

    assert_eq!(
        m.history.laps.len(),
        n0 + 1,
        "troca de posição deve gerar um snapshot na hora (sem virar a volta)"
    );
    let last = m.history.laps.last().unwrap();
    assert_eq!(last.lap, 3);
    assert!(
        (last.progress - 0.4).abs() < 1e-5,
        "progresso do líder na volta"
    );
    assert_eq!(
        last.cars.iter().find(|c| c.idx == 0).unwrap().position,
        2,
        "jogador subiu para P2 no ponto registrado"
    );
}

#[test]
fn setores_do_jogador_sao_cronometrados() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());
    // Jogador (idx 0) avança pela pista; capture_player_sectors cronometra ao
    // fechar cada setor entrado LIMPO (do começo dele).
    fn push(m: &mut RaceMonitor, pct: f64, time: f64, lap: i32) {
        let mut f = race_frame(2, 3);
        f.session_time = time;
        f.lap = lap;
        f.lap_dist_pct = pct;
        f.track_surface = SURFACE_ON_TRACK;
        f.cars = vec![on_track(0, 2, lap - 1), on_track(1, 1, lap)];
        m.record_history(&f);
    }
    push(&mut m, 0.10, 100.0, 2); // base no S1 (parcial → não grava)
    push(&mut m, 0.40, 110.0, 2); // entra S2 LIMPO
    push(&mut m, 0.70, 118.0, 2); // entra S3 → fecha S2 (8s)
    push(&mut m, 0.05, 126.0, 3); // cruza a linha → fecha S3 (8s)

    let secs: Vec<(i32, f64)> = m
        .history
        .player_sectors
        .iter()
        .map(|s| (s.sector, (s.time * 10.0).round() / 10.0))
        .collect();
    assert!(secs.contains(&(2, 8.0)), "S2 cronometrado: {secs:?}");
    assert!(secs.contains(&(3, 8.0)), "S3 cronometrado: {secs:?}");
    // O S1 inicial era parcial (entramos no meio) → não pode ter sido gravado.
    assert!(
        !secs.iter().any(|(s, _)| *s == 1),
        "S1 parcial não grava: {secs:?}"
    );
}

#[test]
fn blip_de_pit_stall_de_dwell_zero_nao_vira_parada() {
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    // Entra na corrida e assenta o histórico.
    m.record_history(&race_frame(2, 2));

    // Jogador pisca `InPitStall` por um instante (mesmo session_time → dwell 0).
    let mut blip = race_frame(2, 3);
    blip.cars[0].track_surface = SURFACE_IN_PIT_STALL;
    m.record_history(&blip);
    let mut leave = race_frame(2, 3);
    leave.cars[0].track_surface = SURFACE_ON_TRACK; // saiu no mesmo instante
    m.record_history(&leave);

    assert!(
        m.history.pit_stops.is_empty(),
        "blip transiente de pit stall (dwell ~0) não pode virar parada"
    );
}

/// Carro parado no grid com posição NA CLASSE — o que alimenta o snapshot de largada.
fn grid_car(idx: i32, class_position: i32) -> CarSnapshot {
    CarSnapshot {
        idx,
        class_position,
        track_surface: SURFACE_ON_TRACK,
        ..Default::default()
    }
}

#[test]
fn parser_acha_o_numero_da_sessao_de_corrida() {
    let yaml = "SessionInfo:\n Sessions:\n  - SessionNum: 0\n    SessionType: Practice\n  \
                - SessionNum: 1\n    SessionType: Open Qualify\n  - SessionNum: 2\n    \
                SessionType: Race\n";

    assert_eq!(parse_race_session_num(yaml), 2);
}

#[test]
fn parser_da_corrida_nao_confunde_quali_nem_inventa_sessao() {
    // "Lone Qualify" e "Warmup" não podem passar por corrida.
    let so_quali =
        "SessionNum: 0\nSessionType: Practice\nSessionNum: 1\nSessionType: Lone Qualify\n\
                    SessionNum: 2\nSessionType: Warmup\n";
    assert_eq!(parse_race_session_num(so_quali), -1);

    // Sem YAML nenhum também é -1 (e o gate fecha, em vez de capturar grid errado).
    assert_eq!(parse_race_session_num(""), -1);
}

#[test]
fn treino_livre_nao_congela_o_grid() {
    let mut m = RaceMonitor::new();
    m.set_race_session_num(2); // a corrida é a sessão 2
    m.ensure_active(0.0); // `record_history` só grava com tentativa ativa

    // Sessão 0 = treino livre. Os carros já têm posição na classe, mas ela não é grid.
    m.record_history(&IracingTelemetry {
        session_num: 0,
        session_state: STATE_RACING, // treino também chega a "Racing"
        session_time: 50.0,
        cars: vec![grid_car(0, 9), grid_car(1, 1)],
        ..Default::default()
    });
    assert_eq!(
        m.grid_class_pos[0], 0,
        "posição vista no treino não pode virar grid da corrida"
    );
    assert_eq!(m.grid_class_pos[1], 0);

    // Já na corrida, o mesmo set-once vale — é a rede de segurança para quando a
    // transição do verde é perdida.
    m.record_history(&IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time: 50.0,
        cars: vec![grid_car(0, 4), grid_car(1, 2)],
        ..Default::default()
    });
    assert_eq!(m.grid_class_pos[0], 4);
    assert_eq!(m.grid_class_pos[1], 2);
}

/// Recorte com a indentação REAL do dump do iRacing (`Sessions:` é uma lista, então a
/// primeira chave de cada item vem com "- "). Os dois parsers têm de aguentar isso.
const SESSIONS_YAML_REAL: &str = "\
Sessions:
 - SessionNum: 0
   SessionType: Open Qualify
   SessionName: QUALIFY
 - SessionNum: 1
   SessionType: Race
   SessionName: RACE
";

#[test]
fn parsers_de_sessao_aguentam_o_traco_da_lista_do_yaml_real() {
    assert_eq!(parse_qualy_session_num(SESSIONS_YAML_REAL), 0);
    assert_eq!(parse_race_session_num(SESSIONS_YAML_REAL), 1);
}

// ── Histórico: o gap dos vizinhos que alimenta o pós-corrida ─────────────

/// Frame com o jogador (idx 0, P2) entre um carro à frente e um atrás, com
/// `f2_time` e `est_time` controlados. Os `f2_time` são os medidos numa captura
/// real de corrida de IA, onde os dois estavam a ~98 s do líder.
fn frame_de_briga(meu_est: f64, est_frente: f64, est_atras: f64) -> IracingTelemetry {
    let mut eu = on_track(0, 2, 3);
    eu.est_time = meu_est;
    eu.f2_time = 98.294_502_258_300_78;
    eu.best_lap_time = 95.0;
    let mut frente = on_track(1, 1, 3);
    frente.est_time = est_frente;
    frente.f2_time = 98.129_798_889_160_16;
    let mut atras = on_track(2, 3, 3);
    atras.est_time = est_atras;
    atras.f2_time = 98.294_502_258_300_78;
    IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![eu, frente, atras],
        ..Default::default()
    }
}

#[test]
fn gap_dos_vizinhos_sai_do_est_time_e_nao_do_f2_time() {
    // O caso que quebrava: `f2_time` a 0,165 s de distância (os dois a ~98 s do
    // líder) com o carro da frente a 40 s de pista. Pelo `f2_time` isso virava
    // card de rival e uma série de gráfico inventadas.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    m.record_history(&frame_de_briga(10.0, 50.0, 8.0));

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert_eq!((p.ahead_idx, p.behind_idx), (1, 2));
    assert!(
        (p.gap_ahead - 40.0).abs() < 0.01,
        "gap à frente devia ser 40 s de pista, veio {}",
        p.gap_ahead
    );
    assert!(
        (p.gap_behind - 2.0).abs() < 0.01,
        "gap atrás devia ser 2 s de pista, veio {}",
        p.gap_behind
    );
}

#[test]
fn gap_dos_vizinhos_fecha_o_circulo_da_volta() {
    // Eu quase na linha (est 93 de uma volta de 95); ele a 1,5 s dela — já cruzou.
    // A subtração crua daria -91,5, e o carro a um segundo e meio viraria a corrida
    // inteira de distância.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    m.record_history(&frame_de_briga(93.0, 1.5, 90.0));

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert!(
        (p.gap_ahead - 3.5).abs() < 0.01,
        "esperava ~3,5 s fechando o círculo, veio {}",
        p.gap_ahead
    );
}

#[test]
fn sem_vizinho_o_gap_e_desconhecido_e_nao_zero() {
    // O líder não tem ninguém à frente. Um 0 aqui seria lido como "colado nele" —
    // é assim que o card de rival nasce de uma corrida solitária.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());
    let mut eu = on_track(0, 1, 3);
    eu.est_time = 10.0;
    eu.best_lap_time = 95.0;

    m.record_history(&IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![eu],
        ..Default::default()
    });

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert_eq!(p.ahead_idx, -1);
    assert!(p.gap_ahead < 0.0, "sem vizinho o gap não pode ser 0");
    assert_eq!(p.behind_idx, -1);
    assert!(p.gap_behind < 0.0, "sem vizinho o gap não pode ser 0");
}

/// Frame com o jogador na PONTA (idx 0, P1), um carro SENTINELADO (idx 7,
/// `position` 0 — o "sem posição atribuída" do iRacing: garagem, fora do mundo,
/// sessão de quali) e o segundo colocado real (idx 2). O sentinelado vem antes
/// no vetor de propósito: é ele que um `find` desguardado acha primeiro.
fn frame_de_lider_com_sentinela() -> IracingTelemetry {
    let mut eu = on_track(0, 1, 3);
    eu.est_time = 10.0;
    eu.best_lap_time = 95.0;
    let mut fantasma = on_track(7, 0, 0);
    fantasma.est_time = 70.0;
    let mut segundo = on_track(2, 2, 3);
    segundo.est_time = 8.0;
    IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        cars: vec![fantasma, eu, segundo],
        ..Default::default()
    }
}

#[test]
fn lider_nao_ganha_carro_a_frente_fabricado_do_sentinela() {
    // Com o jogador em P1, `me.position - 1` vale 0 — e 0 não é uma posição, é o
    // sentinela de "sem posição atribuída". Sem guarda, o líder recebia como
    // vizinho da frente um carro na garagem, com gap tirado do `est_time` dele.
    // Em captura real (race_1785889561.jsonl.gz) isso era 29% das amostras.
    let mut m = RaceMonitor::new();
    m.attempts.push(active_attempt());

    m.record_history(&frame_de_lider_com_sentinela());

    let p = m.history.player_track.last().expect("amostra de vizinho");
    assert_eq!(
        p.ahead_idx, -1,
        "carro de position 0 não é o carro à frente do líder"
    );
    assert!(
        p.gap_ahead < 0.0,
        "sem vizinho à frente o gap é desconhecido, veio {}",
        p.gap_ahead
    );
    assert_eq!(p.behind_idx, 2, "o carro de trás continua sendo achado");
}

// ── EstadoAgora: as contas que a fala consome ────────────────────────────

#[test]
fn estado_agora_do_lider_nao_inventa_carro_a_frente() {
    // Mesmo sentinela, os outros dois lugares que fazem a busca: a amostra de gap
    // (de onde sai a tendência) e a vizinhança que a fala consome.
    let mut m = RaceMonitor::new();
    let t = frame_de_lider_com_sentinela();

    m.guardar_estado_agora(&t);

    let a = m.gap_hist.last().expect("amostra de gap");
    assert_eq!(a.idx_frente, -1, "position 0 não vira vizinho da frente");
    assert!(a.gap_frente < 0.0, "gap fabricado, veio {}", a.gap_frente);
    assert_eq!(a.idx_atras, 2);

    let e = m.montar_estado_agora();
    assert!(
        e.frente.is_none(),
        "o líder não tem ninguém à frente para o engenheiro comentar"
    );
    assert!(e.atras.is_some(), "o segundo colocado tem de aparecer");
}

#[test]
fn gap_fecha_o_circulo_quando_o_da_frente_ja_cruzou_a_linha() {
    // Eu a 95% de uma volta de 92 s (est 87,4); ele a 2% (est 1,8) — ele acabou de cruzar.
    // A subtração crua daria -85,6; o gap real é ~6,4 s.
    let g = gap_circular(87.4, 1.8, 92.0);
    assert!((g - 6.4).abs() < 0.01, "esperava ~6,4 s, veio {g}");
}

#[test]
fn gap_sem_volta_de_referencia_admite_que_nao_sabe() {
    // Sem tempo de volta não há círculo a fechar, e um número aqui seria invenção.
    assert_eq!(gap_circular(10.0, 4.0, 0.0), -1.0);
    assert_eq!(gap_circular(f64::NAN, 4.0, 92.0), -1.0);
}

#[test]
fn voltas_restantes_por_voltas_vem_do_sim_e_por_tempo_e_estimativa() {
    // Prova por VOLTAS: o sim responde, e a resposta não é estimativa.
    assert_eq!(voltas_restantes(12, -1.0, 92.0), (12, false));
    // Prova por TEMPO: o sim manda o sentinela e a conta é nossa — 500 s a 92 s/volta.
    let (n, estimada) = voltas_restantes(32767, 500.0, 92.0);
    assert_eq!(
        (n, estimada),
        (6, true),
        "sentinela tem de cair na estimativa"
    );
}

#[test]
fn sentinela_de_voltas_nunca_vaza_como_numero() {
    // O defeito que esta constante existe para impedir: anunciar "faltam 32767 voltas".
    // Sem ritmo nem tempo não há como estimar, e o certo é dizer que não se sabe.
    assert_eq!(voltas_restantes(32767, 0.0, 0.0), (-1, false));
}

#[test]
fn alcance_so_existe_quando_a_distancia_encurta() {
    // 8 s de gap, meio segundo por volta mais rápido → 16 voltas.
    let v = voltas_para_alcancar(8.0, -0.5);
    assert!((v - 16.0).abs() < 0.01, "esperava 16 voltas, veio {v}");
    // Mais lento que ele: o alcance nunca acontece. -1, não um número gigante.
    assert_eq!(voltas_para_alcancar(8.0, 0.3), -1.0);
    // Ritmo idêntico: idem — a disputa está congelada.
    assert_eq!(voltas_para_alcancar(8.0, 0.0), -1.0);
}

#[test]
fn tendencia_exige_janela_de_observacao() {
    // Duas amostras a 3 s de distância: dentro do ruído de uma freada, não é tendência.
    let curta = [(100.0, 2.0), (103.0, 1.4)];
    assert_eq!(tendencia_por_volta(&curta, 92.0), 0.0);
    // 20 s de janela, gap caindo de 2,0 para 1,0 → -0,05 s/s × 92 s = -4,6 s por volta.
    let longa = [(100.0, 2.0), (120.0, 1.0)];
    let t = tendencia_por_volta(&longa, 92.0);
    assert!(
        t < 0.0,
        "gap encurtando tem de dar tendência negativa, veio {t}"
    );
    assert!((t + 4.6).abs() < 0.01, "esperava ~-4,6, veio {t}");
}

#[test]
fn bandeira_anuncia_a_mais_grave_e_nao_a_primeira() {
    // A PRETA É INVISÍVEL para este rótulo: no Loop ela é o mecanismo de quebra de peça,
    // não punição. E, por ser a segunda na ordem de gravidade, ela ESCONDIA a amarela de
    // verdade que estivesse ativa junto — que é o caso testado aqui.
    assert_eq!(rotulo_bandeira(FLAG_BLACK | FLAG_CAUTION), "Bandeira amarela");
    assert_eq!(rotulo_bandeira(FLAG_BLACK), "");
    // A desclassificação continua acima de tudo: essa é real.
    assert_eq!(
        rotulo_bandeira(FLAG_DISQUALIFY | FLAG_BLACK),
        "Desclassificado"
    );
    assert_eq!(rotulo_bandeira(FLAG_YELLOW), "Bandeira amarela");
    assert_eq!(rotulo_bandeira(FLAG_CAUTION_WAVING), "Bandeira amarela");
    // Nada digno de nota devolve vazio — não "Verde", que viraria fala sem motivo.
    assert_eq!(rotulo_bandeira(0), "");
}

#[test]
fn saldo_de_combustivel_se_recusa_a_responder_pela_metade() {
    // 14 voltas de autonomia, 12 restantes → sobram 2.
    assert!((saldo_combustivel(14.0, 12) - 2.0).abs() < 0.001);
    // Falta combustível: negativo, e é a informação que faz parar no box.
    assert!(saldo_combustivel(9.0, 12) < 0.0);
    // Sem voltas restantes conhecidas, um saldo seria número com cara de resposta.
    assert!(saldo_combustivel(14.0, -1).is_nan());
    assert!(saldo_combustivel(-1.0, 12).is_nan());
}

#[test]
fn saldo_desconhecido_nao_se_confunde_com_faltar_uma_volta() {
    // O motivo de este campo usar NaN e não -1 como os vizinhos: `-1` é uma RESPOSTA aqui
    // ("falta combustível para uma volta"), e é a resposta que manda o piloto ao box.
    // Com sentinela numérico, ela seria indistinguível de "não sei" — e o dossiê calaria
    // exatamente o fato mais urgente da corrida.
    let falta_uma = saldo_combustivel(11.0, 12);
    assert!(
        (falta_uma + 1.0).abs() < 0.001,
        "esperava -1,0, veio {falta_uma}"
    );
    assert!(
        falta_uma.is_finite(),
        "faltar uma volta é resposta, não ausência"
    );
    assert!(saldo_combustivel(-1.0, 12).is_nan(), "ausência é NaN");
}

#[test]
fn idade_do_pneu_conta_da_ultima_parada_com_troca() {
    use crate::iracing_sdk::tire_strategy::PitStop;
    let parada = |car_idx, lap, secs| PitStop {
        car_idx,
        lap,
        stationary_secs: secs,
        track_wet_at_stop: false,
    };
    // Duas paradas: a da volta 6 só abasteceu (18 s, abaixo do bloco de pneu), a da volta
    // 12 trocou (23 s). Na volta 20, o pneu tem 8 voltas — não 14.
    let paradas = [parada(7, 6, 18.0), parada(7, 12, 23.0)];
    assert_eq!(idade_do_pneu(&paradas, 7, 20), 8);

    // Parada de OUTRO carro não conta. Sem troca própria, o pneu é o da largada e a idade
    // é a corrida inteira — que é a resposta certa, não uma ausência.
    let paradas = [parada(3, 12, 23.0)];
    assert_eq!(idade_do_pneu(&paradas, 7, 20), 20);
    assert_eq!(idade_do_pneu(&[], 7, 20), 20);
}

#[test]
fn abastecimento_longo_nao_e_confundido_com_troca_de_pneu() {
    use crate::iracing_sdk::tire_strategy::PitStop;
    // O caso que a constante de 20 s existe para separar: o maior abastecimento sozinho
    // medido foi ~19 s, e o serviço mínimo de pneu ~21 s. Uma parada de 19,5 s NÃO trocou
    // pneu, e tratá-la como troca zeraria a idade — o engenheiro anunciaria pneu novo num
    // carro que está com o mesmo jogo desde a largada.
    let quase = [PitStop {
        car_idx: 7,
        lap: 12,
        stationary_secs: 19.5,
        track_wet_at_stop: false,
    }];
    assert_eq!(
        idade_do_pneu(&quase, 7, 20),
        20,
        "abastecimento virou troca"
    );
}

#[test]
fn composto_so_e_nomeado_dentro_do_dominio_de_dois() {
    use crate::iracing_sdk::tire_strategy::Compound;
    // O iRacing tem dois compostos e a tradução do índice é exata para eles.
    assert_eq!(Compound::from_indice(0), Compound::Dry);
    assert_eq!(Compound::from_indice(1), Compound::Wet);
    // Fora disso ninguém chuta: -1 é o "não informado" do carro mono-composto, e um 2
    // significaria que a premissa dos dois compostos caiu — em nenhum dos casos vale
    // arredondar para o vizinho mais próximo e sair falando "chuva".
    assert_eq!(Compound::from_indice(-1), Compound::Unknown);
    assert_eq!(Compound::from_indice(2), Compound::Unknown);
}
