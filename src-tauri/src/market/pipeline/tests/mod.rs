//! Suíte de testes do pipeline de mercado (extraída de `pipeline.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: `use super::*` enxerga o
//! módulo `pipeline` inteiro, incluindo os itens privados.
//!
//! Aqui ficam só as SEMENTES compartilhadas (o mundo de testes e os construtores de
//! piloto/equipe/classificação); os casos moram nas fatias por tema abaixo, que
//! enxergam estas sementes pelo mesmo `use super::*`.

/// Assédio: o leilão entre IAs e o que o jogador decide.
mod assedio;
/// Contrato serde dos DTOs que cruzam a ponte para o React.
mod contrato;
/// Escada fechada: promoção, recrutamento profundo e pool de resgate.
mod escada;
/// A passada completa sobre a grade: nenhuma vaga sobra.
mod grade;
/// Dado corrompido, passo que falha no meio, rollback e i18n.
mod integridade;
/// A janela pelo lado do jogador: propostas, assentos reservados, prazos.
mod janela_jogador;
/// A penalidade de motivação por perder a vaga: quem fica sem assento e quem não.
mod perda_de_vaga;
/// O bônus de motivação da renovação (B35): quem renova de verdade e quem não.
mod renovacao;
/// Temporadas de dois dígitos nas colunas TEXT de vigência de contrato.
mod temporada_texto;

use rand::{rngs::StdRng, SeedableRng};
use rusqlite::Connection;

use super::*;

use crate::constants::teams::get_team_templates;
use crate::db::migrations;
use crate::db::queries::seasons as season_queries;
use crate::licensing::driver_has_required_license_for_category;
use crate::models::season::Season;

fn setup_market_fixture() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    migrations::run_all(&conn).expect("schema");

    let previous = Season::new("S001".to_string(), 1, 2024);
    let next = Season::new("S002".to_string(), 2, 2025);
    season_queries::insert_season(&conn, &previous).expect("previous season");
    season_queries::finalize_season(&conn, &previous.id).expect("finalize previous");
    season_queries::insert_season(&conn, &next).expect("next season");

    let mut team_rng = StdRng::seed_from_u64(200);
    let team_a = sample_team("gt4", "T001", &mut team_rng);
    let team_b = sample_team("gt4", "T002", &mut team_rng);
    team_queries::insert_team(&conn, &team_a).expect("team a");
    team_queries::insert_team(&conn, &team_b).expect("team b");

    let driver_a = sample_driver("P001", "Piloto A", Some("gt4"), 78.0, DriverStatus::Ativo);
    let driver_b = sample_driver("P002", "Piloto B", Some("gt4"), 66.0, DriverStatus::Ativo);
    let driver_c = sample_driver(
        "P003",
        "Piloto C",
        Some("gt4"),
        62.0,
        DriverStatus::Aposentado,
    );
    let driver_d = sample_driver("P004", "Piloto D", Some("gt4"), 74.0, DriverStatus::Ativo);
    let driver_e = sample_driver("P005", "Piloto E", None, 59.0, DriverStatus::Ativo);
    let driver_f = sample_driver("P006", "Piloto F", Some("gt3"), 76.0, DriverStatus::Ativo);
    for driver in [
        &driver_a, &driver_b, &driver_c, &driver_d, &driver_e, &driver_f,
    ] {
        driver_queries::insert_driver(&conn, driver).expect("insert driver");
    }

    let contract_a = Contract::new(
        "C001".to_string(),
        driver_a.id.clone(),
        driver_a.nome.clone(),
        team_a.id.clone(),
        team_a.nome.clone(),
        1,
        2,
        140_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    let contract_b = Contract::new(
        "C002".to_string(),
        driver_b.id.clone(),
        driver_b.nome.clone(),
        team_a.id.clone(),
        team_a.nome.clone(),
        1,
        1,
        95_000.0,
        TeamRole::Numero2,
        "gt4".to_string(),
    );
    let contract_c = Contract::new(
        "C003".to_string(),
        driver_c.id.clone(),
        driver_c.nome.clone(),
        team_b.id.clone(),
        team_b.nome.clone(),
        1,
        2,
        85_000.0,
        TeamRole::Numero1,
        "gt4".to_string(),
    );
    contract_queries::insert_contract(&conn, &contract_a).expect("contract a");
    contract_queries::insert_contract(&conn, &contract_b).expect("contract b");
    contract_queries::insert_contract(&conn, &contract_c).expect("contract c");

    team_queries::update_team_pilots(&conn, &team_a.id, Some(&driver_a.id), Some(&driver_b.id))
        .expect("team a pilots");
    team_queries::update_team_pilots(&conn, &team_b.id, Some(&driver_c.id), None)
        .expect("team b pilots");

    insert_standing(
        &conn,
        &previous.id,
        &driver_a.id,
        &team_a.id,
        "gt4",
        1,
        120.0,
        3,
        2,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_b.id,
        &team_a.id,
        "gt4",
        4,
        72.0,
        1,
        1,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_c.id,
        &team_b.id,
        "gt4",
        6,
        40.0,
        0,
        0,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_d.id,
        &team_b.id,
        "gt4",
        2,
        96.0,
        2,
        1,
    );
    insert_standing(
        &conn,
        &previous.id,
        &driver_f.id,
        &team_a.id,
        "gt3",
        3,
        88.0,
        1,
        2,
    );

    conn.execute(
        "UPDATE meta SET value = '4' WHERE key = 'next_contract_id'",
        [],
    )
    .expect("contract counter");
    conn.execute(
        "UPDATE meta SET value = '7' WHERE key = 'next_driver_id'",
        [],
    )
    .expect("driver counter");

    conn
}

fn sample_team(category: &str, id: &str, rng: &mut StdRng) -> crate::models::team::Team {
    let template = get_team_templates(category)[0];
    crate::models::team::Team::from_template_with_rng(template, category, id.to_string(), 2025, rng)
}

// ── Porta de saída da falência ────────────────────────────────────────────────

fn sample_driver(
    id: &str,
    name: &str,
    category: Option<&str>,
    skill: f64,
    status: DriverStatus,
) -> Driver {
    let mut driver = Driver::new(
        id.to_string(),
        name.to_string(),
        "Brasil".to_string(),
        "M".to_string(),
        24,
        2020,
    );
    driver.categoria_atual = category.map(str::to_string);
    driver.status = status;
    driver.atributos.skill = skill;
    driver.atributos.consistencia = 68.0;
    driver.stats_temporada.vitorias = 1;
    driver.stats_temporada.poles = 1;
    driver.stats_carreira.corridas = 40;
    driver.stats_carreira.temporadas = 5;
    driver.stats_carreira.titulos = 1;
    driver
}

fn insert_standing(
    conn: &Connection,
    season_id: &str,
    driver_id: &str,
    team_id: &str,
    category: &str,
    position: i32,
    points: f64,
    wins: i32,
    poles: i32,
) {
    conn.execute(
        "INSERT INTO standings (
            temporada_id, piloto_id, equipe_id, categoria, posicao, pontos, vitorias, podios, poles, corridas
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![season_id, driver_id, team_id, category, position, points, wins, wins + 1, poles, 8],
    )
    .expect("insert standing");
}
