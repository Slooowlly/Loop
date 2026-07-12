//! Caixa de entrada (dossiê pessoal da temporada). Monta FATOS reais do save —
//! confronto direto com um rival do grid e o favorito ao título — e deixa a prosa
//! em PT para o front (inboxMessages.js), seguindo o padrão "Rust = dados, JS = voz".
//! Read-only: nada aqui altera o estado do jogo.

use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::{
    contracts as cq, drivers as dq, race_history as rh, seasons as sq, teams as tq,
};
use crate::models::driver::{Driver, DriverAttributes};

#[derive(serde::Serialize, Default)]
pub struct InboxFacts {
    /// Categoria atual do jogador (None = sem carreira/categoria → caixa vazia).
    pub category: Option<String>,
    pub head_to_head: Option<HeadToHeadFact>,
    pub title_favorite: Option<TitleFavoriteFact>,
    /// Fase 2a do estrelato: equipes cobiçando o nome do jogador pela FAMA (apelo
    /// comercial). None quando ninguém tem interesse ativo (fama baixa).
    pub team_interest: Option<TeamInterestFact>,
}

/// "Times de olho em você": equipes com interesse ATIVO no jogador por conta da
/// fama (apelo comercial ponderado pela necessidade financeira). É o aviso de
/// poder de barganha — na Janela essas equipes ofertam N1 + salário-prêmio.
#[derive(serde::Serialize)]
pub struct TeamInterestFact {
    /// Fama atual do jogador (0–100) — o JS traduz para o nível (Estrela/Ídolo…).
    pub player_fama: u8,
    /// Times interessados, do maior apelo comercial para o menor.
    pub teams: Vec<InterestedTeam>,
}

#[derive(serde::Serialize)]
pub struct InterestedTeam {
    pub team_name: String,
    pub category: String,
}

/// "Já cruzei com esse cara": confronto direto com o rival de história mais longa
/// no grid atual.
#[derive(serde::Serialize)]
pub struct HeadToHeadFact {
    pub rival_name: String,
    pub rival_team: Option<String>,
    pub races_together: i32,
    pub player_ahead: i32,
    /// Melhor posição do jogador numa corrida em que bateu o rival (para citar).
    pub best_finish: Option<i32>,
    pub best_track: Option<String>,
}

/// "O favorito ao título": o nome a bater na temporada.
#[derive(serde::Serialize)]
pub struct TitleFavoriteFact {
    pub driver_name: String,
    pub driver_team: Option<String>,
    /// Posição nos standings. 0 = pré-temporada (sem corridas ainda).
    pub position: i32,
    /// Está à frente do jogador no campeonato?
    pub leads_player: bool,
    /// Vantagem (pontos) sobre o 2º colocado — só faz sentido se `position == 1`.
    pub points_lead: i32,
    pub player_position: i32,
    pub career_titles: i32,
    pub age: i32,
    pub veteran: bool,
    /// Chaves de atributo (forte/fraco) — o front mapeia para PT.
    pub strong_attr: String,
    pub weak_attr: String,
}

