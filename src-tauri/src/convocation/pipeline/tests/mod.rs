//! Suíte de testes do bloco especial (extraída de `convocation/pipeline.rs`).
//!
//! Continua sendo o mesmo conjunto de módulos de teste de antes: `use super::*`
//! enxerga o módulo `pipeline` inteiro, incluindo os itens privados.

use super::*;

#[cfg(test)]
fn setup_world_db() -> (rusqlite::Connection, String) {
    use rand::{rngs::StdRng, SeedableRng};

    let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    crate::db::migrations::run_all(&conn).expect("migrations");

    let mut rng = StdRng::seed_from_u64(99);
    let world = crate::generators::world::generate_world_with_rng(
        "Test Player",
        "🇧🇷 Brasileiro",
        20,
        "mazda_rookie",
        0,
        "medio",
        &mut rng,
    )
    .expect("world generation");

    let season_id = "S001".to_string();
    let season = crate::models::season::Season::new(season_id.clone(), 1, 2024);
    crate::db::queries::seasons::insert_season(&conn, &season).expect("insert season");
    for driver in &world.drivers {
        crate::db::queries::drivers::insert_driver(&conn, driver).expect("insert driver");
    }
    crate::db::queries::teams::insert_teams(&conn, &world.teams).expect("insert teams");
    crate::db::queries::contracts::insert_contracts(&conn, &world.contracts)
        .expect("insert contracts");

    let next_contract = world.contracts.len() + 1;
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'next_contract_id'",
        rusqlite::params![next_contract.to_string()],
    )
    .expect("update meta contract counter");

    (conn, season_id)
}

#[cfg(test)]
fn make_player_eligible_for_specials(conn: &rusqlite::Connection, category: &str) -> String {
    let mut player = crate::db::queries::drivers::get_player_driver(conn).expect("player");
    player.categoria_atual = Some(category.to_string());
    player.atributos.skill = 98.0;
    player.melhor_resultado_temp = Some(1);
    player.stats_temporada.vitorias = 4;
    crate::db::queries::drivers::update_driver(conn, &player).expect("update player");
    player.id
}

/// As fases da convocação, o grid especial e o encerramento do bloco.
mod fases_e_grid;
/// Ofertas de convocação dirigidas ao jogador.
mod ofertas_do_jogador;
/// PosEspecial: o que a virada do bloco especial desfaz.
mod pos_especial;
