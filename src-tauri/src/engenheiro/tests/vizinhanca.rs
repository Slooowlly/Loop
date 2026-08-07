//! O vizinho com nome.
//!
//! O modo de falha aqui não é a fala sair errada — é ela sair **sobre a pessoa errada**.
//! "Seu maior rival, Cooper, está a um e dois" dito sobre um piloto qualquer é pior que
//! "o carro da frente está a um e dois": a primeira inventa uma história que o jogador vai
//! carregar pela corrida inteira. Daí a maioria dos casos ser sobre a fala se RECUSAR a
//! nomear.

use crate::engenheiro::quebra::Vinculo;
use crate::engenheiro::vizinhanca::{familia_vizinho, linha, pecas, Contexto};
use crate::engenheiro::{responder, Intencao};
use crate::iracing_sdk::race_monitor::Vizinho;

use super::{estado_base, vizinho};

/// Um vizinho com nome de pool (`Cooper` existe nos 355 gravados).
fn cooper(gap: f64) -> Vizinho {
    vizinho("James Cooper", gap)
}

fn com(frente: Option<Vinculo>) -> Contexto {
    Contexto {
        frente,
        atras: None,
    }
}

#[test]
fn sem_vinculo_a_fala_e_o_nome_mais_o_gap() {
    assert_eq!(
        pecas(&cooper(1.2), true, &Contexto::default()),
        Some(vec!["nm_cooper".into(), "viz_frente_gap_1_2".into()])
    );
}

#[test]
fn o_vinculo_entra_como_ABERTURA_e_nao_muda_o_resto() {
    // As três aberturas são as MESMAS peças da fala de quebra — mesma pessoa dizendo a
    // mesma coisa, uma tomada só. Regravá-las aqui daria dois timbres para "Seu rival,".
    for (v, ab) in [
        (Vinculo::Nemesis, "ab_nemesis"),
        (Vinculo::Rival, "ab_rival"),
        (Vinculo::Companheiro, "ab_companheiro"),
    ] {
        assert_eq!(
            pecas(&cooper(1.2), true, &com(Some(v))),
            Some(vec![
                ab.into(),
                "nm_cooper".into(),
                "viz_frente_gap_1_2".into()
            ]),
            "vínculo {v:?}"
        );
    }
}

#[test]
fn os_vinculos_de_TABELA_nao_viram_abertura() {
    // `Lider`, `PontosAFrente` e `PontosAtras` descrevem o campeonato, e o campeonato já
    // tem quem o diga. Repeti-los aqui faria a mesma informação sair duas vezes na mesma
    // resposta — uma na abertura e outra na frase da tabela logo atrás.
    for v in [Vinculo::Lider, Vinculo::PontosAFrente, Vinculo::PontosAtras] {
        assert_eq!(
            pecas(&cooper(1.2), true, &com(Some(v))),
            Some(vec!["nm_cooper".into(), "viz_frente_gap_1_2".into()]),
            "vínculo {v:?}"
        );
    }
}

#[test]
fn nome_fora_dos_pools_NAO_e_nomeado() {
    // Piloto de save antigo, ou um humano que entrou no grid: o sobrenome não tem gravação.
    // Nomear assim mesmo pediria um `.wav` inexistente, e o engenheiro emudeceria — o
    // modo de falha que o acervo inteiro existe para evitar.
    let mut v = cooper(1.2);
    v.nome = "Carlos Magnossilva".to_string();
    assert_eq!(pecas(&v, true, &Contexto::default()), None);
}

#[test]
fn volta_a_parte_nao_e_disputa_nem_com_nome() {
    // Tráfego, não briga. Anunciar o gap dele como se fosse disputa é o erro que faz o
    // piloto atacar quem não está na sua corrida — e o nome não muda isso.
    let mut v = cooper(1.2);
    v.volta_a_parte = true;
    assert_eq!(pecas(&v, true, &Contexto::default()), None);
}

#[test]
fn o_box_dispensa_o_lado_porque_o_nome_ja_disse_quem_e() {
    let mut v = cooper(1.2);
    v.no_box = true;
    assert_eq!(
        pecas(&v, true, &Contexto::default()),
        Some(vec!["nm_cooper".into(), "viz_no_box".into()])
    );
    assert_eq!(
        pecas(&v, false, &Contexto::default()),
        Some(vec!["nm_cooper".into(), "viz_no_box".into()])
    );
}

#[test]
fn colado_tem_fala_propria_dos_dois_lados() {
    assert_eq!(
        pecas(&cooper(0.04), true, &Contexto::default()).unwrap()[1],
        "viz_frente_colado"
    );
    assert_eq!(
        pecas(&cooper(0.04), false, &Contexto::default()).unwrap()[1],
        "viz_atras_colado"
    );
}

