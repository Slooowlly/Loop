//! Identidade da equipe: origem, heranca, perfil de desempenho, rival historico e
//! piloto simbolo.

use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct DriverSymbolAggregate {
    pub(super) name: String,
    pub(super) races: i32,
    pub(super) wins: i32,
    pub(super) podiums: i32,
}

pub(super) fn build_real_team_identity(
    conn: &rusqlite::Connection,
    team_id: &str,
    category: &str,
    record_scope: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    titles: i32,
) -> Result<TeamHistoryIdentity, String> {
    let origin_category = selected_facts
        .first()
        .map(|fact| fact.category.as_str())
        .unwrap_or(category);
    let current = current_team_category_label(conn, team_id)
        .unwrap_or_else(|| team_history_category_label(category));
    let profile = real_team_profile(
        selected_facts.len() as i32,
        selected_facts.iter().filter(|fact| fact.win).count() as i32,
        selected_facts.iter().filter(|fact| fact.podium).count() as i32,
        titles,
    );
    let team_name = conn
        .query_row(
            "SELECT nome FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("Falha ao buscar equipe para identidade real: {e}"))?
        .unwrap_or_else(|| rust_i18n::t!("team_dossier.team_fallback").to_string());
    let rival = real_team_rival(conn, team_id, selected_facts, aggregates, record_scope)?;
    let (symbol_driver, symbol_driver_detail) = real_symbol_driver(conn, team_id, selected_facts)?;

    let heritage = team_heritage_label(distinct_seasons(selected_facts).len() as i32);

    Ok(TeamHistoryIdentity {
        origin: team_history_category_label(origin_category),
        current,
        heritage,
        profile: team_profile_label(profile),
        summary: real_identity_summary(&team_name, profile, selected_facts.len() as i32, titles),
        rival,
        symbol_driver,
        symbol_driver_detail,
    })
}

fn current_team_category_label(conn: &rusqlite::Connection, team_id: &str) -> Option<String> {
    let category = conn
        .query_row(
            "SELECT categoria FROM teams WHERE id = ?1",
            rusqlite::params![team_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()?;
    Some(team_history_category_label(&category))
}

/// Herança da equipe por EXPERIÊNCIA real (temporadas competidas), substituindo
/// o antigo corte de ano de fundação (1970) que rotulava quase todo time igual.
fn team_heritage_label(seasons: i32) -> String {
    let key = match seasons {
        0 => "debutant",
        1..=2 => "new",
        3..=6 => "rising",
        7..=14 => "established",
        _ => "traditional",
    };
    let full = format!("team_dossier.heritage.{key}");
    rust_i18n::t!(&full).to_string()
}

/// Perfil de DESEMPENHO por histórico real, numa escada coerente do topo ao
/// fundo do grid. Fonte única de verdade (o fallback do frontend só vale durante
/// o carregamento). Taxas calculadas no nível de corrida.
/// Perfil de DESEMPENHO por histórico real. Retorna uma CHAVE estável (não o texto)
/// — o display resolve por `team_profile_label` e a lógica de resumo casa a chave,
/// evitando o antipattern de comparar prosa traduzível.
fn real_team_profile(races: i32, wins: i32, podiums: i32, titles: i32) -> &'static str {
    if races < 4 {
        return "forming";
    }
    let win_rate = wins as f64 / races as f64;
    let podium_rate = podiums as f64 / races as f64;

    if titles > 0 || win_rate >= 0.30 || podium_rate >= 0.60 {
        "dominant"
    } else if win_rate >= 0.10 {
        "winning"
    } else if podium_rate >= 0.30 {
        "competitive"
    } else if podium_rate >= 0.10 {
        "midfield"
    } else {
        "support"
    }
}

/// Rótulo de display do perfil (i18n) a partir da chave estável.
fn team_profile_label(key: &str) -> String {
    let full = format!("team_dossier.profile.{key}");
    rust_i18n::t!(&full).to_string()
}

fn real_identity_summary(team_name: &str, profile_key: &str, races: i32, _titles: i32) -> String {
    match profile_key {
        "dominant" => {
            rust_i18n::t!("team_dossier.identity_summary.dominant", team = team_name).to_string()
        }
        "winning" => {
            rust_i18n::t!("team_dossier.identity_summary.winning", team = team_name).to_string()
        }
        "competitive" => rust_i18n::t!(
            "team_dossier.identity_summary.competitive",
            team = team_name
        )
        .to_string(),
        "midfield" => {
            rust_i18n::t!("team_dossier.identity_summary.midfield", team = team_name).to_string()
        }
        "support" => {
            rust_i18n::t!("team_dossier.identity_summary.support", team = team_name).to_string()
        }
        _ => rust_i18n::t!(
            "team_dossier.identity_summary.forming",
            team = team_name,
            races = races
        )
        .to_string(),
    }
}

fn real_team_rival(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
    aggregates: &HashMap<String, TeamHistoryAggregate>,
    record_scope: &str,
) -> Result<TeamHistoryRival, String> {
    let selected_races: HashSet<(i32, i32, String)> = selected_facts
        .iter()
        .map(|fact| (fact.season_number, fact.round, fact.category.clone()))
        .collect();
    let mut shared_races: HashMap<String, i32> = HashMap::new();
    for fact in load_team_race_facts(
        conn,
        &selected_facts
            .iter()
            .map(|fact| fact.category.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>(),
    )? {
        if fact.team_id == team_id {
            continue;
        }
        if selected_races.contains(&(fact.season_number, fact.round, fact.category.clone())) {
            *shared_races.entry(fact.team_id).or_default() += 1;
        }
    }

    let selected_points = aggregates
        .get(team_id)
        .map(|entry| entry.points)
        .unwrap_or(0.0);
    let rival_id = shared_races
        .iter()
        .max_by(|(left_id, left_shared), (right_id, right_shared)| {
            left_shared.cmp(right_shared).then_with(|| {
                let left_gap = aggregates
                    .get(left_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                let right_gap = aggregates
                    .get(right_id.as_str())
                    .map(|entry| (entry.points - selected_points).abs())
                    .unwrap_or(f64::MAX);
                right_gap.total_cmp(&left_gap)
            })
        })
        .map(|(id, _)| id.clone());

    let Some(rival_id) = rival_id else {
        return Ok(TeamHistoryRival {
            name: rust_i18n::t!("team_dossier.rival_none").to_string(),
            current_category: record_scope.to_string(),
            note: "Histórico real ainda sem confronto repetido o bastante para formar rivalidade."
                .to_string(),
        });
    };

    let (name, category): (String, String) = conn
        .query_row(
            "SELECT nome, categoria FROM teams WHERE id = ?1",
            rusqlite::params![&rival_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Falha ao buscar rival histórico real: {e}"))?;
    let shared = shared_races.get(&rival_id).copied().unwrap_or(0);
    Ok(TeamHistoryRival {
        name,
        current_category: team_history_category_label(&category),
        note: format!("{shared} disputas diretas reais no {record_scope}."),
    })
}

fn real_symbol_driver(
    conn: &rusqlite::Connection,
    team_id: &str,
    selected_facts: &[TeamRaceFact],
) -> Result<(String, String), String> {
    if selected_facts.is_empty() {
        return Ok((
            rust_i18n::t!("team_dossier.symbol_none").to_string(),
            "A equipe ainda não tem corridas registradas suficientes nesse recorte.".to_string(),
        ));
    }
    let category_ids = selected_facts
        .iter()
        .map(|fact| fact.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let placeholders = vec!["?"; category_ids.len()].join(", ");
    let sql = format!(
        "SELECT
            r.piloto_id,
            d.nome,
            COUNT(DISTINCT r.race_id) AS races,
            SUM(CASE WHEN r.posicao_final = 1 THEN 1 ELSE 0 END) AS wins,
            SUM(CASE WHEN r.posicao_final BETWEEN 1 AND 3 THEN 1 ELSE 0 END) AS podiums
         FROM race_results r
         JOIN calendar c ON c.id = r.race_id
         JOIN drivers d ON d.id = r.piloto_id
         WHERE r.equipe_id = ?1
           AND c.categoria IN ({placeholders})
         GROUP BY r.piloto_id, d.nome
         ORDER BY wins DESC, podiums DESC, races DESC, d.nome ASC
         LIMIT 1"
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&team_id];
    for category in &category_ids {
        params.push(category);
    }
    let symbol = conn
        .query_row(&sql, params.as_slice(), |row| {
            Ok(DriverSymbolAggregate {
                name: row.get(1)?,
                races: row.get(2)?,
                wins: row.get(3)?,
                podiums: row.get(4)?,
            })
        })
        .optional()
        .map_err(|e| format!("Falha ao buscar piloto símbolo real: {e}"))?;

    let Some(symbol) = symbol else {
        return Ok((
            rust_i18n::t!("team_dossier.symbol_none").to_string(),
            "A equipe ainda não tem piloto com resultados registrados nesse recorte.".to_string(),
        ));
    };

    Ok((
        symbol.name,
        format!(
            "{}, {}, {} pela equipe.",
            count_label(symbol.races, "corrida", "corridas"),
            count_label(symbol.wins, "vitória", "vitórias"),
            count_label(symbol.podiums, "pódio", "pódios")
        ),
    ))
}

fn count_label(count: i32, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}
