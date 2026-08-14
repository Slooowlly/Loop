//! **O bônus irmão do B35 — a penalidade de -10 por perder a vaga.**
//!
//! O -10 era derivado no segundo passe da motivação da virada de temporada: "não tem
//! assento depois do mercado". A derivação chegava cedo pelo mesmo motivo da renovação —
//! no modo jogável, quando o passe rodava, o contrato antigo ainda estava de pé e ninguém
//! tinha perdido vaga nenhuma. O sinal nunca disparava por lá.
//!
//! O gatilho agora é o fato: a pré-passe que EXPIRA o contrato e decide quem a equipe
//! segurou. Estes testes cobram os cinco casos na passada de contratos — perda real,
//! renovação, plurianual em andamento, aposentado, jogador — mais a reentrância.

use super::super::*;
use super::*;

/// A motivação de um piloto como está no banco.
fn motivacao(conn: &Connection, driver_id: &str) -> f64 {
    driver_queries::get_all_drivers(conn)
        .expect("pilotos")
        .into_iter()
        .find(|driver| driver.id == driver_id)
        .unwrap_or_else(|| panic!("piloto {driver_id} não está no banco"))
        .motivacao
}

fn renovou(report: &MarketReport, driver_id: &str) -> bool {
    report
        .new_signings
        .iter()
        .any(|s| s.driver_id == driver_id && s.tipo == "renovacao")
}

/// **Quem venceu contrato e não foi segurado perde 10, na hora em que isso se decide.**
///
/// Na fixture o P002 tem contrato vencendo na T2 com a T001. A decisão de renovar é
/// sorteada, então o teste varre sementes e cobra o par completo: onde a renovação
/// aconteceu a motivação subiu 5 (B35) e onde não aconteceu ela caiu exatamente 10. Não há
/// terceiro desfecho — a pré-passe não move ninguém de equipe, então quem não renovou saiu
/// dela sem assento.
#[test]
fn quem_nao_e_renovado_perde_dez_na_pre_passe() {
    let mut perdas = 0;
    let mut renovacoes = 0;
    for seed in 0..40u64 {
        let conn = setup_market_fixture();
        let antes = motivacao(&conn, "P002");
        let mut rng = StdRng::seed_from_u64(seed);

        let report = run_market_prepasses(&conn, 2, &mut rng).expect("pré-passes do mercado");

        if renovou(&report, "P002") {
            renovacoes += 1;
            assert_eq!(
                motivacao(&conn, "P002"),
                antes + 5.0,
                "semente {seed}: quem renovou não pode levar a penalidade da vaga"
            );
        } else {
            perdas += 1;
            assert_eq!(
                motivacao(&conn, "P002"),
                antes - 10.0,
                "semente {seed}: contrato vencido sem renovação tinha de descontar 10"
            );
        }
    }
    assert!(
        perdas > 0 && renovacoes > 0,
        "as 40 sementes não produziram os dois desfechos (perdas={perdas}, renovações={renovacoes})"
    );
}

/// **Contrato plurianual em andamento não perde nada.** O P001 está no meio de um contrato
/// que vai até a T2: ele não entra na lista de vencidos, e por isso não tem assento a
/// perder nem renovação a comemorar.
#[test]
fn plurianual_em_andamento_nao_perde_a_vaga() {
    for seed in 0..40u64 {
        let conn = setup_market_fixture();
        let antes = motivacao(&conn, "P001");
        let mut rng = StdRng::seed_from_u64(seed);

        run_market_prepasses(&conn, 2, &mut rng).expect("pré-passes do mercado");

        assert_eq!(
            motivacao(&conn, "P001"),
            antes,
            "semente {seed}: quem só continua no meio do plurianual teve a motivação mexida"
        );
    }
}

/// **Aposentar-se não é perder a vaga.** O P003 encerra a carreira com contrato em
/// vigência; a pré-passe rescinde o contrato dele, e essa saída não pode ser cobrada como
/// dispensa.
#[test]
fn aposentadoria_nao_conta_como_vaga_perdida() {
    for seed in 0..12u64 {
        let conn = setup_market_fixture();
        let antes = motivacao(&conn, "P003");
        let mut rng = StdRng::seed_from_u64(seed);

        run_market_prepasses(&conn, 2, &mut rng).expect("pré-passes do mercado");

        assert_eq!(
            motivacao(&conn, "P003"),
            antes,
            "semente {seed}: o aposentado levou penalidade de vaga perdida"
        );
    }
}

/// **O jogador não passa por este fluxo.** O futuro dele é decidido por ele na janela, e a
/// pré-passe da IA pula quem é jogador — nem renova em nome dele nem o dispensa. A
/// mecânica é de IA e continua sendo.
#[test]
fn o_jogador_nao_perde_a_vaga_na_pre_passe_da_ia() {
    for seed in 0..12u64 {
        let conn = setup_market_fixture();
        let mut jogador = driver_queries::get_all_drivers(&conn)
            .expect("pilotos")
            .into_iter()
            .find(|driver| driver.id == "P002")
            .expect("P002");
        jogador.is_jogador = true;
        driver_queries::update_driver(&conn, &jogador).expect("marca o jogador");
        let antes = jogador.motivacao;
        let mut rng = StdRng::seed_from_u64(seed);

        run_market_prepasses(&conn, 2, &mut rng).expect("pré-passes do mercado");

        assert_eq!(
            motivacao(&conn, "P002"),
            antes,
            "semente {seed}: o jogador levou o -10 de vaga perdida"
        );
    }
}

/// **Rodar a mesma janela de novo não desconta duas vezes.**
///
/// A idempotência não é uma flag: a lista de candidatos é a dos contratos ATIVOS já
/// vencidos, e o próprio passo que os expira a esvazia. Na segunda passada não sobra
/// ninguém para dispensar — nem para renovar.
#[test]
fn reexecutar_a_pre_passe_nao_desconta_de_novo() {
    for seed in 0..24u64 {
        let conn = setup_market_fixture();
        let antes = motivacao(&conn, "P002");
        let mut rng = StdRng::seed_from_u64(seed);

        run_market_prepasses(&conn, 2, &mut rng).expect("primeira pré-passe");
        let depois_da_primeira = motivacao(&conn, "P002");
        run_market_prepasses(&conn, 2, &mut rng).expect("segunda pré-passe");

        assert_eq!(
            motivacao(&conn, "P002"),
            depois_da_primeira,
            "semente {seed}: a segunda passada mexeu na motivação de novo"
        );
        let delta = depois_da_primeira - antes;
        assert!(
            delta == -10.0 || delta == 5.0,
            "semente {seed}: delta inesperado de {delta} — só cabem -10 (dispensa) ou +5 (renovação)"
        );
    }
}
