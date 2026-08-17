//! A sessão que está rolando chega ao modelo, e ele para de chamar quali de corrida.
//!
//! O defeito, medido em 17/08/2026: no fim de uma classificatória em Oschersleben, com o
//! jogador sem ter marcado tempo, o engenheiro abriu com **"Novato, que corrida"**. A causa
//! estava duas camadas abaixo da fala — `EstadoAgora::em_corrida` vinha de
//! `session_state == Racing`, e esse estado vale também em treino e classificação. O modelo
//! recebia os fatos sem nenhuma linha de sessão e escrevia o que era plausível.
//!
//! Estes testes prendem as duas pontas: a linha de sessão sai SEMPRE e é afirmativa, e a
//! classificatória diz explicitamente que não há resultado de corrida para comentar.

use super::estado_base;
use crate::engenheiro::fatos::dossie_completo;

/// A linha de sessão, que é sempre a primeira do dossiê.
fn linha_de_sessao(e: &crate::iracing_sdk::race_monitor::EstadoAgora) -> String {
    dossie_completo(e)
        .into_iter()
        .find(|l| l.starts_with("Sessão:"))
        .expect("o dossiê tem de dizer que sessão é esta")
}

#[test]
fn a_corrida_se_apresenta_como_corrida() {
    let e = estado_base();
    assert!(linha_de_sessao(&e).contains("CORRIDA"));
}

/// O caso que originou tudo. A negativa sozinha não bastava: "não é corrida" deixa o modelo
/// escolher entre treino e classificatória, e a linha precisa fechar a porta.
#[test]
fn a_classificatoria_avisa_que_nao_ha_corrida_para_comentar() {
    let mut e = estado_base();
    e.tipo_sessao = "classificacao";
    e.em_corrida = false;

    let linha = linha_de_sessao(&e);
    assert!(linha.contains("CLASSIFICATÓRIA"), "{linha}");
    assert!(
        linha.contains("Não é corrida"),
        "a linha tem de negar a corrida explicitamente: {linha}"
    );
    assert!(
        linha.contains("volta rápida") && linha.contains("grid"),
        "e dizer o que se busca aqui: {linha}"
    );
}

#[test]
fn o_treino_livre_se_apresenta_como_treino() {
    let mut e = estado_base();
    e.tipo_sessao = "treino";
    e.em_corrida = false;

    let linha = linha_de_sessao(&e);
    assert!(linha.contains("TREINO LIVRE"), "{linha}");
    assert!(linha.contains("Não é corrida"), "{linha}");
}

/// A linha sai em toda sessão, e não só quando algo está errado. Um dossiê sem ela é o
/// dossiê que produziu o defeito.
#[test]
fn nenhuma_sessao_fica_sem_a_linha() {
    for tipo in ["corrida", "classificacao", "treino"] {
        let mut e = estado_base();
        e.tipo_sessao = tipo;
        e.em_corrida = tipo == "corrida";
        let n = dossie_completo(&e)
            .iter()
            .filter(|l| l.starts_with("Sessão:"))
            .count();
        assert_eq!(n, 1, "exatamente uma linha de sessão em {tipo}");
    }
}

/// A volta de formação continua sendo dita, e agora ao LADO da sessão em vez de no lugar
/// dela. As duas informações são independentes: a formação diz em que ponto da corrida se
/// está, e a sessão diz que corrida é.
#[test]
fn a_formacao_nao_engole_a_linha_de_sessao() {
    let mut e = estado_base();
    e.em_formacao = true;
    let linhas = dossie_completo(&e);
    assert!(linhas.iter().any(|l| l.starts_with("Sessão:")));
    assert!(linhas.iter().any(|l| l.contains("volta de formação")));
}
