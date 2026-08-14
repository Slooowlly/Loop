use super::*;
use std::collections::HashSet;

fn base() -> Contexto {
    Contexto {
        nome_completo: "James Cooper".into(),
        equipe: Some("Kitsune".into()),
        assento: 2,
        e_nemesis: false,
        e_rival: false,
        e_companheiro: false,
        lidera_campeonato: false,
        delta_pontos: None,
        peca: "engine".into(),
        severidade: "heavy".into(),
        variante: 0,
        abandonos_ate_aqui: 0,
    }
}

// ─── Precedência ─────────────────────────────────────────────────────────────

#[test]
fn a_precedencia_do_vinculo_e_a_declarada() {
    let mut c = base();
    c.e_nemesis = true;
    c.e_rival = true;
    c.e_companheiro = true;
    c.lidera_campeonato = true;
    c.delta_pontos = Some(3);
    assert_eq!(Vinculo::escolher(&c), Vinculo::Nemesis);

    c.e_nemesis = false;
    assert_eq!(Vinculo::escolher(&c), Vinculo::Rival);
    c.e_rival = false;
    assert_eq!(Vinculo::escolher(&c), Vinculo::Companheiro);
    c.e_companheiro = false;
    assert_eq!(Vinculo::escolher(&c), Vinculo::Lider);
    c.lidera_campeonato = false;
    assert_eq!(Vinculo::escolher(&c), Vinculo::PontosAFrente);
}

#[test]
fn o_vizinho_de_pontos_tem_faixa_dos_dois_lados_e_teto() {
    let mut c = base();
    c.delta_pontos = Some(PONTOS_VIZINHO);
    assert_eq!(Vinculo::escolher(&c), Vinculo::PontosAFrente);
    c.delta_pontos = Some(-PONTOS_VIZINHO);
    assert_eq!(Vinculo::escolher(&c), Vinculo::PontosAtras);
    c.delta_pontos = Some(PONTOS_VIZINHO + 1);
    assert_eq!(Vinculo::escolher(&c), Vinculo::Nenhum);
    // Empate em pontos não é "à frente" nem "atrás" — e dizer "alguns pontos" para zero
    // pontos de diferença seria simplesmente falso.
    c.delta_pontos = Some(0);
    assert_eq!(Vinculo::escolher(&c), Vinculo::Nenhum);
    c.delta_pontos = None;
    assert_eq!(Vinculo::escolher(&c), Vinculo::Nenhum);
}

// ─── A gramática ─────────────────────────────────────────────────────────────

#[test]
fn nunca_sai_abertura_e_aposto_na_mesma_fala() {
    // O defeito que este teste existe para pegar: "Seu rival Cooper, que lidera o
    // campeonato, abandona…" — quatro emendas e um trava-língua. A precedência garante
    // que só um enquadramento sobrevive; isto prova que a montagem obedece.
    let aberturas: HashSet<&str> = ["ab_nemesis", "ab_rival", "ab_companheiro"].into();
    let apostos: HashSet<&str> = ["ap_lider", "ap_frente", "ap_atras"].into();
    let mut c = base();
    for (nem, riv, comp, lidera, delta) in [
        (true, true, true, true, Some(3)),
        (false, true, false, true, Some(-2)),
        (false, false, true, true, Some(5)),
        (false, false, false, true, Some(1)),
    ] {
        c.e_nemesis = nem;
        c.e_rival = riv;
        c.e_companheiro = comp;
        c.lidera_campeonato = lidera;
        c.delta_pontos = delta;
        let f = montar(&c);
        let tem_ab = f.pecas.iter().any(|p| aberturas.contains(p.as_str()));
        let tem_ap = f.pecas.iter().any(|p| apostos.contains(p.as_str()));
        assert!(
            !(tem_ab && tem_ap),
            "abertura e aposto juntos em {:?}",
            f.pecas
        );
    }
}

#[test]
fn o_rival_e_nomeado_e_a_abertura_vem_antes_do_nome() {
    let mut c = base();
    c.e_rival = true;
    let f = montar(&c);
    assert_eq!(f.pecas, vec!["ab_rival", "nm_cooper", "qb_heavy_engine_0"]);
    assert_eq!(f.texto, "Seu rival Cooper está com o motor em pane.");
}

