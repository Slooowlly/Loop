use super::*;

fn estilo_neutro() -> EstiloPiloto {
    EstiloPiloto {
        smoothness: 50.0,
        consistencia: 50.0,
        adaptabilidade: 50.0,
        aggression: 50.0,
    }
}

// ─────────────────────────── Camada 1: afinidade ───────────────────────────

// ─────────────── Pacote H: injetabilidade com equivalência ───────────────

#[test]
fn injetabilidade_com_default_reproduz_os_numeros_de_hoje() {
    // O critério de aceitação do H: valor-padrão reproduz o de hoje, BIT A BIT. Eu não escolho
    // valor nenhum — só torno escolhível.
    let e = EscalasDeForma::default();
    assert_eq!(e.afinidade, AFINIDADE_ESCALA_PONTOS);
    assert_eq!(e.forma, FORMA_ESCALA_PONTOS);
    assert_eq!(e.acerto, ACERTO_ESCALA_PONTOS);
    assert_eq!(e.rho, FORMA_RHO);
    assert_eq!(e.peso_animo, FORMA_PESO_ANIMO);

    let estilo = EstiloPiloto {
        smoothness: 62.0,
        consistencia: 71.0,
        adaptabilidade: 48.0,
        aggression: 55.0,
    };
    for i in 0..200 {
        let id = format!("DRV-{i:04}");
        for track in [523, 166, 413, 249, 554] {
            assert_eq!(
                afinidade_pista(&id, track, &estilo),
                afinidade_pista_com_escala(&id, track, &estilo, e.afinidade)
            );
        }
        for rodada in 1..=6 {
            assert_eq!(
                acerto_fim_de_semana(2, rodada, "TEAM-X", &id),
                acerto_fim_de_semana_com_escala(2, rodada, "TEAM-X", &id, e.acerto)
            );
            let s = semente_forma(2, rodada, &id);
            assert_eq!(
                proxima_forma(0.4, s, 63.0, 71.0),
                proxima_forma_com_escalas(0.4, s, 63.0, 71.0, e.rho, e.peso_animo)
            );
        }
        assert_eq!(
            ajuste_fim_de_semana("D", "T", 523, 1, 1, &estilo, 0.7).corrida,
            ajuste_fim_de_semana_com_escalas("D", "T", 523, 1, 1, &estilo, 0.7, &e).corrida
        );
    }
    for estado in [-2.5, -1.0, 0.0, 0.33, 1.0, 2.5] {
        assert_eq!(
            forma_em_pontos(estado),
            forma_em_pontos_com_escala(estado, e.forma)
        );
    }
}

#[test]
fn injetabilidade_escala_realmente_escala() {
    // E o outro lado: mexer no parâmetro tem que MOVER o número, senão a busca não tem alavanca.
    let estilo = EstiloPiloto {
        smoothness: 50.0,
        consistencia: 50.0,
        adaptabilidade: 50.0,
        aggression: 50.0,
    };
    let a1 = afinidade_pista_com_escala("D", 523, &estilo, 3.0);
    let a2 = afinidade_pista_com_escala("D", 523, &estilo, 6.0);
    assert!(
        (a2 - a1 * 2.0).abs() < 1e-9,
        "a escala é linear: {a1} → {a2}"
    );

    assert_eq!(forma_em_pontos_com_escala(1.0, 7.5), 7.5);
    let c1 = acerto_fim_de_semana_com_escala(1, 1, "T", "D", 2.5);
    let c2 = acerto_fim_de_semana_com_escala(1, 1, "T", "D", 5.0);
    assert!((c2 - c1 * 2.0).abs() < 1e-9);

    // `peso_animo = 0` desliga o deslocamento por qualidade do piloto: com ânimo neutro os dois
    // coincidem, e com ânimo alto eles divergem. É esse par que separa serial de permanente.
    let s = semente_forma(1, 1, "D");
    // O ânimo só é zero em 50, que é onde `normalizar_atributo` centra.
    assert_eq!(
        proxima_forma_com_escalas(0.0, s, 50.0, 50.0, 0.65, 0.0),
        proxima_forma_com_escalas(0.0, s, 50.0, 50.0, 0.65, 0.20),
        "em 50 o ânimo é zero, então o peso não muda nada"
    );
    // **E em 70 NÃO é** — este é o descasamento que motivou zerar o peso. O neutro declarado da
    // motivação é `MOTIVATION_REF = 70` (com teste próprio afirmando efeito zero ali), mas
    // `normalizar_atributo` centra em 50, então no ponto que o outro módulo chama de neutro o
    // ânimo vale 0,4. Constante dentro do AR(1) vira média estacionária `c/(1−ρ)`, e era daí que
    // saía o deslocamento permanente dentro de uma camada declarada serial.
    assert_ne!(
        proxima_forma_com_escalas(0.0, s, 70.0, 70.0, 0.65, 0.0),
        proxima_forma_com_escalas(0.0, s, 70.0, 70.0, 0.65, 0.20),
        "em 70 o ânimo NÃO é zero — é este o descasamento que o item 2 removeu"
    );
    assert_ne!(
        proxima_forma_com_escalas(0.0, s, 100.0, 100.0, 0.65, 0.0),
        proxima_forma_com_escalas(0.0, s, 100.0, 100.0, 0.65, 0.20),
        "com ânimo alto, o peso tem que aparecer"
    );
}

