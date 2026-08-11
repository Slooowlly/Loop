//! Testes de [`crate::commands::career`] — indice, `use` compartilhados e helpers de
//! fixture.
//!
//! Extraidos do bloco `#[cfg(test)]` que ficava no fim de `career.rs` e depois fatiados
//! por area: os 93 testes viviam neste mesmo arquivo, com 5.897 linhas, enquanto a
//! logica ja estava dividida em dez irmaos — rodar o teste de uma area custava o arquivo
//! inteiro. Cada `mod` abaixo espelha um irmao de `career/` e pega os helpers daqui pelo
//! `use super::*;`.

use chrono::{Datelike, NaiveDate};
use std::fs;

use super::*;
use crate::commands::career_team_dossier::{
    get_team_history_dossier_in_base_dir, get_team_records_ranking_in_base_dir,
};
use crate::commands::career_types::TeamRecordsRow;
use crate::db::queries::teams::SEASON_CLOSE_ROUND;

mod briefing;
mod lifecycle;
mod market_window;
mod queries;
mod season_flow;
mod standings;
mod vacancies;

fn driver_name(db_path: &Path, driver_id: &str) -> String {
    let db = Database::open_existing(db_path).expect("db");
    db.conn
        .query_row(
            "SELECT nome FROM drivers WHERE id = ?1",
            rusqlite::params![driver_id],
            |row| row.get::<_, String>(0),
        )
        .expect("driver name")
}

fn team_driver_ids(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<(String, String), rusqlite::Error> {
    conn.query_row(
        "SELECT piloto_1_id, piloto_2_id FROM teams WHERE id = ?1",
        rusqlite::params![team_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
}

fn create_test_career_dir(label: &str) -> std::path::PathBuf {
    let base_dir = unique_test_dir(label);
    fs::create_dir_all(&base_dir).expect("base dir");

    let input = CreateCareerInput {
        player_name: "Joao Silva".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        category: "mazda_rookie".to_string(),
        team_index: 2,
        difficulty: "medio".to_string(),
    };

    let _ = create_career_in_base_dir(&base_dir, input).expect("career should be created");
    base_dir
}

fn mark_all_races_completed(base_dir: &Path, career_id: &str) {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    db.conn
        .execute("UPDATE calendar SET status = 'Concluida'", [])
        .expect("mark all races completed");
    db.conn
        .execute(
            "UPDATE seasons SET fase = 'PosEspecial' WHERE status = 'EmAndamento'",
            [],
        )
        .expect("mark season as post-special");
}

fn mark_regular_races_completed(db: &Database) {
    db.conn
        .execute(
            "UPDATE calendar SET status = 'Concluida' WHERE season_phase = 'BlocoRegular'",
            [],
        )
        .expect("complete regular block");
}

/// Força a temporada ativa e o calendário para o estado legado BlocoRegular.
/// Necessário em testes que exercem o fluxo de convocação legado (BlocoRegular →
/// JanelaConvocacao) em saves criados pelo modelo 9D (fase Temporada).
/// Remove as entradas de production_challenger e endurance do calendário 9D para
/// que iniciar_bloco_especial possa gerá-las no estilo legado BlocoEspecial.
fn force_legacy_blocoregular_state(db: &Database) {
    db.conn
        .execute(
            "UPDATE seasons SET fase = 'BlocoRegular' WHERE status = 'EmAndamento'",
            [],
        )
        .expect("set season to BlocoRegular");
    db.conn
        .execute(
            "DELETE FROM calendar WHERE categoria IN ('production_challenger', 'endurance')",
            [],
        )
        .expect("remove 9D special category entries");
    db.conn
        .execute("UPDATE calendar SET season_phase = 'BlocoRegular'", [])
        .expect("set calendar to BlocoRegular phase");
}

fn insert_test_endurance_team(conn: &rusqlite::Connection) -> Team {
    let mut team = crate::models::team::placeholder_team_from_db(
        "T_TEST_ENDURANCE".to_string(),
        "Endurance Test Team".to_string(),
        "endurance".to_string(),
        crate::common::time::current_timestamp(),
    );
    team.classe = Some("gt4".to_string());
    team_queries::insert_team(conn, &team).expect("insert endurance test team");
    team
}

fn insert_test_production_team(conn: &rusqlite::Connection, class_name: &str) -> Team {
    let mut team = crate::models::team::placeholder_team_from_db(
        format!("T_TEST_PRODUCTION_{}", class_name.to_uppercase()),
        format!("Production {class_name} Test Team"),
        "production_challenger".to_string(),
        crate::common::time::current_timestamp(),
    );
    team.classe = Some(class_name.to_string());
    team_queries::insert_team(conn, &team).expect("insert production test team");
    team
}

fn unique_test_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("iracerapp_{label}_{nanos}"))
}

fn seed_player_proposal(
    conn: &rusqlite::Connection,
    season_id: &str,
    player_id: &str,
    team_id: &str,
    status: &str,
) {
    let team = team_queries::get_team_by_id(conn, team_id)
        .expect("team query")
        .expect("team");
    let player = driver_queries::get_driver(conn, player_id).expect("player");
    crate::db::queries::market_proposals::insert_player_proposal(
        conn,
        season_id,
        &crate::market::proposals::MarketProposal {
            id: format!("MP-{team_id}-{player_id}"),
            equipe_id: team.id.clone(),
            equipe_nome: team.nome.clone(),
            piloto_id: player.id.clone(),
            piloto_nome: player.nome.clone(),
            categoria: team.categoria.clone(),
            papel: crate::models::enums::TeamRole::Numero1,
            salario_oferecido: 95_000.0,
            duracao_anos: 2,
            status: match status {
                "Aceita" => crate::market::proposals::ProposalStatus::Aceita,
                "Recusada" => crate::market::proposals::ProposalStatus::Recusada,
                "Expirada" => crate::market::proposals::ProposalStatus::Expirada,
                _ => crate::market::proposals::ProposalStatus::Pendente,
            },
            motivo_recusa: None,
        },
    )
    .expect("insert player proposal");
}

fn force_complete_preseason_plan(save_dir: &Path) {
    let mut plan = crate::market::preseason::load_preseason_plan(save_dir)
        .expect("load plan")
        .expect("plan");
    plan.state.is_complete = true;
    plan.state.current_week = plan.state.total_weeks + 1;
    plan.state.phase = crate::market::preseason::PreSeasonPhase::Complete;
    plan.state.player_has_pending_proposals = false;
    crate::market::preseason::save_preseason_plan(save_dir, &plan).expect("save plan");
}

fn latest_regular_contract_for_driver(
    conn: &rusqlite::Connection,
    driver_id: &str,
) -> crate::models::contract::Contract {
    contract_queries::get_contracts_for_pilot(conn, driver_id)
        .expect("driver contracts query")
        .into_iter()
        .filter(|contract| contract.tipo == crate::models::enums::ContractType::Regular)
        .max_by(|a, b| {
            a.temporada_inicio
                .cmp(&b.temporada_inicio)
                .then_with(|| a.created_at.cmp(&b.created_at))
        })
        .expect("latest regular contract")
}

// O confronto direto é o que separa "seis nomes que você nunca ouviu" de "aquele
// que te tirou da pista em Interlagos". Duas armadilhas na consulta agregada: o
// índice do parâmetro do jogador vem DEPOIS da lista de ids (e errar isso troca
// silenciosamente quem é quem), e abandono não pode contar como duelo perdido.
