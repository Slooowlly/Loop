//! Foco da equipe (ideia 4 "Foco + Vínculo", Fase 1).
//!
//! A **fase/filosofia atual** do time — um de 6 focos. NÃO é uma personalidade
//! eterna (pedido do usuário): **deriva** do estado que o jogo já calcula
//! (`financial_state` + `strategic_plan` do Pilar C + reputação), **vira em eventos**
//! (promoção/rebaixamento/venda/título) e tem **histerese** (um dwell mínimo antes de
//! trocar), de modo que uma fase dura uma "era" (~2+ temporadas) e então muda.
//!
//! Nesta fase o módulo só deriva e persiste o foco (com a notícia da virada). As
//! consequências (renovação leal, segurar-vs-vender) penduram nele depois.

use rusqlite::Connection;

use crate::constants::categories::get_category_config;
use crate::models::team::Team;

/// Dwell mínimo (temporadas) numa fase antes de trocar por deriva "suave". Um evento
/// duro (promoção/rebaixamento/venda/título) fura a histerese e troca na hora.
const MIN_DWELL: i32 = 2;
/// Abaixo desta reputação, um time saudável sem plano agressivo é lido como "celeiro"
/// (constrói via jovem); acima, "meio de grid ambicioso" (estabelecido querendo subir).
const CELEIRO_REPUTATION_CEIL: f64 = 45.0;

/// Os 6 focos possíveis. Ordem/nomes conforme o design doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamFocus {
    Sobrevivencia,
    Reconstrucao,
    Celeiro,
    MeioDeGrid,
    ProjetoDeTitulo,
    Dinastia,
}

impl TeamFocus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TeamFocus::Sobrevivencia => "sobrevivencia",
            TeamFocus::Reconstrucao => "reconstrucao",
            TeamFocus::Celeiro => "celeiro",
            TeamFocus::MeioDeGrid => "meio_de_grid",
            TeamFocus::ProjetoDeTitulo => "projeto_de_titulo",
            TeamFocus::Dinastia => "dinastia",
        }
    }

    /// Rótulo exibível (ficha do time / notícia).
    ///
    /// NOTA: `Dinastia` é DISFARÇADA como "Projeto de título" de propósito — o rótulo
    /// "Dinastia" gritaria pro jogador "essa é a equipe forte, entra aqui". Por dentro
    /// os dois focos seguem distintos (as_str/lógica), mas pro usuário parecem iguais.
    pub fn label(&self) -> &'static str {
        match self {
            TeamFocus::Sobrevivencia => "Sobrevivência",
            TeamFocus::Reconstrucao => "Reconstrução",
            TeamFocus::Celeiro => "Celeiro de talentos",
            TeamFocus::MeioDeGrid => "Meio de grid ambicioso",
            TeamFocus::ProjetoDeTitulo => "Projeto de título",
            TeamFocus::Dinastia => "Projeto de título",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value.trim() {
            "sobrevivencia" => TeamFocus::Sobrevivencia,
            "reconstrucao" => TeamFocus::Reconstrucao,
            "celeiro" => TeamFocus::Celeiro,
            "projeto_de_titulo" => TeamFocus::ProjetoDeTitulo,
            "dinastia" => TeamFocus::Dinastia,
            _ => TeamFocus::MeioDeGrid,
        }
    }
}

/// O foco "natural" que o estado atual do time pede (antes da histerese).
pub fn derive_natural_focus(
    financial_state: &str,
    plan_type: &str,
    reputacao: f64,
    is_elite: bool,
    was_relegated: bool,
) -> TeamFocus {
    if matches!(financial_state, "crisis" | "collapse") {
        return TeamFocus::Sobrevivencia;
    }
    if was_relegated || plan_type == "rebuild" {
        return TeamFocus::Reconstrucao;
    }
    if is_elite || plan_type == "elite_dominance" {
        return TeamFocus::Dinastia;
    }
    if plan_type == "title_push" {
        return TeamFocus::ProjetoDeTitulo;
    }
    if reputacao < CELEIRO_REPUTATION_CEIL {
        return TeamFocus::Celeiro;
    }
    TeamFocus::MeioDeGrid
}

/// Aplica a histerese: troca só se houve evento duro OU se o time já cumpriu o dwell
/// mínimo na fase atual. Retorna (novo_foco, novo_dwell, mudou).
pub fn resolve_focus(
    current: TeamFocus,
    dwell: i32,
    natural: TeamFocus,
    hard_event: bool,
    min_dwell: i32,
) -> (TeamFocus, i32, bool) {
    if natural == current {
        return (current, dwell + 1, false);
    }
    if hard_event || dwell >= min_dwell {
        (natural, 0, true)
    } else {
        (current, dwell + 1, false)
    }
}

/// `true` se a categoria anterior era de um tier MAIOR que a atual (rebaixamento).
pub fn was_relegated_from(categoria_anterior: Option<&str>, categoria_atual: &str) -> bool {
    let Some(prev) = categoria_anterior else {
        return false;
    };
    let tier = |c: &str| get_category_config(c).map(|cfg| cfg.tier).unwrap_or(0);
    tier(prev) > tier(categoria_atual)
}

