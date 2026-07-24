//! Utilidades compartilhadas pelas etapas da entressafra: transação aninhada,
//! carimbo de data e as leituras de "quanto vale" uma equipe/categoria usadas
//! tanto pelo assédio da IA quanto pelas telas do jogador.

use super::*;

pub(super) fn with_savepoint<T, F>(conn: &Connection, name: &str, action: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    conn.execute_batch(&format!("SAVEPOINT {name}"))
        .map_err(|e| format!("Falha ao abrir savepoint '{name}': {e}"))?;

    match action() {
        Ok(value) => {
            conn.execute_batch(&format!("RELEASE SAVEPOINT {name}"))
                .map_err(|e| format!("Falha ao confirmar savepoint '{name}': {e}"))?;
            Ok(value)
        }
        Err(err) => {
            conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
            ))
            .map_err(|rollback_err| {
                format!("{err}; alem disso falhou o rollback do savepoint '{name}': {rollback_err}")
            })?;
            Err(err)
        }
    }
}

pub(super) fn timestamp_now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub(super) fn team_quality(team: &crate::models::team::Team) -> f64 {
    // `team_prestige_quality` sempre assumiu carro em 0–100 (ela clampa nisso, e os testes
    // passam 90). Recebia a coluna legada em 0–16: o carro pesava no máximo 9,6 contra 100 de
    // reputação, então o astro só olhava para prestígio. `car_strength` entrega a escala certa.
    crate::fame::team_prestige_quality(
        team.reputacao,
        team.car_strength(),
        team.historico_titulos_pilotos + team.historico_titulos_construtores,
    )
}

/// Prestígio competitivo (0-100) de uma equipe pelos ÚLTIMOS 10 ANOS do campeonato
/// de construtores (título alto, pódio médio, com peso por recência). O que o
/// piloto mais confia (vs a promessa não-verificável do carro). Sem archive → 0.
pub(super) fn team_prestige(conn: &Connection, team_id: &str, current_season: i32) -> Result<f64, String> {
    let mut stmt = match conn.prepare(
        "SELECT season_number, posicao_campeonato FROM team_season_archive
         WHERE team_id = ?1 AND season_number > ?2",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok(0.0),
    };
    let rows = stmt
        .query_map(params![team_id, current_season - 10], |row| {
            Ok((row.get::<_, i32>(0)?, row.get::<_, Option<i32>>(1)?))
        })
        .map_err(|e| format!("Falha ao consultar prestigio da equipe: {e}"))?;
    let mut raw = 0.0;
    for row in rows {
        let (season, pos) = row.map_err(|e| format!("Falha ao ler prestigio da equipe: {e}"))?;
        let Some(pos) = pos else { continue };
        let pts = match pos {
            1 => 10.0,
            2..=3 => 5.0,
            4..=6 => 2.0,
            _ => 0.0,
        };
        let age = (current_season - season).max(0) as f64;
        let recency = (1.0 - age / 10.0).clamp(0.1, 1.0);
        raw += pts * recency;
    }
    Ok((raw * 2.5).min(100.0))
}

/// Marca (mazda/toyota) derivada do id da categoria — só tiers 0-1.
pub(super) fn brand_of_category(category: &str) -> Option<String> {
    if category.starts_with("mazda_") {
        Some("mazda".to_string())
    } else if category.starts_with("toyota_") {
        Some("toyota".to_string())
    } else {
        None
    }
}

/// Piloto-estrela sintético p/ derivar o teto de salário que a equipe comporta.
pub(super) fn synthetic_star() -> Driver {
    let mut star = Driver::new(
        "STAR".to_string(),
        "Star".to_string(),
        "BR".to_string(),
        "M".to_string(),
        26,
        2000,
    );
    star.atributos.skill = 92.0;
    star
}
