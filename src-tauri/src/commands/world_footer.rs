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

    let (kind, tag, tone, text) = if ownership_sale {
        (
            "nova_diretoria",
            "MERCADO",
            "reforma",
            format!(
                "Reviravolta nos bastidores: a {} passou por uma troca de comando depois de anos difíceis.",
                team.nome
            ),
        )
    } else if in_crisis {
        let text = if team.financial_state == "collapse" || in_debt {
            format!(
                "Rumores no paddock dão conta de que a {} acumula dívidas e luta para fechar as contas da temporada.",
                team.nome
            )
        } else {
            format!(
                "A {} atravessa um momento financeiro delicado e opera no limite do orçamento neste ano.",
                team.nome
            )
        };
        ("crise_financeira", "FINANÇAS", "crise", text)
    } else if bad_mood {
        (
            "clima_pesado",
            "BASTIDORES",
            "alerta",
            format!(
                "O ambiente interno da {} pesa: o rendimento abaixo do esperado abalou o vestiário.",
                team.nome
            ),
        )
    } else {
        return None;
    };

    Some(WorldNote {
        id: format!("team:{}:{}", team.id, kind),
        tag: tag.to_string(),
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
        tag: "BASTIDORES".to_string(),
        subject: mate.nome.clone(),
        kind: "piloto_time_crise".to_string(),
        tone: "alerta".to_string(),
        text: format!(
            "{} vive um ano conturbado: a {}, sua equipe atual, enfrenta dificuldades financeiras.",
            mate.nome, contract.equipe_nome
        ),
    })
}

