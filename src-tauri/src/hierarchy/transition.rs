//! Transição de hierarquia interna de equipe entre temporadas.
//!
//! Helpers de decisão (domínio puro) + invariante final (com DB).
//! A persistência usa as DB functions existentes em db::queries::teams.
//!
//! Regra:
//! - `PartialPreserve`: mesma dupla, mesma direção, mesma categoria → preserva tensao/status,
//!   zera os 5 contadores temporais.
//! - `FullReset`: qualquer outra condição → tensao=0, status=Estavel, todos os contadores=0.

#![allow(dead_code)]

use rusqlite::Connection;

use crate::db::queries::teams as team_queries;
use crate::models::team::TeamHierarchyClimate;

// ── Tipos ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum HierarchyTransition {
    PartialPreserve,
    FullReset,
}

/// Estado hierárquico anterior da equipe (início da preseason — capturado de original_teams_by_id).
pub struct PrevHierarchyState<'a> {
    pub n1_id: Option<&'a str>,
    pub n2_id: Option<&'a str>,
    pub tensao: f64,
    pub status: &'a str,
    pub categoria: &'a str,
}

/// Configuração final da equipe para a nova temporada.
pub struct NewSeasonSetup<'a> {
    pub n1_id: Option<&'a str>,
    pub n2_id: Option<&'a str>,
    pub categoria: &'a str,
}

/// Configuração final canônica de uma equipe regular ao fim da preseason.
///
/// Garante: N1 presente, N2 presente, N1 ≠ N2.
/// Construída via `ResolvedTeamLineup::new()` — nunca diretamente.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTeamLineup {
    pub team_id: String,
    pub n1_id: String,
    pub n2_id: String,
}

impl ResolvedTeamLineup {
    /// Constrói e valida a configuração final.
    /// Recebe `Option<&str>` para mapear diretamente ao padrão `.as_deref()` existente.
    pub fn new(team_id: &str, n1_id: Option<&str>, n2_id: Option<&str>) -> Result<Self, String> {
        let n1 = n1_id.unwrap_or("").trim().to_string();
        let n2 = n2_id.unwrap_or("").trim().to_string();

        if n1.is_empty() {
            return Err(format!(
                "Equipe '{}': N1 ausente na configuração final",
                team_id
            ));
        }
        if n2.is_empty() {
            return Err(format!(
                "Equipe '{}': N2 ausente na configuração final",
                team_id
            ));
        }
        if n1 == n2 {
            return Err(format!(
                "Equipe '{}': N1 e N2 não podem ser o mesmo piloto ({})",
                team_id, n1
            ));
        }

        Ok(Self {
            team_id: team_id.to_string(),
            n1_id: n1,
            n2_id: n2,
        })
    }
}

// ── Decisão (domínio puro) ────────────────────────────────────────────────────

/// Decide o tipo de transição de hierarquia entre temporadas.
///
/// `PartialPreserve` apenas quando **todas** as condições são verdadeiras:
/// 1. mesma categoria (sem promoção/rebaixamento)
/// 2. novo lineup completo (N1 e N2 definidos)
/// 3. mesma identidade de N1 (mesma direção)
/// 4. mesma identidade de N2
///
/// Qualquer condição falsa → `FullReset`.
pub fn decide_hierarchy_transition(
    prev: &PrevHierarchyState<'_>,
    new: &NewSeasonSetup<'_>,
) -> HierarchyTransition {
    let preserve = prev.categoria == new.categoria
        && new.n1_id.is_some()
        && new.n2_id.is_some()
        && prev.n1_id == new.n1_id
        && prev.n2_id == new.n2_id;

    if preserve {
        HierarchyTransition::PartialPreserve
    } else {
        HierarchyTransition::FullReset
    }
}

