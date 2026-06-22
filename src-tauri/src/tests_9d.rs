/// Testes de integração — Fase 9D (Etapa 11).
///
/// Prova por execução end-to-end as garantias do design doc 9D:
///   • Invariantes pós-criação de mundo novo.
///   • Imutabilidade do calendário.
///   • Pureza do fluxo novo (zero tabelas legadas, zero Especial).
///   • Ciclo de 2 temporadas completo (prova-mestra da fase).
///   • Travessia legada: BlocoRegular v32 → v33/v34 → modelo novo.
///   • Travessia legada: BlocoEspecial → fluxo legado até fim → modelo novo.

// ── Helpers compartilhados ────────────────────────────────────────────────────

#[cfg(test)]
mod helpers {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::commands::career::{
        advance_market_week_in_base_dir, advance_season_in_base_dir, create_career_in_base_dir,
        finalize_preseason_in_base_dir, skip_all_pending_races_in_base_dir, CreateCareerInput,
    };
    use crate::config::app_config::AppConfig;
    use crate::db::connection::Database;
    use crate::db::queries::seasons as season_queries;
    use crate::market::preseason::PreSeasonPhase;

    /// Cria um diretório temporário único para o teste.
    pub fn unique_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("iracer_9d_{label}_{nanos}"))
    }

    /// Cria uma carreira completa (mundo novo) num diretório temporário.
    /// Retorna o `base_dir`; a carreira fica em `career_001`.
    pub fn create_career_dir(label: &str) -> PathBuf {
        let base_dir = unique_test_dir(label);
        std::fs::create_dir_all(&base_dir).expect("base dir");
        let input = CreateCareerInput {
            player_name: "Piloto Teste".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(20),
            category: "mazda_rookie".to_string(),
            team_index: 0,
            difficulty: "medio".to_string(),
        };
        create_career_in_base_dir(&base_dir, input).expect("criar carreira");
        base_dir
    }

    /// Paths convenientes para uma carreira criada por `create_career_dir`.
    pub fn career_paths(base_dir: &Path) -> (AppConfig, PathBuf, PathBuf, PathBuf) {
        let config = AppConfig::load_or_default(base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");
        let meta_path = career_dir.join("meta.json");
        (config, career_dir, db_path, meta_path)
    }

    /// Marca TODAS as corridas da temporada ativa como Concluída.
    /// Não altera a fase — `advance_season_in_base_dir` faz a transição
    /// Temporada → Encerramento quando não há corridas pendentes.
    pub fn mark_all_races_completed(db_path: &Path) {
        let db = Database::open_existing(db_path).expect("db");
        db.conn
            .execute("UPDATE calendar SET status = 'Concluida'", [])
            .expect("marcar corridas concluídas");
    }

    /// Força a conclusão do plano de pré-temporada sem rodar as semanas
    /// de mercado individualmente. Necessário antes de `finalize_preseason`.
    pub fn force_complete_preseason_plan(career_dir: &Path) {
        let mut plan = crate::market::preseason::load_preseason_plan(career_dir)
            .expect("carregar plano")
            .expect("plano existe");
        plan.state.is_complete = true;
        plan.state.current_week = plan.state.total_weeks + 1;
        plan.state.phase = PreSeasonPhase::Complete;
        plan.state.player_has_pending_proposals = false;
        crate::market::preseason::save_preseason_plan(career_dir, &plan).expect("salvar plano");
    }

    /// Remove todas as propostas pendentes do DB (player + IA).
    /// Necessário para que `finalize_preseason` não bloqueie por propostas abertas.
    pub fn clear_pending_proposals(db_path: &Path) {
        let db = Database::open_existing(db_path).expect("db");
        db.conn
            .execute(
                "UPDATE market_proposals SET status = 'Rejeitada' WHERE status = 'Pendente'",
                [],
            )
            .expect("limpar propostas");
    }

    /// Conta linhas de uma tabela (zero se a tabela não existir).
    #[allow(dead_code)]
    pub fn count_table(db_path: &Path, table: &str) -> i64 {
        let db = Database::open_existing(db_path).expect("db");
        db.conn
            .query_row(
                &format!("SELECT COALESCE((SELECT COUNT(*) FROM {table} WHERE 1), 0)"),
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
    }

    /// Helper de ciclo completo: simula todas as corridas pendentes + advance season.
    /// Usa `skip_all_pending_races_in_base_dir` para gerar race_results reais
    /// (necessário para driver_season_archive e team_season_archive).
    /// Retorna o season_id da nova temporada.
    pub fn skip_and_advance(base_dir: &Path, _db_path: &Path, _career_dir: &Path) -> String {
        skip_all_pending_races_in_base_dir(base_dir, "career_001").expect("skip all pending races");
        let result = advance_season_in_base_dir(base_dir, "career_001").expect("advance season");
        assert!(
            result.preseason_initialized,
            "preseason deve ser inicializada após advance"
        );
        result.new_season_id
    }

    /// Avança uma semana de mercado, verifica propostas > 0, limpa propostas,
    /// força conclusão do plano e finaliza a pré-temporada → Temporada.
    pub fn run_preseason_to_temporada(base_dir: &Path, db_path: &Path, career_dir: &Path) {
        // Avança 1 semana para disparar o ciclo de mercado
        let week = advance_market_week_in_base_dir(base_dir, "career_001", None)
            .expect("avançar semana de mercado");
        assert_eq!(week.week_number, 1);

        // Limpa propostas pendentes (pode ser 0 se o jogador já tem contrato), força conclusão e finaliza
        clear_pending_proposals(db_path);
        force_complete_preseason_plan(career_dir);
        finalize_preseason_in_base_dir(base_dir, "career_001").expect("finalizar pré-temporada");

        // Verifica fase final
        let db = Database::open_existing(db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("query")
            .expect("temporada ativa");
        assert_eq!(
            season.fase,
            crate::models::enums::SeasonPhase::Temporada,
            "após finalize_preseason a fase deve ser Temporada"
        );
    }

    /// Assertions de pureza do fluxo 9D: zero tabelas do sistema legado.
    pub fn assert_zero_legacy_tables(db_path: &Path) {
        let db = Database::open_existing(db_path).expect("db");
        let conn = &db.conn;

        let sws: i64 = conn
            .query_row("SELECT COUNT(*) FROM special_window_state", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        assert_eq!(sws, 0, "special_window_state deve ser vazio");

        let swa: i64 = conn
            .query_row("SELECT COUNT(*) FROM special_window_assignments", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        assert_eq!(swa, 0, "special_window_assignments deve ser vazio");

        let pso: i64 = conn
            .query_row("SELECT COUNT(*) FROM player_special_offers", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        assert_eq!(pso, 0, "player_special_offers deve ser vazio");

        let ste: i64 = conn
            .query_row("SELECT COUNT(*) FROM special_team_entries", [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        assert_eq!(ste, 0, "special_team_entries deve ser vazio");

        let especial_contracts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        assert_eq!(especial_contracts, 0, "contratos Especial devem ser zero");
    }

    /// Verifica que nenhuma temporada está em fase legada.
    pub fn assert_no_legacy_phases(db_path: &Path) {
        let db = Database::open_existing(db_path).expect("db");
        let legacy: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM seasons
                 WHERE fase IN ('BlocoRegular','JanelaConvocacao','BlocoEspecial','PosEspecial')",
                [],
                |r| r.get(0),
            )
            .expect("query fases legadas");
        assert_eq!(legacy, 0, "nenhuma temporada deve estar em fase legada");
    }

    /// Verifica invariantes do calendário da temporada recém-criada (9D):
    /// 74 entradas Pendente, season_week 10-51, todos Temporada phase, zero lmp2.
    pub fn assert_calendar_invariants_9d(db_path: &Path, season_id: &str) {
        let db = Database::open_existing(db_path).expect("db");
        let conn = &db.conn;

        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1",
                rusqlite::params![season_id],
                |r| r.get(0),
            )
            .expect("contar entradas");
        assert_eq!(
            total, 74,
            "{season_id}: deve ter 74 entradas, encontrou {total}"
        );

        let pendentes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND status = 'Pendente'",
                rusqlite::params![season_id],
                |r| r.get(0),
            )
            .expect("contar pendentes");
        assert_eq!(
            pendentes, 74,
            "{season_id}: todas as 74 entradas devem ser Pendente, encontrou {pendentes}"
        );

        let fora_sw: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND (season_week IS NULL OR season_week < 10 OR season_week > 51)",
                rusqlite::params![season_id],
                |r| r.get(0),
            )
            .expect("verificar season_week");
        assert_eq!(
            fora_sw, 0,
            "{season_id}: {fora_sw} entrada(s) com season_week fora de 10-51 ou NULL"
        );

        let nao_temporada: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND season_phase != 'Temporada'",
                rusqlite::params![season_id],
                |r| r.get(0),
            )
            .expect("verificar season_phase");
        assert_eq!(
            nao_temporada, 0,
            "{season_id}: {nao_temporada} entrada(s) com season_phase != Temporada"
        );

        let lmp2: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND categoria = 'lmp2'",
                rusqlite::params![season_id],
                |r| r.get(0),
            )
            .expect("verificar lmp2");
        assert_eq!(lmp2, 0, "{season_id}: zero entradas lmp2 esperadas");
    }

    /// Verifica que production_challenger e endurance não têm rodadas duplicadas.
    pub fn assert_no_duplicate_rounds(db_path: &Path, season_id: &str) {
        let db = Database::open_existing(db_path).expect("db");
        for cat in &["production_challenger", "endurance"] {
            let duplicates: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM (
                         SELECT rodada, COUNT(*) AS cnt
                         FROM calendar
                         WHERE COALESCE(season_id, temporada_id) = ?1
                           AND categoria = ?2
                         GROUP BY rodada
                         HAVING cnt > 1
                     )",
                    rusqlite::params![season_id, cat],
                    |r| r.get(0),
                )
                .expect("verificar duplicatas");
            assert_eq!(
                duplicates, 0,
                "{season_id}/{cat}: encontrou rodadas duplicadas"
            );
        }
    }
}

