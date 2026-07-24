//! Notas de MERCADO/FINANÇAS/BASTIDORES: estado das equipes e dos ex-companheiros.

use super::*;

/// Ex-equipes de um piloto (contratos passados), ids únicos preservando a ordem.
pub(super) fn pilot_ex_team_ids(conn: &rusqlite::Connection, pilot_id: &str) -> Vec<String> {
    use crate::db::queries::contracts;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    if let Ok(history) = contracts::get_contracts_for_pilot(conn, pilot_id) {
        for c in history.iter().filter(|c| {
            matches!(c.status, ContractStatus::Expirado | ContractStatus::Rescindido)
        }) {
            if seen.insert(c.equipe_id.clone()) {
                out.push(c.equipe_id.clone());
            }
        }
    }
    out
}

/// Nota sobre o ESTADO de uma equipe (crise, clima, nova diretoria) — jornalística.
/// `None` quando a equipe não tem nada digno de nota (time que vai bem não é notícia).
pub(super) fn team_state_note(
    conn: &rusqlite::Connection,
    team_id: &str,
    categoria: &str,
) -> Option<WorldNote> {
    use crate::db::queries::teams;

    let team = teams::get_team_by_id(conn, team_id).ok().flatten()?;
    if !team.ativa || team.categoria != categoria {
        return None;
    }

    let ownership_sale = teams::get_latest_ownership_event(conn, &team.id)
        .ok()
        .flatten()
        .map(|(event_type, _)| event_type == "sale")
        .unwrap_or(false);
    let in_debt = team.debt_balance > 0.0;
    let in_crisis = matches!(team.financial_state.as_str(), "crisis" | "collapse") || in_debt;
    let bad_mood = team.morale < 0.85 || team.hierarquia_tensao > 55.0;

    let nome = team.nome.as_str();
    let (kind, tag_id, tone, text) = if ownership_sale {
        (
            "nova_diretoria",
            "market",
            "reforma",
            rust_i18n::t!("world_footer.team_state.new_board", team = nome).to_string(),
        )
    } else if in_crisis {
        let text = if team.financial_state == "collapse" || in_debt {
            rust_i18n::t!("world_footer.team_state.debt", team = nome).to_string()
        } else {
            rust_i18n::t!("world_footer.team_state.tight_budget", team = nome).to_string()
        };
        ("crise_financeira", "finance", "crise", text)
    } else if bad_mood {
        (
            "clima_pesado",
            "backstage",
            "alerta",
            rust_i18n::t!("world_footer.team_state.bad_mood", team = nome).to_string(),
        )
    } else {
        return None;
    };

    Some(WorldNote {
        id: format!("team:{}:{}", team.id, kind),
        tag: tag_label(tag_id),
        subject: team.nome,
        kind: kind.to_string(),
        tone: tone.to_string(),
        text,
    })
}

/// Nota sobre um ex-companheiro (piloto) — só quando há NOTÍCIA: o time atual dele
/// passa por dificuldade. Sem notícia → `None` (piloto correndo não é manchete).
/// `skip_team_ids` evita repetir uma crise já contada como nota de equipe.
pub(super) fn teammate_news_note(
    conn: &rusqlite::Connection,
    mate_id: &str,
    categoria: &str,
    skip_team_ids: &HashSet<String>,
) -> Option<WorldNote> {
    use crate::db::queries::{contracts, drivers, teams};

    let mate = drivers::get_driver(conn, mate_id).ok()?;
    if mate.status != DriverStatus::Ativo || mate.categoria_atual.as_deref() != Some(categoria) {
        return None;
    }
    let contract = contracts::get_active_contract_for_pilot(conn, mate_id)
        .ok()
        .flatten()?;
    if skip_team_ids.contains(&contract.equipe_id) {
        return None; // a crise desse time já virou nota própria.
    }
    let team = teams::get_team_by_id(conn, &contract.equipe_id).ok().flatten()?;
    let in_crisis =
        matches!(team.financial_state.as_str(), "crisis" | "collapse") || team.debt_balance > 0.0;
    if !in_crisis {
        return None;
    }

    Some(WorldNote {
        id: format!("mate:{mate_id}"),
        tag: tag_label("backstage"),
        subject: mate.nome.clone(),
        kind: "piloto_time_crise".to_string(),
        tone: "alerta".to_string(),
        text: rust_i18n::t!(
            "world_footer.teammate.team_crisis",
            mate = mate.nome.as_str(),
            team = contract.equipe_nome.as_str()
        )
        .to_string(),
    })
}
