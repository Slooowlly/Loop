//! Sistema Interno de Equipe — Passos 2 a 10
//!
//! Camada A: estado persistido (Team struct + DB) — vive em models/team.rs
//! Camada B: cálculo pós-corrida — vive aqui

#![allow(dead_code)]

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::models::driver::Driver;
use crate::models::team::{Team, TeamHierarchyClimate};
use crate::simulation::race::RaceDriverResult;

// ── Tipos do confronto (Passo 4) ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DuelWinner {
    N1,
    N2,
}

/// Resumo do confronto interno de uma equipe em uma rodada.
/// É o insumo para `apply_duel_counters`.
#[derive(Debug, Clone)]
pub struct DuelResult {
    pub team_id: String,
    pub n1_id: String,
    pub n2_id: String,
    /// Passo 3: houve duelo interno válido nesta rodada?
    pub valid: bool,
    /// Some(DuelWinner) se duelo válido; None caso contrário.
    pub vencedor: Option<DuelWinner>,
}

// ── Passo 3 — Regra de duelo válido ───────────────────────────────────────────

/// Retorna `true` apenas quando os dois pilotos aparecem no resultado.
///
/// Casos que NÃO contam: piloto lesionado ou ausente (None).
/// Casos que CONTAM: ambos largaram, mesmo que um tenha abandonado.
pub fn is_duel_valid(n1_result: Option<i32>, n2_result: Option<i32>) -> bool {
    n1_result.is_some() && n2_result.is_some()
}

// ── Passo 4 — Leitura do confronto interno ────────────────────────────────────

/// Lê o confronto interno da equipe a partir do resultado bruto da corrida.
///
/// `race_results`: mapa de `driver_id → posição de chegada`.
/// Retorna `None` se a equipe não tem N1 e N2 definidos.
pub fn read_team_duel(team: &Team, race_results: &HashMap<String, i32>) -> Option<DuelResult> {
    let n1_id = team.hierarquia_n1_id.as_deref()?;
    let n2_id = team.hierarquia_n2_id.as_deref()?;

    let n1_pos = race_results.get(n1_id).copied();
    let n2_pos = race_results.get(n2_id).copied();
    let valid = is_duel_valid(n1_pos, n2_pos);

    let vencedor = if valid {
        // Posição menor = melhor resultado
        match (n1_pos, n2_pos) {
            (Some(p1), Some(p2)) if p2 < p1 => Some(DuelWinner::N2),
            _ => Some(DuelWinner::N1),
        }
    } else {
        None
    };

    Some(DuelResult {
        team_id: team.id.clone(),
        n1_id: n1_id.to_string(),
        n2_id: n2_id.to_string(),
        valid,
        vencedor,
    })
}

// ── Passo 5 — Atualização dos contadores básicos ──────────────────────────────

/// Atualiza os contadores de disputa interna da equipe com base no duelo da rodada.
///
/// Não altera tensão, status nem inversões — esses ficam para os próximos passos.
pub fn apply_duel_counters(team: &mut Team, duel: &DuelResult) {
    if !duel.valid {
        return;
    }

    team.hierarquia_duelos_total += 1;

    match &duel.vencedor {
        Some(DuelWinner::N2) => {
            team.hierarquia_duelos_n2_vencidos += 1;
            team.hierarquia_sequencia_n2 += 1;
            team.hierarquia_sequencia_n1 = 0;
        }
        Some(DuelWinner::N1) => {
            team.hierarquia_sequencia_n1 += 1;
            team.hierarquia_sequencia_n2 = 0;
        }
        None => {}
    }
}

// ── Passo 6 — Percentual de duelos vencidos pelo N2 ──────────────────────────

/// Retorna a fração de duelos vencidos pelo N2 (0.0 a 1.0).
/// Retorna 0.0 se ainda não houve duelos válidos.
pub fn n2_win_rate(duelos_total: i32, duelos_n2_vencidos: i32) -> f64 {
    if duelos_total == 0 {
        return 0.0;
    }
    duelos_n2_vencidos as f64 / duelos_total as f64
}

// ── Passo 7 — Atualizar tensão com base no resultado da corrida ───────────────

