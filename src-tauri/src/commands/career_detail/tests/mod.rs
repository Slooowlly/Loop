use std::collections::HashMap;

use super::{
    archived_season_is_title, base_category_of, build_archived_recent_results_for_driver,
    build_benchmark_block, build_career_details, build_career_history_block,
    build_category_timeline, build_current_summary_block, build_driver_career_arc_block,
    build_driver_career_path_block, build_driver_championship_curve, build_driver_form_block,
    build_driver_market_curve, build_driver_rivals_block, build_driver_technical_read_block,
    build_driver_title_entries, build_form_seasons_for_driver, build_grid_rank_block,
    build_qualifying_block, build_teammate_block, career_debut_year_from_archive,
    expected_position_from_grid, fallback_injury_display_name, find_championship_context,
    longest_finish_streak, longest_podium_streak_span, longest_podiumless_streak_span,
    longest_winless_streak_span, resolve_driver_category, season_block, transfer_forces_for_driver,
    worst_career_season, CareerRaceHistoryRow, CareerSeasonArchiveRow, ChampionshipContext,
    HistoricalRaceResult, PosicaoDeHoje,
};
use crate::commands::career_types::{
    DriverCareerDroughtBlock, DriverCareerPeakBlock, DriverCareerReliabilityBlock,
    DriverCareerTeammateBlock,
};
use crate::constants::categories::competitive_division_label;
use crate::models::contract::Contract;
use crate::models::driver::Driver;
use crate::models::enums::{InjuryType, TeamRole};

fn sample_driver() -> Driver {
    let mut driver = Driver::new(
        "P001".to_string(),
        "Piloto Teste".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        22,
        2024,
    );
    driver.stats_carreira.corridas = 5;
    driver.stats_temporada.corridas = 5;
    driver
}

/// A temporada em curso, quando o caso so precisa do ANO dela — a trajetoria
/// mede a estreia contra o calendario, e o numero da temporada acompanha o ano.
fn temporada_em(ano: i32) -> crate::models::season::Season {
    crate::models::season::Season::new("S001".to_string(), ano, ano)
}

/// Tabela de campeonato mínima para os testes de resumo: so a posicao importa
/// quando o caso nao esta olhando os gaps.
fn standings_at(posicao: i32) -> ChampionshipContext {
    ChampionshipContext {
        posicao,
        total: 20,
        gap_lider: 0,
        gap_proximo: None,
    }
}

/// Grid SPEC (rookie): 6 equipes, 12 assentos, TODAS com o mesmo carro. Ninguém pode
/// "esperar" posição de fundo por causa do pacote — o pacote não separa ninguém, então
/// a expectativa honesta é o meio do grid, igual pra todo mundo.
#[test]
fn grid_spec_espera_o_meio_do_grid_pra_todo_mundo() {
    let grid: Vec<(f64, i32)> = vec![(0.0, 2); 6];

    assert_eq!(expected_position_from_grid(0.0, &grid), Some(6));
}

/// Carro claramente melhor → topo; claramente pior → fundo. O rank é por assentos, não
/// por uma tabela de limiares absolutos.
#[test]
fn rank_segue_os_assentos_a_frente() {
    // 3 equipes de 2 assentos: carros 10, 5 e 1.
    let grid = [(10.0, 2), (5.0, 2), (1.0, 2)];

    assert_eq!(expected_position_from_grid(10.0, &grid), Some(1));
    assert_eq!(expected_position_from_grid(5.0, &grid), Some(3));
    assert_eq!(expected_position_from_grid(1.0, &grid), Some(5));
}

/// Assento VAZIO não conta: o grid é o que está na pista, não a capacidade nominal.
#[test]
fn assento_vazio_nao_empurra_a_expectativa() {
    // A líder só tem 1 piloto inscrito → quem vem atrás espera P2, não P3.
    let grid = [(10.0, 1), (5.0, 2)];

    assert_eq!(expected_position_from_grid(5.0, &grid), Some(2));
}

/// Equipe sem nenhum assento ocupado não tem expectativa a dar.
#[test]
fn sem_assento_ocupado_nao_ha_expectativa() {
    let grid = [(10.0, 2), (5.0, 0)];

    assert_eq!(expected_position_from_grid(5.0, &grid), None);
}

fn finish(rodada: i32, position: i32) -> HistoricalRaceResult {
    HistoricalRaceResult {
        rodada,
        position,
        is_dnf: false,
        has_fastest_lap: false,
    }
}

#[test]
#[serial_test::serial]
fn fallback_injury_display_name_uses_the_severity_pool() {
    rust_i18n::set_locale("pt-BR"); // nome de lesão resolve no locale ativo.
    assert_eq!(
        fallback_injury_display_name(&InjuryType::Moderada, "A"),
        "Ombro machucado"
    );
    assert_eq!(
        fallback_injury_display_name(&InjuryType::Moderada, "B"),
        "Pescoço travado"
    );
}

#[test]
#[serial_test::serial]
fn current_summary_uses_avaliacao_instead_of_em_avaliacao() {
    rust_i18n::set_locale("pt-BR"); // veredito assevera prosa PT (ver race_eval).
    let driver = sample_driver();
    let results = vec![finish(1, 12), finish(2, 13)];

    let summary = build_current_summary_block(&driver, &results, None);

    assert_eq!(summary.veredito, "Avaliação");
    assert_eq!(summary.tom, "info");
}

#[test]
#[serial_test::serial]
fn current_summary_names_bad_and_critical_seasons() {
    rust_i18n::set_locale("pt-BR");
    let driver = sample_driver();
    let bad_results = vec![finish(1, 11), finish(2, 12), finish(3, 13)];
    let critical_results = vec![finish(1, 18), finish(2, 19), finish(3, 20)];

    let bad = build_current_summary_block(&driver, &bad_results, Some(&standings_at(16)));
    let critical = build_current_summary_block(&driver, &critical_results, Some(&standings_at(22)));

    assert_eq!(bad.veredito, "Ruim");
    assert_eq!(bad.tom, "danger");
    assert_eq!(critical.veredito, "Crítico");
    assert_eq!(critical.tom, "danger");
}

/// A posicao sozinha nao diz se o campeonato esta perto ou perdido: o resumo tem
/// que carregar a distancia para o lider e a vantagem sobre quem vem atras.
#[test]
#[serial_test::serial]
fn current_summary_carries_championship_gaps() {
    rust_i18n::set_locale("pt-BR");
    let driver = sample_driver();
    let results = vec![finish(1, 3), finish(2, 4), finish(3, 2)];

    let summary = build_current_summary_block(
        &driver,
        &results,
        Some(&ChampionshipContext {
            posicao: 3,
            total: 20,
            gap_lider: 8,
            gap_proximo: Some(2),
        }),
    );

    assert_eq!(summary.posicao_campeonato, Some(3));
    assert_eq!(summary.gap_lider, Some(8));
    assert_eq!(summary.gap_proximo, Some(2));
}

/// Antes da primeira largada a tabela inteira esta zerada e a ordem e desempate
/// alfabetico: "0 do lider" valeria para todo mundo do grid. O resumo cala os
/// gaps, do mesmo jeito que a ficha ja calava a posicao.
#[test]
#[serial_test::serial]
fn current_summary_hides_gaps_before_the_first_start() {
    rust_i18n::set_locale("pt-BR");
    let mut driver = sample_driver();
    driver.stats_temporada.corridas = 0;

    let summary = build_current_summary_block(
        &driver,
        &[],
        Some(&ChampionshipContext {
            posicao: 1,
            total: 20,
            gap_lider: 0,
            gap_proximo: Some(0),
        }),
    );

    assert_eq!(summary.gap_lider, None);
    assert_eq!(summary.gap_proximo, None);
}

/// A leitura tecnica abriu de 4 eixos para 14, em quatro grupos. Agressividade,
/// suavidade e confianca entram como ESTILO: um piloto muito agressivo nao e
/// "elite em agressividade" — seria o julgamento errado com a cara de certo.
#[test]
#[serial_test::serial]
fn the_technical_read_groups_every_axis_and_keeps_style_out_of_the_quality_scale() {
    rust_i18n::set_locale("pt-BR");
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    let mut driver = sample_driver();
    driver.atributos.aggression = 92.0;
    driver.atributos.skill = 90.0;

    let bloco = build_driver_technical_read_block(&conn, &driver, None, None);
    let chaves: Vec<&str> = bloco.itens.iter().map(|item| item.chave.as_str()).collect();
    assert_eq!(chaves.len(), 14);
    assert!(chaves.contains(&"defesa") && chaves.contains(&"chuva") && chaves.contains(&"pneus"));

    let conta = |alvo: &str| bloco.itens.iter().filter(|item| item.grupo == alvo).count();
    assert_eq!(conta("volta_seca"), 3);
    // Mentalidade entra na CORRIDA, que e onde a pressao se manifesta: grupo
    // proprio para um eixo so seria cerimonia.
    assert_eq!(conta("corrida"), 4);
    assert_eq!(conta("condicoes"), 4);
    // Estilo em grupo PROPRIO: misturado aos eixos com nota, o marcador no meio
    // da regua se lia como nota media e o julgamento voltava pela vizinhanca.
    assert_eq!(conta("estilo"), 3);

    let agressividade = bloco
        .itens
        .iter()
        .find(|item| item.chave == "agressividade")
        .expect("eixo de agressividade");
    assert_eq!(agressividade.tom, "neutral");
    // Dois polos, e nao quatro faixas: o eixo E o par, e o `nivel` e so o lado
    // para o qual ele pende — 92 pende para agressivo.
    assert_eq!(agressividade.polo_min.as_deref(), Some("Calculista"));
    assert_eq!(agressividade.polo_max.as_deref(), Some("Agressivo"));
    assert_eq!(agressividade.nivel, "Agressivo");

    // Confianca e o terceiro do trio, apesar do peso positivo que o motor lhe da
    // no fim da prova: "Fraco em confianca" seria defeito onde so ha um jeito de
    // correr, e e a mesma leitura que o roster de IA do iRacing usa (optimism).
    let confianca = bloco
        .itens
        .iter()
        .find(|item| item.chave == "confianca")
        .expect("eixo de confianca");
    assert_eq!(confianca.grupo, "estilo");
    assert_eq!(confianca.polo_min.as_deref(), Some("Cauteloso"));

    let ritmo = bloco
        .itens
        .iter()
        .find(|item| item.chave == "ritmo")
        .expect("eixo de ritmo");
    assert_eq!(ritmo.nivel, "Elite");
    assert_eq!(ritmo.label, "Ritmo");
    // Eixo de qualidade nao tem polos nomeados: a cor ja diz para que lado e bom.
    assert!(ritmo.polo_min.is_none() && ritmo.polo_max.is_none());
    // Sem categoria nao ha grid para comparar, e sem temporada arquivada nao ha
    // de quando comparar: os dois ficam calados em vez de chutar um zero.
    assert_eq!(ritmo.referencia, None);
    assert_eq!(ritmo.delta, None);
}

/// A regua de 0–100 abriu duas perguntas que a leitura nao respondia: "Instavel"
/// contra QUEM, e desde QUANDO. A mediana do grid e o snapshot do ano passado sao
/// as duas ancoras — sem elas, 45 de ritmo na F4 e 45 na GT3 desenham a mesma
/// barra e descrevem pilotos que nao se parecem.
#[test]
#[serial_test::serial]
fn the_technical_read_anchors_each_axis_in_the_grid_and_in_last_season() {
    rust_i18n::set_locale("pt-BR");
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    // Grid de cinco pilotos COM ASSENTO: skills 20/30/40/50/60.
    for (indice, skill) in [20.0, 30.0, 40.0, 50.0, 60.0].into_iter().enumerate() {
        let mut rival = Driver::new(
            format!("R{indice:03}"),
            format!("Rival {indice}"),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2024,
        );
        rival.categoria_atual = Some("gt3".to_string());
        rival.atributos.skill = skill;
        crate::db::queries::drivers::insert_driver(&conn, &rival).expect("rival");
    }

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    driver.atributos.skill = 70.0;
    driver.atributos.racecraft = 55.0;
    crate::db::queries::drivers::insert_driver(&conn, &driver).expect("driver");

    // Um agente livre da MESMA categoria, sem assento nenhum. Ele nao corre
    // contra ninguem, entao nao pode entrar na mediana — e com skill 0 ele a
    // derrubaria de forma visivel se entrasse.
    let mut sem_assento = Driver::new(
        "R900".to_string(),
        "Sem assento".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2024,
    );
    sem_assento.categoria_atual = Some("gt3".to_string());
    sem_assento.atributos.skill = 0.0;
    crate::db::queries::drivers::insert_driver(&conn, &sem_assento).expect("agente livre");

    // Tres equipes de gt3 ocupando os seis assentos do grid.
    let equipe_do_piloto = gt3_team("T001", &[driver.id.as_str(), "R000"]);
    for (id, assentos) in [
        ("T001", [driver.id.as_str(), "R000"]),
        ("T002", ["R001", "R002"]),
        ("T003", ["R003", "R004"]),
    ] {
        crate::db::queries::teams::insert_team(&conn, &gt3_team(id, &assentos)).expect("equipe");
    }

    let mut antes = driver.atributos.clone();
    antes.skill = 64.0;
    conn.execute(
        "INSERT INTO driver_season_archive
         (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
         VALUES (?1, 4, 2024, 'Piloto Teste', 'gt3', 3, 120, ?2)",
        rusqlite::params![
            &driver.id,
            serde_json::json!({ "atributos": antes }).to_string()
        ],
    )
    .expect("arquivo da temporada");

    let bloco =
        build_driver_technical_read_block(&conn, &driver, Some("gt3"), Some(&equipe_do_piloto));
    let eixo = |chave: &str| {
        bloco
            .itens
            .iter()
            .find(|item| item.chave == chave)
            .expect("eixo")
            .clone()
    };

    let ritmo = eixo("ritmo");
    assert_eq!(ritmo.escala, 70);
    // O piloto entra na propria mediana: seis ASSENTOS (20/30/40/50/60/70) dao
    // mediana 50 pela regra do indice do meio. O agente livre de skill 0 fica de
    // fora — com ele a mediana cairia para 40, e e essa a diferenca entre "o
    // grid" e "todo mundo com ficha na categoria".
    assert_eq!(ritmo.referencia, Some(50));
    assert_eq!(ritmo.delta, Some(6));

    // Eixo que nao mudou nao ganha um "+0" ocupando a linha — sao dez por coluna,
    // e eles escondem os dois que de fato andaram.
    assert_eq!(eixo("racecraft").delta, None);
}

/// O CONFRONTO DIRETO e o que a aba Rivais nao tinha: ela mostrava os dois eixos
/// do motor de rivalidade e mais nada, tres numeros numa escala que o jogador nao
/// ve. Corrida em que os dois largaram juntos esta em `race_results`, e dela sai o
/// unico placar que se le sem legenda.
///
/// ABANDONO E ENCONTRO, MAS NAO E DUELO: um motor quebrado nao diz quem e mais
/// rapido, e conta-lo como derrota transformaria azar em hierarquia. Por isso
/// vitorias + derrotas fica ABAIXO do total de confrontos aqui.
#[test]
fn the_rivals_block_counts_the_head_to_head_and_leaves_retirements_out_of_the_score() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &driver).expect("piloto");

    let mut rival = Driver::new(
        "R777".to_string(),
        "Tiago Sousa".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        26,
        2024,
    );
    rival.categoria_atual = Some("gt3".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &rival).expect("rival");

    // Percebida = 0.4*historico + 0.6*recente = 16 + 36 = 52 → faixa "clara".
    conn.execute(
        "INSERT INTO rivalries (id, piloto1_id, piloto2_id, historical_intensity, recent_activity, tipo)
         VALUES ('RV1', ?1, ?2, 40.0, 60.0, 'Colisao')",
        rusqlite::params![&driver.id, &rival.id],
    )
    .expect("rivalidade");

    // Equipes separadas: `race_results.equipe_id` tem chave estrangeira, e a do
    // rival e o que da cor ao card na tela.
    let equipe_do_rival = gt3_team("T002", &[rival.id.as_str()]);
    for equipe in [
        gt3_team("T001", &[driver.id.as_str()]),
        equipe_do_rival.clone(),
    ] {
        crate::db::queries::teams::insert_team(&conn, &equipe).expect("equipe");
    }

    conn.execute(
        "INSERT INTO seasons (id, numero, ano) VALUES ('S3', 3, 2024)",
        [],
    )
    .expect("temporada");

    // Tres encontros: ele ganha um, perde um, e no terceiro abandona. NA
    // CLASSIFICACAO ele larga atras nas tres — o mesmo par de pilotos com dois
    // placares opostos, que e exatamente o que o sabado existe para mostrar.
    let corridas = [
        (1, "Monza", 2, 7, 0, 0, 8, 3),
        (2, "Spa", 9, 4, 0, 0, 11, 5),
        (3, "Interlagos", 15, 6, 1, 0, 14, 2),
    ];
    for (rodada, pista, minha, dele, meu_dnf, dnf_dele, minha_largada, largada_dele) in corridas {
        let race_id = format!("C{rodada}");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, season_id, rodada, pista, categoria)
             VALUES (?1, 'S3', 'S3', ?2, ?3, 'gt3')",
            rusqlite::params![&race_id, rodada, pista],
        )
        .expect("corrida");
        for (piloto, equipe, posicao, dnf, largada) in [
            (&driver.id, "T001", minha, meu_dnf, minha_largada),
            (&rival.id, "T002", dele, dnf_dele, largada_dele),
        ] {
            conn.execute(
                "INSERT INTO race_results
                    (race_id, piloto_id, equipe_id, posicao_final, dnf, posicao_largada)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![&race_id, piloto, equipe, posicao, dnf, largada],
            )
            .expect("resultado");
        }
    }

    let bloco = build_driver_rivals_block(&conn, &driver).expect("bloco de rivais");
    let item = bloco.itens.first().expect("um rival");

    assert_eq!(item.nome, "Tiago Sousa");
    assert_eq!(item.nivel_chave, "clara");
    assert_eq!(item.confrontos, 3);
    assert_eq!(item.vitorias, 1);
    assert_eq!(item.derrotas, 1);
    // O terceiro encontro nao decidiu nada, e por isso 1 + 1 < 3.
    assert!(item.vitorias + item.derrotas < item.confrontos);

    // O SABADO nao e o domingo: no grid ele perdeu as tres, inclusive a que
    // abandonou na corrida. A classificacao ja aconteceu quando o motor quebra,
    // entao o abandono nao a apaga como apaga o duelo de corrida.
    assert_eq!(item.vitorias_quali, 0);
    assert_eq!(item.derrotas_quali, 3);

    // A ORDEM CRONOLOGICA e o que deixa a tela desenhar a faixa do confronto: um
    // placar diz quanto, so a sequencia diz quando a mare virou.
    let sequencia: Vec<&str> = item
        .encontros
        .iter()
        .map(|encontro| encontro.vencedor.as_str())
        .collect();
    assert_eq!(sequencia, ["piloto", "rival", "nenhum"]);

    let ultimo = item.encontros.last().expect("ultimo encontro");
    assert_eq!(ultimo.pista, "Interlagos");
    assert_eq!(ultimo.rodada, 3);
    // O ANO, e nao so o numero da temporada: "T3" nao e uma data que alguem
    // reconheca, e e o ano que a faixa escreve embaixo de cada bloco.
    assert_eq!(ultimo.ano, 2024);
    // Abandono nao decide o dia: a marca fica neutra em vez de virar derrota.
    assert_eq!(ultimo.vencedor, "nenhum");
    // Mesma categoria: os dois ainda dividem grid, entao a ficha nao anuncia
    // separacao.
    assert!(item.mesma_categoria);
    // A equipe do RIVAL, e nao a do dono da ficha: e dela que sai a cor que tira
    // a aba do cinza uniforme.
    assert_eq!(
        item.equipe_nome.as_deref(),
        Some(equipe_do_rival.nome.as_str())
    );
}

