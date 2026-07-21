//! Rodapé "Do mundo do Grid" do boletim (aba Notícias / revista).
//!
//! Notinhas curtas com VOZ DE REVISTA (3ª pessoa, jornalística — nunca se dirige ao
//! jogador). O laço com o jogador é só o CRITÉRIO DE SELEÇÃO, não aparece no texto.
//!
//! Cascata de assuntos, sempre da categoria ATUAL do jogador:
//!   1. Ex-equipes e ex-companheiros DO JOGADOR (com estado digno de nota).
//!   2. Se faltar, o mesmo para o 1º e o 2º do campeonato (ex-time/ex-parceiro deles).
//!   3. Se ainda faltar, RECORDES da categoria — com ênfase nos que estão a caminho
//!      ("Fulano está a N vitórias de igualar o recorde histórico").
//!
//! Estado lido de campos REAIS de `teams`. Fonte determinística (fallback); os mesmos
//! fatos podem virar IA via `/world-notes` (contrato em `docs/world-notes-endpoint.md`).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::models::enums::{ContractStatus, DriverStatus};

/// Uma notinha do rodapé. `tone` guia o acento visual; `tag` é o rótulo temático.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldNote {
    /// Chave estável (key de lista / dedup no front).
    pub id: String,
    /// Rótulo temático da revista: MERCADO | FINANÇAS | BASTIDORES | RECORDE.
    pub tag: String,
    /// Nome da equipe ou piloto de quem a nota fala.
    pub subject: String,
    /// Categoria de estado (máquina).
    pub kind: String,
    /// "crise" | "alerta" | "reforma" | "recorde" | "neutro" — acento visual.
    pub tone: String,
    /// Texto PT jornalístico (fallback determinístico, sem 2ª pessoa).
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct WorldFooterResult {
    pub notes: Vec<WorldNote>,
    /// "template" (determinístico) | "ai" (reescrito pelo servidor, futuro).
    pub source: String,
    /// Fatos crus (uma linha por nota) — reservados para a reescrita por IA.
    pub facts: String,
}

/// Quantas notas tentar reunir antes de recorrer aos recordes, e o teto duro.
const TARGET_NOTES: usize = 4;
const MAX_NOTES: usize = 5;
/// Distância máxima (em vitórias/pódios/largadas) para um recorde contar como "a caminho".
const RECORD_GAP_MAX: i32 = 3;

