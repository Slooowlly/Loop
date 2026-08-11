//! Testes de `career::standings`: classificacao, grid e dossie de equipe.
//!
//! Fatiado de `tests/mod.rs`, que juntava as dez areas num arquivo so. Os helpers
//! e os `use` continuam no `mod.rs` e chegam aqui pelo glob.

use super::*;

#[test]
fn test_get_teams_standings_keeps_special_lineup_after_skip_cleanup() {
    let base_dir = create_test_career_dir("special_team_standings_after_skip");

    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    force_legacy_blocoregular_state(&db);
    db.conn
        .execute(
            "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
            [],
        )
        .expect("complete regular block");
    crate::convocation::advance_to_convocation_window(&db.conn).expect("advance convocation");
    crate::convocation::run_convocation_window(&db.conn).expect("run convocation");
    crate::convocation::iniciar_bloco_especial(&db.conn).expect("start special block");
    drop(db);

    crate::commands::race::simulate_special_block_in_base_dir(&base_dir, "career_001")
        .expect("simulate special block");
    let db = Database::open_existing(&db_path).expect("db after special sim");
    crate::convocation::encerrar_bloco_especial(&db.conn).expect("end special block");
    crate::convocation::run_pos_especial(&db.conn).expect("run pos especial");
    drop(db);

    let standings =
        get_teams_standings_in_base_dir(&base_dir, "career_001", "production_challenger")
            .expect("production team standings");

    assert!(
        !standings.is_empty(),
        "standings de equipes especiais devem continuar visiveis apos o cleanup"
    );
    assert!(
        standings.iter().any(|team| team.pontos > 0),
        "standings de equipes especiais devem refletir pontos simulados"
    );
    assert!(
        standings
            .iter()
            .any(|team| { team.piloto_1_nome.is_some() || team.piloto_2_nome.is_some() }),
        "standings de equipes especiais devem preservar os pilotos pelo historico de corrida"
    );
    assert!(
        standings
            .iter()
            .any(|team| team.classe.as_deref() == Some("bmw")),
        "standings de equipes especiais devem carregar a classe/carro da equipe"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_teams_standings_returns_category_grid() {
    let base_dir = create_test_career_dir("teams_standings");
    let standings = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");

    assert_eq!(standings.len(), 6);
    assert_eq!(standings[0].posicao, 1);
    assert!(standings[0].founded_year > 0);

    // Assento ocupado precisa vir com o ID do piloto, e nao so com o nome: e ele
    // que o grid do mercado usa para abrir a ficha rapida. Nome sem id deixa o
    // assento mudo — que foi exatamente o sintoma quando o payload so tinha nome.
    let ocupado = standings
        .iter()
        .find(|team| team.piloto_1_nome.is_some())
        .expect("ao menos um assento ocupado no grid");
    assert!(
        ocupado.piloto_1_id.is_some(),
        "assento com piloto '{:?}' veio sem id",
        ocupado.piloto_1_nome
    );

    // E o contrario tambem: assento VAZIO nao pode trazer id de ninguem.
    for team in &standings {
        if team.piloto_2_nome.is_none() {
            assert!(
                team.piloto_2_id.is_none(),
                "assento vazio da equipe {} trouxe id",
                team.nome
            );
        }
    }

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_teams_car_parts_returns_eleven_parts_per_team() {
    let base_dir = create_test_career_dir("teams_car_parts");
    let cars = get_teams_car_parts_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("car parts");

    // Equipe sem carro persistido é omitida de propósito; o que não pode acontecer é
    // uma equipe entrar na lista com um conjunto de peças incompleto.
    for car in &cars {
        assert_eq!(
            car.parts.len(),
            11,
            "equipe {} deve trazer as 11 pecas do carro",
            car.nome
        );
        assert!(!car.team_id.is_empty());
        for part in &car.parts {
            assert!(!part.key.is_empty(), "peca sem chave estavel");
            assert!(
                (1..=10).contains(&part.level),
                "nivel de peca fora de 1..=10: {}",
                part.level
            );
        }
    }

    // As chaves seguem a ordem estavel de `PartType::ALL` — o radar depende dela para
    // que o eixo N seja a mesma peca em todas as equipes.
    if let Some(first) = cars.first() {
        let keys: Vec<&str> = first.parts.iter().map(|part| part.key.as_str()).collect();
        assert_eq!(keys[0], "chassis");
        assert_eq!(keys[10], "electronics");
    }

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_get_teams_standings_uses_previous_season_order_before_first_race() {
    let base_dir = create_test_career_dir("teams_standings_previous_order");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let teams = team_queries::get_teams_by_category(&db.conn, "mazda_rookie").expect("teams");
    let first_team = teams.first().expect("first team");
    let second_team = teams.get(1).expect("second team");

    db.conn
        .execute(
            "UPDATE seasons SET numero = 2, ano = 2026 WHERE status = 'EmAndamento'",
            [],
        )
        .expect("move active season");
    db.conn
        .execute(
            "INSERT INTO seasons (id, numero, ano, status, rodada_atual, fase, created_at, updated_at)
             VALUES ('S_PREV_TEAM_ORDER', 1, 2025, 'Finalizada', 8, 'PosEspecial', '', '')",
            [],
        )
        .expect("insert previous season");
    db.conn
        .execute(
            "INSERT INTO drivers (id, nome, idade, nacionalidade, genero)
             VALUES
                ('P_PREV_LOW', 'Piloto Anterior Baixo', 24, 'Brasil', 'M'),
                ('P_PREV_HIGH', 'Piloto Anterior Alto', 26, 'Brasil', 'M')",
            [],
        )
        .expect("insert previous drivers");
    db.conn
        .execute(
            "INSERT INTO standings (
                temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
             ) VALUES
                ('S_PREV_TEAM_ORDER', 'P_PREV_LOW', ?1, 'mazda_rookie', 2, 12, 0, 0, 0, 8),
                ('S_PREV_TEAM_ORDER', 'P_PREV_HIGH', ?2, 'mazda_rookie', 1, 88, 4, 6, 0, 8)",
            rusqlite::params![&first_team.id, &second_team.id],
        )
        .expect("insert previous standings");

    let standings = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");

    assert_eq!(standings[0].id, second_team.id);
    assert_eq!(standings[0].posicao, 1);
    assert_eq!(standings[1].id, first_team.id);
    assert_eq!(standings[1].posicao, 2);
    assert_eq!(
        standings[0].pontos, 0,
        "temporada atual ainda deve estar zerada"
    );

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
#[serial_test::serial]
fn test_get_team_history_dossier_uses_real_race_results_for_any_team() {
    rust_i18n::set_locale("pt-BR"); // dossiê assevera prosa PT (ver race_eval).
    let base_dir = create_test_career_dir("team_history_dossier_real_results");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");
    let selected = teams.first().expect("selected team");
    let rival = teams.get(1).expect("rival team");
    let (selected_driver_1, selected_driver_2) =
        team_driver_ids(&db.conn, &selected.id).expect("selected drivers");
    let (rival_driver_1, _) = team_driver_ids(&db.conn, &rival.id).expect("rival drivers");
    let race_ids: Vec<String> = db
        .conn
        .prepare(
            "SELECT id FROM calendar
             WHERE categoria = 'mazda_rookie'
             ORDER BY rodada ASC
             LIMIT 4",
        )
        .expect("prepare races")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query races")
        .collect::<Result<Vec<_>, _>>()
        .expect("race ids");

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("clear race results");
    for (race_id, driver_id, team_id, finish, points) in [
        (&race_ids[0], &selected_driver_1, &selected.id, 1, 25.0),
        (&race_ids[0], &selected_driver_2, &selected.id, 4, 12.0),
        (&race_ids[0], &rival_driver_1, &rival.id, 2, 18.0),
        (&race_ids[1], &selected_driver_1, &selected.id, 2, 18.0),
        (&race_ids[1], &selected_driver_2, &selected.id, 5, 10.0),
        (&race_ids[1], &rival_driver_1, &rival.id, 1, 25.0),
        (&race_ids[2], &selected_driver_1, &selected.id, 8, 4.0),
        (&race_ids[2], &selected_driver_2, &selected.id, 9, 2.0),
        (&race_ids[2], &rival_driver_1, &rival.id, 1, 25.0),
        (&race_ids[3], &selected_driver_1, &selected.id, 3, 15.0),
        (&race_ids[3], &selected_driver_2, &selected.id, 6, 8.0),
        (&race_ids[3], &rival_driver_1, &rival.id, 1, 25.0),
    ] {
        db.conn
            .execute(
                "INSERT INTO race_results (
                    race_id, piloto_id, equipe_id, posicao_final, pontos
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![race_id, driver_id, team_id, finish, points],
            )
            .expect("insert race result");
    }
    db.conn
        .execute(
            "UPDATE teams
             SET cash_balance = ?1,
                 debt_balance = ?2,
                 financial_state = ?3,
                 last_round_income = ?4,
                 last_round_expenses = ?5,
                 last_round_net = ?6,
                 car_performance = ?7,
                 engineering = ?8,
                 facilities = ?9
             WHERE id = ?10",
            rusqlite::params![
                4_200_000.0,
                1_250_000.0,
                "pressured",
                380_000.0,
                510_000.0,
                -130_000.0,
                7.4,
                63.0,
                58.0,
                &selected.id,
            ],
        )
        .expect("update real finance snapshot");
    // O "pacote técnico" do dossiê é o Nível do Carro (as 11 peças), NÃO a coluna legada
    // `car_performance` acima — que o sistema de peças nunca atualiza. Semeia o carro no
    // nível 7 pra o dossiê ter o que ler.
    crate::db::queries::team_car::upsert_team_car(
        &db.conn,
        &selected.id,
        &crate::car::Car::uniform(7),
    )
    .expect("seed team car");
    drop(db);

    let dossier =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &selected.id, "mazda_rookie")
            .expect("team dossier");

    assert!(dossier.has_history);
    // Os cards de record comparam dentro da CATEGORIA, não do grupo: o card
    // responde "onde esta equipe está entre as que correm com ela", e quem corre
    // com ela é a categoria. É também o que faz o card e a tabela de recordes
    // (que abre em "só a categoria") mostrarem o mesmo número.
    assert_eq!(dossier.record_scope, "Mazda Rookie");
    assert_eq!(dossier.sport.races, 4);
    assert_eq!(dossier.sport.wins, 1);
    assert_eq!(dossier.sport.podiums, 3);
    assert_eq!(dossier.sport.win_rate, "25%");
    assert_eq!(dossier.sport.podium_rate, "75%");
    assert_eq!(dossier.sport.seasons, "1 Temporada");
    assert_eq!(dossier.sport.current_streak, "1 temporada no nível Rookie");
    assert_eq!(dossier.sport.best_streak, "2 Pódios consecutivos");
    assert!(dossier
        .timeline
        .iter()
        .any(|item| item.text.contains("vitória real")));
    assert_eq!(
        dossier
            .records
            .iter()
            .find(|record| record.label == "Vitórias")
            .map(|record| (record.rank.as_str(), record.value.as_str())),
        Some(("2º", "1"))
    );
    // Todos os records comparam contra o MESMO universo: as equipes que correram
    // no grupo. Títulos rankeava só contra as campeãs, e o dossiê mostrava
    // denominadores diferentes lado a lado ("10º de 10" junto de "14º de 19").
    let record_by_id = |id: &str| {
        dossier
            .records
            .iter()
            .find(|record| record.id == id)
            .unwrap_or_else(|| panic!("record {id}"))
            .clone()
    };
    assert_eq!(record_by_id("titles").rank_total, 2);
    assert_eq!(record_by_id("wins").rank_total, 2);
    assert_eq!(record_by_id("podiums").rank_total, 2);
    assert_eq!(record_by_id("titles").value, "0");
    // A média do grupo em títulos conta os zeros das não-campeãs.
    assert_eq!(record_by_id("titles").group_average, "0,0");
    // Colocações da temporada: 1º, 2º, 8º e 3º nas quatro corridas — o 8º não
    // entra em nenhum degrau, e os degraus não se sobrepõem.
    let season = dossier.season_results.first().expect("season result");
    assert_eq!(
        (
            season.races,
            season.wins,
            season.seconds,
            season.thirds,
            season.fourths,
            season.fifths,
            season.podiums
        ),
        (4, 1, 1, 1, 0, 0, 3)
    );
    // Fita de forma recente: uma entrada por corrida, da mais antiga para a mais
    // nova, com a colocação de cada uma.
    assert_eq!(
        dossier
            .recent_form
            .iter()
            .map(|race| race.position)
            .collect::<Vec<_>>(),
        vec![Some(1), Some(2), Some(8), Some(3)]
    );
    assert_eq!(dossier.recent_form[0].category_id, "mazda_rookie");
    // Assinatura: as faixas são exclusivas e somam as corridas. O 8º cai em
    // 6º-10º, e não some como caía da faixa de top 5.
    let spread = &dossier.result_spread;
    assert_eq!(
        (
            spread.races,
            spread.first,
            spread.podium,
            spread.near_miss,
            spread.top_ten,
            spread.outside
        ),
        (4, 1, 2, 0, 1, 0)
    );
    // Campanha do campeonato: a equipe do dossiê contra o campo, rodada a
    // rodada. Só as duas equipes com resultado viram linha, e o acumulado soma
    // os DOIS carros de cada uma — 25+12 na primeira, 18+10 na segunda...
    let run = dossier
        .championship_run
        .as_ref()
        .expect("campanha do campeonato");
    assert_eq!(run.rounds.len(), 4);
    assert_eq!(run.lines.len(), 2);
    let minha = run
        .lines
        .iter()
        .find(|line| line.selected)
        .expect("linha da equipe do dossiê");
    assert_eq!(minha.points, vec![37.0, 65.0, 71.0, 94.0]);
    assert_eq!(minha.total, "94");
    // 94 a 93: a ordenação é pela pontuação final, e é ela que dá a colocação.
    assert_eq!(minha.position, 1);
    let rival_line = run
        .lines
        .iter()
        .find(|line| !line.selected)
        .expect("linha do rival");
    assert_eq!(rival_line.points, vec![18.0, 43.0, 68.0, 93.0]);
    assert_eq!(rival_line.position, 2);
    // O nome vem da tabela de equipes: linha sem nome não dá nem para
    // identificar no tooltip do campo cinza.
    assert!(!rival_line.team.is_empty());

    // Tabela de recordes: o destino dos cards. O contrato é que ela e o card
    // saiam do MESMO agregado — o dossiê diz que a equipe é a "2ª" em vitórias
    // num recorte de 2, e a tabela ordenada por vitórias tem de pôr ela em
    // segundo. Duas contagens separadas divergiriam no primeiro empate.
    let ranking = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "mazda_rookie",
        "group",
        None,
    )
    .expect("ranking");
    assert_eq!(ranking.scope, "Grupo Mazda");
    assert_eq!(ranking.rows.len(), 2);
    let minha = ranking
        .rows
        .iter()
        .find(|row| row.team_id == selected.id)
        .expect("linha da equipe");
    assert_eq!(
        (
            minha.wins,
            minha.podiums,
            minha.races,
            minha.win_rate,
            minha.podium_rate
        ),
        (1, 3, 4, 25, 75)
    );
    assert!(!minha.team.is_empty());
    let mut por_vitorias: Vec<&TeamRecordsRow> = ranking.rows.iter().collect();
    por_vitorias.sort_by(|a, b| b.wins.cmp(&a.wins));
    assert_eq!(por_vitorias[1].team_id, selected.id);
    // As três amplitudes são três perguntas, e cada uma muda o recorte de fato —
    // não só o rótulo. Só a categoria: apenas a Mazda Rookie.
    assert_eq!(ranking.scope_kind, "group");
    let so_categoria = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "mazda_rookie",
        "category",
        None,
    )
    .expect("categoria");
    assert_eq!(so_categoria.scope, "Mazda Rookie");
    assert_eq!(so_categoria.scope_kind, "category");
    // A promessa que sustenta a tela: a contagem é do RECORTE, não da carreira.
    // Estas 4 corridas são todas de mazda_rookie, então categoria e grupo dão o
    // mesmo número aqui; o que o teste trava é que a conta sai dos fatos do
    // recorte, e não de um total guardado em outro lugar.
    let minha_categoria = so_categoria
        .rows
        .iter()
        .find(|row| row.team_id == selected.id)
        .expect("linha na categoria");
    assert_eq!(minha_categoria.races, 4);
    // O período vem dos mesmos fatos: os anos das corridas que foram contadas.
    assert!(!minha_categoria.first_year.is_empty());
    assert_eq!(minha_categoria.first_year, minha_categoria.last_year);
    // O par recorte/carreira: aqui as 4 corridas são as únicas do save, então os
    // dois números coincidem e a tela não desenha o segundo. O que o teste trava
    // é que o total existe e é uma conta À PARTE — sem ele, o recorte agiria em
    // silêncio e um "5" solto se pareceria com uma equipe que mal correu.
    assert_eq!(minha_categoria.total_races, minha_categoria.races);
    assert_eq!(minha_categoria.total_wins, minha_categoria.wins);
    // Pedir uma categoria em que a equipe nunca correu não devolve a carreira
    // dela em outro lugar — devolve nada. É o caso que mais importa: era ele que
    // fazia uma equipe da Production aparecer com as vitórias que fez na Mazda.
    let outra_escada =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "gt3", "category", None)
            .expect("gt3");
    assert!(outra_escada
        .rows
        .iter()
        .all(|row| row.team_id != selected.id));
    // O grupo junta Rookie e Championship, e é por isso que o rótulo tem de dizer
    // "Grupo": foi tratá-lo como categoria que fez os títulos da Championship
    // aparecerem debaixo de um filtro escrito "Mazda Rookie".
    assert_ne!(so_categoria.scope, ranking.scope);
    assert_eq!(
        so_categoria.scope_categories,
        vec!["Mazda Rookie".to_string()]
    );
    // O Grupo Mazda vai até a Production, que é onde a escada da marca termina —
    // a equipe sobe sem trocar de mundo, o carro continua sendo o mesmo.
    assert_eq!(
        ranking.scope_categories,
        vec![
            "Mazda Rookie".to_string(),
            "Mazda Championship".to_string(),
            "Production".to_string()
        ]
    );
    // Mas só a classe da marca: Toyota e BMW correm a MESMA categoria em
    // campeonatos separados, e nunca dividiram a pista com uma Mazda.
    assert_eq!(ranking.scope_family, "mazda");
    let grupo_toyota = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "toyota_rookie",
        "group",
        None,
    )
    .expect("grupo toyota");
    assert_eq!(grupo_toyota.scope_family, "toyota");
    // A Production não tem marca própria — é o ponto onde as três escadas
    // convergem —, então o grupo dela segue sendo a convergência inteira, sem
    // recorte de classe.
    let grupo_production = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "production_challenger",
        "group",
        None,
    )
    .expect("grupo production");
    assert_eq!(grupo_production.scope, "Grupo Production");
    assert_eq!(grupo_production.scope_categories.len(), 6);
    assert_eq!(grupo_production.scope_family, "");
    // O mundo ignora a categoria pedida: a mesma resposta venha de onde vier.
    let mundo = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "mazda_rookie",
        "world",
        None,
    )
    .expect("mundo");
    let mundo_pela_gt3 =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "gt3", "world", None)
            .expect("mundo gt3");
    assert_eq!(mundo.scope_kind, "world");
    assert_eq!(mundo.rows.len(), mundo_pela_gt3.rows.len());
    // Na amplitude mundial recorte e carreira são a mesma conta por definição, e
    // é por isso que a tela não desenha o segundo número lá.
    let minha_no_mundo = mundo
        .rows
        .iter()
        .find(|row| row.team_id == selected.id)
        .expect("linha no mundo");
    assert_eq!(minha_no_mundo.total_races, minha_no_mundo.races);
    // Amplitude desconhecida cai em grupo, que é a porta por onde a tela abre.
    let padrao =
        get_team_records_ranking_in_base_dir(&base_dir, "career_001", "mazda_rookie", "xpto", None)
            .expect("padrão");
    assert_eq!(padrao.scope_kind, "group");
    // A escada sai do backend porque é regra de domínio, e traz o grupo de cada
    // categoria junto — é o que deixa a tela dizer o que "grupo" significa AQUI
    // antes de o jogador escolher.
    // 14, e não 10: as duas multiclasse (Production e Endurance) abrem em três
    // entradas cada, uma por carro.
    assert_eq!(ranking.categories.len(), 14);
    let rookie = ranking
        .categories
        .iter()
        .find(|item| item.id == "mazda_rookie")
        .expect("mazda rookie na escada");
    assert_eq!(
        (rookie.label.as_str(), rookie.group_label.as_str()),
        ("Mazda Rookie", "Grupo Mazda")
    );
    let championship = ranking
        .categories
        .iter()
        .find(|item| item.id == "mazda_amador")
        .expect("mazda championship na escada");
    assert_eq!(championship.label, "Mazda Championship");

    assert_eq!(dossier.identity.origin, "Mazda Rookie");
    assert_eq!(dossier.identity.current, "Mazda Rookie");
    // 4 corridas com 1 vitória e 3 pódios é "Vencedora", não "Dominante": o topo
    // por TAXA exige amostra (ver `real_team_profile`), senão o card contradizia o
    // cabeçalho anunciando domínio para quem tem zero título e meia temporada.
    assert_eq!(dossier.identity.profile, "Vencedora");
    assert_eq!(dossier.identity.profile_races, 4);
    assert_eq!(dossier.identity.profile_wins, 1);
    assert_eq!(dossier.identity.profile_podiums, 3);
    assert_eq!(dossier.identity.rival.name, rival.nome);
    assert_eq!(dossier.identity.rival.current_category, "Mazda Rookie");
    assert!(dossier.identity.rival.note.contains("4 disputas diretas"));
    // Sem rivalidade registrada pelo motor, o rival vem da heurística de confronto
    // compartilhado — e o card precisa dizer isso em vez de vender uma origem.
    assert!(dossier.identity.rival.origin_kind.is_none());
    assert!(dossier.identity.rival.perceived_intensity.is_none());
    // Retrospecto: 1º×2º, 2º×1º, 8º×1º e 3º×1º — uma a favor, três contra. É a
    // informação que uma rivalidade existe para dar, e o card não tinha.
    assert_eq!(
        (
            dossier.identity.rival.head_to_head_wins,
            dossier.identity.rival.head_to_head_losses
        ),
        (1, 3)
    );
    let ultimo = dossier
        .identity
        .rival
        .last_meeting
        .as_ref()
        .expect("último encontro");
    assert_eq!(
        (ultimo.round, ultimo.position, ultimo.rival_position),
        (4, 3, 1)
    );
    // A quarta rodada É a corrida mais recente do recorte, então o encontro é
    // "agora" — zero semanas de distância do relógio do mundo.
    assert_eq!(ultimo.weeks_ago, 0);
    assert_eq!(
        dossier.identity.symbol_driver,
        driver_name(&db_path, &selected_driver_1)
    );
    assert!(dossier
        .identity
        .symbol_driver_detail
        .contains("4 corridas, 1 vitória, 3 pódios"));
    // Os mesmos números soltos: a tela desenha cada um como métrica, e prosa não
    // dá para alinhar em coluna com os cards vizinhos.
    assert_eq!(
        (
            dossier.identity.symbol_driver_races,
            dossier.identity.symbol_driver_wins,
            dossier.identity.symbol_driver_podiums
        ),
        (4, 1, 3)
    );
    // O símbolo agora tem biografia: em que anos correu pela equipe e se ficou.
    // Sem isso o card não distinguia o cara que construiu a equipe do que ganhou
    // duas e foi embora.
    assert!(!dossier.identity.symbol_driver_years.is_empty());
    assert!(dossier.identity.symbol_driver_active);
    // Quatro corridas em quatro circuitos distintos não formam fetiche nem
    // carrasco: a leitura exige pista repetida, senão elege a sorte de um domingo.
    assert!(dossier.identity.best_track.is_none());
    assert!(dossier.identity.worst_track.is_none());
    assert_eq!(dossier.management.peak_cash, "$4,200,000");
    assert_eq!(dossier.management.worst_crisis, "$1,250,000 de dívida");
    assert_eq!(dossier.management.healthy_years, "0 Temporadas");
    assert_eq!(dossier.management.operation_health, "Pressionada");
    assert!(dossier.management.efficiency.contains("pts/temporada"));
    assert!(dossier
        .management
        .efficiency_detail
        .contains("média esportiva"));
    assert_eq!(
        dossier.management.biggest_investment,
        "Nível 7 - pacote técnico atual"
    );
    assert!(dossier.management.summary.contains("Pressionada"));

    // Galeria por vaga: os dois titulares da temporada em curso, um em cada
    // coluna, com os números da PASSAGEM e não da carreira. Ambos seguem na
    // equipe, então ambos são vigentes — o que não pode acontecer é a mesma vaga
    // ter duas passagens marcadas como atuais.
    let lineup = &dossier.lineup;
    assert_eq!(lineup.len(), 2);
    let vaga = |slot: i32| {
        lineup
            .iter()
            .find(|term| term.slot == slot)
            .unwrap_or_else(|| panic!("vaga {slot}"))
    };
    assert_eq!(vaga(1).driver_id, selected_driver_1);
    assert_eq!(vaga(2).driver_id, selected_driver_2);
    assert_eq!((vaga(1).races, vaga(1).wins, vaga(1).podiums), (4, 1, 3));
    // 4º, 5º, 9º e 6º: nenhum pódio, e o melhor resultado é o que sobra de
    // concreto para separar quem chegou perto de quem nunca ameaçou.
    assert_eq!((vaga(2).races, vaga(2).wins, vaga(2).podiums), (4, 0, 0));
    assert_eq!(vaga(2).best_position, 4);
    assert!(vaga(1).still_here && vaga(2).still_here);
    for slot in [1, 2] {
        assert_eq!(
            lineup
                .iter()
                .filter(|term| term.slot == slot && term.still_here)
                .count(),
            1,
            "vaga {slot} não pode ter dois titulares atuais"
        );
    }

    // Confiabilidade: sem `dnf` marcado, as quatro largadas de cada carro viraram
    // chegada — e a taxa do grupo sai da mesma conta para todo mundo.
    assert_eq!(
        (
            dossier.reliability.races,
            dossier.reliability.finished,
            dossier.reliability.finish_rate,
            dossier.reliability.mechanical,
            dossier.reliability.driver_error,
            dossier.reliability.other
        ),
        (8, 8, 100, 0, 0, 0)
    );
    assert_eq!(dossier.reliability.group_finish_rate, 100);

    let _ = fs::remove_dir_all(base_dir);
}

