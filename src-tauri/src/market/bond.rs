//! Vínculo piloto-equipe (ideia 4 "Foco da equipe + Vínculo", Fase 1 — fundação).
//!
//! Relação de longo prazo por par (piloto, equipe): um valor 0–100 que **cresce a
//! cada temporada juntos** (mais rápido vencendo) e **decai devagar quando
//! separados** (mantém a "casa" morna para o filho pródigo, na Fase 3). É o motor
//! do objetivo do usuário — um time segurar um piloto pra fazer história junto, em
//! vez de ele pular de galho em galho.
//!
//! Exibido como **selo qualitativo de 6 níveis** (decisão do usuário; sem número
//! cru na UI). Nesta fase o módulo só ACUMULA o vínculo e expõe o selo; as
//! consequências (renovação leal, segurar-vs-vender) vêm em cima disto.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::models::season::Season;

/// Ganho-base por temporada juntos.
const BASE_PER_SEASON: f64 = 12.0;
/// Bônus de "vencer juntos": título de construtores naquela temporada.
const TITLE_BONUS: f64 = 15.0;
/// Perda por temporada separados (lenta — a casa esfria devagar).
const DECAY_APART: f64 = 8.0;
const MIN: f64 = 0.0;
const MAX: f64 = 100.0;

/// Nível do vínculo (1–6) a partir do valor 0–100.
pub fn bond_level(vinculo: f64) -> u8 {
    match vinculo {
        v if v < 15.0 => 1,
        v if v < 33.0 => 2,
        v if v < 53.0 => 3,
        v if v < 73.0 => 4,
        v if v < 90.0 => 5,
        _ => 6,
    }
}

/// Selo qualitativo do vínculo (o que a UI mostra).
pub fn bond_level_label(vinculo: f64) -> &'static str {
    match bond_level(vinculo) {
        1 => "Recém-chegado",
        2 => "Entrosado",
        3 => "Confiança",
        4 => "Pilar do time",
        5 => "Símbolo da equipe",
        _ => "Casa",
    }
}

/// Acúmulo de uma temporada juntos (com bônus se venceram o campeonato de construtores).
pub fn accumulate_bond(current: f64, won_constructor_title: bool) -> f64 {
    let gain = BASE_PER_SEASON + if won_constructor_title { TITLE_BONUS } else { 0.0 };
    (current + gain).clamp(MIN, MAX)
}

/// Decaimento de uma temporada separados.
pub fn decay_bond(current: f64) -> f64 {
    (current - DECAY_APART).clamp(MIN, MAX)
}

/// Vínculo atual de um par (0.0 se não houver registro).
pub fn get_bond(conn: &Connection, piloto_id: &str, equipe_id: &str) -> Result<f64, String> {
    conn.query_row(
        "SELECT vinculo FROM driver_team_bond WHERE piloto_id = ?1 AND equipe_id = ?2",
        rusqlite::params![piloto_id, equipe_id],
        |row| row.get::<_, f64>(0),
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(0.0),
        other => Err(format!("Falha ao ler vínculo: {other}")),
    })
}