#[test]
fn sem_vinculo_a_fala_e_pela_equipe_e_o_nome_nao_entra() {
    // O jogador não precisa saber quem é o 19º — e é justamente NÃO nomear que diz isso.
    let mut c = base();
    c.severidade = "dnf".into();
    let f = montar(&c);
    assert_eq!(f.pecas, vec!["ab_piloto2", "eq_kitsune", "qb_dnf_engine_0"]);
    assert_eq!(
        f.texto,
        "O piloto dois da Kitsune abandona a corrida com problemas no motor."
    );
}

#[test]
fn o_lider_ganha_aposto_depois_do_nome() {
    let mut c = base();
    c.lidera_campeonato = true;
    c.severidade = "dnf".into();
    let f = montar(&c);
    assert_eq!(
        f.pecas,
        vec!["nm_cooper", "ap_lider", "qb_dnf_engine_0", "co_otima"],
    );
    assert_eq!(
        f.texto,
        "Cooper, que lidera o campeonato, abandona a corrida com problemas no motor. \
         Ótima notícia pra nós.",
    );
}

// ─── A coda ──────────────────────────────────────────────────────────────────

#[test]
fn a_coda_so_sai_em_abandono() {
    // Comemorar quebra leve é comemorar cedo: o cara pode terminar na frente mesmo assim.
    let mut c = base();
    c.e_rival = true;
    for sev in ["light", "heavy"] {
        c.severidade = sev.into();
        let f = montar(&c);
        assert!(
            !f.pecas.iter().any(|p| p.starts_with("co_")),
            "coda em severidade {sev}: {:?}",
            f.pecas,
        );
    }
    c.severidade = "dnf".into();
    assert!(montar(&c).pecas.iter().any(|p| p.starts_with("co_")));
}

#[test]
fn a_coda_nao_sai_por_quem_estava_atras() {
    // A queda de quem já estava atrás não muda nada para o jogador. "Ótima notícia pra nós"
    // ali é o engenheiro comemorando o próprio nada.
    let mut c = base();
    c.severidade = "dnf".into();
    c.delta_pontos = Some(-5);
    let f = montar(&c);
    assert_eq!(Vinculo::escolher(&c), Vinculo::PontosAtras);
    assert!(
        !f.pecas.iter().any(|p| p.starts_with("co_")),
        "{:?}",
        f.pecas
    );

    c.delta_pontos = Some(5);
    assert!(montar(&c).pecas.iter().any(|p| p.starts_with("co_")));
}

#[test]
fn o_companheiro_quebrando_nao_e_boa_noticia() {
    let mut c = base();
    c.e_companheiro = true;
    c.severidade = "dnf".into();
    let f = montar(&c);
    assert_eq!(
        f.pecas,
        vec!["ab_companheiro", "nm_cooper", "qb_dnf_engine_0"]
    );
}

// ─── Degradação ──────────────────────────────────────────────────────────────

#[test]
fn sobrenome_sem_gravacao_cai_para_a_equipe() {
    let mut c = base();
    c.nome_completo = "Carlos Magnossilva".into();
    let f = montar(&c);
    assert_eq!(
        f.pecas,
        vec!["ab_piloto2", "eq_kitsune", "qb_heavy_engine_0"]
    );
    assert!(f.texto.starts_with("O piloto dois da Kitsune"));
}

#[test]
fn rival_sem_gravacao_do_nome_cala_o_audio_mas_nao_o_card() {
    // Chamar o rival do jogador de "o piloto dois da Kitsune" apaga exatamente o que fazia a
    // fala existir. Sem nome, sem áudio — o card continua contando.
    let mut c = base();
    c.nome_completo = "Carlos Magnossilva".into();
    c.e_rival = true;
    let f = montar(&c);
    assert!(f.pecas.is_empty(), "{:?}", f.pecas);
    assert_eq!(f.texto, "Carlos Magnossilva está com o motor em pane.");
}

#[test]
fn sem_nome_e_sem_equipe_no_catalogo_o_audio_cala() {
    let mut c = base();
    c.nome_completo = "Carlos Magnossilva".into();
    c.equipe = Some("Equipe de Teste".into());
    assert!(montar(&c).pecas.is_empty());
    c.equipe = None;
    assert!(montar(&c).pecas.is_empty());
}

