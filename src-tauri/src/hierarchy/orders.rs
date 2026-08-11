//! Sistema Interno de Equipe — Passos 2 a 10
//!
//! Camada A: estado persistido (Team struct + DB) — vive em models/team.rs
//! Camada B: cálculo pós-corrida — vive aqui

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::connection::DbError;
use crate::db::queries::contracts as contract_queries;
use crate::db::queries::drivers as driver_queries;
use crate::db::queries::teams as team_queries;
use crate::models::driver::Driver;
use crate::models::enums::TeamRole;
use crate::models::team::{Team, TeamHierarchyClimate};
use crate::simulation::race::RaceDriverResult;

// O núcleo puro do Passo 7 (os deltas e o desfecho do duelo) mora em `tensao.rs`,
// sem dependência nenhuma, para poder ser medido isolado — ver a nota de lá.
pub use super::tensao::{
    delta_da_rodada, DuelResult, DuelWinner, TENSAO_BONUS_SEQ_N2_3, TENSAO_BONUS_SEQ_N2_5,
    TENSAO_DECAIMENTO_SEM_DUELO, TENSAO_DELTAS, TENSAO_DELTA_N1_VENCE, TENSAO_DELTA_N2_VENCE,
    TENSAO_PENALIDADE_SEQ_N1_3,
};

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

    Some(DuelResult { valid, vencedor })
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
/// Os valores são os de [`TENSAO_DELTAS`]; o cálculo é [`delta_da_rodada`].
/// Resultado final é clampado em [0, 100].
///
/// O DECAIMENTO SAIU DAS CORRIDAS DISPUTADAS, e essa é a diferença entre este
/// eixo medir disputa ou medir o tempo passando.
///
/// Antes o -1 incidia sempre, inclusive quando o N2 vencia. Com o N2 ganhando
/// uma fração `p` dos duelos, a variação esperada por corrida era `5p - 3`: zero
/// em p = 0,6, negativa abaixo disso. Ou seja, o segundo piloto precisava bater
/// o primeiro em MAIS DE 60% das corridas só para a tensão não cair — e como ela
/// tem piso em zero, todo par equilibrado morava no chão. Num save de 27
/// temporadas isso deu 101 de 102 equipes em "estável" com tensão média 0,1, a
/// maior sequência do N2 do mundo inteiro em 3, e nenhuma rivalidade de
/// companheiros jamais criada — o gatilho existia e nunca disparava.
///
/// O decaimento continua existindo para a corrida em que não houve duelo (lesão,
/// ausência), que é onde ele de fato representa uma disputa que esfriou por falta
/// de disputa.
///
/// A SEGUNDA correção veio em 11/08/2026, medindo em vez de estimando. A conta
/// `n1_vence / (n2_vence + n1_vence)`, que dava o equilíbrio em 0,40, ignora os bônus
/// de sequência — e eles não são simétricos: com o N2 levando ~28% dos duelos, a
/// sequência de 3 do N1 fecha em ~10% das corridas e a do N2 em ~1,5%. O equilíbrio
/// REAL era 0,42, acima de toda a faixa que o mundo simulado produz (0,227 a 0,325):
/// nenhuma dupla montável tinha deriva positiva. Com o delta do N1 em 0,5 o
/// equilíbrio medido cai para 0,308 — a dupla mediana continua no chão (é o que
/// "hierarquia respeitada" deve valer) e a minoria de duplas parelhas sobe. Os
/// valores e a medição estão em [`crate::hierarchy::tensao`].
pub fn update_tensao(team: &mut Team, duel: &DuelResult) {
    let delta = delta_da_rodada(
        &TENSAO_DELTAS,
        duel,
        team.hierarquia_sequencia_n2,
        team.hierarquia_sequencia_n1,
    );
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
///
/// Ambos clampados em [0, 100].
pub fn apply_inversao_driver_effects(promoted: &mut Driver, demoted: &mut Driver) {
    promoted.motivacao = (promoted.motivacao + 15.0).clamp(0.0, 100.0);
    demoted.motivacao = (demoted.motivacao - 10.0).clamp(0.0, 100.0);
}

/// Faz a inversão valer no CONTRATO, não só no estado da equipe.
///
/// `team.hierarquia_n1_id` e `contract.papel` são duas representações do mesmo "quem
/// é o N1", e o mercado lê a segunda: [`crate::market::renewal::should_renew_contract`]
/// aplica os gates de N2 (`"N2 fraco"`, `"N2 mediano"`) e classifica continuidade por
/// `contract.papel`. Enquanto a inversão só trocava os ids na tabela `teams`, o piloto
/// que ganhava a política interna continuava sendo julgado como N2 na renovação — a
/// consequência mais natural do sistema nunca chegava ao mercado.
///
/// Só mexe em contrato **Regular** e **da própria equipe** — o contrato Especial
/// (bloco de convocação) tem hierarquia própria e não deve ser tocado aqui.
/// Best-effort por piloto: contrato ausente simplesmente não é atualizado.
fn sync_contract_roles_after_inversao(
    conn: &Connection,
    team_id: &str,
    promoted_id: &str,
    demoted_id: &str,
) -> Result<(), DbError> {
    for (driver_id, papel) in [
        (promoted_id, TeamRole::Numero1),
        (demoted_id, TeamRole::Numero2),
    ] {
        let Some(contract) =
            contract_queries::get_active_regular_contract_for_pilot(conn, driver_id)?
        else {
            continue;
        };
        if contract.equipe_id != team_id {
            continue;
        }
        contract_queries::update_contract_role(conn, &contract.id, &papel)?;
    }
    Ok(())
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
            team.hierarquia_tensao =
                (team.hierarquia_tensao - TENSAO_DECAIMENTO_SEM_DUELO).clamp(0.0, 100.0);
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

                // A inversão precisa valer no CONTRATO — é o que o mercado lê.
                sync_contract_roles_after_inversao(conn, &team.id, &promoted_id, &demoted_id)?;
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

    // ── Passo 12b — sincronia do contrato ────────────────────────────────────

    /// Schema mínimo de `contracts` — só o necessário para `SELECT *` mapear.
    fn contracts_only_db() -> Connection {
        let conn = Connection::open_in_memory().expect("db em memoria");
        conn.execute_batch(
            "CREATE TABLE contracts (
                id TEXT PRIMARY KEY NOT NULL,
                piloto_id TEXT NOT NULL,
                piloto_nome TEXT NOT NULL,
                equipe_id TEXT NOT NULL,
                equipe_nome TEXT NOT NULL,
                temporada_inicio INTEGER NOT NULL,
                duracao_anos INTEGER NOT NULL,
                temporada_fim INTEGER NOT NULL,
                salario REAL NOT NULL DEFAULT 0.0,
                salario_anual REAL NOT NULL DEFAULT 0.0,
                papel TEXT NOT NULL DEFAULT 'Numero2',
                status TEXT NOT NULL DEFAULT 'Ativo',
                tipo TEXT NOT NULL DEFAULT 'Regular',
                categoria TEXT NOT NULL,
                classe TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .expect("schema de contratos");
        conn
    }

    fn insert_contract_stub(
        conn: &Connection,
        id: &str,
        piloto_id: &str,
        equipe_id: &str,
        papel: TeamRole,
        tipo: crate::models::enums::ContractType,
    ) {
        conn.execute(
            "INSERT INTO contracts (
                id, piloto_id, piloto_nome, equipe_id, equipe_nome,
                temporada_inicio, duracao_anos, temporada_fim,
                salario, salario_anual, papel, status, tipo, categoria, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 2, 2, 0.0, 100000.0, ?6, 'Ativo', ?7, 'gt3', '2026-01-01T12:00:00')",
            rusqlite::params![
                id,
                piloto_id,
                format!("Piloto {piloto_id}"),
                equipe_id,
                format!("Equipe {equipe_id}"),
                papel.as_str(),
                tipo.as_str(),
            ],
        )
        .expect("insert contrato");
    }

    fn papel_de(conn: &Connection, contract_id: &str) -> String {
        conn.query_row(
            "SELECT papel FROM contracts WHERE id = ?1",
            rusqlite::params![contract_id],
            |row| row.get::<_, String>(0),
        )
        .expect("papel")
    }

    #[test]
    fn inversao_troca_o_papel_no_contrato() {
        use crate::models::enums::ContractType;
        let conn = contracts_only_db();
        insert_contract_stub(
            &conn,
            "C1",
            "P001",
            "T001",
            TeamRole::Numero1,
            ContractType::Regular,
        );
        insert_contract_stub(
            &conn,
            "C2",
            "P002",
            "T001",
            TeamRole::Numero2,
            ContractType::Regular,
        );

        // P002 (o N2) venceu a política interna e foi promovido.
        sync_contract_roles_after_inversao(&conn, "T001", "P002", "P001").expect("sync");

        assert_eq!(papel_de(&conn, "C2"), TeamRole::Numero1.as_str());
        assert_eq!(papel_de(&conn, "C1"), TeamRole::Numero2.as_str());
    }

    #[test]
    fn sync_ignora_contrato_de_outra_equipe() {
        use crate::models::enums::ContractType;
        let conn = contracts_only_db();
        // O "demoted" já assinou com outro time — não é nosso para rebaixar.
        insert_contract_stub(
            &conn,
            "C1",
            "P001",
            "T999",
            TeamRole::Numero1,
            ContractType::Regular,
        );
        insert_contract_stub(
            &conn,
            "C2",
            "P002",
            "T001",
            TeamRole::Numero2,
            ContractType::Regular,
        );

        sync_contract_roles_after_inversao(&conn, "T001", "P002", "P001").expect("sync");

        assert_eq!(papel_de(&conn, "C2"), TeamRole::Numero1.as_str());
        assert_eq!(
            papel_de(&conn, "C1"),
            TeamRole::Numero1.as_str(),
            "contrato de outra equipe nao pode ser tocado"
        );
    }

    #[test]
    fn sync_ignora_contrato_especial() {
        use crate::models::enums::ContractType;
        let conn = contracts_only_db();
        insert_contract_stub(
            &conn,
            "C1",
            "P001",
            "T001",
            TeamRole::Numero1,
            ContractType::Especial,
        );
        insert_contract_stub(
            &conn,
            "C2",
            "P002",
            "T001",
            TeamRole::Numero2,
            ContractType::Especial,
        );

        sync_contract_roles_after_inversao(&conn, "T001", "P002", "P001").expect("sync");

        assert_eq!(
            papel_de(&conn, "C2"),
            TeamRole::Numero2.as_str(),
            "o bloco especial tem hierarquia propria"
        );
        assert_eq!(papel_de(&conn, "C1"), TeamRole::Numero1.as_str());
    }

    #[test]
    fn sync_sem_contrato_nao_falha() {
        let conn = contracts_only_db();
        sync_contract_roles_after_inversao(&conn, "T001", "P002", "P001")
            .expect("piloto sem contrato e caso normal, nao erro");
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
            valid: true,
            vencedor: Some(winner),
        }
    }

    fn duel_invalid() -> DuelResult {
        DuelResult {
            valid: false,
            vencedor: None,
        }
    }

    #[test]
    fn test_update_tensao_n2_wins_increases() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 20.0;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3, sem decaimento: a corrida foi disputada.
        assert_eq!(team.hierarquia_tensao, 23.0);
    }

    /// O QUE ESTAVA QUEBRADO, em uma linha de aritmética.
    ///
    /// Com o -1 incidindo em toda corrida, um par 50/50 perdia 0,5 de tensão por
    /// rodada — e com piso em zero, morava no chão para sempre. Num save de 27
    /// temporadas isso deu 101 de 102 equipes em "estável" e nenhuma rivalidade
    /// de companheiros jamais criada.
    #[test]
    fn a_balanced_pairing_now_builds_tension_instead_of_bleeding_it() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 20.0;

        // Dez corridas, cinco para cada lado. As sequências ficam em 1 o tempo
        // todo, então nenhum bônus entra e sobra só a regra base.
        for rodada in 0..10 {
            let vencedor = if rodada % 2 == 0 {
                DuelWinner::N2
            } else {
                DuelWinner::N1
            };
            update_tensao(&mut team, &duel(vencedor));
        }

        // O número exato sai dos deltas — o que o teste guarda é o SINAL: uma dupla
        // 50/50 tem que subir. Antes da correção do decaimento ela descia.
        let esperado = 20.0 + 5.0 * TENSAO_DELTA_N2_VENCE - 5.0 * TENSAO_DELTA_N1_VENCE;
        assert_eq!(team.hierarquia_tensao, esperado);
        assert!(
            team.hierarquia_tensao > 20.0,
            "uma dupla 50/50 precisa acumular tensão, e ficou em {}",
            team.hierarquia_tensao
        );
    }

    #[test]
    fn test_update_tensao_n1_wins_decreases() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 20.0;
        update_tensao(&mut team, &duel(DuelWinner::N1));
        assert_eq!(team.hierarquia_tensao, 20.0 - TENSAO_DELTA_N1_VENCE);
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
        // +3 + 10 = +13 → 112 → clampado em 100
        update_tensao(&mut team, &duel(DuelWinner::N2));
        assert_eq!(team.hierarquia_tensao, 100.0);
    }

    #[test]
    fn test_update_tensao_bonus_seq_n2_3() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 30.0;
        team.hierarquia_sequencia_n2 = 3;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3 + 10 = +13
        assert_eq!(team.hierarquia_tensao, 43.0);
    }

    #[test]
    fn test_update_tensao_bonus_seq_n2_5() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 30.0;
        team.hierarquia_sequencia_n2 = 5;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3 + 15 = +18
        assert_eq!(team.hierarquia_tensao, 48.0);
    }

    #[test]
    fn test_update_tensao_bonus_seq_n1_3() {
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 40.0;
        team.hierarquia_sequencia_n1 = 3;
        update_tensao(&mut team, &duel(DuelWinner::N1));
        assert_eq!(
            team.hierarquia_tensao,
            40.0 - TENSAO_DELTA_N1_VENCE - TENSAO_PENALIDADE_SEQ_N1_3
        );
    }

    #[test]
    fn test_update_tensao_no_bonus_on_seq_4() {
        // A sequência 4 não aplica bônus extra — só 3 e 5
        let mut team = sample_team("P001", "P002");
        team.hierarquia_tensao = 30.0;
        team.hierarquia_sequencia_n2 = 4;
        update_tensao(&mut team, &duel(DuelWinner::N2));
        // +3 (sem bônus)
        assert_eq!(team.hierarquia_tensao, 33.0);
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

    /// O TETO DE UMA TEMPORADA, e o motivo de `has_inversao_trigger` nunca disparar
    /// no jogo — que não é calibração, é aritmética, e já era verdade antes da
    /// recalibração de 11/08/2026 (ela mexeu só no lado do N1).
    ///
    /// A temporada mais longa da escada tem 14 corridas e toda equipe começa em
    /// tensão 0 (a virada dá `FullReset`). O máximo que um N2 pode acumular é vencer
    /// as 14: 14 × 3 mais os bônus de sequência 3 e 5, que valem uma vez cada. Dá 67,
    /// e o gatilho de inversão exige status Crise, que começa em 90. Medido em 40 mil
    /// temporadas sintéticas: 0,0% chegam à Crise mesmo com o N2 levando 95% dos
    /// duelos.
    ///
    /// Sair disso é decisão de produto e mora em outros arquivos: baixar o limiar de
    /// Crise (`models/team.rs`) ou deixar a tensão atravessar a virada
    /// (`hierarchy/transition.rs`, hoje sempre reset). Enquanto for assim, este teste
    /// guarda o fato; quando deixar de valer, ele quebra e cobra a atualização.
    #[test]
    fn a_temporada_perfeita_do_n2_ainda_fica_longe_da_crise() {
        let mut team = sample_team("P001", "P002");

        for _ in 0..14 {
            apply_duel_counters(&mut team, &duel(DuelWinner::N2));
            update_tensao(&mut team, &duel(DuelWinner::N2));
        }
        update_status(&mut team);

        let teto = 14.0 * TENSAO_DELTA_N2_VENCE + TENSAO_BONUS_SEQ_N2_3 + TENSAO_BONUS_SEQ_N2_5;
        assert_eq!(
            team.hierarquia_tensao, teto,
            "o teto de uma temporada perfeita do N2 deixou de ser {teto}"
        );
        assert_ne!(
            TeamHierarchyClimate::from_str(&team.hierarquia_status),
            TeamHierarchyClimate::Crise,
            "a Crise virou alcançável numa temporada — o gatilho de inversão passou a \
             existir, atualize este teste e o item A2.2"
        );
        assert!(
            !has_inversao_trigger(&team, 14, 14),
            "o gatilho de inversão passou a disparar; reveja o teto e o limiar de Crise"
        );
    }

    #[test]
    fn test_has_inversao_trigger_status_nao_crise() {
        let mut team = team_gatilho_inversao();
        team.hierarquia_tensao = 80.0;
        team.hierarquia_status = "inversao".to_string();
        assert!(!has_inversao_trigger(&team, 9, 14));
    }
}
