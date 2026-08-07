//! Testes do renderizador de peças. O mais importante é o de cobertura: ele é o que
//! garante que o app nunca peça um `.wav` que o gerador não vai produzir.

use crate::engenheiro::intencao::Intencao;
use crate::engenheiro::catalogo;
use crate::engenheiro::fala::renderizar;
use crate::iracing_sdk::race_monitor::EstadoAgora;

use super::{estado_base, vizinho};

const TODAS: [Intencao; 12] = [
    Intencao::Posicao,
    Intencao::Frente,
    Intencao::Atras,
    Intencao::Restante,
    Intencao::Combustivel,
    Intencao::Ritmo,
    Intencao::Carro,
    Intencao::Bandeira,
    Intencao::Pista,
    Intencao::Pneu,
    Intencao::Campeonato,
    Intencao::Geral,
];

/// Estados de varredura, montados por EIXO e não por produto cartesiano.
///
/// A tentação é cruzar tudo com tudo, e ela custa caro: os oito eixos deste módulo dão
/// milhões de combinações e um teste que ninguém roda. Variar um eixo por vez a partir de
/// uma base cobre cada valor pelo menos uma vez, que é o que o teste de cobertura precisa —
/// ele procura chave inexistente, e uma chave inexistente aparece no VALOR, não na
/// combinação. Os eixos que de fato interagem (pneu × pista) ganham um produto pequeno à
/// parte.
fn estados_de_varredura() -> Vec<EstadoAgora> {
    let mut v = Vec::new();
    let com_vizinhos = |mut e: EstadoAgora, gap: f64| {
        let mut viz = vizinho("Rodrigues", gap);
        viz.composto = e.meu_composto;
        viz.pneu_voltas = e.meu_pneu_voltas;
        e.frente = Some(viz.clone());
        e.atras = Some(viz);
        e
    };

    // Posição: dentro, na borda e além do teto do acervo.
    for pos in [0, 1, 2, 5, 17, 24, 39, 40, 41, 63] {
        let mut e = estado_base();
        e.posicao = pos;
        v.push(com_vizinhos(e, 1.2));
    }
    // Voltas restantes, nas duas formas (exata e estimada de prova por tempo).
    for restantes in [-1, 0, 1, 2, 12, 21, 59, 60, 61, 200] {
        for estimadas in [false, true] {
            let mut e = estado_base();
            e.voltas_restantes = restantes;
            e.voltas_restantes_estimadas = estimadas;
            v.push(com_vizinhos(e, 1.2));
        }
    }
    // Gap: as três faixas de fala, as bordas de arredondamento e o desconhecido.
    for gap in [
        -1.0, 0.0, 0.02, 0.049, 0.05, 0.7, 0.96, 0.99, 1.0, 1.24, 1.25, 5.55, 9.4, 9.94, 9.97,
        10.0, 10.4, 30.0, 59.6, 60.0, 60.4, 61.0, 250.0,
    ] {
        v.push(com_vizinhos(estado_base(), gap));
    }
    // Saldo de combustível: sobra, limite, falta, e o NaN de desconhecido.
    for saldo in [f64::NAN, -61.0, -60.0, -3.2, -1.0, -0.1, 0.0, 0.5, 1.0, 9.0] {
        let mut e = estado_base();
        e.saldo_combustivel_voltas = saldo;
        v.push(com_vizinhos(e, 1.2));
    }
    // Bandeiras: todo rótulo que o monitor sabe produzir, mais um inventado.
    for bandeira in [
        "",
        "Bandeira amarela",
        "Bandeira vermelha",
        "Bandeira azul",
        "Bandeirada",
        "Reparo obrigatório",
        "Bandeira preta",
        "Desclassificado",
        "Última volta",
        "Bandeira Que Não Existe",
    ] {
        let mut e = estado_base();
        e.bandeira = bandeira.to_string();
        v.push(com_vizinhos(e, 1.2));
    }
    // Preta e DQ chegam por campo próprio, não só pelo rótulo.
    for (preta, dq, reparo) in [(true, false, 0.0), (false, true, 0.0), (false, false, 12.0)] {
        let mut e = estado_base();
        e.bandeira_preta = preta;
        e.desclassificado = dq;
        e.reparo_obrigatorio_s = reparo;
        v.push(com_vizinhos(e, 1.2));
    }
    // Pneu × pista: aqui o produto se justifica, porque o descasamento SÓ existe no
    // cruzamento — um composto isolado não diz se está certo ou errado para a pista.
    for composto in [-1, 0, 1, 2] {
        for idade in [-1, 0, 1, 15, 59, 60, 61] {
            for molhada in [false, true] {
                let mut e = estado_base();
                e.meu_composto = composto;
                e.meu_pneu_voltas = idade;
                e.pista_molhada = molhada;
                e.chuva_agora = if molhada { 0.3 } else { 0.0 };
                v.push(com_vizinhos(e, 1.2));
            }
        }
    }
    // Diferença de idade entre o meu pneu e o dele, nos dois sentidos e nas bordas.
    for dele in [0, 1, 8, 15, 59, 60, 61] {
        let mut e = estado_base();
        e.meu_composto = 0;
        e.meu_pneu_voltas = 15;
        let mut viz = vizinho("Rodrigues", 1.2);
        viz.composto = 0;
        viz.pneu_voltas = dele;
        e.frente = Some(viz.clone());
        e.atras = Some(viz);
        v.push(e);
    }
    // Estados de sessão e de vizinhança que mudam o roteamento.
    for ajuste in 0..6 {
        let mut e = com_vizinhos(estado_base(), 1.2);
        match ajuste {
            0 => e.conectado = false,
            1 => e.em_corrida = false,
            2 => e.em_formacao = true,
            3 => {
                e.frente = None;
                e.atras = None;
            }
            4 => {
                if let Some(f) = e.frente.as_mut() {
                    f.no_box = true;
                }
            }
            _ => {
                if let Some(f) = e.frente.as_mut() {
                    f.volta_a_parte = true;
                }
            }
        }
        v.push(e);
    }
    v
}