/// O recorte que o placar geral esconde: a DISTANCIA. Num grid fechado todo
/// mundo divide o mesmo numero de corridas e o placar sozinho nao separa tres
/// rivais; o tempo medio entre os dois separa.
#[test]
fn the_rivals_block_measures_the_distance_between_the_two_not_only_the_count() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &driver).expect("piloto");

    let mut rival = Driver::new(
        "R779".to_string(),
        "Rival Territorial".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        28,
        2024,
    );
    rival.categoria_atual = Some("gt3".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &rival).expect("rival");

    conn.execute(
        "INSERT INTO rivalries (id, piloto1_id, piloto2_id, historical_intensity, recent_activity, tipo)
         VALUES ('RV3', ?1, ?2, 40.0, 60.0, 'Pista')",
        rusqlite::params![&driver.id, &rival.id],
    )
    .expect("rivalidade");
    for equipe in [
        gt3_team("T001", &[driver.id.as_str()]),
        gt3_team("T002", &[rival.id.as_str()]),
    ] {
        crate::db::queries::teams::insert_team(&conn, &equipe).expect("equipe");
    }
    conn.execute(
        "INSERT INTO seasons (id, numero, ano) VALUES ('S3', 3, 2024)",
        [],
    )
    .expect("temporada");

    // Seis corridas. Diferenca de tempo para o rival, em segundos: −2, −2, +8,
    // +4, +4, +4 (negativo = ele na frente). Media = 16/6 = +2.667s: ele ganha
    // duas de perto e perde quatro de longe.
    let corridas = [
        // (rodada, pista, minha_pos, pos_dele, meu_gap, gap_dele)
        (1, "Spa", 1, 4, 0.0, 2000.0),
        (2, "Spa", 2, 5, 1000.0, 3000.0),
        (3, "Spa", 9, 2, 12000.0, 4000.0),
        (4, "Interlagos", 8, 3, 9000.0, 5000.0),
        (5, "Interlagos", 11, 6, 10000.0, 6000.0),
        (6, "Interlagos", 12, 7, 11000.0, 7000.0),
    ];
    for (rodada, pista, minha, dele, meu_gap, gap_dele) in corridas {
        let race_id = format!("C{rodada}");
        conn.execute(
            "INSERT INTO calendar (id, temporada_id, season_id, rodada, pista, categoria)
             VALUES (?1, 'S3', 'S3', ?2, ?3, 'gt3')",
            rusqlite::params![&race_id, rodada, pista],
        )
        .expect("corrida");
        for (piloto, equipe, posicao, gap) in [
            (&driver.id, "T001", minha, meu_gap),
            (&rival.id, "T002", dele, gap_dele),
        ] {
            conn.execute(
                "INSERT INTO race_results
                    (race_id, piloto_id, equipe_id, posicao_final, posicao_largada, gap_to_winner_ms)
                 VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
                rusqlite::params![&race_id, piloto, equipe, posicao, gap],
            )
            .expect("resultado");
        }
    }

    let bloco = build_driver_rivals_block(&conn, &driver).expect("bloco de rivais");
    let item = bloco.itens.first().expect("um rival");

    assert_eq!(item.vitorias, 2);
    assert_eq!(item.derrotas, 4);

    // A DISTANCIA, que o placar nao carrega: media positiva = ele termina atras.
    // Um 2–4 de meio segundo e um 2–4 de meia volta desenham o mesmo placar e
    // descrevem duas relacoes diferentes.
    let gap = item.gap_medio.expect("gap medio");
    assert!((gap - 2.667).abs() < 0.01, "gap medio inesperado: {gap}");

    // Corrida em que os dois marcam gap zero e save antigo sem o campo gravado,
    // e nao empate perfeito: ela ficaria puxando a media para o zero.
    assert_eq!(item.confrontos, 6);

    // O gap tambem vai POR CORRIDA, e nao so na media: e dele que sai a altura
    // de cada barra da faixa. Duas vitorias apertadas e quatro derrotas largas
    // desenham uma forma; seis empates tecnicos desenhariam outra, e a media
    // sozinha nao separa as duas.
    let gaps: Vec<Option<f64>> = item.encontros.iter().map(|e| e.gap).collect();
    assert_eq!(gaps.len(), 6);
    assert!((gaps[0].expect("gap da primeira") + 2.0).abs() < 0.001);
    assert!((gaps[2].expect("gap da terceira") - 8.0).abs() < 0.001);

    // Nunca dividiram box: a comparacao sem o carro no meio nao existe aqui.
    assert!(item.companheirismo.is_none());
}

/// Rival sem uma unica corrida em comum continua sendo rival: a rivalidade pode ter
/// nascido de mercado ou de box e sobreviver a uma promocao. O bloco devolve o
/// placar zerado e nenhum encontro, em vez de sumir com a pessoa.
#[test]
fn a_rival_they_never_raced_against_still_shows_up_with_an_empty_score() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt4".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &driver).expect("piloto");

    let mut rival = Driver::new(
        "R778".to_string(),
        "Eli Green".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        30,
        2024,
    );
    rival.categoria_atual = Some("gt3".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &rival).expect("rival");

    conn.execute(
        "INSERT INTO rivalries (id, piloto1_id, piloto2_id, historical_intensity, recent_activity, tipo)
         VALUES ('RV2', ?1, ?2, 10.0, 5.0, 'Companheiros')",
        rusqlite::params![&driver.id, &rival.id],
    )
    .expect("rivalidade");

    let bloco = build_driver_rivals_block(&conn, &driver).expect("bloco de rivais");
    let item = bloco.itens.first().expect("um rival");

    assert_eq!(item.confrontos, 0);
    assert!(item.encontros.is_empty());
    assert_eq!(item.nivel_chave, "atrito_leve");
    // Subiu de categoria: a tela avisa que os dois nao se cruzam mais.
    assert!(!item.mesma_categoria);
    assert_eq!(item.categoria_atual.as_deref(), Some("GT3"));
}

/// Grid de dois nao tem mediana: "a mediana" ali e so o outro piloto com nome de
/// estatistica, e um traço no meio da regua diria que ha um grid por tras.
#[test]
#[serial_test::serial]
fn a_grid_too_small_to_have_a_middle_says_nothing() {
    rust_i18n::set_locale("pt-BR");
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &driver).expect("driver");

    // Um assento ocupado no grid inteiro: nao ha meio para achar.
    let equipe = gt3_team("T001", &[driver.id.as_str()]);
    crate::db::queries::teams::insert_team(&conn, &equipe).expect("equipe");

    let bloco = build_driver_technical_read_block(&conn, &driver, Some("gt3"), Some(&equipe));
    assert!(bloco.itens.iter().all(|item| item.referencia.is_none()));
}

/// Jovem com folga larga ate o teto esta EM ASCENSAO; o mesmo piloto aos 34 nao
/// esta, ainda que a folga fosse igual. As faixas de idade sao as mesmas em que a
/// simulacao muda de comportamento.
#[test]
#[serial_test::serial]
fn the_career_arc_reads_age_against_the_room_left_to_the_ceiling() {
    rust_i18n::set_locale("pt-BR");
    let mut driver = sample_driver();
    driver.idade = 22;
    driver.atributos.skill = 60.0;
    driver.atributos.potencial = 88.0;

    let arco = build_driver_career_arc_block(&driver);
    assert_eq!(arco.fase, "Em ascensão");
    assert_eq!(arco.nivel_margem.as_deref(), Some("Larga"));

    driver.idade = 34;
    let veterano = build_driver_career_arc_block(&driver);
    assert_eq!(veterano.fase, "No platô");
}

/// Potencial 0.0 e teto NAO DERIVADO (jogador e saves antigos), e nao teto no
/// chao: a ficha nao pode anunciar "no teto" para quem nunca teve um teto medido.
#[test]
#[serial_test::serial]
fn an_underived_ceiling_stays_quiet_instead_of_reading_as_maxed_out() {
    rust_i18n::set_locale("pt-BR");
    let mut driver = sample_driver();
    driver.idade = 24;
    driver.atributos.skill = 70.0;
    driver.atributos.potencial = 0.0;

    let arco = build_driver_career_arc_block(&driver);
    assert_eq!(arco.nivel_margem, None);
    // Sem folga medida a fase cai na idade, e nao em "ascensao" por suposicao.
    assert_eq!(arco.fase, "No auge");
}

/// Numa categoria multiclasse a chave de divisao ("endurance:lmp2") nunca bate
/// com o `categoria_atual` do piloto, que guarda so "endurance". A consulta
/// voltava vazia e o piloto sumia da propria classificacao — sem posicao, sem
/// gap e sem delta contra o esperado. Aqui o LMP2 tem que se ver como LMP2, e o
/// maior pontuador do GT4 nao pode entrar na conta dele.
#[test]
fn multiclass_standings_are_scored_inside_the_class() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    for (id, nome, pontos) in [
        ("P001", "Ponta do LMP2", 40.0),
        ("P002", "Segundo do LMP2", 10.0),
        ("P003", "Ponta do GT4", 90.0),
        ("P004", "Segundo do GT4", 80.0),
    ] {
        let mut driver = Driver::new(
            id.to_string(),
            nome.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2024,
        );
        driver.categoria_atual = Some("endurance".to_string());
        driver.stats_temporada.corridas = 3;
        driver.stats_temporada.pontos = pontos;
        crate::db::queries::drivers::insert_driver(&conn, &driver).expect("driver");
    }

    for (id, classe, assentos) in [
        ("T001", "lmp2", ["P001", "P002"]),
        ("T002", "gt4", ["P003", "P004"]),
    ] {
        let mut team = endurance_team(id, classe);
        team.piloto_1_id = Some(assentos[0].to_string());
        team.piloto_2_id = Some(assentos[1].to_string());
        crate::db::queries::teams::insert_team(&conn, &team).expect("team");
    }

    let lider = find_championship_context(&conn, "endurance:lmp2", "P001")
        .expect("classificacao")
        .expect("piloto na tabela");
    assert_eq!(lider.posicao, 1);
    assert_eq!(lider.gap_lider, 0);
    assert_eq!(lider.gap_proximo, Some(30));

    let segundo = find_championship_context(&conn, "endurance:lmp2", "P002")
        .expect("classificacao")
        .expect("piloto na tabela");
    assert_eq!(segundo.posicao, 2);
    assert_eq!(segundo.gap_lider, 30);
    // Lanterna da CLASSE, e nao do grid: os dois do GT4 nao entram atras dele.
    assert_eq!(segundo.gap_proximo, None);
}

/// O calendario e os arquivos de resultado sao da categoria inteira: consultados
/// pela chave com classe nao devolviam nada, e a "temporada atual" do piloto
/// multiclasse caia calada no arquivo da temporada passada.
#[test]
fn division_key_reduces_to_the_calendar_category() {
    assert_eq!(base_category_of("endurance:lmp2"), "endurance");
    assert_eq!(
        base_category_of("production_challenger:bmw"),
        "production_challenger"
    );
    assert_eq!(base_category_of("gt3"), "gt3");
}

/// Equipe de gt3 com os assentos dados preenchidos — ocupar assento e o que faz
/// um piloto ser "do grid" em vez de so ter a categoria na ficha.
fn gt3_team(id: &str, assentos: &[&str]) -> crate::models::team::Team {
    use rand::SeedableRng;

    let template =
        crate::constants::teams::get_reference_team_template("gt3", None).expect("template gt3");
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let mut team = crate::models::team::Team::from_template_with_rng(
        template,
        "gt3",
        id.to_string(),
        2026,
        &mut rng,
    );
    team.piloto_1_id = assentos.first().map(|id| id.to_string());
    team.piloto_2_id = assentos.get(1).map(|id| id.to_string());
    team
}

/// O recorte "grid atual" da ficha conta so quem divide o grid com ele: mesma
/// categoria E, em categoria multiclasse, mesma classe. Somar as duas classes
/// daria um denominador que nao existe em pista nenhuma — o piloto de LMP2 nao
/// corre contra o GT4, entao nao pode ser "3º de 4" contra ele.
#[test]
fn grid_rank_block_conta_so_a_classe_do_piloto() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut pilotos = HashMap::new();
    for (id, nome, vitorias) in [
        ("P001", "Segundo do LMP2", 5u32),
        ("P002", "Ponta do LMP2", 9),
        ("P003", "Ponta do GT4", 40),
        ("P004", "Segundo do GT4", 30),
    ] {
        let mut driver = Driver::new(
            id.to_string(),
            nome.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2024,
        );
        driver.categoria_atual = Some("endurance".to_string());
        driver.stats_carreira.vitorias = vitorias;
        crate::db::queries::drivers::insert_driver(&conn, &driver).expect("driver");
        pilotos.insert(id, driver);
    }

    let mut lmp2 = endurance_team("T001", "lmp2");
    lmp2.piloto_1_id = Some("P001".to_string());
    lmp2.piloto_2_id = Some("P002".to_string());
    crate::db::queries::teams::insert_team(&conn, &lmp2).expect("team");

    let mut gt4 = endurance_team("T002", "gt4");
    gt4.piloto_1_id = Some("P003".to_string());
    gt4.piloto_2_id = Some("P004".to_string());
    crate::db::queries::teams::insert_team(&conn, &gt4).expect("team");

    let bloco = build_grid_rank_block(&conn, &pilotos["P001"], Some("endurance:lmp2"), Some(&lmp2))
        .expect("bloco do grid");

    // Dois assentos na classe, e nao os quatro do endurance inteiro.
    assert_eq!(bloco.total, Some(2));
    // Os 40 e 30 do GT4 nao empurram o LMP2 para tras: aqui ele e 2º de 2.
    assert_eq!(bloco.vitorias, Some(2));
    assert_eq!(
        build_grid_rank_block(&conn, &pilotos["P002"], Some("endurance:lmp2"), Some(&lmp2))
            .expect("bloco do grid")
            .vitorias,
        Some(1)
    );

    // Sem categoria nao ha grid — a ficha esconde o seletor em vez de comparar
    // com pares que nao sao dele.
    assert!(build_grid_rank_block(&conn, &pilotos["P001"], None, Some(&lmp2)).is_none());
    // E quem nao esta no grid resultante tambem nao tem recorte: o piloto do
    // GT4 medido contra a equipe de LMP2 nao aparece na propria lista.
    assert!(
        build_grid_rank_block(&conn, &pilotos["P003"], Some("endurance:lmp2"), Some(&lmp2))
            .is_none()
    );
}

