use super::quebras::BREAKDOWN_GRACE_SECS;
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
        sim_repair_needed_s: 0.0,
        sim_repair_required_s: 0.0,
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

/// Uma volta de classificatória entregue do jeito que o SDK entrega de verdade.
///
/// O `CarIdxLastLapTime` NÃO acompanha a virada da contagem: no instante em que o carro cruza
/// a linha ele ainda exibe o tempo da volta ANTERIOR (`sdk_antes`), e o valor desta volta só
/// aparece alguns décimos depois. É por isso que um tique isolado não fecha volta nenhuma —
/// ver `race_monitor/voltas.rs`, e a medição que originou o módulo.
fn volta_de_quali(
    m: &mut RaceMonitor,
    session_num: i32,
    idx: i32,
    volta: i32,
    tempo: f64,
    sdk_antes: f64,
    best: f64,
) {
    let inicio = volta as f64 * 200.0;
    for (agora, contagem, sdk) in [
        (inicio, volta - 1, sdk_antes),
        (inicio + tempo, volta, sdk_antes),
        (inicio + tempo + 0.2, volta, tempo),
    ] {
        let mut car = on_track(idx, 0, contagem);
        car.last_lap_time = sdk;
        car.best_lap_time = best;
        m.capture_qualy(&IracingTelemetry {
            session_num,
            session_time: agora,
            cars: vec![car],
            ..Default::default()
        });
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

    let o = m.build_race_outcome(&ev, 12, Some("leve".to_string()));

    assert_eq!(o.posicao_final, 4);
    assert_eq!(o.posicao_grid, 9);
    // Só a classe do jogador, e o pace car fora da conta.
    assert_eq!(o.carros_na_classe, 3);
    assert_eq!(o.voltas, 12);
    assert_eq!(o.incidentes, 4);
    assert!(o.off_track);
    assert_eq!(o.carro.as_deref(), Some("Global Mazda MX-5 Cup"));
    assert_eq!(o.pior_batida.as_deref(), Some("leve"));
}

/// O reinício vem do CONTADOR POR SESSÃO, e não do número da tentativa.
///
/// Antes de 10/08/2026 o desfecho mandava `attempt_number - 1`, e a tentativa também é
/// recriada a cada troca de sessão: um fim de semana normal (treino → quali → corrida,
/// sem ninguém reiniciar nada) chegava ao servidor como "duas largadas refeitas". Este
/// teste existe para essa conta não voltar.
#[test]
fn reinicio_conta_por_sessao_e_nao_por_numero_da_tentativa() {
    let mut m = monitor_com_historico();

    // 3ª tentativa do fim de semana (treino, quali, corrida), zero reinícios.
    let limpo = m.build_race_outcome(&AttemptEvidence::default(), 12, None);
    assert_eq!(
        limpo.restarts, 0,
        "troca de sessão virou reinício de corrida"
    );
    assert_eq!(limpo.restarts_quali, 0);

    // Agora com reinícios de verdade, cada um na sua sessão.
    m.restarts_corrida = 1;
    m.restarts_quali = 2;
    let refeito = m.build_race_outcome(&AttemptEvidence::default(), 12, None);
    assert_eq!(refeito.restarts, 1);
    assert_eq!(
        refeito.restarts_quali, 2,
        "quali e corrida não podem cair no mesmo balde"
    );
}

#[test]
fn melhor_volta_ignora_volta_invalida_e_outra_classe() {
    let m = monitor_com_historico();
    let o = m.build_race_outcome(&AttemptEvidence::default(), 12, None);

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

    let o = m.build_race_outcome(&AttemptEvidence::default(), 5, None);

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
    // A sessão da telemetria (`session_num` 0) é a CORRIDA — sem isto o gate de sessão da
    // quebra cortaria o tick e nem o desgaste seria medido.
    m.race_session_num = 0;

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

// ── Onde a quebra pode acontecer: corrida, e não antes dos 3 minutos ────────

/// Monitor com o carro do jogador (nº 7) de motor ALÉM DA PAREDE (110%): a próxima volta
/// avaliada quebra na certa (falha forçada, sem depender de sorte). A sessão de CORRIDA é a
/// de número 2 — a 1 faz o papel da classificatória.
fn monitor_com_motor_estourado() -> RaceMonitor {
    let mut m = RaceMonitor::new();
    m.history.player_car_idx = 0;
    m.car_number[0] = 7;

    let mut car = crate::car::Car::uniform(3);
    car.set_wear(crate::car::PartType::Engine, 1.25); // além da PAREDE (120%) → falha forçada
    let live = crate::car::breakdown::LiveBreakdown::new(&car, 42, 50.0, (1.0, 1.0, 1.0));
    let mut dir = crate::car::breakdown::BreakdownDirector::new();
    dir.add_car(7, live, Vec::new());
    dir.prime_lap(7, 0);
    m.breakdown = Some(dir);
    m.breakdown_needs_prime = false;
    m.race_session_num = 2;
    m
}

/// Frame na pista, verde, com o jogador (nº 7) cruzando a volta `lap`.
fn frame_de_quebra(session_num: i32, session_time: f64, lap: i32) -> IracingTelemetry {
    IracingTelemetry {
        session_num,
        session_time,
        session_state: STATE_RACING,
        player_car_idx: 0,
        lap_completed: lap,
        cars: vec![on_track(0, 1, lap)],
        ..Default::default()
    }
}

#[test]
fn peca_nao_quebra_na_classificatoria() {
    // A quali também passa por `SessionState = Racing` — o estado sozinho nunca separou as
    // duas. Aqui a sessão viva é a 1 (quali) e a corrida é a 2: nem comando, nem log.
    let mut m = monitor_com_motor_estourado();
    let t = frame_de_quebra(1, 600.0, 1); // muito além da carência, e ainda assim quali
    m.tick_breakdown_player(&t);
    m.tick_breakdown_grid(&t);
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "quali não pode gerar !black nem !dq: {:?}",
        m.pending_breakdown_cmds
    );
    assert!(
        m.breakdown_log.is_empty(),
        "quali não pode registrar quebra"
    );
}

#[test]
fn peca_nao_quebra_nos_tres_primeiros_minutos_de_corrida() {
    let mut m = monitor_com_motor_estourado();
    let t = frame_de_quebra(2, 60.0, 1); // 1 min de corrida
    m.tick_breakdown_player(&t);
    m.tick_breakdown_grid(&t);
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "dentro da carência de largada nada pode largar: {:?}",
        m.pending_breakdown_cmds
    );
}