/// Ao fim da temporada, atualiza o vínculo de todos os pares: os que estiveram
/// juntos acumulam (bônus por título); todos os demais registros decaem.
///
/// Fonte "quem esteve com quem": `team_season_archive` (snapshot piloto_1/2 + título
/// de construtor da temporada). Roda no offseason ([`crate::evolution::pipeline`]),
/// após o arquivamento.
pub(crate) fn update_bonds_from_season(conn: &Connection, season: &Season) -> Result<(), String> {
    // 1) Carrega o estado atual de todos os vínculos.
    let mut existing: HashMap<(String, String), (f64, i64)> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT piloto_id, equipe_id, vinculo, temporadas FROM driver_team_bond")
            .map_err(|e| format!("Falha ao preparar leitura de vínculos: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    (row.get::<_, f64>(2)?, row.get::<_, i64>(3)?),
                ))
            })
            .map_err(|e| format!("Falha ao consultar vínculos: {e}"))?;
        for row in rows {
            let (key, val) = row.map_err(|e| format!("Falha ao mapear vínculo: {e}"))?;
            existing.insert(key, val);
        }
    }

    // 2) Pares que estiveram juntos nesta temporada (do arquivo).
    let mut together: HashMap<(String, String), bool> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT team_id, piloto_1_id, piloto_2_id, titulos_construtores
                 FROM team_season_archive WHERE season_number = ?1",
            )
            .map_err(|e| format!("Falha ao preparar arquivo p/ vínculo: {e}"))?;
        let rows = stmt
            .query_map([season.numero], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|e| format!("Falha ao consultar arquivo p/ vínculo: {e}"))?;
        for row in rows {
            let (team_id, p1, p2, titles) =
                row.map_err(|e| format!("Falha ao mapear arquivo p/ vínculo: {e}"))?;
            let won = titles > 0;
            for pilot in [p1, p2].into_iter().flatten() {
                together.insert((pilot, team_id.clone()), won);
            }
        }
    }

    // 3) Pares juntos: acumulam (upsert). 4) Demais registros: decaem.
    for ((piloto_id, equipe_id), won) in &together {
        let current = existing
            .get(&(piloto_id.clone(), equipe_id.clone()))
            .map(|(v, _)| *v)
            .unwrap_or(0.0);
        let vinculo = accumulate_bond(current, *won);
        conn.execute(
            "INSERT INTO driver_team_bond (piloto_id, equipe_id, vinculo, temporadas, updated_at)
             VALUES (?1, ?2, ?3, 1, datetime('now'))
             ON CONFLICT(piloto_id, equipe_id) DO UPDATE SET
                vinculo = ?3,
                temporadas = temporadas + 1,
                updated_at = datetime('now')",
            rusqlite::params![piloto_id, equipe_id, vinculo],
        )
        .map_err(|e| format!("Falha ao acumular vínculo: {e}"))?;
    }

    for ((piloto_id, equipe_id), (current, _)) in &existing {
        if together.contains_key(&(piloto_id.clone(), equipe_id.clone())) {
            continue;
        }
        let vinculo = decay_bond(*current);
        conn.execute(
            "UPDATE driver_team_bond SET vinculo = ?3, updated_at = datetime('now')
             WHERE piloto_id = ?1 AND equipe_id = ?2",
            rusqlite::params![piloto_id, equipe_id, vinculo],
        )
        .map_err(|e| format!("Falha ao decair vínculo: {e}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use rusqlite::{params, Connection};

    #[test]
    fn levels_map_the_full_ladder() {
        assert_eq!((bond_level(0.0), bond_level_label(0.0)), (1, "Recém-chegado"));
        assert_eq!(bond_level(20.0), 2);
        assert_eq!(bond_level(40.0), 3);
        assert_eq!(bond_level(60.0), 4);
        assert_eq!(bond_level(80.0), 5);
        assert_eq!((bond_level(95.0), bond_level_label(95.0)), (6, "Casa"));
    }

    #[test]
    fn accumulation_rewards_winning_together_and_clamps() {
        assert_eq!(accumulate_bond(0.0, false), 12.0);
        assert_eq!(accumulate_bond(0.0, true), 27.0);
        assert_eq!(accumulate_bond(95.0, true), 100.0);
    }

    #[test]
    fn decay_is_slow_and_floors_at_zero() {
        assert_eq!(decay_bond(30.0), 22.0);
        assert_eq!(decay_bond(4.0), 0.0);
    }

    #[test]
    fn bond_deepens_gradually_over_seasons_together() {
        // Sem títulos: nível sobe devagar (vínculo profundo é raro/especial).
        let mut v = 0.0;
        let levels: Vec<u8> = (0..6)
            .map(|_| {
                v = accumulate_bond(v, false);
                bond_level(v)
            })
            .collect();
        // 12,24,36,48,60,72 → níveis 1,2,3,3,4,4.
        assert_eq!(levels, vec![1, 2, 3, 3, 4, 4], "progressão de níveis");
        // "Pilar do time" (nível 4) chega na 5ª temporada juntos.
        assert!(levels[4] >= 4);
    }

    fn seed_archived_pair(conn: &Connection, team: &str, p1: &str, p2: Option<&str>, titles: i64) {
        conn.execute(
            "INSERT INTO team_season_archive
                (team_id, season_number, ano, categoria, classe, posicao_campeonato,
                 pontos, vitorias, podios, poles, corridas, titulos_construtores,
                 piloto_1_id, piloto_2_id, snapshot_json)
             VALUES (?1, 1, 2026, 'gt3', NULL, 1, 0, 0, 0, 0, 12, ?4, ?2, ?3, '')",
            params![team, p1, p2, titles],
        )
        .expect("seed archive");
    }

    #[test]
    fn season_update_accumulates_together_pairs_and_decays_the_rest() {
        let conn = Connection::open_in_memory().expect("db");
        migrations::run_all(&conn).expect("schema");

        // Par que já tinha vínculo e ficou junto (campeão) + um par antigo que se separou.
        conn.execute(
            "INSERT INTO driver_team_bond (piloto_id, equipe_id, vinculo, temporadas)
             VALUES ('P1', 'T1', 40.0, 3), ('POLD', 'TOLD', 50.0, 4)",
            [],
        )
        .expect("seed bonds");
        seed_archived_pair(&conn, "T1", "P1", Some("P2"), 1); // T1 campeã: P1+P2 juntos

        let season = Season::new("S1".to_string(), 1, 2026);
        update_bonds_from_season(&conn, &season).expect("update");

        // P1 continuou e foi campeão: 40 + 12 + 15 = 67 (Pilar do time).
        assert_eq!(get_bond(&conn, "P1", "T1").unwrap(), 67.0);
        // P2 novo no time campeão: 0 + 27 = 27.
        assert_eq!(get_bond(&conn, "P2", "T1").unwrap(), 27.0);
        // Par antigo separado decaiu: 50 − 8 = 42.
        assert_eq!(get_bond(&conn, "POLD", "TOLD").unwrap(), 42.0);
        // temporadas de P1 incrementou.
        let temp: i64 = conn
            .query_row(
                "SELECT temporadas FROM driver_team_bond WHERE piloto_id='P1' AND equipe_id='T1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(temp, 4);
    }
}