fn endurance_team(id: &str, classe: &str) -> crate::models::team::Team {
    use rand::SeedableRng;

    let template = crate::constants::teams::get_reference_team_template("endurance", Some(classe))
        .expect("template de equipe");
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let mut team = crate::models::team::Team::from_template_with_rng(
        template,
        "endurance",
        id.to_string(),
        2026,
        &mut rng,
    );
    team.classe = Some(classe.to_string());
    team
}

#[test]
fn archived_recent_results_marks_previous_season_without_team() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            INSERT INTO driver_season_archive (
                piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json
            ) VALUES (
                'P001', 25, 2024, 'Piloto Teste', '', NULL, 0.0,
                '{\"corridas\":0,\"categoria\":\"\",\"ultimos_resultados\":[]}'
            );
            ",
        )
        .expect("archive setup");

    let archived =
        build_archived_recent_results_for_driver(&conn, 26, "P001").expect("archive results");

    assert!(archived.results.is_empty());
    assert_eq!(
        archived.form_context.as_deref(),
        Some("sem_time_temporada_passada")
    );
}

#[test]
fn driver_form_block_exposes_previous_season_without_team_context() {
    let form = build_driver_form_block(&[], Some("sem_time_temporada_passada"), Vec::new());

    assert_eq!(form.momento, "sem_dados");
    assert_eq!(form.contexto.as_deref(), Some("sem_time_temporada_passada"));
    assert!(form.temporadas.is_empty());
}

#[test]
fn form_seasons_split_previous_and_current_calendar() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE seasons (id TEXT PRIMARY KEY, numero INTEGER, ano INTEGER);
         CREATE TABLE calendar (id TEXT PRIMARY KEY, season_id TEXT, temporada_id TEXT, rodada INTEGER, pista TEXT, track_name TEXT, categoria TEXT, data TEXT);
         CREATE TABLE race_results (id INTEGER PRIMARY KEY, race_id TEXT, piloto_id TEXT, posicao_final INTEGER, dnf INTEGER, posicao_largada INTEGER DEFAULT 0, fastest_lap INTEGER DEFAULT 0);
         INSERT INTO seasons (id, numero, ano) VALUES ('S4', 4, 2024), ('S5', 5, 2025), ('S6', 6, 2026);
         INSERT INTO calendar (id, season_id, temporada_id, rodada) VALUES
            ('C40', 'S4', NULL, 1),
            ('C50', 'S5', NULL, 1), ('C51', 'S5', NULL, 2), ('C52', 'S5', NULL, 3),
            ('C60', 'S6', NULL, 1);
         INSERT INTO race_results (race_id, piloto_id, posicao_final, dnf) VALUES
            ('C40', 'D1', 9, 0),
            ('C50', 'D1', 4, 0), ('C51', 'D1', 18, 1), ('C52', 'D1', 2, 0),
            ('C60', 'D1', 1, 0);",
    )
    .expect("schema");

    let seasons = build_form_seasons_for_driver(&conn, "D1", 6).expect("forma por temporada");

    // A retrasada (2024) fica de fora: a faixa é do calendário anterior e do atual.
    assert_eq!(seasons.len(), 2);
    assert_eq!(seasons[0].ano, 2025);
    assert!(!seasons[0].atual);
    // A temporada fechada vem INTEIRA, e não recortada nas últimas N corridas.
    assert_eq!(seasons[0].resultados.len(), 3);
    assert_eq!(seasons[0].resultados[0].chegada, Some(4));
    // DNF não carrega posição de chegada.
    assert!(seasons[0].resultados[1].dnf);
    assert_eq!(seasons[0].resultados[1].chegada, None);
    assert_eq!(seasons[1].ano, 2026);
    assert!(seasons[1].atual);
    assert_eq!(seasons[1].resultados.len(), 1);
}

#[test]
fn career_history_block_derives_presence_marks_peak_and_mobility() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL,
                pista TEXT,
                track_name TEXT,
                data TEXT
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0,
                posicao_largada INTEGER NOT NULL DEFAULT 0,
                fastest_lap INTEGER NOT NULL DEFAULT 0
            );

            INSERT INTO seasons (id, numero, ano) VALUES
                ('S001', 1, 2020),
                ('S002', 2, 2021),
                ('S003', 3, 2022),
                ('S004', 4, 2023),
                ('S005', 5, 2024);

            INSERT INTO driver_season_archive
                (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
            VALUES
                ('P001', 1, 2020, 'Piloto Teste', 'mazda_rookie', 4, 50.0,
                 '{\"corridas\":5,\"vitorias\":0,\"podios\":1,\"pontos\":50,\"categoria\":\"mazda_rookie\"}'),
                ('P001', 2, 2021, 'Piloto Teste', 'mazda_amador', 2, 180.0,
                 '{\"corridas\":8,\"vitorias\":3,\"podios\":5,\"pontos\":180,\"categoria\":\"mazda_amador\"}'),
                ('P001', 3, 2022, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 4, 2023, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 5, 2024, 'Piloto Teste', 'gt4', 5, 90.0,
                 '{\"corridas\":10,\"vitorias\":1,\"podios\":2,\"pontos\":90,\"categoria\":\"gt4\"}'),
                ('P001', 6, 2025, 'Piloto Teste', '', NULL, 0.0,
                 '{\"corridas\":0,\"vitorias\":0,\"podios\":0,\"pontos\":0,\"categoria\":\"\"}'),
                ('P001', 7, 2026, 'Piloto Teste', 'bmw_m2', 1, 220.0,
                 '{\"corridas\":8,\"vitorias\":4,\"podios\":6,\"pontos\":220,\"categoria\":\"bmw_m2\"}');
            ",
        )
        .expect("history schema");

    for (season, races) in [("S001", 5), ("S002", 8), ("S004", 10), ("S005", 8)] {
        for rodada in 1..=races {
            conn.execute(
                "INSERT INTO calendar (id, temporada_id, season_id, rodada, categoria)
                     VALUES (?1, ?2, ?2, ?3, 'mazda_rookie')",
                rusqlite::params![format!("{season}_R{rodada:02}"), season, rodada],
            )
            .expect("calendar");
        }
    }

    for (race_id, team_id, position, dnf) in [
        ("S001_R01", "T1", 5, 0),
        ("S001_R02", "T1", 4, 0),
        ("S001_R03", "T1", 3, 0),
        ("S001_R04", "T1", 12, 1),
        ("S001_R05", "T1", 4, 0),
        ("S002_R01", "T2", 2, 0),
        ("S002_R02", "T2", 1, 0),
        ("S002_R03", "T2", 1, 0),
        ("S002_R04", "T2", 1, 0),
        ("S002_R05", "T2", 4, 0),
        ("S004_R01", "T3", 9, 0),
        ("S005_R01", "T3", 1, 0),
    ] {
        conn.execute(
            "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos)
                 VALUES (?1, 'P001', ?2, ?3, ?4, 0.0)",
            rusqlite::params![race_id, team_id, position, dnf],
        )
        .expect("race result");
    }

    let history = build_career_history_block(&conn, "P001").expect("history block");

    assert_eq!(history.presenca.temporadas_disputadas, 4);
    assert_eq!(history.presenca.tempo_carreira, 7);
    assert_eq!(history.presenca.anos_desempregado, 3);
    assert_eq!(
        history.presenca.periodos_desempregado,
        vec!["2022->2023".to_string(), "2025".to_string()]
    );
    assert_eq!(history.presenca.categorias_disputadas, 4);
    assert_eq!(history.primeiros_marcos.primeiro_podio_corrida, Some(3));
    assert_eq!(history.primeiros_marcos.primeira_vitoria_corrida, Some(7));
    assert_eq!(history.primeiros_marcos.primeiro_dnf_corrida, Some(4));
    assert_eq!(history.auge.maior_sequencia_vitorias, 3);
    // As tres vitorias seguidas sao da temporada 2 (2021), e a data e o que faz
    // a marca dizer alguma coisa: tres seguidas ha oito anos e tres nesta
    // temporada falam de pilotos diferentes.
    assert_eq!(history.auge.sequencia_ano_inicio, Some(2021));
    assert_eq!(history.auge.sequencia_ano_fim, Some(2021));
    assert_eq!(
        history.auge.melhor_temporada.as_ref().map(|item| item.ano),
        Some(2026)
    );
    assert_eq!(
        history
            .auge
            .melhor_temporada
            .as_ref()
            .map(|item| item.categoria.as_str()),
        Some("bmw_m2")
    );
    assert_eq!(history.mobilidade.promocoes, 2);
    assert_eq!(history.mobilidade.rebaixamentos, 1);
    assert_eq!(history.mobilidade.equipes_defendidas, 3);
    assert!((history.mobilidade.tempo_medio_por_equipe.unwrap() - 1.3).abs() < 0.05);
}

#[test]
fn category_timeline_compresses_category_stints_and_returns() {
    let seasons = vec![
        season_archive_row(2017, "mazda_rookie", 5),
        season_archive_row(2018, "mazda_rookie", 5),
        season_archive_row(2022, "mazda_amador", 8),
        season_archive_row(2023, "mazda_amador", 8),
        season_archive_row(2024, "", 0),
        season_archive_row(2025, "mazda_rookie", 5),
    ];

    let timeline = build_category_timeline(&seasons, Some("mazda_rookie"), 2025);

    assert_eq!(timeline.len(), 3);
    assert_eq!(timeline[0].categoria, "mazda_rookie");
    assert_eq!(timeline[0].ano_inicio, 2017);
    assert_eq!(timeline[0].ano_fim, 2018);
    assert_eq!(timeline[1].categoria, "mazda_amador");
    assert_eq!(timeline[1].ano_inicio, 2022);
    assert_eq!(timeline[2].categoria, "mazda_rookie");
    assert_eq!(timeline[2].ano_inicio, 2025);
}

#[test]
fn category_timeline_keeps_archived_special_seasons_as_divisions() {
    let especial = |ano: i32, classe: &str| {
        let mut row = season_archive_row(ano, "endurance", 6);
        row.classe = Some(classe.to_string());
        row
    };
    let seasons = vec![
        season_archive_row(2010, "gt3", 14),
        especial(2011, "gt3"),
        especial(2012, "lmp2"),
        especial(2013, "lmp2"),
    ];

    let timeline = build_category_timeline(&seasons, Some("endurance:lmp2"), 2014);

    // As temporadas do bloco especial eram DESCARTADAS: o piloto que passou a
    // carreira na Endurance ficava com a escada vazia nesses anos, enquanto o
    // card ao lado listava os títulos ganhos justamente neles.
    assert_eq!(timeline.len(), 3);
    assert_eq!(timeline[0].categoria, "gt3");
    // A classe é o que transforma "endurance" na divisão de verdade — e é a
    // mesma chave com que o ano corrente já era desenhado.
    assert_eq!(timeline[1].categoria, "endurance:gt3");
    assert_eq!(timeline[1].ano_inicio, 2011);
    assert_eq!(timeline[2].categoria, "endurance:lmp2");
    assert_eq!(timeline[2].ano_inicio, 2012);
    // O ano em curso continua fechando a última passagem, sem abrir outra.
    assert_eq!(timeline[2].ano_fim, 2014);
}

#[test]
fn category_timeline_falls_back_to_the_base_special_without_class() {
    // Snapshot antigo, gravado antes de a classe entrar no arquivo.
    let seasons = vec![season_archive_row(2011, "endurance", 6)];

    let timeline = build_category_timeline(&seasons, None, 2012);

    // Perde a classe, mas pinta o ano: uma célula genérica diz mais que um buraco.
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].categoria, "endurance");
}

#[test]
fn category_timeline_ignores_current_special_category() {
    let seasons = vec![
        season_archive_row(2022, "mazda_rookie", 5),
        season_archive_row(2023, "", 0),
        season_archive_row(2024, "gt3", 14),
    ];

    let timeline = build_category_timeline(&seasons, Some("endurance"), 2025);

    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].categoria, "mazda_rookie");
    assert_eq!(timeline[1].categoria, "gt3");
    assert!(timeline.iter().all(|item| item.categoria != "endurance"));
}

#[test]
fn career_detail_resolves_endurance_contract_as_gt3_endurance() {
    let mut driver = sample_driver();
    driver.categoria_atual = Some("endurance".to_string());
    let mut contract = Contract::new(
        "C_END_GT3".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        "T_END_GT3".to_string(),
        "GT3 Endurance Team".to_string(),
        1,
        2,
        100_000.0,
        TeamRole::Numero1,
        "endurance".to_string(),
    );
    contract.classe = Some("gt3".to_string());

    let category = resolve_driver_category(&driver, Some(&contract), None);

    assert_eq!(category.as_deref(), Some("endurance:gt3"));
    assert_eq!(
        competitive_division_label(&contract.categoria, contract.classe.as_deref()),
        "GT3 Endurance"
    );
}

#[test]
fn career_detail_resolves_production_contract_as_mazda_production() {
    let mut driver = sample_driver();
    driver.categoria_atual = Some("production_challenger".to_string());
    let mut contract = Contract::new(
        "C_PROD_MAZDA".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        "T_PROD_MAZDA".to_string(),
        "Mazda Production Team".to_string(),
        1,
        2,
        70_000.0,
        TeamRole::Numero1,
        "production_challenger".to_string(),
    );
    contract.classe = Some("mazda".to_string());

    let category = resolve_driver_category(&driver, Some(&contract), None);

    assert_eq!(category.as_deref(), Some("production_challenger:mazda"));
    assert_eq!(
        competitive_division_label(&contract.categoria, contract.classe.as_deref()),
        "Mazda Production"
    );
}

#[test]
fn career_debut_year_uses_earliest_competitive_archive_entry() {
    let seasons = vec![
        season_archive_row(2022, "mazda_rookie", 5),
        season_archive_row(2023, "", 0),
        season_archive_row(2024, "gt3", 14),
    ];

    assert_eq!(career_debut_year_from_archive(&seasons, 2024), 2022);
}

#[test]
fn career_path_without_archive_uses_current_season_for_debutants() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL,
                pista TEXT,
                track_name TEXT,
                data TEXT
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0,
                posicao_largada INTEGER NOT NULL DEFAULT 0,
                fastest_lap INTEGER NOT NULL DEFAULT 0
            );
            ",
    )
    .expect("history schema");

    // `ano_inicio_carreira` é o ano do kart (aos 16), não a estreia: sem
    // temporada fechada a estreia é o ano corrente, nunca 2020.
    let mut rookie = sample_driver();
    rookie.ano_inicio_carreira = 2020;
    rookie.stats_carreira.corridas = 0;
    rookie.stats_carreira.temporadas = 0;

    let path = build_driver_career_path_block(
        &conn,
        &rookie,
        None,
        None,
        Some("mazda_rookie"),
        &temporada_em(2024),
        None,
    )
    .expect("career path");

    // Ele já largou 5 vezes na temporada em curso (ainda não arquivada):
    // está no primeiro ano de carreira.
    assert_eq!(path.ano_estreia, 2024);
    assert_eq!(path.historico.presenca.tempo_carreira, 1);

    // Antes da primeira largada não há carreira nenhuma: ele ainda é um novato.
    let mut sem_largada = rookie.clone();
    sem_largada.stats_temporada.corridas = 0;
    let path = build_driver_career_path_block(
        &conn,
        &sem_largada,
        None,
        None,
        Some("mazda_rookie"),
        &temporada_em(2024),
        None,
    )
    .expect("career path");

    assert_eq!(path.ano_estreia, 2024);
    assert_eq!(path.historico.presenca.tempo_carreira, 0);
}

#[test]
fn career_path_without_archive_uses_seeded_seasons_for_veterans() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL,
                pista TEXT,
                track_name TEXT,
                data TEXT
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0,
                posicao_largada INTEGER NOT NULL DEFAULT 0,
                fastest_lap INTEGER NOT NULL DEFAULT 0
            );
            ",
    )
    .expect("history schema");

    let mut veteran = sample_driver();
    veteran.stats_carreira.temporadas = 3;
    veteran.stats_carreira.corridas = 24;

    let path = build_driver_career_path_block(
        &conn,
        &veteran,
        None,
        None,
        Some("gt4"),
        &temporada_em(2024),
        None,
    )
    .expect("career path");

    assert_eq!(path.ano_estreia, 2022);
    assert_eq!(path.historico.presenca.tempo_carreira, 3);
}

