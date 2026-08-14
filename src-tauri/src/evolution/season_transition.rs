use std::collections::HashMap;

use rand::Rng;
use rusqlite::Connection;

use crate::db::queries::meta as meta_queries;

use crate::calendar::full_season::generate_full_season_calendar;
use crate::calendar::generate_all_calendars_with_id_factory;
use crate::constants::categories::get_all_categories;
use crate::db::queries::calendar as calendar_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::seasons as season_queries;
use crate::db::queries::teams as team_queries;
use crate::evolution::context::StandingEntry;
use crate::generators::ids::{next_id, next_ids, IdType};
use crate::models::enums::SeasonPhase;
use crate::models::season::Season;

pub(crate) fn create_and_persist_new_season(
    conn: &Connection,
    season: &Season,
) -> Result<Season, String> {
    let new_season_id = next_id(conn, IdType::Season)
        .map_err(|e| format!("Falha ao gerar ID da nova temporada: {e}"))?;
    let new_year = season.ano + 1;
    let new_season = Season::new(new_season_id, season.numero + 1, new_year);
    season_queries::insert_season(conn, &new_season)
        .map_err(|e| format!("Falha ao inserir nova temporada: {e}"))?;
    Ok(new_season)
}

pub(crate) fn create_next_season_9d(
    conn: &Connection,
    season: &Season,
    fase_inicial: SeasonPhase,
    seed: u64,
) -> Result<Season, String> {
    let mut new_season = create_and_persist_new_season(conn, season)?;
    if new_season.fase != fase_inicial {
        season_queries::update_season_fase(conn, &new_season.id, &fase_inicial)
            .map_err(|e| format!("Falha ao definir fase inicial da temporada 9D: {e}"))?;
        new_season.fase = fase_inicial;
    }
    generate_full_season_calendar(conn, &new_season.id, new_season.ano, seed)
        .map_err(|e| format!("Falha ao inserir calendario 9D da nova temporada: {e}"))?;
    Ok(new_season)
}