#[test]
fn toda_quebra_vira_audio_inclusive_a_leve_de_desconhecido() {
    // Já foi o contrário: quebra leve de piloto sem vínculo ficava muda, para poupar o canal.
    // A decisão de produto é a oposta — se um carro do grid teve problema, o rádio conta.
    // O custo está medido em `car::breakdown::medicao`: ~33 falas por corrida num grid de 24
    // em 18 voltas. É a linha para apertar se um dia o rádio ficar tagarela, e agora ela está
    // escrita num teste em vez de escondida num `if`.
    let mut c = base();
    for sev in ["light", "heavy", "dnf"] {
        c.severidade = sev.into();
        assert!(!montar(&c).pecas.is_empty(), "severidade {sev} ficou muda");
    }
}

#[test]
fn peca_desconhecida_usa_a_chave_generica_junto_com_a_redacao_generica() {
    // O defeito silencioso: cair na redação genérica e manter a chave da peça inexistente
    // pediria `qb_heavy_turbo_0.wav`, que ninguém gravou. Fala morta, sem erro nenhum.
    let mut c = base();
    c.peca = "turbo".into();
    c.e_rival = true;
    let f = montar(&c);
    assert!(
        f.pecas.contains(&"qb_heavy_outra_0".to_string()),
        "{:?}",
        f.pecas
    );
    assert!(f.texto.contains("problema grave no carro"));
}

#[test]
fn severidade_desconhecida_cai_em_leve() {
    let mut c = base();
    c.severidade = "warn".into();
    c.e_rival = true;
    let f = montar(&c);
    assert!(
        f.pecas.contains(&"qb_light_engine_0".to_string()),
        "{:?}",
        f.pecas
    );
}

// ─── Duas quebras de uma vez ─────────────────────────────────────────────────

fn outro(nome: &str) -> Contexto {
    let mut c = base();
    c.nome_completo = nome.into();
    c
}

#[test]
fn duas_quebras_simultaneas_viram_uma_fala_com_os_dois_nomes() {
    let mut a = base();
    a.severidade = "dnf".into();
    let mut b = outro("Marco Bianchi");
    b.severidade = "dnf".into();
    let f = montar_duplo(&a, &b).expect("devia juntar");
    assert_eq!(
        f.pecas,
        vec!["nm_cooper", "conj_e", "nm_bianchi", "qb_dupla_dnf_0"]
    );
    assert_eq!(
        f.texto,
        "Cooper e Bianchi abandonaram a corrida com problemas no carro."
    );
}

#[test]
fn a_peca_some_na_fusao_e_isso_e_de_proposito() {
    // "Cooper e Bianchi tiveram problemas no motor e no câmbio" é trava-língua, e dizer a peça
    // de um só mentiria sobre o outro. Duas quebras ao mesmo tempo são um fato sobre a CORRIDA.
    let mut a = base();
    a.peca = "engine".into();
    let mut b = outro("Marco Bianchi");
    b.peca = "gearbox".into();
    let f = montar_duplo(&a, &b).expect("devia juntar");
    assert!(
        !f.texto.contains("motor") && !f.texto.contains("câmbio"),
        "{}",
        f.texto
    );
    assert!(f.texto.contains("no carro"));
}

#[test]
fn severidades_diferentes_nao_se_juntam() {
    // Um abandono e uma quebra leve viram "tiveram problemas" — e a fala apaga justamente a
    // que importa.
    let mut a = base();
    a.severidade = "dnf".into();
    let mut b = outro("Marco Bianchi");
    b.severidade = "light".into();
    assert!(montar_duplo(&a, &b).is_none());
}

#[test]
fn quem_tem_vinculo_nao_entra_em_fala_coletiva() {
    // O enquadramento é o que fazia aquela fala valer a pena; juntar o dissolve.
    for (rival_a, rival_b) in [(true, false), (false, true)] {
        let mut a = base();
        a.e_rival = rival_a;
        let mut b = outro("Marco Bianchi");
        b.e_rival = rival_b;
        assert!(
            montar_duplo(&a, &b).is_none(),
            "juntou com rival ({rival_a}, {rival_b})"
        );
    }
    // E o líder do campeonato também não.
    let mut a = base();
    a.lidera_campeonato = true;
    assert!(montar_duplo(&a, &outro("Marco Bianchi")).is_none());
}

#[test]
fn sem_gravacao_de_algum_sobrenome_nao_junta() {
    // "O piloto dois da Kitsune e o piloto um da Ferrari…" é longo demais para um evento que a
    // fusão existe para ENCURTAR. Melhor duas falas separadas, que já funcionam.
    let a = base();
    let b = outro("Carlos Magnossilva");
    assert!(montar_duplo(&a, &b).is_none());
    assert!(montar_duplo(&b, &a).is_none());
}

