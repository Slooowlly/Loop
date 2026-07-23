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

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::common::time::current_timestamp;
use crate::db::connection::DbError;
use crate::db::queries::news::insert_news;
use crate::db::queries::team_rivalries::{
    delete_team_rivalry, get_all_team_rivalries, get_team_rivalries_for_team,
    get_team_rivalry_by_pair, insert_team_rivalry, update_team_rivalry_axes,
};
use crate::db::queries::teams as team_queries;
use crate::generators::ids::{next_id, IdType};
use crate::models::driver::Driver;
use crate::models::rivalry::{
    normalize_pair, perceived_intensity, rivalry_lifecycle, RivalryLifecycle,
};
use crate::models::team_rivalry::{TeamRivalry, TeamRivalryType};
use crate::news::{NewsImportance, NewsItem, NewsType};
use crate::rivalry::{crossed_threshold, RivalryIntensityLevel};

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

// ── Fonte 1: Briga de construtores (Campeonato) ───────────────────────────────
//
// Fim de temporada, lendo `team_season_archive` (mesma fonte que reputação/moral). Dentro
// de cada categoria/classe pega os top-4; um par vira rivalidade se os dois estão no top-3
// OU o gap de pontos é apertado (≤ 15% dos pontos do líder). É a espinha dorsal: reforça a
// cada temporada que a briga se repete, fazendo um clássico crescer no eixo histórico.

/// Fração dos pontos do líder dentro da qual o gap de pontos conta como "briga apertada".
const CONSTRUCTOR_CLOSE_FRAC: f64 = 0.15;