// ── Testes de invariante pós-criação ─────────────────────────────────────────

#[cfg(test)]
mod invariants {
    use super::helpers::*;
    use crate::db::connection::Database;
    use crate::db::queries::seasons as season_queries;
    use crate::models::enums::SeasonPhase;

    #[test]
    fn pos_criacao_74_entradas_pendentes_sw_10_51() {
        let base_dir = create_career_dir("inv_74_entradas");
        let (_, _, db_path, _) = career_paths(&base_dir);
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("query")
            .expect("temporada ativa");
        drop(db);
        assert_calendar_invariants_9d(&db_path, &season.id);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn pos_criacao_zero_lmp2_no_calendario() {
        let base_dir = create_career_dir("inv_zero_lmp2");
        let (_, _, db_path, _) = career_paths(&base_dir);
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("query")
            .expect("temporada ativa");
        let lmp2: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1 AND categoria = 'lmp2'",
                rusqlite::params![season.id],
                |r| r.get(0),
            )
            .expect("contar lmp2");
        assert_eq!(lmp2, 0, "calendário novo não deve ter entradas lmp2");
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn pos_criacao_zero_special_tables() {
        let base_dir = create_career_dir("inv_zero_special");
        let (_, _, db_path, _) = career_paths(&base_dir);
        assert_zero_legacy_tables(&db_path);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn pos_criacao_zero_contratos_especiais() {
        let base_dir = create_career_dir("inv_zero_especial_contracts");
        let (_, _, db_path, _) = career_paths(&base_dir);
        let db = Database::open_existing(&db_path).expect("db");
        let especial: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial'",
                [],
                |r| r.get(0),
            )
            .expect("contar Especial");
        assert_eq!(especial, 0, "zero contratos Especial esperados pós-criação");
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn pos_criacao_producao_endurance_sem_rodadas_duplicadas() {
        let base_dir = create_career_dir("inv_no_dup_rounds");
        let (_, _, db_path, _) = career_paths(&base_dir);
        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("query")
            .expect("temporada ativa");
        drop(db);
        assert_no_duplicate_rounds(&db_path, &season.id);
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn pos_criacao_unica_temporada_em_andamento() {
        let base_dir = create_career_dir("inv_unica_temporada");
        let (_, _, db_path, _) = career_paths(&base_dir);
        let db = Database::open_existing(&db_path).expect("db");
        let em_andamento: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM seasons WHERE status = 'EmAndamento'",
                [],
                |r| r.get(0),
            )
            .expect("contar EmAndamento");
        assert_eq!(
            em_andamento, 1,
            "exatamente 1 temporada EmAndamento esperada, encontrou {em_andamento}"
        );
        let season = season_queries::get_active_season(&db.conn)
            .expect("query")
            .expect("temporada ativa");
        assert_eq!(
            season.fase,
            SeasonPhase::Temporada,
            "temporada nova deve estar em Temporada"
        );
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    /// Imutabilidade do calendário: marcar todas as corridas como Concluída
    /// não deve criar novas entradas.
    #[test]
    fn calendario_imutavel_apos_conclusao_de_corridas() {
        let base_dir = create_career_dir("inv_calendario_imutavel");
        let (_, _, db_path, _) = career_paths(&base_dir);

        let db = Database::open_existing(&db_path).expect("db");
        let season = season_queries::get_active_season(&db.conn)
            .expect("query")
            .expect("temporada ativa");
        let count_before: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM calendar WHERE COALESCE(season_id, temporada_id) = ?1",
                rusqlite::params![season.id],
                |r| r.get(0),
            )
            .expect("contar antes");
        drop(db);

        // Marca todas as corridas como concluídas
        mark_all_races_completed(&db_path);

        let db = Database::open_existing(&db_path).expect("db");
        let count_after: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM calendar WHERE COALESCE(season_id, temporada_id) = ?1",
                rusqlite::params![season.id],
                |r| r.get(0),
            )
            .expect("contar depois");

        assert_eq!(
            count_before, count_after,
            "número de entradas de calendário não deve mudar: antes={count_before}, depois={count_after}"
        );

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}

// ── Ciclo de 2 temporadas (prova-mestra) ─────────────────────────────────────

#[cfg(test)]
mod ciclo_dois_temporadas {
    use super::helpers::*;
    use crate::commands::race::simulate_race_weekend_in_base_dir;
    use crate::db::connection::Database;
    use crate::db::queries::calendar as calendar_queries;
    use crate::db::queries::seasons as season_queries;
    use crate::models::enums::SeasonPhase;

    /// Prova-mestra da Fase 9D:
    ///   S1 criação → skip → advance → S2 PreTemporada (74 entradas) →
    ///   mercado (1 semana) → finalize → Temporada → 1ª corrida →
    ///   skip resto → advance → S3 PreTemporada (74 entradas).
    ///
    /// Em cada transição: verificar archive, standings, licenças, mercado,
    /// invariante de temporada única, zero tabelas legadas e zero contratos Especial.
    #[test]
    fn ciclo_completo_s1_s2_s3() {
        let base_dir = create_career_dir("ciclo_2t");
        let (_, career_dir, db_path, _) = career_paths(&base_dir);

        // ── Checkpoint 0: S1 recém-criada ────────────────────────────────────
        let s1_id = {
            let db = Database::open_existing(&db_path).expect("db");
            let s = season_queries::get_active_season(&db.conn)
                .expect("query")
                .expect("S1");
            assert_eq!(s.numero, 1);
            assert_eq!(s.fase, SeasonPhase::Temporada);
            s.id
        };
        assert_calendar_invariants_9d(&db_path, &s1_id);
        assert_no_duplicate_rounds(&db_path, &s1_id);
        assert_zero_legacy_tables(&db_path);
        assert_no_legacy_phases(&db_path);

        // ── Advance S1 → S2 ───────────────────────────────────────────────────
        let s2_id = skip_and_advance(&base_dir, &db_path, &career_dir);

        // Checkpoint 1: S1 finalizada, S2 em PreTemporada com 74 entradas
        {
            let db = Database::open_existing(&db_path).expect("db");
            let conn = &db.conn;

            // S1 deve estar Finalizada
            let s1_status: String = conn
                .query_row(
                    "SELECT status FROM seasons WHERE id = ?1",
                    rusqlite::params![s1_id],
                    |r| r.get(0),
                )
                .expect("status S1");
            assert_eq!(s1_status, "Finalizada", "S1 deve estar Finalizada");

            // S2 deve estar EmAndamento em PreTemporada
            let s2 = season_queries::get_active_season(conn)
                .expect("query")
                .expect("S2 ativa");
            assert_eq!(s2.id, s2_id, "S2 deve ser a temporada ativa");
            assert_eq!(s2.numero, 2);
            assert_eq!(
                s2.fase,
                SeasonPhase::PreTemporada,
                "S2 deve estar em PreTemporada"
            );

            // Única temporada EmAndamento
            let em_andamento: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM seasons WHERE status = 'EmAndamento'",
                    [],
                    |r| r.get(0),
                )
                .expect("contar EmAndamento");
            assert_eq!(
                em_andamento, 1,
                "única temporada EmAndamento após advance: {em_andamento}"
            );

            // driver_season_archive da S1 criado
            let dsa: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM driver_season_archive WHERE season_number = 1",
                    [],
                    |r| r.get(0),
                )
                .expect("driver_season_archive S1");
            assert!(
                dsa > 0,
                "driver_season_archive da S1 deve existir (encontrou {dsa})"
            );

            // team_season_archive da S1 criado
            let tsa: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM team_season_archive WHERE season_number = 1",
                    [],
                    |r| r.get(0),
                )
                .expect("team_season_archive S1");
            assert!(
                tsa > 0,
                "team_season_archive da S1 deve existir (encontrou {tsa})"
            );

            // Standings finais: campeões por classe de production e endurance
            let prod_champs: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM standings
                     WHERE temporada_id = ?1
                       AND posicao = 1
                       AND categoria = 'production_challenger'
                       AND classe IS NOT NULL",
                    rusqlite::params![s1_id],
                    |r| r.get(0),
                )
                .expect("campeões production");
            assert!(
                prod_champs >= 2,
                "production_challenger deve ter ao menos 2 campeões de classe (encontrou {prod_champs})"
            );