#[test]
fn dois_sobrenomes_iguais_nao_se_juntam() {
    // "Silva e Silva tiveram problemas" soa como gagueira do rádio, não como dois pilotos.
    let a = outro("João Silva");
    let b = outro("Pedro Silva");
    assert!(montar_duplo(&a, &b).is_none());
}

#[test]
fn a_fusao_varia_a_redacao_como_as_outras_familias() {
    let mut vistas = HashSet::new();
    for v in 0..3 {
        let mut a = base();
        a.variante = v;
        let f = montar_duplo(&a, &outro("Marco Bianchi")).expect("devia juntar");
        vistas.insert(f.texto.clone());
        assert!(f.pecas.contains(&chave_dupla(&a.severidade, v)));
    }
    assert_eq!(vistas.len(), 3, "as três variantes repetiram redação");
}

#[test]
fn todo_trecho_plural_existe_no_catalogo() {
    let catalogo: HashSet<String> = familia_quebra().into_iter().map(|(k, _)| k).collect();
    assert!(catalogo.contains(CONJUNCAO.0));
    for sev in ["light", "heavy", "dnf", "warn"] {
        for v in 0..4 {
            assert!(catalogo.contains(&chave_dupla(sev, v)), "falta {sev}/{v}");
        }
    }
}

#[test]
fn o_trecho_plural_nao_tem_pausa_interna() {
    for sev in ["light", "heavy", "dnf"] {
        for t in dupla_frases(sev) {
            assert!(
                !t.contains(',') && !t.contains('—') && !t.contains(';'),
                "{sev}: {t:?}"
            );
        }
    }
}

// ─── O catálogo ──────────────────────────────────────────────────────────────

#[test]
fn toda_peca_que_montar_emite_existe_no_catalogo() {
    // O acoplamento que não compila junto: `montar` pede um `.wav` pela chave, e o gerador
    // grava a partir do catálogo. Chave fora do catálogo = silêncio na pista.
    let catalogo: HashSet<String> = familia_quebra().into_iter().map(|(k, _)| k).collect();
    let mut c = base();
    let mut visto = 0;
    for peca in PECAS.iter().chain(["turbo"].iter()) {
        for sev in ["light", "heavy", "dnf", "warn"] {
            for variante in 0..4 {
                for (nem, riv, comp, lidera, delta, assento) in [
                    (true, false, false, false, None, 1),
                    (false, true, false, false, None, 2),
                    (false, false, true, false, None, 1),
                    (false, false, false, true, None, 2),
                    (false, false, false, false, Some(4), 1),
                    (false, false, false, false, Some(-4), 2),
                    (false, false, false, false, Some(90), 1),
                ] {
                    c.peca = (*peca).into();
                    c.severidade = sev.into();
                    c.variante = variante;
                    c.e_nemesis = nem;
                    c.e_rival = riv;
                    c.e_companheiro = comp;
                    c.lidera_campeonato = lidera;
                    c.delta_pontos = delta;
                    c.assento = assento;
                    for chave in montar(&c).pecas {
                        assert!(catalogo.contains(&chave), "chave fora do catálogo: {chave}");
                        visto += 1;
                    }
                }
            }
        }
    }
    assert!(visto > 500, "a varredura cobriu pouco: {visto}");
}

#[test]
fn o_catalogo_nao_tem_chave_repetida_e_tem_o_tamanho_medido() {
    let v = familia_quebra();
    let unicas: HashSet<&String> = v.iter().map(|(k, _)| k).collect();
    assert_eq!(v.len(), unicas.len(), "chave repetida na família de quebra");
    // 15 de enquadramento (5 aberturas + 3 apostos + 3 codas de ganho + 3 de atrito + a
    // conjunção) + 12 peças × 9 trechos + 9 trechos plurais + 355 sobrenomes + 102 equipes.
    // Se este número mudar, mudou o custo de gravação — e ele deve mudar de propósito.
    assert_eq!(v.len(), 15 + 12 * 9 + 9 + 355 + 102);
}

