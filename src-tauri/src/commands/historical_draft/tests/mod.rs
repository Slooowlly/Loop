use std::path::{Path, PathBuf};

use super::{
    create_historical_career_draft_base_for_test,
    create_historical_career_draft_for_range_for_test, discard_career_draft_in_base_dir,
    get_career_draft_in_base_dir, simulate_historical_range,
    update_career_draft_identity_in_base_dir,
};
use crate::commands::career::get_driver_detail_in_base_dir;
use crate::commands::career_team_dossier::get_team_history_dossier_in_base_dir;
use crate::commands::career_types::{
    CreateHistoricalDraftInput, FinalizeHistoricalDraftInput, SaveLifecycleStatus,
    UpdateDraftIdentityInput,
};
use crate::commands::global_driver_rankings::get_global_driver_rankings_in_base_dir;
use crate::config::app_config::AppConfig;
use crate::constants::categories::{get_all_categories, runs_in_special_phase};
use crate::constants::historical_timeline::is_category_active_in_year;
use crate::db::connection::Database;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::{calendar as calendar_queries, seasons as season_queries};
use crate::db::queries::{contracts as contract_queries, teams as team_queries};
use crate::finance::planning::{category_finance_scale, category_finance_scale_for};
use crate::models::enums::{RaceStatus, SeasonPhase};
use std::collections::HashMap;

#[test]
fn create_draft_base_world_has_no_player_and_starts_in_2000() {
    let base_dir = unique_test_dir("draft_base_world");
    let input = sample_draft_input();

    let state = create_historical_career_draft_base_for_test(&base_dir, input)
        .expect("draft base should be created");

    assert_eq!(state.lifecycle_status, SaveLifecycleStatus::Draft);
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);
    assert!(driver_queries::get_player_driver(&db.conn).is_err());
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    assert_eq!(season.ano, 2000);

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_draft_base_starts_with_full_9d_calendar_and_simulable_first_race() {
    let base_dir = unique_test_dir("draft_base_9d_calendar");
    let input = sample_draft_input();

    let state = create_historical_career_draft_base_for_test(&base_dir, input)
        .expect("draft base should be created");

    let career_id = state.career_id.as_deref().expect("draft career id");
    let mut db = open_draft_db(&base_dir, career_id);
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    assert_eq!(season.status.as_str(), "EmAndamento");
    assert_eq!(season.fase, SeasonPhase::Temporada);

    let entries =
        calendar_queries::get_pending_races(&db.conn, &season.id).expect("pending calendar");
    assert_eq!(entries.len(), 74);
    assert!(entries.iter().all(|entry| {
        entry.season_phase == SeasonPhase::Temporada && matches!(entry.season_week, Some(10..=51))
    }));

    let special_entry_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM special_team_entries", [], |row| {
            row.get(0)
        })
        .expect("special entry count");
    assert_eq!(special_entry_count, 0);

    let special_contract_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
            [],
            |row| row.get(0),
        )
        .expect("special contract count");
    assert_eq!(special_contract_count, 0);

    let first_active_race = entries
        .iter()
        .filter(|entry| is_category_active_in_year(&entry.categoria, season.ano))
        .min_by_key(|entry| (entry.season_week.unwrap_or(u32::MAX), entry.rodada))
        .cloned()
        .expect("first active race");
    crate::commands::race::simulate_historical_category_race(&mut db, &first_active_race)
        .expect("first historical race should simulate");

    let simulated = calendar_queries::get_calendar_entry_by_id(&db.conn, &first_active_race.id)
        .expect("race lookup")
        .expect("race after simulation");
    assert_eq!(simulated.status, RaceStatus::Concluida);

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_simulation_reaches_playable_year_with_results_and_no_news() {
    let base_dir = unique_test_dir("historical_short");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2001, 2002)
            .expect("historical generation should finish");

    assert_eq!(state.lifecycle_status, SaveLifecycleStatus::Draft);
    let career_id = state.career_id.as_deref().expect("draft career id");
    let career_dir = AppConfig::load_or_default(&base_dir)
        .saves_dir()
        .join(career_id);
    assert!(!career_dir.join("preseason_plan.json").exists());
    let db = open_draft_db(&base_dir, career_id);
    let season = season_queries::get_active_season(&db.conn)
        .expect("season query")
        .expect("active season");
    assert_eq!(season.ano, 2002);
    assert_eq!(season.fase, SeasonPhase::Temporada);

    let playable_calendar = calendar_queries::get_pending_races(&db.conn, &season.id)
        .expect("playable pending calendar");
    assert_eq!(playable_calendar.len(), 74);
    assert!(playable_calendar.iter().all(|entry| {
        entry.season_phase == SeasonPhase::Temporada && matches!(entry.season_week, Some(10..=51))
    }));

    let result_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM race_results", [], |row| row.get(0))
        .expect("race result count");
    assert!(result_count > 0);

    let news_count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM news", [], |row| row.get(0))
        .expect("news count");
    assert_eq!(news_count, 0);

    let _ = std::fs::remove_dir_all(base_dir);
}

/// As categorias do bloco especial (Endurance, Production Challenger) rodam no Sistema de
/// Nível do Carro como todas as outras.
///
/// Regressão: o cérebro de manutenção morava depois de um `return` que essas categorias
/// tomavam em `apply_race_result_to_database`, então elas NUNCA criavam linha em `team_car`.
/// `Team::effective_car_performance` caía na coluna legada `car_performance` — que o sistema
/// de peças não atualiza e que não tem teto — e o campeonato de GT3 do Endurance passou a ser
/// o único do jogo decidido por um número fora de escala. Uma equipe chegava a ~5× o topo do
/// domínio e ganhava 20 dos 21 títulos.
#[test]
fn endurance_teams_race_on_the_car_level_system_with_class_ceilings() {
    let base_dir = unique_test_dir("endurance_car_level");
    let input = sample_draft_input();

    // Até 2006: a classe GT3 do Endurance começa em 2005, então há temporadas correndo nela.
    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2006, 2007)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);

    let teams_without_car: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
               FROM teams t
              WHERE t.ativa = 1
                AND t.categoria IN ('endurance', 'production_challenger')
                AND NOT EXISTS (SELECT 1 FROM team_car c WHERE c.team_id = t.id)",
            [],
            |row| row.get(0),
        )
        .expect("endurance teams without car");
    assert_eq!(
        teams_without_car, 0,
        "toda equipe do bloco especial precisa de carro em team_car"
    );

    // O teto é o da CLASSE: o GT4 do Endurance não pode herdar o teto do LMP2 só por
    // dividir a categoria com ele.
    let mut stmt = db
        .conn
        .prepare(
            "SELECT COALESCE(t.classe, ''), MAX(c.level)
               FROM teams t
               JOIN team_car c ON c.team_id = t.id
              WHERE t.ativa = 1 AND t.categoria = 'endurance'
              GROUP BY COALESCE(t.classe, '')",
        )
        .expect("prepare class ceiling query");
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query class ceilings")
        .collect::<Result<Vec<_>, _>>()
        .expect("read class ceilings");
    assert!(!rows.is_empty(), "o Endurance deve ter equipes com carro");

    // O limite por classe é o teto de DESENVOLVIMENTO (teto natural + a parede), não o teto
    // natural: acima dele o custo explode, mas é território legítimo de quem sangra dinheiro
    // (design §6, "9–10 só pros ultra-ricos"). O que o teste guarda é que a classe manda —
    // um GT4 do Endurance não pode andar na faixa do LMP2 só por dividir a categoria.
    for (classe, max_level) in rows {
        let natural = crate::car::cost::category_ceiling_for("endurance", Some(classe.as_str()));
        let expected = crate::car::cost::development_ceiling(natural) as i64;
        assert!(
            max_level <= expected,
            "classe '{classe}': nivel maximo {max_level} passou do teto de desenvolvimento {expected} (natural {natural})"
        );
    }

    let _ = std::fs::remove_dir_all(base_dir);
}

