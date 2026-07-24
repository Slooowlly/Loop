//! Memória entre etapas: o arco narrativo das últimas corridas do jogador.

/// Prévia pré-corrida (narrativa + voz da equipe, CURTAS) para a Sala de Estratégia.
/// Monta o bloco "MEMÓRIA RECENTE" que dá continuidade entre etapas: uma LISTA CURTA
/// das últimas até 3 corridas do jogador na categoria (chegada + manchete do debrief
/// de cada uma). É de propósito compacto — apenas o fio da meada para o servidor
/// retomar a voz, NÃO o debrief inteiro.
///
/// Histórico: antes este bloco despejava o corpo COMPLETO do debrief anterior. Com a
/// prévia reescrita em torno de uma tese dominante (o front já marca o eixo, ex.:
/// "reação a um DNF"), esse despejo passava por cima do eixo e fazia o texto colapsar
/// no último tombo. Agora a memória reforça o eixo em vez de competir com ele.
///
/// Só depende do banco (`conn` + `race_id`), como `build_post_race_facts`. Sem
/// histórico (1ª corrida da carreira / 1ª na categoria) devolve string vazia e o
/// briefing fica idêntico ao de hoje. Etapa antiga sem debrief de IA (offline / gate
/// de engajamento) ainda entra pela chegada — a fala aparece só quando existe.
pub(crate) fn build_recent_arc_facts(conn: &rusqlite::Connection, race_id: &str) -> String {
    use std::fmt::Write;

    let Some(entry) = crate::db::queries::calendar::get_calendar_entry_by_id(conn, race_id)
        .ok()
        .flatten()
    else {
        return String::new();
    };
    let Ok(player) = crate::db::queries::drivers::get_player_driver(conn) else {
        return String::new();
    };
    let season_num = crate::db::queries::seasons::get_active_season(conn)
        .ok()
        .flatten()
        .map(|s| s.numero)
        .unwrap_or(0);

    let recent = match crate::db::queries::race_history::get_recent_races_before(
        conn,
        &player.id,
        &entry.categoria,
        season_num,
        entry.rodada,
        3,
    ) {
        Ok(r) if !r.is_empty() => r,
        _ => return String::new(),
    };

    let mut f = String::new();
    let _ = writeln!(f, "{}", rust_i18n::t!("ai_news.arc.header"));
    for r in &recent {
        let resultado = if r.is_dnf {
            rust_i18n::t!("ai_news.arc.dnf").to_string()
        } else {
            format!("P{}", r.finish)
        };
        let manchete = crate::db::queries::ai_post_race::get_post_race(conn, &r.race_id)
            .ok()
            .flatten()
            .map(|d| d.headline)
            .filter(|h| !h.is_empty());
        let round = r.round.to_string();
        match manchete {
            Some(h) => {
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.arc.line_headline",
                        round = round.as_str(),
                        track = r.track_name.as_str(),
                        result = resultado.as_str(),
                        headline = h.as_str()
                    )
                );
            }
            None => {
                let _ = writeln!(
                    f,
                    "{}",
                    rust_i18n::t!(
                        "ai_news.arc.line",
                        round = round.as_str(),
                        track = r.track_name.as_str(),
                        result = resultado.as_str()
                    )
                );
            }
        }
    }

    f
}
