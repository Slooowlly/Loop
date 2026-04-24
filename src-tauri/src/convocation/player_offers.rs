use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::models::enums::TeamRole;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerSpecialOffer {
    pub id: String,
    pub player_driver_id: String,
    pub team_id: String,
    pub team_name: String,
    pub special_category: String,
    pub class_name: String,
    pub papel: TeamRole,
    pub status: String,
}

pub fn replace_player_special_offers(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
    offers: &[PlayerSpecialOffer],
) -> Result<(), DbError> {
    conn.execute_batch("SAVEPOINT replace_player_special_offers_batch;")?;

    let result = (|| -> Result<(), DbError> {
        conn.execute(
            "DELETE FROM player_special_offers WHERE season_id = ?1 AND player_driver_id = ?2",
            params![season_id, player_id],
        )?;

        for offer in offers {
            if offer.player_driver_id != player_id {
                return Err(DbError::InvalidData(format!(
                    "Oferta especial '{}' pertence ao piloto '{}', esperado '{}'",
                    offer.id, offer.player_driver_id, player_id
                )));
            }

            conn.execute(
                "INSERT INTO player_special_offers (
                    id, season_id, player_driver_id, team_id, team_name,
                    special_category, class_name, papel, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    &offer.id,
                    season_id,
                    &offer.player_driver_id,
                    &offer.team_id,
                    &offer.team_name,
                    &offer.special_category,
                    &offer.class_name,
                    offer.papel.as_str(),
                    &offer.status,
                    current_timestamp(),
                ],
            )?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("RELEASE SAVEPOINT replace_player_special_offers_batch;")?;
            Ok(())
        }
        Err(err) => {
            let rollback = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT replace_player_special_offers_batch;
                 RELEASE SAVEPOINT replace_player_special_offers_batch;",
            );
            if let Err(rollback_err) = rollback {
                return Err(DbError::Migration(format!(
                    "Falha ao reverter replace_player_special_offers apos erro '{err}': {rollback_err}"
                )));
            }
            Err(err)
        }
    }
}

#[allow(dead_code)]
pub(crate) fn get_pending_player_special_offers(
    conn: &Connection,
    player_id: &str,
) -> Result<Vec<PlayerSpecialOffer>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            id, player_driver_id, team_id, team_name, special_category, class_name, papel, status
         FROM player_special_offers
         WHERE player_driver_id = ?1 AND status = 'Pendente'
         ORDER BY created_at DESC, team_name ASC",
    )?;
    let rows = stmt.query_map(params![player_id], offer_from_row)?;

    let mut offers = Vec::new();
    for row in rows {
        offers.push(row?);
    }
    Ok(offers)
}

pub fn get_pending_player_special_offers_for_season(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
) -> Result<Vec<PlayerSpecialOffer>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            id, player_driver_id, team_id, team_name, special_category, class_name, papel, status
         FROM player_special_offers
         WHERE season_id = ?1 AND player_driver_id = ?2 AND status = 'Pendente'
         ORDER BY created_at DESC, team_name ASC",
    )?;
    let rows = stmt.query_map(params![season_id, player_id], offer_from_row)?;

    let mut offers = Vec::new();
    for row in rows {
        offers.push(row?);
    }
    Ok(offers)
}

#[allow(dead_code)]
pub(crate) fn get_player_special_offer_by_id(
    conn: &Connection,
    offer_id: &str,
) -> Result<Option<PlayerSpecialOffer>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            id, player_driver_id, team_id, team_name, special_category, class_name, papel, status
         FROM player_special_offers
         WHERE id = ?1
         LIMIT 1",
    )?;
    stmt.query_row(params![offer_id], offer_from_row)
        .optional()
        .map_err(DbError::from)
}

pub fn get_player_special_offer_by_id_for_season(
    conn: &Connection,
    season_id: &str,
    offer_id: &str,
) -> Result<Option<PlayerSpecialOffer>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT
            id, player_driver_id, team_id, team_name, special_category, class_name, papel, status
         FROM player_special_offers
         WHERE season_id = ?1 AND id = ?2
         LIMIT 1",
    )?;
    stmt.query_row(params![season_id, offer_id], offer_from_row)
        .optional()
        .map_err(DbError::from)
}

#[allow(dead_code)]
pub(crate) fn update_player_special_offer_status(
    conn: &Connection,
    offer_id: &str,
    status: &str,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE player_special_offers
         SET status = ?1, responded_at = ?2
         WHERE id = ?3",
        params![status, current_timestamp(), offer_id],
    )?;
    Ok(())
}

pub fn update_player_special_offer_status_for_season(
    conn: &Connection,
    season_id: &str,
    offer_id: &str,
    status: &str,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE player_special_offers
         SET status = ?1, responded_at = ?2
         WHERE season_id = ?3 AND id = ?4",
        params![status, current_timestamp(), season_id, offer_id],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn expire_remaining_player_special_offers(
    conn: &Connection,
    player_id: &str,
    except_offer_id: &str,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE player_special_offers
         SET status = 'Expirada', responded_at = ?1
         WHERE player_driver_id = ?2 AND status = 'Pendente' AND id <> ?3",
        params![current_timestamp(), player_id, except_offer_id],
    )?;
    Ok(())
}