/// A privateer (sem `marca`) constrói até um nível abaixo da fábrica nas arenas GT3 — é a
/// hierarquia real × fictícia expressa no Sistema de Nível do Carro, no lugar da banda que
/// prendia as marcas reais num teto e deixava as fictícias crescerem sem limite.
#[test]
fn gt3_privateer_ceiling_stays_below_the_factory_ceiling() {
    use crate::constants::teams::get_team_templates;
    use crate::models::team::Team;
    use rand::SeedableRng;

    let factory = get_team_templates("gt3")
        .into_iter()
        .find(|template| template.marca.is_some())
        .expect("gt3 deve ter equipe de marca real");
    let privateer = get_team_templates("gt3")
        .into_iter()
        .find(|template| template.marca.is_none())
        .expect("gt3 deve ter equipe ficticia");

    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let factory_team =
        Team::from_template_with_rng(factory, "gt3", "T001".to_string(), 2026, &mut rng);
    let privateer_team =
        Team::from_template_with_rng(privateer, "gt3", "T002".to_string(), 2026, &mut rng);

    assert!(
        privateer_team.car_ceiling() < factory_team.car_ceiling(),
        "privateer ({}) deveria ter teto abaixo da fabrica ({})",
        privateer_team.car_ceiling(),
        factory_team.car_ceiling()
    );

    // Fora das arenas GT3 nada muda. A classe GT4 do Endurance é o caso limpo: tem equipes
    // fictícias e nenhuma disputa com marca real, então elas mantêm o teto cheio da classe.
    let endurance_gt4 = get_team_templates("endurance")
        .into_iter()
        .find(|template| template.marca.is_none() && template.classe == Some("gt4"))
        .expect("endurance/gt4 deve ter equipe ficticia");
    let endurance_gt4_team = Team::from_template_with_rng(
        endurance_gt4,
        "endurance",
        "T003".to_string(),
        2026,
        &mut rng,
    );
    assert_eq!(
        endurance_gt4_team.car_ceiling(),
        crate::car::cost::category_ceiling("gt4"),
        "ficticia fora da arena GT3 mantem o teto cheio da classe"
    );
}

#[test]
fn historical_simulation_runs_production_and_endurance_without_special_artifacts() {
    let base_dir = unique_test_dir("historical_special_events");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2004, 2005)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);

    let special_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM contracts
                 WHERE tipo = 'Especial' AND status = 'Expirado'",
            [],
            |row| row.get(0),
        )
        .expect("special contract count");
    let active_special_contracts: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM contracts
                 WHERE tipo = 'Especial' AND status = 'Ativo'",
            [],
            |row| row.get(0),
        )
        .expect("active special contract count");
    let special_races: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM calendar
                 WHERE categoria IN ('production_challenger', 'endurance')
                   AND status = 'Concluida'",
            [],
            |row| row.get(0),
        )
        .expect("special calendar count");
    let special_entries: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM special_team_entries", [], |row| {
            row.get(0)
        })
        .expect("special team entries count");
    let special_results: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM race_results rr
                 JOIN calendar c ON c.id = rr.race_id
                 WHERE c.categoria IN ('production_challenger', 'endurance')",
            [],
            |row| row.get(0),
        )
        .expect("special race result count");

    assert_eq!(special_contracts, 0);
    assert_eq!(active_special_contracts, 0);
    assert_eq!(special_entries, 0);
    assert!(special_races > 0);
    assert!(special_results > 0);

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_simulation_completes_preseason_lineups_for_gt3() {
    let base_dir = unique_test_dir("historical_gt3_lineups");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2002, 2003)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);

    let empty_slots: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM teams
                 WHERE categoria = 'gt3'
                   AND ativa = 1
                   AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
            [],
            |row| row.get(0),
        )
        .expect("gt3 empty slot count");

    assert_eq!(
        empty_slots, 0,
        "historical GT3 simulation must auto-complete preseason transfers before the next season"
    );

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn closed_system_playable_world_has_no_orphans_and_drivers_raced() {
    let base_dir = unique_test_dir("closed_system_validation");
    let input = sample_draft_input();
    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2024, 2025)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);

    let count = |sql: &str| -> i64 { db.conn.query_row(sql, [], |row| row.get(0)).expect(sql) };

    let total = count("SELECT COUNT(*) FROM drivers");
    let active = count("SELECT COUNT(*) FROM drivers WHERE status = 'Ativo'");
    let retired = count("SELECT COUNT(*) FROM drivers WHERE status = 'Aposentado'");
    let active_orphans = count(
        "SELECT COUNT(*) FROM drivers
             WHERE status = 'Ativo' AND is_jogador = 0 AND categoria_atual IS NULL",
    );
    let active_never_raced = count(
        "SELECT COUNT(*) FROM drivers
             WHERE status = 'Ativo' AND is_jogador = 0 AND carreira_corridas = 0
               AND categoria_atual IS NOT NULL
               AND categoria_atual NOT IN ('mazda_rookie', 'toyota_rookie')",
    );

    // Quem são eles, quando são. Este bloco existe porque o teste é NÃO-DETERMINÍSTICO (o
    // caminho do draft passa por vários `thread_rng()` sem semente), então comparar contagens
    // entre execuções não distingue conserto de ruído. Ver o piloto e o contrato dele, sim.
    if active_never_raced > 0 {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT d.id, d.categoria_atual,
                        COALESCE(c.tipo, '(sem contrato)'),
                        COALESCE(c.temporada_inicio, '-') || ' cat_contrato=' ||
                        COALESCE(c.categoria, '-')
                   FROM drivers d
                   LEFT JOIN contracts c ON c.piloto_id = d.id AND c.status = 'Ativo'
                  WHERE d.status='Ativo' AND d.is_jogador=0 AND d.carreira_corridas=0
                    AND d.categoria_atual IS NOT NULL
                    AND d.categoria_atual NOT IN ('mazda_rookie','toyota_rookie')",
            )
            .expect("never-raced stmt");
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .expect("never-raced rows");
        for row in rows {
            let (id, cat, tipo, desde) = row.expect("never-raced row");
            eprintln!("[NUNCA CORREU] {id} cat={cat} contrato={tipo} desde={desde}");
        }
    }

    eprintln!(
        "[SISTEMA FECHADO] total={total} ativos={active} aposentados={retired} \
             orfaos_ativos={active_orphans} ativos_nunca_correu_nao_rookie={active_never_raced}"
    );
    let mut stmt = db
        .conn
        .prepare(
            "SELECT COALESCE(categoria_atual, '(sem categoria)'), COUNT(*)
                 FROM drivers WHERE status = 'Ativo'
                 GROUP BY categoria_atual ORDER BY 2 DESC",
        )
        .expect("dist stmt");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("dist rows");
    for row in rows {
        let (categoria, n) = row.expect("dist row");
        eprintln!("  ativos {categoria}: {n}");
    }

    let orphans_raced = count(
            "SELECT COUNT(*) FROM drivers
             WHERE status='Ativo' AND is_jogador=0 AND categoria_atual IS NULL AND carreira_corridas>0",
        );
    let orphans_with_especial_hist = count(
        "SELECT COUNT(DISTINCT d.id) FROM drivers d
             JOIN contracts c ON c.piloto_id = d.id AND c.tipo='Especial'
             WHERE d.status='Ativo' AND d.is_jogador=0 AND d.categoria_atual IS NULL",
    );
    let orphans_no_contract = count(
        "SELECT COUNT(*) FROM drivers d
             WHERE d.status='Ativo' AND d.is_jogador=0 AND d.categoria_atual IS NULL
               AND NOT EXISTS (SELECT 1 FROM contracts c WHERE c.piloto_id=d.id)",
    );
    eprintln!(
            "[ORFAOS] total={active_orphans} ja_correram={orphans_raced} \
             com_hist_especial={orphans_with_especial_hist} sem_nenhum_contrato={orphans_no_contract}"
        );

    // Invariantes do modelo fechado + poda do backstory. A populacao ativa fica
    // ~tamanho dos grids (~205), nao explode (o leak antigo levava a 850+ e
    // crescendo); os orfaos sao praticamente eliminados (de ~649 para ~0-2; os
    // residuais sao free agents que JA correram, entre contratos).
    assert!(
        active < 400,
        "populacao ativa deve ficar ~grid, nao explodir: {active}"
    );
    // INVARIANTE PRINCIPAL: nenhum orfao do tipo "nunca correu". A entrada
    // dinamica gera rookies sempre na categoria simulada da epoca; os poucos
    // orfaos remanescentes sao free agents que JA correram (entre contratos).
    // **A válvula de emergência alcança UM degrau, nunca a escada inteira.**
    //
    // A asserção anterior era `active_never_raced == 0`, e ela pedia algo que o design não pode
    // entregar: duas invariantes do jogo colidem quando o poço de pilotos aperta.
    //
    //   1. ninguém entra em pista sem nunca ter corrido (o que este teste queria);
    //   2. todo assento regular tem que estar preenchido — `validate_and_normalize_team_
    //      hierarchies` ABORTA a temporada se sobrar buraco no grid.
    //
    // A promoção de emergência existe para honrar a (2), e é o único caminho que ainda senta um
    // estreante. Fechá-la também (tentado, medido) mata o processo por grid inválido. Enquanto
    // grid furado não for permitido, a (1) é inalcançável em absoluto.
    //
    // O que É garantível, e o que passamos a exigir: o estreante só aparece no degrau
    // IMEDIATAMENTE acima da entrada, para onde a emergência o puxa da categoria em que nasceu.
    // Nunca no meio nem no topo da escada — lá a promoção é por mérito e mérito exige ter largado.
    // Um regressão que reabra a cadeia antiga (rookie chegando ao gt3 sem correr) derruba isto na
    // hora, que era o defeito real que a asserção de zero pegava por acidente.
    let never_raced_acima_do_amador = count(
        "SELECT COUNT(*) FROM drivers
             WHERE status = 'Ativo' AND is_jogador = 0 AND carreira_corridas = 0
               AND categoria_atual IS NOT NULL
               AND categoria_atual NOT IN ('mazda_rookie', 'toyota_rookie',
                                           'mazda_amador', 'toyota_amador')",
    );
    assert_eq!(
        never_raced_acima_do_amador, 0,
        "estreante sem nenhuma corrida subiu ALÉM do degrau da emergência — a promoção por \
         mérito voltou a aceitar quem nunca largou"
    );

    // E a válvula tem que continuar sendo exceção, não caminho. O grid da entrada tem ~24
    // assentos; um punhado por temporada de escassez é resíduo, dezenas seria a escada
    // inteira sendo preenchida por ela.
    assert!(
        active_never_raced <= 24,
        "a emergência deixou de ser exceção: {active_never_raced} estreantes sentados"
    );
    assert_eq!(
        active_orphans, orphans_raced,
        "todo orfao remanescente ja deve ter corrido (entre contratos), nao ser artefato"
    );
    // Reserva de free agents (que correram) pequena e limitada.
    assert!(
        active_orphans <= 40,
        "free agents entre contratos devem ser poucos: {active_orphans}"
    );
}