#[test]
fn peca_quebra_normalmente_depois_da_carencia() {
    // Prova que os dois testes acima não passam por acidente: a MESMA peça, na mesma volta,
    // larga assim que a carência termina.
    let mut m = monitor_com_motor_estourado();
    let verde = frame_de_quebra(2, 10.0, 1);
    m.tick_breakdown_player(&verde); // marca o verde; nada quebra
    let depois = frame_de_quebra(2, 10.0 + BREAKDOWN_GRACE_SECS, 2);
    m.tick_breakdown_player(&depois);
    assert!(
        m.pending_breakdown_cmds.iter().any(|c| c.contains("#7")),
        "passada a carência, o motor estourado tem que largar: {:?}",
        m.pending_breakdown_cmds
    );
}

#[test]
fn a_carencia_nao_congela_o_desgaste_das_pecas() {
    // A restrição é do EVENTO de quebra, não do desgaste: durante a carência o carro continua
    // gastando, senão os 3 minutos viriam de graça pra vida das peças.
    let mut m = monitor_com_quebra_do_jogador().0;
    let antes = desgaste_da_eletronica(&m);
    let t = IracingTelemetry {
        session_state: STATE_RACING,
        player_car_idx: 0,
        lap_completed: 1,
        cars: vec![on_track(0, 1, 1)],
        ..Default::default()
    };
    m.tick_breakdown_player(&t); // session_time 0 → dentro da carência
    assert!(
        desgaste_da_eletronica(&m) > antes,
        "a peça tem que envelhecer mesmo sem poder quebrar"
    );
}

// ── Reinício de sessão: carro inteiro de novo, e sorte nova ─────────────────

/// Roda 12 voltas de 20 carros no diretor e devolve o ROTEIRO de quebras:
/// `(nº do carro, volta, peça)`. É a assinatura do que aconteceu naquela corrida.
fn roteiro_de_quebras(
    dir: &mut crate::car::breakdown::BreakdownDirector,
) -> Vec<(u32, u32, &'static str)> {
    let clima = crate::car::breakdown::Weather::NEUTRAL;
    let mut out = Vec::new();
    for lap in 1..=12u32 {
        for num in 1..=20u32 {
            for ev in dir.on_lap_at(num, lap, clima, 0.0) {
                out.push((num, ev.lap, ev.part.as_str()));
            }
        }
    }
    out
}

/// Grade de 20 carros com o motor na janela de risco (95%): quem larga e em que volta é
/// SORTE, não a parede — que é o que precisa mudar entre uma tentativa e outra.
fn grade_na_janela_de_risco() -> crate::car::breakdown::BreakdownDirector {
    let mut dir = crate::car::breakdown::BreakdownDirector::new();
    for num in 1..=20u32 {
        let mut car = crate::car::Car::uniform(3);
        car.set_wear(crate::car::PartType::Engine, 0.95);
        let live =
            crate::car::breakdown::LiveBreakdown::new(&car, u64::from(num), 50.0, (1.0, 1.0, 1.0));
        dir.add_car(num, live, Vec::new());
    }
    dir
}

#[test]
fn reinicio_devolve_a_grade_inteira_e_com_sorte_nova() {
    let mut m = RaceMonitor::new();
    m.breakdown_base = Some(grade_na_janela_de_risco());

    // 1ª tentativa: a sorte com que o diretor foi instalado.
    m.current_attempt = 1;
    m.restaurar_diretor_de_quebra();
    let primeira = roteiro_de_quebras(m.breakdown.as_mut().expect("diretor restaurado"));
    assert!(
        !primeira.is_empty(),
        "o cenário tem que produzir quebras, senão o teste não mede nada"
    );

    // O jogador bateu e reiniciou: tentativa nova.
    m.current_attempt = 2;
    m.restaurar_diretor_de_quebra();
    let dir = m.breakdown.as_mut().expect("diretor restaurado");

    // O carro que largou uma peça na tentativa anterior está inteiro de novo — a peça voltou
    // pro desgaste de entrada e segue na janela, correndo o mesmo risco.
    let quebrado = primeira[0].0;
    assert!(
        dir.car_parts_in_danger(quebrado)
            .iter()
            .any(|(_, pt, _)| *pt == crate::car::PartType::Engine),
        "o motor do carro {quebrado} tinha que voltar à janela de risco"
    );

    let segunda = roteiro_de_quebras(dir);
    assert!(!segunda.is_empty(), "a corrida refeita também tem quebras");
    assert_ne!(
        primeira, segunda,
        "o reinício não pode repetir o mesmo roteiro de quebras"
    );
}

#[test]
fn a_primeira_tentativa_mantem_a_sorte_instalada() {
    // O re-roll é do REINÍCIO. Sem ele na tentativa 1, o disparo ao vivo continua na mesma
    // família de semente do export (e do aviso pré-corrida).
    let mut m = RaceMonitor::new();
    m.breakdown_base = Some(grade_na_janela_de_risco());
    m.current_attempt = 1;
    m.restaurar_diretor_de_quebra();
    let restaurado = roteiro_de_quebras(m.breakdown.as_mut().expect("diretor restaurado"));

    let instalado = roteiro_de_quebras(&mut grade_na_janela_de_risco());
    assert_eq!(restaurado, instalado);
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

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);

    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 1);
    let lap = &snapshot[0];
    assert_eq!(lap.car_idx, 7);
    assert_eq!(lap.lap, 1);
    assert!((lap.time - 82.345).abs() < f64::EPSILON);
}