/// Substantivo de uma métrica de recorde no plural (para os textos).
fn metric_noun_plural(metric: &str) -> &'static str {
    match metric {
        "wins" => "vitórias",
        "podiums" => "pódios",
        "poles" => "poles",
        "titles" => "títulos",
        _ => "largadas",
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
        let text = match m.metric.as_str() {
            "lap_record" => format!(
                "{} fez a volta mais rápida da história em {}: {} (recorde anterior: {}).",
                m.pilot_name,
                m.context,
                format_lap_ms(m.value),
                m.previous_value.map(format_lap_ms).unwrap_or_default()
            ),
            "comeback" => format!(
                "{} protagonizou a maior recuperação da história da categoria: subiu {} posições numa única corrida.",
                m.pilot_name, m.value
            ),
            "season_wins" => match m.previous_value {
                Some(prev) => format!(
                    "{} chegou a {} vitórias numa única temporada — novo recorde da categoria, superando a marca de {}.",
                    m.pilot_name, m.value, prev
                ),
                None => format!(
                    "{} chegou a {} vitórias numa única temporada — recorde da categoria.",
                    m.pilot_name, m.value
                ),
            },
            "track_wins" => format!(
                "{} é o maior vencedor da história em {}, agora com {} vitórias no circuito.",
                m.pilot_name, m.context, m.value
            ),
            "win_streak" => format!(
                "{} venceu {} corridas seguidas — recorde de sequência da categoria.",
                m.pilot_name, m.value
            ),
            "constructor_titles" => format!(
                "A {} conquistou seu {}º título de construtores — recorde da categoria.",
                m.pilot_name, m.value
            ),
            "team_wins" => format!(
                "A {} tornou-se a maior vencedora da história da categoria, com {} vitórias.",
                m.pilot_name, m.value
            ),
            "one_two" => format!(
                "A {} chegou à sua {}ª dobradinha (1-2) — recorde da categoria.",
                m.pilot_name, m.value
            ),
            "youngest_winner" => format!(
                "{}, aos {} anos, tornou-se o mais jovem a vencer na história da categoria.",
                m.pilot_name, m.value
            ),
            "oldest_winner" => format!(
                "{}, aos {} anos, tornou-se o mais velho a vencer na história da categoria.",
                m.pilot_name, m.value
            ),
            "youngest_champion" => format!(
                "{} sagrou-se campeão aos {} anos — o mais jovem da história da categoria.",
                m.pilot_name, m.value
            ),
            "most_chaotic_race" => format!(
                "{} viveu a corrida mais caótica da história da categoria: {} abandonos.",
                m.pilot_name, m.value
            ),
            "drought_broken" => format!(
                "{} voltou a vencer após {} temporadas de jejum — o maior já quebrado na categoria.",
                m.pilot_name, m.value
            ),
            "closest_championship" => {
                if m.value == 0 {
                    format!(
                        "{} levou o título na decisão mais apertada da história da categoria — empatado em pontos, no critério de desempate.",
                        m.pilot_name
                    )
                } else {
                    format!(
                        "{} levou o título na decisão mais apertada da história da categoria: {} pontos de diferença.",
                        m.pilot_name, m.value
                    )
                }
            }
            "biggest_blowout" => format!(
                "{} dominou a temporada mais desequilibrada da história da categoria: {} pontos à frente do vice.",
                m.pilot_name, m.value
            ),
            "longest_pairing" => format!(
                "{} — {} temporadas juntos, a parceria mais longeva da história da categoria.",
                m.pilot_name, m.value
            ),
            "most_starts_no_win" => format!(
                "{} superou {} como o piloto com mais largadas sem vencer na categoria: {}.",
                m.pilot_name, m.context, m.value
            ),
            "most_career_dnfs" => format!(
                "{} passou {} e é agora quem mais abandonou na história da categoria: {} DNFs.",
                m.pilot_name, m.context, m.value
            ),
            "most_poles_no_title" => format!(
                "{} superou {}: {} poles sem nunca ter sido campeão — recorde da categoria.",
                m.pilot_name, m.context, m.value
            ),
            "most_career_points" => format!(
                "{} superou {} e tornou-se o maior pontuador da história da categoria.",
                m.pilot_name, m.context
            ),
            _ => {
                let noun = metric_noun_plural(&m.metric);
                match m.previous_value {
                    Some(prev) => format!(
                        "{} entrou para a história: quebrou o recorde de {} da categoria, agora em {} (a marca anterior era {}).",
                        m.pilot_name, noun, m.value, prev
                    ),
                    None => format!(
                        "{} entrou para a história ao estabelecer o novo recorde de {} da categoria: {}.",
                        m.pilot_name, noun, m.value
                    ),
                }
            }
        };
        out.push(WorldNote {
            id: format!("broken:{}:{}:{}", categoria, m.metric, m.value),
            tag: "RECORDE".to_string(),
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
            (&records.most_wins, career.wins, "vitórias"),
            (&records.most_podiums, career.podiums, "pódios"),
            (&records.most_starts, career.starts, "largadas"),
        ];
        // Uma métrica por piloto: a mais próxima do recorde.
        let mut best: Option<(i32, String)> = None;
        for (rec, val, noun) in metrics {
            let Some(r) = rec else { continue };
            if r.pilot_id == d.id {
                continue; // já é o recordista.
            }
            let gap = r.value - val;
            if !(0..=RECORD_GAP_MAX).contains(&gap) {
                continue;
            }
            let text = if gap == 0 {
                format!(
                    "{} igualou o recorde histórico de {} da categoria: {}, marca de {}.",
                    d.nome, noun, r.value, r.pilot_name
                )
            } else {
                format!(
                    "{} está a {} {} de igualar o recorde histórico da categoria ({}, de {}).",
                    d.nome,
                    gap,
                    noun_gap(noun, gap),
                    r.value,
                    r.pilot_name
                )
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
            tag: "RECORDE".to_string(),
            subject: text.clone(),
            kind: "recorde_a_caminho".to_string(),
            tone: "recorde".to_string(),
            text,
        });
    }
    out
}

/// Singulariza o substantivo do recorde quando falta só 1 (1 vitória, não "vitórias").
fn noun_gap(noun: &str, gap: i32) -> &'static str {
    match (noun, gap) {
        ("vitórias", 1) => "vitória",
        ("pódios", 1) => "pódio",
        ("largadas", 1) => "largada",
        ("vitórias", _) => "vitórias",
        ("pódios", _) => "pódios",
        _ => "largadas",
    }
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
        format!(
            "{} é o maior fenômeno de público da categoria: onde corre, as arquibancadas lotam e os patrocinadores fazem fila.",
            star.nome
        )
    } else {
        format!(
            "{} virou um dos grandes nomes da categoria e arrasta torcida para os autódromos — presença de público garantida onde disputa.",
            star.nome
        )
    };

    used_drivers.insert(star.id.clone());
    Some(WorldNote {
        id: format!("star:{}", star.id),
        tag: "BASTIDORES".to_string(),
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
}