#[test]
fn the_debut_team_comes_from_the_first_race_not_from_the_current_deal() {
    // A etiqueta da linha do tempo ("Equipe de estreia: X") e o hover de "Tempo
    // de carreira" mostravam equipes DIFERENTES na mesma tela: o hover lia a
    // primeira corrida, a etiqueta lia o contrato/equipe de agora. Numa carreira
    // de treze anos a etiqueta anunciava a equipe atual como a de estreia.
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (id TEXT PRIMARY KEY, numero INTEGER NOT NULL, ano INTEGER NOT NULL);
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY, temporada_id TEXT, season_id TEXT, rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL, pista TEXT, track_name TEXT, data TEXT
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT, race_id TEXT NOT NULL, piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL, posicao_final INTEGER NOT NULL, dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0, posicao_largada INTEGER NOT NULL DEFAULT 0,
                fastest_lap INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE teams (id TEXT PRIMARY KEY, nome TEXT, cor_primaria TEXT);
            INSERT INTO teams (id, nome, cor_primaria) VALUES
                ('T1', 'Aures Racing', '#3fb950'), ('T2', 'Vector Racing', '#1f6feb');
            INSERT INTO seasons (id, numero, ano) VALUES ('S1', 1, 2019), ('S2', 2, 2020);
            INSERT INTO calendar (id, season_id, rodada, categoria, track_name, data) VALUES
                ('R1', 'S1', 1, 'gt4', 'Circuito de Navarra', '2019-03-10'),
                ('R2', 'S2', 1, 'gt3', 'Spa-Francorchamps', '2020-04-05');
            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, posicao_largada) VALUES
                ('R1', 'P001', 'T1', 4, 3),
                ('R2', 'P001', 'T2', 2, 2);
            ",
    )
    .expect("history schema");

    // O contrato em vigor e o da SEGUNDA equipe — e ele comeca na temporada 1
    // justamente para exercitar o ramo que antes vencia.
    let contrato = Contract::new(
        "C1".to_string(),
        "P001".to_string(),
        "Piloto Teste".to_string(),
        "T2".to_string(),
        "Vector Racing".to_string(),
        1,
        3,
        100_000.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );

    let path = build_driver_career_path_block(
        &conn,
        &sample_driver(),
        None,
        Some(&contrato),
        Some("gt3"),
        &temporada_em(2020),
        None,
    )
    .expect("career path");

    assert_eq!(path.equipe_estreia.as_deref(), Some("Aures Racing"));
    // E a MESMA linha que o hover abre — e isso que a tela precisa garantir.
    let pontas = path
        .historico
        .detalhes
        .get("tempo_carreira")
        .expect("tempo de carreira");
    assert_eq!(path.equipe_estreia, pontas[0].equipe);
}

#[test]
fn career_history_block_derives_special_event_summary() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
            "
            CREATE TABLE driver_season_archive (
                piloto_id TEXT NOT NULL,
                season_number INTEGER NOT NULL,
                ano INTEGER NOT NULL,
                nome TEXT NOT NULL,
                categoria TEXT NOT NULL DEFAULT '',
                posicao_campeonato INTEGER,
                pontos REAL,
                snapshot_json TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id TEXT PRIMARY KEY,
                numero INTEGER NOT NULL,
                ano INTEGER NOT NULL
            );
            CREATE TABLE calendar (
                id TEXT PRIMARY KEY,
                temporada_id TEXT NOT NULL,
                season_id TEXT,
                rodada INTEGER NOT NULL,
                categoria TEXT NOT NULL,
                pista TEXT,
                track_name TEXT,
                data TEXT
            );
            CREATE TABLE race_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                race_id TEXT NOT NULL,
                piloto_id TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                posicao_final INTEGER NOT NULL,
                dnf INTEGER NOT NULL DEFAULT 0,
                pontos REAL NOT NULL DEFAULT 0.0,
                posicao_largada INTEGER NOT NULL DEFAULT 0,
                fastest_lap INTEGER NOT NULL DEFAULT 0
            );
            -- `temporada_inicio` e `temporada_fim` sao TEXT aqui porque sao TEXT na tabela
            -- real (db::migrations::baseline). Como INTEGER, o fixture escondia a comparacao
            -- lexicografica: as temporadas 9, 10, 12 e 26 ordenavam certo por acidente do tipo.
            CREATE TABLE contracts (
                id TEXT PRIMARY KEY,
                piloto_id TEXT NOT NULL,
                piloto_nome TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                equipe_nome TEXT NOT NULL,
                temporada_inicio TEXT NOT NULL,
                temporada_fim TEXT NOT NULL,
                duracao_anos INTEGER NOT NULL,
                salario_anual REAL NOT NULL DEFAULT 0.0,
                papel TEXT NOT NULL DEFAULT 'Numero1',
                status TEXT NOT NULL DEFAULT 'Expirado',
                tipo TEXT NOT NULL DEFAULT 'Especial',
                categoria TEXT NOT NULL,
                classe TEXT,
                created_at TEXT NOT NULL DEFAULT ''
            );

            INSERT INTO seasons (id, numero, ano) VALUES
                ('S006', 6, 2026),
                ('S008', 8, 2028);

            INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome, temporada_inicio,
                temporada_fim, duracao_anos, tipo, categoria, classe, status
            ) VALUES
                ('CSP1', 'P001', 'Piloto Teste', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP2', 'P001', 'Piloto Teste', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP3', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP4', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP5', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP6', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP7', 'P002', 'Piloto Ranking 2', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP8', 'P002', 'Piloto Ranking 2', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP9', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP10', 'P003', 'Piloto Ranking 3', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP11', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP12', 'P003', 'Piloto Ranking 3', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP13', 'P003', 'Piloto Ranking 3', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP14', 'P004', 'Piloto Ranking 4', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP15', 'P004', 'Piloto Ranking 4', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP16', 'P004', 'Piloto Ranking 4', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP17', 'P004', 'Piloto Ranking 4', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP18', 'P005', 'Piloto Ranking 5', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado'),
                ('CSP19', 'P005', 'Piloto Ranking 5', 'SP2', 'Heart of Racing', 8, 8, 1, 'Especial', 'endurance', 'gt4', 'Expirado'),
                ('CSP20', 'P005', 'Piloto Ranking 5', 'SP1', 'Bayern Division', 6, 6, 1, 'Especial', 'production_challenger', 'bmw', 'Expirado');

            INSERT INTO calendar (id, temporada_id, season_id, rodada, categoria) VALUES
                ('SP6_R01', 'S006', 'S006', 1, 'production_challenger'),
                ('SP6_R02', 'S006', 'S006', 2, 'production_challenger'),
                ('SP8_R01', 'S008', 'S008', 1, 'endurance'),
                ('SP8_R02', 'S008', 'S008', 2, 'endurance');

            INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, pontos) VALUES
                ('SP6_R01', 'P001', 'SP1', 2, 0, 18.0),
                ('SP6_R02', 'P001', 'SP1', 6, 0, 8.0),
                ('SP8_R01', 'P001', 'SP2', 1, 0, 25.0),
                ('SP8_R02', 'P001', 'SP2', 3, 0, 17.0),
                ('SP6_R01', 'P002', 'SP1', 1, 0, 25.0),
                ('SP6_R02', 'P002', 'SP1', 1, 0, 25.0),
                ('SP8_R01', 'P002', 'SP2', 2, 0, 18.0),
                ('SP8_R02', 'P002', 'SP2', 2, 0, 18.0),
                ('SP6_R01', 'P003', 'SP1', 2, 0, 18.0),
                ('SP6_R02', 'P003', 'SP1', 2, 0, 18.0),
                ('SP8_R01', 'P003', 'SP2', 2, 0, 18.0),
                ('SP8_R02', 'P003', 'SP2', 2, 0, 18.0),
                ('SP6_R01', 'P004', 'SP1', 3, 0, 15.0),
                ('SP6_R02', 'P004', 'SP1', 3, 0, 15.0),
                ('SP8_R01', 'P004', 'SP2', 3, 0, 15.0),
                ('SP8_R02', 'P004', 'SP2', 3, 0, 15.0);
            ",
        )
        .expect("special event schema");

    let history = build_career_history_block(&conn, "P001").expect("history block");
    let special = history.eventos_especiais;

    assert_eq!(special.participacoes, 2);
    assert_eq!(special.convocacoes, 2);
    assert_eq!(special.vitorias, 1);
    assert_eq!(special.podios, 3);
    assert_eq!(special.rankings.participacoes, Some(5));
    assert_eq!(special.rankings.convocacoes, Some(5));
    assert_eq!(special.rankings.vitorias, Some(2));
    assert_eq!(special.rankings.podios, Some(4));
    assert_eq!(special.timeline.len(), 2);
    assert_eq!(special.timeline[0].ano, 2026);
    assert_eq!(special.timeline[0].categoria, "production_challenger");
    assert_eq!(special.timeline[0].classe.as_deref(), Some("bmw"));
    assert_eq!(special.timeline[1].ano, 2028);
    assert_eq!(
        special.ultimo_evento.as_ref().map(|item| item.ano),
        Some(2028)
    );
    assert_eq!(
        special
            .melhor_campanha
            .as_ref()
            .map(|campaign| (campaign.ano, campaign.pontos)),
        Some((2028, 42))
    );
}

#[test]
fn title_entries_list_year_and_team_of_each_championship() {
    let mut campea_2023 = season_archive_row(2023, "gt3", 12);
    campea_2023.posicao_campeonato = Some(1);
    campea_2023.vitorias = 5;
    campea_2023.pontos = 240.0;
    campea_2023.equipe_id = Some("T-FER".to_string());

    let mut campea_2021 = season_archive_row(2021, "gt4", 10);
    campea_2021.titulos = Some(1);
    campea_2021.vitorias = 4;
    campea_2021.pontos = 190.0;
    campea_2021.equipe_id = Some("T-AUR".to_string());

    // Primeiro lugar sem ter largado: o arquivo diz P1 porque o grid inteiro
    // ficou zerado, e isso NAO e um campeonato. A ficha tem que contar igual a
    // tabela que ordena o mundo.
    let mut vazia = season_archive_row(2022, "gt3", 0);
    vazia.posicao_campeonato = Some(1);

    // Vice nao entra, obviamente — mas entra na fixture para o filtro ter o que
    // recusar por outro motivo que nao a falta de corrida.
    let mut vice = season_archive_row(2020, "gt4", 10);
    vice.posicao_campeonato = Some(2);
    vice.pontos = 150.0;

    let team_lookup = HashMap::from([
        (
            "T-FER".to_string(),
            ("Ferrari".to_string(), "#dc0000".to_string()),
        ),
        (
            "T-AUR".to_string(),
            ("Aures Racing".to_string(), "#3fb950".to_string()),
        ),
    ]);

    let entries =
        build_driver_title_entries(&[vice, campea_2021, vazia, campea_2023], &team_lookup);

    assert_eq!(entries.len(), 2);
    // Do mais recente para o mais antigo.
    assert_eq!(entries[0].ano, 2023);
    assert_eq!(entries[0].equipe.as_deref(), Some("Ferrari"));
    assert_eq!(entries[0].categoria, "gt3");
    assert_eq!(entries[1].ano, 2021);
    assert_eq!(entries[1].equipe.as_deref(), Some("Aures Racing"));
}

#[test]
fn title_entry_survives_a_team_that_no_longer_exists() {
    let mut campea = season_archive_row(2019, "gt4", 10);
    campea.posicao_campeonato = Some(1);
    campea.vitorias = 3;
    campea.pontos = 170.0;
    campea.equipe_id = Some("T-SUMIU".to_string());

    let entries = build_driver_title_entries(&[campea], &HashMap::new());

    // Equipe fechada (ou renomeada para outro id) nao apaga o titulo: fica o ano
    // sem logo, que e o que se sabe.
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].ano, 2019);
    assert!(entries[0].equipe.is_none());
}

fn season_archive_row(ano: i32, categoria: &str, corridas: i32) -> CareerSeasonArchiveRow {
    CareerSeasonArchiveRow {
        // Uma temporada por ano nas fixtures: o número acompanha o ano para os
        // agrupamentos por temporada não colidirem entre linhas diferentes.
        season_number: ano,
        ano,
        categoria: categoria.to_string(),
        classe: None,
        posicao_campeonato: None,
        pontos: 0.0,
        corridas,
        vitorias: 0,
        podios: 0,
        poles: 0,
        titulos: None,
        equipe_id: None,
    }
}

/// Uma corrida do corrida-a-corrida. `posicao` 0 combinada com `dnf` cobre o
/// abandono, que nao tem posicao util.
fn race_row(indice: i32, temporada: i32, posicao: i32, dnf: bool) -> CareerRaceHistoryRow {
    race_row_full(indice, temporada, 0, posicao, dnf, false)
}

#[test]
fn the_drought_is_the_mirror_of_the_peak_and_crosses_the_turn_of_the_year() {
    // Vence a primeira, passa quatro corridas em branco atravessando a virada do
    // ano, e volta a vencer. O jejum e de quatro, comecando em 2024 e terminando
    // em 2025 — exatamente como a sequencia de vitorias e contada, so que ao
    // contrario.
    let corridas = vec![
        race_row(1, 2024, 1, false),
        race_row(2, 2024, 4, false),
        race_row(3, 2024, 2, false),
        race_row(4, 2025, 9, false),
        race_row(5, 2025, 3, false),
        race_row(6, 2025, 1, false),
    ];

    let (jejum, inicio, fim) = longest_winless_streak_span(&corridas);
    assert_eq!(jejum, 4);
    assert_eq!(inicio, Some(2024));
    assert_eq!(fim, Some(2025));
}

#[test]
fn a_retirement_counts_as_a_race_without_a_win_in_the_drought() {
    // Ele largou e nao venceu: o abandono nao interrompe o jejum, ele faz parte
    // dele. Tres corridas sem vitoria, com um DNF no meio.
    let corridas = vec![
        race_row(1, 2024, 1, false),
        race_row(2, 2024, 5, false),
        race_row(3, 2024, 0, true),
        race_row(4, 2024, 6, false),
        race_row(5, 2024, 1, false),
    ];

    assert_eq!(longest_winless_streak_span(&corridas).0, 3);
}

#[test]
fn a_driver_who_never_won_is_in_a_drought_as_long_as_his_career() {
    let corridas: Vec<CareerRaceHistoryRow> = (1..=7)
        .map(|indice| race_row(indice, 2024, indice + 3, false))
        .collect();

    assert_eq!(longest_winless_streak_span(&corridas).0, 7);
}

#[test]
fn the_finish_streak_breaks_on_the_retirement_and_starts_over() {
    // 2 chegadas, DNF, 4 chegadas, DNF, 1 chegada -> a maior e de 4.
    let corridas = vec![
        race_row(1, 2024, 5, false),
        race_row(2, 2024, 7, false),
        race_row(3, 2024, 0, true),
        race_row(4, 2024, 2, false),
        race_row(5, 2024, 8, false),
        race_row(6, 2025, 3, false),
        race_row(7, 2025, 4, false),
        race_row(8, 2025, 0, true),
        race_row(9, 2025, 6, false),
    ];

    assert_eq!(longest_finish_streak(&corridas), 4);
}

#[test]
fn the_worst_season_uses_the_same_rule_as_the_best_one_inverted() {
    let mut ruim = season_archive_row(2019, "gt3", 14);
    ruim.posicao_campeonato = Some(11);
    ruim.pontos = 49.0;
    let mut boa = season_archive_row(2021, "gt3", 14);
    boa.posicao_campeonato = Some(1);
    boa.pontos = 284.0;
    boa.vitorias = 8;
    let mut mediana = season_archive_row(2020, "gt3", 14);
    mediana.posicao_campeonato = Some(4);
    mediana.pontos = 115.0;
    let temporadas = vec![&ruim, &mediana, &boa];

    let pior = worst_career_season(&temporadas).expect("pior temporada");
    assert_eq!(pior.ano, 2019);
    assert_eq!(pior.posicao_campeonato, Some(11));
}

#[test]
fn a_single_season_has_no_worst_one() {
    // Com uma temporada so, a melhor e a pior sao a mesma linha — e chamar a
    // estreia de um novato de "a pior da carreira" e ruido, nao informacao.
    let unica = season_archive_row(2026, "gt4", 6);
    assert!(worst_career_season(&[&unica]).is_none());
    assert!(worst_career_season(&[]).is_none());
}

