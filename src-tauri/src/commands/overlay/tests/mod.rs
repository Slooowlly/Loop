use super::formato::{
    best_positive_lap, contagem_de_voltas, history_matches_subsession, name_key,
    parse_session_times, parse_session_types, ritmo_de_referencia, roster_with_telemetry,
    session_kind, ContagemVoltas,
};
use super::ordem::{modo_da_sessao, ordem_pre_sessao, ordenar, OrderInput, OrderMode, PreSinal};
use crate::iracing_sdk::{race_monitor::YamlCarMeta, CarSnapshot};

/// Carro de teste: sem posição oficial, sem volta, sem grid — cada caso liga só
/// o campo que está exercitando.
fn carro(car_number: i64) -> OrderInput {
    OrderInput {
        class_position: 0,
        lap_completed: -1,
        lap_dist_pct: -1.0,
        best_lap_ms: i64::MAX,
        qualy_best_ms: i64::MAX,
        pre_ordem: i64::MAX,
        car_number,
    }
}

/// Piloto da carreira já com temporada em andamento.
fn no_campeonato(pontos: i32, vitorias: i32) -> PreSinal {
    PreSinal {
        corridas_temporada: 3,
        pontos,
        vitorias,
        podios: vitorias,
        expectativa: 0.0,
        conhecido: true,
    }
}

/// Piloto antes da 1ª corrida: só a expectativa de pré-temporada existe.
fn na_pre_temporada(expectativa: f64) -> PreSinal {
    PreSinal {
        corridas_temporada: 0,
        pontos: 0,
        vitorias: 0,
        podios: 0,
        expectativa,
        conhecido: true,
    }
}

// ─── Ritmo de referência da estimativa de voltas (bug #6, parte 2) ───────────

#[test]
fn o_ritmo_e_a_mediana_das_ultimas_voltas_do_lider() {
    // Cinco voltas: a mediana ignora a mais rápida e a mais lenta.
    let voltas = [92.0, 90.5, 91.0, 90.8, 91.2];
    assert_eq!(ritmo_de_referencia(&voltas, Some(89.0)), Some(91.0));
}

#[test]
fn uma_parada_de_box_nao_arrasta_o_ritmo() {
    // O caso que a média estragaria: um in-lap de 2 minutos entre voltas normais.
    let voltas = [91.0, 90.8, 120.0, 91.2, 91.1];
    let ritmo = ritmo_de_referencia(&voltas, Some(89.0)).expect("há voltas suficientes");
    assert!(
        (90.0..=92.0).contains(&ritmo),
        "a parada não pode virar o ritmo da prova: {ritmo}"
    );
}

#[test]
fn so_as_ultimas_voltas_contam() {
    // A prova evolui (pista borrachando, combustível queimando): voltas do começo não
    // dizem mais qual é o ritmo agora.
    let voltas = [99.0, 99.0, 99.0, 99.0, 91.0, 91.1, 90.9, 91.2, 91.0];
    let ritmo = ritmo_de_referencia(&voltas, None).expect("há voltas suficientes");
    assert!((90.5..=91.5).contains(&ritmo), "{ritmo}");
}

#[test]
fn sem_voltas_suficientes_cai_na_melhor_do_campo() {
    // Começo de corrida: ninguém fechou cinco voltas ainda, e um número aproximado no
    // cabeçalho é melhor que nenhum.
    assert_eq!(ritmo_de_referencia(&[], Some(89.0)), Some(89.0));
    assert_eq!(ritmo_de_referencia(&[91.0, 91.5], Some(89.0)), Some(89.0));
}

#[test]
fn sem_volta_nenhuma_nao_ha_estimativa() {
    assert_eq!(ritmo_de_referencia(&[], None), None);
    assert_eq!(ritmo_de_referencia(&[], Some(0.0)), None);
    assert_eq!(ritmo_de_referencia(&[0.0, 0.0, 0.0], None), None);
}

#[test]
fn a_mediana_do_lider_estima_menos_voltas_que_a_melhor_do_campo() {
    // O defeito, medido: dividir pela melhor volta absoluta subestima o tempo por volta
    // e, portanto, SUPERESTIMA quantas voltas cabem. Com 20 min restantes e ritmo real
    // de 91 s, a melhor volta de 88 s inventava voltas que não cabem.
    let voltas = [91.0, 91.2, 90.9, 91.1, 91.0];
    let restante: f64 = 20.0 * 60.0;
    let pela_melhor = (restante / 88.0).round() as i32;
    let ritmo = ritmo_de_referencia(&voltas, Some(88.0)).expect("há voltas");
    let pela_mediana = (restante / ritmo).round() as i32;
    assert!(
        pela_mediana < pela_melhor,
        "a mediana ({pela_mediana}) tem de ser mais conservadora que a melhor volta ({pela_melhor})"
    );
}

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

/// Sentinela de "ilimitado" que o iRacing manda nos canais de voltas em prova por tempo.
const ILIMITADO: i32 = 32_767;

/// Prova por VOLTAS: o pior instante é o líder cruzando a linha final. `lap_completed`
/// vira 8 e as voltas restantes zeram — era aí que o cabeçalho anunciava a volta 9 numa
/// prova de 8, e ainda perdia o "/8".
#[test]
fn prova_por_voltas_nao_passa_do_total_na_bandeirada() {
    // Última volta em curso: líder completou 7, falta 1.
    assert_eq!(
        contagem_de_voltas(7, true, 8, 1, false, None),
        ContagemVoltas { lap: 8, total: 8 }
    );
    // Líder cruzou a linha final: completou 8, restantes = 0, bandeirada caiu.
    assert_eq!(
        contagem_de_voltas(8, true, 8, 0, true, None),
        ContagemVoltas { lap: 8, total: 8 }
    );
    // O mesmo instante com a bandeirada ainda não refletida no estado da sessão: o teto
    // do total segura o número sozinho.
    assert_eq!(
        contagem_de_voltas(8, true, 8, 0, false, None),
        ContagemVoltas { lap: 8, total: 8 }
    );
    // Cool down com o pelotão ainda rodando: o iRacing continua contando voltas de quem
    // está na pista, e o cabeçalho não pode subir junto.
    assert_eq!(
        contagem_de_voltas(9, true, 8, 0, true, None),
        ContagemVoltas { lap: 8, total: 8 }
    );
}