/// Pista-fetiche e pista-carrasco: a equipe volta ao mesmo circuito e a média de
/// colocação separa onde ela anda de onde ela apanha. A média usa a MELHOR
/// colocação da equipe em cada corrida — o segundo carro não pode arrastar a
/// leitura para baixo em toda pista.
#[test]
#[serial_test::serial]
fn test_team_dossier_reads_track_affinity_from_repeated_circuits() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_history_dossier_track_affinity");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");
    let selected = teams.first().expect("selected team");
    let (driver_1, driver_2) = team_driver_ids(&db.conn, &selected.id).expect("selected drivers");
    let race_ids: Vec<String> = db
        .conn
        .prepare(
            "SELECT id FROM calendar
             WHERE categoria = 'mazda_rookie'
             ORDER BY rodada ASC
             LIMIT 4",
        )
        .expect("prepare races")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query races")
        .collect::<Result<Vec<_>, _>>()
        .expect("race ids");

    // Duas visitas a cada circuito: sem repetição não há afinidade a ler.
    for (race_id, track) in [
        (&race_ids[0], "Pista Alfa"),
        (&race_ids[3], "Pista Alfa"),
        (&race_ids[1], "Pista Beta"),
        (&race_ids[2], "Pista Beta"),
    ] {
        db.conn
            .execute(
                "UPDATE calendar SET track_name = ?1 WHERE id = ?2",
                rusqlite::params![track, race_id],
            )
            .expect("update track name");
    }

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("clear race results");
    // Alfa: melhores 1º e 3º (média 2.0). Beta: melhores 2º e 8º (média 5.0).
    // O carro 2 sempre atrás — se ele entrasse na média, as duas pistas ficariam
    // piores e a ordem entre elas poderia inverter.
    for (race_id, driver_id, finish, points) in [
        (&race_ids[0], &driver_1, 1, 25.0),
        (&race_ids[0], &driver_2, 12, 0.0),
        (&race_ids[3], &driver_1, 3, 15.0),
        (&race_ids[3], &driver_2, 14, 0.0),
        (&race_ids[1], &driver_1, 2, 18.0),
        (&race_ids[1], &driver_2, 15, 0.0),
        (&race_ids[2], &driver_1, 8, 4.0),
        (&race_ids[2], &driver_2, 16, 0.0),
    ] {
        db.conn
            .execute(
                "INSERT INTO race_results (
                    race_id, piloto_id, equipe_id, posicao_final, pontos
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![race_id, driver_id, &selected.id, finish, points],
            )
            .expect("insert race result");
    }
    drop(db);

    let dossier =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &selected.id, "mazda_rookie")
            .expect("dossier");

    let fetiche = dossier.identity.best_track.expect("pista-fetiche");
    assert_eq!(fetiche.track, "Pista Alfa");
    assert_eq!((fetiche.races, fetiche.best_position), (2, 1));
    assert!((fetiche.average_position - 2.0).abs() < 1e-9);

    let carrasco = dossier.identity.worst_track.expect("pista-carrasco");
    assert_eq!(carrasco.track, "Pista Beta");
    assert_eq!((carrasco.races, carrasco.best_position), (2, 2));
    assert!((carrasco.average_position - 5.0).abs() < 1e-9);

    let _ = fs::remove_dir_all(base_dir);
}