#[test]
fn injetabilidade_de_rho_preserva_a_variancia_estacionaria() {
    // Varrer ρ não pode mexer na amplitude junto, senão a busca mede dois efeitos de uma vez.
    // O `√(1 − ρ²)` acompanha o ρ ESCOLHIDO, e não o `const`.
    for rho in [0.0, 0.3, 0.5, 0.65, 0.85] {
        let mut f = 0.0;
        let mut v = Vec::new();
        for rodada in 0..4000 {
            f = proxima_forma_com_escalas(
                f,
                semente_forma(1, rodada, "DRV-RHO"),
                50.0,
                50.0,
                rho,
                0.0,
            );
            v.push(f);
        }
        let m: f64 = v.iter().sum::<f64>() / v.len() as f64;
        let sigma = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
        assert!(
            (0.8..=1.2).contains(&sigma),
            "ρ = {rho} deu σ = {sigma}, e σ tem que ficar em 1 para qualquer ρ"
        );
    }
}

#[test]
fn afinidade_e_permanente_e_reprodutivel() {
    // Mesmo par (piloto, pista) → mesmo número, sempre. É o ponto do hash.
    let a = afinidade_pista("DRV-KOWALCZYK", 523, &estilo_neutro());
    let b = afinidade_pista("DRV-KOWALCZYK", 523, &estilo_neutro());
    assert_eq!(a, b);
}

#[test]
fn afinidade_tem_sinal_e_varia_por_pista() {
    // O mesmo piloto voa numa pista e apanha na outra — e isso vale todo ano.
    let estilo = estilo_neutro();
    let valores: Vec<f64> = [523, 166, 413, 586, 239, 95, 168, 249]
        .iter()
        .map(|t| afinidade_pista("DRV-KOWALCZYK", *t, &estilo))
        .collect();
    assert!(valores.iter().any(|v| *v > 0.3), "{valores:?}");
    assert!(valores.iter().any(|v| *v < -0.3), "{valores:?}");
}

#[test]
fn afinidade_varia_por_piloto_na_mesma_pista() {
    let estilo = estilo_neutro();
    let a = afinidade_pista("DRV-001", 523, &estilo);
    let b = afinidade_pista("DRV-002", 523, &estilo);
    assert!((a - b).abs() > 1e-9);
}

#[test]
fn afinidade_respeita_a_amplitude_maxima() {
    let estilo = EstiloPiloto {
        smoothness: 100.0,
        consistencia: 100.0,
        adaptabilidade: 100.0,
        aggression: 100.0,
    };
    for i in 0..500 {
        let id = format!("DRV-{i:04}");
        for track in [523, 166, 413, 249, 554] {
            let v = afinidade_pista(&id, track, &estilo);
            let teto = AFINIDADE_ESCALA_PONTOS * TETO_SIGMAS;
            assert!(
                v.abs() <= teto + 1e-9,
                "{id}/{track} = {v} passou de {teto}"
            );
        }
    }
}

