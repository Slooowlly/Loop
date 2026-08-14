use super::*;

fn passagem(volta: i32, minha: f64, melhor: f64, dono: i32, minha_e: bool) -> Passagem {
    Passagem {
        volta,
        minha_volta_s: minha,
        melhor_da_corrida_s: melhor,
        dono_idx: dono,
        e_minha: minha_e,
    }
}

#[test]
fn a_primeira_leitura_so_ancora() {
    // Entrar na sessão com a corrida em andamento não pode anunciar como novidade uma volta
    // mais rápida marcada dez voltas atrás. O jogador ouviria um anúncio sobre algo que ele
    // não viu acontecer — e não teria como saber que o rádio está atrasado, não errado.
    let mut o = Observador::novo();
    assert_eq!(o.observar(passagem(9, 93.0, 92.4, 7, false)), None);
    // A partir daí, uma troca de verdade fala.
    assert!(matches!(
        o.observar(passagem(10, 93.0, 92.0, 3, false)),
        Some(Fala::DeOutro { .. })
    ));
}

#[test]
fn sem_volta_marcada_o_radio_cala() {
    let mut o = Observador::novo();
    assert_eq!(o.observar(passagem(1, -1.0, -1.0, -1, false)), None);
    assert_eq!(o.observar(passagem(2, 95.0, 0.0, -1, false)), None);
}

#[test]
fn tomar_a_melhor_volta_fala_sempre_sem_esperar_intervalo() {
    // A trava existe para a tagarelice, não para a boa notícia. O jogador não crava a volta
    // mais rápida toda volta; quando crava, segurar seria calar justamente o prêmio.
    let mut o = Observador::novo();
    o.observar(passagem(1, 95.0, 94.0, 7, false));
    for volta in 2..=5 {
        // Alterna o dono a cada volta: nós tomamos, outro toma de volta, nós de novo.
        let outro = o.observar(passagem(volta, 94.5, 93.9, 7, false));
        let nosso = o.observar(passagem(volta, 93.5, 93.5, 0, true));
        assert!(
            matches!(nosso, Some(Fala::Tomamos(_))),
            "volta {volta}: tomar a melhor não falou (o de antes foi {outro:?})",
        );
    }
}

#[test]
fn as_tres_redacoes_de_tomamos_fazem_rodizio() {
    // Cravar três voltas mais rápidas na mesma corrida e ouvir a MESMA frase três vezes é o
    // que faz o rádio soar mecânico logo no momento em que ele devia soar humano.
    let mut o = Observador::novo();
    o.observar(passagem(1, 95.0, 94.0, 7, false));
    let mut vistas = Vec::new();
    for volta in 2..=4 {
        o.observar(passagem(volta, 94.0, 93.9, 7, false));
        if let Some(Fala::Tomamos(c)) = o.observar(passagem(volta, 93.0, 93.0, 0, true)) {
            vistas.push(c);
        }
    }
    assert_eq!(vistas.len(), 3);
    vistas.sort();
    vistas.dedup();
    assert_eq!(vistas.len(), 3, "as três tomadas repetiram redação");
}

#[test]
fn a_troca_de_dono_respeita_o_intervalo_em_voltas() {
    // Nas primeiras voltas o grid inteiro melhora a cada passagem e a melhor troca de dono
    // toda hora. Sem a trava, o engenheiro vira locutor de leilão.
    let mut o = Observador::novo();
    o.observar(passagem(1, 95.0, 94.0, 7, false)); // ancora
    assert!(matches!(
        o.observar(passagem(2, 95.0, 93.5, 3, false)),
        Some(Fala::DeOutro { .. })
    ));
    // Voltas 3 e 4: trocou de novo, mas o intervalo não fechou.
    assert!(!matches!(
        o.observar(passagem(3, 95.0, 93.4, 9, false)),
        Some(Fala::DeOutro { .. })
    ));
    assert!(!matches!(
        o.observar(passagem(4, 95.0, 93.3, 12, false)),
        Some(Fala::DeOutro { .. })
    ));
    // Volta 5: fechou (5 - 2 = 3).
    assert!(matches!(
        o.observar(passagem(5, 95.0, 93.2, 15, false)),
        Some(Fala::DeOutro { .. })
    ));
}

#[test]
fn o_dono_melhorando_a_propria_marca_e_noticia() {
    // O teste que faltava, e a ausência dele deixou a família inteira muda.
    //
    // O gatilho era "trocou de dono", e medido em três corridas gravadas a volta mais rápida
    // trocou de CARRO zero vez nas duas provas longas — um piloto crava a melhor cedo e depois
    // só melhora a própria marca. Com o gatilho antigo, as 14 peças desta família nunca tocavam,
    // e nenhum teste reclamava porque todos falavam de troca de dono.
    let mut o = Observador::novo();
    o.observar(passagem(1, 99.0, 94.0, 7, false));
    // MESMO dono, marca mais baixa: é notícia.
    assert!(matches!(
        o.observar(passagem(4, 99.0, 93.4, 7, false)),
        Some(Fala::DeOutro { .. })
    ));
    // E continua respeitando o intervalo, senão vira locutor de leilão nas primeiras voltas.
    assert!(!matches!(
        o.observar(passagem(5, 99.0, 93.0, 7, false)),
        Some(Fala::DeOutro { .. })
    ));
    assert!(matches!(
        o.observar(passagem(7, 99.0, 92.6, 7, false)),
        Some(Fala::DeOutro { .. })
    ));
}

