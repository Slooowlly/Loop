//! Previsão de quebra de peça: risco por peça na pré-corrida e risco agregado da grade.

use super::*;

/// Risco de UMA peça na previsão pré-corrida (probabilidade + nível pra UI).
#[derive(serde::Serialize)]
pub struct ForecastPartView {
    pub part: String,
    pub part_name: String,
    pub any_prob: f64,
    pub dnf_prob: f64,
    /// "baixo" | "médio" | "alto".
    pub level: String,
    /// CONSEQUÊNCIA pro jogador (o que a UI mostra em palavra + cor), derivada do que a peça
    /// custa e não da probabilidade crua: "confiavel" | "custa_tempo" | "pode_abandonar".
    pub consequencia: String,
}

/// Previsão de risco de quebra do carro do jogador pra próxima corrida (aviso pré-corrida).
#[derive(serde::Serialize)]
pub struct BreakdownForecastView {
    /// `false` se não deu pra prever (sem time/corrida/carro) — a UI esconde o card.
    pub available: bool,
    /// Risco geral de ABANDONO por quebra nesta corrida.
    pub dnf_prob: f64,
    pub overall_level: String,
    /// Peças em risco, a mais arriscada primeiro (só as relevantes, no máx. 5).
    pub parts: Vec<ForecastPartView>,
}

/// Contexto compartilhado da previsão de quebra da PRÓXIMA corrida do jogador — categoria,
/// clima, pista, seed determinística e enduro. Base tanto do card do jogador
/// ([`get_breakdown_forecast`]) quanto do aviso na tabela do campeonato
/// ([`get_grid_breakdown_risk`]). `None` quando não dá pra prever (sem time/corrida).
struct RaceBreakdownCtx {
    player_team_id: String,
    categoria: String,
    weather: crate::car::breakdown::Weather,
    track_pha: (f64, f64, f64),
    ev_seed: u64,
    is_enduro: bool,
}

fn resolve_race_breakdown_ctx(
    db: &crate::db::connection::Database,
    career_id: &str,
) -> Option<RaceBreakdownCtx> {
    use crate::db::queries::{
        calendar as calq, contracts as cq, drivers as dq, seasons as sq, teams as tq,
    };
    use crate::market::car_maintenance::maintenance_demand;

    // Time + categoria do jogador (a tabela do campeonato mostrada é a da categoria dele).
    let team_id = dq::get_player_driver(&db.conn)
        .ok()
        .and_then(|p| cq::get_active_contract_for_pilot(&db.conn, &p.id).ok().flatten())
        .map(|c| c.equipe_id)?;
    let team = tq::get_team_by_id(&db.conn, &team_id).ok().flatten()?;
    let categoria = team.categoria.clone();

    // Próxima corrida pendente da categoria.
    let season = sq::get_active_season(&db.conn).ok().flatten()?;
    let race = calq::get_next_race(&db.conn, &season.id, &categoria).ok().flatten()?;

    // Clima da etapa — MESMA história determinística do export/disparo vivo.
    let ev_seed = event_seed(career_id, &race.id);
    let weather = race_breakdown_weather(race.track_id, race.week_of_year, ev_seed, false);
    let track_pha = maintenance_demand(&[race.track_id]);

    // Enduro (corrida longa) → o forecast reflete o DNF raro (severidade abrandada).
    let is_enduro = crate::constants::categories::get_category_config(&categoria)
        .map(|c| crate::car::breakdown::is_enduro_duration(c.duracao_corrida_min))
        .unwrap_or(false);

    Some(RaceBreakdownCtx {
        player_team_id: team_id,
        categoria,
        weather,
        track_pha,
        ev_seed,
        is_enduro,
    })
}