#[test]
fn estilo_puxa_a_afinidade_na_direcao_do_carater_da_pista() {
    // Spa (Flowing) premia fluidez: o mesmo piloto, com smoothness alta, tem
    // afinidade MAIOR lá do que teria com smoothness baixa.
    let fluido = EstiloPiloto {
        smoothness: 95.0,
        ..estilo_neutro()
    };
    let travado = EstiloPiloto {
        smoothness: 5.0,
        ..estilo_neutro()
    };
    let spa_fluido = afinidade_pista("DRV-X", 523, &fluido);
    let spa_travado = afinidade_pista("DRV-X", 523, &travado);
    assert!(spa_fluido > spa_travado, "{spa_fluido} vs {spa_travado}");

    // E o efeito é do CARÁTER, não da pista: em Hungaroring (Tight, que premia
    // consistência) a smoothness não muda nada.
    let hungaro_fluido = afinidade_pista("DRV-X", 413, &fluido);
    let hungaro_travado = afinidade_pista("DRV-X", 413, &travado);
    assert!((hungaro_fluido - hungaro_travado).abs() < 1e-9);
}

#[test]
fn afinidade_tem_o_desvio_padrao_da_escala() {
    // A constante é σ, não teto: num grid grande de estilos neutros, o desvio da
    // afinidade tem que bater com AFINIDADE_ESCALA_PONTOS. Sem isso, "±3 pontos"
    // não quer dizer nada.
    let estilo = estilo_neutro();
    let amostras: Vec<f64> = (0..4000)
        .map(|i| afinidade_pista(&format!("DRV-{i:05}"), 523, &estilo))
        .collect();
    let media: f64 = amostras.iter().sum::<f64>() / amostras.len() as f64;
    let sigma =
        (amostras.iter().map(|v| (v - media).powi(2)).sum::<f64>() / amostras.len() as f64).sqrt();
    // Estilo neutro zera o pedaço de estilo, então sobra √0,65 da escala.
    let esperado = AFINIDADE_ESCALA_PONTOS * AFINIDADE_FRACAO_IDIOSSINCRATICA.sqrt();
    assert!(
        (sigma - esperado).abs() < esperado * 0.1,
        "σ = {sigma}, esperado ≈ {esperado}"
    );
}

#[test]
fn afinidade_e_centrada_no_zero_no_grid_inteiro() {
    // Não é bônus disfarçado: na média do grid, ela não move o nível da categoria.
    let estilo = estilo_neutro();
    let n = 400;
    let media: f64 = (0..n)
        .map(|i| afinidade_pista(&format!("DRV-{i:04}"), 523, &estilo))
        .sum::<f64>()
        / n as f64;
    assert!(media.abs() < 0.30, "média = {media}");
}

// ─────────────────────────── Camada 2: forma ───────────────────────────

#[test]
fn forma_e_reprodutivel_para_a_mesma_etapa() {
    let s = semente_forma(3, 7, "DRV-001");
    assert_eq!(
        proxima_forma(0.4, s, 70.0, 60.0),
        proxima_forma(0.4, s, 70.0, 60.0)
    );
}

#[test]
fn forma_fica_no_intervalo() {
    let mut f = 0.0;
    for rodada in 0..200 {
        f = proxima_forma(f, semente_forma(1, rodada, "DRV-001"), 100.0, 100.0);
        assert!(
            (-TETO_SIGMAS..=TETO_SIGMAS).contains(&f),
            "rodada {rodada}: {f}"
        );
    }
}

#[test]
fn forma_tem_memoria_e_gera_sequencias() {
    // O ponto do AR(1): valores consecutivos são CORRELACIONADOS. Uma série
    // i.i.d. teria correlação ≈ 0; esta tem que ficar claramente positiva.
    let mut serie = Vec::new();
    let mut f = 0.0;
    for rodada in 0..600 {
        f = proxima_forma(f, semente_forma(1, rodada, "DRV-SERIE"), 50.0, 50.0);
        serie.push(f);
    }
    let media: f64 = serie.iter().sum::<f64>() / serie.len() as f64;
    let (mut cov, mut var) = (0.0, 0.0);
    for par in serie.windows(2) {
        cov += (par[0] - media) * (par[1] - media);
        var += (par[0] - media).powi(2);
    }
    let correlacao = cov / var;
    assert!(
        correlacao > 0.35,
        "correlação de lag 1 = {correlacao}, esperado próximo de ρ = {FORMA_RHO}"
    );
}

#[test]
fn forma_relaxa_para_o_centro_sem_ruido_novo() {
    // Sem empurrão, uma fase boa se dissolve — não fica presa no piloto.
    let mut f: f64 = 1.0;
    for _ in 0..10 {
        f *= FORMA_RHO;
    }
    assert!(f.abs() < 0.02, "{f}");
}