/// O bug de origem, no lado da quali: o tempo lido no tique da virada é o da volta anterior.
/// Sem o ciclo de volta, a volta 2 era gravada com o tempo da volta 1.
#[test]
fn a_volta_de_quali_nao_leva_o_tempo_da_anterior() {
    let mut m = RaceMonitor::new();
    m.qualy_session_num = 1;

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
    volta_de_quali(&mut m, 1, 7, 2, 81.100, 82.345, 81.100);

    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].lap, 1);
    assert!((snapshot[0].time - 82.345).abs() < f64::EPSILON);
    assert_eq!(snapshot[1].lap, 2);
    assert!(
        (snapshot[1].time - 81.100).abs() < f64::EPSILON,
        "a volta 2 tem de levar o tempo DA 2, não os 82,345 que o campo ainda exibia"
    );
}

/// A regra da classificatória: o tempo que vale é o VÁLIDO. O carro corta a pista e
/// marca 80,1 s; o iRacing anula a volta, então o `CarIdxBestLapTime` continua nos
/// 82,3 s da volta limpa, e é esse que a torre tem de usar. As voltas cruas guardam a
/// cortada com o tempo que ela marcou, e é justamente por isso que elas não servem
/// para ordenar a quali.
#[test]
fn melhor_volta_valida_da_quali_ignora_a_volta_anulada() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
    // Volta seguinte, mais rápida no relógio e anulada: o melhor VÁLIDO não se move.
    volta_de_quali(&mut m, 1, 7, 2, 80.100, 82.345, 82.345);

    assert_eq!(
        m.qualy_best_valid_snapshot(),
        vec![(7, 82.345)],
        "a volta anulada não pode virar o melhor tempo da classificatória"
    );
    // A volta crua continua registrada — ela alimenta outros consumidores.
    assert_eq!(m.qualy_laps_snapshot().len(), 2);
}

/// Carro na garagem SAI de `cars`, e com ele sairia o `CarIdxBestLapTime`. Sem a trava,
/// o piloto que marcou o melhor tempo e voltou pro box desaparecia da ordenação.
#[test]
fn melhor_volta_valida_sobrevive_ao_carro_sair_do_mundo() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    car.best_lap_time = 82.345;
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    });

    // Tique seguinte já sem o carro no mundo (foi pra garagem).
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![],
        ..Default::default()
    });

    assert_eq!(m.qualy_best_valid_snapshot(), vec![(7, 82.345)]);
}

/// Quali nova zera o melhor válido junto com o resto. Sem isso o tempo da pista
/// anterior seguiria ordenando a torre.
#[test]
fn quali_nova_zera_o_melhor_valido() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    let mut car = on_track(7, 0, 1);
    car.last_lap_time = 82.345;
    car.best_lap_time = 82.345;
    m.capture_qualy(&IracingTelemetry {
        session_num: 1,
        cars: vec![car],
        ..Default::default()
    });
    assert_eq!(m.qualy_best_valid_snapshot().len(), 1);

    m.set_qualy_session_num(2);

    assert!(m.qualy_best_valid_snapshot().is_empty());
}

#[test]
fn troca_do_numero_da_quali_reseta_estado_e_captura_a_sessao_nova() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
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
    assert!(
        m.voltas_quali.iter().all(|c| c.ocioso()),
        "uma volta aberta na quali anterior fecharia dentro da nova"
    );

    volta_de_quali(&mut m, 2, 7, 1, 81.234, 0.0, 81.234);
    assert!(m.prev_in_qualy);
    let snapshot = m.qualy_laps_snapshot();
    assert_eq!(snapshot.len(), 1);
    assert!((snapshot[0].time - 81.234).abs() < f64::EPSILON);
    assert_eq!(snapshot[0].lap, 1);
}

#[test]
fn troca_de_subsession_reseta_quali_com_session_num_igual() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);
    m.set_session_subsession_id(1001);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);
    assert_eq!(m.qualy_laps_snapshot().len(), 1);

    m.set_session_subsession_id(1002);

    assert_eq!(m.session_subsession_id, 1002);
    assert_eq!(m.qualy_session_num, 1);
    assert!(!m.prev_in_qualy);
    assert!(m.qualy_laps_snapshot().is_empty());
    assert!(m.voltas_quali.iter().all(|c| c.ocioso()));
}

#[test]
fn subsession_zero_nao_descarta_estado_valido() {
    let mut m = RaceMonitor::new();
    m.set_qualy_session_num(1);
    m.set_session_subsession_id(1001);

    volta_de_quali(&mut m, 1, 7, 1, 82.345, 0.0, 82.345);

    m.set_session_subsession_id(0);

    assert_eq!(m.session_subsession_id, 1001);
    assert!(m.prev_in_qualy);
    assert_eq!(m.qualy_laps_snapshot().len(), 1);
    assert!(
        !m.voltas_quali[7].ocioso(),
        "subsession 0 é sentinela e não pode zerar o ciclo de volta em andamento"
    );
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

/// O retrato que segura o cabeçalho da torre em prova por TEMPO, onde não existe total
/// previsto para limitar a conta.
#[test]
fn a_bandeirada_congela_a_volta_do_lider() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    // Corrida rolando na última volta: o líder fechou 7.
    m.observe(&race_frame(2, 7));
    assert_eq!(
        m.volta_final_lider, 0,
        "corrida em andamento não congela nada"
    );

    // O líder cruza a linha final: 8 completas, e a bandeirada cai no mesmo frame.
    let mut bandeirada = race_frame(2, 8);
    bandeirada.session_state = STATE_CHECKERED;
    m.observe(&bandeirada);
    assert_eq!(m.volta_final_lider, 8);

    // Cool down com o pelotão ainda girando: `lap_completed` sobe e o retrato não.
    let mut cool_down = race_frame(2, 9);
    cool_down.session_state = STATE_CHECKERED + 1;
    m.observe(&cool_down);
    assert_eq!(
        m.volta_final_lider, 8,
        "o congelamento é do instante da bandeirada, não do último frame visto"
    );
}