// Harness de MEDIÇÃO da deflação da grade: roda um mundo maduro por várias
// temporadas e imprime o skill médio por categoria (a escada) a cada ano, para
// observar se o topo (GT3/endurance) deflaciona e se a escada mantém a ordem.
// Serve de laboratório A/B para correções de alocação do mercado. #[ignore] (lento).
#[test]
#[ignore = "harness de medição de deflação da escada (lento); rodar sob demanda"]
fn grid_skill_ladder_over_time() {
    use super::{clear_historical_news, simulate_current_historical_season};
    use crate::constants::categories::get_category_config;
    use crate::evolution::pipeline::run_historical_end_of_season;
    use crate::market::pipeline::{fill_all_remaining_vacancies, read_slam_history};
    use crate::market::slam_ambition::{self, SlamDecision};
    use crate::models::enums::{DriverStatus, PrimaryPersonality};

    let base_dir = unique_test_dir("ladder_deflation");
    let input = sample_draft_input();
    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2024, 2025)
            .expect("historical generation should finish");
    let career_id = state
        .career_id
        .as_deref()
        .expect("draft career id")
        .to_string();
    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let mut db = open_draft_db(&base_dir, &career_id);

    // Categorias-chave da escada, do topo p/ baixo.
    let cats = ["endurance", "gt3", "gt4", "production_challenger", "bmw_m2"];
    let measure = |db: &Database, ano: i32| {
        let mut cells = Vec::new();
        for cat in &cats {
            let (avg, n): (f64, i64) = db
                    .conn
                    .query_row(
                        "SELECT COALESCE(AVG(skill),0), COUNT(*) FROM drivers \
                         WHERE is_jogador=0 AND status IN ('Ativo','Lesionado') AND categoria_atual=?1",
                        [cat],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .expect("cat skill");
            cells.push(format!("{cat}={avg:.1}(n{n})"));
        }
        // Craques skill>=74: quantos existem e quantos no GT3.
        let craques: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM drivers WHERE is_jogador=0 \
                     AND status IN ('Ativo','Lesionado') AND skill>=74",
                [],
                |r| r.get(0),
            )
            .expect("craques");
        let craques_gt3: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM drivers WHERE is_jogador=0 \
                     AND status IN ('Ativo','Lesionado') AND skill>=74 AND categoria_atual='gt3'",
                [],
                |r| r.get(0),
            )
            .expect("craques gt3");
        eprintln!(
            "{ano} | {} | craques74={craques} (gt3={craques_gt3})",
            cells.join(" ")
        );

        // ── DIAGNÓSTICO teto vs oportunidade ──────────────────────────────
        // Pergunta: o pool de craque encolhe porque FALTAM pilotos capazes de
        // virar craque (teto pessoal baixo = problema de INTAKE), ou porque
        // pilotos CAPAZES ficam presos embaixo sem chance de subir (problema de
        // OPORTUNIDADE/pódio)? Só medimos os vivos, sem contar o jogador.
        let one = |sql: &str| -> i64 {
            db.conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM drivers WHERE is_jogador=0 \
                             AND status IN ('Ativo','Lesionado') AND {sql}"
                    ),
                    [],
                    |r| r.get(0),
                )
                .expect(sql)
        };
        // capaz = teto pessoal já permite virar craque (independe do skill atual)
        let capaz = one("potencial>=74");
        // teto de ELITE: sustentar GT3/endurance nos 80s precisa de teto nos 80s,
        // não teto 74. Se pot>=82/>=88 forem escassos e caírem, o gargalo é a
        // GERAÇÃO da cauda de elite (intake), não conversão/alocação.
        let cap82 = one("potencial>=82");
        let cap88 = one("potencial>=88");
        // teto realmente alcançado no topo: média das 28 maiores skills (= vagas GT3)
        let top28_skill: f64 = db
            .conn
            .query_row(
                "SELECT COALESCE(AVG(skill),0) FROM (SELECT skill FROM drivers \
                     WHERE is_jogador=0 AND status IN ('Ativo','Lesionado') \
                     ORDER BY skill DESC LIMIT 28)",
                [],
                |r| r.get(0),
            )
            .expect("top28 skill");
        // preso = capaz mas ainda NÃO chegou (tem o teto, falta subir)
        let preso = one("potencial>=74 AND skill<74");
        // oportunidade-bloqueado = capaz, longe do teto E já não é jovem (não vai
        // mais crescer sozinho): talento desperdiçado por falta de carro/pódio
        let opp_travado = one("potencial>=74 AND skill<70 AND idade>=27");
        // intake-bloqueado = piloto BOM (skill>=60) mas com teto que PROÍBE craque
        let bom_capado = one("skill>=60 AND potencial<74");
        // cobertura: quantos ainda sem teto derivado (potencial=0) => ruído
        let sem_teto = one("potencial<=0");
        eprintln!(
            "      diag: capazes(pot>=74)={capaz} presos={preso} opp_travado={opp_travado} \
                 bom_capado(skill>=60,pot<74)={bom_capado} sem_teto={sem_teto}"
        );
        eprintln!("      elite: teto>=82={cap82} teto>=88={cap88} top28_skill={top28_skill:.1}");
        // ONDE estão os 28 melhores do mundo? Se a elite (skill ~90) não está no
        // GT3/endurance, é ralo de ALOCAÇÃO (órfão, classe excluída, tier inferior).
        let elite_loc: Vec<(String, i64)> = {
            let mut stmt = db
                .conn
                .prepare(
                    "SELECT COALESCE(categoria_atual,'ORFAO') AS cat, COUNT(*) \
                         FROM (SELECT categoria_atual, skill FROM drivers \
                               WHERE is_jogador=0 AND status IN ('Ativo','Lesionado') \
                               ORDER BY skill DESC LIMIT 28) \
                         GROUP BY cat ORDER BY COUNT(*) DESC",
                )
                .expect("prep elite loc");
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .expect("elite loc rows")
                .collect::<Result<Vec<_>, _>>()
                .expect("elite loc collect");
            rows
        };
        let loc_str = elite_loc
            .iter()
            .map(|(cat, n)| format!("{cat}={n}"))
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!("      top28-onde: {loc_str}");

        // ── FSM de ambição: a elite presa na várzea QUER subir ou está feliz? ──
        // Roda a `slam_ambition::decide` REAL em cada craque preso abaixo do gt3
        // (skill>=82, sentado, fora de gt3/endurance) pra separar quem a escada
        // deveria promover (frustrado) de quem está construindo/perseguindo título
        // (contente — não deve ser forçado). Isso prevê se ligar a FSM enche o GT3.
        let all_drivers =
            crate::db::queries::drivers::get_all_drivers(&db.conn).expect("get all drivers");
        let (mut nao_amb, mut fica_dinastia, mut fica_lado, mut quer_subir, mut sobe_normal) =
            (0i32, 0i32, 0i32, 0i32, 0i32);
        let mut com_historico = 0i32;
        for d in &all_drivers {
            if d.is_jogador
                || !matches!(d.status, DriverStatus::Ativo | DriverStatus::Lesionado)
                || d.atributos.skill < 82.0
            {
                continue;
            }
            let cur = match &d.categoria_atual {
                Some(c) if c != "gt3" && c != "endurance" => c.clone(),
                _ => continue, // órfão ou já no topo → não é "preso na várzea"
            };
            let ambicioso = d.personalidade_primaria == Some(PrimaryPersonality::Ambicioso);
            if !ambicioso {
                nao_amb += 1;
                continue;
            }
            let (history, current_results) = read_slam_history(&db.conn, d).unwrap_or_default();
            if !history.is_empty() || !current_results.is_empty() {
                com_historico += 1;
            }
            let cur_tier = get_category_config(&cur).map(|c| c.tier).unwrap_or(0);
            match slam_ambition::decide(&history, &cur, d.atributos.skill, true, &current_results) {
                Some(SlamDecision::Stay { .. }) => fica_dinastia += 1,
                Some(SlamDecision::Chase { category, .. }) => {
                    let alvo_tier = get_category_config(&category).map(|c| c.tier).unwrap_or(0);
                    if alvo_tier > cur_tier {
                        quer_subir += 1;
                    } else {
                        fica_lado += 1; // persegue base na categoria atual/lateral
                    }
                }
                None => sobe_normal += 1, // ambicioso, esgotou/desistiu → sobe normal
            }
        }
        let sobem = quer_subir + sobe_normal;
        let ficam = nao_amb + fica_dinastia + fica_lado;
        eprintln!(
                "      elite-presa(skill>=82,fora gt3/endur): SOBEM={sobem} [quer_subir={quer_subir} \
                 sobe_normal={sobe_normal}] | FICAM={ficam} [nao_amb={nao_amb} dinastia={fica_dinastia} \
                 base_atual={fica_lado}] (com_historico={com_historico})"
            );

        // ── Composição dos PRESOS (capaz, pot>=74, skill<74) ──────────────
        // Quebra por IDADE (o age_factor do crescimento cai a 0.7 aos 29-32 e
        // 0.3 aos 33+: quem passa disso está age-locked, nenhum peso salva) e por
        // ASSENTO (órfão sem categoria não corre → não cresce nunca).
        const PRESO: &str = "potencial>=74 AND skill<74";
        let p_jovem = one(&format!("{PRESO} AND idade<=24"));
        let p_meio = one(&format!("{PRESO} AND idade BETWEEN 25 AND 28"));
        let p_2932 = one(&format!("{PRESO} AND idade BETWEEN 29 AND 32"));
        let p_velho = one(&format!("{PRESO} AND idade>=33"));
        let p_orfao = one(&format!("{PRESO} AND categoria_atual IS NULL"));
        let p_sentado = one(&format!("{PRESO} AND categoria_atual IS NOT NULL"));
        let (skill_med, gap_med): (f64, f64) = db
            .conn
            .query_row(
                &format!(
                    "SELECT COALESCE(AVG(skill),0), COALESCE(AVG(potencial-skill),0) \
                         FROM drivers WHERE is_jogador=0 \
                         AND status IN ('Ativo','Lesionado') AND {PRESO}"
                ),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("preso stats");
        eprintln!(
                "      preso-comp: idade[<=24={p_jovem} 25-28={p_meio} 29-32={p_2932} 33+={p_velho}] \
                 assento[orfao={p_orfao} sentado={p_sentado}] skill_med={skill_med:.1} folga_ate_teto={gap_med:.1}"
            );
    };

    eprintln!("=== BASELINE (fim da geração 2000-2024) ===");
    measure(&db, 2025);
    eprintln!("=== +15 temporadas ===");
    for _ in 0..15 {
        simulate_current_historical_season(&mut db).expect("simulate season");
        let season = season_queries::get_active_season(&db.conn)
            .expect("active season query")
            .expect("active season exists");
        run_historical_end_of_season(&mut db.conn, &season, &career_dir).expect("eos");
        let next = season_queries::get_active_season(&db.conn)
            .expect("next season query")
            .expect("next season exists");
        fill_all_remaining_vacancies(&db.conn, next.numero, &mut rand::thread_rng())
            .expect("fill vacancies");
        clear_historical_news(&db.conn).expect("clear news");
        measure(&db, next.ano);
    }
}

