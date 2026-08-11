//! O Nemesis ATUAL do jogador — a memória que dá HISTERESE à seleção.
//!
//! Sem isso, o Nemesis seria só o par de maior intensidade a cada leitura e trocaria
//! toda semana quando dois rivais ficassem parelhos no topo. Guardamos quem reina; a
//! seleção só destitui o reinante quando outro o supera por uma margem. Uma linha só
//! (singleton). A tabela nasceu fora das migrações e entrou nelas na v62; o
//! `ensure_table` reaplica o MESMO DDL, de forma idempotente, para conexões de teste
//! in-memory que não migram.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::DbError;

/// DDL da tabela, num lugar só — a migração v62 executa esta MESMA constante.
pub(crate) const DDL_PLAYER_NEMESIS: &str = "
    CREATE TABLE IF NOT EXISTS player_nemesis (
        id       INTEGER PRIMARY KEY CHECK (id = 1),
        rival_id TEXT
    );
";

fn ensure_table(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(DDL_PLAYER_NEMESIS)?;
    Ok(())
}

/// driver_id do Nemesis atual, ou `None` se ainda não há (ou foi limpo).
pub fn get_current_nemesis(conn: &Connection) -> Result<Option<String>, DbError> {
    ensure_table(conn)?;
    let id = conn
        .query_row(
            "SELECT rival_id FROM player_nemesis WHERE id = 1",
            [],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    Ok(id)
}

/// Define (ou limpa, com `None`) o Nemesis atual. Upsert na linha singleton.
pub fn set_current_nemesis(conn: &Connection, rival_id: Option<&str>) -> Result<(), DbError> {
    ensure_table(conn)?;
    conn.execute(
        "INSERT INTO player_nemesis (id, rival_id) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET rival_id = excluded.rival_id",
        params![rival_id],
    )?;
    Ok(())
}