/// DNA de recrutamento: a experiência é contada em ANOS DE CARREIRA na chegada
/// (primeiro ano na equipe menos o ano de início de carreira), não em idade — que
/// dependeria do ano corrente e erraria um ano a cada temporada.
#[test]
#[serial_test::serial]
fn test_team_dossier_reads_recruitment_dna_from_career_start_years() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_history_dossier_recruitment");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");
    let selected = teams.first().expect("selected team");
    let field_team = teams.get(1).expect("equipe do grid");
    let race_id: String = db
        .conn
        .query_row(
            "SELECT id FROM calendar WHERE categoria = 'mazda_rookie' ORDER BY rodada ASC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("race id");
    let season_year: i32 = db
        .conn
        .query_row(
            "SELECT s.ano FROM seasons s
               JOIN calendar c ON c.temporada_id = s.id
              WHERE c.id = ?1",
            rusqlite::params![&race_id],
            |row| row.get(0),
        )
        .expect("season year");
    let driver_ids: Vec<String> = db
        .conn
        .prepare("SELECT id FROM drivers ORDER BY id ASC LIMIT 6")
        .expect("prepare drivers")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query drivers")
        .collect::<Result<Vec<_>, _>>()
        .expect("driver ids");

    // Equipe do dossiê: dois estreando na temporada e um com oito anos de estrada
    // (67% de estreantes). Equipe do grid: três veteranos (0%). A distância é o
    // que faz o rótulo — o mesmo 67% num grid que também forma seria "Mista".
    for (driver_id, career_start) in [
        (&driver_ids[0], season_year),
        (&driver_ids[1], season_year),
        (&driver_ids[2], season_year - 8),
        (&driver_ids[3], season_year - 6),
        (&driver_ids[4], season_year - 7),
        (&driver_ids[5], season_year - 9),
    ] {
        db.conn
            .execute(
                "UPDATE drivers SET ano_inicio_carreira = ?1 WHERE id = ?2",
                rusqlite::params![career_start, driver_id],
            )
            .expect("update career start");
    }

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("clear race results");
    for (driver_id, team_id) in [
        (&driver_ids[0], &selected.id),
        (&driver_ids[1], &selected.id),
        (&driver_ids[2], &selected.id),
        (&driver_ids[3], &field_team.id),
        (&driver_ids[4], &field_team.id),
        (&driver_ids[5], &field_team.id),
    ] {
        db.conn
            .execute(
                "INSERT INTO race_results (
                    race_id, piloto_id, equipe_id, posicao_final, pontos
                ) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![&race_id, driver_id, team_id, 5, 10.0],
            )
            .expect("insert race result");
    }
    drop(db);

    let dossier =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &selected.id, "mazda_rookie")
            .expect("dossier");

    let dna = dossier.identity.recruitment.expect("DNA de recrutamento");
    assert_eq!(dna.profile, "Escola");
    assert_eq!((dna.drivers, dna.rookies), (3, 2));
    assert!((dna.average_experience - 8.0 / 3.0).abs() < 1e-9);
    assert!((dna.rookie_share - 200.0 / 3.0).abs() < 1e-9);
    assert!(dna.field_rookie_share.abs() < 1e-9);

    let _ = fs::remove_dir_all(base_dir);
}