// Harness de MEDIÇÃO (não regressão): roda um mundo maduro por várias temporadas
// e observa se os pilotos que se lesionam e perdem o assento são REABSORVIDOS pelo
// mercado, ou se acumulam como órfãos ao longo do tempo. Marcado #[ignore] porque
// simula ~40 temporadas (lento); rodar com `--ignored --nocapture`.
#[test]
#[ignore = "harness de medição de absorção de lesionados (lento); rodar sob demanda"]
fn injured_orphans_are_reabsorbed_by_market_over_time() {
    use super::{clear_historical_news, simulate_current_historical_season};
    use crate::evolution::pipeline::run_historical_end_of_season;
    use crate::market::pipeline::fill_all_remaining_vacancies;

    let base_dir = unique_test_dir("injury_absorption");
    let input = sample_draft_input();
    // Mundo maduro (escada cheia, aposentadorias em regime) já em 2025.
    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2024, 2025)
            .expect("historical generation should finish");
    let career_id = state
        .career_id
        .as_deref()
        .expect("draft career id")
        .to_string();
    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let mut db = open_draft_db(&base_dir, &career_id);

    let count = |db: &Database, sql: &str| -> i64 {
        db.conn.query_row(sql, [], |row| row.get(0)).expect(sql)
    };
    // "Vivos" = na ativa. Aposentados ficam na tabela para sempre (histórico) e
    // cresceriam sempre — não são o que interessa para "sistema fechado inflando".
    const Q_TOTAL: &str =
        "SELECT COUNT(*) FROM drivers WHERE is_jogador=0 AND status IN ('Ativo','Lesionado')";
    const Q_SEATED: &str =
        "SELECT COUNT(*) FROM drivers WHERE is_jogador=0 AND categoria_atual IS NOT NULL";
    const Q_ORPHAN_ACTIVE: &str = "SELECT COUNT(*) FROM drivers \
             WHERE is_jogador=0 AND status='Ativo' AND categoria_atual IS NULL";
    const Q_ORPHAN_INJURED: &str = "SELECT COUNT(*) FROM drivers \
             WHERE is_jogador=0 AND status='Lesionado' AND categoria_atual IS NULL";
    const Q_INJURED_SEATED: &str = "SELECT COUNT(*) FROM drivers \
             WHERE is_jogador=0 AND status='Lesionado' AND categoria_atual IS NOT NULL";

    // 15 temporadas adicionais em modo histórico não-interativo (janela semanal
    // resolvida sozinha por advance_week dentro do EOS).
    let mut trajectory: Vec<(i32, i64, i64, i64, i64, i64)> = Vec::new();
    for _ in 0..15 {
        simulate_current_historical_season(&mut db).expect("simulate season");
        let season = season_queries::get_active_season(&db.conn)
            .expect("active season query")
            .expect("active season exists");
        run_historical_end_of_season(&mut db.conn, &season, &career_dir).expect("eos");
        let next = season_queries::get_active_season(&db.conn)
            .expect("next season query")
            .expect("next season exists");
        fill_all_remaining_vacancies(&db.conn, next.numero, &mut rand::thread_rng())
            .expect("fill vacancies");
        clear_historical_news(&db.conn).expect("clear news");

        trajectory.push((
            next.ano,
            count(&db, Q_TOTAL),
            count(&db, Q_SEATED),
            count(&db, Q_ORPHAN_ACTIVE),
            count(&db, Q_ORPHAN_INJURED),
            count(&db, Q_INJURED_SEATED),
        ));
    }

    eprintln!("ano  | total | assento | orf_ativo | orf_lesionado | lesionado_c/assento");
    for (ano, total, seated, orf_a, orf_i, inj_seat) in &trajectory {
        eprintln!(
                "{ano} |  {total:4} |   {seated:4}  |    {orf_a:3}    |      {orf_i:3}      |        {inj_seat:3}"
            );
    }

    // Órfãos-Lesionados NÃO podem acumular: a cura de fim de temporada os zera.
    // Se o deadlock voltasse, este número cresceria monotonicamente.
    let max_orphan_injured = trajectory.iter().map(|t| t.4).max().unwrap_or(0);
    assert!(
            max_orphan_injured <= 8,
            "órfãos-lesionados não devem acumular (deadlock de recuperação): pico={max_orphan_injured}\n{trajectory:?}"
        );

    // ABSORÇÃO: os órfãos-Ativo (lesionados curados sem assento) não podem crescer
    // sem parar. Comparo a média da 2ª metade com a 1ª — não deve explodir.
    let n = trajectory.len();
    let first_half: f64 =
        trajectory[..n / 2].iter().map(|t| t.3 as f64).sum::<f64>() / (n / 2) as f64;
    let second_half: f64 =
        trajectory[n / 2..].iter().map(|t| t.3 as f64).sum::<f64>() / (n - n / 2) as f64;
    eprintln!("[ABSORÇÃO] orf_ativo média 1ª metade={first_half:.1} | 2ª metade={second_half:.1}");
    assert!(
            second_half <= first_half + 15.0,
            "órfãos-Ativo crescendo sem absorção: 1ª metade {first_half:.1} → 2ª metade {second_half:.1}\n{trajectory:?}"
        );

    // População VIVA estável (sistema fechado não pode inflar por pilotos extras
    // que se lesionam, perdem o assento e nunca são reabsorvidos).
    let alive_start = trajectory.first().map(|t| t.1).unwrap_or(0);
    let alive_end = trajectory.last().map(|t| t.1).unwrap_or(0);
    eprintln!("[POPULAÇÃO VIVA] início={alive_start} → fim={alive_end}");
    assert!(
            alive_end < alive_start + 60,
            "população viva inflando (piloto extra por lesão não reabsorvido): {alive_start} → {alive_end}"
        );
}