#[test]
fn the_podium_drought_is_deeper_than_the_win_drought() {
    // Podio na 1a, depois cinco corridas fora do top 3, e um podio de novo.
    // O jejum de PODIOS e 5; o de vitorias e maior, porque ele so venceu no fim.
    let corridas = vec![
        race_row(1, 2024, 2, false),
        race_row(2, 2024, 4, false),
        race_row(3, 2024, 7, false),
        race_row(4, 2024, 0, true),
        race_row(5, 2025, 5, false),
        race_row(6, 2025, 9, false),
        race_row(7, 2025, 3, false),
        race_row(8, 2025, 1, false),
    ];

    let (jejum, inicio, fim) = longest_podiumless_streak_span(&corridas);
    assert_eq!(jejum, 5);
    assert_eq!(inicio, Some(2024));
    assert_eq!(fim, Some(2025));
    // O de vitorias cobre tudo antes da unica vitoria.
    assert_eq!(longest_winless_streak_span(&corridas).0, 7);
}

#[test]
fn the_podium_drought_is_the_whole_career_of_a_midfielder() {
    // Quem nunca subiu ao podio tem o jejum do tamanho da carreira — e essa e a
    // marca que diz alguma coisa sobre ele, ao contrario do jejum de vitorias.
    let corridas: Vec<CareerRaceHistoryRow> = (1..=9)
        .map(|indice| race_row(indice, 2024, 4 + (indice % 5), false))
        .collect();

    assert_eq!(longest_podiumless_streak_span(&corridas).0, 9);
}

#[test]
fn the_first_title_is_the_earliest_archived_championship() {
    let mut vice = season_archive_row(2019, "gt4", 10);
    vice.posicao_campeonato = Some(2);
    vice.pontos = 120.0;
    vice.podios = 5;
    let mut primeiro = season_archive_row(2021, "gt4", 10);
    primeiro.posicao_campeonato = Some(1);
    primeiro.pontos = 180.0;
    primeiro.vitorias = 4;
    primeiro.podios = 7;
    let mut segundo = season_archive_row(2024, "gt3", 12);
    segundo.posicao_campeonato = Some(1);
    segundo.pontos = 260.0;
    segundo.vitorias = 6;
    segundo.podios = 9;
    let temporadas = vec![vice, primeiro, segundo];

    let primeiro_titulo = temporadas
        .iter()
        .find(|season| archived_season_is_title(season))
        .map(season_block)
        .expect("primeiro titulo");

    assert_eq!(primeiro_titulo.ano, 2021);
    assert_eq!(primeiro_titulo.categoria, "gt4");
}

fn race_row_full(
    indice: i32,
    temporada: i32,
    largada: i32,
    posicao: i32,
    dnf: bool,
    volta_rapida: bool,
) -> CareerRaceHistoryRow {
    CareerRaceHistoryRow {
        race_index: indice,
        season_number: temporada,
        team_id: "T001".to_string(),
        position: posicao,
        is_dnf: dnf,
        grid_position: largada,
        has_fastest_lap: volta_rapida,
        // A identidade da corrida no calendario so importa para o detalhe do
        // hover; os calculos de sequencia e sabado nao olham para ela.
        race_id: format!("R{indice}"),
        ano: temporada,
        rodada: indice,
        pista: None,
        categoria: None,
        data: None,
    }
}

#[test]
fn saturday_separates_starting_up_front_from_converting_it() {
    // Tres poles, uma virou vitoria, uma virou P3 e uma virou abandono.
    let corridas = vec![
        race_row_full(1, 2024, 1, 1, false, true),
        race_row_full(2, 2024, 1, 3, false, false),
        race_row_full(3, 2024, 1, 0, true, false),
        race_row_full(4, 2024, 5, 2, false, true),
        race_row_full(5, 2024, 9, 8, false, false),
    ];

    let sabado = build_qualifying_block(&corridas);
    assert_eq!(sabado.poles, 3);
    assert_eq!(sabado.poles_convertidas, 1);
    assert_eq!(sabado.voltas_rapidas, 2);
    // (1 + 1 + 1 + 5 + 9) / 5 = 3.4
    assert_eq!(sabado.grid_medio, Some(3.4));
}

#[test]
fn a_race_without_a_registered_grid_is_not_a_pole() {
    // Grid 0 e largada nao registrada. Contar como pole inventaria a marca, e
    // entrar na media puxaria o grid medio para a frente sem motivo.
    let corridas = vec![
        race_row_full(1, 2024, 0, 1, false, false),
        race_row_full(2, 2024, 4, 2, false, false),
    ];

    let sabado = build_qualifying_block(&corridas);
    assert_eq!(sabado.poles, 0);
    assert_eq!(sabado.poles_convertidas, 0);
    assert_eq!(sabado.grid_medio, Some(4.0));
}

#[test]
fn without_a_single_registered_start_there_is_no_average_grid() {
    let corridas = vec![race_row_full(1, 2024, 0, 7, false, false)];

    assert_eq!(build_qualifying_block(&corridas).grid_medio, None);
    assert_eq!(build_qualifying_block(&[]).grid_medio, None);
}

#[test]
fn the_podium_streak_mirrors_the_podium_drought() {
    let corridas = vec![
        race_row(1, 2024, 5, false),
        race_row(2, 2024, 2, false),
        race_row(3, 2024, 1, false),
        race_row(4, 2025, 3, false),
        race_row(5, 2025, 8, false),
        race_row(6, 2025, 2, false),
    ];

    let (sequencia, inicio, fim) = longest_podium_streak_span(&corridas);
    assert_eq!(sequencia, 3);
    assert_eq!(inicio, Some(2024));
    assert_eq!(fim, Some(2025));
    // O jejum e o complemento: as corridas 1 e 5, isoladas, valem 1 cada.
    assert_eq!(longest_podiumless_streak_span(&corridas).0, 1);
}

/// Banco minimo para o confronto com companheiros: o arquivo das duas pontas.
fn conn_com_duplas() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE team_season_archive (
            team_id TEXT, season_number INTEGER, piloto_1_id TEXT, piloto_2_id TEXT
         );
         CREATE TABLE driver_season_archive (
            piloto_id TEXT, season_number INTEGER, nome TEXT, pontos REAL
         );
         INSERT INTO team_season_archive (team_id, season_number, piloto_1_id, piloto_2_id) VALUES
            ('T1', 1, 'P1', 'P2'),
            ('T1', 2, 'P1', 'P2'),
            ('T1', 3, 'P1', 'P3'),
            ('T2', 4, 'P1', NULL);
         INSERT INTO driver_season_archive (piloto_id, season_number, nome, pontos) VALUES
            ('P1', 1, 'Nosso Piloto', 120.0),
            ('P1', 2, 'Nosso Piloto',  80.0),
            ('P1', 3, 'Nosso Piloto', 150.0),
            ('P1', 4, 'Nosso Piloto', 200.0),
            ('P2', 1, 'Rival Duro',    90.0),
            ('P2', 2, 'Rival Duro',   140.0),
            ('P3', 3, 'Novato',        60.0);",
    )
    .expect("schema de duplas");
    conn
}

#[test]
fn the_duel_counts_seasons_by_points_and_names_the_toughest_teammate() {
    let conn = conn_com_duplas();

    let duelos = build_teammate_block(&conn, "P1").expect("duelos");

    // Tres temporadas com companheiro; a quarta nao tem par e nao entra.
    assert_eq!(duelos.temporadas, 3);
    assert_eq!(duelos.companheiros, 2);
    // Venceu a 1 (120x90) e a 3 (150x60); perdeu a 2 (80x140).
    assert_eq!(duelos.temporadas_vencidas, 2);

    let mais_duro = duelos.rival_mais_duro.expect("rival mais duro");
    // O mais duro e quem o VENCEU, e nao quem ele mais enfrentou.
    assert_eq!(mais_duro.nome, "Rival Duro");
    assert_eq!(mais_duro.derrotas, 1);
    assert_eq!(mais_duro.vitorias, 1);
}

#[test]
fn a_driver_who_never_shared_a_garage_has_no_duel() {
    let conn = conn_com_duplas();

    let duelos = build_teammate_block(&conn, "P9").expect("duelos");

    assert_eq!(duelos.temporadas, 0);
    assert_eq!(duelos.companheiros, 0);
    assert!(duelos.rival_mais_duro.is_none());
}

#[test]
fn the_duel_reads_the_same_from_the_other_side_of_the_garage() {
    let conn = conn_com_duplas();

    // O P3 dividiu uma temporada so, e perdeu: para ELE o mais duro e o P1.
    // O mesmo par, lido das duas pontas, tem que dar placares invertidos.
    let duelos = build_teammate_block(&conn, "P3").expect("duelos");

    assert_eq!(duelos.temporadas, 1);
    assert_eq!(duelos.temporadas_vencidas, 0);
    let mais_duro = duelos.rival_mais_duro.expect("rival mais duro");
    assert_eq!(mais_duro.nome, "Nosso Piloto");
    assert_eq!(mais_duro.derrotas, 1);
    assert_eq!(mais_duro.vitorias, 0);
}

#[test]
fn a_teammate_who_never_won_a_season_is_not_the_toughest_one() {
    let conn = conn_com_duplas();

    // O P1 enfrentou o P2 em duas temporadas e o P3 numa; o P3 nunca o venceu.
    // "Mais duro" e quem VENCEU, e nao quem apareceu mais vezes.
    let mais_duro = build_teammate_block(&conn, "P1")
        .expect("duelos")
        .rival_mais_duro
        .expect("rival mais duro");

    assert_ne!(mais_duro.nome, "Novato");
    assert!(mais_duro.derrotas > 0);
}

#[test]
fn the_world_benchmark_pools_every_start_in_the_database() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE race_results (piloto_id TEXT, posicao_largada INTEGER, dnf INTEGER);
         INSERT INTO race_results (piloto_id, posicao_largada, dnf) VALUES
            ('P1', 1, 0), ('P1', 3, 0), ('P2', 8, 1), ('P2', 0, 0);",
    )
    .expect("schema de resultados");

    let referencias = build_benchmark_block(&conn).expect("referencias");

    // 1 abandono em 4 largadas.
    assert_eq!(referencias.taxa_abandono, Some(25.0));
    // Grid 0 e largada nao registrada e fica fora da media: (1+3+8)/3 = 4.0.
    assert_eq!(referencias.grid_medio, Some(4.0));
}

/// Banco minimo para o detalhe: calendario com data e pista, resultados com
/// grid e volta rapida, e uma lesao amarrada a uma corrida.
fn conn_com_detalhe() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE seasons (id TEXT PRIMARY KEY, numero INTEGER, ano INTEGER);
         CREATE TABLE calendar (
            id TEXT PRIMARY KEY, season_id TEXT, temporada_id TEXT, rodada INTEGER,
            pista TEXT, track_name TEXT, categoria TEXT, data TEXT
         );
         CREATE TABLE race_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT, race_id TEXT, piloto_id TEXT, equipe_id TEXT,
            posicao_final INTEGER, dnf INTEGER DEFAULT 0, posicao_largada INTEGER DEFAULT 0,
            fastest_lap INTEGER DEFAULT 0
         );
         CREATE TABLE injuries (
            id TEXT, pilot_id TEXT, type TEXT, injury_name TEXT, season INTEGER, race_occurred TEXT
         );
         CREATE TABLE teams (id TEXT PRIMARY KEY, nome TEXT, cor_primaria TEXT);
         INSERT INTO teams (id, nome, cor_primaria) VALUES
            ('T1', 'Aures Racing', '#3fb950'), ('T2', 'Vector Racing', '#1f6feb');
         INSERT INTO seasons (id, numero, ano) VALUES ('S1', 1, 2019), ('S2', 2, 2020);
         INSERT INTO calendar (id, season_id, rodada, pista, track_name, categoria, data) VALUES
            ('R1', 'S1', 1, 'Navarra', 'Circuito de Navarra', 'gt4', '2019-03-10'),
            ('R2', 'S1', 2, 'Oulton', 'Oulton Park', 'gt4', '2019-05-13'),
            ('R3', 'S2', 1, 'Spa', 'Spa-Francorchamps', 'gt3', '2020-04-05');
         INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, posicao_largada, fastest_lap) VALUES
            ('R1', 'P1', 'T1', 4, 0, 3, 0),
            ('R2', 'P1', 'T1', 1, 0, 1, 1),
            ('R3', 'P1', 'T2', 0, 1, 2, 0);
         INSERT INTO injuries (id, pilot_id, type, injury_name, season, race_occurred) VALUES
            ('L1', 'P1', 'Leve', 'Dor no pescoco', 2, 'R3');",
    )
    .expect("schema de detalhe");
    conn
}

#[test]
fn the_race_history_carries_the_real_date_track_and_round() {
    let conn = conn_com_detalhe();

    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");

    assert_eq!(corridas.len(), 3);
    assert_eq!(corridas[1].race_index, 2);
    assert_eq!(corridas[1].ano, 2019);
    assert_eq!(corridas[1].rodada, 2);
    // `track_name` ganha de `pista` quando os dois existem.
    assert_eq!(corridas[1].pista.as_deref(), Some("Oulton Park"));
    assert_eq!(corridas[1].data.as_deref(), Some("2019-05-13"));
    assert_eq!(corridas[1].categoria.as_deref(), Some("gt4"));
}

#[test]
fn the_injury_detail_carries_the_race_it_happened_in() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");

    let detalhes = build_career_details(
        &conn,
        "P1",
        &[],
        &corridas,
        &[],
        &DriverCareerPeakBlock::default(),
        &DriverCareerDroughtBlock::default(),
        &DriverCareerReliabilityBlock::default(),
        &DriverCareerTeammateBlock::default(),
    )
    .expect("detalhes");

    // Uma chave POR GRAVIDADE: a lesao leve nao pode aparecer na linha das
    // moderadas, que diria "0" e abriria uma lista com uma entrada.
    let lesoes = detalhes
        .get("lesoes_leves")
        .expect("detalhe das lesoes leves");
    assert_eq!(lesoes.len(), 1);
    assert_eq!(lesoes[0].texto.as_deref(), Some("Dor no pescoco"));
    assert_eq!(lesoes[0].periodo.as_deref(), Some("2020"));
    assert_eq!(lesoes[0].rodada, Some(1));
    assert_eq!(lesoes[0].pista.as_deref(), Some("Spa-Francorchamps"));
    assert!(!detalhes.contains_key("lesoes_moderadas"));
    assert!(!detalhes.contains_key("lesoes_graves"));
}

#[test]
fn the_first_marks_detail_points_at_the_exact_race() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");

    let detalhes = build_career_details(
        &conn,
        "P1",
        &[],
        &corridas,
        &[],
        &DriverCareerPeakBlock::default(),
        &DriverCareerDroughtBlock::default(),
        &DriverCareerReliabilityBlock::default(),
        &DriverCareerTeammateBlock::default(),
    )
    .expect("detalhes");

    let vitoria = &detalhes.get("primeira_vitoria").expect("primeira vitoria")[0];
    assert_eq!(vitoria.data.as_deref(), Some("2019-05-13"));
    assert_eq!(vitoria.rodada, Some(2));
    // O indice na carreira segue disponivel: e o "6a corrida" do card.
    assert_eq!(vitoria.contagem, Some(2));

    let abandonos = detalhes.get("abandonos").expect("abandonos");
    // Abandono vem corrida a corrida, e nao agrupado: data e pista dizem mais.
    assert_eq!(abandonos.len(), 1);
    assert_eq!(abandonos[0].pista.as_deref(), Some("Spa-Francorchamps"));

    // Pole e volta rapida vem AGRUPADAS por equipe e divisao.
    let poles = detalhes.get("poles").expect("poles");
    assert_eq!(poles.len(), 1);
    assert_eq!(poles[0].contagem, Some(1));
    assert_eq!(poles[0].categoria.as_deref(), Some("gt4"));
}

#[test]
fn the_promotion_detail_says_from_which_rung_to_which() {
    let de = season_archive_row(2019, "gt4", 10);
    let para = season_archive_row(2022, "gt3", 12);
    let ativas = vec![&de, &para];
    let conn = conn_com_detalhe();

    let detalhes = build_career_details(
        &conn,
        "P1",
        &[],
        &[],
        &ativas,
        &DriverCareerPeakBlock::default(),
        &DriverCareerDroughtBlock::default(),
        &DriverCareerReliabilityBlock::default(),
        &DriverCareerTeammateBlock::default(),
    )
    .expect("detalhes");

    let promocoes = detalhes.get("promocoes").expect("promocoes");
    assert_eq!(promocoes.len(), 1);
    assert_eq!(promocoes[0].categoria_origem.as_deref(), Some("gt4"));
    assert_eq!(promocoes[0].categoria.as_deref(), Some("gt3"));
    assert_eq!(promocoes[0].periodo.as_deref(), Some("2022"));
    // Nao houve rebaixamento: a chave nem entra no mapa.
    assert!(!detalhes.contains_key("rebaixamentos"));

    let categorias = detalhes.get("categorias").expect("categorias");
    assert_eq!(categorias.len(), 2);
    assert_eq!(categorias[0].categoria.as_deref(), Some("gt4"));
    assert_eq!(categorias[0].contagem, Some(10));
}

