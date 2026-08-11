//! Testes de [`super::super::sessao`]: o que os parsers tiram do YAML do fim de semana e
//! o portão de sessão que decide quando o grid é capturado.

use super::super::*;

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