#[test]
fn toda_peca_que_o_renderizador_emite_existe_no_catalogo() {
    // ESTE é o teste que sustenta o caminho pré-gravado, e o motivo de ele existir é o
    // formato do modo de falha: o renderizador diz "sei responder", o app procura um `.wav`
    // que nunca foi gerado, e o engenheiro fica MUDO no meio de uma corrida. Sem erro, sem
    // log, sem nada — indistinguível de o jogador não ter apertado o botão.
    // O acervo do engenheiro MAIS as peças herdadas do spotter. As herdadas não estão no
    // catálogo de propósito — ele é a lista do que GERAR, e regerá-las produziria uma segunda
    // tomada de uma frase que já existe na voz da mesma pessoa.
    let mut catalogo: std::collections::HashSet<String> =
        catalogo().into_iter().map(|(chave, _)| chave).collect();
    catalogo.extend(
        crate::engenheiro::fala::PECAS_DO_SPOTTER
            .iter()
            .map(|s| s.to_string()),
    );

    let mut vistas = std::collections::HashSet::new();
    let mut renderizados = 0;
    for estado in estados_de_varredura() {
        for intencao in TODAS {
            let Some(pecas) = renderizar(&estado, intencao) else {
                continue;
            };
            renderizados += 1;
            assert!(!pecas.is_empty(), "renderizou fala VAZIA para {intencao:?}");
            for p in pecas {
                assert!(
                    catalogo.contains(&p),
                    "o renderizador pediu a peça '{p}', que NÃO existe no catálogo \
                     (intenção {intencao:?})"
                );
                vistas.insert(p);
            }
        }
    }
    // Guardas contra o teste se auto-neutralizar: uma varredura que parasse de renderizar
    // (ou um `renderizar` que passasse a devolver sempre `None`) continuaria "passando".
    assert!(
        renderizados > 200,
        "a varredura renderizou pouco: {renderizados}"
    );
    assert!(
        vistas.len() > 50,
        "a varredura tocou poucas peças distintas: {}",
        vistas.len()
    );
}

#[test]
fn o_catalogo_nao_tem_chave_repetida_nem_texto_torto() {
    // Chave repetida com textos diferentes faria o gerador sobrescrever uma gravação pela
    // outra, e a fala errada sairia só num valor específico — o tipo de defeito que só
    // aparece em corrida.
    let itens = catalogo();
    let mut vistas = std::collections::HashMap::new();
    for (chave, texto) in &itens {
        assert!(!texto.trim().is_empty(), "texto vazio na chave '{chave}'");
        // A família de QUEBRA é a única montada por peças, e por um motivo aritmético: a
        // frase inteira custaria 355 sobrenomes × 108 trechos × 8 enquadramentos. As duas
        // regras abaixo — frase completa, começando em maiúscula — descrevem peça que se
        // toca sozinha, e nenhuma das duas vale para um fragmento de oração. A gramática
        // dela é conferida no lugar dela (`quebra::tests`), que sabe qual fragmento pode
        // terminar em vírgula e qual pode terminar no artigo.
        // O TEMPO DE VOLTA entra na mesma isenção, e pelo mesmo motivo: `"trinta e zero."`
        // nunca abre uma frase — ele vem sempre depois de um lead ("Volta em,") ou de um nome
        // ("…é do Cooper,"). Maiúscula ali seria mentira sobre o papel da peça.
        if chave.starts_with(crate::engenheiro::quebra::PREFIXO)
            || chave.starts_with(crate::engenheiro::nomes::PREFIXO_SOBRENOME)
            || chave.starts_with(crate::engenheiro::nomes::PREFIXO_EQUIPE)
            || chave.starts_with(crate::engenheiro::tempo_volta::PREFIXO_TEMPO)
            || chave.starts_with("tv_")
            || chave.starts_with("conj_")
            || chave.starts_with("ab_")
            || chave.starts_with("ap_")
            || chave.starts_with("co_")
            // O VIZINHO NOMEADO é fecho de fala montada: "está a um e dois na sua frente."
            // vem sempre depois de "Cooper," e começar em maiúscula seria mentira sobre o
            // papel da peça, exatamente como nos trechos de quebra.
            || chave.starts_with(crate::engenheiro::vizinhanca::PREFIXO)
        {
            if let Some(anterior) = vistas.insert(chave.clone(), texto.clone()) {
                panic!("chave '{chave}' duplicada: {anterior:?} e {texto:?}");
            }
            continue;
        }
        // Texto que o jogador ouve começa em maiúscula; o tom discreto vem da voz, não de
        // escrever em caixa baixa.
        let primeira = texto.chars().next().unwrap();
        assert!(
            primeira.is_uppercase(),
            "a peça '{chave}' começa em minúscula: {texto:?}"
        );
        // O VOCATIVO é a única peça que ABRE a fala sem fechá-la. Por isso ele não entra na
        // isenção de cima — ele começa em maiúscula como qualquer coisa que o jogador ouve
        // primeiro —, e ao mesmo tempo não pode terminar em ponto: "Novato." e "Novato," são
        // gravações diferentes, e a primeira entraria com contorno de frase encerrada na
        // frente de uma frase que ainda vai começar.
        if chave.starts_with(crate::engenheiro::tratamento::PREFIXO) {
            assert!(
                texto.ends_with(','),
                "o vocativo '{chave}' tem que terminar em vírgula: {texto:?}"
            );
            if let Some(anterior) = vistas.insert(chave.clone(), texto.clone()) {
                panic!("chave '{chave}' duplicada: {anterior:?} e {texto:?}");
            }
            continue;
        }
        // TODA peça do acervo é uma frase completa — não há mais colagem de lead + valor,
        // porque o gap virou fundido. Isso é o que torna esta regra verificável em vez de
        // uma exceção caso a caso: quem fecha, fecha com ponto.
        assert!(
            texto.ends_with('.'),
            "a peça '{chave}' não termina em ponto: {texto:?}"
        );
        if let Some(anterior) = vistas.insert(chave.clone(), texto.clone()) {
            panic!("chave '{chave}' duplicada: {anterior:?} e {texto:?}");
        }
    }

    // TEXTO repetido também é defeito, e mais caro que chave repetida. Dois arquivos com a
    // mesma frase custam duas gerações, dois lugares em disco — e, porque a Chirp 3 não é
    // determinística, viram duas TOMADAS distintas da mesma fala. O jogador ouviria a mesma
    // frase com timbres sutilmente diferentes conforme o caminho que a acionou, que é
    // exatamente a deriva que o pacote pré-gravado existe para evitar.
    let mut por_texto: std::collections::HashMap<&String, &String> =
        std::collections::HashMap::new();
    for (chave, texto) in &itens {
        if let Some(anterior) = por_texto.insert(texto, chave) {
            panic!("texto repetido em '{anterior}' e '{chave}': {texto:?}");
        }
    }

    assert!(
        itens.len() > 300,
        "catálogo pequeno demais: {}",
        itens.len()
    );
}