#[test]
fn animo_esta_desligado_por_padrao_mas_o_mecanismo_existe() {
    // O termo de ânimo foi ZERADO por default: ele era um terceiro caminho para motivação e
    // confiança, que já entram no resultado por `motivation_pace_delta` (elo próprio da
    // esteira) e pelo peso 0,20 da confiança no trecho `Finish`. Ver a doc de
    // `FORMA_PESO_ANIMO`.
    let media = |mot: f64, conf: f64, peso: f64| {
        let mut f = 0.0;
        let mut soma = 0.0;
        for rodada in 0..400 {
            f = proxima_forma_com_escalas(
                f,
                semente_forma(9, rodada, "DRV-ANIMO"),
                mot,
                conf,
                FORMA_RHO,
                peso,
            );
            soma += f;
        }
        soma / 400.0
    };

    // Com o default (0,0), motivação não desloca a forma — é o conserto da contagem dupla.
    assert_eq!(FORMA_PESO_ANIMO, 0.0);
    assert!(
        (media(95.0, 95.0, FORMA_PESO_ANIMO) - media(10.0, 10.0, FORMA_PESO_ANIMO)).abs() < 1e-9,
        "com o peso zerado a motivação não pode mais deslocar a forma"
    );

    // Mas o mecanismo continua parametrizado, porque o harness precisa medir a contaminação
    // (0,20 contra 0,0) antes do recongelamento do baseline.
    assert!(
        media(95.0, 95.0, 0.20) > media(10.0, 10.0, 0.20),
        "com peso explícito o deslocamento tem que voltar a existir"
    );
}

/// **Guarda contra a componente permanente voltando por qualquer caminho.**
///
/// Não existe campo `sigma_permanente` de propósito: com o ânimo zerado ele seria sempre zero,
/// e campo sempre-zero convida uso errado depois. No lugar dele, este teste — que pega uma
/// componente permanente reentrando por um caminho que ninguém previu, o que um campo não
/// pegaria. Tornar o erro inexprimível em vez de documentá-lo.
#[test]
fn sigma_realizado_da_forma_bate_com_o_nominal() {
    // Dois pilotos em extremos opostos de motivação/confiança. Se houver QUALQUER termo
    // constante dentro do AR(1), as médias das duas séries divergem e o σ da população
    // (as duas juntas) estoura o nominal.
    let serie = |mot: f64, conf: f64, id: &str| -> Vec<f64> {
        let mut f = 0.0;
        (0..3000)
            .map(|rodada| {
                f = proxima_forma(f, semente_forma(11, rodada, id), mot, conf);
                forma_em_pontos(f)
            })
            .collect()
    };
    let mut amostras = serie(98.0, 98.0, "DRV-ALTO");
    amostras.extend(serie(8.0, 8.0, "DRV-BAIXO"));

    let media: f64 = amostras.iter().sum::<f64>() / amostras.len() as f64;
    let sigma =
        (amostras.iter().map(|v| (v - media).powi(2)).sum::<f64>() / amostras.len() as f64).sqrt();

    // O nominal é `FORMA_ESCALA_PONTOS` (o estado tem σ = 1 por construção). O clamp em
    // TETO_SIGMAS encolhe um pouco, então a tolerância é assimétrica para baixo.
    assert!(
        sigma <= FORMA_ESCALA_PONTOS * 1.05,
        "σ realizado {sigma:.3} passou do nominal {FORMA_ESCALA_PONTOS:.3} — voltou componente \
         permanente para dentro da forma"
    );
    assert!(
        sigma >= FORMA_ESCALA_PONTOS * 0.85,
        "σ realizado {sigma:.3} ficou muito abaixo do nominal {FORMA_ESCALA_PONTOS:.3}"
    );

    // E as duas médias têm que coincidir: é aí que um termo constante apareceria primeiro.
    //
    // Comparadas em unidades NORMALIZADAS (divididas pela escala), como as duas asserções de σ
    // acima já fazem. A primeira versão comparava em pontos contra um limite absoluto, e passou
    // por acidente enquanto a escala valia 2,0 — quando ela subiu para 3,6 na calibração, o
    // mesmo desvio relativo estourou o limite e o teste acusou deslocamento onde só havia
    // mudança de unidade.
    //
    // O tamanho da tolerância vem do ruído de amostragem, não de gosto. Um AR(1) com ρ = 0,65
    // tem tamanho efetivo de amostra `n(1−ρ)/(1+ρ)` ≈ 636, não 3000; o erro-padrão da diferença
    // entre as duas médias é `√2/√636` ≈ 0,056. A 0,20 (≈ 3,5 σ) o teste não acusa por ruído.
    //
    // E ele continua enxergando o que existe para enxergar: com `peso_animo` = 0,20, os dois
    // extremos de ânimo diferem em ~1,8 depois de normalizados, o termo constante vira
    // `0,20 × 1,8 / (1 − 0,65)` ≈ **1,03** de deslocamento — dezoito vezes a tolerância.
    const TOLERANCIA_RELATIVA: f64 = 0.20;
    let m_alto: f64 = amostras[..3000].iter().sum::<f64>() / 3000.0 / FORMA_ESCALA_PONTOS;
    let m_baixo: f64 = amostras[3000..].iter().sum::<f64>() / 3000.0 / FORMA_ESCALA_PONTOS;
    assert!(
        (m_alto - m_baixo).abs() < TOLERANCIA_RELATIVA,
        "as médias divergiram ({m_alto:.3} vs {m_baixo:.3}, normalizadas) — há deslocamento por \
         qualidade do piloto dentro de uma camada que a análise trata como serial"
    );
}