#[test]
fn gap_fora_da_faixa_falavel_derruba_a_fala_inteira() {
    // Tudo ou nada, como no resto do acervo: sem o fecho não há fala, e nomear sem dizer o
    // gap seria responder "é o Cooper" a quem perguntou a que distância ele está.
    assert_eq!(pecas(&cooper(250.0), true, &Contexto::default()), None);
    assert_eq!(pecas(&cooper(-1.0), true, &Contexto::default()), None);
}

#[test]
fn a_fala_anonima_volta_quando_a_nomeada_nao_sai() {
    // O contrato com o resto do sistema: perder o nome custa o nome, nunca a resposta.
    let mut e = estado_base();
    let mut v = cooper(1.2);
    v.nome = "Carlos Magnossilva".to_string();
    e.frente = Some(v);

    let r = responder::renderizar(&e, None, Intencao::Frente).unwrap();
    assert_eq!(r, vec!["frente_gap_1_2".to_string()]);
}

#[test]
fn com_nome_a_nomeada_GANHA_da_anonima() {
    let mut e = estado_base();
    e.frente = Some(cooper(1.2));
    let save = responder::Extras {
        vizinhanca: com(Some(Vinculo::Nemesis)),
        ..responder::Extras::default()
    };
    assert_eq!(
        responder::renderizar_com(&e, &save, Intencao::Frente).unwrap(),
        vec![
            "ab_nemesis".to_string(),
            "nm_cooper".to_string(),
            "viz_frente_gap_1_2".to_string()
        ]
    );
}

#[test]
fn a_grade_de_gaps_e_a_MESMA_das_duas_familias() {
    // Duas derivações do mesmo gap divergiriam na borda de arredondamento, e o sintoma
    // seria a fala com nome sumir exatamente nos valores em que a anônima sai — um buraco
    // que só apareceria em corrida, num décimo específico.
    let nomeada: std::collections::HashSet<String> = familia_vizinho()
        .into_iter()
        .filter_map(|(c, _)| c.strip_prefix("viz_frente_").map(str::to_string))
        .collect();
    let anonima: std::collections::HashSet<String> = crate::engenheiro::catalogo()
        .into_iter()
        .filter_map(|(c, _)| c.strip_prefix("frente_").map(str::to_string))
        .filter(|s| s.starts_with("gap_"))
        .collect();
    assert_eq!(nomeada.len(), anonima.len() + 1, "só 'colado' é a mais");
    for chave in &anonima {
        assert!(nomeada.contains(chave), "a nomeada não tem '{chave}'");
    }
}

#[test]
fn o_dossie_diz_QUEM_e_o_vizinho_e_o_vinculo() {
    // Sem isto, o retrato da corrida que sobe ao modelo é um retrato de trânsito: gaps e
    // posições sem uma pessoa em nenhuma delas.
    let l = linha(&cooper(1.2), true, &com(Some(Vinculo::Nemesis))).unwrap();
    assert!(l.contains("Cooper"), "{l}");
    assert!(l.contains("MAIOR RIVAL"), "{l}");

    let sem = linha(&cooper(1.2), true, &Contexto::default()).unwrap();
    assert!(sem.contains("Cooper"), "{sem}");
    assert!(!sem.contains("RIVAL"), "{sem}");
}

#[test]
fn a_pergunta_aberta_leva_os_DOIS_vizinhos_ao_modelo() {
    let mut e = estado_base();
    e.frente = Some(cooper(1.2));
    e.atras = Some(vizinho("Marco Bianchi", 0.8));
    let d = responder::dossie(&e, None, Intencao::Geral);
    assert!(d.iter().any(|l| l.contains("Cooper")), "{d:?}");
    assert!(d.iter().any(|l| l.contains("Bianchi")), "{d:?}");
}

#[test]
fn a_pergunta_de_um_lado_nao_leva_o_outro() {
    // Quem perguntou do carro da frente não pediu um retrato do de trás. O dossiê é o
    // contexto do modelo, e contexto a mais é convite para responder outra coisa.
    let mut e = estado_base();
    e.frente = Some(cooper(1.2));
    e.atras = Some(vizinho("Marco Bianchi", 0.8));
    let d = responder::dossie(&e, None, Intencao::Frente);
    assert!(d.iter().any(|l| l.contains("Cooper")), "{d:?}");
    assert!(!d.iter().any(|l| l.contains("Bianchi")), "{d:?}");
}
