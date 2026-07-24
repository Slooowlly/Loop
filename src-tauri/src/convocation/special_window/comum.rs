//! Auxiliares puros compartilhados: rankings visíveis por categoria, badges de
//! licença, modificadores de perfil de mercado e leitura de níveis de licença.

use super::*;

pub(super) fn feeder_category_for_class(class_name: Option<&str>) -> Option<&'static str> {
    let class_name = class_name?;
    CLASSES_CONVOCADAS
        .iter()
        .find(|cfg| cfg.class_name == class_name)
        .map(|cfg| cfg.feeder_category)
}

pub(super) fn display_day_for_reveal(day: i32, total_days: i32) -> i32 {
    if day >= total_days {
        total_days
    } else {
        day + 1
    }
}

pub(super) fn is_visible_regular_origin(category: &str) -> bool {
    is_visible_production_origin(category) || is_visible_endurance_origin(category)
}

pub(super) fn is_visible_production_origin(category: &str) -> bool {
    VISIBLE_PRODUCTION_ORIGINS.contains(&category)
}

pub(super) fn is_visible_endurance_origin(category: &str) -> bool {
    VISIBLE_ENDURANCE_ORIGINS.contains(&category)
}

pub(super) fn build_visible_category_rankings(
    conn: &Connection,
) -> Result<HashMap<(String, String), (i32, i32)>, DbError> {
    let mut rankings = HashMap::new();
    let visible_categories = VISIBLE_PRODUCTION_ORIGINS
        .iter()
        .chain(VISIBLE_ENDURANCE_ORIGINS.iter());

    for category in visible_categories {
        let mut drivers = driver_queries::get_drivers_by_category(conn, category)?
            .into_iter()
            .filter(|driver| driver.status == crate::models::enums::DriverStatus::Ativo)
            .collect::<Vec<_>>();
        drivers.sort_by(|left, right| {
            right
                .stats_temporada
                .pontos
                .total_cmp(&left.stats_temporada.pontos)
                .then_with(|| {
                    right
                        .stats_temporada
                        .vitorias
                        .cmp(&left.stats_temporada.vitorias)
                })
                .then_with(|| {
                    right
                        .stats_temporada
                        .podios
                        .cmp(&left.stats_temporada.podios)
                })
                .then_with(|| {
                    left.stats_temporada
                        .posicao_media
                        .total_cmp(&right.stats_temporada.posicao_media)
                })
                .then_with(|| left.nome.cmp(&right.nome))
        });

        let total = drivers.len() as i32;
        for (index, driver) in drivers.iter().enumerate() {
            rankings.insert(
                (driver.id.clone(), (*category).to_string()),
                (index as i32 + 1, total),
            );
        }
    }

    Ok(rankings)
}

pub(super) fn insert_log(
    conn: &Connection,
    season_id: &str,
    day: i32,
    event_type: &str,
    message: &str,
    special_category: Option<&str>,
    class_name: Option<&str>,
    team_id: Option<&str>,
    driver_id: Option<&str>,
) -> Result<(), DbError> {
    let team_part = team_id.unwrap_or("sem-equipe");
    let driver_part = driver_id.unwrap_or("sem-piloto");
    conn.execute(
        "INSERT INTO special_window_daily_log (
            id, season_id, day_number, event_type, message, special_category,
            class_name, team_id, driver_id, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            format!("SWL-{season_id}-{day}-{event_type}-{team_part}-{driver_part}"),
            season_id,
            day,
            event_type,
            message,
            special_category,
            class_name,
            team_id,
            driver_id,
            current_timestamp(),
        ],
    )?;
    Ok(())
}

pub(super) fn schedule_reveal_day(rank_index: usize, total: usize, team_strength: f64, team_id: &str) -> i32 {
    let base_day = if total <= 1 {
        1
    } else {
        1 + ((rank_index * (TOTAL_SPECIAL_WINDOW_DAYS as usize - 1)) / (total - 1)) as i32
    };
    // Limiares na escala única 0–100 (eram 12,0 / 4,0 no domínio 0–16 do escalar cru).
    let strength_modifier = if team_strength >= 75.0 {
        -1
    } else if team_strength <= 25.0 {
        1
    } else {
        0
    };
    let profile_modifier = market_profile_modifier(team_id);
    (base_day + strength_modifier + profile_modifier).clamp(1, TOTAL_SPECIAL_WINDOW_DAYS)
}

pub(super) fn market_profile_modifier(team_id: &str) -> i32 {
    match team_id.bytes().fold(0_u32, |acc, value| acc + value as u32) % 4 {
        0 => -1,
        1 => 1,
        2 => 1,
        _ => 0,
    }
}

pub(super) fn derive_player_desirability(player: &Driver) -> i32 {
    let champion_bonus = if player.melhor_resultado_temp == Some(1) {
        8
    } else {
        0
    };
    let wins_bonus = (player.stats_temporada.vitorias as i32).min(5) * 2;
    (player.atributos.skill.round() as i32 + champion_bonus + wins_bonus).clamp(50, 99)
}

pub(super) fn license_badge(level: Option<u8>) -> (&'static str, &'static str) {
    match level {
        Some(0) => ("Rookie", "R"),
        Some(1) => ("Amador", "A"),
        Some(2) => ("Pro", "P"),
        Some(3) => ("Super Pro", "SP"),
        Some(4) => ("Elite", "E"),
        Some(_) => ("Super Elite", "SE"),
        None => ("Rookie", "R"),
    }
}

pub(super) fn load_license_levels(conn: &Connection) -> Result<HashMap<String, u8>, DbError> {
    let mut stmt = conn.prepare(
        "SELECT piloto_id, MAX(CAST(nivel AS INTEGER)) AS max_nivel
         FROM licenses
         GROUP BY piloto_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u8))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (piloto_id, nivel) = row?;
        map.insert(piloto_id, nivel);
    }
    Ok(map)
}