/// Livro-caixa da aba Gestão: pico, fundo do poço e temporadas no azul saem de
/// `team_finance_history`, não do saldo de HOJE.
///
/// A regressão que este teste tranca: os cards liam `teams.cash_balance` e
/// `teams.debt_balance` e chamavam de "maior saldo histórico" e "pior crise". Uma
/// equipe que quebrou e se recuperou aparecia sem crise nenhuma — como a fixture
/// aqui, que fecha com caixa cheio e zero dívida depois de ter devido $1,5M.
#[test]
#[serial_test::serial]
fn test_team_dossier_reads_management_ledger_from_finance_history() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_history_dossier_ledger");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");
    let selected = teams.first().expect("selected team");

    // Temporada 0 é BACKSTORY: uma linha só de encerramento, com prêmio de
    // construtores e nenhuma das outras nove colunas — exatamente o que o sorteio
    // histórico grava para as ~26 temporadas anteriores à carreira. Ela fica FORA de
    // todo o livro-caixa: repartição, pico, fundo do poço, temporadas no azul e
    // curva. Receita sem despesa não é economia, é o resíduo de um sorteio que não
    // simula economia — e o caixa que ela acumula o início da carreira apaga.
    //
    // Temporadas 1 e 2 são jogadas: rodada a rodada, com as linhas de verdade. A 1
    // fecha DEVENDO, a 2 fecha no azul e no pico.
    for (season, round, cash, debt, prize, sponsorship, salary) in [
        (0, SEASON_CLOSE_ROUND, 120_000.0, 0.0, 5_000_000.0, 0.0, 0.0),
        (1, 1, 300_000.0, 0.0, 0.0, 400_000.0, 250_000.0),
        (1, 2, 0.0, 1_500_000.0, 200_000.0, 380_000.0, 250_000.0),
        (2, 1, 2_400_000.0, 0.0, 0.0, 900_000.0, 300_000.0),
        (2, 2, 6_100_000.0, 0.0, 700_000.0, 950_000.0, 300_000.0),
    ] {
        db.conn
            .execute(
                "INSERT INTO team_finance_history (
                    team_id, season_number, round, category,
                    sponsorship_income, constructor_prize_income, salary_expense,
                    income_total, expenses_total, net, cash_balance, debt_balance
                 ) VALUES (?1, ?2, ?3, 'mazda_rookie', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    &selected.id,
                    season,
                    round,
                    sponsorship,
                    prize,
                    salary,
                    sponsorship + prize,
                    salary,
                    sponsorship + prize - salary,
                    cash,
                    debt
                ],
            )
            .expect("insert finance history");
    }
    drop(db);

    let dossier =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &selected.id, "mazda_rookie")
            .expect("dossier");
    let ledger = dossier.management.ledger.expect("livro-caixa");

    // A janela é uma só: as temporadas com rodada de corrida gravada. A 0 não entra
    // em lugar nenhum.
    assert_eq!((ledger.seasons, ledger.rounds), (2, 4));
    assert_eq!((ledger.first_season, ledger.last_season), (1, 2));
    assert_eq!(ledger.flow_seasons, 2);
    assert_eq!((ledger.flow_first_season, ledger.flow_last_season), (1, 2));
    assert_eq!(
        (
            ledger.peak_cash,
            ledger.peak_cash_season,
            ledger.peak_cash_round
        ),
        (6_100_000.0, 2, 2)
    );
    assert_eq!(
        (ledger.worst_debt, ledger.worst_debt_season),
        (1_500_000.0, 1)
    );
    // Só a 2 fecha sem dívida: a 1 termina devendo e a 0 nem é contada. A regra
    // antiga devolvia 0 ou "todas", nunca o meio-termo, que é o caso normal — e
    // ainda somava os ~26 anos de backstory, que fecham sem dívida por construção.
    assert_eq!(ledger.healthy_seasons, 1);

    // A repartição soma só as temporadas medidas e ordena da maior para a menor.
    // Linha zerada (bilheteria, auxílios) não entra: um "$0" só ocupa espaço. E o
    // prêmio de $5M da temporada de backstory fica DE FORA — se entrasse, ele
    // sozinho seria a maior linha de receita da equipe.
    let patrocinio = &ledger.income_lines[0];
    assert_eq!(patrocinio.id, "sponsorship_income");
    assert!((patrocinio.value - 2_630_000.0).abs() < 1e-6);
    assert_eq!(ledger.income_lines[1].id, "constructor_prize_income");
    assert!((ledger.income_lines[1].value - 900_000.0).abs() < 1e-6);
    assert_eq!(ledger.income_lines.len(), 2);
    assert_eq!(ledger.expense_lines.len(), 1);
    assert_eq!(ledger.expense_lines[0].id, "salary_expense");
    assert!((ledger.expense_lines[0].value - 1_100_000.0).abs() < 1e-6);

    // A curva cobre as rodadas medidas e marca os encerramentos. A linha de
    // backstory fica fora: com ela o traço juntava duas leis financeiras diferentes
    // — anos de receita pura e anos de operação real — no mesmo desenho.
    assert_eq!(ledger.cash_curve.len(), 4);
    assert!(ledger
        .cash_curve
        .iter()
        .all(|point| point.season_number > 0));
    assert_eq!(
        ledger
            .cash_curve
            .iter()
            .filter(|point| point.is_season_close)
            .count(),
        2
    );

    // E a prosa dos cards passa a citar a temporada do superlativo.
    assert!(dossier.management.peak_cash.contains("6,100,000"));
    assert!(dossier.management.worst_crisis.contains("1,500,000"));
    assert_eq!(dossier.management.healthy_years, "1 Temporada");
    assert!(dossier
        .management
        .healthy_years_detail
        .contains("1 de 2 temporadas"));
    assert!(dossier
        .management
        .worst_crisis_detail
        .contains("Fundo do poço na temporada 1"));

    let _ = fs::remove_dir_all(base_dir);
}

