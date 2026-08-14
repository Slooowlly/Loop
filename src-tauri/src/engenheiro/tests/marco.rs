//! As duas bandeiradas que não são só mais uma bandeirada.
//!
//! O erro caro aqui é o FALSO POSITIVO, e por uma razão que não vale para o resto do rádio:
//! estas falas acontecem uma vez na vida da carreira. Um gap dito errado é corrigido na volta
//! seguinte; um título anunciado errado não tem volta seguinte. Por isso a maioria dos casos
//! abaixo prova que o marco NÃO sai.

use crate::engenheiro::marco::{self, Contexto, Marco};

/// Uma carreira já rodada, sem vitórias, no meio da temporada.
fn veterano_sem_vitoria() -> Contexto {
    Contexto {
        vitorias: 0,
        corridas: 11,
        ultima_da_temporada: false,
        projecao: None,
    }
}

// ─── A primeira vitória ──────────────────────────────────────────────────────

#[test]
fn vencer_pela_primeira_vez_depois_de_uma_espera_e_um_marco() {
    assert_eq!(
        marco::na_bandeirada(1, &veterano_sem_vitoria()),
        Some(Marco::PrimeiraVitoria)
    );
}

#[test]
fn vencer_de_cara_NAO_e_um_marco() {
    // A regra que o pedido trouxe, e ela é sobre história e não sobre contabilidade: vencer na
    // estreia não é uma vitória esperada, é um piloto na categoria errada. Um discurso ali
    // soaria como quem nunca viu corrida.
    for corridas in 0..=1 {
        let c = Contexto {
            corridas,
            ..veterano_sem_vitoria()
        };
        assert_eq!(marco::na_bandeirada(1, &c), None, "corridas = {corridas}");
    }
    // Na TERCEIRA corrida já vale — duas disputadas antes desta.
    let c = Contexto {
        corridas: 2,
        ..veterano_sem_vitoria()
    };
    assert_eq!(marco::na_bandeirada(1, &c), Some(Marco::PrimeiraVitoria));
}

#[test]
fn a_segunda_vitoria_e_so_uma_vitoria() {
    let c = Contexto {
        vitorias: 1,
        ..veterano_sem_vitoria()
    };
    assert_eq!(marco::na_bandeirada(1, &c), None);
}

#[test]
fn segundo_lugar_nao_e_vitoria() {
    // O caso bobo que a fala mais cara do rádio não pode errar.
    for posicao in [2, 3, 24] {
        assert_eq!(
            marco::na_bandeirada(posicao, &veterano_sem_vitoria()),
            None,
            "posição {posicao}"
        );
    }
}

// ─── O título ────────────────────────────────────────────────────────────────

/// A última corrida do ano, terminando como campeão.
fn ultima_corrida_campeao() -> Contexto {
    Contexto {
        vitorias: 3,
        corridas: 19,
        ultima_da_temporada: true,
        projecao: Some(1),
    }
}

#[test]
fn terminar_a_ultima_corrida_na_ponta_da_tabela_e_o_titulo() {
    assert_eq!(
        marco::na_bandeirada(4, &ultima_corrida_campeao()),
        Some(Marco::Titulo),
        "o título não depende de ganhar a corrida — depende de terminar o ano em primeiro"
    );
}

#[test]
fn liderar_no_MEIO_da_temporada_nao_e_titulo() {
    // O falso positivo que arruinaria a feature. "Terminando assim você é campeão" é verdade
    // em metade das corridas de quem lidera, e anunciar o título ali gastaria o momento e
    // ainda estaria errado na corrida seguinte.
    let c = Contexto {
        ultima_da_temporada: false,
        ..ultima_corrida_campeao()
    };
    assert_eq!(marco::na_bandeirada(1, &c), None);
}

#[test]
fn sem_projecao_nao_ha_titulo() {
    // A projeção é tudo ou nada: um carro do grid que não case com um piloto do save a mata
    // inteira (ver `engenheiro::campeonato`). Quando ela morre, o piloto ouve a fala normal
    // de bandeirada — que é o modo de falha certo para uma frase que se diz uma vez.
    let c = Contexto {
        projecao: None,
        ..ultima_corrida_campeao()
    };
    assert_eq!(marco::na_bandeirada(1, &c), None);
}

#[test]
fn terminar_o_ano_em_segundo_nao_e_titulo() {
    let c = Contexto {
        projecao: Some(2),
        ..ultima_corrida_campeao()
    };
    assert_eq!(marco::na_bandeirada(1, &c), None);
}

#[test]
fn o_titulo_GANHA_da_primeira_vitoria_e_leva_ela_junto() {
    // Vencer pela primeira vez na corrida que decide o campeonato é uma história só, e o
    // assunto dela é o campeonato. Mas o marco menor não pode sumir: sem a linha extra, o
    // piloto ouviria falar do título sem uma palavra sobre a primeira vitória da vida dele.
    let c = Contexto {
        vitorias: 0,
        corridas: 19,
        ultima_da_temporada: true,
        projecao: Some(1),
    };
    assert_eq!(marco::na_bandeirada(1, &c), Some(Marco::Titulo));
    let linhas = marco::linhas(Marco::Titulo, &c).join(" ");
    assert!(linhas.contains("CAMPEÃO"), "{linhas}");
    assert!(linhas.contains("primeira vitória"), "{linhas}");
    assert!(
        linhas.contains("dezenove"),
        "a espera entra por extenso: {linhas}"
    );
}

// ─── As linhas que sobem ao modelo ───────────────────────────────────────────

#[test]
fn o_titulo_de_quem_ja_vencia_nao_menciona_estreia() {
    let linhas = marco::linhas(Marco::Titulo, &ultima_corrida_campeao()).join(" ");
    assert!(linhas.contains("CAMPEÃO"), "{linhas}");
    assert!(!linhas.contains("primeira vitória"), "{linhas}");
}

#[test]
fn a_contagem_de_corridas_sobe_por_EXTENSO() {
    // Como todo número que vai ao modelo: ele copia o que está escrito. Um "11" na linha
    // viraria "onze" ou "um um" conforme o humor do dia.
    let linhas = marco::linhas(Marco::PrimeiraVitoria, &veterano_sem_vitoria()).join(" ");
    assert!(linhas.contains("onze corridas"), "{linhas}");
    assert!(!linhas.contains("11"), "{linhas}");
}

#[test]
fn contagem_indizivel_some_em_vez_de_virar_algarismo() {
    // O acervo de cardinais vai até sessenta. Uma carreira mais longa que isso não põe um
    // número cru no meio da prosa — a linha simplesmente não entra, e o marco continua.
    let c = Contexto {
        corridas: 200,
        ..veterano_sem_vitoria()
    };
    let linhas = marco::linhas(Marco::PrimeiraVitoria, &c);
    assert_eq!(linhas.len(), 1, "{linhas:?}");
    assert!(linhas[0].contains("PRIMEIRA VITÓRIA"));
}

#[test]
fn os_nomes_sao_os_que_o_servidor_espera() {
    // Contrato com o proxy: ele escolhe o prompt por esta string. Renomear de um lado só
    // faria a fala do título sair com a redação da bandeirada comum.
    assert_eq!(Marco::PrimeiraVitoria.nome(), "primeira_vitoria");
    assert_eq!(Marco::Titulo.nome(), "titulo");
}