#[test]
fn melhorar_a_nossa_propria_marca_nao_vira_anuncio_de_outro() {
    // Quando a melhor já é nossa, baixá-la de novo não pede "a volta mais rápida da corrida é
    // do fulano" — o fulano somos nós, e a fala sairia dizendo o óbvio com o nome errado. O
    // prêmio ("tomamos") sai na hora em que ela VIRA nossa, e isso é outro caminho.
    let mut o = Observador::novo();
    o.observar(passagem(1, 94.0, 94.0, 3, true));
    for volta in 4..=10 {
        let f = o.observar(passagem(volta, 93.0, 93.0, 3, true));
        assert!(
            !matches!(f, Some(Fala::DeOutro { .. })),
            "volta {volta}: anunciou a nossa própria melhor como sendo de outro",
        );
    }
}

#[test]
fn melhor_volta_parada_nao_e_noticia() {
    // A melhor volta parada não é notícia. Repeti-la a cada três voltas seria o rádio
    // preenchendo silêncio, que é exatamente o que ele não deve fazer.
    let mut o = Observador::novo();
    o.observar(passagem(1, 99.0, 94.0, 7, false));
    for volta in 2..=12 {
        assert!(
            !matches!(
                o.observar(passagem(volta, 99.0, 94.0, 7, false)),
                Some(Fala::DeOutro { .. })
            ),
            "volta {volta} anunciou uma melhor que não mudou",
        );
    }
}

#[test]
fn a_aproximacao_sai_quando_estamos_a_menos_de_um_segundo() {
    let mut o = Observador::novo();
    o.observar(passagem(1, 95.0, 94.0, 7, false)); // ancora
                                                   // 94,5 contra 94,0 = 5 décimos.
    assert_eq!(
        o.observar(passagem(2, 94.5, 94.0, 7, false)),
        Some(Fala::Aproximando("tv_faltam_5".into())),
    );
}

#[test]
fn longe_da_melhor_o_radio_nao_comenta() {
    // Um segundo numa volta de um e trinta é mais de 1% do tempo. "Está chegando perto" ali
    // seria otimismo, não informação — e o piloto perceberia.
    let mut o = Observador::novo();
    o.observar(passagem(1, 99.0, 94.0, 7, false));
    assert_eq!(o.observar(passagem(2, 95.1, 94.0, 7, false)), None);
    assert_eq!(o.observar(passagem(3, 98.0, 94.0, 7, false)), None);
}

#[test]
fn nao_comenta_aproximacao_da_propria_melhor_volta() {
    // Perseguir a própria melhor não é perseguir nada, e a conta daria zero ou negativo.
    let mut o = Observador::novo();
    o.observar(passagem(1, 94.0, 94.0, 0, true));
    for volta in 2..=8 {
        assert_eq!(
            o.observar(passagem(volta, 94.3, 94.0, 0, true)),
            None,
            "volta {volta} comentou aproximação da própria melhor",
        );
    }
}

#[test]
fn a_aproximacao_tambem_respeita_o_intervalo() {
    let mut o = Observador::novo();
    o.observar(passagem(1, 99.0, 94.0, 7, false));
    assert!(matches!(
        o.observar(passagem(2, 94.3, 94.0, 7, false)),
        Some(Fala::Aproximando(_))
    ));
    assert_eq!(o.observar(passagem(3, 94.2, 94.0, 7, false)), None);
    assert_eq!(o.observar(passagem(4, 94.2, 94.0, 7, false)), None);
    assert!(matches!(
        o.observar(passagem(5, 94.2, 94.0, 7, false)),
        Some(Fala::Aproximando(_))
    ));
}

#[test]
fn pista_longa_demais_nao_anuncia_e_nao_queima_o_intervalo() {
    // Nordschleife: 11:43, fora da faixa gravada. Marcar o intervalo ali faria a próxima
    // troca — que pode estar dentro da faixa — ser engolida por um anúncio que nunca saiu.
    let mut o = Observador::novo();
    o.observar(passagem(1, 500.0, 490.0, 7, false));
    assert_eq!(o.observar(passagem(2, 500.0, 489.0, 3, false)), None);
    // Volta seguinte, ainda dentro do intervalo, mas agora com tempo gravável: fala.
    assert!(matches!(
        o.observar(passagem(3, 95.0, 93.0, 9, false)),
        Some(Fala::DeOutro { .. })
    ));
}

#[test]
fn reiniciar_esquece_a_corrida_anterior() {
    // Sem isto, o dono da volta mais rápida da corrida passada seguiria valendo e a primeira
    // troca da corrida nova não seria anunciada.
    let mut o = Observador::novo();
    o.observar(passagem(1, 95.0, 94.0, 7, false));
    o.observar(passagem(2, 95.0, 93.0, 3, false));
    o.reiniciar();
    // Ancora de novo, e só então volta a falar.
    assert_eq!(o.observar(passagem(1, 95.0, 94.0, 3, false)), None);
    assert!(matches!(
        o.observar(passagem(2, 95.0, 93.0, 7, false)),
        Some(Fala::DeOutro { .. })
    ));
}

#[test]
fn o_anuncio_de_outro_carrega_o_dono_para_quem_souber_o_nome() {
    // Este módulo não conhece nome de piloto nenhum — só `car_idx`. Quem resolve o sobrenome é
    // quem tem o banco aberto. Se o idx se perdesse aqui, a fala sairia sem sujeito.
    let mut o = Observador::novo();
    o.observar(passagem(1, 95.0, 94.0, 7, false));
    match o.observar(passagem(2, 95.0, 93.0, 42, false)) {
        Some(Fala::DeOutro {
            dono_idx,
            lead,
            tempo,
        }) => {
            assert_eq!(dono_idx, 42);
            assert_eq!(lead, "tv_melhor_e_do");
            assert_eq!(tempo, "t_930");
        }
        outro => panic!("esperava DeOutro, veio {outro:?}"),
    }
}
