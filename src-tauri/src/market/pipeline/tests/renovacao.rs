//! **B35 — o bônus de motivação da renovação de contrato.**
//!
//! O +5 era derivado no segundo passe da motivação da virada de temporada, comparando o
//! assento de antes com o de depois do mercado. A derivação estava certa e chegava cedo: no
//! modo jogável a virada termina antes das pré-passes rodarem, então quando o passe olhava o
//! banco não havia renovação nenhuma para reconhecer.
//!
//! O gatilho agora é o fato — a pré-passe que GRAVA o contrato novo. Estes testes cobram os
//! quatro casos: plurianual em andamento, renovação de verdade, troca de equipe e o jogador.

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

/// Motivação de todo mundo, por id — a foto de antes.
fn motivacoes(conn: &Connection) -> HashMap<String, f64> {
    driver_queries::get_all_drivers(conn)
        .expect("pilotos")
        .into_iter()
        .map(|driver| (driver.id, driver.motivacao))
        .collect()
}

fn renovou(report: &MarketReport, driver_id: &str) -> bool {
    report
        .new_signings
        .iter()
        .any(|s| s.driver_id == driver_id && s.tipo == "renovacao")
}

/// **A renovação de verdade paga +5, e o plurianual em andamento não paga nada.**
///
/// Na fixture, P002 tem contrato vencendo na T2 com a T001 (início 1, duração 1) e P001 está
/// no meio de um plurianual (início 1, duração 2, fim 2) — este último nem entra na laça de
/// renováveis, porque só contrato vencido chega lá.
///
/// A decisão de renovar é sorteada, então o teste varre sementes: em toda semente onde a
/// renovação aconteceu a motivação subiu exatamente 5, e em nenhuma delas o plurianual em
/// andamento ganhou coisa alguma.
///
/// Onde a renovação NÃO acontece o piloto não fica parado: ele sai da pré-passe sem
/// assento e leva o -10 da vaga perdida, que é o irmão deste bônus e mora em
/// [`super::perda_de_vaga`]. Aqui só se cobra que bônus de renovação não houve.
#[test]
fn renovacao_real_paga_cinco_e_plurianual_em_andamento_nao_paga() {
    let mut renovacoes = 0;
    for seed in 0..40u64 {
        let conn = setup_market_fixture();
        let antes_p001 = motivacao(&conn, "P001");
        let antes_p002 = motivacao(&conn, "P002");
        let mut rng = StdRng::seed_from_u64(seed);

        let report = run_market_prepasses(&conn, 2, &mut rng).expect("pré-passes do mercado");

        assert!(
            !renovou(&report, "P001"),
            "semente {seed}: contrato no meio da vigência não pode ser renovado"
        );
        assert_eq!(
            motivacao(&conn, "P001"),
            antes_p001,
            "semente {seed}: quem só continua no meio do plurianual ganhou bônus"
        );

        if renovou(&report, "P002") {
            renovacoes += 1;
            assert_eq!(
                motivacao(&conn, "P002"),
                antes_p002 + 5.0,
                "semente {seed}: a renovação real tinha de somar 5 uma vez"
            );
        } else {
            assert_eq!(
                motivacao(&conn, "P002"),
                antes_p002 - 10.0,
                "semente {seed}: sem renovação não há bônus, e sobra a dispensa"
            );
        }
    }
    assert!(
        renovacoes > 0,
        "nenhuma das 40 sementes renovou o P002 — o teste não mediu nada"
    );
}

/// **Trocar de equipe não é renovar, e continua não sendo depois que o -10 entrou.**
///
/// Na passada completa há transferência, assinatura de agente livre e rookie entrando.
/// Quem assina sem ter vindo de um contrato vencido não tem motivação mexida por nada
/// disso — o único +5 do fluxo é o de quem aparece no report com `tipo == "renovacao"`.
///
/// Quem TINHA contrato vencendo sai da passada com um dos dois desfechos e nenhum outro:
/// renovou (+5) ou ficou sem assento na pré-passe (-10). Ser contratado por outra equipe
/// semanas depois é o mercado dando a volta, não a dispensa desacontecendo — por isso o
/// -10 permanece mesmo em quem termina a passada empregado.
#[test]
fn so_renovacao_e_dispensa_movem_a_motivacao_na_passada_completa() {
    for seed in [7u64, 21, 44, 90] {
        let conn = setup_market_fixture();
        let antes = motivacoes(&conn);
        // Quem entra a passada com contrato ATIVO já vencido: a lista fechada de quem
        // pode renovar ou perder a vaga nesta janela.
        let vencendo: HashSet<String> = contract_queries::get_all_active_regular_contracts(&conn)
            .expect("contratos ativos")
            .into_iter()
            .filter(|contract| contract.temporada_fim < 2)
            .map(|contract| contract.piloto_id)
            .collect();
        let mut rng = StdRng::seed_from_u64(seed);

        let report = run_market(&conn, 2, &mut rng).expect("mercado completo");

        let renovados: HashSet<&str> = report
            .new_signings
            .iter()
            .filter(|s| s.tipo == "renovacao")
            .map(|s| s.driver_id.as_str())
            .collect();
        // Assinatura que NÃO é renovação e que não vinha de contrato vencido: o piloto
        // não renovou nem perdeu vaga nenhuma, e a motivação dele não pode ter mudado.
        for signing in report
            .new_signings
            .iter()
            .filter(|s| s.tipo != "renovacao" && !vencendo.contains(&s.driver_id))
        {
            if let Some(base) = antes.get(&signing.driver_id) {
                assert_eq!(
                    motivacao(&conn, &signing.driver_id),
                    *base,
                    "semente {seed}: '{}' de {} moveu a motivação",
                    signing.tipo,
                    signing.driver_name
                );
            }
        }
        for driver_id in &renovados {
            let base = antes
                .get(*driver_id)
                .unwrap_or_else(|| panic!("renovado {driver_id} não existia antes"));
            assert_eq!(
                motivacao(&conn, driver_id),
                base + 5.0,
                "semente {seed}: renovação de {driver_id} não somou 5"
            );
        }
        // O par completo de quem tinha contrato vencendo: +5 ou -10, sem meio-termo.
        for driver_id in &vencendo {
            let Some(base) = antes.get(driver_id) else {
                continue;
            };
            let esperado = if renovados.contains(driver_id.as_str()) {
                base + 5.0
            } else {
                base - 10.0
            };
            assert_eq!(
                motivacao(&conn, driver_id),
                esperado,
                "semente {seed}: {driver_id} tinha contrato vencendo e não fechou em {esperado}"
            );
        }
    }
}

/// **O jogador não passa por este fluxo.** A renovação dele é decidida na janela, por ele; a
/// pré-passe da IA pula quem é jogador (`player_was_expiring`) e não assina nada em nome
/// dele — logo não há bônus a conceder aqui, e a motivação não se move.
#[test]
fn o_jogador_nao_recebe_bonus_na_pre_passe_da_ia() {
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

        let report = run_market_prepasses(&conn, 2, &mut rng).expect("pré-passes do mercado");

        assert!(
            !renovou(&report, "P002"),
            "semente {seed}: a IA renovou o contrato do jogador"
        );
        assert_eq!(
            motivacao(&conn, "P002"),
            antes,
            "semente {seed}: o jogador recebeu bônus de renovação sem renovar"
        );
    }
}