/// Treino e classificatória também chegam a `STATE_CHECKERED`, e nenhum dos dois tem
/// volta final de corrida para congelar.
#[test]
fn bandeirada_fora_da_corrida_nao_congela_volta() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    let mut quali = race_frame(1, 4);
    quali.session_state = STATE_CHECKERED;
    m.observe(&quali);

    assert_eq!(m.volta_final_lider, 0);
}

#[test]
fn total_da_prova_nao_entrega_o_sentinela_ao_engenheiro() {
    // Prova por TEMPO: `SessionLapsTotal` vem sentinelado e o contrato do campo é 0.
    // Sem isto o contexto do rádio dizia "Volta: 12 de 32767".
    assert_eq!(volta_e_total(12, 32767), (12, 0));
    // Prova por VOLTAS: o total é o número da prova mesmo.
    assert_eq!(volta_e_total(7, 8), (7, 8));
}

#[test]
fn volta_do_engenheiro_para_no_fim_da_prova() {
    // Última volta fechada: o jogador completou as 8 de uma prova de 8.
    assert_eq!(volta_e_total(8, 8), (8, 8));
    // Cool down: o jogador segue girando e o `lap_completed` sobe com a corrida acabada.
    // A prova não tem uma nona volta para o rádio anunciar.
    assert_eq!(volta_e_total(9, 8), (8, 8));
    // Sem total conhecido não há teto a aplicar, e o número segue como veio.
    assert_eq!(volta_e_total(9, 0), (9, 0));
    // Antes da primeira volta o SDK pode mandar -1; volta negativa não existe.
    assert_eq!(volta_e_total(-1, 8), (0, 8));
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
    assert_eq!(
        rotulo_bandeira(FLAG_BLACK | FLAG_CAUTION),
        "Bandeira amarela"
    );
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

// ── Dano do jogador: o que o monitor aceita como batida ──────────────────────
/// Frame do jogador (idx 0) correndo, com os canais que o scorer de batida lê.
fn frame_do_jogador(session_num: i32, incidentes: i32, surface: i32) -> IracingTelemetry {
    IracingTelemetry {
        session_num,
        session_state: STATE_RACING,
        session_time: 100.0,
        player_car_idx: 0,
        lap_completed: 3,
        incident_count: incidentes,
        track_surface: surface,
        speed_ms: 50.0,
        cars: vec![on_track(0, 2, 3), on_track(1, 1, 3)],
        ..Default::default()
    }
}

/// Uma RODADA sem tocar em nada: pontos de incidente, guinada, rotação e fora da
/// pista somam bem acima de "moderado" no scorer — mas o carro não bateu. O pico, que
/// é a base do conserto, não pode registrar nada.
#[test]
fn rodada_limpa_nao_alimenta_o_pico_de_batida() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    // Tick de referência (estabelece o `prev_incident`).
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    // Roda: +2 de incidente (perda de controle), guinada e rotação violentas, fora da
    // pista. Nenhuma aceleração de impacto — não houve contato.
    let mut rodada = frame_do_jogador(2, 2, SURFACE_OFF_TRACK);
    rodada.yaw_rate = 5.0;
    rodada.roll_rate = 5.0;
    rodada.speed_ms = 10.0;
    m.observe(&rodada);

    assert!(m.in_crash, "o scorer ainda vê o evento (bandeira, DNF)");
    let a = m.attempts.last().expect("tentativa ativa");
    assert_eq!(
        a.peak_crash_score, 0.0,
        "sem impacto não há dano: o pico é a base do conserto"
    );
    assert!(a.peak_impact_dir.is_none());

    // Fecha a batida (silêncio além da janela de fusão) e confere a marca do evento.
    let mut depois = frame_do_jogador(2, 2, SURFACE_ON_TRACK);
    depois.session_time = 100.0 + MERGE_WINDOW_SECS + 1.0;
    m.observe(&depois);
    let crash = m.attempts.last().unwrap().crashes.first().expect("evento");
    assert!(crash.score >= SEV_MODERATE, "o evento pontuou alto");
    assert!(!crash.had_impact, "mas sem impacto — não vira conserto");
}

/// DUPLA CONFIRMAÇÃO: o sim passando a pedir reparo, com uma batida em curso, confirma o
/// impacto que o G não pegou (toque de baixa energia que quebra a asa).
#[test]
fn reparo_pedido_pelo_sim_confirma_o_impacto() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    // Abre a janela de batida sem nenhum G: incidente, guinada, rotação, fora da pista.
    let mut rodada = frame_do_jogador(2, 2, SURFACE_OFF_TRACK);
    rodada.yaw_rate = 5.0;
    rodada.roll_rate = 5.0;
    m.observe(&rodada);
    assert_eq!(m.attempts.last().unwrap().peak_crash_score, 0.0);

    // O sim passa a pedir reparo: o carro quebrou de verdade.
    let mut quebrou = frame_do_jogador(2, 2, SURFACE_OFF_TRACK);
    quebrou.pit_opt_repair_needed = 12.0;
    m.observe(&quebrou);

    let a = m.attempts.last().expect("tentativa ativa");
    assert!(a.peak_crash_score >= SEV_MODERATE, "o dano passa a contar");
    assert_eq!(a.sim_repair_needed_s, 12.0);
    assert!(
        a.peak_impact_dir.is_none(),
        "sem G não há eixo dominante: a direção não pode ser inventada"
    );
}