/// Carreira recém-criada: só backstory no histórico financeiro. O livro-caixa existe
/// (para a aba EXPLICAR que não há temporada jogada), mas a janela é zerada e os
/// cards caem no retrato atual da equipe.
///
/// A regressão que este teste tranca é a que o jogador viu na tela: ~26 anos de
/// prêmio de construtores sem despesa nenhuma desenhavam uma rampa monotônica até
/// dezenas de milhões, o card anunciava aquilo como "maior saldo histórico", e o
/// penhasco final não era uma quebra — era o reset de caixa do início da carreira.
#[test]
#[serial_test::serial]
fn test_team_dossier_ledger_ignores_backstory_only_history() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_history_dossier_backstory");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("team standings");
    let selected = teams.first().expect("selected team");

    // Três anos de backstory, caixa subindo só de prêmio — a forma que o sorteio
    // histórico grava.
    for (season, cash, prize) in [
        (0, 30_000_000.0, 5_000_000.0),
        (1, 60_000_000.0, 5_000_000.0),
        (2, 90_000_000.0, 5_000_000.0),
    ] {
        db.conn
            .execute(
                "INSERT INTO team_finance_history (
                    team_id, season_number, round, category,
                    constructor_prize_income, income_total, expenses_total, net,
                    cash_balance, debt_balance
                 ) VALUES (?1, ?2, ?3, 'mazda_rookie', ?4, ?4, 0, ?4, ?5, 0)",
                rusqlite::params![&selected.id, season, SEASON_CLOSE_ROUND, prize, cash],
            )
            .expect("insert finance history");
    }
    drop(db);

    let dossier =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &selected.id, "mazda_rookie")
            .expect("dossier");
    let ledger = dossier.management.ledger.expect("livro-caixa");

    assert_eq!((ledger.seasons, ledger.rounds), (0, 0));
    assert_eq!(ledger.flow_seasons, 0);
    assert!(ledger.cash_curve.is_empty());
    assert!(ledger.income_lines.is_empty());
    assert!(ledger.flow_note.contains("Nenhuma temporada jogada"));
    assert_eq!((ledger.peak_cash, ledger.worst_debt), (0.0, 0.0));
    assert_eq!(ledger.healthy_seasons, 0);

    // Nenhum superlativo inventado: o card mostra o caixa de HOJE e diz que é o
    // retrato atual, não uma série.
    assert!(!dossier.management.peak_cash.contains("90,000,000"));
    assert!(dossier
        .management
        .peak_cash_detail
        .contains("retrato atual da equipe"));
    assert_eq!(dossier.management.healthy_years, "0 Temporadas");

    let _ = fs::remove_dir_all(base_dir);
}

