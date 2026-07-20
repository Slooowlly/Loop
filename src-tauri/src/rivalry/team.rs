#![allow(dead_code)]
//! Motor de rivalidade entre EQUIPES (Fase 1: fundação).
//!
//! Gêmeo enxuto de [`crate::rivalry`] (piloto↔piloto) para o par de TIMES. Reusa o núcleo
//! puro de `models::rivalry` (`perceived_intensity`, `rivalry_lifecycle`, `normalize_pair`)
//! — que é agnóstico de piloto — e a camada de persistência `db::queries::team_rivalries`.
//!
//! Esta fase entrega SÓ o mecanismo: aplicar um evento (upsert nos dois eixos), ler por
//! time e decair no fim da temporada. As FONTES que geram os eventos (briga de
//! construtores, roubo de talento, guerra na pista, transbordamento de piloto) e as
//! CONSEQUÊNCIAS (manchete, moral de derby) entram nas fases seguintes.
//!
//! Ver `docs/superpowers/specs/2026-07-19-team-rivalry-design.md`.

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::db::queries::team_rivalries::{
    delete_team_rivalry, get_all_team_rivalries, get_team_rivalries_for_team,
    get_team_rivalry_by_pair, insert_team_rivalry, update_team_rivalry_axes,
};
use crate::generators::ids::{next_id, IdType};
use crate::models::rivalry::{normalize_pair, perceived_intensity, rivalry_lifecycle, RivalryLifecycle};
use crate::models::team_rivalry::{TeamRivalry, TeamRivalryType};

// ── Constantes de domínio ─────────────────────────────────────────────────────

const AXIS_MAX: f64 = 100.0;
const AXIS_MIN: f64 = 0.0;

fn clamp(v: f64) -> f64 {
    v.clamp(AXIS_MIN, AXIS_MAX)
}

// ── Evento ────────────────────────────────────────────────────────────────────

/// Um reforço de rivalidade entre dois times. Os deltas seguem a mesma escala do sistema
/// de piloto (recente aquece rápido, histórico é memória).
pub struct TeamRivalryEvent {
    pub team_a: String,
    pub team_b: String,
    /// Origem — define o tipo se a rivalidade for nova (preservado nos reforços).
    pub tipo: TeamRivalryType,
    pub historical_delta: f64,
    pub recent_delta: f64,
    pub temporada: i32,
}

/// Resultado de [`apply_team_rivalry_event`] — a percebida antes/depois, para as fases
/// seguintes decidirem manchete por cruzamento de threshold.
pub struct TeamRivalryApplied {
    pub rivalry_id: String,
    pub old_perceived: f64,
    pub new_perceived: f64,
}

// ── Upsert com dois eixos ─────────────────────────────────────────────────────