// Harness de MEDIÇÃO da rivalidade entre COMPANHEIROS de equipe.
//
// A pergunta que ele responde: a tensão interna das equipes sobe, e as
// rivalidades de companheiros passam a existir?
//
// O baseline não é hipótese, é observação. Num save real de 27 temporadas com a
// regra antiga — o -1 de decaimento incidindo até nas corridas disputadas — o
// mundo tinha 101 de 102 equipes em "estável" com tensão média 0,1, a maior
// sequência do N2 travada em 3, e ZERO rivalidades do tipo `Companheiros` em
// 140 registradas. O gatilho existia e nunca disparava.
//
// Marcado #[ignore] porque gera um mundo maduro e simula temporadas inteiras
// (lento); rodar com `--ignored --nocapture`.
#[test]
#[ignore = "harness de medição de rivalidade entre companheiros (lento); rodar sob demanda"]
fn teammate_tension_climbs_and_finally_creates_rivalries() {
    use super::{clear_historical_news, simulate_current_historical_season};
    use crate::evolution::pipeline::run_historical_end_of_season;
    use crate::market::pipeline::fill_all_remaining_vacancies;

    let base_dir = unique_test_dir("teammate_tension");
    let input = sample_draft_input();
    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2024, 2025)
            .expect("historical generation should finish");
    let career_id = state
        .career_id
        .as_deref()
        .expect("draft career id")
        .to_string();
    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    let mut db = open_draft_db(&base_dir, &career_id);

    let count = |db: &Database, sql: &str| -> i64 {
        db.conn.query_row(sql, [], |row| row.get(0)).expect(sql)
    };
    let media = |db: &Database, sql: &str| -> f64 {
        db.conn.query_row(sql, [], |row| row.get(0)).expect(sql)
    };

    const Q_TENSAO_MEDIA: &str = "SELECT COALESCE(AVG(hierarquia_tensao), 0) FROM teams";
    const Q_TENSAO_MAX: &str = "SELECT COALESCE(MAX(hierarquia_tensao), 0) FROM teams";
    // Equipes que saíram do chão: "estável" é tudo abaixo de 20.
    const Q_ACIMA_DE_ESTAVEL: &str = "SELECT COUNT(*) FROM teams WHERE hierarquia_tensao >= 20";
    // A porta de entrada nova.
    const Q_EM_TENSAO: &str = "SELECT COUNT(*) FROM teams WHERE hierarquia_tensao >= 40";
    const Q_COMPANHEIROS: &str = "SELECT COUNT(*) FROM rivalries WHERE tipo = 'Companheiros'";
    const Q_PISTA: &str = "SELECT COUNT(*) FROM rivalries WHERE tipo = 'Pista'";
    // Quantas saíram do piso: `atrito_leve` é tudo abaixo de percebida 20, e num
    // save real de 27 temporadas eram 139 de 140. É o número que diz se o mundo
    // tem alguma história ou só ruído.
    const Q_ACIMA_DO_PISO: &str = "SELECT COUNT(*) FROM rivalries
         WHERE 0.4 * historical_intensity + 0.6 * recent_activity >= 20";
    const Q_RIVALIDADES: &str = "SELECT COUNT(*) FROM rivalries";
    // O NÚMERO QUE DECIDE TUDO. A tensão sobe +3 quando o N2 vence e cai -2
    // quando o N1 vence, então a deriva por corrida é `5p - 2` com `p` = fração
    // de duelos que o N2 leva. Abaixo de p = 0,40 ela é negativa e a tensão,
    // que tem piso em zero, mora no chão independentemente de qualquer outro
    // ajuste. Medir `p` é medir se este eixo pode existir.
    const Q_TAXA_N2: &str = "SELECT CASE WHEN SUM(hierarquia_duelos_total) > 0
             THEN CAST(SUM(hierarquia_duelos_n2_vencidos) AS REAL) / SUM(hierarquia_duelos_total)
             ELSE 0 END FROM teams WHERE ativa = 1";

    // DUAS MEDIÇÕES POR TEMPORADA, e não uma.
    //
    // A primeira versão deste harness media só depois do fim de temporada e viu
    // a tensão congelada em 0,07 nas doze — o que parecia "a mudança não fez
    // efeito" era o instante errado: a transição de temporada dá `FullReset` na
    // tensão de toda equipe que trocou de dupla, e quase todas trocam. Medir ali
    // é medir o zero recém-escrito, não o que a temporada produziu.
    let mut trajetoria: Vec<(i32, f64, f64, i64, i64, f64, i64, i64, i64, i64)> = Vec::new();
    for _ in 0..12 {
        simulate_current_historical_season(&mut db).expect("simulate season");

        // PICO: fim da temporada corrida, antes de qualquer reset.
        let pico_media = media(&db, Q_TENSAO_MEDIA);
        let pico_max = media(&db, Q_TENSAO_MAX);
        let pico_acima = count(&db, Q_ACIMA_DE_ESTAVEL);
        let pico_tensao = count(&db, Q_EM_TENSAO);
        let taxa_n2 = media(&db, Q_TAXA_N2);

        let season = season_queries::get_active_season(&db.conn)
            .expect("active season query")
            .expect("active season exists");
        run_historical_end_of_season(&mut db.conn, &season, &career_dir).expect("eos");
        let next = season_queries::get_active_season(&db.conn)
            .expect("next season query")
            .expect("next season exists");
        fill_all_remaining_vacancies(&db.conn, next.numero, &mut rand::thread_rng())
            .expect("fill vacancies");
        clear_historical_news(&db.conn).expect("clear news");

        trajetoria.push((
            next.ano,
            pico_media,
            pico_max,
            pico_acima,
            pico_tensao,
            taxa_n2,
            count(&db, Q_COMPANHEIROS),
            count(&db, Q_PISTA),
            count(&db, Q_ACIMA_DO_PISO),
            count(&db, Q_RIVALIDADES),
        ));
    }

    eprintln!(
        "ano  | pico_medio | pico_max | eq>=20 | eq>=40 | taxa_n2 | comp | pista | >piso | total"
    );
    for (ano, m, mx, acima, tensao, taxa, comp, pista, forte, total) in &trajetoria {
        eprintln!(
            "{ano} |   {m:6.1}   |  {mx:6.1}  |  {acima:4}  |  {tensao:4}  |  {taxa:5.3}  | {comp:4} | {pista:5} | {forte:5} | {total:5}"
        );
    }

    let taxa_media = trajetoria.iter().map(|t| t.5).sum::<f64>() / trajetoria.len().max(1) as f64;
    eprintln!(
        "[TAXA DO N2] média={taxa_media:.3} — equilíbrio da tensão em 0.400, \
         abaixo disso o eixo mora no piso"
    );

    // O harness é de MEDIÇÃO, e a única coisa que ele exige é ter medido: mundo
    // gerado, temporadas simuladas, duelos internos contados. Os números acima
    // são o retrato, não a meta — travar uma meta aqui transformaria uma régua
    // em alarme, e a decisão do que fazer com a leitura é de balanceamento.
    let duelos: i64 = count(
        &db,
        "SELECT SUM(hierarquia_duelos_total) FROM teams WHERE ativa = 1",
    );
    assert!(
        duelos > 0,
        "nenhum duelo interno registrado — o pipeline de hierarquia não rodou\n{trajetoria:?}"
    );
}

