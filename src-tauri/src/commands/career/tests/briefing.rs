//! Testes de `career::briefing`: briefing da proxima etapa e historico de frases.
//!
//! Fatiado de `tests/mod.rs`, que juntava as dez areas num arquivo so. Os helpers
//! e os `use` continuam no `mod.rs` e chegam aqui pelo glob.

use super::*;

#[test]
fn test_next_race_briefing_summarizes_track_history() {
    let base_dir = create_test_career_dir("load_briefing_track_history");
    let career_id = "career_001";
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season")
        .expect("active season");
    let calendar =
        calendar_queries::get_calendar(&db.conn, &season.id, "mazda_rookie").expect("calendar");
    let race_one = calendar.first().expect("race one");
    let race_two = calendar.get(1).expect("race two");

    db.conn
        .execute(
            "UPDATE calendar SET track_name = ?1 WHERE id IN (?2, ?3)",
            rusqlite::params!["Pista Espelho", race_one.id, race_two.id],
        )
        .expect("update track names");

    let race_result = crate::commands::race::simulate_race_weekend_in_base_dir(
        &base_dir,
        career_id,
        &race_one.id,
    )
    .expect("simulate race");
    let player_finish = race_result
        .player_race
        .race_results
        .iter()
        .find(|entry| entry.is_jogador)
        .map(|entry| entry.finish_position)
        .expect("player finish");
    let player_dnf = race_result
        .player_race
        .race_results
        .iter()
        .find(|entry| entry.is_jogador)
        .map(|entry| entry.is_dnf)
        .expect("player dnf flag");

    let career = load_career_in_base_dir(&base_dir, career_id).expect("load career");
    let track_history = career
        .next_race_briefing
        .as_ref()
        .and_then(|briefing| briefing.track_history.as_ref())
        .expect("track history");

    assert!(track_history.has_data);
    assert_eq!(track_history.starts, 1);
    assert_eq!(
        track_history.best_finish,
        if player_dnf {
            None
        } else {
            Some(player_finish)
        }
    );
    assert_eq!(track_history.last_finish, Some(player_finish));
    assert_eq!(track_history.dnfs, if player_dnf { 1 } else { 0 });
    assert_eq!(track_history.last_visit_season, Some(1));
    assert_eq!(track_history.last_visit_round, Some(1));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_next_race_briefing_exposes_primary_rival() {
    let base_dir = create_test_career_dir("load_briefing_primary_rival");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let player = driver_queries::get_player_driver(&db.conn).expect("player");
    let rival_driver = driver_queries::get_drivers_by_category(&db.conn, "mazda_rookie")
        .expect("category drivers")
        .into_iter()
        .find(|driver| !driver.is_jogador)
        .expect("ai rival");

    db.conn
        .execute(
            "UPDATE drivers SET temp_pontos = 90.0, temp_vitorias = 3, temp_podios = 4 WHERE id = ?1",
            rusqlite::params![player.id],
        )
        .expect("update player");
    db.conn
        .execute(
            "UPDATE drivers SET temp_pontos = 96.0, temp_vitorias = 4, temp_podios = 5 WHERE id = ?1",
            rusqlite::params![rival_driver.id],
        )
        .expect("update rival");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let rival = career
        .next_race_briefing
        .as_ref()
        .and_then(|briefing| briefing.primary_rival.as_ref())
        .expect("primary rival");

    assert_eq!(rival.driver_id, rival_driver.id);
    assert_eq!(rival.driver_name, rival_driver.nome);
    assert_eq!(rival.championship_position, 1);
    assert_eq!(rival.gap_points, 6);
    assert!(rival.is_ahead);

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_next_race_briefing_filters_weekend_stories() {
    let base_dir = create_test_career_dir("load_briefing_weekend_stories");
    let config = AppConfig::load_or_default(&base_dir);
    let db_path = config.saves_dir().join("career_001").join("career.db");
    let db = Database::open_existing(&db_path).expect("db");
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");

    news_queries::insert_news_batch(
        &db.conn,
        &vec![
            NewsItem {
                id: "BRF001".to_string(),
                tipo: NewsType::Rivalidade,
                icone: "R".to_string(),
                titulo: "Duelo esquenta a abertura".to_string(),
                texto: "A tensao entre os protagonistas cresce antes da etapa de abertura."
                    .to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Destaque,
                timestamp: 300,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: Some("P002".to_string()),
                team_id: None,
            },
            NewsItem {
                id: "BRF002".to_string(),
                tipo: NewsType::Hierarquia,
                icone: "H".to_string(),
                titulo: "Equipe reavalia ordem interna".to_string(),
                texto: "O box chega atento ao equilibrio interno antes da largada.".to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Alta,
                timestamp: 250,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: None,
                team_id: None,
            },
            NewsItem {
                id: "BRF003".to_string(),
                tipo: NewsType::Corrida,
                icone: "C".to_string(),
                titulo: "Abertura promete grid apertado".to_string(),
                texto: "A etapa de abertura deve embaralhar o pelotao logo nas primeiras voltas."
                    .to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("mazda_rookie".to_string()),
                categoria_nome: Some("Mazda MX-5 Rookie Cup".to_string()),
                importancia: NewsImportance::Alta,
                timestamp: 200,
                driver_id: Some("P001".to_string()),
                driver_id_secondary: None,
                team_id: None,
            },
            NewsItem {
                id: "BRF004".to_string(),
                tipo: NewsType::Corrida,
                icone: "X".to_string(),
                titulo: "Outra categoria movimenta a semana".to_string(),
                texto: "Essa noticia nao deve entrar na previa da etapa do jogador.".to_string(),
                rodada: Some(1),
                semana_pretemporada: None,
                temporada: season.numero,
                categoria_id: Some("gt4".to_string()),
                categoria_nome: Some("GT4".to_string()),
                importancia: NewsImportance::Destaque,
                timestamp: 400,
                driver_id: None,
                driver_id_secondary: None,
                team_id: None,
            },
        ],
    )
    .expect("seed news");

    let career = load_career_in_base_dir(&base_dir, "career_001").expect("load career");
    let stories = &career
        .next_race_briefing
        .as_ref()
        .expect("briefing")
        .weekend_stories;

    assert_eq!(stories.len(), 3);
    assert_eq!(stories[0].title, "Duelo esquenta a abertura");
    assert!(stories
        .iter()
        .all(|story| !story.title.contains("Outra categoria")));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_briefing_phrase_history_persists_and_keeps_only_last_five_rounds_per_driver_bucket() {
    let base_dir = create_test_career_dir("briefing_phrase_history");
    let career_id = "career_001";

    for round_number in 1..=7 {
        save_briefing_phrase_history_in_base_dir(
            &base_dir,
            career_id,
            1,
            vec![BriefingPhraseEntryInput {
                round_number,
                driver_id: "drv-player".to_string(),
                bucket_key: "p1".to_string(),
                phrase_id: format!("p1-baseline-{round_number}"),
            }],
        )
        .expect("save phrase history");
    }

    let history =
        get_briefing_phrase_history_in_base_dir(&base_dir, career_id).expect("phrase history");

    assert_eq!(history.season_number, 1);
    assert_eq!(history.entries.len(), 5);
    assert_eq!(
        history
            .entries
            .iter()
            .map(|entry| entry.round_number)
            .collect::<Vec<_>>(),
        vec![7, 6, 5, 4, 3]
    );
    assert!(history
        .entries
        .iter()
        .all(|entry| entry.driver_id == "drv-player" && entry.bucket_key == "p1"));

    let _ = fs::remove_dir_all(base_dir);
}

#[test]
fn test_briefing_phrase_history_resets_when_season_changes() {
    let base_dir = create_test_career_dir("briefing_phrase_history_reset");
    let career_id = "career_001";

    save_briefing_phrase_history_in_base_dir(
        &base_dir,
        career_id,
        1,
        vec![BriefingPhraseEntryInput {
            round_number: 5,
            driver_id: "drv-player".to_string(),
            bucket_key: "p2".to_string(),
            phrase_id: "p2-stable-1".to_string(),
        }],
    )
    .expect("save season one");

    let history = save_briefing_phrase_history_in_base_dir(
        &base_dir,
        career_id,
        2,
        vec![BriefingPhraseEntryInput {
            round_number: 1,
            driver_id: "drv-player".to_string(),
            bucket_key: "p2".to_string(),
            phrase_id: "p2-stable-2".to_string(),
        }],
    )
    .expect("save season two");

    assert_eq!(history.season_number, 2);
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].round_number, 1);
    assert_eq!(history.entries[0].phrase_id, "p2-stable-2");

    let _ = fs::remove_dir_all(base_dir);
}
