//! Gestao da equipe: saude da operacao, caixa/divida, eficiencia por temporada e os
//! eventos de propriedade/diretoria.

use super::*;

/// Carrega os eventos de propriedade/diretoria da equipe (ex.: venda por colapso).
/// Degrada graciosamente para vazio se a tabela ainda não existe (saves antigos
/// abertos somente-leitura antes da migração v36).
pub(super) fn load_team_ownership_events(
    conn: &rusqlite::Connection,
    team_id: &str,
) -> Result<Vec<TeamHistoryOwnershipEvent>, String> {
    let mut stmt = match conn.prepare(
        "SELECT ano, event_type, debt_cleared, cash_injected, detail
         FROM team_ownership_events
         WHERE team_id = ?1
         ORDER BY ano ASC, id ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Ok(Vec::new()),
    };
    let rows = stmt
        .query_map(rusqlite::params![team_id], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao consultar eventos de propriedade: {e}"))?;

    let mut events = Vec::new();
    for row in rows {
        let (ano, event_type, debt_cleared, cash_injected, detail) =
            row.map_err(|e| format!("Falha ao mapear evento de propriedade: {e}"))?;
        let title = match event_type.as_str() {
            "sale" => rust_i18n::t!("team_dossier.ownership.sale_title").to_string(),
            _ => rust_i18n::t!("team_dossier.ownership.change_title").to_string(),
        };
        let financial_note = rust_i18n::t!(
            "team_dossier.ownership.financial_note",
            cleared = format_brl(debt_cleared),
            injected = format_brl(cash_injected)
        )
        .to_string();
        events.push(TeamHistoryOwnershipEvent {
            year: ano.to_string(),
            event_type,
            title,
            detail,
            financial_note,
        });
    }
    Ok(events)
}

pub(super) fn build_real_team_management(
    conn: &rusqlite::Connection,
    team_id: &str,
    facts: &[TeamRaceFact],
) -> Result<TeamHistoryManagement, String> {
    let team = team_queries::get_team_by_id(conn, team_id)
        .map_err(|e| format!("Falha ao carregar gestão histórica da equipe: {e}"))?
        .ok_or_else(|| format!("Equipe '{team_id}' não encontrada para gestão histórica"))?;
    let cash = team.cash_balance.max(0.0);
    let debt = team.debt_balance.max(0.0);
    let points: f64 = facts.iter().map(|fact| fact.points).sum();
    let seasons = distinct_seasons(facts);
    let seasons_count = seasons.len().max(1) as f64;
    let points_per_season = points / seasons_count;
    let healthy_years = if debt <= 0.0 && team.financial_state == "healthy" {
        seasons.len() as i32
    } else {
        0
    };
    let state_label = financial_state_label_for_dossier(&team.financial_state);
    // "Nível do pacote" aqui é o Nível do Carro (1–10) — a MESMA leitura de carro que o
    // jogador vê no ranking e na aba da equipe. Antes era o escalar legado arredondado numa
    // faixa 0–16: outro número, outra escala, e ainda por cima cego ao sistema de peças.
    let technical_level = team
        .car
        .as_ref()
        .map(|car| car.display_level())
        .unwrap_or(1) as i32;

    let efficiency_value = format_decimal_pt(points_per_season, 1);
    let points_int = points.round() as i32;
    Ok(TeamHistoryManagement {
        operation_health: state_label.clone(),
        peak_cash: format_brl(cash),
        worst_crisis: if debt > 0.0 {
            rust_i18n::t!("team_dossier.management.worst_crisis_debt", debt = format_brl(debt))
                .to_string()
        } else {
            rust_i18n::t!("team_dossier.management.worst_crisis_none").to_string()
        },
        healthy_years: rust_i18n::t!("team_dossier.management.healthy_years", count = healthy_years)
            .to_string(),
        efficiency: rust_i18n::t!(
            "team_dossier.management.efficiency",
            value = efficiency_value.as_str()
        )
        .to_string(),
        biggest_investment: rust_i18n::t!(
            "team_dossier.management.biggest_investment",
            level = technical_level
        )
        .to_string(),
        summary: rust_i18n::t!(
            "team_dossier.management.summary",
            state = state_label.as_str(),
            cash = format_brl(cash),
            debt = format_brl(debt),
            points = points_int
        )
        .to_string(),
        peak_cash_detail: rust_i18n::t!("team_dossier.management.peak_cash_detail").to_string(),
        worst_crisis_detail: if debt > 0.0 {
            rust_i18n::t!("team_dossier.management.worst_crisis_detail_debt").to_string()
        } else {
            rust_i18n::t!("team_dossier.management.worst_crisis_detail_none").to_string()
        },
        healthy_years_detail: rust_i18n::t!("team_dossier.management.healthy_years_detail")
            .to_string(),
        efficiency_detail: rust_i18n::t!(
            "team_dossier.management.efficiency_detail",
            points = points_int,
            avg = efficiency_value.as_str()
        )
        .to_string(),
        investment_detail: rust_i18n::t!("team_dossier.management.investment_detail").to_string(),
    })
}

fn financial_state_label_for_dossier(state: &str) -> String {
    let key = match state {
        "dominant" | "healthy" => "healthy",
        "stable" => "stable",
        "pressured" => "pressured",
        "critical" => "critical",
        _ => "monitored",
    };
    let full = format!("team_dossier.state.{key}");
    rust_i18n::t!(&full).to_string()
}
