//! Testes de [`super::super::resultado`]: o desfecho que o import consome — posição,
//! tamanho da classe e a contagem de reinícios.

use super::super::*;

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

    let o = m.build_race_outcome(&ev, 12, Some(Severidade::Leve));

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