#[test]
fn todo_texto_do_catalogo_termina_em_pontuacao() {
    // Peça sem pontuação final sai do TTS com entoação suspensa, e colada dá a impressão de
    // que a frase foi cortada. As de continuação terminam em vírgula, as de fim em ponto.
    for (chave, texto) in familia_quebra() {
        // A única exceção deliberada: a abertura de assento termina em artigo ("da"), porque
        // ela emenda direto no nome da equipe sem respiro nenhum.
        if chave.starts_with("ab_piloto") {
            continue;
        }
        assert!(
            texto.ends_with(',') || texto.ends_with('.'),
            "{chave}: \"{texto}\" sem pontuação final",
        );
    }
}

#[test]
fn a_abertura_de_assento_emenda_no_nome_da_equipe_sem_pontuacao() {
    let catalogo: std::collections::HashMap<String, String> =
        familia_quebra().into_iter().collect();
    for chave in ["ab_piloto1", "ab_piloto2"] {
        let t = &catalogo[chave];
        assert!(
            t.ends_with(" da"),
            "{chave}: \"{t}\" devia terminar no artigo"
        );
    }
}

#[test]
fn o_catalogo_e_estavel_entre_chamadas() {
    // O gerador de voz decide o que gravar comparando com o que já existe em disco. Uma
    // ordem instável faria toda execução parecer uma lista nova.
    assert_eq!(familia_quebra(), familia_quebra());
}

#[test]
fn nenhum_trecho_tem_pontuacao_no_meio() {
    // MEDIDO no acervo gravado, não suposto: das 576 peças da família, as 13 que tinham
    // travessão saíram com 0,35 s de silêncio DENTRO da fala, em média, contra 0,01 s das
    // outras 106 com texto de frase. Trinta e cinco vezes mais.
    //
    // Isso não importava enquanto o texto só aparecia no card. Agora ele é FALADO, e numa fala
    // montada por colagem a pausa interna soma com a da emenda e vira buraco — o ouvinte
    // escuta a frase quebrando no meio, que é indistinguível de rádio falhando.
    for peca in PECAS {
        for sev in ["light", "heavy"] {
            for (v, t) in breakdown_frases(peca, sev).iter().enumerate() {
                assert!(
                    !t.contains('—') && !t.contains(';') && !t.contains(','),
                    "{sev}/{peca}/{v}: \"{t}\" tem pausa interna",
                );
            }
        }
        for v in 0..3 {
            let t = dnf_trecho(peca, v);
            assert!(
                !t.contains(',') && !t.contains('—') && !t.contains(';'),
                "dnf/{peca}/{v}: \"{t}\" tem pausa interna",
            );
        }
    }
}

/// A fusão engolia o comentário de atrito.
///
/// Duas quebras na mesma volta viram uma fala só. Quando o cruzamento do limiar caía
/// justamente nesse par, a contagem pulava de 3 para 5 e a fala nunca saía na corrida
/// inteira — o comentário existe para marcar o momento em que a prova vira assunto por si,
/// e era exatamente nesse momento que ele sumia.
#[test]
fn a_dupla_tambem_comenta_o_atrito_quando_o_limiar_cruza_nela() {
    let mut a = base();
    a.severidade = "dnf".into();
    a.abandonos_ate_aqui = ABANDONOS_PARA_COMENTAR + 1;
    let mut b = outro("Marco Bianchi");
    b.severidade = "dnf".into();
    b.abandonos_ate_aqui = ABANDONOS_PARA_COMENTAR + 2;

    let f = montar_duplo(&a, &b).expect("devia juntar");

    assert!(
        f.pecas.iter().any(|p| p.starts_with("co_atrito")),
        "a coda de atrito não entrou na fala dupla: {:?}",
        f.pecas
    );
    assert!(
        f.texto.contains("devorando")
            || f.texto.contains("abandonos")
            || f.texto.contains("caindo"),
        "o texto do card não recebeu a coda: {}",
        f.texto
    );
}

/// E não sai fora do cruzamento: o comentário é de uma vez só.
#[test]
fn a_dupla_longe_do_limiar_nao_comenta_atrito() {
    let mut a = base();
    a.severidade = "dnf".into();
    a.abandonos_ate_aqui = 1;
    let mut b = outro("Marco Bianchi");
    b.severidade = "dnf".into();
    b.abandonos_ate_aqui = 2;

    let f = montar_duplo(&a, &b).expect("devia juntar");

    assert!(
        !f.pecas.iter().any(|p| p.starts_with("co_atrito")),
        "comentou atrito com dois abandonos: {:?}",
        f.pecas
    );
}