/// Resolve os valores de tensao e status a persistir no banco.
///
/// `PartialPreserve` → preserva tensao e status anteriores (normalizados via enum).
/// `FullReset`       → tensao=0.0, status="estavel".
///
/// Os contadores de duelo (`hierarquia_duelos_total` etc.) são **sempre** resetados
/// pelo chamador via `update_team_duel_counters(..., 0, 0, 0, 0, 0)` — são temporais
/// por natureza e não fazem parte desta decisão.
pub fn resolve_transition_values(
    decision: &HierarchyTransition,
    prev_tensao: f64,
    prev_status: &str,
) -> (f64, &'static str) {
    match decision {
        HierarchyTransition::FullReset => (0.0, TeamHierarchyClimate::Estavel.as_str()),
        HierarchyTransition::PartialPreserve => {
            // Normaliza via enum para rejeitar valores legacy ("n1", "n2", etc.)
            let status = TeamHierarchyClimate::from_str(prev_status).as_str();
            (prev_tensao, status)
        }
    }
}

// ── Invariante de season (DB) ─────────────────────────────────────────────────

/// Garante que N1/N2 de toda equipe regular está alinhado com o lineup final.
///
/// Chamada após `fill_all_remaining_vacancies()` em `finalize_preseason_in_base_dir()`.
/// Equipes que passaram pelo `UpdateHierarchy` do mercado nunca chegam aqui desalinhadas —
/// esta função é safety net para equipes preenchidas por fallback sem contexto de hierarquia.
///
/// Retorna `Err` se encontrar equipe regular com lineup incompleto — violação de contrato
/// de grid que `fill_all_remaining_vacancies()` deve ter garantido.
pub fn validate_and_normalize_team_hierarchies(conn: &Connection) -> Result<(), String> {
    // Carregar equipes regulares ativas (categorias especiais têm reset próprio via
    // reset_special_team_hierarchies() no ciclo PosEspecial)
    let mut stmt = conn
        .prepare(
            "SELECT id, piloto_1_id, piloto_2_id, hierarquia_n1_id, hierarquia_n2_id
             FROM teams
             WHERE ativa = 1
               AND categoria NOT IN ('production_challenger', 'endurance')",
        )
        .map_err(|e| format!("Falha ao preparar validacao de hierarquia: {e}"))?;

    let rows: Vec<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("Falha ao executar validacao de hierarquia: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Falha ao coletar equipes: {e}"))?;

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Falha ao abrir transacao de hierarquia: {e}"))?;

    for (team_id, piloto_1, piloto_2, n1_id, n2_id) in rows {
        // Lineup incompleto = violação de contrato (fill_all_remaining_vacancies falhou)
        if piloto_1.is_none() || piloto_2.is_none() {
            return Err(format!(
                "Equipe '{}' com lineup incompleto antes de EmAndamento \
                 (piloto_1={:?}, piloto_2={:?})",
                team_id, piloto_1, piloto_2
            ));
        }

        let lineup = ResolvedTeamLineup::new(&team_id, piloto_1.as_deref(), piloto_2.as_deref())?;
        let p1 = Some(lineup.n1_id.as_str());
        let p2 = Some(lineup.n2_id.as_str());

        // N1/N2 já corretos → skip (caminho normal para equipes que passaram pelo UpdateHierarchy)
        if n1_id.as_deref() == p1 && n2_id.as_deref() == p2 {
            continue;
        }

        // Desalinhado → normalizar com reset completo.
        // Ocorre apenas para equipes sem contexto de preservação (vagas preenchidas por fallback).
        team_queries::update_team_hierarchy(
            &tx,
            &team_id,
            p1,
            p2,
            TeamHierarchyClimate::Estavel.as_str(),
            0.0,
        )
        .map_err(|e| format!("Falha ao normalizar hierarquia da equipe '{team_id}': {e}"))?;

        team_queries::update_team_duel_counters(&tx, &team_id, 0, 0, 0, 0, 0)
            .map_err(|e| format!("Falha ao resetar contadores da equipe '{team_id}': {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("Falha ao confirmar normalizacao de hierarquia: {e}"))?;

    Ok(())
}

// ── Testes ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn prev<'a>(
        n1: Option<&'a str>,
        n2: Option<&'a str>,
        tensao: f64,
        status: &'a str,
        cat: &'a str,
    ) -> PrevHierarchyState<'a> {
        PrevHierarchyState {
            n1_id: n1,
            n2_id: n2,
            tensao,
            status,
            categoria: cat,
        }
    }

    fn new_setup<'a>(n1: Option<&'a str>, n2: Option<&'a str>, cat: &'a str) -> NewSeasonSetup<'a> {
        NewSeasonSetup {
            n1_id: n1,
            n2_id: n2,
            categoria: cat,
        }
    }

    // ── decide_hierarchy_transition ──

    #[test]
    fn test_partial_preserve_same_pair_same_cat() {
        let p = prev(Some("P001"), Some("P002"), 45.0, "tensao", "gt3");
        let n = new_setup(Some("P001"), Some("P002"), "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::PartialPreserve
        );
    }

    #[test]
    fn test_full_reset_changed_pilot() {
        let p = prev(Some("P001"), Some("P002"), 45.0, "tensao", "gt3");
        let n = new_setup(Some("P001"), Some("P003"), "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::FullReset
        );
    }

    #[test]
    fn test_full_reset_direction_swap() {
        // Mesma dupla mas N1↔N2 trocados
        let p = prev(Some("P001"), Some("P002"), 45.0, "tensao", "gt3");
        let n = new_setup(Some("P002"), Some("P001"), "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::FullReset
        );
    }

    #[test]
    fn test_full_reset_different_category() {
        // Mesmos pilotos, mesma direção, mas categoria diferente (promoção/rebaixamento)
        let p = prev(Some("P001"), Some("P002"), 45.0, "tensao", "gt4");
        let n = new_setup(Some("P001"), Some("P002"), "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::FullReset
        );
    }

    #[test]
    fn test_full_reset_incomplete_new_lineup() {
        let p = prev(Some("P001"), Some("P002"), 45.0, "tensao", "gt3");
        let n = new_setup(None, Some("P002"), "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::FullReset
        );
    }

    #[test]
    fn test_full_reset_both_new_none() {
        let p = prev(Some("P001"), Some("P002"), 45.0, "tensao", "gt3");
        let n = new_setup(None, None, "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::FullReset
        );
    }

    #[test]
    fn test_full_reset_no_prev_hierarchy() {
        // Equipe sem hierarquia anterior (nova dupla, nunca tinha N1/N2)
        let p = prev(None, None, 0.0, "estavel", "gt3");
        let n = new_setup(Some("P001"), Some("P002"), "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::FullReset
        );
    }

    #[test]
    fn test_full_reset_prev_cat_empty_string() {
        // prev_categoria vazio (old save sem context) → FullReset (comportamento seguro)
        let p = prev(Some("P001"), Some("P002"), 60.0, "crise", "");
        let n = new_setup(Some("P001"), Some("P002"), "gt3");
        assert_eq!(
            decide_hierarchy_transition(&p, &n),
            HierarchyTransition::FullReset
        );
    }

    // ── resolve_transition_values ──

    #[test]
    fn test_resolve_partial_preserve_returns_prev_values() {
        let (tensao, status) =
            resolve_transition_values(&HierarchyTransition::PartialPreserve, 45.0, "tensao");
        assert!((tensao - 45.0).abs() < f64::EPSILON);
        assert_eq!(status, "tensao");
    }

    #[test]
    fn test_resolve_partial_preserve_normalizes_legacy_status() {
        // Status legado "n1" deve ser normalizado para "estavel"
        let (tensao, status) =
            resolve_transition_values(&HierarchyTransition::PartialPreserve, 10.0, "n1");
        assert!((tensao - 10.0).abs() < f64::EPSILON);
        assert_eq!(status, "estavel");
    }

    #[test]
    fn test_resolve_full_reset_ignores_prev_values() {
        let (tensao, status) =
            resolve_transition_values(&HierarchyTransition::FullReset, 60.0, "crise");
        assert!((tensao - 0.0).abs() < f64::EPSILON);
        assert_eq!(status, "estavel");
    }

    #[test]
    fn test_resolve_partial_preserve_high_tensao() {
        let (tensao, status) =
            resolve_transition_values(&HierarchyTransition::PartialPreserve, 88.5, "inversao");
        assert!((tensao - 88.5).abs() < f64::EPSILON);
        assert_eq!(status, "inversao");
    }

    // ── validate_and_normalize_team_hierarchies (integração DB) ──

    fn setup_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE teams (
                id TEXT PRIMARY KEY,
                ativa INTEGER DEFAULT 1,
                categoria TEXT NOT NULL,
                piloto_1_id TEXT,
                piloto_2_id TEXT,
                hierarquia_n1_id TEXT,
                hierarquia_n2_id TEXT,
                hierarquia_status TEXT DEFAULT 'estavel',
                hierarquia_tensao REAL DEFAULT 0.0,
                hierarquia_duelos_total INTEGER DEFAULT 0,
                hierarquia_duelos_n2_vencidos INTEGER DEFAULT 0,
                hierarquia_sequencia_n2 INTEGER DEFAULT 0,
                hierarquia_sequencia_n1 INTEGER DEFAULT 0,
                hierarquia_inversoes_temporada INTEGER DEFAULT 0
            );",
        )
        .unwrap();
        conn
    }

    fn insert_team(
        conn: &rusqlite::Connection,
        id: &str,
        cat: &str,
        p1: Option<&str>,
        p2: Option<&str>,
        n1: Option<&str>,
        n2: Option<&str>,
        tensao: f64,
        status: &str,
    ) {
        conn.execute(
            "INSERT INTO teams (id, categoria, piloto_1_id, piloto_2_id, hierarquia_n1_id, \
             hierarquia_n2_id, hierarquia_tensao, hierarquia_status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, cat, p1, p2, n1, n2, tensao, status],
        )
        .unwrap();
    }

    fn read_hierarchy(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> (Option<String>, Option<String>, f64, String, i32) {
        conn.query_row(
            "SELECT hierarquia_n1_id, hierarquia_n2_id, hierarquia_tensao, \
             hierarquia_status, hierarquia_duelos_total FROM teams WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap()
    }

    #[test]
    fn test_normalize_skips_aligned_team() {
        let conn = setup_test_db();
        insert_team(
            &conn,
            "T001",
            "gt3",
            Some("P001"),
            Some("P002"),
            Some("P001"),
            Some("P002"),
            55.0,
            "tensao",
        );

        validate_and_normalize_team_hierarchies(&conn).unwrap();

        // Tensao preservada — alinhado, não foi tocado
        let (n1, n2, tensao, status, _) = read_hierarchy(&conn, "T001");
        assert_eq!(n1.as_deref(), Some("P001"));
        assert_eq!(n2.as_deref(), Some("P002"));
        assert!((tensao - 55.0).abs() < f64::EPSILON);
        assert_eq!(status, "tensao");
    }

    #[test]
    fn test_normalize_fixes_misaligned_team() {
        // N1/N2 desalinhados com o lineup (equipe preenchida por fallback)
        let conn = setup_test_db();
        insert_team(
            &conn,
            "T001",
            "gt3",
            Some("P001"),
            Some("P002"),
            None,
            None,
            0.0,
            "estavel",
        );

        validate_and_normalize_team_hierarchies(&conn).unwrap();

        let (n1, n2, tensao, status, duelos) = read_hierarchy(&conn, "T001");
        assert_eq!(n1.as_deref(), Some("P001"));
        assert_eq!(n2.as_deref(), Some("P002"));
        assert!((tensao - 0.0).abs() < f64::EPSILON);
        assert_eq!(status, "estavel");
        assert_eq!(duelos, 0);
    }

    #[test]
    fn test_normalize_fails_on_incomplete_lineup() {
        let conn = setup_test_db();
        insert_team(
            &conn,
            "T001",
            "gt3",
            Some("P001"),
            None,
            None,
            None,
            0.0,
            "estavel",
        );

        let result = validate_and_normalize_team_hierarchies(&conn);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("lineup incompleto"));
    }

    #[test]
    fn test_normalize_fails_on_duplicate_pilot_lineup() {
        let conn = setup_test_db();
        insert_team(
            &conn,
            "T001",
            "gt3",
            Some("P001"),
            Some("P001"),
            None,
            None,
            0.0,
            "estavel",
        );

        let result = validate_and_normalize_team_hierarchies(&conn);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mesmo piloto"));
    }

    #[test]
    fn test_normalize_skips_special_categories() {
        let conn = setup_test_db();
        // Equipe especial com lineup incompleto — deve ser ignorada (não retorna erro)
        insert_team(
            &conn,
            "T001",
            "production_challenger",
            Some("P001"),
            None,
            None,
            None,
            0.0,
            "estavel",
        );
        insert_team(
            &conn,
            "T002",
            "gt3",
            Some("P001"),
            Some("P002"),
            Some("P001"),
            Some("P002"),
            0.0,
            "estavel",
        );

        validate_and_normalize_team_hierarchies(&conn).unwrap();
    }

    #[test]
    fn test_normalize_rolls_back_when_counter_reset_fails() {
        let conn = setup_test_db();
        insert_team(
            &conn,
            "T001",
            "gt3",
            Some("P001"),
            Some("P002"),
            None,
            None,
            77.0,
            "crise",
        );
        conn.execute(
            "UPDATE teams SET hierarquia_duelos_total = 5 WHERE id = 'T001'",
            [],
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TRIGGER fail_duel_counter_reset
             BEFORE UPDATE OF hierarquia_duelos_total ON teams
             FOR EACH ROW
             WHEN NEW.id = 'T001'
             BEGIN
                 SELECT RAISE(FAIL, 'boom duel reset');
             END;",
        )
        .unwrap();

        let result = validate_and_normalize_team_hierarchies(&conn);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("resetar contadores"));

        let (n1, n2, tensao, status, duelos) = read_hierarchy(&conn, "T001");
        assert_eq!(n1, None);
        assert_eq!(n2, None);
        assert!((tensao - 77.0).abs() < f64::EPSILON);
        assert_eq!(status, "crise");
        assert_eq!(duelos, 5);
    }

    // ── ResolvedTeamLineup ──

    #[test]
    fn test_resolved_lineup_valid() {
        let lineup = ResolvedTeamLineup::new("T001", Some("P001"), Some("P002")).unwrap();
        assert_eq!(lineup.team_id, "T001");
        assert_eq!(lineup.n1_id, "P001");
        assert_eq!(lineup.n2_id, "P002");
    }

    #[test]
    fn test_resolved_lineup_n1_absent() {
        let err = ResolvedTeamLineup::new("T001", None, Some("P002")).unwrap_err();
        assert!(err.contains("N1 ausente"), "erro inesperado: {err}");
    }

    #[test]
    fn test_resolved_lineup_n2_absent() {
        let err = ResolvedTeamLineup::new("T001", Some("P001"), None).unwrap_err();
        assert!(err.contains("N2 ausente"), "erro inesperado: {err}");
    }

    #[test]
    fn test_resolved_lineup_same_pilot() {
        let err = ResolvedTeamLineup::new("T001", Some("P001"), Some("P001")).unwrap_err();
        assert!(err.contains("mesmo piloto"), "erro inesperado: {err}");
    }

    #[test]
    fn test_normalize_does_not_touch_rivalry_table() {
        // As funções de transição operam apenas na tabela teams — garantia estrutural.
        // Este teste verifica que a função NÃO acessa a tabela rivalries
        // (se tentasse, falharia porque a tabela não existe neste DB de teste).
        let conn = setup_test_db();
        insert_team(
            &conn,
            "T001",
            "gt3",
            Some("P001"),
            Some("P002"),
            Some("P001"),
            Some("P002"),
            0.0,
            "estavel",
        );

        // Se validate_and_normalize_team_hierarchies tentasse acessar rivalries,
        // o DB sem a tabela causaria erro. Deve passar sem erros.
        validate_and_normalize_team_hierarchies(&conn).unwrap();
    }
}