/// AVISO PRÉ-CORRIDA: prevê o risco de quebra do carro do JOGADOR na PRÓXIMA corrida via Monte
/// Carlo sobre o desgaste REAL do `team_car` + a pista + o clima da etapa — os MESMOS inputs do
/// disparo ao vivo. É RISCO (probabilidade), não o desfecho: não revela qual peça/volta vai
/// quebrar. Alimenta o card da Sala de Estratégia e um fato do briefing do engenheiro.
#[tauri::command]
pub fn get_breakdown_forecast(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<BreakdownForecastView, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{team_car as tcq, teams as tq};
    use tauri::Manager;

    let none = BreakdownForecastView {
        available: false,
        dnf_prob: 0.0,
        overall_level: "baixo".to_string(),
        parts: Vec::new(),
    };

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Ok(none);
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let Some(ctx) = resolve_race_breakdown_ctx(&db, &career_id) else {
        return Ok(none);
    };
    let categoria = ctx.categoria.clone();
    let Some(team) = tq::get_team_by_id(&db.conn, &ctx.player_team_id).ok().flatten() else {
        return Ok(none);
    };
    let Some(car) = tcq::get_team_car(&db.conn, &ctx.player_team_id).ok().flatten() else {
        return Ok(none);
    };

    // 18 voltas = referência de sprint (a escala calibrada). 400 amostras dão um % estável.
    let f = crate::car::breakdown::forecast_breakdown_risk(
        &car,
        18,
        ctx.ev_seed,
        team.pit_crew_quality,
        ctx.track_pha,
        ctx.weather,
        &[],
        400,
        ctx.is_enduro,
        crate::car::cost::category_ceiling(&categoria) > 2,
    );

    let part_level = |p: f64| {
        if p < 0.08 {
            "baixo"
        } else if p < 0.20 {
            "médio"
        } else {
            "alto"
        }
    };
    // CONSEQUÊNCIA (o que a UI mostra). Limiares de calibração — ainda por afinar na pista:
    //  · "pode_abandonar" (vermelho): há risco REAL de a peça encerrar a corrida.
    //  · "custa_tempo" (laranja): penalidade pesada provável, OU tantas idas ao box que doem.
    //  · "confiavel" (verde): no máximo desgaste trivial.
    const DNF_VERMELHO: f64 = 0.03;
    const CUSTO_LARANJA: f64 = 0.08;
    const IDAS_LARANJA: f64 = 0.50;
    let consequencia = |r: &crate::car::breakdown::PartRisk| {
        if r.dnf_prob >= DNF_VERMELHO {
            "pode_abandonar"
        } else if r.costly_prob >= CUSTO_LARANJA || r.any_prob >= IDAS_LARANJA {
            "custa_tempo"
        } else {
            "confiavel"
        }
    };
    let overall_level = if f.dnf_prob < 0.05 {
        "baixo"
    } else if f.dnf_prob < 0.12 {
        "médio"
    } else {
        "alto"
    };
    // A mais perigosa primeiro (DNF > custo > idas): o topo vira o "ponto fraco" na UI.
    let mut ranked: Vec<&crate::car::breakdown::PartRisk> =
        f.parts.iter().filter(|r| r.any_prob >= 0.03).collect();
    ranked.sort_by(|a, b| {
        (b.dnf_prob, b.costly_prob, b.any_prob)
            .partial_cmp(&(a.dnf_prob, a.costly_prob, a.any_prob))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let parts = ranked
        .into_iter()
        .take(5)
        .map(|r| ForecastPartView {
            part: r.part.as_str().to_string(),
            part_name: r.part.display_name(&categoria).to_string(),
            any_prob: r.any_prob,
            dnf_prob: r.dnf_prob,
            level: part_level(r.any_prob).to_string(),
            consequencia: consequencia(r).to_string(),
        })
        .collect();

    Ok(BreakdownForecastView {
        available: true,
        dnf_prob: f.dnf_prob,
        overall_level: overall_level.to_string(),
        parts,
    })
}

/// AVISO NA TABELA DO CAMPEONATO: devolve os IDs das EQUIPES cujo carro tem risco REAL de
/// quebra na próxima corrida (penalidade pesada ou DNF — o desgaste trivial NÃO conta, senão
/// quase toda equipe acenderia). A UI marca com 🔧 os pilotos dessas equipes (ambos partilham o
/// carro). Mesmos inputs deterministas do card do jogador; menos amostras (é só sim/não).
#[tauri::command]
pub fn get_grid_breakdown_risk(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<Vec<String>, String> {
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::{team_car as tcq, teams as tq};
    use tauri::Manager;

    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let db = Database::open_existing(&db_path)
        .map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let Some(ctx) = resolve_race_breakdown_ctx(&db, &career_id) else {
        return Ok(Vec::new());
    };
    let teams = tq::get_teams_by_category(&db.conn, &ctx.categoria).unwrap_or_default();

    let mut risky: Vec<String> = Vec::new();
    for team in teams {
        let Some(car) = tcq::get_team_car(&db.conn, &team.id).ok().flatten() else {
            continue;
        };
        // Semente decorrelacionada por equipe (FNV-1a do id) pra os times não partilharem o
        // mesmo padrão de sorteio — a probabilidade em si já é estável com 150 amostras.
        let team_hash = team
            .id
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
            });
        let f = crate::car::breakdown::forecast_breakdown_risk(
            &car,
            18,
            ctx.ev_seed ^ team_hash,
            team.pit_crew_quality,
            ctx.track_pha,
            ctx.weather,
            &[],
            150,
            ctx.is_enduro,
            crate::car::cost::category_ceiling(&ctx.categoria) > 2,
        );
        // "Risco real" = mesma régua do card (peça que custa tempo de verdade ou pode abandonar);
        // o desgaste trivial (any_prob) fica de fora pra o marcador não virar ruído.
        let notable = f.dnf_prob >= 0.05
            || f
                .parts
                .iter()
                .any(|p| p.dnf_prob >= 0.03 || p.costly_prob >= 0.08);
        if notable {
            risky.push(team.id);
        }
    }

    Ok(risky)
}