/// Sem o canal do total, as voltas restantes ainda dão o número — e o teto continua de pé.
#[test]
fn prova_por_voltas_cai_para_as_restantes_sem_o_total() {
    assert_eq!(
        contagem_de_voltas(7, true, 0, 1, false, None),
        ContagemVoltas { lap: 8, total: 8 }
    );
    // Restantes zeradas E sem total declarado: sobra a volta em curso, travada pela
    // bandeirada em vez de virar 9.
    assert_eq!(
        contagem_de_voltas(8, true, 0, 0, true, None),
        ContagemVoltas { lap: 8, total: 0 }
    );
}

/// Prova por TEMPO: os dois canais de volta vêm sentinelados e o total é estimativa.
#[test]
fn prova_por_tempo_estima_o_total_e_para_na_bandeirada() {
    // Meio de prova: 12 completas, cabem mais 8.
    assert_eq!(
        contagem_de_voltas(12, false, ILIMITADO, ILIMITADO, false, Some(8)),
        ContagemVoltas { lap: 13, total: 20 }
    );
    // Reta final: a estimativa colapsa a zero e não pode ficar atrás da volta em curso,
    // senão o "/total" some do cabeçalho.
    assert_eq!(
        contagem_de_voltas(19, false, ILIMITADO, ILIMITADO, false, Some(0)),
        ContagemVoltas { lap: 20, total: 20 }
    );
    // Bandeirada: a volta em curso é a que o líder completou, sem inventar a seguinte.
    assert_eq!(
        contagem_de_voltas(20, false, ILIMITADO, ILIMITADO, true, Some(0)),
        ContagemVoltas { lap: 20, total: 20 }
    );
    // Cool down: a torre entrega aqui o retrato congelado pelo monitor (20), e não o
    // `lap_completed` ao vivo, que já subiu para 21 com o pelotão girando. Sem esse
    // congelamento a prova por tempo não teria teto nenhum, porque o total é estimativa.
    assert_eq!(
        contagem_de_voltas(20, false, ILIMITADO, ILIMITADO, true, None),
        ContagemVoltas { lap: 20, total: 0 }
    );
}

/// Prova por tempo com um total de voltas declarado junto (o caso híbrido dos enduros)
/// continua no regime de tempo: quem manda no fim é o relógio, e o teto vem da bandeirada.
#[test]
fn prova_por_tempo_ignora_o_total_declarado() {
    assert_eq!(
        contagem_de_voltas(12, false, 40, 28, false, Some(8)),
        ContagemVoltas { lap: 13, total: 20 }
    );
}