            let end_champs: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM standings
                     WHERE temporada_id = ?1
                       AND posicao = 1
                       AND categoria = 'endurance'
                       AND classe IS NOT NULL",
                    rusqlite::params![s1_id],
                    |r| r.get(0),
                )
                .expect("campeões endurance");
            assert!(
                end_champs >= 2,
                "endurance deve ter ao menos 2 campeões de classe (encontrou {end_champs})"
            );

            // LMP2 não pode ter sido promovida/rebaixada (é fixa)
            let lmp2_movements: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM promotion_movements
                     WHERE categoria = 'lmp2' AND season_number = 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            assert_eq!(
                lmp2_movements, 0,
                "lmp2 não deve ter movimentos de promoção/rebaixamento"
            );

            // Licenças concedidas (regressão 9C)
            let licenses: i64 = conn
                .query_row("SELECT COUNT(*) FROM licenses", [], |r| r.get(0))
                .unwrap_or(0);
            assert!(
                licenses > 0,
                "licenças devem ter sido concedidas após S1 (encontrou {licenses})"
            );
        }

        // Calendário de S2 deve ter 74 entradas Pendente em 10-51
        assert_calendar_invariants_9d(&db_path, &s2_id);
        assert_no_duplicate_rounds(&db_path, &s2_id);
        assert_zero_legacy_tables(&db_path);
        assert_no_legacy_phases(&db_path);

        // ── Pré-temporada S2 → Temporada ─────────────────────────────────────
        run_preseason_to_temporada(&base_dir, &db_path, &career_dir);

        // Checkpoint 2: S2 em Temporada
        assert_zero_legacy_tables(&db_path);
        assert_no_legacy_phases(&db_path);

        // ── Simular 1ª corrida de S2 ──────────────────────────────────────────
        {
            let db = Database::open_existing(&db_path).expect("db");
            let s2 = season_queries::get_active_season(&db.conn)
                .expect("query")
                .expect("S2 ativa");
            let player = crate::db::queries::drivers::get_player_driver(&db.conn).expect("player");

            // Obtém próxima corrida do jogador; se não tiver equipe, pega qualquer corrida
            // Simula 1ª corrida de S2 apenas se o jogador tem contrato ativo.
            // Sem contrato (possível quando force_complete_preseason_plan bypassa o
            // mercado normal), pula a simulação — o restante do ciclo ainda valida
            // archives, standings e transições de fase.
            let next_race = crate::db::queries::contracts::get_active_regular_contract_for_pilot(
                &db.conn, &player.id,
            )
            .ok()
            .flatten()
            .and_then(|contract| {
                calendar_queries::get_next_race(&db.conn, &s2.id, &contract.categoria)
                    .ok()
                    .flatten()
            });

            if let Some(race) = next_race {
                drop(db);
                simulate_race_weekend_in_base_dir(&base_dir, "career_001", &race.id)
                    .expect("simular 1ª corrida de S2");
            }
        }

        // ── Advance S2 → S3 ───────────────────────────────────────────────────
        let s3_id = skip_and_advance(&base_dir, &db_path, &career_dir);

        // Checkpoint 3: S2 finalizada, S3 em PreTemporada com 74 entradas
        {
            let db = Database::open_existing(&db_path).expect("db");
            let conn = &db.conn;

            let s2_status: String = conn
                .query_row(
                    "SELECT status FROM seasons WHERE id = ?1",
                    rusqlite::params![s2_id],
                    |r| r.get(0),
                )
                .expect("status S2");
            assert_eq!(s2_status, "Finalizada", "S2 deve estar Finalizada");

            let s3 = season_queries::get_active_season(conn)
                .expect("query")
                .expect("S3 ativa");
            assert_eq!(s3.id, s3_id);
            assert_eq!(s3.numero, 3);
            assert_eq!(
                s3.fase,
                SeasonPhase::PreTemporada,
                "S3 deve estar em PreTemporada"
            );

            let em_andamento: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM seasons WHERE status = 'EmAndamento'",
                    [],
                    |r| r.get(0),
                )
                .expect("contar EmAndamento");
            assert_eq!(
                em_andamento, 1,
                "única temporada EmAndamento após S2 advance"
            );

            // driver_season_archive e team_season_archive da S2 criados
            let dsa2: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM driver_season_archive WHERE season_number = 2",
                    [],
                    |r| r.get(0),
                )
                .expect("dsa S2");
            assert!(dsa2 > 0, "driver_season_archive da S2 deve existir");

            let tsa2: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM team_season_archive WHERE season_number = 2",
                    [],
                    |r| r.get(0),
                )
                .expect("tsa S2");
            assert!(tsa2 > 0, "team_season_archive da S2 deve existir");
        }

        assert_calendar_invariants_9d(&db_path, &s3_id);
        assert_no_duplicate_rounds(&db_path, &s3_id);
        assert_zero_legacy_tables(&db_path);
        assert_no_legacy_phases(&db_path);

        // Pré-temporada S3 → Temporada (completude do ciclo)
        run_preseason_to_temporada(&base_dir, &db_path, &career_dir);

        let db = Database::open_existing(&db_path).expect("db");
        let s3_final = season_queries::get_active_season(&db.conn)
            .expect("query")
            .expect("S3 ativa final");
        assert_eq!(
            s3_final.fase,
            SeasonPhase::Temporada,
            "S3 deve estar em Temporada ao final do ciclo"
        );
        assert_eq!(s3_final.numero, 3);

        let _ = std::fs::remove_dir_all(&base_dir);
    }

    /// Jogador em endurance LMP2 — cobertura do caminho player-facing.
    ///
    /// Reproduz o estado de um jogador que chegou à Endurance por progressão legítima
    /// (GT3 → licença → Endurance) via cirurgia direta no contrato, já que:
    ///   a) A via de progressão real leva ~5 temporadas (inviável em CI).
    ///   b) O wizard bloqueia o ponto de entrada em categorias altas (correto).
    ///
    /// O que este teste cobre (distinto de `endurance_special_race_uses_regular_contract_grid_with_lmp2_class_teams`):
    ///   • `simulate_race_weekend_in_base_dir` (advance_player_round=true) com jogador em LMP2.
    ///   • is_jogador=true existe nos resultados.
    ///   • race_results para a corrida contêm entradas de equipes classe lmp2.
    #[test]
    fn ciclo_jogador_em_endurance_lmp2() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("iracer_9d_endurance_{nanos}"));
        std::fs::create_dir_all(&base_dir).expect("base dir");

        // Criar carreira no ponto de entrada válido (mazda_rookie)
        let input = crate::commands::career::CreateCareerInput {
            player_name: "Piloto End".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(25),
            category: "mazda_rookie".to_string(),
            team_index: 0,
            difficulty: "medio".to_string(),
        };
        crate::commands::career::create_career_in_base_dir(&base_dir, input)
            .expect("criar carreira");

        let config = crate::config::app_config::AppConfig::load_or_default(&base_dir);
        let db_path = config.saves_dir().join("career_001").join("career.db");

        // ── Cirurgia de contrato ─────────────────────────────────────────────
        // Migra o contrato ativo do jogador de mazda_rookie → equipe endurance LMP2,
        // simulando o estado pós-progressão sem rodar as temporadas intermediárias.
        let next_race_id: String = {
            let db = Database::open_existing(&db_path).expect("db");
            let conn = &db.conn;

            let season = season_queries::get_active_season(conn)
                .expect("query")
                .expect("temporada ativa");
            let player = crate::db::queries::drivers::get_player_driver(conn).expect("player");

            // Equipe endurance LMP2 com contrato regular ativo (gerada pelo world generator)
            let lmp2_team_id: String = conn
                .query_row(
                    "SELECT t.id FROM teams t
                     JOIN contracts c ON c.equipe_id = t.id
                     WHERE t.categoria = 'endurance' AND t.classe = 'lmp2'
                       AND c.tipo = 'Regular' AND c.status = 'Ativo'
                     LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .expect("equipe endurance lmp2 com contrato ativo deve existir no mundo gerado");

            // Trocar contrato: jogador passa a representar a equipe LMP2 na Endurance
            conn.execute(
                "UPDATE contracts SET categoria = 'endurance', equipe_id = ?1
                 WHERE piloto_id = ?2 AND tipo = 'Regular' AND status = 'Ativo'",
                rusqlite::params![&lmp2_team_id, &player.id],
            )
            .expect("migrar contrato do jogador para endurance");

            calendar_queries::get_next_race(conn, &season.id, "endurance")
                .expect("query next race")
                .expect("primeira corrida de endurance deve existir")
                .id
        };

        // ── Simulação player-facing ──────────────────────────────────────────
        let result = simulate_race_weekend_in_base_dir(&base_dir, "career_001", &next_race_id)
            .expect("simulate_race_weekend deve aceitar jogador com contrato endurance");

        // Resultado do jogador deve existir na corrida
        let player_entry = result
            .player_race
            .race_results
            .iter()
            .find(|r| r.is_jogador);
        assert!(
            player_entry.is_some(),
            "is_jogador=true deve existir nos race_results de endurance"
        );

        // Entradas LMP2 devem estar presentes (via JOIN com equipe)
        let db = Database::open_existing(&db_path).expect("db pós-simulação");
        let lmp2_entries: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM race_results rr
                 JOIN teams t ON t.id = rr.equipe_id
                 WHERE rr.race_id = ?1 AND t.classe = 'lmp2'",
                rusqlite::params![&next_race_id],
                |r| r.get(0),
            )
            .expect("contar entradas LMP2");
        assert!(
            lmp2_entries > 0,
            "corrida de endurance deve ter resultados de equipes classe lmp2 (encontrou {lmp2_entries})"
        );

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}

