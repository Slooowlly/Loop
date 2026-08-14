//! Testes de `career::vacancies`: vagas, salario de oferta e encaixe do jogador.
//!
//! Fatiado de `tests/mod.rs`, que juntava as dez areas num arquivo so. Os helpers
//! e os `use` continuam no `mod.rs` e chegam aqui pelo glob.

use super::*;

#[test]
fn offer_salary_uses_real_money_instead_of_legacy_budget() {
    let mut team = crate::models::team::placeholder_team_from_db(
        "TGT4".to_string(),
        "GT4 Rich".to_string(),
        "gt4".to_string(),
        "2026-01-01".to_string(),
    );
    team.cash_balance = 6_000_000.0;
    team.debt_balance = 0.0;
    team.financial_state = "healthy".to_string();
    team.budget = 1.0;

    let mut driver = Driver::new(
        "P001".to_string(),
        "Piloto Forte".to_string(),
        "br".to_string(),
        "M".to_string(),
        24,
        2026,
    );
    driver.atributos.skill = 80.0;

    let offer = calculate_offer_salary_for_team(&team, &driver);

    assert!(offer > 100_000.0);
}

/// Mundo recém-gerado tem todos os assentos ocupados, então o painel abre vazio —
/// e é isso que a tela precisa poder dizer sem inventar vaga.
#[test]
fn season_market_board_is_empty_when_the_world_has_no_open_seat() {
    let base_dir = create_test_career_dir("market_board_empty");
    let career_id = "career_001";

    let board = get_season_market_board_in_base_dir(&base_dir, career_id).expect("market board");

    assert_eq!(board.player_categoria.as_deref(), Some("mazda_rookie"));
    assert!(board.vagas.is_empty(), "mundo cheio não tem assento aberto");
    assert_eq!(board.vagas_elegiveis, 0);
}

/// Um assento aberto na PRÓPRIA categoria do jogador é elegível e vem com salário
/// estimado; um assento na categoria mais alta da escada não é, e vem sem salário —
/// a tela mostra a cadeira e diz que ela ainda não é dele.
#[test]
fn season_market_board_marks_eligibility_by_tier_and_license() {
    let base_dir = create_test_career_dir("market_board_eligibility");
    let career_id = "career_001";
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");

    // Esvazia um assento na categoria do jogador e outro no topo da escada. O
    // `UPDATE` direto é de propósito: o que está sob teste é a LEITURA das vagas,
    // e o caminho de produção que abre assento (aposentadoria, rescisão) traria
    // metade do pipeline de mercado para dentro do teste.
    let equipe_da_faixa: String = db
        .conn
        .query_row(
            "SELECT id FROM teams WHERE categoria = 'mazda_rookie' AND piloto_2_id IS NOT NULL LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("equipe da categoria do jogador");
    let equipe_do_topo: String = db
        .conn
        .query_row(
            "SELECT id FROM teams WHERE categoria = 'gt3' AND piloto_2_id IS NOT NULL LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("equipe de gt3");
    db.conn
        .execute(
            "UPDATE teams SET piloto_2_id = NULL WHERE id IN (?1, ?2)",
            rusqlite::params![equipe_da_faixa, equipe_do_topo],
        )
        .expect("abrir dois assentos");
    drop(db);

    let board = get_season_market_board_in_base_dir(&base_dir, career_id).expect("market board");

    let na_faixa = board
        .vagas
        .iter()
        .find(|vaga| vaga.team_id == equipe_da_faixa)
        .expect("a vaga da categoria do jogador aparece");
    assert!(na_faixa.tier_ok, "mesma categoria está na faixa");
    assert!(na_faixa.licenca_ok, "o jogador corre na própria divisão");
    assert!(
        na_faixa.salario_estimado.unwrap_or(0.0) > 0.0,
        "vaga elegível estima salário"
    );
    assert_eq!(na_faixa.papel, "Numero2");

    let no_topo = board
        .vagas
        .iter()
        .find(|vaga| vaga.team_id == equipe_do_topo)
        .expect("a vaga do topo da escada aparece");
    assert!(!no_topo.tier_ok, "gt3 está muito acima do rookie");
    assert!(
        no_topo.salario_estimado.is_none(),
        "vaga inelegível não inventa oferta"
    );

    assert_eq!(board.vagas_elegiveis, 1);
    // Elegível primeiro: a ordem é parte do contrato que a tela consome.
    assert_eq!(
        board.vagas.first().map(|vaga| vaga.team_id.as_str()),
        Some(equipe_da_faixa.as_str())
    );
}