/// Antes da largada e em bandeirada precoce: nada de "Volta 0".
#[test]
fn contagem_tem_piso_de_uma_volta() {
    assert_eq!(
        contagem_de_voltas(0, true, 8, 8, false, None),
        ContagemVoltas { lap: 1, total: 8 }
    );
    assert_eq!(
        contagem_de_voltas(0, true, 8, 8, true, None),
        ContagemVoltas { lap: 1, total: 8 }
    );
    // Grid vazio: `lap_completed` vem -1 e não pode virar volta 0.
    assert_eq!(
        contagem_de_voltas(-1, true, 0, 0, false, None),
        ContagemVoltas { lap: 1, total: 0 }
    );
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
fn name_key_matches_sdk_username_against_roster_name() {
    // O `UserName` do SDK volta com caixa/espaços próprios; o join por nome não
    // pode depender disso.
    assert_eq!(name_key("  Ana Ribeiro "), name_key("ana ribeiro"));
    assert_ne!(name_key("Ana Ribeiro"), name_key("Ana Ribeira"));
}

/// Recorte com a indentação REAL do dump do iRacing: `Sessions:` é uma lista, então a
/// primeira chave de cada item vem com "- " na frente.
const SESSIONS_YAML_REAL: &str = "\
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionType: Practice
   SessionName: PRACTICE
 - SessionNum: 1
   SessionType: Open Qualify
   SessionName: QUALIFY
 - SessionNum: 2
   SessionType: Race
   SessionName: RACE
";

/// O parser tem de aguentar o traço da lista. Sem isso o `SessionNum` nunca casava, o
/// mapa saía vazio, TODA sessão virava "R" e a torre da classificatória ordenava por
/// progresso na pista em vez de por tempo de volta.
#[test]
fn tipo_de_sessao_sai_do_yaml_com_o_traco_da_lista() {
    let tipos = parse_session_types(SESSIONS_YAML_REAL);

    assert_eq!(tipos.get(&0).map(String::as_str), Some("Practice"));
    assert_eq!(tipos.get(&1).map(String::as_str), Some("Open Qualify"));
    assert_eq!(tipos.get(&2).map(String::as_str), Some("Race"));

    assert_eq!(session_kind(&tipos, 0), "P");
    assert_eq!(session_kind(&tipos, 1), "Q");
    assert_eq!(session_kind(&tipos, 2), "R");
}

const SESSIONS_YAML_COM_TEMPO: &str = "\
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: unlimited
   SessionTime: unlimited
   SessionType: Practice
 - SessionNum: 1
   SessionLaps: unlimited
   SessionTime: 480.0000 sec
   SessionType: Open Qualify
 - SessionNum: 2
   SessionLaps: 25
   SessionTime: unlimited
   SessionType: Race
";

/// A duração declarada no YAML é a reserva do relógio do cabeçalho: quando o canal ao
/// vivo vem sentinelado, é ela que impede a classificatória de cair na contagem de voltas.
/// Sessão `unlimited` fica de fora do mapa — não existe relógio para mostrar ali.
#[test]
fn duracao_da_sessao_sai_do_yaml_e_ignora_ilimitado() {
    let tempos = parse_session_times(SESSIONS_YAML_COM_TEMPO);

    assert_eq!(tempos.get(&1).copied(), Some(480.0));
    assert_eq!(tempos.get(&0), None); // treino sem relógio
    assert_eq!(tempos.get(&2), None); // corrida por voltas
}

/// O elo completo: com a classificatória rolando (`SessionState = Racing`, o mesmo
/// estado da corrida no verde), o critério da torre tem de ser o TEMPO.
#[test]
fn classificatoria_rolando_ordena_por_tempo_e_nao_por_progresso() {
    let tipos = parse_session_types(SESSIONS_YAML_REAL);
    let kind = session_kind(&tipos, 1);

    assert_eq!(modo_da_sessao(kind, 4), OrderMode::Tempo);
}

#[test]
fn modo_segue_o_tipo_e_o_estado_da_sessao() {
    // Treino/quali sempre por tempo, em qualquer estado.
    assert_eq!(modo_da_sessao("Q", 4), OrderMode::Tempo);
    assert_eq!(modo_da_sessao("P", 1), OrderMode::Tempo);
    // Corrida: antes do verde é grid; rolando é progresso; na bandeirada é oficial.
    assert_eq!(modo_da_sessao("R", 1), OrderMode::Grid); // GetInCar
    assert_eq!(modo_da_sessao("R", 3), OrderMode::Grid); // ParadeLaps
    assert_eq!(modo_da_sessao("R", 4), OrderMode::Progresso);
    assert_eq!(modo_da_sessao("R", 5), OrderMode::Oficial); // Checkered
    assert_eq!(modo_da_sessao("R", 6), OrderMode::Oficial); // CoolDown
}

#[test]
fn quali_ordena_pelo_melhor_tempo_e_quem_nao_marcou_fica_no_fim() {
    let cars = vec![
        OrderInput {
            best_lap_ms: 80_500,
            ..carro(7)
        },
        OrderInput {
            best_lap_ms: i64::MAX,
            ..carro(3)
        },
        OrderInput {
            best_lap_ms: 79_900,
            ..carro(12)
        },
    ];

    assert_eq!(ordenar(OrderMode::Tempo, &cars), vec![2, 0, 1]);
}

#[test]
fn corrida_rolando_ordena_por_progresso_real_e_nao_pela_posicao_oficial() {
    // O #7 já ultrapassou o #12 no meio da volta, mas o SDK ainda diz P1 pro #12
    // (a posição oficial só vira na linha). A torre tem de mostrar a ultrapassagem.
    let cars = vec![
        OrderInput {
            class_position: 2,
            lap_completed: 3,
            lap_dist_pct: 0.62,
            ..carro(7)
        },
        OrderInput {
            class_position: 1,
            lap_completed: 3,
            lap_dist_pct: 0.58,
            ..carro(12)
        },
    ];

    assert_eq!(ordenar(OrderMode::Progresso, &cars), vec![0, 1]);
}

#[test]
fn progresso_conta_a_volta_inteira_e_manda_lapeado_e_ausente_pro_fim() {
    let cars = vec![
        // Lapeado: uma volta atrás, mesmo estando adiante na fração da volta.
        OrderInput {
            lap_completed: 2,
            lap_dist_pct: 0.95,
            ..carro(5)
        },
        // Líder: acabou de cruzar a linha.
        OrderInput {
            lap_completed: 3,
            lap_dist_pct: 0.02,
            ..carro(1)
        },
        // Fora do mundo (abandonou / nunca apareceu na telemetria).
        carro(9),
    ];

    assert_eq!(ordenar(OrderMode::Progresso, &cars), vec![1, 0, 2]);
}

#[test]
fn grid_ordena_pela_melhor_volta_da_quali() {
    let cars = vec![
        OrderInput {
            qualy_best_ms: 80_400,
            ..carro(7)
        },
        OrderInput {
            qualy_best_ms: 79_800,
            ..carro(12)
        },
        OrderInput {
            qualy_best_ms: i64::MAX,
            ..carro(3)
        },
    ];

    assert_eq!(ordenar(OrderMode::Grid, &cars), vec![1, 0, 2]);
}

#[test]
fn grid_sem_nenhum_tempo_de_quali_cai_na_posicao_oficial() {
    // Corrida sem quali (grade fixa): sem tempo pra ordenar, a posição do SDK é o
    // que existe de melhor — cair no número do carro embaralharia a grade.
    let cars = vec![
        OrderInput {
            class_position: 3,
            ..carro(7)
        },
        OrderInput {
            class_position: 1,
            ..carro(12)
        },
        OrderInput {
            class_position: 2,
            ..carro(3)
        },
    ];

    assert_eq!(ordenar(OrderMode::Grid, &cars), vec![1, 2, 0]);
}

#[test]
fn antes_da_primeira_corrida_a_previa_e_a_expectativa_de_pre_temporada() {
    // Ninguém correu ainda: o campeonato está zerado pra todo mundo e a única
    // hierarquia é a percepção pública da matéria de pré-temporada.
    let sinais = vec![
        na_pre_temporada(120.0),
        na_pre_temporada(480.0),
        na_pre_temporada(300.0),
    ];

    assert_eq!(ordem_pre_sessao(&sinais), vec![3, 1, 2]);
}

#[test]
fn depois_da_primeira_corrida_a_previa_e_a_posicao_no_campeonato() {
    // Com temporada rolando a tabela manda — mesmo que a expectativa dissesse
    // outra coisa (o #0 era o mais cotado e está sem ponto).
    let sinais = vec![
        PreSinal {
            expectativa: 999.0,
            ..no_campeonato(0, 0)
        },
        no_campeonato(50, 2),
        no_campeonato(50, 1),
    ];

    assert_eq!(ordem_pre_sessao(&sinais), vec![3, 1, 2]);
}

#[test]
fn piloto_sem_dono_na_carreira_nao_entra_na_previa() {
    let sinais = vec![PreSinal::default(), na_pre_temporada(10.0)];

    assert_eq!(ordem_pre_sessao(&sinais), vec![i64::MAX, 1]);
}

#[test]
fn quali_sem_nenhum_tempo_usa_a_previa_no_lugar_do_numero_do_carro() {
    // O #3 tem o número mais baixo, mas é o último da prévia: sem isto a torre
    // abria a classificatória numa fila por número de carro.
    let cars = vec![
        OrderInput {
            pre_ordem: 2,
            ..carro(7)
        },
        OrderInput {
            pre_ordem: 3,
            ..carro(3)
        },
        OrderInput {
            pre_ordem: 1,
            ..carro(12)
        },
    ];

    assert_eq!(ordenar(OrderMode::Tempo, &cars), vec![2, 0, 1]);
}

#[test]
fn a_previa_nao_atropela_quem_ja_marcou_tempo() {
    // Marcou volta? O tempo decide. A prévia só desempata quem ainda não marcou —
    // e quem marcou fica sempre à frente de quem não marcou.
    let cars = vec![
        OrderInput {
            best_lap_ms: 80_500,
            pre_ordem: 9,
            ..carro(7)
        },
        OrderInput {
            pre_ordem: 1,
            ..carro(1)
        },
        OrderInput {
            best_lap_ms: 79_900,
            pre_ordem: 10,
            ..carro(12)
        },
    ];

    assert_eq!(ordenar(OrderMode::Tempo, &cars), vec![2, 0, 1]);
}

#[test]
fn oficial_classifica_primeiro_e_desempata_por_numero() {
    let cars = vec![
        carro(12),
        OrderInput {
            class_position: 2,
            ..carro(9)
        },
        OrderInput {
            class_position: 1,
            ..carro(4)
        },
        carro(3),
    ];

    assert_eq!(ordenar(OrderMode::Oficial, &cars), vec![2, 1, 3, 0]);
}

// ── Feed do rádio da equipe: a época e os ids que o cursor do front segue ───────
//
// `get_breakdown_feed` em si não é testável (lê o log vivo do monitor e abre o save), mas a
// montagem do feed é: [`montar_feed`] recebe a época, as linhas do log e os contextos já
// resolvidos, e devolve os cards com a fusão de quebras simultâneas aplicada.
mod radio {
    use super::super::radio::{montar_feed, LinhaDeQuebra};
    use crate::engenheiro::quebra::Contexto;

    /// Uma linha de quebra sem vínculo nenhum com o jogador — é o único caso que a fusão
    /// aceita, e é justamente nele que o `id` nascia sem a época.
    fn sem_vinculo(nome: &str, peca: &str) -> Contexto {
        Contexto {
            nome_completo: nome.to_string(),
            equipe: Some("Kitsune".to_string()),
            assento: 1,
            e_nemesis: false,
            e_rival: false,
            e_companheiro: false,
            lidera_campeonato: false,
            delta_pontos: None,
            peca: peca.to_string(),
            severidade: "dnf".to_string(),
            variante: 0,
            abandonos_ate_aqui: 1,
        }
    }

    fn linha(lap: u32) -> LinhaDeQuebra {
        LinhaDeQuebra {
            lap,
            severity: "dnf".to_string(),
            label: "Motor".to_string(),
        }
    }

    /// CONTRATO DE TIPO, verificado na compilação: `LinhaDeQuebra` é um espelho do que o log do
    /// monitor guarda, e a volta dele é `u32`. Enquanto o espelho declarava `i32`, o produtor
    /// carregava um `as i32` que não servia a consumidor nenhum — a volta não cruza a ponte e só
    /// é lida na igualdade que funde duas quebras do mesmo instante. Esta função deixa de
    /// compilar no dia em que os dois lados divergirem de novo.
    fn _a_volta_do_log_entra_no_espelho_sem_cast(
        o: &crate::iracing_sdk::race_monitor::BreakdownOutcome,
    ) -> LinhaDeQuebra {
        LinhaDeQuebra {
            lap: o.lap,
            severity: o.severity.key().to_string(),
            label: o.label.clone(),
        }
    }

    /// O defeito: a fala FUNDIDA nascia com `id = i + 1`, sem a época do rádio. Depois de um
    /// reinício de tentativa — que zera o log e avança a época — um card fundido renascia com
    /// id 1 enquanto o front já tinha visto ids muito maiores, e o overlay, que só mostra id
    /// novo, ficava mudo pelo resto da sessão.
    #[test]
    fn a_fala_fundida_nasce_com_a_epoca_do_radio() {
        const EPOCA: usize = 1_000;
        let linhas = [linha(7), linha(7)];
        let contextos = [
            sem_vinculo("James Cooper", "engine"),
            sem_vinculo("Owen Turner", "gearbox"),
        ];

        let feed = montar_feed(EPOCA, &linhas, &contextos);

        assert_eq!(feed.len(), 1, "duas quebras na mesma volta viram uma fala");
        assert_eq!(feed[0].id, EPOCA + 1, "o id da fundida ignorou a época");
    }

    /// A época é um piso: todo id do feed tem de estar acima do que o front já viu, fundido
    /// ou não, e a sequência tem de subir.
    #[test]
    fn os_ids_sobem_e_ficam_acima_da_epoca() {
        const EPOCA: usize = 1_000;
        // Duas na volta 7 (fundem) e uma solta na volta 9.
        let linhas = [linha(7), linha(7), linha(9)];
        let contextos = [
            sem_vinculo("James Cooper", "engine"),
            sem_vinculo("Owen Turner", "gearbox"),
            sem_vinculo("Liam Walker", "suspension"),
        ];

        let feed = montar_feed(EPOCA, &linhas, &contextos);

        assert_eq!(feed.len(), 2);
        let ids: Vec<usize> = feed.iter().map(|m| m.id).collect();
        assert!(
            ids.iter().all(|id| *id >= EPOCA),
            "id abaixo da época: {ids:?}"
        );
        assert!(
            ids.windows(2).all(|p| p[1] > p[0]),
            "os ids não sobem: {ids:?}"
        );
        assert_eq!(ids, vec![EPOCA + 1, EPOCA + 2]);
    }

    /// Sem época (primeira tentativa da corrida) o feed continua nos índices do log, que é o
    /// contrato que já existia para as falas soltas.
    #[test]
    fn sem_epoca_os_ids_sao_os_indices_do_log() {
        let linhas = [linha(7), linha(9)];
        let contextos = [
            sem_vinculo("James Cooper", "engine"),
            sem_vinculo("Owen Turner", "gearbox"),
        ];

        let ids: Vec<usize> = montar_feed(0, &linhas, &contextos)
            .iter()
            .map(|m| m.id)
            .collect();

        assert_eq!(ids, vec![0, 1]);
    }
}

// ── Medição do custo de `get_overlay_data` (ignorada por padrão) ─────────
// Roda contra um save REAL. Não é asserção: imprime os tempos para decidir
// onde otimizar o caminho de 500 ms do overlay.
//   LOOP_BENCH_DB=<career.db> LOOP_BENCH_YAML=<session.yaml> \
//     cargo test --manifest-path src-tauri/Cargo.toml bench_overlay -- --ignored --nocapture
#[test]
#[ignore]
fn bench_overlay_custo() {
    use std::time::Instant;

    let db_path = std::env::var("LOOP_BENCH_DB").expect("LOOP_BENCH_DB");
    let yaml_path = std::env::var("LOOP_BENCH_YAML").expect("LOOP_BENCH_YAML");
    let yaml = std::fs::read_to_string(&yaml_path).expect("yaml");
    let db_path = std::path::PathBuf::from(db_path);

    let media = |rot: u32, t: std::time::Duration| t.as_secs_f64() * 1000.0 / rot as f64;

    // 1) Abrir o banco (inclui `migrations::run_pending`).
    let rot = 20;
    let t0 = Instant::now();
    for _ in 0..rot {
        let _db = crate::db::connection::Database::open_existing(&db_path).expect("abrir");
    }
    println!("abrir_banco: {:.2} ms/chamada", media(rot, t0.elapsed()));

    let db = crate::db::connection::Database::open_existing(&db_path).expect("abrir");

    // 2) Piloto do jogador + categoria (contrato ativo → equipe).
    let t0 = Instant::now();
    for _ in 0..rot {
        let p = crate::db::queries::drivers::get_player_driver(&db.conn).ok();
        let _cat = p
            .as_ref()
            .and_then(|p| {
                crate::db::queries::contracts::get_active_contract_for_pilot(&db.conn, &p.id)
                    .ok()
                    .flatten()
            })
            .and_then(|c| {
                crate::db::queries::teams::get_team_by_id(&db.conn, &c.equipe_id)
                    .ok()
                    .flatten()
            });
    }
    println!(
        "jogador+categoria: {:.2} ms/chamada",
        media(rot, t0.elapsed())
    );

    // 3) Papéis de rivalidade.
    let t0 = Instant::now();
    for _ in 0..rot {
        let current =
            crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
        let _ = crate::commands::career::select_player_interests(&db.conn, current.as_deref());
    }
    println!("rivalidade: {:.2} ms/chamada", media(rot, t0.elapsed()));

    // 4) `resolve` por carro do grid: piloto + contrato + equipe + percepção.
    let ids: Vec<String> = crate::db::queries::drivers::get_all_drivers(&db.conn)
        .expect("elenco")
        .into_iter()
        .take(30)
        .map(|d| d.id)
        .collect();
    println!("(grid simulado: {} carros)", ids.len());
    let t0 = Instant::now();
    for _ in 0..rot {
        for id in &ids {
            let d = crate::db::queries::drivers::get_driver(&db.conn, id).expect("piloto");
            let _team = crate::db::queries::contracts::get_active_contract_for_pilot(&db.conn, id)
                .ok()
                .flatten()
                .and_then(|c| {
                    crate::db::queries::teams::get_team_by_id(&db.conn, &c.equipe_id)
                        .ok()
                        .flatten()
                });
            let _ = crate::commands::season_preview::perception_score(&d);
        }
    }
    println!(
        "resolve(grid inteiro): {:.2} ms/chamada",
        media(rot, t0.elapsed())
    );

    // 5) Parse do YAML de sessão (28 KB reais).
    let rot_yaml = 100;
    let t0 = Instant::now();
    for _ in 0..rot_yaml {
        let _ = super::formato::parse_session_types(&yaml);
    }
    println!(
        "parse_session_types: {:.2} ms/chamada",
        media(rot_yaml, t0.elapsed())
    );
}

// ── Montagem da torre: pista + carreira → o payload que o front recebe ──────────
//
// `get_overlay_data` em si não é testável (lê o SDK e abre o save), mas as etapas em
// que ela foi quebrada são: [`montar_grade`] transforma o retrato da pista em linhas,
// [`finalizar_classes`] ordena e deriva posição/delta/pontos, e o cabeçalho sai de
// [`voltas_do_cabecalho`]/[`relogio_da_sessao`]. As asserções são sobre o JSON
// serializado de propósito — é ele o contrato com o front, camelCase incluído.
mod montagem {
    use std::collections::{HashMap, HashSet};

    use super::super::ordem::{OrderMode, PreSinal};
    use super::super::torre::{
        finalizar_classes, montar_grade, relogio_da_sessao, voltas_do_cabecalho, CarreiraNaTorre,
        DriverInfo, GradeMontada, IdentidadeNaTorre, QuadroDaPista,
    };
    use crate::iracing_sdk::race_monitor::YamlCarMeta;
    use crate::iracing_sdk::tire_strategy::{CarTireStrategy, Compound, PitStopDetail, TireStint};
    use crate::iracing_sdk::CarSnapshot;

    const CLASSE: i64 = 10;

    /// Os mapas que o [`QuadroDaPista`] aponta, num dono só: cada caso liga apenas o
    /// canal que está exercitando e deixa o resto vazio.
    struct Pista {
        carreira: CarreiraNaTorre,
        class_names: HashMap<i64, String>,
        recorded_best_lap: HashMap<i32, f64>,
        qualy_best_ms: HashMap<i32, i64>,
        grid_by_idx: HashMap<i32, i32>,
        tire_by_idx: HashMap<i32, CarTireStrategy>,
        breakdown_by_idx: HashMap<i32, &'static str>,
        repair_laps_by_idx: HashMap<i32, Vec<u32>>,
        flash_idxs: HashSet<i32>,
        lead_lap_now: i32,
        player_idx: i32,
        focus_idx: i32,
        player_black: bool,
    }

    impl Default for Pista {
        fn default() -> Self {
            Self {
                carreira: CarreiraNaTorre::default(),
                class_names: HashMap::new(),
                recorded_best_lap: HashMap::new(),
                qualy_best_ms: HashMap::new(),
                grid_by_idx: HashMap::new(),
                tire_by_idx: HashMap::new(),
                breakdown_by_idx: HashMap::new(),
                repair_laps_by_idx: HashMap::new(),
                flash_idxs: HashSet::new(),
                lead_lap_now: 1,
                // -1 = ninguém. Zero seria um `CarIdx` de verdade, e todo caso que não
                // fala de jogador ganharia um jogador silencioso no primeiro carro.
                player_idx: -1,
                focus_idx: -1,
                player_black: false,
            }
        }
    }

    impl Pista {
        fn quadro(&self) -> QuadroDaPista<'_> {
            QuadroDaPista {
                carreira: &self.carreira,
                class_names: &self.class_names,
                recorded_best_lap: &self.recorded_best_lap,
                qualy_best_ms: &self.qualy_best_ms,
                grid_by_idx: &self.grid_by_idx,
                tire_by_idx: &self.tire_by_idx,
                breakdown_by_idx: &self.breakdown_by_idx,
                repair_laps_by_idx: &self.repair_laps_by_idx,
                flash_idxs: &self.flash_idxs,
                lead_lap_now: self.lead_lap_now,
                player_idx: self.player_idx,
                focus_idx: self.focus_idx,
                player_black: self.player_black,
            }
        }
    }

    fn meta(idx: i32, car_number: i32, class_id: i64) -> YamlCarMeta {
        YamlCarMeta {
            idx,
            is_ai: true,
            is_pace: false,
            class_id,
            car_number,
        }
    }

    fn na_pista(idx: i32, class_position: i32, best_lap_time: f64) -> CarSnapshot {
        CarSnapshot {
            idx,
            class_position,
            best_lap_time,
            ..CarSnapshot::default()
        }
    }

    fn identidade(nome: &str, equipe: &str, pontos: i32) -> IdentidadeNaTorre {
        IdentidadeNaTorre {
            info: DriverInfo {
                nome: nome.to_string(),
                pontos,
                equipe: equipe.to_string(),
                cor: "#ff0000".to_string(),
                pre: PreSinal::default(),
            },
            rival_role: None,
        }
    }

    /// Roda as duas etapas e devolve o JSON das classes — o que atravessa a ponte.
    fn torre(
        carros: &[(YamlCarMeta, Option<CarSnapshot>)],
        pista: &Pista,
        modo: OrderMode,
    ) -> serde_json::Value {
        let roster: Vec<(&YamlCarMeta, Option<&CarSnapshot>)> =
            carros.iter().map(|(m, c)| (m, c.as_ref())).collect();
        let GradeMontada {
            por_classe,
            rotulos,
            ..
        } = montar_grade(&roster, &pista.quadro());
        serde_json::to_value(finalizar_classes(por_classe, &rotulos, modo, false))
            .expect("as classes serializam")
    }

    #[test]
    fn a_torre_junta_a_identidade_da_carreira_com_o_que_esta_na_pista() {
        let carros = vec![
            (meta(1, 11, CLASSE), Some(na_pista(1, 2, 91.5))),
            (meta(2, 22, CLASSE), Some(na_pista(2, 1, 90.0))),
        ];
        let mut pista = Pista::default();
        pista.class_names.insert(CLASSE, "GT3".to_string());
        pista
            .carreira
            .por_idx
            .insert(1, identidade("Ana Ribeiro", "Equipe A", 40));
        pista
            .carreira
            .por_idx
            .insert(2, identidade("Bruno Sá", "Equipe B", 60));
        // Ana largou em 1º e caiu para 2º; Bruno largou em 3º e subiu para 1º.
        pista.grid_by_idx.insert(1, 1);
        pista.grid_by_idx.insert(2, 3);

        let json = torre(&carros, &pista, OrderMode::Oficial);
        let classe = &json[0];
        assert_eq!(classe["id"], "gt3");
        assert_eq!(classe["label"], "GT3");

        let lider = &classe["cars"][0];
        assert_eq!(lider["name"], "Bruno Sá");
        assert_eq!(lider["team"], "Equipe B");
        assert_eq!(lider["color"], "#ff0000");
        assert_eq!(lider["points"], 60);
        assert_eq!(lider["pos"], 1);
        assert_eq!(lider["delta"], 2);
        assert_eq!(lider["gain"], 25);
        assert_eq!(lider["fastest"], "01:30.000");
        assert_eq!(lider["bestMs"], 90_000);
        // Volta mais rápida da classe: só um carro leva o marcador.
        assert_eq!(lider["fol"], true);

        let segundo = &classe["cars"][1];
        assert_eq!(segundo["name"], "Ana Ribeiro");
        assert_eq!(segundo["pos"], 2);
        assert_eq!(segundo["delta"], -1);
        assert_eq!(segundo["gain"], 18);
        assert_eq!(segundo["fol"], false);
    }

    #[test]
    fn carro_sem_dono_na_carreira_entra_pelo_numero_e_conta_no_funil() {
        // O carro EXISTE na pista: não pode sumir da torre só porque o join do save
        // não achou dono. Entra pelo número, neutro, e sem posição oficial vai pro fim.
        let carros = vec![
            (meta(1, 11, CLASSE), Some(na_pista(1, 1, 90.0))),
            (meta(2, 22, CLASSE), None),
        ];
        let mut pista = Pista::default();
        pista
            .carreira
            .por_idx
            .insert(1, identidade("Ana Ribeiro", "Equipe A", 40));

        let roster: Vec<(&YamlCarMeta, Option<&CarSnapshot>)> =
            carros.iter().map(|(m, c)| (m, c.as_ref())).collect();
        let grade = montar_grade(&roster, &pista.quadro());
        assert_eq!(grade.sem_posicao, 1);

        let json = torre(&carros, &pista, OrderMode::Oficial);
        let orfao = &json[0]["cars"][1];
        assert_eq!(orfao["name"], "#22");
        assert_eq!(orfao["team"], "");
        assert_eq!(orfao["color"], "#7d8590");
        assert_eq!(orfao["points"], 0);
        assert_eq!(orfao["pos"], 2);
        // Sem posição na pista não há delta: o grid não diz nada sobre quem não corre.
        assert_eq!(orfao["delta"], 0);
        assert_eq!(orfao["fastest"], "");
        assert_eq!(orfao["bestMs"], 0);
    }

    #[test]
    fn a_bandeira_preta_e_do_jogador_e_o_destaque_segue_a_camera() {
        let carros = vec![
            (meta(1, 11, CLASSE), Some(na_pista(1, 1, 90.0))),
            (meta(2, 22, CLASSE), Some(na_pista(2, 2, 91.0))),
        ];
        let pista = Pista {
            player_idx: 1,
            // No replay a câmera migra: o destaque acompanha, a black flag não.
            focus_idx: 2,
            player_black: true,
            ..Pista::default()
        };

        let json = torre(&carros, &pista, OrderMode::Oficial);
        assert_eq!(json[0]["cars"][0]["flag"], "black");
        assert_eq!(json[0]["cars"][0]["player"], false);
        assert_eq!(json[0]["cars"][1]["flag"], serde_json::Value::Null);
        assert_eq!(json[0]["cars"][1]["player"], true);
    }

    #[test]
    fn a_quebra_com_dnf_vira_bandeira_preta_para_qualquer_carro() {
        let carros = vec![(meta(1, 11, CLASSE), Some(na_pista(1, 1, 90.0)))];
        let mut pista = Pista::default();
        pista.breakdown_by_idx.insert(1, "dnf");
        assert_eq!(
            torre(&carros, &pista, OrderMode::Oficial)[0]["cars"][0]["flag"],
            "black"
        );

        let mut pista = Pista::default();
        pista.breakdown_by_idx.insert(1, "heavy");
        pista.flash_idxs.insert(1);
        let json = torre(&carros, &pista, OrderMode::Oficial);
        assert_eq!(json[0]["cars"][0]["alert"], "heavy");
        assert_eq!(json[0]["cars"][0]["flash"], true);
        assert_eq!(json[0]["cars"][0]["flag"], serde_json::Value::Null);
    }

    #[test]
    fn a_coluna_de_paradas_diz_por_que_o_piloto_parou() {
        let carros = vec![(meta(1, 11, CLASSE), Some(na_pista(1, 1, 90.0)))];
        // Líder na volta 21 → a última parada ainda está dentro da janela do badge.
        let mut pista = Pista {
            lead_lap_now: 21,
            ..Pista::default()
        };
        pista.tire_by_idx.insert(
            1,
            CarTireStrategy {
                car_idx: 1,
                pilot_name: String::new(),
                team_name: String::new(),
                start_compound: Compound::Dry,
                stints: vec![
                    TireStint {
                        from_lap: 1,
                        compound: Compound::Dry,
                        changed: false,
                        confidence: 1.0,
                    },
                    TireStint {
                        from_lap: 6,
                        compound: Compound::Wet,
                        changed: true,
                        confidence: 1.0,
                    },
                ],
                tire_changes: 2,
                wrong_tire: false,
                stops: vec![
                    PitStopDetail {
                        lap: 5,
                        box_secs: 22.0,
                        tire_change: true,
                        track_wet: false,
                    },
                    PitStopDetail {
                        lap: 12,
                        box_secs: 9.0,
                        tire_change: false,
                        track_wet: false,
                    },
                    PitStopDetail {
                        lap: 15,
                        box_secs: 21.0,
                        tire_change: true,
                        track_wet: true,
                    },
                    PitStopDetail {
                        lap: 20,
                        box_secs: 44.0,
                        tire_change: true,
                        track_wet: true,
                    },
                ],
                summary: String::new(),
            },
        );
        // A parada da volta 20 foi um REPARO de peça: o triângulo manda no ícone, mesmo
        // com troca de pneu junto.
        pista.repair_laps_by_idx.insert(1, vec![20]);

        let json = torre(&carros, &pista, OrderMode::Oficial);
        let carro = &json[0]["cars"][0];
        assert_eq!(
            carro["pitIcons"],
            serde_json::json!(["dry", "dry", "fuel", "wet", "part"])
        );
        assert_eq!(carro["tireHistory"], serde_json::json!(["dry", "wet"]));
        assert_eq!(carro["stops"], 2);
        assert_eq!(carro["pitSecs"], 44);
    }

    #[test]
    fn o_badge_de_tempo_de_box_some_depois_de_duas_voltas() {
        let carros = vec![(meta(1, 11, CLASSE), Some(na_pista(1, 1, 90.0)))];
        let mut pista = Pista {
            lead_lap_now: 13,
            ..Pista::default()
        };
        pista.tire_by_idx.insert(
            1,
            CarTireStrategy {
                car_idx: 1,
                pilot_name: String::new(),
                team_name: String::new(),
                start_compound: Compound::Dry,
                stints: Vec::new(),
                tire_changes: 1,
                wrong_tire: false,
                stops: vec![PitStopDetail {
                    lap: 10,
                    box_secs: 22.0,
                    tire_change: true,
                    track_wet: false,
                }],
                summary: String::new(),
            },
        );
        assert_eq!(
            torre(&carros, &pista, OrderMode::Oficial)[0]["cars"][0]["pitSecs"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn os_blocos_de_classe_saem_em_ordem_de_id() {
        // `por_classe` é um HashMap refeito a cada poll; sem a ordenação por id os
        // BLOCOS trocavam de lugar ~1x/s numa prova multiclasse.
        let carros = vec![
            (meta(1, 11, 30), Some(na_pista(1, 1, 90.0))),
            (meta(2, 22, 20), Some(na_pista(2, 1, 95.0))),
        ];
        let mut pista = Pista::default();
        pista.class_names.insert(30, "GT3".to_string());
        pista.class_names.insert(20, "LMP2".to_string());

        let json = torre(&carros, &pista, OrderMode::Oficial);
        assert_eq!(json[0]["label"], "LMP2");
        assert_eq!(json[1]["label"], "GT3");
        // Cada classe tem a SUA volta mais rápida.
        assert_eq!(json[0]["cars"][0]["fol"], true);
        assert_eq!(json[1]["cars"][0]["fol"], true);
    }

    #[test]
    fn prova_por_tempo_estima_o_total_pela_mediana_do_lider() {
        // Líder girando em 95 s, melhor volta do campo em 90 s, 950 s restantes. Pela
        // mediana cabem 10 voltas; pela melhor volta absoluta a conta dava 11 — é a
        // superestimativa de ritmo que subestimava o TOTAL na torre (bug #6, parte 2).
        let voltas = [95.0, 95.0, 95.0, 95.0, 95.0];
        let contagem = voltas_do_cabecalho(
            10,
            0,
            false,
            true,
            true,
            950.0,
            0,
            32767,
            &voltas,
            Some(90.0),
        );
        assert_eq!(contagem.lap, 11);
        assert_eq!(contagem.total, 20);
    }

    #[test]
    fn sem_voltas_do_lider_a_estimativa_cai_na_melhor_do_campo() {
        let contagem =
            voltas_do_cabecalho(10, 0, false, true, true, 900.0, 0, 32767, &[], Some(90.0));
        assert_eq!(contagem.total, 20);
    }

    #[test]
    fn fora_de_corrida_por_tempo_nao_ha_estimativa() {
        // Classificatória: o cabeçalho mostra o relógio, não um "/total" inventado.
        let voltas = [95.0, 95.0, 95.0];
        let contagem = voltas_do_cabecalho(
            3,
            0,
            false,
            true,
            false,
            300.0,
            0,
            32767,
            &voltas,
            Some(90.0),
        );
        assert_eq!(contagem.total, 0);
    }

    #[test]
    fn prova_por_voltas_ignora_o_ritmo_e_usa_o_total_declarado() {
        let voltas = [95.0, 95.0, 95.0];
        let contagem =
            voltas_do_cabecalho(7, 0, false, false, true, 0.0, 8, 1, &voltas, Some(90.0));
        assert_eq!(contagem.lap, 8);
        assert_eq!(contagem.total, 8);
    }

    #[test]
    fn depois_da_bandeirada_a_contagem_usa_a_volta_congelada() {
        // Na volta de desaceleração o `lap_completed` da grade continua subindo.
        let contagem = voltas_do_cabecalho(9, 8, true, false, true, 0.0, 8, 0, &[], None);
        assert_eq!(contagem.lap, 8);
        assert_eq!(contagem.total, 8);
    }

    #[test]
    fn o_relogio_prefere_o_canal_ao_vivo_e_cai_no_yaml_quando_ele_e_sentinela() {
        let ao_vivo = relogio_da_sessao(480.0, 300.0, Some(900.0));
        assert_eq!(ao_vivo.duration_s, Some(480));
        assert_eq!(ao_vivo.remaining_s, Some(300));
        assert_eq!(ao_vivo.elapsed_s, Some(180));

        // Classificatória: o `SessionTimeTotal` vem sentinelado e só o YAML sabe a
        // duração. Sem a reserva o cabeçalho ficava sem relógio nenhum.
        let do_yaml = relogio_da_sessao(604_800.0, 300.0, Some(480.0));
        assert_eq!(do_yaml.duration_s, Some(480));
        assert_eq!(do_yaml.elapsed_s, Some(180));

        // Sem nenhuma das duas fontes não há relógio — e nada de decorrido inventado.
        let sem_nada = relogio_da_sessao(604_800.0, 604_800.0, None);
        assert_eq!(sem_nada.duration_s, None);
        assert_eq!(sem_nada.remaining_s, None);
        assert_eq!(sem_nada.elapsed_s, None);
    }

    #[test]
    fn restante_zerado_ainda_e_restante() {
        // A sessão acabou de fechar: 0 é valor válido, não ausência.
        let relogio = relogio_da_sessao(480.0, 0.0, None);
        assert_eq!(relogio.remaining_s, Some(0));
        assert_eq!(relogio.elapsed_s, Some(480));
    }
}

// ─── O CANAL DA CLASSIFICAÇÃO: do log do monitor ao card do overlay ──────────
//
// O canal existia pela metade: `quali.rs` decidia a fala e a empilhava no `classificacao_log`,
// e nenhum comando lia esse log — a família inteira aparecia no registro do rádio como
// `decidida` e nunca como `tocada`. Estes dois casos prendem as duas pontas da costura.

mod classificacao {
    use crate::commands::overlay::get_classificacao_feed;
    use crate::engenheiro::classificacao::Fala;
    use crate::iracing_sdk::race_monitor;

    fn fala(chave: &str, texto: &str) -> Fala {
        Fala {
            pecas: vec![chave.to_string()],
            texto: texto.to_string(),
        }
    }

    /// **A fala escrita no log do monitor chega ao comando que a UI consulta.** É a cadeia
    /// inteira do lado do Rust; o resto (comando → hook → card) está em
    /// `src/overlay/useBreakdownFeed.test.js`.
    ///
    /// `#[serial]` porque o monitor é global do processo — dois casos escrevendo no mesmo log
    /// leriam um o retrato do outro.
    #[test]
    #[serial_test::serial]
    fn a_fala_posta_no_log_chega_ao_comando() {
        race_monitor::reset();
        let base = race_monitor::radio_epoch();

        race_monitor::injetar_fala_de_classificacao(fala("cl_despedida_0", "Vamos lá."));
        race_monitor::injetar_fala_de_classificacao(fala("cl_perdeu_0", "Perdeu essa."));

        let feed = get_classificacao_feed().expect("o comando não falha sem sessão");
        race_monitor::reset();

        assert_eq!(feed.len(), 2, "o comando não leu o log da classificação");
        assert_eq!(feed[0].text, "Vamos lá.");
        assert_eq!(feed[0].pecas, vec!["cl_despedida_0".to_string()]);
        assert_eq!(feed[1].text, "Perdeu essa.");
        // Severidade própria: o front escolhe por ela o canal de voz e o tom do card, e nada
        // aqui é quebra.
        assert_eq!(feed[0].severity, "quali");
        // Ids CRESCENTES a partir da época — é deles que o cursor do overlay depende.
        assert_eq!((feed[0].id, feed[1].id), (base, base + 1));
    }

    /// **A época entra no `id`, como nos outros canais.** Sem ela, o log esvaziado por um
    /// reinício de tentativa devolveria ids repetidos e o overlay — que só mostra id inédito —
    /// emudeceria pelo resto da sessão.
    #[test]
    #[serial_test::serial]
    fn o_id_nasce_da_epoca_e_nao_do_indice_cru() {
        use super::super::radio::montar_classificacao;

        let log = [
            fala("cl_despedida_0", "Vamos lá."),
            fala("cl_perdeu_0", "Perdeu essa."),
        ];
        let ids: Vec<usize> = montar_classificacao(41, &log)
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, vec![41, 42]);
    }
}
