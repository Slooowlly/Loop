//! Pilotos de interesse do jogador (Nemesis e Rivais), lidos do motor de rivalidade.

use super::*;

/// Um piloto de interesse do jogador (Nemesis ou Rival) — o mínimo para decorar o
/// nome nas telas. Vem do motor de rivalidade (intensidade percebida acumulada).
#[derive(Debug, Clone, serde::Serialize)]
pub struct RivalInterest {
    pub driver_id: String,
    pub driver_name: String,
    /// Intensidade percebida (0–100) no momento — para ordenar/depurar.
    pub perceived: f64,
    /// Nome determinístico da rivalidade ("A Revanche de Interlagos"), do 1º capítulo.
    /// `None` até haver um episódio registrado.
    pub label: Option<String>,
    /// Retrospecto direto (h2h): capítulos que o JOGADOR levou a melhor.
    pub h2h_player_wins: i32,
    /// Capítulos que o RIVAL levou a melhor.
    pub h2h_rival_wins: i32,
    /// Total de capítulos registrados do par.
    pub chapters: i32,
}

/// Os 3 pilotos de interesse mostrados ao jogador: 1 Nemesis + até 2 Rivais.
/// O motor rastreia mais; só estes recebem marcador nas telas.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerInterests {
    pub nemesis: Option<RivalInterest>,
    pub rivais: Vec<RivalInterest>,
}

/// Intensidade mínima para ser Nemesis ("rivalidade clara"). Abaixo disso, sem Nemesis.
const NEMESIS_MIN_PERCEIVED: f64 = 40.0;
/// Intensidade mínima para ser Rival mostrado ("rivalidade inicial").
const RIVAL_MIN_PERCEIVED: f64 = 20.0;
/// Margem de histerese: o Nemesis reinante só é destituído se outro rival o superar em
/// intensidade por mais que isto — evita o Nemesis trocar toda semana no empate técnico.
const NEMESIS_HYSTERESIS_MARGIN: f64 = 10.0;

/// Seleciona os pilotos de interesse do jogador a partir do estado acumulado do motor
/// de rivalidade: Nemesis = maior intensidade (se ≥ 40); Rivais = os 2 seguintes
/// (se ≥ 20). Sem histerese ainda (a acumulação do eixo histórico já dá estabilidade;
/// histerese persistida é um refino futuro). Atravessa categorias.
pub(crate) fn get_player_interests_in_base_dir(
    base_dir: &Path,
    career_id: &str,
) -> Result<PlayerInterests, String> {
    let (db, _dir, _meta) = open_career_resources_read_only(base_dir, career_id)?;
    let current = crate::db::queries::player_nemesis::get_current_nemesis(&db.conn).unwrap_or(None);
    let interests = select_player_interests(&db.conn, current.as_deref());
    // Persiste a eventual troca de Nemesis (o estado da histerese). Best-effort — só
    // aqui (no load, infrequente); o overlay lê e não escreve.
    let new_id = interests.nemesis.as_ref().map(|n| n.driver_id.as_str());
    if new_id != current.as_deref() {
        let _ = crate::db::queries::player_nemesis::set_current_nemesis(&db.conn, new_id);
    }
    Ok(interests)
}

/// Núcleo da seleção (Nemesis + Rivais) sobre uma conexão já aberta — reusado pelo
/// comando e pelo overlay. `current_nemesis` = o Nemesis reinante (para a histerese);
/// passe `None` para seleção pura por intensidade. NÃO escreve nada (quem persiste a
/// troca é o caller). Best-effort: erro/sem jogador → vazio.
pub(crate) fn select_player_interests(
    conn: &rusqlite::Connection,
    current_nemesis: Option<&str>,
) -> PlayerInterests {
    let empty = PlayerInterests {
        nemesis: None,
        rivais: Vec::new(),
    };

    let player_id: String = match conn.query_row(
        "SELECT id FROM drivers WHERE is_jogador = 1 LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(id) => id,
        Err(_) => return empty,
    };

    let mut rivalries = match crate::rivalry::get_pilot_rivalries(conn, &player_id) {
        Ok(r) => r,
        Err(_) => return empty,
    };
    rivalries.sort_by(|a, b| {
        b.perceived_intensity
            .partial_cmp(&a.perceived_intensity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let name_of = |id: &str| {
        crate::db::queries::drivers::get_driver(conn, id)
            .map(|d| d.nome)
            .unwrap_or_else(|_| id.to_string())
    };
    let to_interest = |r: &crate::rivalry::PilotRivalrySummary| {
        // Um só fetch de episódios do par → label (1º capítulo) + retrospecto h2h.
        let eps = crate::db::queries::rivalry_episodes::get_episodes_for_pair(
            conn,
            &player_id,
            &r.rival_id,
        )
        .unwrap_or_default();
        let label = eps
            .first()
            .map(crate::db::queries::rivalry_episodes::rivalry_label);
        let mut pw = 0;
        let mut rw = 0;
        for e in &eps {
            match e.winner_id.as_deref() {
                Some(w) if w == player_id.as_str() => pw += 1,
                Some(w) if w == r.rival_id.as_str() => rw += 1,
                _ => {}
            }
        }
        RivalInterest {
            driver_id: r.rival_id.clone(),
            driver_name: name_of(&r.rival_id),
            perceived: r.perceived_intensity,
            label,
            h2h_player_wins: pw,
            h2h_rival_wins: rw,
            chapters: eps.len() as i32,
        }
    };

    let top = rivalries.first();
    // Reinante ainda presente e acima do piso de Nemesis?
    let reign = current_nemesis
        .and_then(|cur| rivalries.iter().find(|r| r.rival_id == cur))
        .filter(|r| r.perceived_intensity >= NEMESIS_MIN_PERCEIVED);

    // Histerese: mantém o reinante, salvo se outro o superar pela margem.
    let nemesis_summary: Option<&crate::rivalry::PilotRivalrySummary> = match (reign, top) {
        (Some(cur), Some(top)) => {
            if top.rival_id != cur.rival_id
                && top.perceived_intensity > cur.perceived_intensity + NEMESIS_HYSTERESIS_MARGIN
            {
                Some(top)
            } else {
                Some(cur)
            }
        }
        (Some(cur), None) => Some(cur),
        (None, Some(top)) if top.perceived_intensity >= NEMESIS_MIN_PERCEIVED => Some(top),
        _ => None,
    };

    let nemesis_id = nemesis_summary.map(|r| r.rival_id.clone());
    let nemesis = nemesis_summary.map(|r| to_interest(r));

    let rivais: Vec<RivalInterest> = rivalries
        .iter()
        .filter(|r| Some(&r.rival_id) != nemesis_id.as_ref())
        .filter(|r| r.perceived_intensity >= RIVAL_MIN_PERCEIVED)
        .take(2)
        .map(|r| to_interest(r))
        .collect();

    PlayerInterests { nemesis, rivais }
}
