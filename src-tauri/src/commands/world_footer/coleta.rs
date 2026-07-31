//! A cascata de seleção das notas: jogador → líderes → astro → recordes.

use super::*;

/// Reúne as notas do rodapé para o save aberto (cascata jogador → líderes → astro → recordes).
pub(super) fn collect_world_notes(conn: &rusqlite::Connection) -> Vec<WorldNote> {
    use crate::db::queries::{contracts, drivers, race_history, seasons};

    let player = drivers::get_player_driver(conn).ok();

    // Categoria atual = do contrato ativo do jogador; senão, do campo do piloto.
    let categoria = player
        .as_ref()
        .and_then(|p| {
            contracts::get_active_contract_for_pilot(conn, &p.id)
                .ok()
                .flatten()
                .map(|c| c.categoria)
                .or_else(|| p.categoria_atual.clone())
        })
        .unwrap_or_default();
    if categoria.is_empty() {
        return Vec::new();
    }

    // Âncoras de seleção: o jogador, depois o 1º e o 2º do campeonato da categoria.
    let mut anchors: Vec<String> = Vec::new();
    let mut anchor_seen = HashSet::new();
    if let Some(p) = &player {
        if anchor_seen.insert(p.id.clone()) {
            anchors.push(p.id.clone());
        }
    }
    let active_season = seasons::get_active_season(conn).ok().flatten();
    let current_season_num = active_season.as_ref().map(|s| s.numero).unwrap_or(0);
    if let Some(season) = &active_season {
        if let Ok(standings) = race_history::get_category_standings(conn, &season.id, &categoria) {
            for e in standings.iter().take(2) {
                if anchor_seen.insert(e.pilot_id.clone()) {
                    anchors.push(e.pilot_id.clone());
                }
            }
        }
    }

    let mut notes: Vec<WorldNote> = Vec::new();
    let mut used_teams: HashSet<String> = HashSet::new();
    let mut used_drivers: HashSet<String> = HashSet::new();

    // Passo 1 — equipes: ex-times das âncoras, com estado digno de nota.
    for anchor in &anchors {
        if notes.len() >= TARGET_NOTES {
            break;
        }
        for team_id in pilot_ex_team_ids(conn, anchor) {
            if used_teams.contains(&team_id) {
                continue;
            }
            if let Some(note) = team_state_note(conn, &team_id, &categoria) {
                used_teams.insert(team_id);
                notes.push(note);
                if notes.len() >= TARGET_NOTES {
                    break;
                }
            }
        }
    }

    // Passo 2 — pilotos: ex-companheiros das âncoras, só quando há notícia.
    for anchor in &anchors {
        if notes.len() >= TARGET_NOTES {
            break;
        }
        if let Ok(mates) = contracts::get_former_teammates(conn, anchor) {
            for (mate_id, _) in mates {
                if used_drivers.contains(&mate_id) {
                    continue;
                }
                if let Some(note) = teammate_news_note(conn, &mate_id, &categoria, &used_teams) {
                    used_drivers.insert(mate_id);
                    notes.push(note);
                    if notes.len() >= TARGET_NOTES {
                        break;
                    }
                }
            }
        }
    }

    // Passo 3 — ASTRO da categoria (Fase 3 do Estrelato): o maior nome de público, se
    // houver um de verdade (fama Estrela+). Vem antes dos recordes — a fama que enche
    // arquibancada é notícia por si só.
    if notes.len() < TARGET_NOTES {
        if let Some(note) = star_of_category_note(conn, &categoria, &mut used_drivers) {
            notes.push(note);
        }
    }

    // Passo 4 — recordes. Primeiro os RECÉM-QUEBRADOS (com data, mais fortes),
    // depois os que estão A CAMINHO preenchem o que faltar.
    if notes.len() < TARGET_NOTES {
        let budget = TARGET_NOTES - notes.len();
        notes.extend(record_broken_notes(
            conn,
            &categoria,
            current_season_num,
            &mut used_drivers,
            budget,
        ));
    }
    if notes.len() < TARGET_NOTES {
        let budget = TARGET_NOTES - notes.len();
        notes.extend(record_watch_notes(
            conn,
            &categoria,
            &mut used_drivers,
            budget,
        ));
    }

    notes.truncate(MAX_NOTES);
    notes
}