/// A ausência dos canais de reparo não conclui nada: o carro que bateu com G continua
/// contando mesmo com o sim calado.
#[test]
fn silencio_do_reparo_nao_cancela_a_batida() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    let mut pancada = frame_do_jogador(2, 4, SURFACE_ON_TRACK);
    pancada.long_accel = -60.0; // e nenhum reparo reportado
    m.observe(&pancada);

    let a = m.attempts.last().expect("tentativa ativa");
    assert!(a.peak_crash_score > 0.0);
    assert_eq!(a.sim_repair_needed_s, 0.0);
}

/// O PICO é sempre um piso: a velocidade PERDIDA na pancada, que é o componente que separa
/// o encostão da sucata (vale até 160 pontos), só é calculada quando a batida FECHA. Ler só
/// o pico chamava de "leve" um carro que morreu no muro.
#[test]
fn pior_batida_bruta_nao_se_contenta_com_o_pico() {
    let mut a = active_attempt();
    a.peak_crash_score = 46.0; // "leve" — G + contato, sem a velocidade perdida
    assert_eq!(worst_raw_severity(&a), "leve");

    a.crashes.push(CrashEvent {
        session_time: 10.0,
        lap: 1,
        score: 200.0,
        severity: "destruído".to_string(),
        impact_severity: "destruído".to_string(),
        had_impact: true,
        factors: vec![],
    });
    assert_eq!(
        worst_raw_severity(&a),
        "destruído",
        "a batida fechada tem a conta inteira e tem de vencer o pico"
    );
}

// ── Carro destruído na CLASSIFICAÇÃO ─────────────────────────────────────────
/// Monitor com a regra armada e a quali (sessão 1) já vivida com o carro `num`: o jogador
/// bateu de verdade e o sim pediu `reparo_s` de conserto OBRIGATÓRIO.
fn monitor_apos_quali_com_numero(reparo_s: f64, armado: bool, num: i32) -> RaceMonitor {
    let mut m = RaceMonitor::new();
    m.quali_wreck_on = Some(armado);
    m.qualy_session_num = 1;
    m.race_session_num = 2;
    m.history.player_car_idx = 0;
    m.car_number[0] = num;

    m.observe(&frame_do_jogador(1, 0, SURFACE_ON_TRACK));
    let mut destruiu = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    destruiu.long_accel = -60.0; // pancada de verdade
    destruiu.pit_repair_needed = reparo_s;
    m.observe(&destruiu);
    m
}

fn monitor_apos_quali(reparo_s: f64, armado: bool) -> RaceMonitor {
    monitor_apos_quali_com_numero(reparo_s, armado, 64)
}

/// Primeiro tick da CORRIDA, ainda na formação (antes do verde).
fn frame_de_formacao() -> IracingTelemetry {
    let mut f = frame_do_jogador(2, 0, SURFACE_ON_TRACK);
    f.session_state = STATE_RACING - 1;
    f
}

/// Batida "grave" (aqui via piso do reparo): a quali TRAVA na hora (`!dq` ao vivo, com o
/// motivo no rádio) e a corrida sai do fundo — `!clear` limpa a ficha da quali e o `!eol`
/// aplica o castigo certo.
#[test]
fn batida_grave_trava_a_quali_e_larga_do_fundo() {
    let mut m = monitor_apos_quali(40.0, true);
    // O lockout saiu DENTRO da quali, no instante da batida.
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
    let aviso = m.player_warning_log.last().expect("motivo no rádio, na hora");
    assert!(matches!(aviso.tipo, TipoAvisoProprio::QualiDestruida));
    assert_eq!(aviso.severidade, "quali_grave");

    // Na virada para a corrida a fila é da tentativa nova (o `!dq` do lockout já foi
    // drenado ao sim ao vivo — o despacho real é a cada ~1,5 s): sobra o par da corrida.
    m.observe(&frame_de_formacao());
    assert_eq!(
        m.pending_breakdown_cmds,
        vec!["!clear #64".to_string(), "!eol #64".to_string()]
    );
    assert_eq!(m.player_warning_log.last().unwrap().severidade, "eol");
    // E não repete no tick seguinte: o castigo é um só.
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds.len(), 2);
}

/// Batida "destruído": DQ na quali E na corrida (reafirmado, sem `!clear`).
#[test]
fn carro_irrecuperavel_nao_corre_o_fim_de_semana() {
    let mut m = monitor_apos_quali(80.0, true);
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
    assert_eq!(
        m.player_warning_log.last().unwrap().severidade,
        "quali_destruido"
    );

    // A fila da corrida é nova (o `!dq` da quali já foi drenado ao vivo); o DQ é reafirmado.
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
    assert_eq!(m.player_warning_log.last().unwrap().severidade, "dq");
}

/// A severidade decide sozinha quando os outros canais ficam mudos — e "catastrófico" só
/// muda a fala do rádio, não a consequência (DQ nos dois casos).
#[test]
fn severidade_castiga_mesmo_com_o_canal_de_reparo_mudo() {
    // Pico mutado DEPOIS da quali → o lockout ao vivo não viu; a fronteira pega.
    let mut m = monitor_apos_quali(0.0, true);
    m.attempts.last_mut().unwrap().peak_crash_score = 180.0; // "destruído"
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);

    let mut m = monitor_apos_quali(0.0, true);
    m.attempts.last_mut().unwrap().peak_crash_score = 240.0; // "catastrófico"
    m.observe(&frame_de_formacao());
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
}