#[test]
fn caso_comum_renderiza_e_e_a_razao_de_existir_do_caminho_barato() {
    let mut e = estado_base();
    e.posicao = 5;
    e.voltas_restantes = 12;
    e.frente = Some(vizinho("Rodrigues", 1.2));

    assert_eq!(
        renderizar(&e, Intencao::Posicao),
        Some(vec!["pos_5".into()])
    );
    assert_eq!(
        renderizar(&e, Intencao::Restante),
        Some(vec!["restam_12".into()])
    );
    // O gap é uma peça FUNDIDA: "O carro da frente está a um e dois." numa gravação só.
    assert_eq!(
        renderizar(&e, Intencao::Frente),
        Some(vec!["frente_gap_1_2".into()])
    );
}

#[test]
fn prova_por_tempo_toca_a_gravacao_aproximada_e_nao_a_exata() {
    // A distinção que o dossiê carrega no texto, o acervo carrega em DUAS gravações. Tocar
    // "Faltam doze voltas" numa prova por tempo seria precisão falsa dita com a voz do
    // engenheiro — pior que no texto, porque ninguém revisa uma fala.
    let mut e = estado_base();
    e.voltas_restantes = 12;
    e.voltas_restantes_estimadas = true;
    assert_eq!(
        renderizar(&e, Intencao::Restante),
        Some(vec!["restam_aprox_12".into()])
    );
}

#[test]
fn numeros_alem_do_teto_caem_no_modelo_em_vez_de_inventar_peca() {
    // O teto do acervo é a fronteira do caminho barato. Passar dele tem de devolver `None`,
    // não uma chave otimista: "pos_41" produziria silêncio, e silêncio é indistinguível de
    // bug.
    let mut e = estado_base();
    e.posicao = 41;
    e.total_carros = 60;
    assert!(renderizar(&e, Intencao::Posicao).is_none());

    let mut e = estado_base();
    e.voltas_restantes = 61;
    assert!(renderizar(&e, Intencao::Restante).is_none());
}

#[test]
fn intencao_sem_acervo_admite_e_vai_para_o_modelo() {
    // Carro mistura segundos de reparo com peças avariadas e não tem forma fixa. Geral é a
    // pergunta aberta — exatamente o que o modelo faz melhor que qualquer template.
    //
    // `Ritmo` estava nesta lista e SAIU: a biblioteca de tempos existe agora.
    let e = estado_base();
    for i in [Intencao::Carro, Intencao::Geral] {
        assert!(renderizar(&e, i).is_none(), "{i:?} deveria cair no modelo");
    }
}

#[test]
fn o_ritmo_responde_com_o_tempo_e_com_o_quanto_falta() {
    // O estado-base tem a nossa última em 92,8 s e a melhor da corrida (de outro) em 92,0 s:
    // oito décimos. As duas frases saem, nessa ordem.
    let e = estado_base();
    assert_eq!(
        renderizar(&e, Intencao::Ritmo),
        Some(vec!["tv_volta_em".into(), "t_928".into(), "tv_faltam_8".into()]),
    );
}

#[test]
fn com_a_melhor_da_corrida_na_mao_o_ritmo_nao_manda_perseguir_a_si_mesmo() {
    // "Faltam zero décimos para a melhor volta" seria pior que o silêncio: informa nada e
    // ainda soa como se o engenheiro não soubesse de quem é a melhor.
    let mut e = estado_base();
    e.melhor_da_corrida_e_minha = true;
    e.melhor_da_corrida_s = e.ultima_volta_s;
    assert_eq!(
        renderizar(&e, Intencao::Ritmo),
        Some(vec!["tv_volta_em".into(), "t_928".into()]),
    );
}

#[test]
fn volta_fora_da_faixa_gravada_manda_a_pergunta_de_ritmo_para_o_modelo() {
    // Nordschleife: 11:43, fora dos 4 min gravados. É tudo-ou-nada — meia resposta gravada
    // emendada com meia gerada mostraria a emenda E a deriva de timbre.
    let mut e = estado_base();
    e.ultima_volta_s = 703.0;
    assert!(renderizar(&e, Intencao::Ritmo).is_none());
    // E sem volta válida ainda, idem.
    e.ultima_volta_s = -1.0;
    assert!(renderizar(&e, Intencao::Ritmo).is_none());
}

#[test]
fn volta_a_parte_nao_vira_anuncio_de_disputa() {
    // Quem está uma volta atrás é tráfego, não adversário. O acervo não tem fala para a
    // distinção, e anunciar o gap dele como briga mandaria o piloto atacar quem não está
    // na corrida dele.
    let mut e = estado_base();
    let mut v = vizinho("Rodrigues", 1.2);
    v.volta_a_parte = true;
    e.frente = Some(v);
    assert!(renderizar(&e, Intencao::Frente).is_none());
}

#[test]
fn arredondamento_de_gap_nunca_produz_chave_fora_da_faixa() {
    // O defeito que a primeira versão tinha: 9,97 caía na faixa de 1 a 9,9 e o décimo
    // arredondava para 10, produzindo "gap_9_10" — que não existe. Arredondar ANTES de
    // escolher a faixa resolve: 9,97 vira 100 décimos, que é a faixa dos inteiros.
    let mut e = estado_base();
    e.frente = Some(vizinho("Rodrigues", 9.97));
    let pecas = renderizar(&e, Intencao::Frente).expect("deveria renderizar");
    assert_eq!(
        pecas,
        vec!["frente_gap_10"],
        "arredondamento vazou: {pecas:?}"
    );

    // Um gap que arredonda para zero décimo não vira "zero décimos" — vira "colado", que é a
    // informação mais clara que existe naquele instante e tem fala própria no acervo.
    e.frente = Some(vizinho("Rodrigues", 0.02));
    assert_eq!(
        renderizar(&e, Intencao::Frente),
        Some(vec!["frente_colado".into()])
    );
}