#[test]
fn historical_playable_year_regular_categories_have_full_lineups() {
    let base_dir = unique_test_dir("historical_regular_lineups");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2024, 2025)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);

    for category in get_all_categories()
        .iter()
        .filter(|category| !runs_in_special_phase(category.id))
    {
        let team_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                     FROM teams
                     WHERE categoria = ?1
                       AND ativa = 1",
                rusqlite::params![category.id],
                |row| row.get(0),
            )
            .expect("active team count");
        let driver_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT piloto_id)
                     FROM contracts
                     WHERE categoria = ?1
                       AND tipo = 'Regular'
                       AND status = 'Ativo'",
                rusqlite::params![category.id],
                |row| row.get(0),
            )
            .expect("active regular driver count");
        let empty_slots: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*)
                     FROM teams
                     WHERE categoria = ?1
                       AND ativa = 1
                       AND (piloto_1_id IS NULL OR piloto_2_id IS NULL)",
                rusqlite::params![category.id],
                |row| row.get(0),
            )
            .expect("empty slot count");
        let expected = i64::from(category.num_equipes) * i64::from(category.pilotos_por_equipe);

        assert_eq!(
            team_count,
            i64::from(category.num_equipes),
            "{} should keep its configured active team count",
            category.id
        );
        assert_eq!(
            empty_slots, 0,
            "{} should have no empty team slots",
            category.id
        );
        assert_eq!(
            driver_count, expected,
            "{} should have all configured regular drivers",
            category.id
        );
    }

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_gt3_heritage_teams_remain_winners_across_archive() {
    let base_dir = unique_test_dir("historical_gt3_heritage_results");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2024, 2025)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);

    let heritage_wins: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM race_results rr
                 JOIN calendar c ON c.id = rr.race_id
                 JOIN teams t ON t.id = rr.equipe_id
                 WHERE c.categoria = 'gt3'
                   AND rr.posicao_final = 1
                   AND t.nome IN ('Mercedes-AMG', 'Ferrari', 'Lamborghini', 'McLaren')",
            [],
            |row| row.get(0),
        )
        .expect("heritage wins");
    let challenger_wins: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM race_results rr
                 JOIN calendar c ON c.id = rr.race_id
                 JOIN teams t ON t.id = rr.equipe_id
                 WHERE c.categoria = 'gt3'
                   AND rr.posicao_final = 1
                   AND t.nome IN ('Audi', 'Acura')",
            [],
            |row| row.get(0),
        )
        .expect("challenger wins");

    assert!(
            heritage_wins > challenger_wins,
            "GT3 heritage teams should not be out-won by Audi/Acura in the generated archive: heritage={heritage_wins}, challengers={challenger_wins}"
        );

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_simulation_skips_categories_before_their_inaugural_year() {
    let base_dir = unique_test_dir("historical_category_timeline");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2001, 2002)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);

    let rookie_results: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*)
                 FROM race_results rr
                 JOIN races r ON r.id = rr.race_id
                 JOIN calendar c ON c.id = r.calendar_id
                 WHERE c.categoria = 'mazda_rookie'",
            [],
            |row| row.get(0),
        )
        .expect("rookie race result count");
    let rookie_standings: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM standings WHERE categoria = 'mazda_rookie'",
            [],
            |row| row.get(0),
        )
        .expect("rookie standings count");

    assert_eq!(rookie_results, 0);
    assert_eq!(rookie_standings, 0);

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_simulation_skips_teams_before_their_foundation_year() {
    let base_dir = unique_test_dir("historical_team_timeline");
    let input = sample_draft_input();
    let state = create_historical_career_draft_base_for_test(&base_dir, input)
        .expect("draft base should be created");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let career_dir = AppConfig::load_or_default(&base_dir)
        .saves_dir()
        .join(career_id);
    let db_path = career_dir.join("career.db");
    let mut db = Database::open_existing(&db_path).expect("db");
    let team_id: String = db
        .conn
        .query_row(
            "SELECT id FROM teams WHERE categoria = 'gt3' ORDER BY car_performance DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("gt3 team id");
    db.conn
        .execute(
            "UPDATE teams SET ano_fundacao = 2002 WHERE id = ?1",
            rusqlite::params![&team_id],
        )
        .expect("update team foundation");

    simulate_historical_range(&mut db, &career_dir, 2000, 2000, 2001)
        .expect("historical range should finish");

    let team_results: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM race_results WHERE equipe_id = ?1",
            rusqlite::params![&team_id],
            |row| row.get(0),
        )
        .expect("team race result count");
    let team_standings: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM standings WHERE equipe_id = ?1",
            rusqlite::params![&team_id],
            |row| row.get(0),
        )
        .expect("team standings count");

    assert_eq!(team_results, 0);
    assert_eq!(team_standings, 0);

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_simulation_starts_playable_year_with_clean_team_finances() {
    let base_dir = unique_test_dir("historical_clean_finance");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2000, 2001)
            .expect("historical generation should finish");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let db = open_draft_db(&base_dir, career_id);
    let teams = team_queries::get_all_teams(&db.conn).expect("teams");

    assert!(teams.iter().all(|team| team.debt_balance == 0.0));
    assert!(teams.iter().all(|team| team.last_round_income == 0.0));
    assert!(teams.iter().all(|team| team.last_round_expenses == 0.0));
    assert!(teams.iter().all(|team| team.last_round_net == 0.0));
    // Ciente da CLASSE: a faixa de caixa de um LMP2 do Endurance não é a de um GT4 do
    // Endurance. Com a forma de um argumento só, os seis times de LMP2 eram medidos contra a
    // faixa do GT3 (a classe representativa) e caíam fora por construção.
    assert!(teams.iter().all(|team| {
        let scale = category_finance_scale_for(&team.categoria, team.classe.as_deref());
        team.cash_balance >= scale.cash_min && team.cash_balance <= scale.cash_max
    }));

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn historical_races_preserve_team_finance_snapshot() {
    let base_dir = unique_test_dir("historical_race_finance_snapshot");
    let state = create_historical_career_draft_base_for_test(&base_dir, sample_draft_input())
        .expect("draft base should be created");
    let career_id = state.career_id.as_deref().expect("draft career id");
    let mut db = open_draft_db(&base_dir, career_id);
    let before: HashMap<String, (f64, f64, f64, f64, f64)> = team_queries::get_all_teams(&db.conn)
        .expect("teams before")
        .into_iter()
        .map(|team| {
            (
                team.id,
                (
                    team.cash_balance,
                    team.debt_balance,
                    team.last_round_income,
                    team.last_round_expenses,
                    team.last_round_net,
                ),
            )
        })
        .collect();

    super::simulate_current_historical_season(&mut db)
        .expect("historical season simulation should finish");

    let after = team_queries::get_all_teams(&db.conn).expect("teams after");
    assert!(after.iter().all(|team| {
        before
            .get(&team.id)
            .is_some_and(|(cash, debt, income, expenses, net)| {
                team.cash_balance == *cash
                    && team.debt_balance == *debt
                    && team.last_round_income == *income
                    && team.last_round_expenses == *expenses
                    && team.last_round_net == *net
            })
    }));

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn get_draft_returns_generated_starting_categories_and_teams() {
    let base_dir = unique_test_dir("get_draft");
    let input = sample_draft_input();
    let created =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2000, 2001)
            .expect("draft should be created");

    let state = get_career_draft_in_base_dir(&base_dir).expect("draft state");

    assert!(state.exists);
    assert_eq!(state.career_id, created.career_id);
    assert_eq!(state.lifecycle_status, SaveLifecycleStatus::Draft);
    assert!(state.categories.contains(&"mazda_rookie".to_string()));
    assert!(state.categories.contains(&"toyota_rookie".to_string()));
    // O draft expõe as categorias de início (mazda/toyota_rookie) para o jogador
    // escolher. Basta haver algum time com lineup completo entre elas.
    let full = state
        .teams
        .iter()
        .filter(|team| team.n1_nome.is_some() && team.n2_nome.is_some())
        .count();
    assert!(full > 0, "o draft deve gerar times com lineup completo");

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn get_draft_returns_pending_player_identity() {
    let base_dir = unique_test_dir("get_draft_identity");
    let input = sample_draft_input();
    create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2000, 2001)
        .expect("draft should be created");

    let state = get_career_draft_in_base_dir(&base_dir).expect("draft state");

    // Retomar o draft precisa devolver a identidade digitada, senão a tela reabre
    // com o nome em branco e o jogador redigita, descartando o mundo simulado.
    assert_eq!(state.player_name.as_deref(), Some("Joao Silva"));
    assert_eq!(state.player_nationality.as_deref(), Some("br"));
    assert_eq!(state.player_age, Some(22));
    assert_eq!(state.difficulty.as_deref(), Some("medio"));

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn update_draft_identity_rewrites_player_without_touching_the_simulated_world() {
    let base_dir = unique_test_dir("update_draft_identity");
    let input = sample_draft_input();
    let created =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2000, 2001)
            .expect("draft should be created");
    let career_id = created.career_id.clone().expect("draft career id");
    let teams_before = created.teams.len();

    let updated = update_career_draft_identity_in_base_dir(
        &base_dir,
        UpdateDraftIdentityInput {
            career_id: career_id.clone(),
            player_name: "  Carlos Magno  ".to_string(),
            player_nationality: "US".to_string(),
            player_age: Some(24),
        },
    )
    .expect("identity update should succeed");

    assert_eq!(updated.player_name.as_deref(), Some("Carlos Magno"));
    assert_eq!(updated.player_nationality.as_deref(), Some("us"));
    assert_eq!(updated.player_age, Some(24));
    // A dificuldade é o único campo que molda o mundo, e ela não muda aqui.
    assert_eq!(updated.difficulty.as_deref(), Some("medio"));

    // O mundo simulado sobrevive à troca: mesmo save, mesmas equipes.
    assert_eq!(updated.career_id.as_deref(), Some(career_id.as_str()));
    assert_eq!(updated.teams.len(), teams_before);

    let reloaded = get_career_draft_in_base_dir(&base_dir).expect("draft state");
    assert_eq!(reloaded.player_name.as_deref(), Some("Carlos Magno"));
    assert_eq!(reloaded.career_id.as_deref(), Some(career_id.as_str()));

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn creating_a_draft_purges_the_previous_one() {
    let base_dir = unique_test_dir("purge_previous_draft");
    let config = AppConfig::load_or_default(&base_dir);

    let first = create_historical_career_draft_base_for_test(&base_dir, sample_draft_input())
        .expect("first draft should be created");
    let first_id = first.career_id.clone().expect("first career id");
    let first_dir = config.saves_dir().join(&first_id);
    assert!(first_dir.exists());

    // Sentinela: o ID pode ser reaproveitado depois da limpeza, então o que prova
    // a remoção é o conteúdo do save anterior ter sumido, não o caminho.
    let sentinel = first_dir.join("sentinela_do_draft_anterior.txt");
    std::fs::write(&sentinel, "draft anterior").expect("sentinel write");

    // Um draft interrompido fica em `failed` e continua no disco com o career.db
    // inteiro. Gerar o próximo precisa levar o anterior junto.
    let second = create_historical_career_draft_base_for_test(&base_dir, sample_draft_input())
        .expect("second draft should be created");
    let second_id = second.career_id.clone().expect("second career id");

    assert!(!sentinel.exists(), "o draft anterior deve sair do disco");
    assert!(config.saves_dir().join(&second_id).exists());

    let drafts_left = std::fs::read_dir(config.saves_dir())
        .expect("saves dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("career_"))
        .count();
    assert_eq!(drafts_left, 1, "so pode sobrar um rascunho por vez");

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn creating_a_draft_never_touches_an_active_save() {
    let base_dir = unique_test_dir("purge_spares_active");
    let config = AppConfig::load_or_default(&base_dir);
    let saves_dir = config.saves_dir();

    // Carreira do jogador, já finalizada. A limpeza de rascunhos filtra por
    // lifecycle_status; apagar um save ativo aqui seria perda de progresso real.
    let active_dir = saves_dir.join("career_007");
    std::fs::create_dir_all(&active_dir).expect("active save dir");
    std::fs::write(
        active_dir.join("meta.json"),
        serde_json::json!({
            "career_number": 7,
            "player_name": "Carlos Magno",
            "current_season": 3,
            "current_year": 2028,
            "created_at": "2026-01-01T00:00:00",
            "last_played": "2026-08-10T00:00:00",
            "category": "gt3",
            "difficulty": "medio",
            "lifecycle_status": "active",
        })
        .to_string(),
    )
    .expect("active meta");
    let active_marker = active_dir.join("career.db");
    std::fs::write(&active_marker, "save do jogador").expect("active db");

    create_historical_career_draft_base_for_test(&base_dir, sample_draft_input())
        .expect("draft should be created");

    assert!(active_marker.exists(), "save ativo nao pode ser apagado");

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn create_draft_response_includes_generated_categories_and_teams() {
    let base_dir = unique_test_dir("create_draft_response");
    let input = sample_draft_input();

    let state =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2000, 2001)
            .expect("draft should be created");

    assert!(state.categories.contains(&"mazda_rookie".to_string()));
    assert!(state.categories.contains(&"toyota_rookie".to_string()));
    assert!(state
        .teams
        .iter()
        .any(|team| team.categoria == "mazda_rookie"));
    assert!(state
        .teams
        .iter()
        .any(|team| team.categoria == "toyota_rookie"));

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn discard_draft_removes_rascunho_save() {
    let base_dir = unique_test_dir("discard_draft");
    let input = sample_draft_input();
    let created =
        create_historical_career_draft_for_range_for_test(&base_dir, input, 2000, 2000, 2001)
            .expect("draft should be created");
    let career_id = created.career_id.expect("draft career id");
    let config = AppConfig::load_or_default(&base_dir);
    let career_dir = config.saves_dir().join(&career_id);
    assert!(career_dir.exists());

    discard_career_draft_in_base_dir(&base_dir).expect("discard should succeed");

    assert!(!career_dir.exists());
    let state = get_career_draft_in_base_dir(&base_dir).expect("draft state");
    assert!(!state.exists);

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn finalize_draft_runs_world_integrity_before_player_insertion() {
    let base_dir = unique_test_dir("finalize_draft_audit");
    let state = create_historical_career_draft_for_range_for_test(
        &base_dir,
        sample_draft_input(),
        2000,
        2000,
        2001,
    )
    .expect("draft should be created");
    let career_id = state.career_id.clone().expect("draft career id");
    let db = open_draft_db(&base_dir, &career_id);
    let selected_team = team_queries::get_teams_by_category(&db.conn, "mazda_rookie")
        .expect("teams by category")
        .into_iter()
        .next()
        .expect("at least one rookie team");
    db.conn
        .execute("DELETE FROM driver_season_archive", [])
        .expect("corrupt archive");
    drop(db);

    let error = super::finalize_career_draft_in_base_dir(
        &base_dir,
        FinalizeHistoricalDraftInput {
            career_id: career_id.clone(),
            category: selected_team.categoria,
            team_id: selected_team.id,
        },
    )
    .expect_err("audit should block finalization");

    assert!(error.contains("Mundo historico invalido"));
    let career_dir = AppConfig::load_or_default(&base_dir)
        .saves_dir()
        .join(&career_id);
    assert!(!career_dir.join("career.db").exists());
    let meta = super::read_save_meta(&career_dir.join("meta.json")).expect("failed meta");
    assert_eq!(meta.lifecycle_status, SaveLifecycleStatus::Failed);
    assert!(meta
        .draft_error
        .as_deref()
        .is_some_and(|value| { value.contains("veteran_without_driver_archive") }));

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn failed_historical_draft_cleans_generated_data_but_preserves_error_meta() {
    let base_dir = unique_test_dir("failed_draft_cleanup");
    let state = create_historical_career_draft_base_for_test(&base_dir, sample_draft_input())
        .expect("draft base should be created");
    let career_id = state.career_id.expect("draft career id");
    let career_dir = AppConfig::load_or_default(&base_dir)
        .saves_dir()
        .join(&career_id);
    std::fs::create_dir_all(career_dir.join("backups")).expect("backups dir");
    std::fs::write(career_dir.join("preseason_plan.json"), "{}").expect("sidecar");

    super::mark_historical_draft_failed(&career_dir, "falha controlada").expect("mark failed");
    super::mark_historical_draft_failed(&career_dir, "falha controlada")
        .expect("mark failed is idempotent");

    assert!(career_dir.join("meta.json").exists());
    assert!(!career_dir.join("career.db").exists());
    assert!(!career_dir.join("preseason_plan.json").exists());
    assert!(!career_dir.join("backups").exists());
    let meta = super::read_save_meta(&career_dir.join("meta.json")).expect("failed meta");
    assert_eq!(meta.lifecycle_status, SaveLifecycleStatus::Failed);
    assert_eq!(meta.draft_error.as_deref(), Some("falha controlada"));

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn finalized_historical_save_feeds_dossiers_and_global_ranking() {
    let base_dir = unique_test_dir("finalized_historical_consumers");
    let state = create_historical_career_draft_for_range_for_test(
        &base_dir,
        sample_draft_input(),
        2000,
        2000,
        2001,
    )
    .expect("draft should be created");
    let career_id = state.career_id.clone().expect("draft career id");
    let db = open_draft_db(&base_dir, &career_id);
    let selected_team_id: String = db
        .conn
        .query_row(
            "SELECT r.equipe_id
                 FROM race_results r
                 JOIN calendar c ON c.id = r.race_id
                 GROUP BY r.equipe_id
                 ORDER BY COUNT(*) DESC, r.equipe_id ASC
                 LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("team with rookie history");
    let selected_team = team_queries::get_team_by_id(&db.conn, &selected_team_id)
        .expect("team query")
        .expect("selected team should exist");
    let ai_driver_id: String = db
        .conn
        .query_row(
            "SELECT piloto_id
                 FROM race_results
                 WHERE equipe_id = ?1
                 ORDER BY race_id ASC, posicao_final ASC
                 LIMIT 1",
            rusqlite::params![&selected_team.id],
            |row| row.get(0),
        )
        .expect("AI veteran with race history");
    drop(db);

    super::finalize_career_draft_in_base_dir(
        &base_dir,
        FinalizeHistoricalDraftInput {
            career_id: career_id.clone(),
            category: selected_team.categoria.clone(),
            team_id: selected_team.id.clone(),
        },
    )
    .expect("finalize should succeed");

    let db = open_draft_db(&base_dir, &career_id);
    let player = driver_queries::get_player_driver(&db.conn).expect("player should exist");
    assert_eq!(player.stats_carreira.corridas, 0);
    assert_eq!(player.stats_temporada.corridas, 0);
    drop(db);

    let driver_detail = get_driver_detail_in_base_dir(&base_dir, &career_id, &ai_driver_id)
        .expect("driver detail should use historical data");
    assert!(driver_detail.trajetoria.historico.presenca.corridas > 0);

    let team_dossier = get_team_history_dossier_in_base_dir(
        &base_dir,
        &career_id,
        &selected_team.id,
        &selected_team.categoria,
    )
    .expect("team dossier should use historical data");
    assert!(team_dossier.has_history);

    let ranking =
        get_global_driver_rankings_in_base_dir(&base_dir, &career_id, Some(&ai_driver_id))
            .expect("global ranking should use historical data");
    assert!(!ranking.rows.is_empty());
    assert!(ranking.rows.iter().any(|row| row.historical_index > 0.0));
    assert_eq!(
        ranking.selected_driver_id.as_deref(),
        Some(ai_driver_id.as_str())
    );

    let _ = std::fs::remove_dir_all(base_dir);
}

#[test]
fn finalize_draft_inserts_player_as_n2_and_displaces_existing_n2() {
    let base_dir = unique_test_dir("finalize_draft");
    let state = create_historical_career_draft_for_range_for_test(
        &base_dir,
        sample_draft_input(),
        2000,
        2000,
        2001,
    )
    .expect("draft should be created");
    let career_id = state.career_id.clone().expect("draft career id");
    let db = open_draft_db(&base_dir, &career_id);
    // Em 2001 a categoria de ENTRADA (mais básica ativa) é a gt3 — mazda_rookie
    // só existe a partir de 2020. O jogador entra na categoria-base da época.
    let selected_team = team_queries::get_teams_by_category(&db.conn, "gt3")
        .expect("teams by category")
        .into_iter()
        .next()
        .expect("at least one entry-category team");
    let displaced_n2 = selected_team
        .piloto_2_id
        .clone()
        .expect("team should have N2 before finalization");
    drop(db);

    let result = super::finalize_career_draft_in_base_dir(
        &base_dir,
        FinalizeHistoricalDraftInput {
            career_id: career_id.clone(),
            category: selected_team.categoria.clone(),
            team_id: selected_team.id.clone(),
        },
    )
    .expect("finalize should succeed");

    assert!(result.success);
    let db = open_draft_db(&base_dir, &career_id);
    let player = driver_queries::get_player_driver(&db.conn).expect("player should exist");
    assert_eq!(player.stats_temporada.corridas, 0);
    assert_eq!(player.stats_carreira.corridas, 0);
    let refreshed_team = team_queries::get_team_by_id(&db.conn, &selected_team.id)
        .expect("team query")
        .expect("selected team");
    assert_eq!(
        refreshed_team.piloto_2_id.as_deref(),
        Some(player.id.as_str())
    );
    assert_eq!(
        refreshed_team.hierarquia_n2_id.as_deref(),
        Some(player.id.as_str())
    );
    assert!(refreshed_team.is_player_team);
    assert!(
        contract_queries::get_active_regular_contract_for_pilot(&db.conn, &displaced_n2)
            .expect("displaced contract query")
            .is_none()
    );

    let _ = std::fs::remove_dir_all(base_dir);
}

fn sample_draft_input() -> CreateHistoricalDraftInput {
    CreateHistoricalDraftInput {
        player_name: "Joao Silva".to_string(),
        player_nationality: "br".to_string(),
        player_age: Some(22),
        difficulty: "medio".to_string(),
    }
}

fn open_draft_db(base_dir: &Path, career_id: &str) -> Database {
    let config = AppConfig::load_or_default(base_dir);
    let db_path = config.saves_dir().join(career_id).join("career.db");
    Database::open_existing(&db_path).expect("draft db should open")
}

fn unique_test_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("iracer_historical_draft_{label}_{nanos}"))
}