#[test]
fn forma_respeita_a_amplitude_em_pontos() {
    // O estado é normalizado em σ = 1, então 1,0 vale exatamente uma escala…
    assert_eq!(forma_em_pontos(1.0), FORMA_ESCALA_PONTOS);
    assert_eq!(forma_em_pontos(-1.0), -FORMA_ESCALA_PONTOS);
    // …e nada passa do teto em sigmas.
    assert_eq!(forma_em_pontos(99.0), FORMA_ESCALA_PONTOS * TETO_SIGMAS);
}

#[test]
fn forma_tem_desvio_padrao_de_uma_escala() {
    // A escala é σ, não teto: a série tem que ficar com desvio perto de 1 na
    // escala adimensional — é isso que dá sentido a "±2 pontos de skill".
    let mut f = 0.0;
    let mut amostras = Vec::new();
    for rodada in 0..2000 {
        f = proxima_forma(f, semente_forma(3, rodada, "DRV-SIGMA"), 50.0, 50.0);
        amostras.push(f);
    }
    let media: f64 = amostras.iter().sum::<f64>() / amostras.len() as f64;
    let sigma =
        (amostras.iter().map(|v| (v - media).powi(2)).sum::<f64>() / amostras.len() as f64).sqrt();
    assert!((0.75..=1.25).contains(&sigma), "σ = {sigma}");
}

// ─────────────────────────── Camada 3: acerto ───────────────────────────

#[test]
fn acerto_e_o_mesmo_na_corrida_inteira() {
    // A propriedade que importa: chamado quantas vezes for, no mesmo evento,
    // devolve o mesmo número. É o que faz ele sobreviver à soma dos segmentos.
    let a = acerto_fim_de_semana(2, 5, "TEAM-A", "DRV-001");
    let b = acerto_fim_de_semana(2, 5, "TEAM-A", "DRV-001");
    assert_eq!(a, b);
}

#[test]
fn acerto_muda_de_etapa_para_etapa() {
    let valores: Vec<f64> = (1..=6)
        .map(|r| acerto_fim_de_semana(2, r, "TEAM-A", "DRV-001"))
        .collect();
    assert!(valores.iter().any(|v| *v > 0.4), "{valores:?}");
    assert!(valores.iter().any(|v| *v < -0.4), "{valores:?}");
}

#[test]
fn acerto_e_majoritariamente_da_equipe_mas_separa_a_garagem() {
    // Os dois pilotos da MESMA equipe andam juntos (o carro é um só): a distância
    // entre eles tem que ser bem menor que a distância entre equipes diferentes.
    let mut soma_mesma_equipe = 0.0;
    let mut soma_equipes_diferentes = 0.0;
    let mut separados = 0;
    let rodadas = 400;
    for r in 1..=rodadas {
        let a = acerto_fim_de_semana(1, r, "TEAM-A", "DRV-001");
        let b = acerto_fim_de_semana(1, r, "TEAM-A", "DRV-002");
        let c = acerto_fim_de_semana(1, r, "TEAM-B", "DRV-003");
        assert!(
            (a - b).abs() > 1e-9,
            "os dois lados não podem ser idênticos"
        );
        soma_mesma_equipe += (a - b).abs();
        soma_equipes_diferentes += (a - c).abs();
        // …mas às vezes um acha o acerto e o outro não.
        if (a - b).abs() > 1.5 {
            separados += 1;
        }
    }
    assert!(
        soma_mesma_equipe * 1.5 < soma_equipes_diferentes,
        "mesma equipe={soma_mesma_equipe} vs equipes diferentes={soma_equipes_diferentes}"
    );
    assert!(
        separados > 0,
        "a garagem precisa se separar de vez em quando"
    );
}