/// Foco atual persistido do time (default `meio_de_grid`, dwell 0 se não houver registro).
pub fn get_focus(conn: &Connection, team_id: &str) -> Result<(TeamFocus, i32), String> {
    let row = conn
        .query_row(
            "SELECT foco, temporadas FROM team_focus WHERE team_id = ?1",
            rusqlite::params![team_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?)),
        )
        .map(|(f, t)| (TeamFocus::from_str(&f), t));
    match row {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok((TeamFocus::MeioDeGrid, 0)),
        Err(e) => Err(format!("Falha ao ler foco da equipe: {e}")),
    }
}

fn set_focus(conn: &Connection, team_id: &str, foco: TeamFocus, dwell: i32) -> Result<(), String> {
    conn.execute(
        "INSERT INTO team_focus (team_id, foco, temporadas) VALUES (?1, ?2, ?3)
         ON CONFLICT(team_id) DO UPDATE SET foco = ?2, temporadas = ?3",
        rusqlite::params![team_id, foco.as_str(), dwell],
    )
    .map_err(|e| format!("Falha ao gravar foco da equipe: {e}"))?;
    Ok(())
}

/// Atualiza o foco de um time na pré-temporada. Retorna `Some(novo_foco)` se mudou
/// (para a notícia da virada), `None` se manteve.
pub(crate) fn update_team_focus(
    conn: &Connection,
    team: &Team,
    is_elite: bool,
    plan_type: &str,
    hard_event: bool,
) -> Result<Option<TeamFocus>, String> {
    let (current, dwell) = get_focus(conn, &team.id)?;
    let was_relegated = was_relegated_from(team.categoria_anterior.as_deref(), &team.categoria);
    let natural = derive_natural_focus(
        &team.financial_state,
        plan_type,
        team.reputacao,
        is_elite,
        was_relegated,
    );
    let (next, next_dwell, changed) = resolve_focus(current, dwell, natural, hard_event, MIN_DWELL);
    set_focus(conn, &team.id, next, next_dwell)?;
    Ok(if changed { Some(next) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crisis_forces_survival_and_relegation_forces_rebuild() {
        assert_eq!(
            derive_natural_focus("collapse", "title_push", 80.0, true, false),
            TeamFocus::Sobrevivencia
        );
        assert_eq!(
            derive_natural_focus("healthy", "sustainable", 80.0, false, true),
            TeamFocus::Reconstrucao
        );
    }

    #[test]
    fn elite_is_dynasty_and_title_push_is_title_project() {
        assert_eq!(
            derive_natural_focus("elite", "elite_dominance", 90.0, true, false),
            TeamFocus::Dinastia
        );
        assert_eq!(
            derive_natural_focus("healthy", "title_push", 70.0, false, false),
            TeamFocus::ProjetoDeTitulo
        );
    }

    #[test]
    fn low_reputation_builder_is_celeiro_mid_is_ambitious() {
        assert_eq!(
            derive_natural_focus("stable", "sustainable", 30.0, false, false),
            TeamFocus::Celeiro
        );
        assert_eq!(
            derive_natural_focus("stable", "sustainable", 60.0, false, false),
            TeamFocus::MeioDeGrid
        );
    }

    #[test]
    fn hysteresis_holds_the_focus_until_min_dwell() {
        // Foco natural mudou, mas o time está há só 1 temporada na fase → segura.
        let (f, dwell, changed) = resolve_focus(
            TeamFocus::Celeiro,
            1,
            TeamFocus::MeioDeGrid,
            false,
            MIN_DWELL,
        );
        assert_eq!((f, changed), (TeamFocus::Celeiro, false));
        assert_eq!(dwell, 2);
        // Na temporada seguinte já cumpriu o dwell → troca.
        let (f2, dwell2, changed2) = resolve_focus(
            TeamFocus::Celeiro,
            2,
            TeamFocus::MeioDeGrid,
            false,
            MIN_DWELL,
        );
        assert_eq!((f2, dwell2, changed2), (TeamFocus::MeioDeGrid, 0, true));
    }

    #[test]
    fn hard_event_pierces_hysteresis_immediately() {
        // Rebaixamento (evento duro) troca na hora, mesmo com dwell baixo.
        let (f, dwell, changed) = resolve_focus(
            TeamFocus::Dinastia,
            0,
            TeamFocus::Reconstrucao,
            true,
            MIN_DWELL,
        );
        assert_eq!((f, dwell, changed), (TeamFocus::Reconstrucao, 0, true));
    }

    #[test]
    fn staying_in_focus_ages_the_dwell() {
        let (f, dwell, changed) =
            resolve_focus(TeamFocus::Dinastia, 3, TeamFocus::Dinastia, false, MIN_DWELL);
        assert_eq!((f, dwell, changed), (TeamFocus::Dinastia, 4, false));
    }

    #[test]
    fn persistence_round_trips_and_defaults() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::db::migrations::run_all(&conn).expect("schema");
        // Default sem registro.
        assert_eq!(get_focus(&conn, "T1").unwrap(), (TeamFocus::MeioDeGrid, 0));
        set_focus(&conn, "T1", TeamFocus::Dinastia, 2).unwrap();
        assert_eq!(get_focus(&conn, "T1").unwrap(), (TeamFocus::Dinastia, 2));
    }
}