pub fn expire_remaining_player_special_offers_for_season(
    conn: &Connection,
    season_id: &str,
    player_id: &str,
    except_offer_id: &str,
) -> Result<(), DbError> {
    conn.execute(
        "UPDATE player_special_offers
         SET status = 'Expirada', responded_at = ?1
         WHERE season_id = ?2 AND player_driver_id = ?3 AND status = 'Pendente' AND id <> ?4",
        params![current_timestamp(), season_id, player_id, except_offer_id],
    )?;
    Ok(())
}

fn offer_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlayerSpecialOffer> {
    Ok(PlayerSpecialOffer {
        id: row.get(0)?,
        player_driver_id: row.get(1)?,
        team_id: row.get(2)?,
        team_name: row.get(3)?,
        special_category: row.get(4)?,
        class_name: row.get(5)?,
        papel: TeamRole::from_str_strict(&row.get::<_, String>(6)?)
            .map_err(rusqlite::Error::InvalidParameterName)?,
        status: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_offers_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE player_special_offers (
                id TEXT PRIMARY KEY,
                season_id TEXT NOT NULL,
                player_driver_id TEXT NOT NULL,
                team_id TEXT NOT NULL,
                team_name TEXT NOT NULL,
                special_category TEXT NOT NULL,
                class_name TEXT NOT NULL,
                papel TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'Pendente',
                created_at TEXT NOT NULL DEFAULT '',
                responded_at TEXT
            );",
        )
        .expect("create table");
    }

    #[test]
    fn test_replace_and_get_pending_player_special_offers() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        setup_offers_table(&conn);

        let offers = vec![
            PlayerSpecialOffer {
                id: "PSO-1".to_string(),
                player_driver_id: "P001".to_string(),
                team_id: "T001".to_string(),
                team_name: "Team One".to_string(),
                special_category: "endurance".to_string(),
                class_name: "gt4".to_string(),
                papel: TeamRole::Numero1,
                status: "Pendente".to_string(),
            },
            PlayerSpecialOffer {
                id: "PSO-2".to_string(),
                player_driver_id: "P001".to_string(),
                team_id: "T002".to_string(),
                team_name: "Team Two".to_string(),
                special_category: "endurance".to_string(),
                class_name: "gt4".to_string(),
                papel: TeamRole::Numero2,
                status: "Expirada".to_string(),
            },
        ];

        replace_player_special_offers(&conn, "S001", "P001", &offers).expect("replace offers");

        let pending = get_pending_player_special_offers(&conn, "P001").expect("pending offers");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "PSO-1");
        assert_eq!(pending[0].special_category, "endurance");
    }

    #[test]
    fn test_invalid_team_role_from_db_returns_error() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        setup_offers_table(&conn);

        let offers = vec![PlayerSpecialOffer {
            id: "PSO-3".to_string(),
            player_driver_id: "P001".to_string(),
            team_id: "T001".to_string(),
            team_name: "Team One".to_string(),
            special_category: "endurance".to_string(),
            class_name: "gt4".to_string(),
            papel: TeamRole::Numero1,
            status: "Pendente".to_string(),
        }];
        replace_player_special_offers(&conn, "S001", "P001", &offers).expect("replace offers");
        conn.execute(
            "UPDATE player_special_offers SET papel = 'papel_quebrado' WHERE id = 'PSO-3'",
            [],
        )
        .expect("corrupt role");

        let err =
            get_player_special_offer_by_id(&conn, "PSO-3").expect_err("invalid role should fail");
        assert!(err.to_string().contains("TeamRole inv"));
    }

    #[test]
    fn test_season_scoped_queries_ignore_other_seasons() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        setup_offers_table(&conn);

        let season_one_offer = PlayerSpecialOffer {
            id: "PSO-S001".to_string(),
            player_driver_id: "P001".to_string(),
            team_id: "T001".to_string(),
            team_name: "Team One".to_string(),
            special_category: "endurance".to_string(),
            class_name: "gt4".to_string(),
            papel: TeamRole::Numero1,
            status: "Pendente".to_string(),
        };
        let season_two_offer = PlayerSpecialOffer {
            id: "PSO-S002".to_string(),
            player_driver_id: "P001".to_string(),
            team_id: "T002".to_string(),
            team_name: "Team Two".to_string(),
            special_category: "endurance".to_string(),
            class_name: "gt4".to_string(),
            papel: TeamRole::Numero2,
            status: "Pendente".to_string(),
        };

        replace_player_special_offers(
            &conn,
            "S001",
            "P001",
            std::slice::from_ref(&season_one_offer),
        )
        .expect("replace s001");
        replace_player_special_offers(
            &conn,
            "S002",
            "P001",
            std::slice::from_ref(&season_two_offer),
        )
        .expect("replace s002");

        let pending = get_pending_player_special_offers_for_season(&conn, "S001", "P001")
            .expect("pending season offers");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "PSO-S001");

        let scoped_offer = get_player_special_offer_by_id_for_season(&conn, "S001", "PSO-S002")
            .expect("scoped offer query");
        assert!(scoped_offer.is_none());
    }

    #[test]
    fn test_season_scoped_status_updates_do_not_touch_other_seasons() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        setup_offers_table(&conn);

        let shared_offer = PlayerSpecialOffer {
            id: "PSO-SHARED-S001".to_string(),
            player_driver_id: "P001".to_string(),
            team_id: "T001".to_string(),
            team_name: "Team One".to_string(),
            special_category: "endurance".to_string(),
            class_name: "gt4".to_string(),
            papel: TeamRole::Numero1,
            status: "Pendente".to_string(),
        };

        replace_player_special_offers(&conn, "S001", "P001", std::slice::from_ref(&shared_offer))
            .expect("replace s001");
        conn.execute(
            "INSERT INTO player_special_offers (
                id, season_id, player_driver_id, team_id, team_name,
                special_category, class_name, papel, status, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                "PSO-SHARED-S002",
                "S002",
                &shared_offer.player_driver_id,
                &shared_offer.team_id,
                &shared_offer.team_name,
                &shared_offer.special_category,
                &shared_offer.class_name,
                shared_offer.papel.as_str(),
                &shared_offer.status,
                current_timestamp(),
            ],
        )
        .expect("insert other season offer");

        update_player_special_offer_status_for_season(&conn, "S001", "PSO-SHARED-S001", "Recusada")
            .expect("scoped reject");
        expire_remaining_player_special_offers_for_season(&conn, "S001", "P001", "PSO-SHARED-S001")
            .expect("scoped expire");

        let season_one = conn
            .query_row(
                "SELECT status FROM player_special_offers
                 WHERE season_id = 'S001' AND id = 'PSO-SHARED-S001'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("s001 status");
        let season_two = conn
            .query_row(
                "SELECT status FROM player_special_offers
                 WHERE season_id = 'S002' AND id = 'PSO-SHARED-S002'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("s002 status");

        assert_eq!(season_one, "Recusada");
        assert_eq!(season_two, "Pendente");
    }

    #[test]
    fn test_replace_player_special_offers_rejects_mismatched_player_and_preserves_existing_rows() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        setup_offers_table(&conn);

        let existing = PlayerSpecialOffer {
            id: "PSO-EXISTING".to_string(),
            player_driver_id: "P001".to_string(),
            team_id: "T001".to_string(),
            team_name: "Team One".to_string(),
            special_category: "endurance".to_string(),
            class_name: "gt4".to_string(),
            papel: TeamRole::Numero1,
            status: "Pendente".to_string(),
        };
        replace_player_special_offers(&conn, "S001", "P001", std::slice::from_ref(&existing))
            .expect("seed existing offer");

        let mismatched = PlayerSpecialOffer {
            id: "PSO-BAD".to_string(),
            player_driver_id: "P999".to_string(),
            ..existing.clone()
        };

        let err = replace_player_special_offers(&conn, "S001", "P001", &[mismatched])
            .expect_err("mismatched player id should fail");
        assert!(matches!(err, DbError::InvalidData(_)));

        let pending = get_pending_player_special_offers_for_season(&conn, "S001", "P001")
            .expect("pending offers should remain");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "PSO-EXISTING");
    }

    #[test]
    fn test_replace_player_special_offers_rolls_back_when_insert_fails() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        setup_offers_table(&conn);

        let existing = PlayerSpecialOffer {
            id: "PSO-EXISTING".to_string(),
            player_driver_id: "P001".to_string(),
            team_id: "T001".to_string(),
            team_name: "Team One".to_string(),
            special_category: "endurance".to_string(),
            class_name: "gt4".to_string(),
            papel: TeamRole::Numero1,
            status: "Pendente".to_string(),
        };
        replace_player_special_offers(&conn, "S001", "P001", std::slice::from_ref(&existing))
            .expect("seed existing offer");

        conn.execute_batch(
            "
            CREATE TRIGGER fail_blocked_offer_insert
            BEFORE INSERT ON player_special_offers
            WHEN NEW.id = 'PSO-BLOCK'
            BEGIN
                SELECT RAISE(ABORT, 'blocked offer insert');
            END;
            ",
        )
        .expect("create trigger");

        let replacement_ok = PlayerSpecialOffer {
            id: "PSO-OK".to_string(),
            player_driver_id: "P001".to_string(),
            team_id: "T002".to_string(),
            team_name: "Team Two".to_string(),
            special_category: "endurance".to_string(),
            class_name: "gt4".to_string(),
            papel: TeamRole::Numero2,
            status: "Pendente".to_string(),
        };
        let replacement_blocked = PlayerSpecialOffer {
            id: "PSO-BLOCK".to_string(),
            ..replacement_ok.clone()
        };

        replace_player_special_offers(
            &conn,
            "S001",
            "P001",
            &[replacement_ok, replacement_blocked],
        )
        .expect_err("insert failure should rollback whole replace");

        let pending = get_pending_player_special_offers_for_season(&conn, "S001", "P001")
            .expect("existing offer should survive rollback");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "PSO-EXISTING");
    }
}