#[test]
fn acerto_respeita_a_amplitude_maxima() {
    for r in 1..=24 {
        for t in 0..60 {
            let team = format!("TEAM-{t:03}");
            for d in 0..2 {
                let drv = format!("{team}-D{d}");
                let v = acerto_fim_de_semana(4, r, &team, &drv);
                let teto = ACERTO_ESCALA_PONTOS * TETO_SIGMAS;
                assert!(v.abs() <= teto + 1e-9, "{v}");
            }
        }
    }
}

// ─────────────────────────── Agregado ───────────────────────────

#[test]
fn ajuste_pesa_mais_a_afinidade_na_classificacao() {
    let estilo = estilo_neutro();
    let a = ajuste_fim_de_semana("DRV-001", "TEAM-A", 523, 1, 3, &estilo, 0.0);
    let afinidade = afinidade_pista("DRV-001", 523, &estilo);

    // O delta entre canais tem DUAS parcelas desde que o acerto passou a ser por trim. Antes só
    // existia a primeira, e o teste comparava contra ela sozinha.
    let trim = |t| {
        acerto_fim_de_semana_por_canal(1, 3, "TEAM-A", "DRV-001", ACERTO_ESCALA_PONTOS, t)
    };
    let delta_acerto = trim(TrimDeAcerto::Classificacao) - trim(TrimDeAcerto::Corrida);
    let delta = a.classificacao - a.corrida;
    assert!(
        (delta - (afinidade * (MULT_AFINIDADE_QUALI - 1.0) + delta_acerto)).abs() < 1e-9,
        "delta={delta} afinidade={afinidade} delta_acerto={delta_acerto}"
    );

    // E a parcela da afinidade continua sendo a que domina — é ela que carrega a intenção de
    // design "a volta perfeita é onde o casamento com a pista aparece inteiro".
    //
    // Afirmação de POPULAÇÃO, e não de amostra: num piloto isolado a afinidade dele pode ser
    // quase nula por acaso e o trim ganhar, sem que nada esteja errado. A primeira versão deste
    // assert olhava um piloto só e reprovava por isso.
    let (mut soma_af, mut soma_trim) = (0.0, 0.0);
    for i in 0..200 {
        let id = format!("DRV-{i:03}");
        let pista = 400 + (i as u32 % 30);
        soma_af += (afinidade_pista(&id, pista, &estilo) * (MULT_AFINIDADE_QUALI - 1.0)).abs();
        let t = |x| acerto_fim_de_semana_por_canal(1, 3, "TEAM-A", &id, ACERTO_ESCALA_PONTOS, x);
        soma_trim += (t(TrimDeAcerto::Classificacao) - t(TrimDeAcerto::Corrida)).abs();
    }
    assert!(
        soma_af > soma_trim * 0.5,
        "a assimetria de canal virou majoritariamente ruído de trim ({soma_trim:.1}) e não \
         afinidade ({soma_af:.1})"
    );
}

#[test]
fn ajuste_total_cabe_no_orcamento_das_tres_camadas() {
    let teto = TETO_SIGMAS
        * (AFINIDADE_ESCALA_PONTOS * MULT_AFINIDADE_QUALI
            + FORMA_ESCALA_PONTOS
            + ACERTO_ESCALA_PONTOS);
    for r in 1..=24 {
        let a = ajuste_fim_de_semana("DRV-007", "TEAM-C", 168, 2, r, &estilo_neutro(), 2.5);
        assert!(a.corrida.abs() <= teto && a.classificacao.abs() <= teto);
    }
}

// ─────────────────────────── Demonstração ───────────────────────────
//
// O sintoma que originou o trabalho: cinco etapas seguidas com a MESMA ordem de
// chegada. Este teste roda uma temporada sintética de grid fixo em cinco pistas
// diferentes, ANTES (só os atributos, como era) e DEPOIS (com as três camadas),
// e imprime as duas tabelas. Rode com:
//
//   cargo test --manifest-path src-tauri/Cargo.toml demonstracao_ordem -- --nocapture