#[test]
fn sem_vizinho_e_lideranca_ou_ultimo_lugar_e_nao_falta_de_dado() {
    let mut e = estado_base();
    e.frente = None;
    e.atras = None;
    assert_eq!(
        renderizar(&e, Intencao::Frente),
        Some(vec!["lidera".into()])
    );
    assert_eq!(renderizar(&e, Intencao::Atras), Some(vec!["ultimo".into()]));
}

#[test]
fn fora_de_corrida_o_acervo_se_cala() {
    // Na formação, uma posição não é uma posição e um gap não é uma disputa. Tocar a
    // gravação de sempre seria dizer algo verdadeiro no campo e falso no sentido.
    let mut e = estado_base();
    e.em_formacao = true;
    assert!(renderizar(&e, Intencao::Posicao).is_none());

    let mut e = estado_base();
    e.em_corrida = false;
    assert!(renderizar(&e, Intencao::Frente).is_none());

    // Sem telemetria, porém, há fala: dizer "estou sem dados" é melhor que silêncio.
    let mut e = estado_base();
    e.conectado = false;
    assert_eq!(
        renderizar(&e, Intencao::Posicao),
        Some(vec!["sem_telemetria".into()])
    );
}

#[test]
fn bandeira_desconhecida_pelo_acervo_cai_no_modelo() {
    // O monitor pode ganhar um rótulo novo sem que ninguém grave a peça correspondente.
    // Nesse dia o certo é o modelo cobrir, não o app tocar a bandeira errada.
    let mut e = estado_base();
    e.bandeira = "Bandeira Que Não Existe".to_string();
    assert!(renderizar(&e, Intencao::Bandeira).is_none());
}

#[test]
fn pneu_de_carro_mono_composto_cai_no_modelo() {
    // No MX-5 o SDK não informa composto. Sem ele não dá para dizer nem "seco" nem
    // "chuva", e a idade sozinha não responde a pergunta que foi feita.
    let mut e = estado_base();
    e.meu_composto = -1;
    e.meu_pneu_voltas = 15;
    assert!(renderizar(&e, Intencao::Pneu).is_none());
}

#[test]
fn descasamento_do_carro_da_frente_tem_fala_propria() {
    // A fala que o usuário pediu: pista molhada, ele ainda de seco, vai ter que parar.
    let mut e = estado_base();
    e.pista_molhada = true;
    e.meu_composto = 1;
    e.meu_pneu_voltas = 4;
    let mut v = vizinho("Rodrigues", 1.2);
    v.composto = 0;
    v.pneu_voltas = 18;
    e.frente = Some(v);
    let pecas = renderizar(&e, Intencao::Pneu).expect("deveria renderizar");
    assert!(
        pecas.contains(&"pneu_frente_seco_pista_molhada".to_string()),
        "sem o descasamento em {pecas:?}"
    );
    // E o meu lado está certo para a pista, então não deve acusar nada.
    assert!(
        !pecas.iter().any(|p| p.starts_with("pneu_voce_")),
        "acusou descasamento inexistente em {pecas:?}"
    );
}

#[test]
fn o_acervo_cabe_na_camada_gratuita_de_geracao() {
    // A Cloud TTS cobra US$ 30 por milhão de caracteres e dá 1 milhão grátis por mês. O
    // acervo é gerado UMA vez, então cabendo aqui ele custa zero — e é essa folga que
    // justifica fundir em vez de colar, já que fundir troca arquivos (baratos) por
    // caracteres (também baratos, até este teto).
    let itens = catalogo();
    let caracteres: usize = itens.iter().map(|(_, t)| t.chars().count()).sum();
    println!(
        "acervo: {} peças, {} caracteres ({:.1}% do milhão gratuito)",
        itens.len(),
        caracteres,
        caracteres as f64 / 10_000.0
    );
    assert!(
        caracteres < 200_000,
        "o acervo passou de 200 mil caracteres ({caracteres}) — ainda cabe no gratuito, \
         mas a essa altura vale reconferir se alguma família explodiu por engano"
    );
}

/// Despeja o catálogo inteiro em texto, agrupado por família, para revisão de COPY.
///
/// As 731 peças são conteúdo, não código: cada uma vira uma gravação que o jogador vai ouvir
/// dezenas de vezes numa corrida, e uma palavra torta ali é permanente até alguém regerar o
/// pacote. Revisar isso lendo Rust é pedir para não revisar.
///
/// Roda com `cargo test --lib despeja_catalogo_para_revisao`.
#[test]
fn despeja_catalogo_para_revisao() {
    let itens = catalogo();
    let familia = |chave: &str| -> &'static str {
        if chave.starts_with("pos_") {
            "POSIÇÃO"
        } else if chave.starts_with("restam_aprox_") {
            "VOLTAS RESTANTES — prova por tempo (estimativa)"
        } else if chave.starts_with("restam_") {
            "VOLTAS RESTANTES — prova por voltas"
        } else if chave.starts_with("frente_gap_") {
            "GAP — carro da frente"
        } else if chave.starts_with("atras_gap_") {
            "GAP — carro de trás"
        } else if chave.starts_with("comb_") {
            "COMBUSTÍVEL"
        } else if chave.starts_with("pneu_idade_") {
            "PNEU — idade do seu"
        } else if chave.starts_with("pneu_dele_") {
            "PNEU — comparação com o vizinho"
        } else if chave.starts_with("pneu_") {
            "PNEU — composto e descasamento"
        } else if chave.starts_with("band_") {
            "BANDEIRAS"
        } else if chave.starts_with("pista_") {
            "PISTA"
        } else if chave.starts_with("nm_") {
            "QUEBRA — sobrenomes"
        } else if chave.starts_with("eq_") {
            "QUEBRA — equipes"
        } else if chave.starts_with("qb_dnf_") {
            "QUEBRA — abandono"
        } else if chave.starts_with("qb_") {
            "QUEBRA — trechos por peça"
        } else if chave.starts_with("ab_") || chave.starts_with("ap_") || chave.starts_with("co_") {
            "QUEBRA — enquadramento"
        } else {
            "AVULSAS"
        }
    };

    let mut por_familia: std::collections::BTreeMap<&str, Vec<(&String, &String)>> =
        std::collections::BTreeMap::new();
    for (c, t) in &itens {
        por_familia.entry(familia(c)).or_default().push((c, t));
    }

    let mut saida = String::from("# Catálogo de falas do engenheiro\n\n");
    saida.push_str(&format!(
        "Gerado por `engenheiro::catalogo()`. **{} peças, {} caracteres.**\n\n\
         Cada linha vira um `.wav` gravado pela Cloud TTS com a voz do engenheiro. \
         A chave é o nome do arquivo; o texto é o que será falado.\n\n\
         Revise o TEXTO, não a chave — é o que o jogador ouve.\n",
        itens.len(),
        itens.iter().map(|(_, t)| t.chars().count()).sum::<usize>(),
    ));
    for (nome, linhas) in &por_familia {
        saida.push_str(&format!("\n## {nome} — {} peças\n\n", linhas.len()));
        for (c, t) in linhas {
            saida.push_str(&format!("- `{c}` — {t}\n"));
        }
    }

    let caminho = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/engenheiro-catalogo.md"
    );
    std::fs::write(caminho, &saida).expect("gravar catálogo");

    // O MESMO catálogo em JSON, para o gerador de pacote consumir. Duas renderizações da
    // mesma lista, escritas na mesma passada: o `.md` é para o humano revisar a copy, o
    // `.json` é para a máquina gerar os `.wav`. Manter o gerador lendo o markdown
    // funcionaria hoje e quebraria no dia em que alguém mexesse no formato do documento.
    let json = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/engenheiro-catalogo.json"
    );
    let pares: Vec<serde_json::Value> = itens
        .iter()
        .map(|(c, t)| serde_json::json!({ "chave": c, "texto": t }))
        .collect();
    std::fs::write(json, serde_json::to_string_pretty(&pares).unwrap()).expect("gravar JSON");
    for (nome, linhas) in &por_familia {
        println!("  {:<50} {} peças", nome, linhas.len());
    }
    println!("\n  {caminho}");
}

