//! Guard da ORDEM da virada de temporada.
//!
//! `run_end_of_season_with_mode` encadeia mais de 15 passos de evolution, market,
//! world, finance, rivalry e promotion, e as invariantes de ordem viviam só em
//! comentário. A ordem já quebrou uma vez em produção: a rivalidade de pista nascia
//! e era apagada no mesmo tique porque o veredito da temporada rodava ANTES do
//! decaimento (dez das doze temporadas do harness com `pista = 0`).
//!
//! Este guard lê o próprio fonte e assevera a sequência. É de propósito um teste de
//! texto: instrumentar o encadeamento em runtime custaria mais do que o passo protege,
//! e o que precisa doer é reordenar as chamadas sem pensar.

const FONTE: &str = include_str!("../orquestracao.rs");

/// Posição da chamada no fonte. Falha com o nome do passo quando o marcador some
/// (renomeou a função? atualize o guard junto).
fn pos(marcador: &str) -> usize {
    FONTE.find(marcador).unwrap_or_else(|| {
        panic!(
            "passo '{marcador}' nao existe mais em orquestracao.rs — \
             se a chamada foi renomeada ou removida, atualize este guard"
        )
    })
}

fn ultima_pos(marcador: &str) -> usize {
    FONTE.rfind(marcador).unwrap_or_else(|| {
        panic!("passo '{marcador}' nao existe mais em orquestracao.rs");
    })
}

/// Assevera que os marcadores aparecem no fonte exatamente nesta ordem.
fn assevera_sequencia(passos: &[&str]) {
    let mut anterior: Option<(&str, usize)> = None;
    for passo in passos {
        let atual = pos(passo);
        if let Some((nome_anterior, pos_anterior)) = anterior {
            assert!(
                pos_anterior < atual,
                "ordem da virada quebrada: '{passo}' passou a rodar ANTES de \
                 '{nome_anterior}' em orquestracao.rs"
            );
        }
        anterior = Some((passo, atual));
    }
}

#[test]
fn ordem_da_virada_de_temporada_preservada() {
    assevera_sequencia(&[
        // 1. Fecha a temporada que acabou: pontos, licenças, título.
        "build_and_persist_standings(&tx",
        "persist_licenses(&tx",
        "finalize_season(&tx",
        // 2. Retrato dos assentos ANTES de qualquer movimento (referência do 2º
        //    passe da motivação, lá no fim).
        "seat_map(&tx)",
        // 3. Evolução dos pilotos e arquivamento da temporada.
        "process_driver_evolution(",
        "archive_driver_season(&tx",
        "archive_team_season(&tx",
        // 4. Consequências que LEEM o arquivo recém-gravado.
        "update_team_reputations_from_season(&tx",
        "roll_up_team_career_history(&tx",
        "update_team_morale_from_season(&tx",
        "update_bonds_from_season(&tx",
        // 5. Dinheiro e ciclo de falência, ainda na categoria em que a equipe correu.
        "award_constructor_prizes(&tx",
        "process_collapse_lifecycle(&tx",
        // 6. Só agora a escada mexe nas categorias.
        "run_promotion_relegation_for_year(&tx",
        // 7. Decaimento ANTES dos vereditos de temporada — inverter aqui apaga a
        //    rivalidade recém-criada no mesmo tique (regressão já vista).
        "apply_season_end_rivalry_decay(&tx",
        "process_teammate_season_rivalry(&tx",
        "process_track_season_rivalry(&tx",
        "apply_season_end_team_rivalry_decay(&tx",
        "process_constructor_battle_rivalry(&tx",
        // 8. Temporada nova e mercado da pré-temporada.
        "create_next_season_phase(&tx",
        "initialize_preseason_phase(&tx",
        // 9. Motivação do offseason por último: precisa do mercado já resolvido.
        "apply_offseason_motivation(&tx",
        "tx.commit()",
        // 10. O plano da pré-temporada é um arquivo, e arquivo não entra na transação:
        //     só vai para o lugar depois do banco confirmado.
        "staging.publicar()",
    ]);
}

/// O `preseason_plan.json` não pode ser publicado antes do commit. Publicá-lo antes é
/// exatamente o bug que deixava um plano novo de pé em cima de um banco que voltou
/// atrás — e o `remove_file` pendurado no erro de `commit` cobria uma única saída de
/// erro entre as várias que a virada tem.
#[test]
fn plano_da_pre_temporada_so_e_publicado_depois_do_commit() {
    assert!(
        pos("tx.commit()") < pos("staging.publicar()"),
        "o plano da pré-temporada está sendo publicado antes do commit"
    );
    assert!(
        !FONTE.contains("save_preseason_plan("),
        "a virada voltou a gravar o plano direto no arquivo definitivo, fora do staging"
    );
}

#[test]
fn vereditos_de_temporada_rodam_depois_do_decaimento() {
    // A invariante que já quebrou, isolada: o resumo do ano não pode passar pelo
    // decaimento do mesmo ano, senão esfria duas vezes.
    let decaimento = pos("apply_season_end_rivalry_decay(&tx");
    assert!(
        decaimento < pos("process_teammate_season_rivalry(&tx"),
        "rivalidade de companheiros rodando antes do decaimento anual"
    );
    assert!(
        decaimento < pos("process_track_season_rivalry(&tx"),
        "rivalidade de pista rodando antes do decaimento anual"
    );
    // A briga de construtores é o mesmo caso, do lado das EQUIPES: rodando antes do
    // decaimento dela, o par não decisor nascia em (4, 10), saía em (4, 5) e era apagado
    // na mesma transação.
    assert!(
        pos("apply_season_end_team_rivalry_decay(&tx")
            < pos("process_constructor_battle_rivalry(&tx"),
        "briga de construtores rodando antes do decaimento anual de equipe"
    );
}

#[test]
fn duas_passagens_de_cura_de_lesao_cercam_o_mercado() {
    // Passe 1 cobre o órfão que JÁ entrou na virada sem assento; passe 2 cobre quem
    // vira órfão no mercado da pré-temporada. Uma só passagem deixa um piloto
    // lesionado uma temporada inteira na bancada.
    let marcador = "process_injury_recovery_without_seat(&tx)";
    let primeira = pos(marcador);
    let segunda = ultima_pos(marcador);
    assert_ne!(
        primeira, segunda,
        "a segunda passagem de cura de lesão sumiu da virada"
    );
    assert!(
        primeira < pos("process_driver_evolution("),
        "1ª passagem de cura precisa rodar antes da evolução"
    );
    assert!(
        segunda > pos("initialize_preseason_phase(&tx"),
        "2ª passagem de cura precisa rodar depois do mercado da pré-temporada"
    );
}