/// O rádio do lockout gradua pela severidade: "catastrófico" pergunta se o piloto está
/// inteiro em vez de falar de conserto.
#[test]
fn catastrofico_muda_a_fala_do_lockout() {
    let mut m = monitor_apos_quali_com_numero(0.0, true, 64);
    m.attempts.last_mut().unwrap().peak_crash_score = 240.0;
    // Mais um tick de quali para o lockout ao vivo avaliar o pico já alto.
    m.observe(&frame_do_jogador(1, 4, SURFACE_ON_TRACK));
    assert_eq!(
        m.player_warning_log.last().unwrap().severidade,
        "quali_catastrofico"
    );
}

/// O MEATBALL é piso de "grave": o sim declarou reparo obrigatório, e isso trava a quali
/// mesmo quando o score fica curto (pista molhada e G subamostrado marcaram "grave" num
/// carro sem roda — caso medido em 2026-08-10).
#[test]
fn meatball_na_quali_trava_mesmo_com_score_curto() {
    let mut m = monitor_apos_quali(0.0, true);
    let mut meatball = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    meatball.session_flags = 0x0010_0000; // FLAG_REPAIR
    m.observe(&meatball);
    // Lockout ao vivo, na quali mesmo.
    assert_eq!(m.pending_breakdown_cmds, vec!["!dq #64".to_string()]);
    assert_eq!(
        m.player_warning_log.last().unwrap().severidade,
        "quali_grave"
    );

    m.observe(&frame_de_formacao());
    assert_eq!(
        m.pending_breakdown_cmds,
        vec!["!clear #64".to_string(), "!eol #64".to_string()]
    );
}

/// Se o carro PIOROU depois do lockout (o pico subiu a "destruído" após o `!dq` ao vivo), a
/// fronteira PROMOVE a pendência: eol vira dq, nunca o contrário.
#[test]
fn piorar_o_carro_depois_do_lockout_promove_o_castigo() {
    let mut m = monitor_apos_quali(0.0, true);
    let mut meatball = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    meatball.session_flags = 0x0010_0000;
    m.observe(&meatball); // lockout "grave" → pendência eol
    m.attempts.last_mut().unwrap().peak_crash_score = 180.0; // piorou: "destruído"

    m.observe(&frame_de_formacao());
    assert_eq!(
        m.pending_breakdown_cmds,
        vec!["!dq #64".to_string()],
        "a corrida tem de sair como DQ, sem clear"
    );
}

/// O castigo tem de sair NA HORA, com a batida ainda aberta. A velocidade perdida é o
/// componente que separa o encostão da destruição, e só o FECHAMENTO da batida a gravava —
/// esperar dez segundos de silêncio numa quali destruída é esperar por algo que pode nunca
/// vir. Medido em 2026-08-10: a fronteira dizia "grave" e o lockout nunca saía.
#[test]
fn o_lockout_nao_espera_a_batida_fechar() {
    let mut m = RaceMonitor::new();
    m.quali_wreck_on = Some(true);
    m.qualy_session_num = 1;
    m.race_session_num = 2;
    m.history.player_car_idx = 0;
    m.car_number[0] = 64;

    // Rodando a 60 m/s; a batida ainda não existe.
    let mut rodando = frame_do_jogador(1, 0, SURFACE_ON_TRACK);
    rodando.speed_ms = 60.0;
    m.observe(&rodando);
    assert!(m.pending_breakdown_cmds.is_empty());

    // Muro: contato do sim + G, e o carro para. A batida NÃO fechou (nada de esperar a
    // janela de fusão), mas os 60 m/s perdidos já valem sozinhos mais de "grave".
    let mut muro = frame_do_jogador(1, 4, SURFACE_ON_TRACK);
    muro.long_accel = -60.0;
    muro.speed_ms = 0.0;
    m.observe(&muro);

    assert!(m.in_crash, "a batida segue ABERTA — é esse o cenário");
    assert_eq!(
        m.pending_breakdown_cmds,
        vec!["!dq #64".to_string()],
        "o castigo tem de sair com a batida ainda aberta"
    );
}

/// O rádio emudecia PARA SEMPRE depois de um reinício: o `id` da fala é a posição no log, os
/// logs são esvaziados a cada tentativa, e o overlay só mostra id INÉDITO — então tudo que
/// vinha depois do primeiro reinício era descartado como "já vi essa". Sem erro em lugar
/// nenhum. Medido em 2026-08-10, com o jogador reiniciando a quali várias vezes.
#[test]
fn os_ids_do_radio_nao_voltam_atras_depois_de_um_reinicio() {
    let mut m = RaceMonitor::new();
    m.player_warning_log.push(PlayerWarning {
        tipo: TipoAvisoProprio::Poupar,
        part: "",
        wear_pct: 0,
        severidade: "",
    });
    m.ritmo_log.push(FalaDeRitmo::Tomamos("x".to_string()));
    let ultimo_id_antes = m.radio_epoch + m.player_warning_log.len() - 1;

    m.start_attempt(0.0); // reinício: os logs vão embora

    assert!(
        m.radio_epoch > ultimo_id_antes,
        "o id da PRÓXIMA fala ({}) tem de superar o da última já vista ({ultimo_id_antes})",
        m.radio_epoch
    );
}

/// O limiar é alto de propósito: batida que o box conserta não trava a quali nem custa a
/// etapa — o jogador pode resetar e tentar de novo.
#[test]
fn batida_pequena_na_quali_nao_castiga() {
    let mut m = monitor_apos_quali(8.0, true);
    m.observe(&frame_de_formacao());

    assert!(m.pending_breakdown_cmds.is_empty());
    assert!(m.player_warning_log.is_empty());
}

/// A regra inteira está atrás de flag até os comandos serem confirmados na pista.
#[test]
fn regra_desarmada_nao_castiga_ninguem() {
    let mut m = monitor_apos_quali(80.0, false);
    m.observe(&frame_de_formacao());

    assert!(m.pending_breakdown_cmds.is_empty());
    assert!(m.player_warning_log.is_empty());
    // Mas a tentativa da quali segue identificada, porque o conserto dela é cobrado
    // no import independentemente do castigo esportivo.
    assert!(m.quali_attempt_number > 0);
}