#[test]
fn a_resolucao_do_gap_afrouxa_conforme_a_distancia() {
    // Décimo de segundo num carro a três segundos é precisão que não muda decisão nenhuma.
    // O décimo sobrevive só abaixo de dois segundos, que é onde há briga de verdade.
    let chave = |g: f64| {
        let mut e = estado_base();
        e.frente = Some(vizinho("Rodrigues", g));
        renderizar(&e, Intencao::Frente).map(|p| p[0].clone())
    };

    // Briga: décimo a décimo.
    assert_eq!(chave(0.7).as_deref(), Some("frente_gap_0_7"));
    assert_eq!(chave(1.2).as_deref(), Some("frente_gap_1_2"));
    assert_eq!(chave(1.9).as_deref(), Some("frente_gap_1_9"));

    // De dois a dez: meio em meio. 2,3 arredonda para 2,5 e 2,2 para 2,0 — os dois caem em
    // peças que existem, e nenhum inventa um `gap_2_3`.
    assert_eq!(chave(2.0).as_deref(), Some("frente_gap_2_0"));
    assert_eq!(chave(2.2).as_deref(), Some("frente_gap_2_0"));
    assert_eq!(chave(2.3).as_deref(), Some("frente_gap_2_5"));
    assert_eq!(chave(2.5).as_deref(), Some("frente_gap_2_5"));
    assert_eq!(chave(9.4).as_deref(), Some("frente_gap_9_5"));

    // Daí para cima, segundo inteiro.
    assert_eq!(chave(10.0).as_deref(), Some("frente_gap_10"));
    assert_eq!(chave(9.9).as_deref(), Some("frente_gap_10"));
    assert_eq!(chave(23.4).as_deref(), Some("frente_gap_23"));

    // E a fronteira entre as faixas não abre buraco: 1,96 sobe para dois segundos cravados.
    assert_eq!(chave(1.96).as_deref(), Some("frente_gap_2_0"));
}

#[test]
fn o_gap_falado_e_o_mesmo_nos_dois_caminhos() {
    // A `gap_falado` alimenta o dossiê do modelo; a `renderizar` nomeia a peça gravada. Se as
    // duas divergirem, o engenheiro passa a dizer o mesmo gap de dois jeitos conforme o
    // caminho — e a costura entre o gravado e o gerado aparece.
    use crate::engenheiro::fala::gap_falado;
    assert_eq!(gap_falado(0.7).as_deref(), Some("sete décimos"));
    assert_eq!(gap_falado(0.1).as_deref(), Some("um décimo"));
    assert_eq!(gap_falado(1.0).as_deref(), Some("um segundo"));
    assert_eq!(gap_falado(1.2).as_deref(), Some("um e dois"));
    assert_eq!(gap_falado(2.5).as_deref(), Some("dois segundos e meio"));
    assert_eq!(gap_falado(3.0).as_deref(), Some("três segundos"));
    assert_eq!(gap_falado(15.0).as_deref(), Some("quinze segundos"));
    // Encostado não é "zero décimos" — não há peça e não há texto.
    assert_eq!(gap_falado(0.02), None);
}