/// Atualiza `hierarquia_tensao` da equipe com base no duelo da rodada.
///
/// Deve ser chamada APÓS `apply_duel_counters`, pois usa as sequências já atualizadas.
///
/// Regras base (aplicadas antes do clamp):
/// - N2 venceu o duelo: +3
/// - N1 venceu o duelo: -2
/// - Decaimento natural por corrida: -1 (sempre)
/// - Sem duelo válido: apenas -1
///
/// Bônus de sequência (aplicados só quando o threshold é atingido exatamente):
/// - N2 com 3 seguidas: +10
/// - N2 com 5 seguidas: +15
/// - N1 com 3 seguidas: -8
///
/// Resultado final é clampado em [0, 100].
pub fn update_tensao(team: &mut Team, duel: &DuelResult) {
    let mut delta: f64 = -1.0; // decaimento natural sempre presente

    if duel.valid {
        match &duel.vencedor {
            Some(DuelWinner::N2) => delta += 3.0,
            Some(DuelWinner::N1) => delta -= 2.0,
            None => {}
        }
    }

    // Bônus de sequência — apenas quando o limiar é atingido exatamente
    if team.hierarquia_sequencia_n2 == 3 {
        delta += 10.0;
    } else if team.hierarquia_sequencia_n2 == 5 {
        delta += 15.0;
    }

    if team.hierarquia_sequencia_n1 == 3 {
        delta -= 8.0;
    }

    team.hierarquia_tensao = (team.hierarquia_tensao + delta).clamp(0.0, 100.0);
}

// ── Passo 8 — Converter tensão em status ─────────────────────────────────────

/// Deriva o status semântico da equipe a partir da tensão acumulada e persiste no campo.
///
/// Deve ser chamada após `update_tensao`.
pub fn update_status(team: &mut Team) {
    team.hierarquia_status = TeamHierarchyClimate::from_tensao(team.hierarquia_tensao)
        .as_str()
        .to_string();
}

// ── Passo 9 — Detectar reavaliação real ──────────────────────────────────────

/// Retorna `true` se a equipe entrou em reavaliação real da hierarquia.
///
/// Condições:
/// - Pelo menos 5 duelos disputados
/// - N2 está em sequência de ≥ 4 vitórias consecutivas
/// - N2 venceu ≥ 60% dos duelos totais
/// - Status atual é reavaliação, inversão ou crise
pub fn is_em_reavaliacao(team: &Team) -> bool {
    let win_rate = n2_win_rate(
        team.hierarquia_duelos_total,
        team.hierarquia_duelos_n2_vencidos,
    );
    let status = TeamHierarchyClimate::from_str(&team.hierarquia_status);
    let status_critico = matches!(
        status,
        TeamHierarchyClimate::Reavaliacao
            | TeamHierarchyClimate::Inversao
            | TeamHierarchyClimate::Crise
    );

    team.hierarquia_duelos_total >= 5
        && team.hierarquia_sequencia_n2 >= 4
        && win_rate >= 0.60
        && status_critico
}

// ── Passo 10 — Detectar gatilho de inversão ──────────────────────────────────

/// Retorna `true` quando todas as condições para trocar N1/N2 estão satisfeitas.
///
/// Esse passo apenas detecta — não executa a inversão.
///
/// Condições:
/// - Pelo menos 8 duelos disputados
/// - N2 em sequência de ≥ 5 vitórias consecutivas
/// - Temporada passou da metade (`rodada_atual * 2 > total_rounds`)
/// - N2 venceu ≥ 65% dos duelos totais
/// - Status atual é crise
pub fn has_inversao_trigger(team: &Team, rodada_atual: i32, total_rounds: i32) -> bool {
    let win_rate = n2_win_rate(
        team.hierarquia_duelos_total,
        team.hierarquia_duelos_n2_vencidos,
    );
    let status = TeamHierarchyClimate::from_str(&team.hierarquia_status);
    let past_halfway = rodada_atual * 2 > total_rounds;

    team.hierarquia_duelos_total >= 8
        && team.hierarquia_sequencia_n2 >= 5
        && past_halfway
        && win_rate >= 0.65
        && status == TeamHierarchyClimate::Crise
}

// ── Passo 11 — Executar a inversão ───────────────────────────────────────────