/// A escada da marca vai até a Production, mas a Production é multiclasse: três
/// marcas disputam a MESMA categoria em campeonatos separados. O Grupo Mazda
/// conta a Production da classe Mazda e ignora a de Toyota — elas nunca
/// dividiram a pista.
#[test]
#[serial_test::serial]
fn test_team_records_group_counts_only_the_family_class_in_production() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_records_family_class");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let corrida_production: Option<String> = db
        .conn
        .query_row(
            "SELECT id FROM calendar WHERE categoria = 'production_challenger' ORDER BY rodada LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .expect("consulta calendário");
    let Some(corrida_production) = corrida_production else {
        // Save de teste sem Production no calendário: não há o que separar.
        return;
    };

    let equipes: Vec<String> = db
        .conn
        .prepare("SELECT id FROM teams ORDER BY id LIMIT 2")
        .expect("prepare")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("equipes");
    let (mazda, toyota) = (&equipes[0], &equipes[1]);
    let (piloto_mazda, _) = team_driver_ids(&db.conn, mazda).expect("piloto mazda");
    let (piloto_toyota, _) = team_driver_ids(&db.conn, toyota).expect("piloto toyota");

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("limpa resultados");
    for (team_id, classe) in [(mazda, "mazda"), (toyota, "toyota")] {
        db.conn
            .execute(
                "UPDATE teams SET classe = ?1 WHERE id = ?2",
                rusqlite::params![classe, team_id],
            )
            .expect("classe da equipe");
    }
    // Uma vitória na Production para cada uma, na mesma corrida.
    for (piloto, equipe) in [(&piloto_mazda, mazda), (&piloto_toyota, toyota)] {
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, 1, 25.0)",
                rusqlite::params![&corrida_production, piloto, equipe],
            )
            .expect("resultado production");
    }
    drop(db);

    let grupo_mazda = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "mazda_rookie",
        "group",
        None,
    )
    .expect("grupo mazda");
    let linha = |ranking: &crate::commands::career_types::TeamRecordsRanking, id: &str| {
        ranking
            .rows
            .iter()
            .find(|row| row.team_id == id)
            .map(|row| row.races)
            .unwrap_or(0)
    };
    // A corrida da equipe Mazda entra; a da Toyota, não — mesma categoria,
    // campeonato outro.
    assert_eq!(linha(&grupo_mazda, mazda), 1);
    assert_eq!(linha(&grupo_mazda, toyota), 0);

    // Espelho: no Grupo Toyota a conta se inverte.
    let grupo_toyota = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "toyota_rookie",
        "group",
        None,
    )
    .expect("grupo toyota");
    assert_eq!(linha(&grupo_toyota, toyota), 1);
    assert_eq!(linha(&grupo_toyota, mazda), 0);

    // A escada abre as multiclasse por carro: a Production não é um campeonato,
    // são três correndo na mesma pista. Escolher "Production" inteira somaria
    // Mazda, Toyota e BMW num número que não existe em classificação nenhuma.
    let producao: Vec<&crate::commands::career_types::TeamRecordsCategory> = grupo_mazda
        .categories
        .iter()
        .filter(|item| item.id == "production_challenger")
        .collect();
    assert_eq!(producao.len(), 3);
    assert_eq!(producao[0].key, "production_challenger:mazda");
    assert_eq!(producao[0].label, "Production · Mazda");
    // Monomarca segue com uma entrada só, e a chave é o próprio id.
    let gt3 = grupo_mazda
        .categories
        .iter()
        .find(|item| item.id == "gt3")
        .expect("gt3 na escada");
    assert_eq!((gt3.key.as_str(), gt3.class.as_str()), ("gt3", ""));

    // E cada campeonato da Production conta só o seu: pedir a classe é pedir um
    // dos três, não a categoria inteira.
    let producao_mazda = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "production_challenger",
        "category",
        Some("mazda"),
    )
    .expect("production mazda");
    assert_eq!(producao_mazda.scope, "Production · Mazda");
    assert_eq!(producao_mazda.scope_family, "mazda");
    assert_eq!(linha(&producao_mazda, mazda), 1);
    assert_eq!(linha(&producao_mazda, toyota), 0);
    let producao_toda = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "production_challenger",
        "category",
        None,
    )
    .expect("production toda");
    assert_eq!(linha(&producao_toda, mazda), 1);
    assert_eq!(linha(&producao_toda, toyota), 1);

    // E o mundo não recorta por marca: as duas corridas existem.
    let mundo = get_team_records_ranking_in_base_dir(
        &base_dir,
        "career_001",
        "mazda_rookie",
        "world",
        None,
    )
    .expect("mundo");
    assert_eq!(linha(&mundo, mazda), 1);
    assert_eq!(linha(&mundo, toyota), 1);
}