/// Ex-equipes de um piloto (contratos passados), ids únicos preservando a ordem.
fn pilot_ex_team_ids(conn: &rusqlite::Connection, pilot_id: &str) -> Vec<String> {
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
fn team_state_note(conn: &rusqlite::Connection, team_id: &str, categoria: &str) -> Option<WorldNote> {
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
fn teammate_news_note(
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

/// Substantivo de uma métrica de recorde, no singular/plural conforme `count`
/// (i18n `world_footer.metric_noun.<id>.{one|other}`).
fn metric_noun(id: &str, count: i32) -> String {
    let form = if count == 1 { "one" } else { "other" };
    let key = format!("world_footer.metric_noun.{id}.{form}");
    rust_i18n::t!(&key).to_string()
}

/// Mapeia a métrica persistida para o id de substantivo do recorde.
fn metric_noun_id(metric: &str) -> &'static str {
    match metric {
        "wins" => "wins",
        "podiums" => "podiums",
        "poles" => "poles",
        "titles" => "titles",
        _ => "starts",
    }
}

/// Rótulo temático da revista (i18n `world_footer.tag.<id>`).
fn tag_label(id: &str) -> String {
    let key = format!("world_footer.tag.{id}");
    rust_i18n::t!(&key).to_string()
}

/// Ordinal formatado no locale ativo. PT é gendered ("2º"/"2ª"); EN é "2nd".
/// Só cobre pt/en por ora (record news); estender ao adicionar locales.
fn ord_label(n: i32, feminine: bool) -> String {
    let loc = rust_i18n::locale();
    if loc.starts_with("en") {
        let suffix = match (n % 100, n % 10) {
            (11..=13, _) => "th",
            (_, 1) => "st",
            (_, 2) => "nd",
            (_, 3) => "rd",
            _ => "th",
        };
        format!("{n}{suffix}")
    } else {
        format!("{n}{}", if feminine { "ª" } else { "º" })
    }
}

/// Formata um tempo de volta em ms para "m:ss.mmm" (ou "ss.mmm" abaixo de 1 min).
fn format_lap_ms(ms: i32) -> String {
    let total = ms.max(0);
    let minutes = total / 60_000;
    let seconds = (total % 60_000) / 1_000;
    let millis = total % 1_000;
    if minutes > 0 {
        format!("{minutes}:{seconds:02}.{millis:03}")
    } else {
        format!("{seconds}.{millis:03}")
    }
}

/// Notas de RECORDE recém-quebrado (com data), a partir dos marcos persistidos
/// (`milestones`). Só marcos RECENTES — desta temporada ou da anterior — para não
/// ressuscitar recorde velho. Uma nota por piloto.
fn record_broken_notes(
    conn: &rusqlite::Connection,
    categoria: &str,
    current_season: i32,
    used_drivers: &mut HashSet<String>,
    budget: usize,
) -> Vec<WorldNote> {
    use crate::db::queries::milestones;

    let mut out = Vec::new();
    if budget == 0 {
        return out;
    }
    let Ok(recent) = milestones::get_recent_milestones(conn, categoria, 5) else {
        return out;
    };
    for m in recent {
        if out.len() >= budget {
            break;
        }
        // Recente = quebrado nesta temporada ou na imediatamente anterior.
        if current_season > 0 && current_season - m.season_number > 1 {
            continue;
        }
        if !used_drivers.insert(m.pilot_id.clone()) {
            continue;
        }
        let name = m.pilot_name.as_str();
        let ctx = m.context.as_str();
        let text = match m.metric.as_str() {
            "lap_record" => rust_i18n::t!(
                "world_footer.record_broken.lap_record",
                name = name, context = ctx,
                lap = format_lap_ms(m.value),
                prev = m.previous_value.map(format_lap_ms).unwrap_or_default()
            )
            .to_string(),
            "comeback" => {
                rust_i18n::t!("world_footer.record_broken.comeback", name = name, value = m.value)
                    .to_string()
            }
            "season_wins" => match m.previous_value {
                Some(prev) => rust_i18n::t!(
                    "world_footer.record_broken.season_wins_prev",
                    name = name, value = m.value, prev = prev
                )
                .to_string(),
                None => rust_i18n::t!(
                    "world_footer.record_broken.season_wins",
                    name = name, value = m.value
                )
                .to_string(),
            },
            "track_wins" => rust_i18n::t!(
                "world_footer.record_broken.track_wins",
                name = name, context = ctx, value = m.value
            )
            .to_string(),
            "win_streak" => {
                rust_i18n::t!("world_footer.record_broken.win_streak", name = name, value = m.value)
                    .to_string()
            }
            "constructor_titles" => rust_i18n::t!(
                "world_footer.record_broken.constructor_titles",
                name = name, ord = ord_label(m.value, false)
            )
            .to_string(),
            "team_wins" => {
                rust_i18n::t!("world_footer.record_broken.team_wins", name = name, value = m.value)
                    .to_string()
            }
            "one_two" => rust_i18n::t!(
                "world_footer.record_broken.one_two",
                name = name, ord = ord_label(m.value, true)
            )
            .to_string(),
            "youngest_winner" => rust_i18n::t!(
                "world_footer.record_broken.youngest_winner",
                name = name, value = m.value
            )
            .to_string(),
            "oldest_winner" => rust_i18n::t!(
                "world_footer.record_broken.oldest_winner",
                name = name, value = m.value
            )
            .to_string(),
            "youngest_champion" => rust_i18n::t!(
                "world_footer.record_broken.youngest_champion",
                name = name, value = m.value
            )
            .to_string(),
            "most_chaotic_race" => rust_i18n::t!(
                "world_footer.record_broken.most_chaotic_race",
                name = name, value = m.value
            )
            .to_string(),
            "drought_broken" => rust_i18n::t!(
                "world_footer.record_broken.drought_broken",
                name = name, value = m.value
            )
            .to_string(),
            "closest_championship" => {
                if m.value == 0 {
                    rust_i18n::t!(
                        "world_footer.record_broken.closest_championship_tie",
                        name = name
                    )
                    .to_string()
                } else {
                    rust_i18n::t!(
                        "world_footer.record_broken.closest_championship",
                        name = name, value = m.value
                    )
                    .to_string()
                }
            }
            "biggest_blowout" => rust_i18n::t!(
                "world_footer.record_broken.biggest_blowout",
                name = name, value = m.value
            )
            .to_string(),
            "longest_pairing" => rust_i18n::t!(
                "world_footer.record_broken.longest_pairing",
                name = name, value = m.value
            )
            .to_string(),
            "most_starts_no_win" => rust_i18n::t!(
                "world_footer.record_broken.most_starts_no_win",
                name = name, context = ctx, value = m.value
            )
            .to_string(),
            "most_career_dnfs" => rust_i18n::t!(
                "world_footer.record_broken.most_career_dnfs",
                name = name, context = ctx, value = m.value
            )
            .to_string(),
            "most_poles_no_title" => rust_i18n::t!(
                "world_footer.record_broken.most_poles_no_title",
                name = name, context = ctx, value = m.value
            )
            .to_string(),
            "most_career_points" => rust_i18n::t!(
                "world_footer.record_broken.most_career_points",
                name = name, context = ctx
            )
            .to_string(),
            _ => {
                let noun = metric_noun(metric_noun_id(&m.metric), 2);
                match m.previous_value {
                    Some(prev) => rust_i18n::t!(
                        "world_footer.record_broken.generic_prev",
                        name = name, noun = noun, value = m.value, prev = prev
                    )
                    .to_string(),
                    None => rust_i18n::t!(
                        "world_footer.record_broken.generic",
                        name = name, noun = noun, value = m.value
                    )
                    .to_string(),
                }
            }
        };
        out.push(WorldNote {
            id: format!("broken:{}:{}:{}", categoria, m.metric, m.value),
            tag: tag_label("record"),
            subject: m.pilot_name,
            kind: "recorde_quebrado".to_string(),
            tone: "recorde".to_string(),
            text,
        });
    }
    out
}

/// Notas de RECORDE a caminho: pilotos ATIVOS da categoria a até `RECORD_GAP_MAX` de
/// igualar um recorde histórico (vitórias, pódios ou largadas). Mais próximos primeiro,
/// uma nota por piloto. É o fallback quando não há assunto de mercado/bastidores.
fn record_watch_notes(
    conn: &rusqlite::Connection,
    categoria: &str,
    used_drivers: &mut HashSet<String>,
    budget: usize,
) -> Vec<WorldNote> {
    use crate::db::queries::{drivers, race_history};

    let mut out = Vec::new();
    if budget == 0 {
        return out;
    }
    let Ok(records) = race_history::get_category_records(conn, categoria) else {
        return out;
    };
    let Ok(field) = drivers::get_drivers_by_category(conn, categoria) else {
        return out;
    };

    // (gap, driver_id, texto) — coleta e ordena por proximidade.
    let mut cands: Vec<(i32, String, String)> = Vec::new();
    for d in &field {
        if d.status != DriverStatus::Ativo {
            continue;
        }
        let Ok(career) = race_history::get_driver_category_career(conn, &d.id, categoria) else {
            continue;
        };
        // (recorde, valor do piloto, substantivo)
        let metrics: [(&Option<race_history::CategoryRecord>, i32, &str); 3] = [
            (&records.most_wins, career.wins, "wins"),
            (&records.most_podiums, career.podiums, "podiums"),
            (&records.most_starts, career.starts, "starts"),
        ];
        // Uma métrica por piloto: a mais próxima do recorde.
        let mut best: Option<(i32, String)> = None;
        for (rec, val, noun_id) in metrics {
            let Some(r) = rec else { continue };
            if r.pilot_id == d.id {
                continue; // já é o recordista.
            }
            let gap = r.value - val;
            if !(0..=RECORD_GAP_MAX).contains(&gap) {
                continue;
            }
            let text = if gap == 0 {
                rust_i18n::t!(
                    "world_footer.record_watch.tied",
                    name = d.nome.as_str(),
                    noun = metric_noun(noun_id, 2),
                    value = r.value,
                    holder = r.pilot_name.as_str()
                )
                .to_string()
            } else {
                rust_i18n::t!(
                    "world_footer.record_watch.approaching",
                    name = d.nome.as_str(),
                    gap = gap,
                    noun = metric_noun(noun_id, gap),
                    value = r.value,
                    holder = r.pilot_name.as_str()
                )
                .to_string()
            };
            if best.as_ref().map_or(true, |(bg, _)| gap < *bg) {
                best = Some((gap, text));
            }
        }
        if let Some((gap, text)) = best {
            cands.push((gap, d.id.clone(), text));
        }
    }

    cands.sort_by_key(|(gap, _, _)| *gap);
    for (_, driver_id, text) in cands {
        if out.len() >= budget {
            break;
        }
        if !used_drivers.insert(driver_id.clone()) {
            continue;
        }
        out.push(WorldNote {
            id: format!("record:{driver_id}"),
            tag: tag_label("record"),
            subject: text.clone(),
            kind: "recorde_a_caminho".to_string(),
            tone: "recorde".to_string(),
            text,
        });
    }
    out
}

/// Fama mínima (escala de EXIBIÇÃO da ficha) para um piloto ser "astro" digno de nota:
/// tier Estrela+ — a régua vai Nome forte ≤70 / Estrela ≤87 / Ídolo >87. Abaixo disso
/// não há estrela de verdade e a categoria não rende manchete de público.
const STAR_MIN_FAMA: f64 = 71.0;
const IDOL_MIN_FAMA: f64 = 88.0;

/// Nota de ASTRO (Fase 3 do Estrelato): o maior nome de PÚBLICO da categoria vira
/// manchete de bastidores — a fama arrasta arquibancada e patrocínio. VOZ de revista
/// (3ª pessoa). `None` quando ninguém tem fama de Estrela+ (categoria sem astro não é
/// notícia) ou quando o maior nome já virou nota em outro passo (dedup por `used_drivers`).
fn star_of_category_note(
    conn: &rusqlite::Connection,
    categoria: &str,
    used_drivers: &mut HashSet<String>,
) -> Option<WorldNote> {
    use crate::db::queries::drivers;

    let field = drivers::get_drivers_by_category(conn, categoria).ok()?;
    let star = field
        .into_iter()
        .filter(|d| d.status == DriverStatus::Ativo && !used_drivers.contains(&d.id))
        .max_by(|a, b| {
            a.atributos
                .midia
                .partial_cmp(&b.atributos.midia)
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    if star.atributos.midia < STAR_MIN_FAMA {
        return None;
    }

    let text = if star.atributos.midia >= IDOL_MIN_FAMA {
        rust_i18n::t!("world_footer.star.idol", name = star.nome.as_str()).to_string()
    } else {
        rust_i18n::t!("world_footer.star.star", name = star.nome.as_str()).to_string()
    };

    used_drivers.insert(star.id.clone());
    Some(WorldNote {
        id: format!("star:{}", star.id),
        tag: tag_label("backstage"),
        subject: star.nome.clone(),
        kind: "astro_da_categoria".to_string(),
        tone: "neutro".to_string(),
        text,
    })
}

/// Reúne as notas do rodapé para o save aberto (cascata jogador → líderes → astro → recordes).
fn collect_world_notes(conn: &rusqlite::Connection) -> Vec<WorldNote> {
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
        notes.extend(record_watch_notes(conn, &categoria, &mut used_drivers, budget));
    }

    notes.truncate(MAX_NOTES);
    notes
}

/// Comando: notinhas do rodapé de notícias do mundo (determinístico). O front chama ao
/// abrir a revista e renderiza entre o boletim e o rodapé GRID·MAGAZINE. Nunca quebra:
/// em qualquer falha devolve lista vazia (o rodapé simplesmente não aparece).
#[tauri::command]
pub fn get_world_footer(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<WorldFooterResult, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    let notes = collect_world_notes(&db.conn);
    let facts = notes
        .iter()
        .map(|n| format!("[{}] {} — {}", n.kind, n.subject, n.text))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(WorldFooterResult {
        notes,
        source: "template".to_string(),
        facts,
    })
}

/// Resultado da reescrita por IA do rodapé. `notes` só vem preenchido quando a IA
/// respondeu e casou 1-para-1 com o template; caso contrário o front mantém o texto
/// determinístico que já recebeu de `get_world_footer`.
#[derive(Debug, Serialize)]
pub struct WorldFooterAiResult {
    pub notes: Option<Vec<WorldNote>>,
    /// "ai" | "template".
    pub source: String,
    /// ok | cached | unavailable | rate_limited | error | mismatch
    pub status: String,
}

/// Aplica as reescritas da IA sobre as notas determinísticas, casando 1-para-1 por
/// índice (o servidor devolve uma string por fato, na mesma ordem). Só substitui se a
/// contagem bate EXATAMENTE e nenhuma vem vazia — senão o alinhamento estaria quebrado
/// e é mais seguro manter o template. Pura e testável.
fn apply_ai_texts(mut notes: Vec<WorldNote>, ai: &[String]) -> Option<Vec<WorldNote>> {
    if notes.is_empty() || ai.len() != notes.len() {
        return None;
    }
    for (n, text) in notes.iter_mut().zip(ai.iter()) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        n.text = trimmed.to_string();
    }
    Some(notes)
}

/// Comando: reescrita por IA do rodapé "Do mundo do Grid". O front chama DEPOIS de já
/// ter mostrado o template (`get_world_footer`) e troca as notas quando a IA chega —
/// sem bloquear a abertura da revista. Cacheado por `temporada:rodada`. Em QUALQUER
/// falha (inclusive o endpoint `/world-notes` ainda não existir no servidor) devolve
/// `notes: None` e o front simplesmente mantém o texto determinístico.
#[tauri::command]
pub fn enrich_world_footer_ai(
    app: tauri::AppHandle,
    career_id: String,
) -> Result<WorldFooterAiResult, String> {
    let base_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Falha ao obter app_data_dir: {e}"))?;
    let mut config = AppConfig::load_or_default(&base_dir);
    let install_id = config.get_or_create_install_id();
    let lang = config.language.clone();
    let db_path = config.saves_dir().join(&career_id).join("career.db");
    let db = Database::open_existing(&db_path).map_err(|e| format!("Falha ao abrir banco: {e}"))?;

    // Chave de cache: temporada:rodada (as notas mudam de estado a cada rodada).
    let (season_num, rodada) = crate::db::queries::seasons::get_active_season(&db.conn)
        .ok()
        .flatten()
        .map(|s| (s.numero, s.rodada_atual))
        .unwrap_or((0, 0));
    let cache_key = format!("{season_num}:{rodada}");

    // Cache → reabrir a revista não regenera.
    if let Ok(Some(json)) = crate::db::queries::ai_world_notes::get_cached(&db.conn, &cache_key) {
        if let Ok(notes) = serde_json::from_str::<Vec<WorldNote>>(&json) {
            return Ok(WorldFooterAiResult {
                notes: Some(notes),
                source: "ai".to_string(),
                status: "cached".to_string(),
            });
        }
    }

    // Reconstrói as notas determinísticas + os fatos (MESMA ordem do get_world_footer).
    let notes = collect_world_notes(&db.conn);
    if notes.is_empty() {
        return Ok(WorldFooterAiResult {
            notes: None,
            source: "template".to_string(),
            status: "unavailable".to_string(),
        });
    }
    let facts = notes
        .iter()
        .map(|n| format!("[{}] {} — {}", n.kind, n.subject, n.text))
        .collect::<Vec<_>>()
        .join("\n");

    match crate::narrative::client::fetch_world_notes(&facts, &lang, &install_id) {
        Ok(ai) => match apply_ai_texts(notes, &ai) {
            Some(enriched) => {
                if let Ok(json) = serde_json::to_string(&enriched) {
                    let _ =
                        crate::db::queries::ai_world_notes::set_cached(&db.conn, &cache_key, &json);
                }
                Ok(WorldFooterAiResult {
                    notes: Some(enriched),
                    source: "ai".to_string(),
                    status: "ok".to_string(),
                })
            }
            None => Ok(WorldFooterAiResult {
                notes: None,
                source: "template".to_string(),
                status: "mismatch".to_string(),
            }),
        },
        Err(crate::narrative::client::StoryError::RateLimited) => Ok(WorldFooterAiResult {
            notes: None,
            source: "template".to_string(),
            status: "rate_limited".to_string(),
        }),
        Err(_) => Ok(WorldFooterAiResult {
            notes: None,
            source: "template".to_string(),
            status: "error".to_string(),
        }),
    }
}

#[cfg(test)]
mod ai_tests {
    use super::*;

    fn note(text: &str) -> WorldNote {
        WorldNote {
            id: "x".into(),
            tag: "RECORDE".into(),
            subject: "Fulano".into(),
            kind: "recorde_quebrado".into(),
            tone: "recorde".into(),
            text: text.into(),
        }
    }

    #[test]
    fn substitui_quando_conta_bate() {
        let notes = vec![note("template 1"), note("template 2")];
        let ai = vec!["reescrita 1".to_string(), "reescrita 2".to_string()];
        let out = apply_ai_texts(notes, &ai).expect("deveria casar");
        assert_eq!(out[0].text, "reescrita 1");
        assert_eq!(out[1].text, "reescrita 2");
        // Preserva os metadados (só o texto muda).
        assert_eq!(out[0].kind, "recorde_quebrado");
    }

    #[test]
    fn mantem_template_quando_conta_diverge() {
        let notes = vec![note("a"), note("b")];
        let ai = vec!["só uma".to_string()];
        assert!(apply_ai_texts(notes, &ai).is_none());
    }

    #[test]
    fn mantem_template_quando_ha_reescrita_vazia() {
        let notes = vec![note("a"), note("b")];
        let ai = vec!["ok".to_string(), "   ".to_string()];
        assert!(apply_ai_texts(notes, &ai).is_none());
    }

    #[test]
    fn vazio_nao_casa() {
        assert!(apply_ai_texts(vec![], &[]).is_none());
    }

    /// Guarda a i18n do rodapé nos DOIS locales: tags, nouns singular/plural, ordinais
    /// gendered (PT) vs sufixo (EN) e interpolação (sem `%{...}` cru). `#[serial]` porque
    /// troca o locale global (não corre junto do i18n_smoke).
    #[test]
    #[serial_test::serial]
    fn i18n_do_rodape_resolve_nos_dois_locales() {
        rust_i18n::set_locale("pt-BR");
        assert_eq!(tag_label("record"), "RECORDE");
        assert_eq!(metric_noun("wins", 1), "vitória");
        assert_eq!(metric_noun("wins", 3), "vitórias");
        assert_eq!(ord_label(2, false), "2º");
        assert_eq!(ord_label(2, true), "2ª");
        let pt = rust_i18n::t!(
            "world_footer.record_broken.season_wins_prev",
            name = "Fulano", value = 9, prev = 7
        )
        .to_string();
        assert!(pt.contains('9') && pt.contains('7') && !pt.contains("%{"), "{pt}");

        rust_i18n::set_locale("en-US");
        assert_eq!(tag_label("record"), "RECORD");
        assert_eq!(metric_noun("wins", 1), "win");
        assert_eq!(metric_noun("wins", 3), "wins");
        assert_eq!(ord_label(2, false), "2nd");
        assert_eq!(ord_label(3, true), "3rd");
        assert_eq!(ord_label(11, false), "11th"); // regra do 11–13
        let en = rust_i18n::t!(
            "world_footer.record_watch.approaching",
            name = "X", gap = 2, noun = metric_noun("wins", 2), value = 50, holder = "Y"
        )
        .to_string();
        assert!(en.contains("2 wins") && !en.contains("%{"), "{en}");

        rust_i18n::set_locale("pt-BR"); // restaura o default pros demais testes.
    }
}