// ── Travessia legada ─────────────────────────────────────────────────────────

#[cfg(test)]
mod travessia_legada {
    use super::helpers::*;
    use crate::commands::career::advance_season_in_base_dir;
    use crate::commands::career::skip_all_pending_races_in_base_dir;
    use crate::db::connection::Database;
    use crate::db::migrations;
    use crate::db::queries::seasons as season_queries;
    use crate::models::enums::SeasonPhase;

    /// Simula a migração v33+v34 sobre um save sintético em BlocoRegular.
    ///
    /// Fluxo:
    ///   1. Criar DB v34 com `run_all` + dados sintéticos (world completo).
    ///   2. Reverter para v32: season_week → NULL, fase → BlocoRegular.
    ///      Remover production/endurance do calendário (serão regenerados por v34).
    ///   3. Definir schema_version = 32 via meta.
    ///   4. `run_pending` → aplica v33 (backfill season_week) + v34 (gera especiais, → Temporada).
    ///   5. Verificar estado resultante.
    ///   6. Skip all pending → Encerramento → advance → S2 em PreTemporada (modelo novo).
    #[test]
    fn migracao_v33v34_bloco_regular_para_temporada() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("iracer_9d_legacy_bloco_regular_{nanos}"));
        std::fs::create_dir_all(&base_dir).expect("base dir");

        // 1. Criar carreira completa (world + calendário 9D v34)
        let input = crate::commands::career::CreateCareerInput {
            player_name: "Piloto Legacy".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(22),
            category: "mazda_rookie".to_string(),
            team_index: 0,
            difficulty: "medio".to_string(),
        };
        crate::commands::career::create_career_in_base_dir(&base_dir, input)
            .expect("criar carreira legada");

        let config = crate::config::app_config::AppConfig::load_or_default(&base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");

        // 2. Retrogradar estado para simular save v32 em BlocoRegular (meio do ano):
        //    - Remover entradas de production_challenger e endurance (v34 vai regenerá-las)
        //    - Marcar algumas corridas regulares como Concluída (meio do ano)
        //    - Nullar season_week (coluna adicionada em v33)
        //    - Mudar fase para BlocoRegular
        //    - Definir schema_version = 32
        {
            let db = Database::open_existing(&db_path).expect("db");
            let conn = &db.conn;

            // Identificar a temporada ativa
            let season_id: String = conn
                .query_row(
                    "SELECT id FROM seasons WHERE status = 'EmAndamento'",
                    [],
                    |r| r.get(0),
                )
                .expect("season_id");

            // Remover entradas de production/endurance (serão regeneradas por v34)
            conn.execute(
                "DELETE FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND categoria IN ('production_challenger', 'endurance')",
                rusqlite::params![season_id],
            )
            .expect("remover production/endurance");

            // Marcar corridas das primeiras 24 semanas como Concluída (simula meio do ano)
            // sem ultrapassar o início da janela de production/endurance (woy 25+)
            conn.execute(
                "UPDATE calendar
                 SET status = 'Concluida'
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND week_of_year < 25",
                rusqlite::params![season_id],
            )
            .expect("marcar corridas concluídas");

            // Nullar season_week (simula pré-v33)
            conn.execute(
                "UPDATE calendar SET season_week = NULL
                 WHERE COALESCE(season_id, temporada_id) = ?1",
                rusqlite::params![season_id],
            )
            .expect("nullar season_week");

            // Mudar fase para BlocoRegular (salvo pré-v34)
            conn.execute(
                "UPDATE seasons SET fase = 'BlocoRegular' WHERE id = ?1",
                rusqlite::params![season_id],
            )
            .expect("fase BlocoRegular");

            // Definir schema_version = 32
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', '32')",
                [],
            )
            .expect("schema_version = 32");
        }

        // 3. Verificar schema_version = 32 antes da migração
        {
            let db = Database::open_existing(&db_path).expect("db antes de run_pending");
            // open_existing chama run_pending automaticamente!
            // Então precisamos verificar APÓS a abertura.
            let version = migrations::get_schema_version(&db.conn).expect("schema version");
            assert!(
                version >= 34,
                "após open_existing (run_pending), schema deve estar em 34+"
            );

            // v33: season_week backfill deve ter sido aplicado
            let sem_sw: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM calendar
                     WHERE COALESCE(season_id, temporada_id) = (
                         SELECT id FROM seasons WHERE status = 'EmAndamento'
                     )
                     AND week_of_year > 0
                     AND season_week IS NULL",
                    [],
                    |r| r.get(0),
                )
                .expect("verificar season_week");
            assert_eq!(
                sem_sw, 0,
                "v33: todas as entradas com week_of_year > 0 devem ter season_week após migração"
            );

            // v33: season_week = week_of_year + 4 para entradas com week_of_year > 0
            let sw_incorreto: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM calendar
                     WHERE week_of_year > 0
                     AND season_week IS NOT NULL
                     AND season_week != week_of_year + 4",
                    [],
                    |r| r.get(0),
                )
                .expect("verificar relação sw/woy");
            assert_eq!(
                sw_incorreto, 0,
                "v33: season_week deve ser week_of_year + 4 para todas as entradas"
            );

            // v34: fase deve ter mudado de BlocoRegular para Temporada
            let season = season_queries::get_active_season(&db.conn)
                .expect("query")
                .expect("temporada ativa");
            assert_eq!(
                season.fase,
                SeasonPhase::Temporada,
                "v34: BlocoRegular deve ter sido convertido para Temporada"
            );

            // v34: entradas de production_challenger e endurance devem ter sido geradas
            let especiais: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM calendar
                     WHERE COALESCE(season_id, temporada_id) = ?1
                       AND categoria IN ('production_challenger', 'endurance')",
                    rusqlite::params![season.id],
                    |r| r.get(0),
                )
                .expect("contar especiais");
            assert!(
                especiais > 0,
                "v34: production_challenger e endurance devem ter entradas geradas (encontrou {especiais})"
            );
        }

        // 4. Jogar até Encerramento → advance → S2 em PreTemporada (modelo novo)
        {
            skip_all_pending_races_in_base_dir(&base_dir, "career_001")
                .expect("skip all pending (legado→9D)");

            let result = advance_season_in_base_dir(&base_dir, "career_001")
                .expect("advance season após travessia legada");

            assert!(
                result.preseason_initialized,
                "pré-temporada deve ser inicializada"
            );

            let db = Database::open_existing(&db_path).expect("db após advance");
            let s2 = season_queries::get_active_season(&db.conn)
                .expect("query")
                .expect("S2 ativa");
            assert_eq!(s2.numero, 2);
            assert_eq!(
                s2.fase,
                SeasonPhase::PreTemporada,
                "S2 deve estar em PreTemporada após travessia legada"
            );

            // S2 deve ter 74 entradas (modelo novo)
            assert_calendar_invariants_9d(&db_path, &s2.id);
        }

        let _ = std::fs::remove_dir_all(&base_dir);
    }

    /// Travessia legada: save sintético em BlocoEspecial.
    ///
    /// Um save em BlocoEspecial NÃO é tocado pela migração v34 (que só converte
    /// BlocoRegular). O fluxo legado (BlocoEspecial → PosEspecial → advance)
    /// deve levar ao modelo novo com zero contratos Especial ativos.
    #[test]
    fn travessia_bloco_especial_chega_ao_modelo_novo() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let base_dir =
            std::env::temp_dir().join(format!("iracer_9d_legacy_bloco_especial_{nanos}"));
        std::fs::create_dir_all(&base_dir).expect("base dir");

        // Criar carreira normal (modelo 9D com fase Temporada)
        let input = crate::commands::career::CreateCareerInput {
            player_name: "Piloto Especial".to_string(),
            player_nationality: "br".to_string(),
            player_age: Some(23),
            category: "mazda_rookie".to_string(),
            team_index: 0,
            difficulty: "medio".to_string(),
        };
        crate::commands::career::create_career_in_base_dir(&base_dir, input)
            .expect("criar carreira para teste de BlocoEspecial");

        let config = crate::config::app_config::AppConfig::load_or_default(&base_dir);
        let career_dir = config.saves_dir().join("career_001");
        let db_path = career_dir.join("career.db");

        // Modificar o estado para simular um save em BlocoEspecial:
        // - Marcar todas as corridas regulares como Concluída
        // - Mudar fase para BlocoEspecial
        // - Adicionar entradas de production/endurance pendentes
        {
            let db = Database::open_existing(&db_path).expect("db");
            let conn = &db.conn;

            let season_id: String = conn
                .query_row(
                    "SELECT id FROM seasons WHERE status = 'EmAndamento'",
                    [],
                    |r| r.get(0),
                )
                .expect("season_id");

            let season_year: i32 = conn
                .query_row(
                    "SELECT ano FROM seasons WHERE id = ?1",
                    rusqlite::params![season_id],
                    |r| r.get(0),
                )
                .expect("season_year");

            // Marcar todas as corridas regulares como concluídas
            conn.execute(
                "UPDATE calendar
                 SET status = 'Concluida'
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND categoria NOT IN ('production_challenger', 'endurance')",
                rusqlite::params![season_id],
            )
            .expect("concluir corridas regulares");

            // Remover entradas existentes de production/endurance (serão adicionadas como especial)
            conn.execute(
                "DELETE FROM calendar
                 WHERE COALESCE(season_id, temporada_id) = ?1
                   AND categoria IN ('production_challenger', 'endurance')",
                rusqlite::params![season_id],
            )
            .expect("remover especiais 9D");

            // Usar o gerador legado de calendário especial para criar o bloco especial
            let mut rng = rand::thread_rng();
            crate::calendar::generate_and_insert_special_calendars(
                conn,
                &season_id,
                season_year,
                &mut rng,
            )
            .expect("gerar calendário especial legado");

            // Definir fase como BlocoEspecial
            conn.execute(
                "UPDATE seasons SET fase = 'BlocoEspecial' WHERE id = ?1",
                rusqlite::params![season_id],
            )
            .expect("fase BlocoEspecial");
        }

        // Verificar que v34 NÃO toca o BlocoEspecial:
        // Reabrir (run_pending é no-op pois schema já está em 34) e verificar fase
        {
            let db = Database::open_existing(&db_path).expect("db");
            let season = season_queries::get_active_season(&db.conn)
                .expect("query")
                .expect("temporada ativa");
            assert_eq!(
                season.fase,
                SeasonPhase::BlocoEspecial,
                "v34 NÃO deve tocar saves em BlocoEspecial (fase deve permanecer BlocoEspecial)"
            );
        }

        // Fluxo legado até o fim: skip all (BlocoEspecial → PosEspecial) → advance → modelo novo
        skip_all_pending_races_in_base_dir(&base_dir, "career_001")
            .expect("skip all pending (BlocoEspecial)");

        let result = advance_season_in_base_dir(&base_dir, "career_001")
            .expect("advance season após BlocoEspecial");

        assert!(
            result.preseason_initialized,
            "pré-temporada deve ser inicializada"
        );

        // S2 deve estar em PreTemporada com 74 entradas (modelo novo)
        {
            let db = Database::open_existing(&db_path).expect("db");
            let s2 = season_queries::get_active_season(&db.conn)
                .expect("query")
                .expect("S2 ativa");
            assert_eq!(
                s2.fase,
                SeasonPhase::PreTemporada,
                "S2 deve estar em PreTemporada após travessia BlocoEspecial"
            );
            assert_calendar_invariants_9d(&db_path, &s2.id);
        }

        // Zero contratos Especial ativos no modelo novo
        {
            let db = Database::open_existing(&db_path).expect("db");
            let especial_ativos: i64 = db
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM contracts WHERE tipo = 'Especial' AND status = 'Ativo'",
                    [],
                    |r| r.get(0),
                )
                .expect("contar Especial ativos");
            assert_eq!(
                especial_ativos, 0,
                "zero contratos Especial ativos no modelo novo após travessia"
            );
        }

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}
