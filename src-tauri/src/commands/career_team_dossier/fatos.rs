//! Fatos brutos do historico da equipe: leitura de corridas, titulos de construtores,
//! posicoes finais arquivadas e as agregacoes derivadas deles.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct TeamRaceFact {
    pub(super) team_id: String,
    pub(super) season_number: i32,
    pub(super) season_year: i32,
    pub(super) category: String,
    pub(super) round: i32,
    pub(super) points: f64,
    pub(super) win: bool,
    pub(super) podium: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct TeamHistoryAggregate {
    pub(super) races: i32,
    pub(super) wins: i32,
    pub(super) podiums: i32,
    pub(super) points: f64,
}

#[derive(Debug, Clone)]
pub(super) struct TeamTitleFact {
    pub(super) season_year: i32,
    pub(super) category: String,
}

pub(super) fn load_team_race_facts(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<Vec<TeamRaceFact>, String> {
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.equipe_id,
            s.numero,
            s.ano,
            c.categoria,
            c.rodada,
            r.race_id,
            SUM(r.pontos) AS team_points,
            MAX(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS has_win,
            MAX(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS has_podium
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN seasons s ON s.id = c.temporada_id
         WHERE c.categoria IN ({placeholders})
         GROUP BY r.equipe_id, s.numero, c.categoria, c.rodada, r.race_id
         ORDER BY s.numero ASC, c.rodada ASC, r.race_id ASC, r.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar histórico real da equipe: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok(TeamRaceFact {
                team_id: row.get(0)?,
                season_number: row.get(1)?,
                season_year: row.get(2)?,
                category: row.get(3)?,
                round: row.get(4)?,
                points: row.get(6)?,
                win: row.get::<_, i32>(7)? > 0,
                podium: row.get::<_, i32>(8)? > 0,
            })
        })
        .map_err(|e| format!("Falha ao consultar histórico real da equipe: {e}"))?;

    let mut facts = Vec::new();
    for row in rows {
        facts.push(row.map_err(|e| format!("Falha ao ler histórico real da equipe: {e}"))?);
    }
    Ok(facts)
}

pub(super) fn aggregate_team_history(
    facts: &[TeamRaceFact],
) -> HashMap<String, TeamHistoryAggregate> {
    let mut aggregates: HashMap<String, TeamHistoryAggregate> = HashMap::new();
    for fact in facts {
        let entry = aggregates.entry(fact.team_id.clone()).or_default();
        entry.races += 1;
        entry.points += fact.points;
        if fact.win {
            entry.wins += 1;
        }
        if fact.podium {
            entry.podiums += 1;
        }
    }
    aggregates
}

pub(super) fn load_constructor_titles_by_team(
    conn: &rusqlite::Connection,
    category_ids: &[String],
) -> Result<HashMap<String, Vec<TeamTitleFact>>, String> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            st.temporada_id,
            s.numero,
            s.ano,
            st.equipe_id,
            st.categoria,
            SUM(st.pontos) AS team_points,
            SUM(st.vitorias) AS team_wins
         FROM standings st
         JOIN seasons s ON s.id = st.temporada_id
         WHERE st.equipe_id IS NOT NULL
           AND st.categoria IN ({placeholders})
         GROUP BY st.temporada_id, s.numero, s.ano, st.equipe_id, st.categoria
         ORDER BY s.numero ASC, team_points DESC, team_wins DESC, st.equipe_id ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Falha ao preparar títulos reais de equipes: {e}"))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(category_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i32>(6)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar títulos reais de equipes: {e}"))?;

    let mut best_by_season_category: BTreeMap<String, (i32, i32, String, String, f64, i32)> =
        BTreeMap::new();
    for row in rows {
        let (season_id, season_number, season_year, team_id, category, points, wins) =
            row.map_err(|e| format!("Falha ao ler títulos reais de equipes: {e}"))?;
        let key = format!("{season_id}:{category}");
        let replace = best_by_season_category
            .get(&key)
            .map(|(_, _, current_team, _, current_points, current_wins)| {
                points > *current_points
                    || ((points - *current_points).abs() < f64::EPSILON
                        && (wins > *current_wins
                            || (wins == *current_wins && team_id < *current_team)))
            })
            .unwrap_or(true);
        if replace {
            best_by_season_category.insert(
                key,
                (season_number, season_year, team_id, category, points, wins),
            );
        }
    }

    let mut titles: HashMap<String, Vec<TeamTitleFact>> = HashMap::new();
    for (_, (_season_number, season_year, team_id, category, _, _)) in best_by_season_category {
        titles.entry(team_id).or_default().push(TeamTitleFact {
            season_year,
            category,
        });
    }
    Ok(titles)
}

/// Posição final no campeonato por temporada (melhor posição se multiclasse).
/// Degrada para vazio se a tabela de arquivo ainda não existe.
pub(super) fn load_team_season_positions(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> HashMap<i32, i32> {
    let mut positions = HashMap::new();
    let mut stmt = match conn.prepare(
        "SELECT season_number, MIN(posicao_campeonato)
         FROM team_season_archive
         WHERE team_id = ?1 AND posicao_campeonato IS NOT NULL
         GROUP BY season_number",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return positions,
    };
    if let Ok(rows) = stmt.query_map(rusqlite::params![team_id], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?))
    }) {
        for row in rows.flatten() {
            positions.insert(row.0, row.1);
        }
    }
    positions
}

pub(super) fn distinct_seasons(facts: &[TeamRaceFact]) -> Vec<i32> {
    facts
        .iter()
        .map(|fact| fact.season_number)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn best_real_season_points(facts: &[TeamRaceFact]) -> Option<(i32, f64)> {
    let mut points_by_season: BTreeMap<i32, f64> = BTreeMap::new();
    for fact in facts {
        *points_by_season.entry(fact.season_year).or_default() += fact.points;
    }
    points_by_season
        .into_iter()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
}