#[test]
fn the_records_rank_the_driver_in_the_grid_and_in_the_world() {
    let conn = conn_com_detalhe();
    conn.execute_batch(
        "CREATE TABLE drivers (id TEXT PRIMARY KEY, categoria_atual TEXT, status TEXT);
         INSERT INTO drivers (id, categoria_atual, status) VALUES
            ('P1', 'gt3', 'Ativo'), ('P2', 'gt3', 'Ativo'), ('P3', 'gt4', 'Ativo');
         -- P2 divide o grid da GT3 e fez 2 poles; P3 corre na GT4 e fez 3.
         INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, dnf, posicao_largada, fastest_lap) VALUES
            ('R1', 'P2', 'T1', 2, 0, 1, 0),
            ('R2', 'P2', 'T1', 3, 0, 1, 0),
            ('R1', 'P3', 'T2', 1, 0, 1, 0),
            ('R2', 'P3', 'T2', 1, 0, 1, 0),
            ('R3', 'P3', 'T2', 1, 0, 1, 0);",
    )
    .expect("mundo de comparacao");

    let recordes = super::build_dossier_ranks(&conn, "P1").expect("recordes");

    // P1 tem 1 pole, P2 tem 2 e P3 tem 3: ele e o 3o do mundo entre os tres.
    let poles = recordes.get("poles").expect("rank de poles");
    assert_eq!(poles.mundo, Some(3));
    assert_eq!(poles.mundo_total, 3);
    // No grid da GT3 so ele e P2 contam — P3 corre na GT4.
    assert_eq!(poles.grid, Some(2));
    assert_eq!(poles.grid_total, 2);

    // Abandono conta ao contrario: P1 abandonou 1 de 3 e os outros nenhuma, o
    // que o deixa em ULTIMO na confiabilidade, e nao em primeiro.
    let taxa = recordes.get("taxa_abandono").expect("rank de abandono");
    assert_eq!(taxa.mundo, Some(3));
    assert_eq!(taxa.grid, Some(2));
}

#[test]
fn without_a_world_to_compare_the_records_stay_empty_instead_of_lying() {
    // Banco sem `drivers` nem arquivo de temporadas: a ficha continua de pe e o
    // dossie simplesmente nao oferece o botao de recordes.
    let conn = conn_com_detalhe();

    let recordes = super::build_dossier_ranks(&conn, "P1").expect("recordes");

    // Ele e o unico do mundo: rank existe, mas com denominador 1 — e o frontend
    // esconde a linha nesse caso em vez de anunciar "1o de 1".
    assert_eq!(recordes.get("poles").map(|r| r.mundo_total), Some(1));
    assert_eq!(recordes.get("poles").and_then(|r| r.grid), None);
}

/// Atalho para os detalhes com o banco de fixture e sem os blocos agregados.
fn detalhes_de(
    conn: &rusqlite::Connection,
    seasons: &[CareerSeasonArchiveRow],
    corridas: &[CareerRaceHistoryRow],
    ativas: &[&CareerSeasonArchiveRow],
) -> std::collections::HashMap<String, Vec<super::DriverCareerDetailEntry>> {
    build_career_details(
        conn,
        "P1",
        seasons,
        corridas,
        ativas,
        &DriverCareerPeakBlock::default(),
        &DriverCareerDroughtBlock::default(),
        &DriverCareerReliabilityBlock::default(),
        &DriverCareerTeammateBlock::default(),
    )
    .expect("detalhes")
}

#[test]
fn the_career_time_detail_opens_with_the_debut_and_the_last_race() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");

    let detalhes = detalhes_de(&conn, &[], &corridas, &[]);

    // A carreira inteira em duas linhas: onde comecou e onde parou.
    let pontas = detalhes.get("tempo_carreira").expect("tempo de carreira");
    assert_eq!(pontas.len(), 2);
    assert_eq!(pontas[0].data.as_deref(), Some("2019-03-10"));
    assert_eq!(pontas[0].equipe.as_deref(), Some("Aures Racing"));
    assert_eq!(pontas[1].data.as_deref(), Some("2020-04-05"));
    assert_eq!(pontas[1].equipe.as_deref(), Some("Vector Racing"));
}

/// O recorte por equipe e divisao, e o separador decimal dos dois resumos.
///
/// A media de grid e a taxa de abandono saem com uma casa decimal, e a casa decimal tem
/// separador diferente em cada idioma. Elas eram montadas com `format!("{:.1}")` cru, que
/// e ponto em toda parte — inclusive na tela em portugues, onde "P2.0" nao e como se
/// escreve. `#[serial]`: o locale e estado global do processo.
#[test]
#[serial_test::serial]
fn the_saturday_and_reliability_details_break_down_by_team_and_division() {
    let anterior = rust_i18n::locale().to_string();
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");

    rust_i18n::set_locale("pt-BR");
    let detalhes = detalhes_de(&conn, &[], &corridas, &[]);

    // Duas corridas de GT4 pela Aures (grids 3 e 1) e uma de GT3 pela Vector
    // (grid 2). O recorte leva a EQUIPE junto: uma media de 71 corridas de GT3
    // nao diz nada se elas foram por tres equipes diferentes.
    let grid = detalhes.get("grid_medio").expect("grid medio");
    assert_eq!(grid.len(), 2);
    assert_eq!(grid[0].categoria.as_deref(), Some("gt4"));
    assert_eq!(grid[0].equipe.as_deref(), Some("Aures Racing"));
    assert_eq!(grid[0].resumo.as_deref(), Some("P2,0"));
    assert_eq!(grid[1].categoria.as_deref(), Some("gt3"));
    assert_eq!(grid[1].equipe.as_deref(), Some("Vector Racing"));
    assert_eq!(grid[1].resumo.as_deref(), Some("P2,0"));

    // O unico abandono foi na GT3: numerador e denominador a vista, senao "100%"
    // esconde que e uma corrida so.
    let taxa = detalhes.get("taxa_abandono").expect("taxa de abandono");
    assert_eq!(taxa[0].equipe.as_deref(), Some("Aures Racing"));
    assert_eq!(taxa[0].resumo.as_deref(), Some("0/2 · 0,0%"));
    assert_eq!(taxa[1].equipe.as_deref(), Some("Vector Racing"));
    assert_eq!(taxa[1].resumo.as_deref(), Some("1/1 · 100,0%"));

    // O MESMO numero, no outro idioma: o ponto volta, e nada disso foi persistido.
    rust_i18n::set_locale("en-US");
    let em_ingles = detalhes_de(&conn, &[], &corridas, &[]);
    let grid_en = em_ingles.get("grid_medio").expect("grid medio");
    assert_eq!(grid_en[0].resumo.as_deref(), Some("P2.0"));
    let taxa_en = em_ingles.get("taxa_abandono").expect("taxa de abandono");
    assert_eq!(taxa_en[1].resumo.as_deref(), Some("1/1 · 100.0%"));

    rust_i18n::set_locale(&anterior);
}

#[test]
fn the_idle_years_detail_says_which_team_he_left_and_when_he_came_back() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");
    // Correu 2019 e 2020, ficou parado 2021 e 2022, voltou em 2023.
    let seasons = vec![
        season_archive_row(2019, "gt4", 2),
        season_archive_row(2020, "gt3", 1),
        season_archive_row(2021, "", 0),
        season_archive_row(2022, "", 0),
        season_archive_row(2023, "gt3", 8),
    ];

    let detalhes = detalhes_de(&conn, &seasons, &corridas, &[]);

    let parados = detalhes.get("anos_parados").expect("anos parados");
    assert_eq!(parados.len(), 1);
    assert_eq!(parados[0].periodo.as_deref(), Some("2021-2022"));
    assert_eq!(parados[0].contagem, Some(2));
    // A equipe e a da ULTIMA corrida antes do buraco — de onde ele saiu.
    assert_eq!(parados[0].equipe.as_deref(), Some("Vector Racing"));
    assert_eq!(parados[0].resumo.as_deref(), Some("→ 2023"));
}

#[test]
fn a_season_without_a_podium_carries_its_best_finish() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");
    // A temporada 1 do banco de fixture tem um P4 e um P1; a 2 so tem o abandono.
    let mut sem_podio = season_archive_row(2019, "gt4", 2);
    sem_podio.season_number = 1;
    let ativas = vec![&sem_podio];

    let detalhes = detalhes_de(&conn, &[], &corridas, &ativas);

    let linhas = detalhes
        .get("temporadas_sem_podio")
        .expect("temporadas sem podio");
    assert_eq!(linhas.len(), 1);
    // Sem podio, a melhor chegada e o unico resultado que da tamanho ao ano.
    assert_eq!(linhas[0].melhor_resultado, Some(1));
    // E a lista completa de temporadas usa as MESMAS linhas.
    assert_eq!(detalhes.get("temporadas").expect("temporadas").len(), 1);
}

#[test]
fn the_win_and_podium_cards_open_season_by_season_with_the_team_of_each_year() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");
    // A temporada 1 do banco de fixture tem um P4 e um P1 pela Aures; a 2 tem so
    // o abandono pela Vector.
    let mut fechada = season_archive_row(2019, "gt4", 2);
    fechada.season_number = 1;
    fechada.posicao_campeonato = Some(2);

    let detalhes = detalhes_de(&conn, &[fechada], &corridas, &[]);

    let vitorias = detalhes.get("vitorias").expect("vitorias");
    assert_eq!(vitorias.len(), 1);
    assert_eq!(vitorias[0].periodo.as_deref(), Some("2019"));
    assert_eq!(vitorias[0].equipe.as_deref(), Some("Aures Racing"));
    assert_eq!(vitorias[0].categoria.as_deref(), Some("gt4"));
    assert_eq!(vitorias[0].contagem, Some(1));
    // A colocacao no campeonato NAO entra: "P2" no comeco de uma linha de
    // vitorias se le como posicao de chegada, e numa de podios ("P10") fica
    // simplesmente errado.
    assert_eq!(vitorias[0].texto, None);

    // Abandono nao e podio: a temporada 2 nao aparece em lista nenhuma.
    let podios = detalhes.get("podios").expect("podios");
    assert_eq!(podios.len(), 1);
    assert_eq!(podios[0].periodo.as_deref(), Some("2019"));
    assert_eq!(podios[0].contagem, Some(1));
}

/// O card conta a vitoria de domingo passado; o hover tem que contar a mesma.
/// Ler do arquivo de temporadas deixaria o ano EM CURSO de fora — e e justamente
/// o ano do jogador.
#[test]
fn a_win_in_the_season_still_running_opens_with_the_year_and_the_team() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");

    let detalhes = detalhes_de(&conn, &[], &corridas, &[]);

    let vitorias = detalhes.get("vitorias").expect("vitorias");
    assert_eq!(vitorias.len(), 1);
    assert_eq!(vitorias[0].periodo.as_deref(), Some("2019"));
    assert_eq!(vitorias[0].equipe.as_deref(), Some("Aures Racing"));
    // Sem arquivo, a divisao vem do calendario da propria corrida.
    assert_eq!(vitorias[0].categoria.as_deref(), Some("gt4"));
}

#[test]
fn the_title_card_opens_the_seasons_he_won_and_leaves_the_rest_out() {
    let conn = conn_com_detalhe();
    let corridas = super::load_career_race_history_rows(&conn, "P1").expect("corridas");
    let vice = season_archive_row(2019, "gt4", 10);
    let mut campea = season_archive_row(2020, "gt3", 12);
    campea.posicao_campeonato = Some(1);
    campea.pontos = 240.0;
    campea.vitorias = 5;
    campea.podios = 9;

    let detalhes = detalhes_de(&conn, &[vice, campea], &corridas, &[]);

    let titulos = detalhes.get("titulos").expect("titulos");
    assert_eq!(titulos.len(), 1);
    assert_eq!(titulos[0].periodo.as_deref(), Some("2020"));
    assert_eq!(titulos[0].categoria.as_deref(), Some("gt3"));
    // Toda linha aqui e P1: repetir a colocacao em todas nao diz nada. Quem da
    // tamanho ao ano e a campanha.
    assert_eq!(titulos[0].texto, None);
    assert_eq!(titulos[0].resumo.as_deref(), Some("240 pts · 5V · 9P"));
}

/// A barra empilhada da ficha desenha as tres parcelas dentro do total. Se a
/// soma delas nao fechar exatamente no numero impresso ao lado, a barra mente —
/// e o clamp em 95 mais o arredondamento fazem a soma crua divergir com
/// facilidade.
#[test]
fn transfer_forces_always_add_up_to_the_chance_they_explain() {
    let contrato = |duracao: i32| {
        Contract::new(
            "C1".to_string(),
            "P001".to_string(),
            "Piloto Teste".to_string(),
            "T1".to_string(),
            "Equipe Teste".to_string(),
            1,
            duracao,
            100_000.0,
            TeamRole::Numero1,
            "gt3".to_string(),
        )
    };

    for motivacao in [5.0, 40.0, 72.0, 100.0] {
        for skill in [30.0, 62.0, 88.0, 100.0] {
            // `Contract::new` limita a duracao a tres temporadas, entao o
            // ultimo caso cobre o contrato mais longo que existe.
            for duracao in [1, 2, 3] {
                let mut driver = sample_driver();
                driver.motivacao = motivacao;
                driver.atributos.skill = skill;

                let forcas = transfer_forces_for_driver(&driver, Some(&contrato(duracao)), 1);
                let parcelas = forcas.parcelas.expect("com contrato ha decomposicao");

                assert_eq!(
                    parcelas.contrato + parcelas.motivacao + parcelas.mercado,
                    forcas.total,
                    "motivacao={motivacao} skill={skill} duracao={duracao}",
                );
                assert_eq!(parcelas.anos_restantes, duracao - 1);
            }
        }
    }
}

/// Arquiva uma temporada com o que a curva de CAMPEONATO consome: posicao,
/// tamanho do grid, equipe e corridas disputadas.
fn arquiva_campeonato(
    conn: &rusqlite::Connection,
    temporada: i32,
    ano: i32,
    posicao: i32,
    total_pilotos: Option<i32>,
    corridas: i32,
) {
    let total = match total_pilotos {
        Some(valor) => valor.to_string(),
        // Save antigo: o snapshot simplesmente nao tem a chave.
        None => "null".to_string(),
    };
    conn.execute(
        "INSERT INTO driver_season_archive
            (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
         VALUES ('P001', ?1, ?2, 'Piloto Teste', 'gt3', ?3, 100.0, ?4)",
        rusqlite::params![
            temporada,
            ano,
            posicao,
            format!(
                r#"{{"categoria":"gt3","team_id":"T1","total_pilotos":{total},
                    "corridas":{corridas},"vitorias":0,"podios":0,"poles":0,"titulos":0,
                    "pontos":100.0}}"#
            ),
        ],
    )
    .expect("arquiva temporada de campeonato");
}

/// O denominador de cada ponto e o grid DAQUELE ano, e a temporada em curso
/// entra pela classificacao viva.
///
/// E a premissa do eixo: sem o tamanho do campeonato de cada temporada, um P8
/// entre doze e um P8 entre trinta desenham a mesma altura, e a curva contaria
/// uma subida que foi so uma mudanca de degrau.
#[test]
fn the_championship_curve_carries_each_seasons_own_grid_size() {
    let conn = conn_com_curva_de_mercado();
    arquiva_campeonato(&conn, 1, 2024, 8, Some(12), 10);
    arquiva_campeonato(&conn, 2, 2025, 8, Some(30), 10);
    let arquivo = super::load_career_season_archive_rows(&conn, "P001").expect("arquivo");

    let curva = build_driver_championship_curve(
        &conn,
        "P001",
        &arquivo,
        3,
        2026,
        Some("gt3"),
        None,
        Some(PosicaoDeHoje {
            posicao: 2,
            total: 24,
        }),
    );

    assert_eq!(curva.len(), 3, "duas arquivadas mais a temporada em curso");
    assert_eq!((curva[0].posicao, curva[0].grid), (Some(8), Some(12)));
    assert_eq!((curva[1].posicao, curva[1].grid), (Some(8), Some(30)));
    assert_eq!((curva[2].posicao, curva[2].grid), (Some(2), Some(24)));
    assert!(curva[2].atual, "o ultimo ponto e a temporada em curso");
    assert!(
        !curva[0].atual && !curva[1].atual,
        "temporada arquivada nao e a corrente",
    );
}

/// Temporada sem largada nenhuma nao tem posicao — mesmo com uma linha de
/// classificacao arquivada.
///
/// O arquivo escreve uma linha por piloto por temporada, inclusive para quem
/// passou o ano sem assento, e a posicao dessas linhas e residuo da ordenacao.
/// Desenhar esse numero poria um vice-campeonato no grafico de quem nao correu;
/// o ponto sem posicao e sem equipe e o que a moldura le como ano fora do grid.
#[test]
fn a_season_without_starts_has_no_championship_position() {
    let conn = conn_com_curva_de_mercado();
    arquiva_campeonato(&conn, 1, 2024, 3, Some(20), 10);
    arquiva_campeonato(&conn, 2, 2025, 2, Some(20), 0);
    let arquivo = super::load_career_season_archive_rows(&conn, "P001").expect("arquivo");

    let curva = build_driver_championship_curve(&conn, "P001", &arquivo, 2, 2025, None, None, None);

    assert_eq!(curva.len(), 2);
    assert_eq!(curva[0].posicao, Some(3));
    assert_eq!(curva[1].posicao, None, "sem largada nao ha classificacao");
    assert_eq!(curva[1].equipe_nome, None);
    assert_eq!(curva[1].categoria, "", "ano fora do grid nao tem categoria");
    assert!(!curva[1].titulo);
}