/// Aplica um evento de rivalidade entre times: cria a rivalidade ou reforça a existente
/// (par normalizado). Idêntico em espírito ao `apply_rivalry_event` de piloto, incluindo
/// o tratamento da corrida de constraint no par único.
pub fn apply_team_rivalry_event(
    conn: &Connection,
    event: &TeamRivalryEvent,
) -> Result<TeamRivalryApplied, DbError> {
    let pair = match normalize_pair(&event.team_a, &event.team_b) {
        Some(p) => p,
        None => {
            return Ok(TeamRivalryApplied {
                rivalry_id: String::new(),
                old_perceived: 0.0,
                new_perceived: 0.0,
            });
        }
    };
    // `normalize_pair` devolve o par ordenado nos campos `piloto1_id/piloto2_id` — aqui
    // eles carregam os ids de TIME (a função é puramente ordenação de strings).
    let team1_id = pair.piloto1_id;
    let team2_id = pair.piloto2_id;
    let now = current_timestamp();

    match get_team_rivalry_by_pair(conn, &team1_id, &team2_id)? {
        Some(existing) => {
            let old_perceived = existing.perceived_intensity();
            let new_historical = clamp(existing.historical_intensity + event.historical_delta);
            let new_recent = clamp(existing.recent_activity + event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            update_team_rivalry_axes(
                conn,
                &existing.id,
                new_historical,
                new_recent,
                &now,
                event.temporada,
            )?;
            Ok(TeamRivalryApplied {
                rivalry_id: existing.id,
                old_perceived,
                new_perceived,
            })
        }
        None => {
            let id = next_id(conn, IdType::TeamRivalry)?;
            let new_historical = clamp(event.historical_delta);
            let new_recent = clamp(event.recent_delta);
            let new_perceived = perceived_intensity(new_historical, new_recent);
            let rivalry = TeamRivalry {
                id: id.clone(),
                team1_id: team1_id.clone(),
                team2_id: team2_id.clone(),
                historical_intensity: new_historical,
                recent_activity: new_recent,
                tipo: event.tipo.clone(),
                criado_em: now.clone(),
                ultima_atualizacao: now,
                temporada_update: event.temporada,
            };
            match insert_team_rivalry(conn, &rivalry) {
                Ok(()) => Ok(TeamRivalryApplied {
                    rivalry_id: id,
                    old_perceived: 0.0,
                    new_perceived,
                }),
                // Corrida: outro caminho criou o par entre o get e o insert → recarrega e reforça.
                Err(err) if is_pair_constraint(&err) => {
                    let existing = get_team_rivalry_by_pair(conn, &team1_id, &team2_id)?
                        .ok_or_else(|| {
                            DbError::InvalidData(format!(
                                "Par de rivalidade de equipe '{team1_id}' x '{team2_id}' conflitou no insert, mas nao foi encontrado no reload"
                            ))
                        })?;
                    let old_perceived = existing.perceived_intensity();
                    let new_historical =
                        clamp(existing.historical_intensity + event.historical_delta);
                    let new_recent = clamp(existing.recent_activity + event.recent_delta);
                    let new_perceived = perceived_intensity(new_historical, new_recent);
                    update_team_rivalry_axes(
                        conn,
                        &existing.id,
                        new_historical,
                        new_recent,
                        &current_timestamp(),
                        event.temporada,
                    )?;
                    Ok(TeamRivalryApplied {
                        rivalry_id: existing.id,
                        old_perceived,
                        new_perceived,
                    })
                }
                Err(err) => Err(err),
            }
        }
    }
}

// ── Leitura por time ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TeamRivalrySummary {
    pub rivalry_id: String,
    /// O "outro lado" do par, do ponto de vista do time consultado.
    pub rival_id: String,
    pub historical_intensity: f64,
    pub recent_activity: f64,
    pub perceived_intensity: f64,
    pub tipo: TeamRivalryType,
    pub ultima_atualizacao: String,
}

pub fn get_team_rivalries(
    conn: &Connection,
    team_id: &str,
) -> Result<Vec<TeamRivalrySummary>, DbError> {
    let rivalries = get_team_rivalries_for_team(conn, team_id)?;
    let summaries = rivalries
        .into_iter()
        .map(|r| {
            let rival_id = if r.team1_id == team_id {
                r.team2_id.clone()
            } else {
                r.team1_id.clone()
            };
            let perceived = r.perceived_intensity();
            TeamRivalrySummary {
                rivalry_id: r.id,
                rival_id,
                historical_intensity: r.historical_intensity,
                recent_activity: r.recent_activity,
                perceived_intensity: perceived,
                tipo: r.tipo,
                ultima_atualizacao: r.ultima_atualizacao,
            }
        })
        .collect();
    Ok(summaries)
}

// ── Decaimento de fim de temporada ────────────────────────────────────────────