#[cfg(test)]
mod demonstracao {
    use super::*;
    use crate::simulation::catalog::IncidentCatalog;
    use crate::simulation::context::{SimDriver, SimulationContext};
    use crate::simulation::engine::run_full_race_with_breakdowns;
    use crate::simulation::track_profile::get_track_simulation_data;
    use rand::{rngs::StdRng, SeedableRng};

    /// As cinco etapas da temporada sintética (id do iRacing + nome).
    const ETAPAS: [(u32, &str); 5] = [
        (523, "Spa"),
        (166, "Okayama"),
        (413, "Hungaroring"),
        (586, "Laguna Seca"),
        (239, "Monza"),
    ];

    /// Grid fixo: 12 pilotos, 6 equipes de 2. O nível cai 1 ponto por piloto (72 →
    /// 61) e os demais atributos ficam COLADOS nesse nível, com uma variação
    /// pequena e determinística — é assim que um grid de categoria de verdade se
    /// parece, apertado. Um grid com atributos espalhados ao acaso esconderia o
    /// efeito das camadas atrás de diferenças fixas enormes entre pilotos.
    fn grid() -> Vec<SimDriver> {
        (0..12)
            .map(|i| {
                let equipe = i / 2;
                let nivel = 72.0 - i as f64;
                let atributo = |k: i32| -> u8 {
                    (nivel + ((i as i32 * k) % 7) as f64 - 3.0).clamp(5.0, 99.0) as u8
                };
                SimDriver {
                    id: format!("DRV-{:02}", i + 1),
                    nome: format!("Piloto {:02}", i + 1),
                    is_jogador: false,
                    skill: nivel as u8,
                    consistencia: atributo(3),
                    racecraft: atributo(5),
                    defesa: atributo(2),
                    ritmo_classificacao: nivel as u8,
                    gestao_pneus: atributo(4),
                    habilidade_largada: atributo(6),
                    adaptabilidade: atributo(11),
                    fator_chuva: 50,
                    fitness: atributo(9),
                    experiencia: atributo(13),
                    aggression: atributo(8),
                    smoothness: atributo(10),
                    mentalidade: atributo(12),
                    confianca: atributo(15),
                    motivacao: 70.0,
                    car_performance: 78.0 - equipe as f64 * 1.2,
                    car_performance_quali: 78.0 - equipe as f64 * 1.2,
                    vies_de_pico: 0.0,
                    qualidade_de_estrategia: 50.0,
                    car_reliability: 90.0,
                    team_id: format!("TEAM-{}", equipe + 1),
                    team_name: format!("Equipe {}", equipe + 1),
                    corridas_na_categoria: 40,
                    pressure_error_mult: 1.0,
                }
            })
            .collect()
    }

    fn estilo_de(sd: &SimDriver) -> EstiloPiloto {
        EstiloPiloto {
            smoothness: sd.smoothness as f64,
            consistencia: sd.consistencia as f64,
            adaptabilidade: sd.adaptabilidade as f64,
            aggression: sd.aggression as f64,
        }
    }

    fn contexto(track_id: u32, nome: &str) -> SimulationContext {
        SimulationContext {
            track_id,
            track_name: nome.to_string(),
            track_character: get_track_simulation_data(track_id).track_character,
            ..SimulationContext::test_default()
        }
    }

    /// Ordem de chegada (ids, do P1 ao último) de uma corrida.
    fn ordem_de_chegada(
        drivers: &[SimDriver],
        track_id: u32,
        nome: &str,
        semente: u64,
    ) -> Vec<String> {
        let ctx = contexto(track_id, nome);
        let mut rng = StdRng::seed_from_u64(semente);
        let resultado = run_full_race_with_breakdowns(
            drivers,
            &ctx,
            false,
            &IncidentCatalog::empty(),
            None,
            &mut rng,
        );
        let mut linhas = resultado.race_results.clone();
        linhas.sort_by_key(|r| r.finish_position);
        linhas.into_iter().map(|r| r.pilot_id).collect()
    }

    /// Aplica as três camadas ao grid, do jeito que a esteira de
    /// `commands/race/simulacao.rs` aplica.
    fn com_camadas(
        base: &[SimDriver],
        forma: &[f64],
        track_id: u32,
        rodada: i32,
    ) -> Vec<SimDriver> {
        base.iter()
            .zip(forma)
            .map(|(sd, estado)| {
                let mut sd = sd.clone();
                let a = ajuste_fim_de_semana(
                    &sd.id,
                    &sd.team_id,
                    track_id,
                    1,
                    rodada,
                    &estilo_de(&sd),
                    *estado,
                );
                sd.skill = (sd.skill as f64 + a.corrida).clamp(5.0, 100.0).round() as u8;
                sd.ritmo_classificacao = (sd.ritmo_classificacao as f64 + a.classificacao)
                    .clamp(5.0, 100.0)
                    .round() as u8;
                sd
            })
            .collect()
    }