/// Executa a troca de N1/N2 quando `has_inversao_trigger` retorna `true`.
///
/// Efeitos:
/// - Troca `hierarquia_n1_id` e `hierarquia_n2_id`
/// - Incrementa `hierarquia_inversoes_temporada`
/// - Reduz tensão em 30 (clamp 0..100) e recalcula status
/// - Zera sequências e contadores de duelo (o contexto anterior não é mais válido)
pub fn apply_inversao(team: &mut Team) {
    std::mem::swap(&mut team.hierarquia_n1_id, &mut team.hierarquia_n2_id);
    team.hierarquia_inversoes_temporada += 1;
    team.hierarquia_tensao = (team.hierarquia_tensao - 30.0).clamp(0.0, 100.0);
    update_status(team);
    team.hierarquia_duelos_total = 0;
    team.hierarquia_duelos_n2_vencidos = 0;
    team.hierarquia_sequencia_n2 = 0;
    team.hierarquia_sequencia_n1 = 0;
}

// ── Passo 12 — Efeitos da inversão nos pilotos ───────────────────────────────

/// Aplica os efeitos motivacionais da inversão de hierarquia.
///
/// - Piloto promovido a N1: +15 de motivação
/// - Piloto rebaixado a N2: -10 de motivação
/// Ambos clampados em [0, 100].
pub fn apply_inversao_driver_effects(promoted: &mut Driver, demoted: &mut Driver) {
    promoted.motivacao = (promoted.motivacao + 15.0).clamp(0.0, 100.0);
    demoted.motivacao = (demoted.motivacao - 10.0).clamp(0.0, 100.0);
}

// ── Passo 14 — Pipeline pós-corrida de hierarquia ────────────────────────────