/// Arquiva o snapshot completo de cada piloto ao fim da temporada.
/// Deve ser chamado DEPOIS do crescimento de atributos e ANTES da promoção/rebaixamento,
/// para capturar atributos finais e categoria original da temporada.
pub(crate) fn archive_driver_season(
    conn: &Connection,
    season: &Season,
    standings_by_driver: &HashMap<String, StandingEntry>,
) -> Result<(), String> {
    let drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao carregar pilotos para arquivo historico: {e}"))?;

    for driver in &drivers {
        let standing = standings_by_driver.get(&driver.id);
        let categoria = standing.map(|s| s.category.as_str()).unwrap_or_default();
        let classe = standing.and_then(|s| s.classe.as_deref());
        let team_id = standing.and_then(|s| s.team_id.as_deref());
        let posicao_campeonato = standing.map(|s| s.position);
        let total_pilotos = standing.map(|s| s.total_drivers);
        let titulos = if posicao_campeonato == Some(1) { 1 } else { 0 };

        let snapshot = serde_json::json!({
            "piloto_id":            driver.id,
            "nome":                 driver.nome,
            "idade":                driver.idade,
            "nacionalidade":        driver.nacionalidade,
            "is_jogador":           driver.is_jogador,
            "season_number":        season.numero,
            "ano":                  season.ano,
            "categoria":            categoria,
            "classe":               classe,
            "team_id":              team_id,
            "posicao_campeonato":   posicao_campeonato,
            "total_pilotos":        total_pilotos,
            "pontos":               driver.stats_temporada.pontos,
            "vitorias":             driver.stats_temporada.vitorias,
            "podios":               driver.stats_temporada.podios,
            "poles":                driver.stats_temporada.poles,
            "titulos":              titulos,
            "corridas":             driver.stats_temporada.corridas,
            "dnfs":                 driver.stats_temporada.dnfs,
            "posicao_media":        driver.stats_temporada.posicao_media,
            "melhor_resultado":     driver.melhor_resultado_temp,
            "ultimos_resultados":   driver.ultimos_resultados,
            "atributos": {
                "skill":                driver.atributos.skill,
                "consistencia":         driver.atributos.consistencia,
                "racecraft":            driver.atributos.racecraft,
                "defesa":               driver.atributos.defesa,
                "ritmo_classificacao":  driver.atributos.ritmo_classificacao,
                "gestao_pneus":         driver.atributos.gestao_pneus,
                "habilidade_largada":   driver.atributos.habilidade_largada,
                "adaptabilidade":       driver.atributos.adaptabilidade,
                "fator_chuva":          driver.atributos.fator_chuva,
                "fitness":              driver.atributos.fitness,
                "experiencia":          driver.atributos.experiencia,
                "desenvolvimento":      driver.atributos.desenvolvimento,
                "aggression":           driver.atributos.aggression,
                "smoothness":           driver.atributos.smoothness,
                "midia":                driver.atributos.midia,
                "carisma":              driver.atributos.carisma,
                "mentalidade":          driver.atributos.mentalidade,
                "confianca":            driver.atributos.confianca,
            },
            "motivacao":                driver.motivacao,
            "temporadas_na_categoria":  driver.temporadas_na_categoria,
        });

        conn.execute(
            "INSERT OR REPLACE INTO driver_season_archive
             (piloto_id, season_number, ano, nome, categoria, posicao_campeonato, pontos, snapshot_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &driver.id,
                season.numero,
                season.ano,
                &driver.nome,
                categoria,
                posicao_campeonato,
                driver.stats_temporada.pontos,
                snapshot.to_string(),
            ],
        )
        .map_err(|e| {
            format!(
                "Falha ao arquivar temporada do piloto '{}': {e}",
                driver.id
            )
        })?;

        // Marco de TÍTULOS: com o arquivo desta temporada já gravado, o campeão pode
        // ter se tornado o dono ISOLADO do recorde de títulos da categoria. A partir de
        // 3 títulos, para não marcar o bicampeão. Camada narrativa — erro é silencioso.
        // Os dois lados da comparação são filtrados pela MESMA classe: nas categorias
        // multiclasse o campeonato é por classe, então o recorde também é.
        if posicao_campeonato == Some(1) && !categoria.is_empty() {
            let champ_titles = crate::db::queries::race_history::get_pilot_category_titles(
                conn, &driver.id, categoria, classe,
            )
            .unwrap_or(0);
            let others_max =
                crate::db::queries::race_history::get_category_titles_leader_excluding(
                    conn, categoria, &driver.id, classe,
                )
                .unwrap_or(0);
            if champ_titles >= 3 && champ_titles > others_max {
                let _ = crate::db::queries::milestones::insert_milestone(
                    conn,
                    categoria,
                    &crate::db::queries::milestones::RecordMilestone {
                        metric: "titles".to_string(),
                        pilot_id: driver.id.clone(),
                        pilot_name: driver.nome.clone(),
                        value: champ_titles,
                        previous_value: Some(champ_titles - 1),
                        context: String::new(),
                        season_number: season.numero,
                        ano: season.ano,
                        round: 0,
                    },
                );
            }

            // Marcos escalares de fim de temporada (rodam uma vez por categoria, aqui no
            // campeão): campeão mais jovem, campeonato mais apertado/dominante, dupla mais
            // longeva. Emite quando o candidato supera o recorde existente.
            let emit_scalar =
                |kind: &str, subj_id: &str, subj_name: &str, value: i32, higher: bool| {
                    if let Ok(Some(prev)) = crate::db::queries::milestones::update_scalar_and_check(
                        conn,
                        categoria,
                        kind,
                        subj_id,
                        subj_name,
                        value,
                        "",
                        season.numero,
                        0,
                        higher,
                    ) {
                        let _ = crate::db::queries::milestones::insert_milestone(
                            conn,
                            categoria,
                            &crate::db::queries::milestones::RecordMilestone {
                                metric: kind.to_string(),
                                pilot_id: subj_id.to_string(),
                                pilot_name: subj_name.to_string(),
                                value,
                                previous_value: Some(prev),
                                context: String::new(),
                                season_number: season.numero,
                                ano: season.ano,
                                round: 0,
                            },
                        );
                    }
                };

            // Campeão mais jovem (idade do campeão nesta temporada).
            if driver.idade > 0 {
                emit_scalar(
                    "youngest_champion",
                    &driver.id,
                    &driver.nome,
                    driver.idade as i32,
                    false,
                );
            }

            // Campeonato mais apertado / mais dominante (gap de pontos entre 1º e 2º).
            if let Ok(standings) = crate::db::queries::race_history::get_category_standings(
                conn, &season.id, categoria,
            ) {
                if standings.len() >= 2 {
                    let gap = (standings[0].points - standings[1].points).round() as i32;
                    if gap >= 0 {
                        emit_scalar("closest_championship", &driver.id, &driver.nome, gap, false);
                        emit_scalar("biggest_blowout", &driver.id, &driver.nome, gap, true);
                    }
                }
            }

            // Dupla piloto-equipe mais longeva da categoria.
            if let Ok(Some(pair)) =
                crate::db::queries::race_history::get_category_longest_pairing(conn, categoria)
            {
                emit_scalar(
                    "longest_pairing",
                    &pair.pilot_id,
                    &pair.pilot_name,
                    pair.value,
                    true,
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn reset_driver_season_stats(conn: &Connection) -> Result<(), String> {
    let mut drivers = driver_queries::get_all_drivers(conn)
        .map_err(|e| format!("Falha ao recarregar pilotos apos a nova temporada: {e}"))?;
    for driver in &mut drivers {
        driver.reset_season_stats();
        driver_queries::update_driver(conn, driver)
            .map_err(|e| format!("Falha ao resetar stats do piloto '{}': {e}", driver.nome))?;
    }
    Ok(())
}

pub(crate) fn reset_team_season_stats(
    conn: &Connection,
    new_season_numero: i32,
) -> Result<(), String> {
    let teams = team_queries::get_all_teams(conn)
        .map_err(|e| format!("Falha ao recarregar equipes: {e}"))?;
    for team in &teams {
        team_queries::reset_team_season_stats(conn, &team.id)
            .map_err(|e| format!("Falha ao resetar stats da equipe '{}': {e}", team.id))?;
        conn.execute(
            "UPDATE teams SET temporada_atual = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            rusqlite::params![new_season_numero, &team.id],
        )
        .map_err(|e| format!("Falha ao atualizar temporada da equipe '{}': {e}", team.id))?;
    }
    Ok(())
}

/// LEGADO 9D - semeia calendario do fluxo pre-9D.
/// Mantido enquanto testes e saves legados ainda referenciam o modelo antigo.
#[allow(dead_code)]
pub(crate) fn seed_new_calendar(
    conn: &Connection,
    new_season_id: &str,
    new_year: i32,
    rng: &mut impl Rng,
) -> Result<(), String> {
    let total_new_races: u32 = get_all_categories()
        .iter()
        .map(|category| category.corridas_por_temporada as u32)
        .sum();
    let race_ids = next_ids(conn, IdType::Race, total_new_races)
        .map_err(|e| format!("Falha ao gerar IDs do calendario: {e}"))?;
    let mut race_ids_iter = race_ids.into_iter();
    let calendars = generate_all_calendars_with_id_factory(
        new_season_id,
        new_year,
        &mut || race_ids_iter.next().expect("calendar race id"),
        rng,
    )?;
    let all_entries: Vec<_> = calendars
        .values()
        .flat_map(|entries| entries.iter().cloned())
        .collect();
    calendar_queries::insert_calendar_entries(conn, &all_entries)
        .map_err(|e| format!("Falha ao inserir calendario da nova temporada: {e}"))?;
    Ok(())
}

pub(crate) fn update_meta_for_new_season(
    conn: &Connection,
    new_season_numero: i32,
    new_year: i32,
) -> Result<(), String> {
    meta_queries::set_current_season(conn, new_season_numero)
        .map_err(|e| format!("Falha ao atualizar meta current_season: {e}"))?;
    meta_queries::set_current_year(conn, new_year)
        .map_err(|e| format!("Falha ao atualizar meta current_year: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! A criação da temporada nova e a geração dos calendários não tinham teste
    //! próprio: a cobertura vinha só dos testes de integração da virada inteira. Um
    //! erro de calendário aqui contamina a temporada antes de qualquer corrida.

    use super::*;
    use crate::db::migrations;

    /// Todas as etapas da temporada, varrendo divisão por divisão (não existe uma
    /// consulta "calendário inteiro" — a produção sempre lê por categoria).
    fn calendario_inteiro(
        conn: &Connection,
        season_id: &str,
    ) -> Vec<crate::calendar::CalendarEntry> {
        get_all_categories()
            .iter()
            .flat_map(|categoria| {
                calendar_queries::get_calendar(conn, season_id, categoria.id)
                    .unwrap_or_else(|e| panic!("calendário de {}: {e}", categoria.id))
            })
            .collect()
    }

    /// Banco com a temporada corrente JÁ ENCERRADA, que é o estado em que a virada
    /// chama estas funções (`finalize_season` roda antes, em `orquestracao.rs`). Sem o
    /// encerramento o banco recusa a temporada nova: só existe uma ativa por save.
    fn conn_com_temporada(numero: i32, ano: i32) -> (Connection, Season) {
        let conn = Connection::open_in_memory().expect("banco em memória");
        migrations::run_all(&conn).expect("migrações");
        let season = Season::new("S001".to_string(), numero, ano);
        season_queries::insert_season(&conn, &season).expect("insere temporada");
        season_queries::finalize_season(&conn, &season.id).expect("encerra a temporada corrente");
        (conn, season)
    }

    #[test]
    fn temporada_nova_avanca_numero_e_ano() {
        let (conn, season) = conn_com_temporada(3, 2029);
        let nova = create_and_persist_new_season(&conn, &season).expect("cria temporada");

        assert_eq!(nova.numero, 4, "o número da temporada precisa avançar 1");
        assert_eq!(nova.ano, 2030, "o ano precisa avançar 1");
        assert_ne!(nova.id, season.id, "a temporada nova precisa de id próprio");
        assert!(
            season_queries::get_season_by_id(&conn, &nova.id)
                .expect("consulta")
                .is_some(),
            "a temporada nova precisa estar persistida"
        );
    }

    #[test]
    fn calendario_9d_nasce_completo_e_na_fase_pedida() {
        let (conn, season) = conn_com_temporada(1, 2027);
        let nova = create_next_season_9d(&conn, &season, SeasonPhase::PreTemporada, 42)
            .expect("cria temporada 9D");

        assert_eq!(nova.fase, SeasonPhase::PreTemporada);

        let entradas = calendario_inteiro(&conn, &nova.id);
        assert_eq!(
            entradas.len(),
            74,
            "o calendário 9D tem 74 entradas (5+5+8+8+8+10+10+14+6)"
        );
        for entrada in &entradas {
            let sw = entrada
                .season_week
                .unwrap_or_else(|| panic!("entrada {} sem season_week", entrada.id));
            assert!(
                (10..=51).contains(&sw),
                "entrada {} com season_week {sw} fora de 10–51",
                entrada.id
            );
            assert_eq!(
                entrada.season_phase,
                SeasonPhase::Temporada,
                "toda etapa do 9D nasce na fase Temporada"
            );
        }
        assert!(
            entradas.iter().all(|e| e.categoria != "lmp2"),
            "lmp2 é classe da Endurance, não divisão com calendário próprio"
        );
    }

    #[test]
    fn calendario_9d_e_deterministico_pela_seed() {
        let (conn_a, season_a) = conn_com_temporada(1, 2027);
        let nova_a = create_next_season_9d(&conn_a, &season_a, SeasonPhase::PreTemporada, 7)
            .expect("cria A");
        let (conn_b, season_b) = conn_com_temporada(1, 2027);
        let nova_b = create_next_season_9d(&conn_b, &season_b, SeasonPhase::PreTemporada, 7)
            .expect("cria B");

        let pistas_a: Vec<u32> = calendario_inteiro(&conn_a, &nova_a.id)
            .iter()
            .map(|e| e.track_id)
            .collect();
        let pistas_b: Vec<u32> = calendario_inteiro(&conn_b, &nova_b.id)
            .iter()
            .map(|e| e.track_id)
            .collect();
        assert_eq!(pistas_a, pistas_b, "mesma seed, mesmo calendário");
    }

    #[test]
    fn meta_acompanha_a_temporada_nova() {
        let (conn, _) = conn_com_temporada(1, 2027);
        update_meta_for_new_season(&conn, 2, 2028).expect("atualiza meta");

        assert_eq!(
            meta_queries::get_meta_value(&conn, "current_season").expect("lê season"),
            Some("2".to_string())
        );
        assert_eq!(
            meta_queries::get_meta_value(&conn, "current_year").expect("lê ano"),
            Some("2028".to_string())
        );
    }
}