/// Num grid de MX-5 identicos "o que o carro dava" continua tendo um numero — a
/// equipe ainda importa por gente e por dinheiro — mas deixa de medir MAQUINA. A
/// tela desenha essa ressalva como linha tracejada, e quem decide o que e
/// monomarca e a constante da categoria, nao uma lista repetida no frontend.
#[test]
fn the_championship_curve_flags_the_one_make_seasons() {
    let conn = conn_com_curva_de_mercado();
    for (temporada, ano, categoria) in [(1, 2024, "toyota_rookie"), (2, 2025, "gt3")] {
        conn.execute(
            "INSERT INTO driver_season_archive
                (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos,
                 snapshot_json)
             VALUES ('P001', ?1, ?2, 'Piloto Teste', ?3, 4, 100.0, ?4)",
            rusqlite::params![
                temporada,
                ano,
                categoria,
                format!(
                    r#"{{"categoria":"{categoria}","team_id":"T1","total_pilotos":12,
                        "corridas":10,"vitorias":0,"podios":0,"poles":0,"titulos":0,
                        "pontos":100.0}}"#
                ),
            ],
        )
        .expect("arquiva temporada");
    }
    let arquivo = super::load_career_season_archive_rows(&conn, "P001").expect("arquivo");

    let curva = build_driver_championship_curve(&conn, "P001", &arquivo, 2, 2025, None, None, None);

    assert!(curva[0].monomarca, "toyota_rookie poe todos no mesmo carro");
    assert!(!curva[1].monomarca, "gt3 e carro de verdade");
}

/// A expectativa de um ano encerrado sai da classificacao de CONSTRUTORES
/// daquele ano, pela mesma regra da temporada corrente: assentos das equipes a
/// frente, mais o meio da faixa de assentos da propria equipe.
///
/// E a linha de referencia do grafico inteiro — sem ela, "P5" nao diz se ele
/// tirou leite de pedra ou desperdicou o melhor carro do grid.
#[test]
fn the_championship_curve_derives_the_expected_finish_from_the_constructors_table() {
    let conn = conn_com_curva_de_mercado();
    conn.execute_batch(
        "CREATE TABLE team_season_archive (
            team_id TEXT, season_number INTEGER, ano INTEGER, categoria TEXT, classe TEXT,
            posicao_campeonato INTEGER, piloto_1_id TEXT, piloto_2_id TEXT
         );
         INSERT INTO team_season_archive
            (team_id, season_number, ano, categoria, classe, posicao_campeonato, piloto_1_id, piloto_2_id)
         VALUES
            ('T0', 1, 2024, 'gt3', NULL, 1, 'PA', 'PB'),
            ('T9', 1, 2024, 'gt3', NULL, 2, 'PC', 'PD'),
            ('T1', 1, 2024, 'gt3', NULL, 3, 'P001', 'PE'),
            ('T2', 1, 2024, 'gt3', NULL, 4, 'PF', 'PG'),
            ('T3', 1, 2024, 'gt3', NULL, 5, 'PH', 'PI');",
    )
    .expect("arquivo de construtores");
    arquiva_campeonato(&conn, 1, 2024, 5, Some(10), 12);
    let arquivo = super::load_career_season_archive_rows(&conn, "P001").expect("arquivo");

    let curva = build_driver_championship_curve(&conn, "P001", &arquivo, 1, 2024, None, None, None);

    // A equipe dele (T1) foi a 3a de cinco, com duas equipes de dois assentos na
    // frente: quatro assentos melhores, e o meio do proprio bloco cai no 5o.
    assert_eq!(curva[0].esperado, Some(5));
    // Ele terminou exatamente onde o carro dava — a distancia entre as duas
    // linhas e zero, e e isso que o grafico precisa saber dizer.
    assert_eq!(curva[0].posicao, Some(5));
}

/// Sem arquivo de construtores — save antigo, ano fora do grid — a linha de
/// referencia se PARTE em vez de inventar uma expectativa.
#[test]
fn without_a_constructors_archive_there_is_no_expected_finish() {
    let conn = conn_com_curva_de_mercado();
    arquiva_campeonato(&conn, 1, 2024, 5, Some(10), 12);
    let arquivo = super::load_career_season_archive_rows(&conn, "P001").expect("arquivo");

    let curva = build_driver_championship_curve(&conn, "P001", &arquivo, 1, 2024, None, None, None);

    assert_eq!(curva[0].esperado, None);
    assert_eq!(curva[0].posicao, Some(5), "o resultado nao depende dela");
}

/// Save antigo nao guardava `total_pilotos`. O chao do grid volta da contagem do
/// proprio arquivo em vez de sumir — a temporada tinha um tamanho, e a linha de
/// fundo so se parte quando nao ha como saber qual era.
#[test]
fn an_old_archive_recovers_the_grid_size_by_counting_the_season() {
    let conn = conn_com_curva_de_mercado();
    arquiva_campeonato(&conn, 1, 2024, 4, None, 10);
    conn.execute(
        "INSERT INTO driver_season_archive
            (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
         VALUES ('P002', 1, 2024, 'Outro', 'gt3', 1, 200.0, '{}'),
                ('P003', 1, 2024, 'Mais um', 'gt3', 2, 150.0, '{}')",
        [],
    )
    .expect("resto do grid");
    let arquivo = super::load_career_season_archive_rows(&conn, "P001").expect("arquivo");

    let curva = build_driver_championship_curve(&conn, "P001", &arquivo, 1, 2024, None, None, None);

    assert_eq!(
        curva[0].grid,
        Some(3),
        "tres pilotos arquivados na gt3 de 2024"
    );
}

fn conn_com_curva_de_mercado() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    conn.execute_batch(
        "CREATE TABLE driver_season_archive (
            piloto_id TEXT, season_number INTEGER, ano INTEGER, nome TEXT,
            categoria TEXT, posicao_campeonato INTEGER, pontos REAL, snapshot_json TEXT
         );
         -- Temporadas em TEXT, como na tabela real (db::migrations::baseline). Em INTEGER o
         -- fixture escondia a comparacao lexicografica das temporadas de dois digitos.
         CREATE TABLE contracts (
            id TEXT PRIMARY KEY, piloto_id TEXT, piloto_nome TEXT, equipe_id TEXT,
            equipe_nome TEXT, categoria TEXT, classe TEXT, tipo TEXT, status TEXT,
            papel TEXT, salario REAL, salario_anual REAL, duracao_anos INTEGER,
            temporada_inicio TEXT, temporada_fim TEXT, created_at TEXT
         );",
    )
    .expect("schema da curva");
    conn
}

fn arquiva_temporada(conn: &rusqlite::Connection, temporada: i32, ano: i32, skill: f64) {
    conn.execute(
        "INSERT INTO driver_season_archive
            (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
         VALUES ('P001', ?1, ?2, 'Piloto Teste', 'gt3', 5, 100.0, ?3)",
        rusqlite::params![
            temporada,
            ano,
            format!(
                r#"{{"categoria":"gt3","vitorias":0,"podios":0,"titulos":0,
                    "atributos":{{"skill":{skill},"midia":50.0,"desenvolvimento":50.0}}}}"#
            ),
        ],
    )
    .expect("arquiva temporada");
}

/// A curva reconstrói o passado com o piloto QUE ELE ERA, não com o de hoje.
///
/// É a premissa inteira do gráfico: se ela usasse os atributos atuais, a linha
/// seria uma reta espelhando o numero de agora para tras, e a leitura ("ele
/// valia pouco e cresceu") seria uma invencao com cara de dado.
#[test]
fn the_market_curve_values_each_season_with_that_seasons_driver() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 30.0);
    arquiva_temporada(&conn, 2, 2025, 95.0);

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    driver.atributos.skill = 95.0;
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    assert_eq!(curva.len(), 3, "duas arquivadas mais a temporada em curso");
    assert_eq!(curva[0].ano, 2024);
    // Skill 30 contra skill 95 no mesmo tier: o piloto de 2024 nao pode valer o
    // mesmo que o de 2025.
    let em_2024 = curva[0].salario_mercado.expect("2024 tem atributos");
    let em_2025 = curva[1].salario_mercado.expect("2025 tem atributos");
    assert!(em_2025 > em_2024 * 1.5, "2024 {em_2024} vs 2025 {em_2025}");
    // A temporada em curso ainda nao foi arquivada e entra pelo piloto vivo.
    assert!(curva[2].atual);
    assert_eq!(curva[2].ano, 2026);
    assert!(!curva[0].atual);
}

/// Temporada sem atributos arquivados nao tem avaliacao — e nao ter e diferente
/// de valer pouco.
///
/// Num save real, dez temporadas vieram com snapshot enxuto e a curva desenhava
/// uma reta chapada em $39k: o `unwrap_or(50.0)` dos atributos fabricava um
/// piloto mediano e o baseline de categoria desconhecida o jogava no fundo da
/// escada.
/// Um piloto de $1,4M aparecia "valendo" 3% do proprio salario por uma decada,
/// com cara de medicao.
#[test]
fn a_season_without_archived_attributes_has_no_market_value_at_all() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 70.0);
    conn.execute(
        "INSERT INTO driver_season_archive
            (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
         VALUES ('P001', 2, 2025, 'Piloto Teste', '', 4, 90.0, '{}')",
        [],
    )
    .expect("temporada com snapshot enxuto");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    assert_eq!(curva.len(), 3, "a temporada existe, o valor dela e que nao");
    assert!(curva[0].salario_mercado.is_some());
    assert!(
        curva[1].salario_mercado.is_none(),
        "sem atributos nao ha o que reconstruir, e o default seria invencao",
    );
    assert!(
        curva[2].salario_mercado.is_some(),
        "a temporada em curso vem do piloto vivo"
    );
}

/// Categoria ausente no arquivo nao pode derrubar o piloto para o baseline de
/// fundo de escada: a ultima categoria conhecida e uma aproximacao honesta, o
/// degrau de entrada nao e.
#[test]
fn a_missing_category_carries_the_last_known_one_forward() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 70.0);
    conn.execute(
        "INSERT INTO driver_season_archive
            (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
         VALUES ('P001', 2, 2025, 'Piloto Teste', '', 4, 90.0,
                 '{\"atributos\":{\"skill\":70.0,\"midia\":50.0,\"desenvolvimento\":50.0}}')",
        [],
    )
    .expect("temporada sem categoria");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    // Mesmo piloto, mesma categoria herdada: mesmo valor. Sem o arraste, o
    // segundo ano despencaria para o baseline de categoria desconhecida.
    assert_eq!(curva[0].salario_mercado, curva[1].salario_mercado);
}

/// Arquiva uma temporada dizendo por QUEM ele correu — ou que ele nao correu.
///
/// `equipe: None` e a temporada sem standings: sem categoria, sem `team_id`. E o
/// ano que a ficha desenha como buraco na trilha, e onde o contrato fantasma
/// aparecia sozinho pintando salario.
fn arquiva_temporada_por(
    conn: &rusqlite::Connection,
    temporada: i32,
    ano: i32,
    equipe: Option<&str>,
) {
    let categoria = if equipe.is_some() { "gt3" } else { "" };
    let team_json = match equipe {
        Some(id) => format!(r#""{id}""#),
        None => "null".to_string(),
    };
    conn.execute(
        "INSERT INTO driver_season_archive
            (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
         VALUES ('P001', ?1, ?2, 'Piloto Teste', ?3, 5, 100.0, ?4)",
        rusqlite::params![
            temporada,
            ano,
            categoria,
            format!(
                r#"{{"categoria":"{categoria}","team_id":{team_json},"vitorias":0,"podios":0,
                    "titulos":0,
                    "atributos":{{"skill":70.0,"midia":50.0,"desenvolvimento":50.0}}}}"#
            ),
        ],
    )
    .expect("arquiva temporada");
}

fn insere_contrato(
    conn: &rusqlite::Connection,
    id: &str,
    equipe: (&str, &str),
    janela: (i32, i32),
    status: &str,
) {
    conn.execute(
        "INSERT INTO contracts
            (id, piloto_id, piloto_nome, equipe_id, equipe_nome, categoria, classe, tipo, status,
             papel, salario, salario_anual, duracao_anos, temporada_inicio, temporada_fim, created_at)
         VALUES (?1, 'P001', 'Piloto Teste', ?2, ?3, 'gt3', NULL, 'Regular', ?4,
                 'Numero1', 500000.0, 500000.0, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            equipe.0,
            equipe.1,
            status,
            janela.1 - janela.0 + 1,
            janela.0,
            janela.1,
            format!("2026-01-0{id}T12:00:00", id = &id[1..]),
        ],
    )
    .expect("insere contrato");
}

/// Contrato rescindido nao pinta ano que ele nao cumpriu.
///
/// O caso que veio de um save real: contrato de tres temporadas com a equipe A,
/// rescindido depois da primeira; a segunda ele correu pela equipe B; na terceira
/// ficou sem vaga e nao correu por ninguem. A vigencia crua do contrato da A
/// ainda cobria as tres, entao a ficha desenhava o piloto recebendo $500k de uma
/// equipe que ele havia deixado dois anos antes — num ano em que nao correu.
#[test]
fn a_rescinded_contract_stops_at_the_last_season_it_was_served() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada_por(&conn, 1, 2024, Some("T_A"));
    arquiva_temporada_por(&conn, 2, 2025, Some("T_B"));
    arquiva_temporada_por(&conn, 3, 2026, None);
    insere_contrato(&conn, "C1", ("T_A", "Equipe A"), (1, 3), "Rescindido");
    insere_contrato(&conn, "C2", ("T_B", "Equipe B"), (2, 2), "Expirado");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let season = crate::models::season::Season::new("S4".to_string(), 4, 2027);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    assert_eq!(curva[0].equipe_nome.as_deref(), Some("Equipe A"));
    assert_eq!(
        curva[1].equipe_nome.as_deref(),
        Some("Equipe B"),
        "o contrato mais recente manda na temporada sobreposta",
    );
    assert_eq!(
        curva[2].equipe_nome, None,
        "2026 e um ano sem vaga: nao ha salario nem equipe a mostrar",
    );
    assert_eq!(curva[2].salario_contrato, None);
}

/// Assinado e desfeito sem uma corrida: o contrato nao pinta ano nenhum.
///
/// Sao 48 celulas num save de 28 temporadas — o piloto que acerta com uma equipe
/// na janela de transferencias e e dispensado antes da estreia. Pela vigencia
/// crua, a ficha dava a ele uma passagem por uma equipe que nunca o alinhou.
#[test]
fn a_rescinded_contract_never_served_paints_nothing() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada_por(&conn, 1, 2024, Some("T_B"));
    arquiva_temporada_por(&conn, 2, 2025, Some("T_B"));
    insere_contrato(&conn, "C1", ("T_A", "Equipe A"), (1, 2), "Rescindido");
    insere_contrato(&conn, "C2", ("T_B", "Equipe B"), (1, 2), "Expirado");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    assert_eq!(curva[0].equipe_nome.as_deref(), Some("Equipe B"));
    assert_eq!(curva[1].equipe_nome.as_deref(), Some("Equipe B"));
}

/// Contrato EXPIRADO que cobre um ano sem corridas continua valendo.
///
/// Ano contratado sem correr existe — lesao, banco, equipe que saiu do grid — e
/// apagar o salario dele seria trocar um erro por outro. So a rescisao encurta a
/// vigencia, porque so ela diz que o vinculo acabou antes do papel.
#[test]
fn an_expired_contract_still_covers_a_season_the_driver_missed() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada_por(&conn, 1, 2024, Some("T_A"));
    arquiva_temporada_por(&conn, 2, 2025, None);
    insere_contrato(&conn, "C1", ("T_A", "Equipe A"), (1, 2), "Expirado");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    assert_eq!(curva[1].equipe_nome.as_deref(), Some("Equipe A"));
    assert!(curva[1].salario_contrato.is_some());
}

/// Save antigo, sem `team_id` no snapshot: a regra do rescindido se desliga.
///
/// Sem o campo nao da para distinguir "nunca correu por essa equipe" de "o
/// arquivo nao sabe" — e no escuro a regra apagaria a carreira inteira de quem
/// teve contrato rescindido.
#[test]
fn without_archived_team_ids_the_rescinded_rule_stays_out_of_the_way() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 70.0);
    arquiva_temporada(&conn, 2, 2025, 70.0);
    insere_contrato(&conn, "C1", ("T_A", "Equipe A"), (1, 2), "Rescindido");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    assert_eq!(curva[0].equipe_nome.as_deref(), Some("Equipe A"));
    assert_eq!(curva[1].equipe_nome.as_deref(), Some("Equipe A"));
}