/// Toda peça do acervo é ALCANÇÁVEL por alguma situação de corrida?
///
/// É a pergunta espelhada da cobertura, e nenhuma das duas responde a outra. Aquela prova que
/// o app nunca pede um arquivo que não existe; esta prova que não existe arquivo que o app
/// nunca pede.
///
/// Uma peça órfã não quebra nada — e é exatamente por isso que ela sobrevive. Ela custa uma
/// geração, ocupa disco no instalador de todo jogador, entra na revisão de copy e ninguém
/// nunca a ouve. O único jeito de encontrá-la é perguntar.
///
/// A varredura aqui é EXAUSTIVA, ao contrário da de cobertura: percorre cada posição, cada
/// número de voltas, cada valor da grade de gap. Amostrar serviria lá, onde o defeito procurado
/// mora no valor de canto; aqui o defeito é uma peça inteira nunca produzida, e pular valores
/// produziria órfãos fantasmas.
#[test]
fn toda_peca_gravada_e_alcancavel_por_alguma_situacao() {
    let mut emitidas = std::collections::HashSet::new();
    let mut colher = |e: &EstadoAgora, i: Intencao| {
        if let Some(pecas) = renderizar(e, i) {
            emitidas.extend(pecas);
        }
    };

    // Posição: todo lugar do grid, com o campo grande o bastante para não virar "último".
    for pos in 1..=40 {
        let mut e = estado_base();
        e.posicao = pos;
        e.total_carros = 60;
        colher(&e, Intencao::Posicao);
    }
    // E a borda que produz "último": ser o último de verdade.
    let mut e = estado_base();
    e.posicao = 24;
    e.total_carros = 24;
    colher(&e, Intencao::Posicao);
    colher(&e, Intencao::Atras);

    // Voltas restantes, nas duas formas e em todo valor.
    for n in 0..=60 {
        for estimadas in [false, true] {
            let mut e = estado_base();
            e.voltas_restantes = n;
            e.voltas_restantes_estimadas = estimadas;
            colher(&e, Intencao::Restante);
        }
    }

    // Gap: a grade inteira, em décimos, dos dois lados. O passo de 1 décimo cobre as três
    // resoluções sem precisar conhecê-las.
    // Começa em ZERO: abaixo de um décimo o gap deixa de ser número e vira "colado", e essa
    // faixa é uma peça do acervo como qualquer outra.
    for decimos in 0..=(60 * 10) {
        let mut e = estado_base();
        let v = vizinho("Rodrigues", decimos as f64 / 10.0);
        e.frente = Some(v.clone());
        e.atras = Some(v);
        colher(&e, Intencao::Frente);
        colher(&e, Intencao::Atras);
    }
    // Vizinho no box, e sem vizinho nenhum.
    let mut e = estado_base();
    let mut v = vizinho("Rodrigues", 1.2);
    v.no_box = true;
    e.frente = Some(v.clone());
    e.atras = Some(v);
    colher(&e, Intencao::Frente);
    colher(&e, Intencao::Atras);
    let mut e = estado_base();
    e.frente = None;
    e.atras = None;
    colher(&e, Intencao::Frente);
    colher(&e, Intencao::Atras);

    // Combustível: sobra, limite, e todo déficit até bem além do teto.
    for saldo in [3.0, 0.5] {
        let mut e = estado_base();
        e.saldo_combustivel_voltas = saldo;
        colher(&e, Intencao::Combustivel);
    }
    for falta in 1..=60 {
        let mut e = estado_base();
        e.saldo_combustivel_voltas = -(falta as f64);
        colher(&e, Intencao::Combustivel);
    }

    // Bandeiras e pista.
    for bandeira in [
        "",
        "Bandeira amarela",
        "Bandeira vermelha",
        "Bandeira azul",
        "Bandeirada",
    ] {
        let mut e = estado_base();
        e.bandeira = bandeira.to_string();
        colher(&e, Intencao::Bandeira);
    }
    for (preta, dq, reparo) in [(true, false, 0.0), (false, true, 0.0), (false, false, 9.0)] {
        let mut e = estado_base();
        e.bandeira_preta = preta;
        e.desclassificado = dq;
        e.reparo_obrigatorio_s = reparo;
        colher(&e, Intencao::Bandeira);
    }
    for (molhada, chuva) in [(false, 0.0), (true, 0.0), (true, 0.4)] {
        let mut e = estado_base();
        e.pista_molhada = molhada;
        e.chuva_agora = chuva;
        colher(&e, Intencao::Pista);
    }

    // Pneu: composto × idade × idade do vizinho × pista.
    for meu_composto in [0, 1] {
        for idade in 0..=60 {
            for molhada in [false, true] {
                for dele_composto in [0, 1] {
                    for dele_idade in [0, 1, 5, 20, 40, 60] {
                        let mut e = estado_base();
                        e.meu_composto = meu_composto;
                        e.meu_pneu_voltas = idade;
                        e.pista_molhada = molhada;
                        let mut v = vizinho("Rodrigues", 1.2);
                        v.composto = dele_composto;
                        v.pneu_voltas = dele_idade;
                        e.frente = Some(v);
                        colher(&e, Intencao::Pneu);
                    }
                }
            }
        }
    }

    // Sem telemetria.
    let mut e = estado_base();
    e.conectado = false;
    colher(&e, Intencao::Posicao);

    // O TEMPO DE VOLTA. Os 2.101 tempos e o lead saem de `renderizar`, um por valor de última
    // volta — varrer a faixa inteira é barato e é o único jeito de provar que não há buraco no
    // meio dela. As frases de anúncio saem do observador de ritmo, que não é uma pergunta.
    {
        use crate::engenheiro::ritmo::{Fala, Observador, Passagem};
        use crate::engenheiro::tempo_volta as tv;
        for d in tv::MIN_DECIMOS..=tv::MAX_DECIMOS {
            let mut e = estado_base();
            e.ultima_volta_s = f64::from(d) / 10.0;
            // A melhor da corrida um décimo à frente: cobre o tempo E a aproximação.
            e.melhor_da_corrida_s = e.ultima_volta_s - 0.1;
            e.melhor_da_corrida_e_minha = false;
            colher(&e, Intencao::Ritmo);
        }
        // As nove distâncias de aproximação.
        for falta in 1..=tv::DECIMOS_DE_APROXIMACAO {
            let mut e = estado_base();
            e.ultima_volta_s = 92.4;
            e.melhor_da_corrida_s = 92.4 - f64::from(falta) / 10.0;
            e.melhor_da_corrida_e_minha = false;
            colher(&e, Intencao::Ritmo);
        }
        // O observador: a troca de dono e as três redações de "tomamos".
        let mut o = Observador::novo();
        o.observar(Passagem {
            volta: 1,
            minha_volta_s: 95.0,
            melhor_da_corrida_s: 94.0,
            dono_idx: 7,
            e_minha: false,
        });
        for volta in 2..=12 {
            for (minha, melhor, dono, e_minha) in [
                (95.0, 94.0 - f64::from(volta) / 10.0, volta, false),
                (93.0, 93.0, 0, true),
            ] {
                if let Some(f) = o.observar(Passagem {
                    volta,
                    minha_volta_s: minha,
                    melhor_da_corrida_s: melhor,
                    dono_idx: dono,
                    e_minha,
                }) {
                    match f {
                        Fala::Tomamos(c) | Fala::Aproximando(c) => {
                            emitidas.insert(c);
                        }
                        Fala::DeOutro { lead, tempo, .. } => {
                            emitidas.insert(lead);
                            emitidas.insert(tempo);
                        }
                    }
                }
            }
        }
    }

    // A família do NOSSO carro. Não sai de `renderizar` nem do montador de quebra: ela responde
    // ao desgaste das peças do jogador, e quem a produz é `commands::overlay::avisos`.
    {
        use crate::engenheiro::peca_propria as pp;
        for peca in pp::PECAS.iter().chain(["turbo"].iter()) {
            for variante in 0..3 {
                emitidas.insert(pp::chave_aviso(peca, variante));
                // O DESFECHO, e não só o aviso. São dois emissores na mesma família, e
                // varrer só o primeiro deixava 108 peças — 12 × 3 severidades × 3
                // redações — marcadas como órfãs: geradas, empacotadas e "nunca ouvidas"
                // segundo o teste, quando na verdade era o teste que não sabia chamá-las.
                for severidade in ["light", "heavy", "dnf"] {
                    emitidas.insert(pp::chave_desfecho(peca, severidade, variante));
                }
            }
        }
        for variante in 0..3 {
            emitidas.insert(pp::poupar_frase(variante).0.to_string());
        }
    }

    // A CLASSIFICAÇÃO. Também não sai de `renderizar` — quem a produz é o observador da
    // sessão de classificação, e as falas dele dependem de ONDE na volta o carro está.
    {
        use crate::engenheiro::classificacao as cl;
        let mut o = cl::Observador::novo();
        let base = cl::Momento {
            ate_a_linha_s: 60.0,
            restante_s: 1200.0,
            volta_referencia_s: 90.0,
            volta_morta: false,
            em_preparacao: true,
            voando: false,
        };
        // As seis despedidas, uma por tentativa — o rodízio é por sessão.
        for _ in 0..6 {
            if let Some(f) = o.observar(cl::Momento { ate_a_linha_s: cl::DESPEDIDA_MAX_S, ..base }) {
                emitidas.extend(f.pecas);
            }
            o.observar(cl::Momento { em_preparacao: false, voando: true, ..base });
            o.observar(base);
        }
        // A volta morta, varrendo o tempo que sobra: pega as três redações de reconhecimento,
        // a faixa de tentativas restantes e as três de "acabou".
        // A faixa é escolhida contra a aritmética de `tentativas_que_cabem` (cada tentativa custa
        // duas voltas de 90 s, menos os 30 s até a linha): 1200→6+, 800→4, 600→3, 400→2, 250→1,
        // 100→0. O 800 entrou porque o guard reclamou de `cl_restam_4` — que é exatamente o
        // trabalho dele.
        for restante in [1200.0, 800.0, 600.0, 400.0, 250.0, 100.0] {
            for _ in 0..3 {
                let morta = cl::Momento {
                    ate_a_linha_s: 30.0,
                    restante_s: restante,
                    volta_morta: true,
                    em_preparacao: false,
                    voando: true,
                    ..base
                };
                if let Some(f) = o.observar(morta) {
                    emitidas.extend(f.pecas);
                }
                o.observar(base); // volta à preparação para rearmar
            }
        }
    }

    // A família de QUEBRA tem outro emissor: ela responde a um evento da corrida, não a uma
    // pergunta do jogador, e por isso não sai de `renderizar`. Varrer a matriz dela AQUI, em
    // vez de isentá-la, é o que mantém a pergunta honesta — "nenhuma situação produz" tem que
    // valer para o acervo inteiro, não só para a parte que este teste sabe percorrer.
    {
        use crate::engenheiro::quebra::{montar, Contexto, PECAS};
        for peca in PECAS {
            for sev in ["light", "heavy", "dnf"] {
                for variante in 0..3 {
                  // Os dois lados do limiar de atrito: abaixo dele sai a coda de GANHO, no
                  // cruzamento exato sai a de ATRITO. Sem varrer os dois, metade das codas
                  // ficaria inalcançável — e o teste diria que elas são órfãs.
                  for abandonos in [0, crate::engenheiro::quebra::ABANDONOS_PARA_COMENTAR + 1] {
                    for (nem, riv, comp, lider, delta, assento, nome) in [
                        (true, false, false, false, None, 1, "James Cooper"),
                        (false, true, false, false, None, 2, "James Cooper"),
                        (false, false, true, false, None, 1, "James Cooper"),
                        (false, false, false, true, None, 2, "James Cooper"),
                        (false, false, false, false, Some(4), 1, "James Cooper"),
                        (false, false, false, false, Some(-4), 2, "James Cooper"),
                        // Sem gravação do sobrenome: é o caminho que produz as duas aberturas
                        // de assento e as 102 equipes.
                        (false, false, false, false, None, 1, "Carlos Magnossilva"),
                        (false, false, false, false, None, 2, "Carlos Magnossilva"),
                    ] {
                        emitidas.extend(montar(&Contexto {
                            nome_completo: nome.into(),
                            equipe: Some("Kitsune".into()),
                            assento,
                            e_nemesis: nem,
                            e_rival: riv,
                            e_companheiro: comp,
                            lidera_campeonato: lider,
                            delta_pontos: delta,
                            peca: peca.into(),
                            severidade: sev.into(),
                            variante,
                            abandonos_ate_aqui: abandonos,
                        })
                        .pecas);
                    }
                  }
                }
            }
        }
        // A FUSÃO de duas quebras simultâneas: outro montador, mesma família.
        for sev in ["light", "heavy", "dnf"] {
            for variante in 0..3 {
                let mut a = Contexto {
                    nome_completo: "James Cooper".into(),
                    equipe: Some("Kitsune".into()),
                    assento: 1,
                    e_nemesis: false,
                    e_rival: false,
                    e_companheiro: false,
                    lidera_campeonato: false,
                    delta_pontos: None,
                    peca: "engine".into(),
                    severidade: sev.into(),
                    variante,
                    abandonos_ate_aqui: 0,
                };
                let mut b = a.clone();
                b.nome_completo = "Marco Bianchi".into();
                if let Some(f) = crate::engenheiro::quebra::montar_duplo(&a, &b) {
                    emitidas.extend(f.pecas);
                }
                // E a recusa também é caminho: com vínculo, a fusão devolve `None` e as duas
                // falas saem separadas — que é o que o resto desta varredura já cobre.
                a.e_rival = true;
                assert!(crate::engenheiro::quebra::montar_duplo(&a, &b).is_none());
            }
        }

        // As 102 equipes e os 355 sobrenomes só saem variando o piloto, não o cenário.
        for (catalogo_nome, _) in crate::engenheiro::nomes::EQUIPES_FALADAS {
            emitidas.extend(montar(&Contexto {
                nome_completo: "Carlos Magnossilva".into(),
                equipe: Some((*catalogo_nome).into()),
                assento: 1,
                e_nemesis: false,
                e_rival: false,
                e_companheiro: false,
                lidera_campeonato: false,
                delta_pontos: None,
                peca: "engine".into(),
                severidade: "heavy".into(),
                variante: 0,
                abandonos_ate_aqui: 0,
            })
            .pecas);
        }
        for sobrenome in crate::engenheiro::nomes::sobrenomes() {
            emitidas.extend(montar(&Contexto {
                nome_completo: sobrenome.into(),
                equipe: Some("Kitsune".into()),
                assento: 1,
                e_nemesis: false,
                e_rival: true,
                e_companheiro: false,
                lidera_campeonato: false,
                delta_pontos: None,
                peca: "engine".into(),
                severidade: "heavy".into(),
                variante: 0,
                abandonos_ate_aqui: 0,
            })
            .pecas);
        }
    }

    // A família do CAMPEONATO tem outro emissor: `responder::renderizar`, que compõe a
    // telemetria com a tabela lida do save. Ela é varrida AQUI em vez de isentada pelo
    // mesmo motivo da família de quebra — "nenhuma situação produz" tem que valer para o
    // acervo inteiro, e não só para a parte que sai de `fala::renderizar`.
    {
        use crate::engenheiro::campeonato::Contexto;
        use crate::engenheiro::responder;
        let e = estado_base();
        // Posição até um a mais que o teto (para exercer o `None`), e a diferença em todo
        // valor inteiro até um a mais que o teto dela. Com `para_o_proximo` e `folga` no
        // mesmo número, uma varredura cobre as duas famílias: o líder fala da folga, o
        // resto fala da diferença para cima.
        for posicao in 1..=41 {
            for p in 0..=61 {
                // A projeção varre TODO destino possível a partir desta posição — subir,
                // cair, ficar e o `None` de não ter conta. É o que alcança as três famílias
                // de `camp_proj_*`, que só existem no cruzamento de duas posições.
                for projecao in std::iter::once(None).chain((1..=41).map(Some)) {
                    let c = Contexto {
                        posicao,
                        para_o_proximo: Some(f64::from(p)),
                        folga: Some(f64::from(p)),
                        projecao,
                    };
                    if let Some(pecas) = responder::renderizar(&e, Some(&c), Intencao::Campeonato)
                    {
                        emitidas.extend(pecas);
                    }
                }
            }
        }
    }

    // A família do VIZINHO NOMEADO, pelo mesmo motivo da anterior: quem a emite é
    // `responder::renderizar_com`, com o vínculo lido do save, e não `fala::renderizar`.
    {
        use crate::engenheiro::quebra::Vinculo;
        use crate::engenheiro::responder::{self, Extras};
        use crate::engenheiro::vizinhanca;

        // Décimo a décimo pela faixa inteira, e não pela grade gravada: qualquer valor cai
        // em alguma chave por `sufixo_gap`, e varrer o contínuo é o que garante que
        // nenhuma resolução da grade fique sem uma situação que a produza.
        for decimos in 0..=610 {
            for frente in [true, false] {
                for vinculo in [
                    None,
                    Some(Vinculo::Nemesis),
                    Some(Vinculo::Rival),
                    Some(Vinculo::Companheiro),
                    Some(Vinculo::Lider),
                ] {
                    for no_box in [false, true] {
                        let mut e = estado_base();
                        let mut v = vizinho("James Cooper", f64::from(decimos) / 10.0);
                        v.no_box = no_box;
                        if frente {
                            e.frente = Some(v);
                        } else {
                            e.atras = Some(v);
                        }
                        let save = Extras {
                            vizinhanca: vizinhanca::Contexto {
                                frente: if frente { vinculo } else { None },
                                atras: if frente { None } else { vinculo },
                            },
                            ..Extras::default()
                        };
                        let intencao = if frente {
                            Intencao::Frente
                        } else {
                            Intencao::Atras
                        };
                        if let Some(pecas) = responder::renderizar_com(&e, &save, intencao) {
                            emitidas.extend(pecas);
                        }
                    }
                }
            }
        }
    }

    // A família da MEMÓRIA, terceira e última que não sai de `fala::renderizar` — ela é um
    // apêndice de quem já respondeu, e o que a dispara é a resposta ANTERIOR.
    {
        use crate::engenheiro::responder::{self, Extras};

        let mut e = estado_base();
        e.frente = Some(vizinho("James Cooper", 1.2));
        for decimos in -610..=610 {
            let extras = Extras {
                memoria: Some(f64::from(decimos) / 10.0),
                ..Extras::default()
            };
            if let Some(pecas) = responder::renderizar_com(&e, &extras, Intencao::Frente) {
                emitidas.extend(pecas);
            }
        }
    }

    // A família do VOCATIVO, quarta e última que não sai de `fala::renderizar`. Quem decide
    // se você ainda é novato é o save, e qual das redações sai é o rodízio da camada de cima
    // — este teste percorre o rodízio inteiro, que é o único jeito de as três aparecerem.
    {
        use crate::engenheiro::responder::{self, Extras};
        use crate::engenheiro::tratamento::Tratamento;

        let mut e = estado_base();
        e.posicao = 5;
        for rodizio in 0..3 {
            let extras = Extras {
                tratamento: Some(Tratamento::Novato),
                rodizio,
                ..Extras::default()
            };
            if let Some(pecas) = responder::renderizar_com(&e, &extras, Intencao::Posicao) {
                emitidas.extend(pecas);
            }
        }
    }

    // Duas isenções, ambas NOMEADAS no código de produção em vez de escondidas aqui: as
    // emprestadas do spotter (que nem estão no nosso catálogo) e as que pertencem à camada de
    // cima. Uma isenção declarada num `const` é revisável; uma escondida no teste é o começo
    // de afrouxá-lo até ele parar de achar o que existe para achar.
    let isentas: std::collections::HashSet<&str> = crate::engenheiro::fala::PECAS_DO_SPOTTER
        .iter()
        .chain(crate::engenheiro::fala::PECAS_DE_OUTRA_CAMADA.iter())
        .copied()
        .collect();
    let orfas: Vec<String> = catalogo()
        .into_iter()
        .map(|(c, _)| c)
        .filter(|c| !emitidas.contains(c) && !isentas.contains(c.as_str()))
        .collect();

    assert!(
        orfas.is_empty(),
        "{} peça(s) do acervo que NENHUMA situação produz — geradas, empacotadas e nunca \
         ouvidas:\n  {}",
        orfas.len(),
        orfas.join("\n  ")
    );
}