/// Sem o número do carro o lockout adia (sem perder a pendência: a fronteira reavalia), e
/// se a largada vier antes de o YAML entregar o número, o castigo cai na bandeira preta.
#[test]
fn castigo_perdido_na_formacao_vira_bandeira_preta() {
    let mut m = monitor_apos_quali_com_numero(40.0, true, 0); // número desconhecido
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "sem número não há lockout ao vivo"
    );
    m.observe(&frame_de_formacao());
    assert!(m.pending_breakdown_cmds.is_empty(), "sem número, não manda");

    m.car_number[0] = 64;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK)); // já em Racing
    assert_eq!(m.pending_breakdown_cmds, vec!["!black #64 15".to_string()]);
}

/// Contato de verdade (o 4x do próprio iRacing) segue virando dano.
#[test]
fn contato_de_verdade_alimenta_o_pico() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));

    let mut pancada = frame_do_jogador(2, 4, SURFACE_ON_TRACK);
    pancada.long_accel = -60.0; // freada violenta contra o muro
    m.observe(&pancada);

    let a = m.attempts.last().expect("tentativa ativa");
    assert!(a.peak_crash_score > 0.0, "contato tem de contar");
    assert_eq!(a.peak_impact_dir.as_deref(), Some("front"));
}

/// O iRacing troca de sessão na MESMA conexão. A tentativa é o container do dano do
/// jogador: sem cortar na fronteira, a batida do treino continuava viva na corrida e o
/// import cobrava conserto de uma corrida limpa.
#[test]
fn batida_do_treino_nao_atravessa_para_a_corrida() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;

    // TREINO (sessão 0): o jogador bate.
    m.observe(&frame_do_jogador(0, 0, SURFACE_ON_TRACK));
    let mut pancada = frame_do_jogador(0, 4, SURFACE_ON_TRACK);
    pancada.long_accel = -60.0;
    m.observe(&pancada);
    let treino = m.current_attempt;
    assert!(m.attempts.last().unwrap().peak_crash_score > 0.0);

    // CORRIDA (sessão 2): tentativa nova, sem herdar nada.
    m.observe(&frame_do_jogador(2, 0, SURFACE_ON_TRACK));
    assert!(
        m.current_attempt > treino,
        "a corrida tem de abrir uma tentativa própria"
    );
    let a = m.attempts.last().expect("tentativa da corrida");
    assert_eq!(a.status, "active");
    assert_eq!(
        a.peak_crash_score, 0.0,
        "a batida do treino não é dano da corrida"
    );
    assert!(a.crashes.is_empty());
    assert!(a.collided_with_car_number.is_none());
    // A tentativa do treino foi fechada, e por troca de sessão — não por abandono.
    let treino_fechado = m.attempts.iter().find(|x| x.number == treino).unwrap();
    assert_ne!(treino_fechado.status, "active");
    assert_eq!(treino_fechado.ended_by.as_deref(), Some("session_change"));
    assert!(
        !m.events.iter().any(|e| e.kind == "dnf_confirmed"),
        "trocar de sessão não é abandono"
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

// ── Reinício da corrida: a tentativa abandonada não deixa rastro ─────────────

/// Frame de corrida verde com o jogador (idx 0) na pista e o líder (idx 1) à frente. O par
/// (`session_time`, `lap_completed`) é justamente o que o detector de reinício compara.
fn frame_de_corrida(session_time: f64, lap: i32) -> IracingTelemetry {
    IracingTelemetry {
        session_num: 2,
        session_state: STATE_RACING,
        session_time,
        player_car_idx: 0,
        lap_completed: lap,
        track_surface: SURFACE_ON_TRACK,
        // O jogador PILOTANDO: é o que separa o reinício da rebobinada do replay.
        on_track: true,
        speed_ms: 50.0,
        cars: vec![on_track(0, 2, lap), on_track(1, 1, lap)],
        ..Default::default()
    }
}

/// Um monitor já com meia corrida disputada na tentativa que o jogador vai jogar fora.
fn monitor_em_corrida_disputada() -> RaceMonitor {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.prev_session_num = 2;
    m.observe(&frame_de_corrida(100.0, 8));
    m.observe(&frame_de_corrida(720.0, 12));
    m
}

/// O reinício da corrida É um salto de relógio: o `SessionTime` volta de uma vez para perto
/// de zero. Enquanto o guarda de salto de replay zerava o `prev` nesse mesmo tick,
/// `restarted()` ficava sem o que comparar — nem pelo tempo, nem pela queda de
/// `lap_completed` — e o reinício NUNCA era detectado. A tentativa abandonada seguia aberta e
/// tudo que aconteceu nela continuava alimentando a corrida que valeu.
#[test]
fn reinicio_e_detectado_apesar_do_salto_de_relogio() {
    let mut m = monitor_em_corrida_disputada();
    let abandonada = m.current_attempt;
    assert!(
        m.attempts.last().unwrap().evidence.raced,
        "a tentativa tem de ter largado antes do reinício valer"
    );

    // RESTART: o relógio volta ao zero e as voltas somem.
    m.observe(&frame_de_corrida(0.4, 0));

    assert!(
        m.current_attempt > abandonada,
        "o reinício tem de abrir uma tentativa nova"
    );
    let morta = m
        .attempts
        .iter()
        .find(|a| a.number == abandonada)
        .expect("a tentativa abandonada");
    assert_eq!(morta.ended_by.as_deref(), Some("restart"));
    assert_ne!(morta.status, "active");
    let viva = m.attempts.last().expect("tentativa nova");
    assert_eq!(viva.status, "active");
    assert_eq!(viva.laps_completed, 0);
    assert!(!viva.evidence.raced || viva.evidence.incident_points == 0);
}

/// Rebobinar o replay para a largada leva o `SessionTime` de volta ao zero exatamente como um
/// reinício — e com o replay PAUSADO o `is_replay_playing` cai a zero junto. O que separa os
/// dois é o jogador estar dentro do carro: assistindo, a corrida em andamento não pode ser
/// jogada fora, e o `prev` fica congelado até ele voltar a pilotar.
#[test]
fn rebobinar_o_replay_nao_e_reinicio() {
    let mut m = monitor_em_corrida_disputada();
    let disputada = m.current_attempt;

    // Replay pausado na largada: o relógio volta ao zero, o jogador está fora do carro.
    let mut assistindo = frame_de_corrida(0.4, 0);
    assistindo.on_track = false;
    m.observe(&assistindo);
    assert_eq!(
        m.current_attempt, disputada,
        "assistir ao replay não pode descartar a corrida"
    );
    assert_eq!(m.attempts.last().unwrap().status, "active");

    // De volta ao carro, ao vivo, com a corrida onde ela parou.
    m.observe(&frame_de_corrida(725.0, 12));
    assert_eq!(m.current_attempt, disputada);

    // E o reinício de verdade, depois disso, segue sendo pego.
    m.observe(&frame_de_corrida(0.4, 0));
    assert!(m.current_attempt > disputada);
}

/// Nada do que a tentativa abandonada acumulou pode sobreviver ao reinício: é isso que ia
/// parar na notícia, no resumo, no histórico e na carreira.
#[test]
fn tentativa_abandonada_nao_deixa_rastro_no_reinicio() {
    let mut m = monitor_em_corrida_disputada();

    // O que aconteceu na tentativa que o jogador jogou fora.
    m.events.push(RaceEvent {
        session_time: 300.0,
        lap: 5,
        kind: "dnf_confirmed".to_string(),
        car_idx: Some(1),
        detail: "Carro 1 abandonou".to_string(),
        severity: Some("grave".to_string()),
    });
    m.breakdown_log.push(BreakdownOutcome {
        car_number: 12,
        part: "engine".to_string(),
        problem: 0,
        lap: 6,
        severity: "dnf".to_string(),
        penalty_secs: None,
        forced: true,
        label: "Motor".to_string(),
    });
    m.player_pit_laps.push(4);
    m.pending_breakdown_cmds.push("!black #12 15".to_string());
    m.breakdown_repair_laps.push((3, 5));
    m.breakdown_flash_at[3] = 690.0;
    m.breakdown_alert[3] = Some(BreakdownAlert {
        severity: crate::car::breakdown::Severity::Heavy,
        entered_pit_since: false,
    });
    m.player_incidents.push(PlayerIncidentMark {
        lap_f: 5.3,
        points: 4,
        off_track: true,
    });

    m.observe(&frame_de_corrida(0.4, 0));

    // O log de eventos nasce com o marcador do reinício e mais nada — em especial, sem o
    // abandono da IA, que o import lê para marcar DNF e incidente notável.
    assert_eq!(
        m.events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        vec!["race_restarted"],
        "o log de eventos é da tentativa"
    );
    assert!(m.breakdown_log.is_empty(), "quebra da tentativa morta");
    assert!(m.player_pit_laps.is_empty(), "pit da tentativa morta");
    assert!(
        m.pending_breakdown_cmds.is_empty(),
        "um !black enfileirado lá puniria um carro nesta corrida"
    );
    assert!(m.breakdown_repair_laps.is_empty());
    assert_eq!(m.breakdown_flash_at[3], 0.0);
    assert!(m.breakdown_alert[3].is_none());
    assert!(m.player_incidents.is_empty());
    // E o histórico volta a volta cobre a tentativa nova, não a que morreu.
    assert_eq!(m.history.attempt_number, m.current_attempt);
    assert!(m.history.player_laps.is_empty());
}

/// O desgaste que as voltas da tentativa abandonada consumiram e as peças que largaram nela
/// são de uma corrida que não aconteceu — e viram consequência de carreira. O diretor volta
/// ao estado em que foi instalado.
#[test]
fn reinicio_devolve_o_diretor_de_quebra_ao_estado_de_instalacao() {
    let mut m = RaceMonitor::new();
    m.race_session_num = 2;
    m.prev_session_num = 2;
    m.history.player_car_idx = 0;
    m.car_number[0] = 7;

    let mut car = crate::car::Car::uniform(3);
    car.set_wear(crate::car::PartType::Engine, 1.25); // além da PAREDE → falha forçada
    let live = crate::car::breakdown::LiveBreakdown::new(&car, 42, 50.0, (1.0, 1.0, 1.0));
    let mut dir = crate::car::breakdown::BreakdownDirector::new();
    dir.add_car(7, live, Vec::new());
    m.install_breakdown_director(dir, None, crate::car::breakdown::Weather::NEUTRAL, false);
    m.breakdown_needs_prime = false;

    // Tentativa 1: a carência passa e o motor larga.
    m.observe(&frame_de_corrida(100.0, 8));
    m.observe(&frame_de_corrida(100.0 + BREAKDOWN_GRACE_SECS + 60.0, 9));
    assert!(
        !m.breakdown_log.is_empty(),
        "a peça tinha de largar na tentativa abandonada"
    );

    // RESTART.
    m.observe(&frame_de_corrida(0.4, 0));
    assert!(m.breakdown_log.is_empty());

    // Tentativa 2: o motor volta a poder largar — o diretor não guardou a quebra da
    // tentativa morta nem o desgaste das voltas dela.
    m.observe(&frame_de_corrida(BREAKDOWN_GRACE_SECS + 60.0, 3));
    assert!(
        !m.breakdown_log.is_empty(),
        "o diretor tem de voltar ao estado de instalação"
    );
}