/// A curva carrega a CLASSE da temporada arquivada, e nao a categoria crua.
///
/// A trilha de categoria do rodape le `categoria` ponto a ponto: sem a classe,
/// as temporadas passadas na Production viravam "Production" e a atual —
/// que chega por `resolve_driver_category`, ja com a classe — virava "BMW
/// Production". A legenda listava as duas lado a lado, como se o piloto tivesse
/// trocado de campeonato ao renovar na MESMA equipe. Production e Endurance nao
/// sao um campeonato so: quem separa e a classe.
#[test]
fn the_market_curve_names_which_production_the_driver_actually_raced() {
    let conn = conn_com_curva_de_mercado();
    let arquiva_production = |temporada: i32, ano: i32| {
        conn.execute(
            "INSERT INTO driver_season_archive
                (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
             VALUES ('P001', ?1, ?2, 'Piloto Teste', 'production_challenger', 3, 120.0, ?3)",
            rusqlite::params![
                temporada,
                ano,
                r#"{"categoria":"production_challenger","classe":"bmw","vitorias":0,"podios":0,
                    "titulos":0,
                    "atributos":{"skill":70.0,"midia":50.0,"desenvolvimento":50.0}}"#,
            ],
        )
        .expect("arquiva temporada de production");
    };
    arquiva_production(1, 2024);
    arquiva_production(2, 2025);

    let mut driver = sample_driver();
    driver.categoria_atual = Some("production_challenger:bmw".to_string());
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    let divisoes: Vec<&str> = curva.iter().map(|ponto| ponto.categoria.as_str()).collect();
    assert_eq!(
        divisoes,
        vec![
            "production_challenger:bmw",
            "production_challenger:bmw",
            "production_challenger:bmw",
        ],
        "passado e presente na mesma Production tem que ser a MESMA divisao",
    );
}

/// A escada de avaliacao tem que subir ate o topo — e o topo e o endurance.
///
/// O `match` por tier do baseline parava no 5. O endurance e tier 6 e caia no
/// `_` de categoria desconhecida ($20k, abaixo do amador): a ficha dizia que o
/// mercado pagaria ~$100k por um piloto de endurance cujo contrato na grade
/// vale $524k-$1.5M, e a curva desenhava um degrau de 10x na temporada em que
/// ele subia de categoria — como se subir ao apice fosse desvalorizar.
///
/// O teste ancora nos dois lados: a ORDEM dos degraus (nenhum tier vale menos
/// que o de baixo) e a FAIXA que a geracao de contratos usa, que e o que as
/// equipes de fato pagam. Se a escada da economia mudar, este teste muda junto
/// — que e exatamente o acoplamento que faltava.
#[test]
fn the_market_model_pays_the_top_of_the_ladder_like_the_grid_does() {
    use super::{avaliar_mercado, EntradasDeMercado};

    let salario = |categoria: &str| {
        avaliar_mercado(&EntradasDeMercado {
            categoria: Some(categoria),
            skill: 70.0,
            midia: 50.0,
            desenvolvimento: 50.0,
            titulos: 0,
            vitorias: 0,
            podios: 0,
        })
        .salario
    };

    let escada = [
        "mazda_rookie",
        "mazda_amador",
        "bmw_m2",
        "gt4",
        "gt3",
        "endurance",
    ];
    for degrau in escada.windows(2) {
        let (abaixo, acima) = (salario(degrau[0]), salario(degrau[1]));
        assert!(
            acima > abaixo,
            "{} ({acima}) tem que pagar mais que {} ({abaixo})",
            degrau[1],
            degrau[0],
        );
    }

    // O piloto mediano do tier ganha a base do tier: o numero da ficha cai
    // dentro da faixa dos contratos que o mundo realmente assina la.
    let endurance = salario("endurance");
    let (piso, teto) = crate::models::contract::salary_range_for_tier(6);
    assert!(
        endurance >= piso && endurance <= teto,
        "endurance mediano em {endurance} fora da faixa real ({piso}..{teto})",
    );

    // A chave de divisao competitiva e a mesma categoria: o card de mercado
    // recebe "endurance:lmp2" de `resolve_driver_category`, e sem tirar a
    // classe ele caia no desconhecido mesmo com o tier 6 mapeado.
    assert_eq!(salario("endurance:lmp2"), endurance);
    assert_eq!(salario("production_challenger:mazda"), salario("bmw_m2"));
}

/// Um contrato de tres anos e UMA linha na tabela e TRES pontos na curva. Sem
/// espalhar a vigencia, os dois anos do meio virariam buracos e a linha do
/// salario apareceria picotada num piloto que nunca ficou sem equipe.
#[test]
fn the_market_curve_spreads_a_multi_year_contract_over_its_seasons() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 60.0);
    arquiva_temporada(&conn, 2, 2025, 62.0);
    conn.execute(
        "INSERT INTO contracts
            (id, piloto_id, piloto_nome, equipe_id, equipe_nome, categoria, tipo, status,
             papel, salario, salario_anual, duracao_anos, temporada_inicio, temporada_fim, created_at)
         VALUES ('C1', 'P001', 'Piloto Teste', 'T1', 'Arclight', 'gt3', 'Regular', 'Ativo',
                 'Numero1', 0.0, 250000.0, 3, 1, 3, '2024-01-01')",
        [],
    )
    .expect("contrato de tres anos");

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    let assinados: Vec<_> = curva.iter().filter_map(|p| p.salario_contrato).collect();
    assert_eq!(assinados, vec![250_000.0, 250_000.0]);
    assert_eq!(curva[0].equipe_nome.as_deref(), Some("Arclight"));
    // A temporada 3 esta na vigencia, mas o ponto atual usa o contrato VIVO que a
    // ficha recebeu — sem contrato ativo em maos, o ponto de hoje fica sem salario.
    assert!(curva[2].salario_contrato.is_none());
}

/// O contrato assinado e fato, e a curva ia embora dele no dia em que o
/// calendario acabava.
///
/// Um piloto de base tem duas temporadas de passado e tres de contrato: a ficha
/// dele desenhava dois pontos num quadro dimensionado para uma carreira inteira,
/// jogando fora a metade da historia que ele de fato tem. Os anos ja assinados
/// entram — mas so a linha do salario. O que o mercado pagaria em 2028 depende
/// de quem ele vai ser em 2028, e inventar isso e exatamente o que a curva
/// existe para nao fazer.
#[test]
fn the_market_curve_carries_on_through_the_seasons_already_under_contract() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 40.0);
    arquiva_temporada(&conn, 2, 2025, 45.0);

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let contrato = Contract::new(
        "C_FUTURO".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        "T1".to_string(),
        "Arclight".to_string(),
        3,
        3,
        180_000.0,
        TeamRole::Numero2,
        "gt3".to_string(),
    );
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva = build_driver_market_curve(&conn, &driver, Some(&contrato), None, &season)
        .expect("curva de mercado");

    assert_eq!(
        curva.len(),
        5,
        "duas arquivadas, a de hoje e as duas assinadas"
    );
    assert_eq!(curva[4].ano, 2028);
    assert!(curva[3].futuro && curva[4].futuro);
    assert!(!curva[2].futuro && curva[2].atual);

    for ponto in &curva[3..] {
        assert_eq!(ponto.salario_contrato, Some(180_000.0));
        assert_eq!(ponto.equipe_nome.as_deref(), Some("Arclight"));
        assert_eq!(ponto.categoria, "gt3");
        assert!(
            ponto.salario_mercado.is_none(),
            "valor de mercado futuro seria invencao"
        );
    }
}

/// Contrato que acaba nesta temporada nao gera ponto nenhum a frente — senao a
/// regua do "hoje" apareceria sem nada depois dela.
#[test]
fn the_market_curve_stops_at_today_when_the_contract_ends_now() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 40.0);

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    let contrato = Contract::new(
        "C_ACABA".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        "T1".to_string(),
        "Arclight".to_string(),
        1,
        2,
        180_000.0,
        TeamRole::Numero2,
        "gt3".to_string(),
    );
    let season = crate::models::season::Season::new("S2".to_string(), 2, 2025);

    let curva = build_driver_market_curve(&conn, &driver, Some(&contrato), None, &season)
        .expect("curva de mercado");

    assert!(curva.iter().all(|p| !p.futuro));
    assert_eq!(curva.last().expect("ponto de hoje").ano, 2025);
}

/// Agente livre nao tem o que decompor: o 100% E a ausencia de vinculo, e uma
/// barra de tres cores ali seria invencao.
#[test]
fn a_free_agent_has_no_forces_to_break_down() {
    let forcas = transfer_forces_for_driver(&sample_driver(), None, 1);

    assert_eq!(forcas.total, 100);
    assert!(forcas.parcelas.is_none());
}

/// O salario estimado e o que o MERCADO pagaria. Ele caia no salario do contrato
/// quando havia contrato, e a ficha imprimia o mesmo numero duas vezes em cards
/// vizinhos — a comparacao que a aba existe para fazer nao existia.
#[test]
fn the_estimated_salary_does_not_echo_the_signed_one() {
    let mut driver = sample_driver();
    driver.atributos.skill = 88.0;
    driver.stats_carreira.vitorias = 12;
    let contrato = Contract::new(
        "C1".to_string(),
        driver.id.clone(),
        driver.nome.clone(),
        "T1".to_string(),
        "Equipe Teste".to_string(),
        1,
        2,
        1.0,
        TeamRole::Numero1,
        "gt3".to_string(),
    );

    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    let bloco = super::build_driver_market_block(&conn, &driver, Some(&contrato), None, 1);

    // Um salario de $1 nao pode arrastar o valor de mercado para $1.
    assert!(bloco.salario_estimado.expect("estimado") > 1.0);
    assert!(bloco.valor_mercado.expect("valor") > bloco.salario_estimado.expect("estimado"));
}

/// "$23,016" nao se julga sozinho: o ordinal no grid e a regua que diz se aquilo
/// e o carro mais caro do pelotao ou o mais barato.
#[test]
fn the_market_block_ranks_the_driver_against_his_own_grid() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut pilotos = HashMap::new();
    for (id, nome, skill) in [
        ("P001", "O caro", 90.0),
        ("P002", "O medio", 70.0),
        ("P003", "O barato", 40.0),
        ("P004", "Outro medio", 70.0),
    ] {
        let mut driver = Driver::new(
            id.to_string(),
            nome.to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2024,
        );
        driver.categoria_atual = Some("gt3".to_string());
        driver.atributos.skill = skill;
        driver.atributos.midia = 50.0;
        driver.atributos.desenvolvimento = 50.0;
        crate::db::queries::drivers::insert_driver(&conn, &driver).expect("driver");
        pilotos.insert(id, driver);
    }

    let equipe_a = gt3_team("T001", &["P001", "P002"]);
    let equipe_b = gt3_team("T002", &["P003", "P004"]);
    crate::db::queries::teams::insert_team(&conn, &equipe_a).expect("team");
    crate::db::queries::teams::insert_team(&conn, &equipe_b).expect("team");

    let bloco = super::build_driver_market_block(&conn, &pilotos["P001"], None, Some(&equipe_a), 1);
    assert_eq!(bloco.posicao_valor, Some(1));
    assert_eq!(bloco.total_valor, Some(4));
    assert_eq!(bloco.categoria_valor.as_deref(), Some("gt3"));

    // Empate divide a posicao: dois pilotos identicos sao os dois 2º, e o de
    // baixo e 4º. Decidir no desempate por id imprimiria uma hierarquia que o
    // modelo nao viu.
    assert_eq!(
        super::build_driver_market_block(&conn, &pilotos["P002"], None, Some(&equipe_a), 1)
            .posicao_valor,
        Some(2)
    );
    assert_eq!(
        super::build_driver_market_block(&conn, &pilotos["P004"], None, Some(&equipe_b), 1)
            .posicao_valor,
        Some(2)
    );
    assert_eq!(
        super::build_driver_market_block(&conn, &pilotos["P003"], None, Some(&equipe_b), 1)
            .posicao_valor,
        Some(4)
    );
}

/// Sem assento nao ha ordinal. Um aposentado ou um piloto entre categorias nao e
/// o ultimo do grid — ele nao esta na lista, e imprimir "12º de 12" para quem
/// nao corre inventaria um lugar.
#[test]
fn the_market_block_has_no_rank_for_a_driver_outside_the_grid() {
    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut sentado = Driver::new(
        "P001".to_string(),
        "Titular".to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2024,
    );
    sentado.categoria_atual = Some("gt3".to_string());
    crate::db::queries::drivers::insert_driver(&conn, &sentado).expect("driver");

    let equipe = gt3_team("T001", &["P001"]);
    crate::db::queries::teams::insert_team(&conn, &equipe).expect("team");

    let mut sem_assento = sample_driver();
    sem_assento.id = "P999".to_string();
    sem_assento.categoria_atual = Some("gt3".to_string());

    let bloco = super::build_driver_market_block(&conn, &sem_assento, None, None, 1);
    assert_eq!(bloco.posicao_valor, None);
    assert_eq!(bloco.total_valor, None);
    assert_eq!(bloco.categoria_valor, None);
    // O valor em si continua existindo: o que falta e a comparacao, nao o numero.
    assert!(bloco.valor_mercado.expect("valor") > 0.0);
}

/// A tendencia do card sai do VALOR de cada temporada, e nao do salario
/// estimado: os dois divergem quando midia ou desenvolvimento mudam, e um "+18%"
/// medido no proxy errado seria um numero preciso sobre a coisa errada.
#[test]
fn the_market_curve_carries_the_value_next_to_the_salary() {
    let conn = conn_com_curva_de_mercado();
    arquiva_temporada(&conn, 1, 2024, 40.0);
    arquiva_temporada(&conn, 2, 2025, 80.0);

    let mut driver = sample_driver();
    driver.categoria_atual = Some("gt3".to_string());
    driver.atributos.skill = 80.0;
    let season = crate::models::season::Season::new("S3".to_string(), 3, 2026);

    let curva =
        build_driver_market_curve(&conn, &driver, None, None, &season).expect("curva de mercado");

    for ponto in &curva {
        assert_eq!(
            ponto.valor_mercado.is_some(),
            ponto.salario_mercado.is_some(),
            "valor e salario nascem da mesma avaliacao, ano {}",
            ponto.ano
        );
    }
    let primeiro = curva[0].valor_mercado.expect("valor de 2024");
    let segundo = curva[1].valor_mercado.expect("valor de 2025");
    assert!(
        segundo > primeiro,
        "o piloto dobrou de skill entre os dois anos"
    );
}

/// A nacionalidade da ficha e um rotulo de DISPLAY, resolvido no locale ATIVO.
///
/// O save guarda a forma que estava em vigor quando o piloto nasceu — e saves antigos
/// gravaram sem acento —, entao a ficha lia "Britanico" e mandava isso para a tela, em
/// portugues, mesmo com o jogo em ingles. Nada disso volta para o banco: `driver` sai
/// deste teste com o valor gravado intacto.
///
/// `#[serial]`: o locale e estado global do processo.
#[test]
#[serial_test::serial]
fn a_nacionalidade_da_ficha_segue_o_locale_e_nao_o_valor_gravado() {
    let anterior = rust_i18n::locale().to_string();

    // A forma legada, sem bandeira e sem acento, que e o pior caso do dado real.
    let mut driver = sample_driver();
    driver.nacionalidade = "Britanico".to_string();
    driver.genero = "M".to_string();

    rust_i18n::set_locale("pt-BR");
    let ficha = super::build_driver_profile_block(&driver, "ativo", None, None, None, Vec::new());
    assert_eq!(ficha.bandeira, "\u{1F1EC}\u{1F1E7}");
    assert_eq!(ficha.nacionalidade, "Britânico");

    rust_i18n::set_locale("en-US");
    let ficha_en =
        super::build_driver_profile_block(&driver, "ativo", None, None, None, Vec::new());
    assert_eq!(ficha_en.bandeira, "\u{1F1EC}\u{1F1E7}");
    assert_eq!(ficha_en.nacionalidade, "British");

    // O gentilico flexiona em PT, e o genero vem do piloto — o rotulo gravado nao e
    // fonte confiavel dele, porque em ingles as duas formas sao a mesma palavra.
    driver.genero = "F".to_string();
    rust_i18n::set_locale("pt-BR");
    let ficha_fem =
        super::build_driver_profile_block(&driver, "ativo", None, None, None, Vec::new());
    assert_eq!(ficha_fem.nacionalidade, "Britânica");

    // E o valor PERSISTIDO nao foi tocado por nada disso.
    assert_eq!(driver.nacionalidade, "Britanico");

    rust_i18n::set_locale(&anterior);
}