/// Aplica o decaimento anual a todas as rivalidades de equipe (mesma regra do piloto):
/// - Ativa nesta temporada (`temporada_update == atual`): `recent *= 0.5`, histórico intacto.
/// - Inativa: `recent *= 0.2`, `historical *= 0.85`.
/// - Ciclo de vida `Extinta` → removida do banco.
///
/// Deve ser chamada uma vez no pipeline de fim de temporada.
pub fn apply_season_end_team_rivalry_decay(
    conn: &Connection,
    temporada_atual: i32,
) -> Result<(), DbError> {
    let all = get_all_team_rivalries(conn)?;
    let now = current_timestamp();

    for r in all {
        let (new_historical, new_recent) = if r.temporada_update == temporada_atual {
            (r.historical_intensity, r.recent_activity * 0.5)
        } else {
            (r.historical_intensity * 0.85, r.recent_activity * 0.2)
        };

        if matches!(
            rivalry_lifecycle(new_historical, new_recent),
            RivalryLifecycle::Extinta
        ) {
            delete_team_rivalry(conn, &r.id)?;
        } else {
            update_team_rivalry_axes(
                conn,
                &r.id,
                new_historical,
                new_recent,
                &now,
                r.temporada_update,
            )?;
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_pair_constraint(err: &DbError) -> bool {
    matches!(
        err,
        DbError::Sqlite(rusqlite::Error::SqliteFailure(inner, _))
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_all(&conn).unwrap();
        conn
    }

    fn event(a: &str, b: &str, tipo: TeamRivalryType, h: f64, r: f64) -> TeamRivalryEvent {
        TeamRivalryEvent {
            team_a: a.to_string(),
            team_b: b.to_string(),
            tipo,
            historical_delta: h,
            recent_delta: r,
            temporada: 1,
        }
    }

    #[test]
    fn cria_rivalidade_nova_normalizando_o_par() {
        let conn = setup_db();
        // Passa fora de ordem de propósito — deve normalizar (T003 < T020).
        let applied = apply_team_rivalry_event(
            &conn,
            &event("T020", "T003", TeamRivalryType::Campeonato, 10.0, 20.0),
        )
        .unwrap();
        // 0.4*10 + 0.6*20 = 16.0
        assert!((applied.new_perceived - 16.0).abs() < 1e-9);
        assert!(applied.old_perceived.abs() < 1e-9);

        let summ = get_team_rivalries(&conn, "T003").unwrap();
        assert_eq!(summ.len(), 1);
        assert_eq!(summ[0].rival_id, "T020");
        assert_eq!(summ[0].tipo, TeamRivalryType::Campeonato);
    }

    #[test]
    fn reforco_acumula_nos_dois_eixos_e_preserva_tipo() {
        let conn = setup_db();
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Mercado, 10.0, 20.0),
        )
        .unwrap();
        // Reforço com outro tipo — o tipo ORIGINAL (Mercado) deve permanecer.
        let applied = apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Pista, 10.0, 20.0),
        )
        .unwrap();
        // acumulado h=20, r=40 → perceived = 0.4*20 + 0.6*40 = 32
        assert!((applied.new_perceived - 32.0).abs() < 1e-9);
        assert!((applied.old_perceived - 16.0).abs() < 1e-9);

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        assert_eq!(summ[0].tipo, TeamRivalryType::Mercado);
    }

    #[test]
    fn clamp_nao_passa_de_100() {
        let conn = setup_db();
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Pista, 70.0, 70.0),
        )
        .unwrap();
        let applied = apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Pista, 70.0, 70.0),
        )
        .unwrap();
        assert!((applied.new_perceived - 100.0).abs() < 1e-9);
    }

    #[test]
    fn mesmo_time_ignorado() {
        let conn = setup_db();
        let applied = apply_team_rivalry_event(
            &conn,
            &event("T001", "T001", TeamRivalryType::Campeonato, 50.0, 50.0),
        )
        .unwrap();
        assert!(applied.rivalry_id.is_empty());
        assert!(get_team_rivalries(&conn, "T001").unwrap().is_empty());
    }

    #[test]
    fn decay_ativa_esfria_recente_mantem_historico() {
        let conn = setup_db();
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Campeonato, 20.0, 40.0),
        )
        .unwrap();
        apply_season_end_team_rivalry_decay(&conn, 1).unwrap();

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        assert_eq!(summ.len(), 1);
        // h intacto (20), r = 40*0.5 = 20
        assert!((summ[0].historical_intensity - 20.0).abs() < 1e-9);
        assert!((summ[0].recent_activity - 20.0).abs() < 1e-9);
    }

    #[test]
    fn decay_inativa_decai_nos_dois_eixos() {
        let conn = setup_db();
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Campeonato, 20.0, 40.0),
        )
        .unwrap();
        // Decaimento de uma temporada POSTERIOR à do reforço → inativa.
        apply_season_end_team_rivalry_decay(&conn, 2).unwrap();

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        // h = 20*0.85 = 17, r = 40*0.2 = 8
        assert!((summ[0].historical_intensity - 17.0).abs() < 1e-9);
        assert!((summ[0].recent_activity - 8.0).abs() < 1e-9);
    }

    #[test]
    fn decay_extinta_e_removida() {
        let conn = setup_db();
        // Rivalidade fraca; após decaimento inativo cai abaixo do limiar de extinção.
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Pista, 3.0, 5.0),
        )
        .unwrap();
        apply_season_end_team_rivalry_decay(&conn, 5).unwrap();
        assert!(get_team_rivalries(&conn, "T001").unwrap().is_empty());
    }

    #[test]
    fn ids_sao_sequenciais_com_prefixo_trv() {
        let conn = setup_db();
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Campeonato, 5.0, 12.0),
        )
        .unwrap();
        apply_team_rivalry_event(
            &conn,
            &event("T003", "T004", TeamRivalryType::Campeonato, 5.0, 12.0),
        )
        .unwrap();
        let first = get_team_rivalries(&conn, "T001").unwrap()[0].rivalry_id.clone();
        let second = get_team_rivalries(&conn, "T003").unwrap()[0].rivalry_id.clone();
        assert!(first.starts_with("TRV"), "id foi {first}");
        assert!(second.starts_with("TRV"), "id foi {second}");
        assert_ne!(first, second);
    }
}
