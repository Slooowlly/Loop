//! Testes de [`super::super::quebras`]: quando uma peça pode quebrar, o clima que ela
//! enxerga e o que um reinício faz com a sorte já sorteada.

use super::super::quebras::BREAKDOWN_GRACE_SECS;
use super::super::*;
use super::comum::*;

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