/// Os cards de record são da CATEGORIA, não do grupo. Uma equipe com corridas na
/// Mazda Rookie e na Mazda Championship vê, na ficha aberta pela Rookie, só o
/// que fez na Rookie — a média e o "17º de 22" que vinham do grupo somavam um
/// campeonato que ela nem sempre disputou.
#[test]
#[serial_test::serial]
fn test_team_dossier_records_are_scoped_to_the_category_not_the_group() {
    rust_i18n::set_locale("pt-BR");
    let base_dir = create_test_career_dir("team_dossier_records_por_categoria");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    let corrida = |categoria: &str| -> Option<String> {
        db.conn
            .query_row(
                "SELECT id FROM calendar WHERE categoria = ?1 ORDER BY rodada LIMIT 1",
                rusqlite::params![categoria],
                |row| row.get(0),
            )
            .optional()
            .expect("consulta calendário")
    };
    let (Some(na_rookie), Some(na_championship)) =
        (corrida("mazda_rookie"), corrida("mazda_amador"))
    else {
        // Sem as duas categorias no calendário não há grupo para separar.
        return;
    };

    let teams = get_teams_standings_in_base_dir(&base_dir, "career_001", "mazda_rookie")
        .expect("standings");
    let equipe = teams.first().expect("equipe").id.clone();
    let (piloto, _) = team_driver_ids(&db.conn, &equipe).expect("piloto");

    db.conn
        .execute("DELETE FROM race_results", [])
        .expect("limpa resultados");
    // Uma vitória na Rookie e duas na Championship. No grupo seriam 3 vitórias em
    // 3 corridas; na categoria são 1 em 1.
    for (race_id, posicao) in [(&na_rookie, 1), (&na_championship, 1)] {
        db.conn
            .execute(
                "INSERT INTO race_results (race_id, piloto_id, equipe_id, posicao_final, pontos)
                 VALUES (?1, ?2, ?3, ?4, 25.0)",
                rusqlite::params![race_id, &piloto, &equipe, posicao],
            )
            .expect("resultado");
    }
    drop(db);

    let ficha_rookie =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &equipe, "mazda_rookie")
            .expect("ficha rookie");
    assert_eq!(ficha_rookie.record_scope, "Mazda Rookie");
    assert_eq!(ficha_rookie.sport.races, 1);
    assert_eq!(ficha_rookie.sport.wins, 1);

    // A mesma equipe, aberta pela Championship: o card muda de recorte junto.
    let ficha_championship =
        get_team_history_dossier_in_base_dir(&base_dir, "career_001", &equipe, "mazda_amador")
            .expect("ficha championship");
    assert_eq!(ficha_championship.record_scope, "Mazda Championship");
    assert_eq!(ficha_championship.sport.races, 1);

    // Mas a HISTÓRIA continua sendo a do grupo: a fita de forma recente mostra as
    // duas corridas, porque ela conta a trajetória e não a comparação.
    assert_eq!(ficha_rookie.recent_form.len(), 2);
    // E "tem histórico" não depende do recorte dos cards: uma equipe que subiu de
    // tier não tem corrida na categoria de baixo, e o dossiê inteiro cairia em
    // "sem histórico" por causa de um filtro que só vale para os cards.
    assert!(ficha_rookie.has_history);
}