    fn imprimir(titulo: &str, ordens: &[(String, Vec<String>)]) {
        println!("\n=== {titulo} ===");
        for (pista, ordem) in ordens {
            let top: Vec<&str> = ordem.iter().take(12).map(|s| s.as_str()).collect();
            println!("{pista:<12} | {}", top.join(" "));
        }
        let distintas: std::collections::HashSet<&Vec<String>> =
            ordens.iter().map(|(_, o)| o).collect();
        println!(
            "{} etapas | ordens distintas: {} | formações de pódio distintas: {}",
            ordens.len(),
            distintas.len(),
            podios_distintos(ordens)
        );
    }

    #[test]
    fn demonstracao_ordem_de_chegada_muda_entre_etapas() {
        let base = grid();

        // ── ANTES: só os atributos. Cada etapa tem semente de RNG diferente e
        // ainda assim a ordem não se mexe — é esse o sintoma.
        let antes: Vec<(String, Vec<String>)> = ETAPAS
            .iter()
            .enumerate()
            .map(|(i, (id, nome))| {
                (
                    nome.to_string(),
                    ordem_de_chegada(&base, *id, nome, 1000 + i as u64),
                )
            })
            .collect();
        imprimir("ANTES (sem as camadas)", &antes);

        // ── DEPOIS: as três camadas, com a forma evoluindo entre as etapas.
        let mut forma = vec![0.0_f64; base.len()];
        let mut depois: Vec<(String, Vec<String>)> = Vec::new();
        for (i, (id, nome)) in ETAPAS.iter().enumerate() {
            let rodada = i as i32 + 1;
            for (idx, sd) in base.iter().enumerate() {
                forma[idx] = proxima_forma(
                    forma[idx],
                    semente_forma(1, rodada, &sd.id),
                    sd.motivacao,
                    sd.confianca as f64,
                );
            }
            let grid_ajustado = com_camadas(&base, &forma, *id, rodada);
            depois.push((
                nome.to_string(),
                ordem_de_chegada(&grid_ajustado, *id, nome, 1000 + i as u64),
            ));
        }
        imprimir("DEPOIS (afinidade + forma + acerto)", &depois);

        // O sintoma não é o pelotão inteiro congelado — o ruído por segmento ainda
        // troca duas posições vizinhas aqui e ali. O sintoma é QUEM sobe no pódio:
        // ANTES é quase sempre a mesma gente, e a troca de lugar entre eles é cosmética.
        //
        // Nota: este "ANTES" mede só a ausência das TRÊS CAMADAS, não o mundo original.
        // Depois que o pacote da classificação entrou (melhor de N, trim de quali do carro
        // e volta perdida), a própria linha de base já se mexe um pouco sozinha — por isso
        // aqui vale "no máximo 2 formações", e não mais exatamente 1.
        assert!(
            podios_distintos(&antes) <= 2,
            "o sintoma tem que estar presente ANTES: {} formações de pódio em 5 etapas",
            podios_distintos(&antes)
        );
        assert!(
            podios_distintos(&depois) >= 3,
            "DEPOIS o pódio tem que trocar de gente: só {} formações distintas em 5 etapas",
            podios_distintos(&depois)
        );
        // E o reembaralho não pode ser só na ponta: a ordem inteira tem que mudar.
        let ordens_depois: std::collections::HashSet<&Vec<String>> =
            depois.iter().map(|(_, o)| o).collect();
        assert!(
            ordens_depois.len() >= 4,
            "DEPOIS: só {} ordens distintas em 5 etapas",
            ordens_depois.len()
        );
    }

    /// Quantas formações de pódio DIFERENTES (como conjunto de pilotos, não como
    /// ordem entre eles) apareceram nas etapas.
    fn podios_distintos(ordens: &[(String, Vec<String>)]) -> usize {
        ordens
            .iter()
            .map(|(_, o)| {
                let mut podio: Vec<String> = o.iter().take(3).cloned().collect();
                podio.sort();
                podio
            })
            .collect::<std::collections::HashSet<_>>()
            .len()
    }
}