#[tauri::command]
pub fn get_inbox_messages(app: tauri::AppHandle, career_id: String) -> Result<InboxFacts, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    if !db_path.exists() {
        return Err(format!("Save não encontrado: {career_id}"));
    }
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;
    let conn = &db.conn;

    let player = dq::get_player_driver(conn).map_err(|e| format!("Falha ao carregar jogador: {e}"))?;
    let categoria = match player.categoria_atual.clone() {
        Some(c) if !c.is_empty() => c,
        _ => return Ok(InboxFacts::default()),
    };

    let mut facts = InboxFacts {
        category: Some(categoria.clone()),
        ..Default::default()
    };

    // ── Confronto direto com o rival mais enfrentado do grid atual ──────────────
    if let Some(rival_id) = rh::most_faced_rival(conn, &player.id, &categoria)
        .map_err(|e| format!("Falha no confronto direto: {e}"))?
    {
        let h = rh::get_head_to_head(conn, &player.id, &rival_id, &categoria)
            .map_err(|e| format!("Falha no confronto direto: {e}"))?;
        if h.races_together > 0 {
            if let Ok(rival) = dq::get_driver(conn, &rival_id) {
                facts.head_to_head = Some(HeadToHeadFact {
                    rival_name: rival.nome.clone(),
                    rival_team: team_name_for(conn, &rival_id),
                    races_together: h.races_together,
                    player_ahead: h.player_ahead,
                    best_finish: h.best_finish,
                    best_track: h.best_track,
                });
            }
        }
    }

    // ── Favorito ao título: líder do campeonato (ou, se for você, o vice) ────────
    if let Ok(Some(season)) = sq::get_active_season(conn) {
        if let Ok(standings) = rh::get_category_standings(conn, &season.id, &categoria) {
            if !standings.is_empty() {
                let player_pos = standings
                    .iter()
                    .position(|s| s.pilot_id == player.id)
                    .map(|i| i as i32 + 1)
                    .unwrap_or(0);
                if let Some((idx, entry)) =
                    standings.iter().enumerate().find(|(_, s)| s.pilot_id != player.id)
                {
                    if let Ok(fav) = dq::get_driver(conn, &entry.pilot_id) {
                        let leads_player = player_pos == 0 || entry.position < player_pos;
                        let points_lead = standings
                            .get(idx + 1)
                            .map(|n| (entry.points - n.points).round() as i32)
                            .unwrap_or(0)
                            .max(0);
                        facts.title_favorite =
                            Some(build_favorite(conn, &fav, entry.position, leads_player, points_lead, player_pos));
                    }
                }
            }
        }
    }

    // ── Pré-temporada (sem standings ainda): o mais condecorado do grid ──────────
    if facts.title_favorite.is_none() {
        if let Ok(drivers) = dq::get_drivers_by_active_category(conn, &categoria) {
            let best = drivers
                .into_iter()
                .filter(|d| !d.is_jogador)
                .max_by_key(|d| (d.stats_carreira.titulos, d.stats_carreira.vitorias));
            if let Some(fav) = best {
                if fav.stats_carreira.titulos > 0 || fav.stats_carreira.vitorias > 0 {
                    facts.title_favorite = Some(build_favorite(conn, &fav, 0, true, 0, 0));
                }
            }
        }
    }

    // ── Times de olho no jogador pela fama (Fase 2a) ────────────────────────────
    facts.team_interest = build_team_interest(conn, &player);

    Ok(facts)
}

/// Os poucos MELHORES times que cobiçam o jogador pela fama — mesma seleção usada
/// nas ofertas destacadas da Janela (`pipeline::player_active_interest_teams`), pra
/// o e-mail e o card baterem. None quando a fama ainda não atrai ninguém.
fn build_team_interest(conn: &rusqlite::Connection, player: &Driver) -> Option<TeamInterestFact> {
    let teams = crate::market::pipeline::player_active_interest_teams(conn, player).ok()?;
    if teams.is_empty() {
        return None;
    }
    Some(TeamInterestFact {
        player_fama: player.atributos.midia.round().clamp(0.0, 100.0) as u8,
        teams: teams
            .into_iter()
            .map(|(_, team_name, category)| InterestedTeam {
                team_name,
                category,
            })
            .collect(),
    })
}

fn build_favorite(
    conn: &rusqlite::Connection,
    fav: &Driver,
    position: i32,
    leads_player: bool,
    points_lead: i32,
    player_position: i32,
) -> TitleFavoriteFact {
    let (strong_attr, weak_attr) = strong_weak(&fav.atributos);
    let veteran = fav.atributos.experiencia >= 70.0
        || fav.temporadas_na_categoria >= 4
        || fav.stats_carreira.titulos >= 1;
    TitleFavoriteFact {
        driver_name: fav.nome.clone(),
        driver_team: team_name_for(conn, &fav.id),
        position,
        leads_player,
        points_lead,
        player_position,
        career_titles: fav.stats_carreira.titulos as i32,
        age: fav.idade as i32,
        veteran,
        strong_attr,
        weak_attr,
    }
}

/// Nome da equipe atual de um piloto (via contrato regular ativo). None se sem time.
fn team_name_for(conn: &rusqlite::Connection, pilot_id: &str) -> Option<String> {
    cq::get_active_contract_for_pilot(conn, pilot_id)
        .ok()
        .flatten()
        .and_then(|c| tq::get_team_by_id(conn, &c.equipe_id).ok().flatten())
        .map(|t| t.nome)
}

/// Maior e menor atributo "de pista" do piloto — vira "forte na X, vulnerável na Y".
fn strong_weak(a: &DriverAttributes) -> (String, String) {
    let set = [
        ("ritmo_classificacao", a.ritmo_classificacao),
        ("gestao_pneus", a.gestao_pneus),
        ("racecraft", a.racecraft),
        ("defesa", a.defesa),
        ("habilidade_largada", a.habilidade_largada),
        ("consistencia", a.consistencia),
    ];
    let strong = set.iter().fold(set[0], |m, x| if x.1 > m.1 { *x } else { m });
    let weak = set.iter().fold(set[0], |m, x| if x.1 < m.1 { *x } else { m });
    (strong.0.to_string(), weak.0.to_string())
}