/// Processa o sistema interno de hierarquia para todas as equipes de uma categoria.
///
/// Deve ser chamado após o resultado da corrida estar consolidado no banco.
/// Cobre os Passos 3 a 13 (e o tratamento de Passo 15 — ausências/lesões).
///
/// `race_results`: resultados brutos da corrida (driver_id → posição)
/// `rodada_atual`: rodada que acabou de ser disputada
/// `total_rounds`: total de corridas da temporada nessa categoria
pub fn process_hierarchy_for_category(
    conn: &Connection,
    race_results: &[RaceDriverResult],
    category_id: &str,
    rodada_atual: i32,
    total_rounds: i32,
    temporada: i32,
) -> Result<(), DbError> {
    let teams = team_queries::get_teams_by_category(conn, category_id)?;

    // Monta mapa driver_id → posição de chegada
    let result_map: HashMap<String, i32> = race_results
        .iter()
        .map(|r| (r.pilot_id.clone(), r.finish_position))
        .collect();

    for mut team in teams {
        // Passo 15 — equipe sem hierarquia definida: só decaimento natural
        let Some(duel) = read_team_duel(&team, &result_map) else {
            team.hierarquia_tensao = (team.hierarquia_tensao - 1.0).clamp(0.0, 100.0);
            update_status(&mut team);
            team_queries::update_team_hierarchy_full(conn, &team)?;
            continue;
        };

        // Captura status antes desta rodada para detectar transições (Passo 6)
        let status_antes = team.hierarquia_status.clone();

        // Passos 5, 7, 8 — contadores, tensão e status
        apply_duel_counters(&mut team, &duel);
        update_tensao(&mut team, &duel);
        update_status(&mut team);

        let status_pos_tensao = team.hierarquia_status.clone();
        let mut inversao_ocorreu = false;

        // Passos 10 + 11 — detecta e executa inversão
        if has_inversao_trigger(&team, rodada_atual, total_rounds) {
            // Salva os IDs antes de inverter (o antigo N2 será promovido)
            let old_n2_id = team.hierarquia_n2_id.clone();
            let old_n1_id = team.hierarquia_n1_id.clone();

            apply_inversao(&mut team); // Passo 11
            inversao_ocorreu = true;

            // Passo 12 — efeitos motivacionais nos pilotos envolvidos
            if let (Some(promoted_id), Some(demoted_id)) = (old_n2_id, old_n1_id) {
                if let (Ok(mut promoted), Ok(mut demoted)) = (
                    driver_queries::get_driver(conn, &promoted_id),
                    driver_queries::get_driver(conn, &demoted_id),
                ) {
                    apply_inversao_driver_effects(&mut promoted, &mut demoted);
                    driver_queries::update_driver_motivation(
                        conn,
                        &promoted.id,
                        promoted.motivacao,
                    )?;
                    driver_queries::update_driver_motivation(conn, &demoted.id, demoted.motivacao)?;
                }
            }
        }

        // Passo 6 — rivalidade por hierarquia
        if let (Some(n1_id), Some(n2_id)) = (&team.hierarquia_n1_id, &team.hierarquia_n2_id) {
            crate::rivalry::process_hierarchy_rivalry(
                conn,
                n1_id,
                n2_id,
                &status_antes,
                &status_pos_tensao,
                inversao_ocorreu,
                category_id,
                &team.id,
                rodada_atual,
                temporada,
            )?;
        }

        // Passo 13 — persiste o estado completo da hierarquia
        team_queries::update_team_hierarchy_full(conn, &team)?;
    }

    Ok(())
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::teams::get_team_templates;
    use crate::models::team::Team;
    use rand::{rngs::StdRng, SeedableRng};

    fn sample_team(n1: &str, n2: &str) -> Team {
        let template = get_team_templates("gt3")[0];
        let mut rng = StdRng::seed_from_u64(99);
        let mut team =
            Team::from_template_with_rng(template, "gt3", "T001".to_string(), 2026, &mut rng);
        team.hierarquia_n1_id = Some(n1.to_string());
        team.hierarquia_n2_id = Some(n2.to_string());
        team
    }

    // Passo 3

    #[test]
    fn test_is_duel_valid_both_raced() {
        assert!(is_duel_valid(Some(3), Some(7)));
    }

    #[test]
    fn test_is_duel_valid_dnf_counts() {
        // DNF representado como posição alta; ainda válido pois ambos largaram
        assert!(is_duel_valid(Some(1), Some(30)));
    }

    #[test]
    fn test_is_duel_invalid_n2_absent() {
        assert!(!is_duel_valid(Some(5), None));
    }

    #[test]
    fn test_is_duel_invalid_both_absent() {
        assert!(!is_duel_valid(None, None));
    }

    // Passo 4

    #[test]
    fn test_read_team_duel_n2_wins() {
        let team = sample_team("P001", "P002");
        let mut results = HashMap::new();
        results.insert("P001".to_string(), 8);
        results.insert("P002".to_string(), 5);

        let duel = read_team_duel(&team, &results).unwrap();
        assert!(duel.valid);
        assert_eq!(duel.vencedor, Some(DuelWinner::N2));
    }

    #[test]
    fn test_read_team_duel_n1_wins() {
        let team = sample_team("P001", "P002");
        let mut results = HashMap::new();
        results.insert("P001".to_string(), 3);
        results.insert("P002".to_string(), 7);

        let duel = read_team_duel(&team, &results).unwrap();
        assert!(duel.valid);
        assert_eq!(duel.vencedor, Some(DuelWinner::N1));
    }

    #[test]
    fn test_read_team_duel_n2_absent_invalid() {
        let team = sample_team("P001", "P002");
        let mut results = HashMap::new();
        results.insert("P001".to_string(), 5);
        // P002 não aparece (lesionado, ausente)

        let duel = read_team_duel(&team, &results).unwrap();
        assert!(!duel.valid);
        assert_eq!(duel.vencedor, None);
    }

    #[test]
    fn test_read_team_duel_no_hierarchy_returns_none() {
        let template = get_team_templates("gt3")[0];
        let mut rng = StdRng::seed_from_u64(99);
        let team =
            Team::from_template_with_rng(template, "gt3", "T001".to_string(), 2026, &mut rng);
        // n1_id e n2_id são None por padrão
        let results = HashMap::new();
        assert!(read_team_duel(&team, &results).is_none());
    }

    // Passo 5

    #[test]
    fn test_apply_duel_counters_n2_wins() {
        let mut team = sample_team("P001", "P002");
        let duel = DuelResult {
            team_id: "T001".to_string(),
            n1_id: "P001".to_string(),
            n2_id: "P002".to_string(),
            valid: true,
            vencedor: Some(DuelWinner::N2),
        };

        apply_duel_counters(&mut team, &duel);

        assert_eq!(team.hierarquia_duelos_total, 1);
        assert_eq!(team.hierarquia_duelos_n2_vencidos, 1);
        assert_eq!(team.hierarquia_sequencia_n2, 1);
        assert_eq!(team.hierarquia_sequencia_n1, 0);
    }

    #[test]
    fn test_apply_duel_counters_n1_wins_resets_n2_sequence() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_sequencia_n2 = 3;

        let duel = DuelResult {
            team_id: "T001".to_string(),
            n1_id: "P001".to_string(),
            n2_id: "P002".to_string(),
            valid: true,
            vencedor: Some(DuelWinner::N1),
        };

        apply_duel_counters(&mut team, &duel);

        assert_eq!(team.hierarquia_duelos_total, 1);
        assert_eq!(team.hierarquia_duelos_n2_vencidos, 0);
        assert_eq!(team.hierarquia_sequencia_n1, 1);
        assert_eq!(team.hierarquia_sequencia_n2, 0);
    }

    #[test]
    fn test_apply_duel_counters_invalid_duel_changes_nothing() {
        let mut team = sample_team("P001", "P002");
        let duel = DuelResult {
            team_id: "T001".to_string(),
            n1_id: "P001".to_string(),
            n2_id: "P002".to_string(),
            valid: false,
            vencedor: None,
        };

        apply_duel_counters(&mut team, &duel);

        assert_eq!(team.hierarquia_duelos_total, 0);
        assert_eq!(team.hierarquia_duelos_n2_vencidos, 0);
        assert_eq!(team.hierarquia_sequencia_n2, 0);
        assert_eq!(team.hierarquia_sequencia_n1, 0);
    }

    #[test]
    fn test_apply_duel_counters_sequence_accumulates() {
        let mut team = sample_team("P001", "P002");

        for _ in 0..3 {
            let duel = DuelResult {
                team_id: "T001".to_string(),
                n1_id: "P001".to_string(),
                n2_id: "P002".to_string(),
                valid: true,
                vencedor: Some(DuelWinner::N2),
            };
            apply_duel_counters(&mut team, &duel);
        }

        assert_eq!(team.hierarquia_duelos_total, 3);
        assert_eq!(team.hierarquia_duelos_n2_vencidos, 3);
        assert_eq!(team.hierarquia_sequencia_n2, 3);
        assert_eq!(team.hierarquia_sequencia_n1, 0);
    }

    // ── Passo 11 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_apply_inversao_swaps_ids() {
        let mut team = sample_team("P001", "P002");
        apply_inversao(&mut team);
        assert_eq!(team.hierarquia_n1_id.as_deref(), Some("P002"));
        assert_eq!(team.hierarquia_n2_id.as_deref(), Some("P001"));
    }

    #[test]
    fn test_apply_inversao_increments_counter() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_inversoes_temporada = 1;
        apply_inversao(&mut team);
        assert_eq!(team.hierarquia_inversoes_temporada, 2);
    }

    #[test]
    fn test_apply_inversao_reduces_tensao() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 95.0;
        apply_inversao(&mut team);
        assert_eq!(team.hierarquia_tensao, 65.0);
    }

    #[test]
    fn test_apply_inversao_tensao_clamp_floor() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 10.0;
        apply_inversao(&mut team);
        assert_eq!(team.hierarquia_tensao, 0.0);
    }

    #[test]
    fn test_apply_inversao_resets_sequences() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_sequencia_n2 = 5;
        team.hierarquia_sequencia_n1 = 2;
        apply_inversao(&mut team);
        assert_eq!(team.hierarquia_sequencia_n2, 0);
        assert_eq!(team.hierarquia_sequencia_n1, 0);
    }

    #[test]
    fn test_apply_inversao_resets_duel_counters() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_duelos_total = 10;
        team.hierarquia_duelos_n2_vencidos = 7;
        apply_inversao(&mut team);
        assert_eq!(team.hierarquia_duelos_total, 0);
        assert_eq!(team.hierarquia_duelos_n2_vencidos, 0);
    }

    #[test]
    fn test_apply_inversao_recalculates_status() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 95.0; // crise
        apply_inversao(&mut team); // 95 - 30 = 65 → reavaliacao
        assert_eq!(team.hierarquia_status, "reavaliacao");
    }

    // ── Passo 12 ──────────────────────────────────────────────────────────────

    fn sample_driver_with_motivation(id: &str, motivacao: f64) -> Driver {
        let mut d = Driver::new(
            id.to_string(),
            id.to_string(),
            "BR".to_string(),
            "M".to_string(),
            25,
            2020,
        );
        d.motivacao = motivacao;
        d
    }

    #[test]
    fn test_apply_inversao_driver_effects_promoted_gains() {
        let mut promoted = sample_driver_with_motivation("P001", 60.0);
        let mut demoted = sample_driver_with_motivation("P002", 70.0);
        apply_inversao_driver_effects(&mut promoted, &mut demoted);
        assert_eq!(promoted.motivacao, 75.0);
        assert_eq!(demoted.motivacao, 60.0);
    }

    #[test]
    fn test_apply_inversao_driver_effects_clamp_ceiling() {
        let mut promoted = sample_driver_with_motivation("P001", 95.0);
        let mut demoted = sample_driver_with_motivation("P002", 50.0);
        apply_inversao_driver_effects(&mut promoted, &mut demoted);
        assert_eq!(promoted.motivacao, 100.0);
    }

    #[test]
    fn test_apply_inversao_driver_effects_clamp_floor() {
        let mut promoted = sample_driver_with_motivation("P001", 50.0);
        let mut demoted = sample_driver_with_motivation("P002", 5.0);
        apply_inversao_driver_effects(&mut promoted, &mut demoted);
        assert_eq!(demoted.motivacao, 0.0);
    }

    // ── Passo 6 ───────────────────────────────────────────────────────────────

    #[test]
    fn test_n2_win_rate_zero_duelos() {
        assert_eq!(n2_win_rate(0, 0), 0.0);
    }

    #[test]
    fn test_n2_win_rate_half() {
        assert_eq!(n2_win_rate(10, 5), 0.5);
    }

    #[test]
    fn test_n2_win_rate_all() {
        assert_eq!(n2_win_rate(4, 4), 1.0);
    }

    #[test]
    fn test_n2_win_rate_none() {
        assert_eq!(n2_win_rate(6, 0), 0.0);
    }

    // ── Passo 7 ───────────────────────────────────────────────────────────────

    fn duel(winner: DuelWinner) -> DuelResult {
        DuelResult {
            team_id: "T001".to_string(),
            n1_id: "P001".to_string(),
            n2_id: "P002".to_string(),
            valid: true,
            vencedor: Some(winner),
        }
    }

    fn duel_invalid() -> DuelResult {
        DuelResult {
            team_id: "T001".to_string(),
            n1_id: "P001".to_string(),
            n2_id: "P002".to_string(),
            valid: false,
            vencedor: None,
        }
    }

    #[test]
    fn test_update_tensao_n2_wins_increases() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 20.0;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3 - 1 = +2
        assert_eq!(team.hierarquia_tensao, 22.0);
    }

    #[test]
    fn test_update_tensao_n1_wins_decreases() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 20.0;
        update_tensao(&mut team, &duel(DuelWinner::N1));
        // -2 - 1 = -3
        assert_eq!(team.hierarquia_tensao, 17.0);
    }

    #[test]
    fn test_update_tensao_no_duel_only_decay() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 20.0;
        update_tensao(&mut team, &duel_invalid());
        assert_eq!(team.hierarquia_tensao, 19.0);
    }

    #[test]
    fn test_update_tensao_clamp_floor() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 0.5;
        update_tensao(&mut team, &duel_invalid());
        assert_eq!(team.hierarquia_tensao, 0.0);
    }

    #[test]
    fn test_update_tensao_clamp_ceiling() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 99.0;
        team.hierarquia_sequencia_n2 = 3;
        // +3 - 1 + 10 = +12 → 111 → clampado em 100
        update_tensao(&mut team, &duel(DuelWinner::N2));
        assert_eq!(team.hierarquia_tensao, 100.0);
    }

    #[test]
    fn test_update_tensao_bonus_seq_n2_3() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 30.0;
        team.hierarquia_sequencia_n2 = 3;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3 - 1 + 10 = +12
        assert_eq!(team.hierarquia_tensao, 42.0);
    }

    #[test]
    fn test_update_tensao_bonus_seq_n2_5() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 30.0;
        team.hierarquia_sequencia_n2 = 5;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3 - 1 + 15 = +17
        assert_eq!(team.hierarquia_tensao, 47.0);
    }

    #[test]
    fn test_update_tensao_bonus_seq_n1_3() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 40.0;
        team.hierarquia_sequencia_n1 = 3;
        update_tensao(&mut team, &duel(DuelWinner::N1));
        // -2 - 1 - 8 = -11
        assert_eq!(team.hierarquia_tensao, 29.0);
    }

    #[test]
    fn test_update_tensao_no_bonus_on_seq_4() {
        // A sequência 4 não aplica bônus extra — só 3 e 5
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 30.0;
        team.hierarquia_sequencia_n2 = 4;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3 - 1 = +2 (sem bônus)
        assert_eq!(team.hierarquia_tensao, 32.0);
    }

    // ── Passo 8 ───────────────────────────────────────────────────────────────

    #[test]
    fn test_update_status_derives_from_tensao() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 50.0; // → tensao
        update_status(&mut team);
        assert_eq!(team.hierarquia_status, "tensao");
    }

    #[test]
    fn test_update_status_estavel() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 10.0;
        update_status(&mut team);
        assert_eq!(team.hierarquia_status, "estavel");
    }

    #[test]
    fn test_update_status_crise() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 95.0;
        update_status(&mut team);
        assert_eq!(team.hierarquia_status, "crise");
    }

    // ── Passo 9 ───────────────────────────────────────────────────────────────

    fn team_em_reavaliacao() -> Team {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_duelos_total = 8;
        team.hierarquia_duelos_n2_vencidos = 6; // 75%
        team.hierarquia_sequencia_n2 = 4;
        team.hierarquia_tensao = 65.0;
        team.hierarquia_status = "reavaliacao".to_string();
        team
    }

    #[test]
    fn test_is_em_reavaliacao_all_conditions_met() {
        assert!(is_em_reavaliacao(&team_em_reavaliacao()));
    }

    #[test]
    fn test_is_em_reavaliacao_poucos_duelos() {
        let mut team = team_em_reavaliacao();
        team.hierarquia_duelos_total = 4;
        assert!(!is_em_reavaliacao(&team));
    }

    #[test]
    fn test_is_em_reavaliacao_sequencia_curta() {
        let mut team = team_em_reavaliacao();
        team.hierarquia_sequencia_n2 = 3;
        assert!(!is_em_reavaliacao(&team));
    }

    #[test]
    fn test_is_em_reavaliacao_win_rate_baixo() {
        let mut team = team_em_reavaliacao();
        team.hierarquia_duelos_n2_vencidos = 4; // 50%
        assert!(!is_em_reavaliacao(&team));
    }

    #[test]
    fn test_is_em_reavaliacao_status_baixo() {
        let mut team = team_em_reavaliacao();
        team.hierarquia_tensao = 30.0;
        team.hierarquia_status = "competitivo".to_string();
        assert!(!is_em_reavaliacao(&team));
    }

    // ── Passo 10 ──────────────────────────────────────────────────────────────

    fn team_gatilho_inversao() -> Team {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_duelos_total = 10;
        team.hierarquia_duelos_n2_vencidos = 7; // 70%
        team.hierarquia_sequencia_n2 = 5;
        team.hierarquia_tensao = 95.0;
        team.hierarquia_status = "crise".to_string();
        team
    }

    #[test]
    fn test_has_inversao_trigger_all_conditions() {
        // rodada 9 de 14 → passou da metade
        assert!(has_inversao_trigger(&team_gatilho_inversao(), 9, 14));
    }

    #[test]
    fn test_has_inversao_trigger_antes_da_metade() {
        // rodada 5 de 14 → não passou da metade
        assert!(!has_inversao_trigger(&team_gatilho_inversao(), 5, 14));
    }

    #[test]
    fn test_has_inversao_trigger_poucos_duelos() {
        let mut team = team_gatilho_inversao();
        team.hierarquia_duelos_total = 7;
        assert!(!has_inversao_trigger(&team, 9, 14));
    }

    #[test]
    fn test_has_inversao_trigger_sequencia_insuficiente() {
        let mut team = team_gatilho_inversao();
        team.hierarquia_sequencia_n2 = 4;
        assert!(!has_inversao_trigger(&team, 9, 14));
    }

    #[test]
    fn test_has_inversao_trigger_win_rate_baixo() {
        let mut team = team_gatilho_inversao();
        team.hierarquia_duelos_n2_vencidos = 6; // 60%
        assert!(!has_inversao_trigger(&team, 9, 14));
    }

    #[test]
    fn test_has_inversao_trigger_status_nao_crise() {
        let mut team = team_gatilho_inversao();
        team.hierarquia_tensao = 80.0;
        team.hierarquia_status = "inversao".to_string();
        assert!(!has_inversao_trigger(&team, 9, 14));
    }
}