/// Reforça rivalidades entre construtores que brigaram na temporada que fecha. Roda no
/// pipeline de fim de temporada, depois do arquivamento.
pub fn process_constructor_battle_rivalry(
    conn: &Connection,
    temporada: i32,
) -> Result<(), DbError> {
    let mut stmt = conn.prepare(
        "SELECT team_id, categoria, COALESCE(classe, ''), posicao_campeonato, pontos
         FROM team_season_archive
         WHERE season_number = ?1 AND posicao_campeonato IS NOT NULL
         ORDER BY categoria, COALESCE(classe, ''), posicao_campeonato",
    )?;
    let rows = stmt.query_map([temporada], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i32>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;

    // Agrupa por (categoria, classe): (team_id, categoria, posição, pontos).
    let mut groups: std::collections::BTreeMap<(String, String), Vec<(String, String, i32, f64)>> =
        std::collections::BTreeMap::new();
    for row in rows {
        let (team_id, categoria, classe, pos, pontos) = row?;
        groups
            .entry((categoria.clone(), classe))
            .or_default()
            .push((team_id, categoria, pos, pontos));
    }
    drop(stmt);

    for (_key, mut teams) in groups {
        teams.sort_by_key(|t| t.2); // por posição ascendente
        teams.truncate(4); // só os top-4 brigam por "clássico"
        if teams.len() < 2 {
            continue;
        }
        let leader_points = teams.iter().map(|t| t.3).fold(f64::MIN, f64::max).max(1.0);
        for i in 0..teams.len() {
            for j in (i + 1)..teams.len() {
                let (a_id, categoria, a_pos, a_pts) = &teams[i];
                let (b_id, _, b_pos, b_pts) = &teams[j];
                let both_top3 = *a_pos <= 3 && *b_pos <= 3;
                let gap = (a_pts - b_pts).abs();
                let close = gap <= CONSTRUCTOR_CLOSE_FRAC * leader_points;
                if !(both_top3 || close) {
                    continue;
                }
                // +50% se o par decidiu o título (1º vs 2º).
                let title_decider =
                    (*a_pos == 1 && *b_pos == 2) || (*a_pos == 2 && *b_pos == 1);
                let (h, r) = if title_decider { (6.0, 15.0) } else { (4.0, 10.0) };
                let applied = apply_team_rivalry_event(
                    conn,
                    &TeamRivalryEvent {
                        team_a: a_id.clone(),
                        team_b: b_id.clone(),
                        tipo: TeamRivalryType::Campeonato,
                        historical_delta: h,
                        recent_delta: r,
                        temporada,
                    },
                )?;
                emit_team_rivalry_news(
                    conn,
                    &applied,
                    TeamRivalryType::Campeonato,
                    a_id,
                    b_id,
                    Some(categoria),
                    None,
                    temporada,
                )?;
            }
        }
    }
    Ok(())
}

// ── Fonte 2: Roubo de talento (Mercado) — o Elo 2 ─────────────────────────────
//
// Todo site do mercado onde o `equipe_id` de um piloto muda de B→A semeia rancor no par de
// times. O rancor é proporcional ao que se perdeu (astro > titular > reserva) e ao
// descaramento (assédio mid-contrato > troca livre). É isto que dá memória duradoura ao
// "piloto largou e foi pro rival" — antes o destino do piloto não deixava marca no mundo.

/// Skill a partir da qual o piloto conta como "astro" para o rancor de mercado.
const STAR_SKILL: f64 = 80.0;
/// Mídia a partir da qual o piloto conta como "astro" (holofote), mesmo sem skill de elite.
const STAR_MIDIA: f64 = 70.0;
/// Skill mínimo para contar como "titular" (abaixo é peça menor/reserva).
const STARTER_SKILL: f64 = 50.0;

/// Semeia/reforça a rivalidade de MERCADO quando um piloto muda de `from_team` para
/// `to_team`. `is_poaching` = assédio mid-contrato (rancor máximo). Best-effort.
pub fn seed_team_rivalry_from_transfer(
    conn: &Connection,
    from_team_id: &str,
    to_team_id: &str,
    driver: &Driver,
    is_poaching: bool,
    temporada: i32,
) -> Result<(), DbError> {
    if from_team_id == to_team_id || from_team_id.is_empty() || to_team_id.is_empty() {
        return Ok(());
    }
    let Some(from_team) = team_queries::get_team_by_id(conn, from_team_id)? else {
        return Ok(());
    };
    let Some(to_team) = team_queries::get_team_by_id(conn, to_team_id)? else {
        return Ok(());
    };

    let was_n1 = from_team.hierarquia_n1_id.as_deref() == Some(driver.id.as_str());
    let is_star =
        driver.atributos.skill >= STAR_SKILL || driver.atributos.midia >= STAR_MIDIA || was_n1;

    let (mut h, mut r) = if is_star && is_poaching {
        (8.0, 16.0)
    } else if is_star {
        (6.0, 12.0)
    } else if driver.atributos.skill >= STARTER_SKILL {
        (3.0, 8.0)
    } else {
        (1.0, 4.0)
    };
    // Rivalidade entre divisões (categorias diferentes) pesa metade.
    if from_team.categoria != to_team.categoria {
        h *= 0.5;
        r *= 0.5;
    }

    let applied = apply_team_rivalry_event(
        conn,
        &TeamRivalryEvent {
            team_a: from_team_id.to_string(),
            team_b: to_team_id.to_string(),
            tipo: TeamRivalryType::Mercado,
            historical_delta: h,
            recent_delta: r,
            temporada,
        },
    )?;
    emit_team_rivalry_news(
        conn,
        &applied,
        TeamRivalryType::Mercado,
        from_team_id,
        to_team_id,
        Some(&to_team.categoria),
        None,
        temporada,
    )?;
    Ok(())
}

// ── Fonte 3: Guerra na pista (Pista) ──────────────────────────────────────────
//
// Piggyback no mesmo `flat_incidents` da rivalidade de piloto: resolve o time de cada
// piloto em colisão e agrega POR PAR DE TIMES (só times diferentes — bater no companheiro
// não é rivalidade de time), pegando a severidade máxima do par no evento.

/// Reforça rivalidades de PISTA a partir das colisões de uma corrida. `team_by_driver`
/// mapeia driver_id → team_id dos participantes.
pub fn process_team_collisions_rivalry(
    conn: &Connection,
    incidents: &[crate::simulation::incidents::IncidentResult],
    team_by_driver: &HashMap<String, String>,
    categoria_id: &str,
    rodada: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::simulation::incidents::{IncidentSeverity, IncidentType};

    let mut pairs: HashMap<(String, String), (f64, f64)> = HashMap::new();
    for inc in incidents {
        if inc.incident_type != IncidentType::Collision {
            continue;
        }
        let Some(linked) = &inc.linked_pilot_id else {
            continue;
        };
        let (Some(ta), Some(tb)) =
            (team_by_driver.get(&inc.pilot_id), team_by_driver.get(linked))
        else {
            continue;
        };
        if ta == tb {
            continue; // bater no próprio companheiro não é rivalidade de time
        }
        let Some(pair) = normalize_pair(ta, tb) else {
            continue;
        };
        // Severidade máxima do par → delta (base capado por corrida).
        let (h, r) = if inc.severity == IncidentSeverity::Critical || inc.is_dnf {
            (3.0, 8.0)
        } else {
            (2.0, 6.0)
        };
        let e = pairs
            .entry((pair.piloto1_id, pair.piloto2_id))
            .or_insert((0.0, 0.0));
        if h > e.0 {
            *e = (h, r);
        }
    }

    for ((t1, t2), (h, r)) in pairs {
        let applied = apply_team_rivalry_event(
            conn,
            &TeamRivalryEvent {
                team_a: t1.clone(),
                team_b: t2.clone(),
                tipo: TeamRivalryType::Pista,
                historical_delta: h,
                recent_delta: r,
                temporada,
            },
        )?;
        emit_team_rivalry_news(
            conn,
            &applied,
            TeamRivalryType::Pista,
            &t1,
            &t2,
            Some(categoria_id),
            Some(rodada),
            temporada,
        )?;
    }
    Ok(())
}

// ── Fonte 4: Transbordamento de piloto (Herdada) ──────────────────────────────
//
// O "Verstappen×Hamilton → RBR×Merc": rivalidades de PILOTO vivas e intensas (percebida ≥
// 60) cujos dois pilotos estão em times diferentes pingam um trickle na rivalidade dos
// times. Trickle deliberadamente pequeno — é eco, não origem.

/// Percebida mínima da rivalidade de PILOTO para transbordar aos times (faixa Forte).
const BLEED_MIN_PERCEIVED: f64 = 60.0;

/// Varre as rivalidades de piloto e transborda as intensas cross-time para os times.
/// `team_by_driver` mapeia driver_id → team_id (dos participantes da corrida).
pub fn process_driver_rivalry_bleed(
    conn: &Connection,
    team_by_driver: &HashMap<String, String>,
    categoria_id: &str,
    rodada: i32,
    temporada: i32,
) -> Result<(), DbError> {
    use crate::db::queries::rivalries::get_all_rivalries;

    let rivalries = get_all_rivalries(conn)?;
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for rv in rivalries {
        if rv.perceived_intensity() < BLEED_MIN_PERCEIVED {
            continue;
        }
        let (Some(ta), Some(tb)) = (
            team_by_driver.get(&rv.piloto1_id),
            team_by_driver.get(&rv.piloto2_id),
        ) else {
            continue;
        };
        if ta == tb {
            continue;
        }
        let Some(pair) = normalize_pair(ta, tb) else {
            continue;
        };
        // Dedupe: dois pares de pilotos podem mapear pro mesmo par de times.
        if !seen.insert((pair.piloto1_id.clone(), pair.piloto2_id.clone())) {
            continue;
        }
        let applied = apply_team_rivalry_event(
            conn,
            &TeamRivalryEvent {
                team_a: pair.piloto1_id.clone(),
                team_b: pair.piloto2_id.clone(),
                tipo: TeamRivalryType::Herdada,
                historical_delta: 1.0,
                recent_delta: 3.0,
                temporada,
            },
        )?;
        emit_team_rivalry_news(
            conn,
            &applied,
            TeamRivalryType::Herdada,
            &pair.piloto1_id,
            &pair.piloto2_id,
            Some(categoria_id),
            Some(rodada),
            temporada,
        )?;
    }
    Ok(())
}

// ── Tier 2: Moral de derby (pulso per-race) ───────────────────────────────────
//
// Para todo par de times com rivalidade viva presente na corrida, o que teve o melhor
// carro à frente do rival ganha moral; o outro perde. Movimento NOVO de moral no meio da
// temporada (hoje a moral só roda no offseason) — sutil, escalado pela percebida, simétrico
// jogador+IA. A moral já é sentida na pista (`morale_pace_delta`) → vira ritmo na corrida
// seguinte. Loop fechado sem tocar em mercado.

/// Base do pulso de moral de derby (multiplicado por 0.5 + percebida/100).
const DERBY_MORALE_BASE: f64 = 0.015;
/// Piso/teto da moral (mesma banda que `advance_team_morale` respeita).
const MORALE_FLOOR: f64 = 0.5;
const MORALE_CEIL: f64 = 1.5;
/// Percebida mínima para um par gerar pulso de derby (abaixo, rivalidade fria demais).
const DERBY_MIN_PERCEIVED: f64 = 20.0;

/// Aplica o pulso de moral de derby de uma corrida. `team_best_finish` = melhor posição de
/// chegada de cada time nesta corrida (menor = melhor).
pub fn apply_derby_morale(
    conn: &Connection,
    team_best_finish: &HashMap<String, i32>,
) -> Result<(), DbError> {
    let all = get_all_team_rivalries(conn)?;
    // Acumula o delta por time (um time pode viver vários derbies na mesma corrida).
    let mut morale_delta: HashMap<String, f64> = HashMap::new();
    for rv in all {
        if matches!(
            rivalry_lifecycle(rv.historical_intensity, rv.recent_activity),
            RivalryLifecycle::Extinta
        ) {
            continue;
        }
        let perceived = rv.perceived_intensity();
        if perceived < DERBY_MIN_PERCEIVED {
            continue;
        }
        let (Some(&pa), Some(&pb)) = (
            team_best_finish.get(&rv.team1_id),
            team_best_finish.get(&rv.team2_id),
        ) else {
            continue;
        };
        if pa == pb {
            continue;
        }
        let delta = DERBY_MORALE_BASE * (0.5 + perceived / 100.0);
        let (winner, loser) = if pa < pb {
            (&rv.team1_id, &rv.team2_id)
        } else {
            (&rv.team2_id, &rv.team1_id)
        };
        *morale_delta.entry(winner.clone()).or_insert(0.0) += delta;
        *morale_delta.entry(loser.clone()).or_insert(0.0) -= delta;
    }

    for (team_id, delta) in morale_delta {
        if delta.abs() < 1e-9 {
            continue;
        }
        let Some(mut team) = team_queries::get_team_by_id(conn, &team_id)? else {
            continue;
        };
        team.morale = (team.morale + delta).clamp(MORALE_FLOOR, MORALE_CEIL);
        team_queries::update_team(conn, &team)?;
    }
    Ok(())
}

// ── Tier 1: Manchete de derby ─────────────────────────────────────────────────

fn team_name(conn: &Connection, team_id: &str) -> String {
    team_queries::get_team_by_id(conn, team_id)
        .ok()
        .flatten()
        .map(|t| t.nome)
        .unwrap_or_else(|| team_id.to_string())
}

/// Gera uma manchete de rivalidade de equipe ao CRUZAR um threshold de percebida (mesma
/// lógica `crossed_threshold` do piloto). Voz jornalística em 3ª pessoa (revista).
#[allow(clippy::too_many_arguments)]
fn emit_team_rivalry_news(
    conn: &Connection,
    applied: &TeamRivalryApplied,
    tipo: TeamRivalryType,
    team_a_id: &str,
    team_b_id: &str,
    categoria_id: Option<&str>,
    rodada: Option<i32>,
    temporada: i32,
) -> Result<(), DbError> {
    let Some(crossed) = crossed_threshold(applied.old_perceived, applied.new_perceived) else {
        return Ok(());
    };
    let importance = match crossed {
        RivalryIntensityLevel::Inicial => NewsImportance::Media,
        RivalryIntensityLevel::Clara => NewsImportance::Alta,
        RivalryIntensityLevel::Forte | RivalryIntensityLevel::Intensa => NewsImportance::Destaque,
        RivalryIntensityLevel::AtritoLeve => NewsImportance::Media,
    };
    let origem = match tipo {
        TeamRivalryType::Campeonato => rust_i18n::t!("team_rivalry.news.origin_championship"),
        TeamRivalryType::Mercado => rust_i18n::t!("team_rivalry.news.origin_market"),
        TeamRivalryType::Pista => rust_i18n::t!("team_rivalry.news.origin_track"),
        TeamRivalryType::Herdada => rust_i18n::t!("team_rivalry.news.origin_inherited"),
    };
    let nome_a = team_name(conn, team_a_id);
    let nome_b = team_name(conn, team_b_id);
    let titulo = rust_i18n::t!(
        "team_rivalry.news.title",
        a = nome_a,
        b = nome_b,
        level = crossed.label()
    )
    .to_string();
    let texto = rust_i18n::t!(
        "team_rivalry.news.text",
        a = nome_a,
        b = nome_b,
        origin = origem
    )
    .to_string();

    let item = NewsItem {
        id: next_id(conn, IdType::News)?,
        tipo: NewsType::Rivalidade,
        icone: NewsType::Rivalidade.icone().to_string(),
        titulo,
        texto,
        rodada,
        semana_pretemporada: None,
        temporada,
        categoria_id: categoria_id.map(str::to_string),
        categoria_nome: None,
        importancia: importance,
        timestamp: chrono::Local::now().timestamp(),
        driver_id: None,
        driver_id_secondary: None,
        team_id: Some(team_a_id.to_string()),
    };
    insert_news(conn, &item)?;
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
        // Estes testes de unidade usam ids sintéticos de time (T001…) sem popular a
        // tabela `teams`; a FK `team_rivalries → teams(id)` é irrelevante aqui (a produção
        // sempre semeia com ids reais). Desliga a checagem para o motor puro.
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
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

    // ── Fontes/consequências (Fase 2+) ────────────────────────────────────────

    fn insert_test_team(conn: &Connection, id: &str, categoria: &str) -> crate::models::team::Team {
        use crate::constants::teams::get_team_templates;
        use crate::db::queries::teams::insert_team;
        use rand::{rngs::StdRng, SeedableRng};
        let template = get_team_templates(categoria)[0];
        let mut rng = StdRng::seed_from_u64(9);
        let mut team = crate::models::team::Team::from_template_with_rng(
            template,
            categoria,
            id.to_string(),
            2026,
            &mut rng,
        );
        team.morale = 1.0;
        team.hierarquia_n1_id = None;
        insert_team(conn, &team).unwrap();
        team
    }

    fn insert_test_driver(conn: &Connection, id: &str, skill: f64, midia: f64) -> Driver {
        use crate::db::queries::drivers::insert_driver;
        let mut d = Driver::create_player(id.to_string(), format!("Piloto {id}"), "BR".to_string(), 25);
        d.is_jogador = false;
        d.atributos.skill = skill;
        d.atributos.midia = midia;
        insert_driver(conn, &d).unwrap();
        d
    }

    fn archive_row(conn: &Connection, team_id: &str, categoria: &str, pos: i32, pontos: f64) {
        conn.execute(
            "INSERT INTO team_season_archive
                 (team_id, season_number, ano, categoria, posicao_campeonato, pontos)
             VALUES (?1, 1, 2026, ?2, ?3, ?4)",
            rusqlite::params![team_id, categoria, pos, pontos],
        )
        .unwrap();
    }

    #[test]
    fn seed_transfer_astro_assediado_gera_rancor_maximo() {
        let conn = setup_db();
        insert_test_team(&conn, "T001", "gt3");
        insert_test_team(&conn, "T002", "gt3");
        let star = insert_test_driver(&conn, "P100", 85.0, 40.0);

        seed_team_rivalry_from_transfer(&conn, "T001", "T002", &star, true, 1).unwrap();

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        assert_eq!(summ.len(), 1);
        assert_eq!(summ[0].tipo, TeamRivalryType::Mercado);
        // Astro + poaching: h=8, r=16 → perceived = 0.4*8 + 0.6*16 = 12.8.
        assert!((summ[0].perceived_intensity - 12.8).abs() < 1e-9);
    }

    #[test]
    fn seed_transfer_entre_divisoes_pesa_metade() {
        let conn = setup_db();
        insert_test_team(&conn, "T001", "gt3");
        insert_test_team(&conn, "T002", "gt4");
        let star = insert_test_driver(&conn, "P100", 85.0, 40.0);

        seed_team_rivalry_from_transfer(&conn, "T001", "T002", &star, true, 1).unwrap();

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        // Metade de (8,16) = (4,8) → perceived = 0.4*4 + 0.6*8 = 6.4.
        assert!((summ[0].perceived_intensity - 6.4).abs() < 1e-9);
    }

    #[test]
    fn seed_transfer_reserva_pesa_pouco() {
        let conn = setup_db();
        insert_test_team(&conn, "T001", "gt3");
        insert_test_team(&conn, "T002", "gt3");
        let reserva = insert_test_driver(&conn, "P100", 40.0, 20.0);

        seed_team_rivalry_from_transfer(&conn, "T001", "T002", &reserva, false, 1).unwrap();

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        // Reserva: h=1, r=4 → perceived = 0.4*1 + 0.6*4 = 2.8.
        assert!((summ[0].perceived_intensity - 2.8).abs() < 1e-9);
    }

    #[test]
    fn constructor_battle_top3_e_gap_apertado() {
        let conn = setup_db();
        for id in ["T001", "T002", "T003", "T004"] {
            insert_test_team(&conn, id, "gt3");
        }
        archive_row(&conn, "T001", "gt3", 1, 100.0);
        archive_row(&conn, "T002", "gt3", 2, 95.0);
        archive_row(&conn, "T003", "gt3", 3, 50.0);
        archive_row(&conn, "T004", "gt3", 4, 10.0);

        process_constructor_battle_rivalry(&conn, 1).unwrap();

        // Top-3 entre si (T001,T002,T003) formam pares; T004 (4º, gap grande) fica de fora.
        assert_eq!(get_team_rivalries(&conn, "T001").unwrap().len(), 2);
        assert!(get_team_rivalries(&conn, "T004").unwrap().is_empty());
        // Par 1º×2º decidiu o título → delta reforçado (6,15) → perceived = 11.4.
        let leader = get_team_rivalries(&conn, "T001").unwrap();
        let vs_t2 = leader.iter().find(|r| r.rival_id == "T002").unwrap();
        assert!((vs_t2.perceived_intensity - 11.4).abs() < 1e-9, "foi {}", vs_t2.perceived_intensity);
    }

    #[test]
    fn collisions_agrega_por_par_de_times_ignora_companheiro() {
        use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
        let conn = setup_db();
        let mut team_by_driver = HashMap::new();
        team_by_driver.insert("P1".to_string(), "T001".to_string());
        team_by_driver.insert("P2".to_string(), "T002".to_string());
        team_by_driver.insert("P3".to_string(), "T001".to_string());

        let mk = |pilot: &str, linked: &str| IncidentResult {
            pilot_id: pilot.to_string(),
            incident_type: IncidentType::Collision,
            severity: IncidentSeverity::Major,
            segment: String::new(),
            positions_lost: 2,
            is_dnf: false,
            description: String::new(),
            linked_pilot_id: Some(linked.to_string()),
            is_two_car_incident: true,
            injury_risk_multiplier: 1.0,
            narrative_importance_hint: 0,
            catalog_id: None,
            damage_origin_segment: None,
        };
        // T001 x T002 (conta) + T001 interno P1×P3 (ignora — mesmo time).
        let incidents = vec![mk("P1", "P2"), mk("P1", "P3")];
        process_team_collisions_rivalry(&conn, &incidents, &team_by_driver, "gt3", 5, 1).unwrap();

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        assert_eq!(summ.len(), 1);
        assert_eq!(summ[0].rival_id, "T002");
        assert_eq!(summ[0].tipo, TeamRivalryType::Pista);
        // Base (2,6) → perceived = 0.4*2 + 0.6*6 = 4.4.
        assert!((summ[0].perceived_intensity - 4.4).abs() < 1e-9);
    }

    #[test]
    fn bleed_so_transborda_rivalidade_intensa_cross_time() {
        use crate::rivalry::{apply_rivalry_event, RivalryEvent};
        use crate::models::rivalry::RivalryType;
        let conn = setup_db();
        // Rivalidade de piloto INTENSA (percebida 60) entre P1(T001) e P2(T002).
        apply_rivalry_event(
            &conn,
            &RivalryEvent {
                piloto_a: "P1".to_string(),
                piloto_b: "P2".to_string(),
                tipo: RivalryType::Colisao,
                historical_delta: 60.0,
                recent_delta: 60.0,
                temporada: 1,
            },
        )
        .unwrap();
        // Rivalidade FRACA (percebida 20) entre P3(T001) e P4(T002) — não transborda.
        apply_rivalry_event(
            &conn,
            &RivalryEvent {
                piloto_a: "P3".to_string(),
                piloto_b: "P4".to_string(),
                tipo: RivalryType::Colisao,
                historical_delta: 20.0,
                recent_delta: 20.0,
                temporada: 1,
            },
        )
        .unwrap();

        let mut team_by_driver = HashMap::new();
        for (p, t) in [("P1", "T001"), ("P2", "T002"), ("P3", "T001"), ("P4", "T002")] {
            team_by_driver.insert(p.to_string(), t.to_string());
        }
        process_driver_rivalry_bleed(&conn, &team_by_driver, "gt3", 5, 1).unwrap();

        let summ = get_team_rivalries(&conn, "T001").unwrap();
        assert_eq!(summ.len(), 1, "só a rivalidade intensa deve transbordar");
        assert_eq!(summ[0].tipo, TeamRivalryType::Herdada);
        // Trickle (1,3) → perceived = 0.4*1 + 0.6*3 = 2.2.
        assert!((summ[0].perceived_intensity - 2.2).abs() < 1e-9);
    }

    #[test]
    fn derby_morale_vencer_o_rival_empurra_moral() {
        let conn = setup_db();
        let t1 = insert_test_team(&conn, "T001", "gt3");
        let t2 = insert_test_team(&conn, "T002", "gt3");
        assert!((t1.morale - 1.0).abs() < 1e-9 && (t2.morale - 1.0).abs() < 1e-9);
        // Rivalidade viva e quente (percebida 40).
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Campeonato, 40.0, 40.0),
        )
        .unwrap();

        let mut best = HashMap::new();
        best.insert("T001".to_string(), 1); // T001 venceu o derby
        best.insert("T002".to_string(), 5);
        apply_derby_morale(&conn, &best).unwrap();

        let m1 = crate::db::queries::teams::get_team_by_id(&conn, "T001")
            .unwrap()
            .unwrap()
            .morale;
        let m2 = crate::db::queries::teams::get_team_by_id(&conn, "T002")
            .unwrap()
            .unwrap()
            .morale;
        assert!(m1 > 1.0, "vencedor sobe a moral, foi {m1}");
        assert!(m2 < 1.0, "perdedor desce a moral, foi {m2}");
        // Pulso sutil: delta = 0.015 * (0.5 + 0.4) = 0.0135.
        assert!((m1 - 1.0135).abs() < 1e-6, "foi {m1}");
    }

    #[test]
    fn derby_morale_ignora_par_ausente_ou_fraco() {
        let conn = setup_db();
        insert_test_team(&conn, "T001", "gt3");
        insert_test_team(&conn, "T002", "gt3");
        // Rivalidade fraca (percebida abaixo do mínimo de derby).
        apply_team_rivalry_event(
            &conn,
            &event("T001", "T002", TeamRivalryType::Campeonato, 2.0, 2.0),
        )
        .unwrap();
        let mut best = HashMap::new();
        best.insert("T001".to_string(), 1);
        best.insert("T002".to_string(), 5);
        apply_derby_morale(&conn, &best).unwrap();

        let m1 = crate::db::queries::teams::get_team_by_id(&conn, "T001")
            .unwrap()
            .unwrap()
            .morale;
        assert!((m1 - 1.0).abs() < 1e-9, "rivalidade fraca não move a moral, foi {m1}");
    }
}
